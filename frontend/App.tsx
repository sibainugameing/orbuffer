import { invoke } from "@tauri-apps/api/core";
import { FormEvent, useCallback, useEffect, useMemo, useState } from "react";
import type { Aria2CommandMap, Download } from "./contracts";

function formatBytes(value: string): string {
  const bytes = Number(value);
  if (!Number.isFinite(bytes) || bytes < 0) return "—";

  const units = ["B", "KiB", "MiB", "GiB", "TiB"];
  let current = bytes;
  let index = 0;

  while (current >= 1024 && index < units.length - 1) {
    current /= 1024;
    index += 1;
  }

  const digits = current >= 100 || index === 0 ? 0 : current >= 10 ? 1 : 2;
  return `${current.toFixed(digits)} ${units[index]}`;
}

function formatSpeed(value: string): string {
  const bytes = Number(value);
  return Number.isFinite(bytes) && bytes > 0 ? `${formatBytes(value)}/s` : "—";
}

function progress(download: Download): number {
  const total = Number(download.totalLength);
  const completed = Number(download.completedLength);
  if (!Number.isFinite(total) || total <= 0) return 0;
  return Math.min(100, Math.max(0, (completed / total) * 100));
}

function filename(download: Download): string {
  const path = download.files?.[0]?.path;
  if (!path) return `GID ${download.gid}`;
  return path.split(/[\\/]/).pop() || path;
}

function verificationLabel(verification: Download["verification"]): string {
  switch (verification) {
    case "verified":
      return "Verified";
    case "mismatch":
      return "Size mismatch";
    case "unavailable":
      return "Unavailable";
    default:
      return "Not checked";
  }
}

function statusLabel(status: Download["status"]): string {
  switch (status) {
    case "active":
      return "Downloading";
    case "waiting":
      return "Waiting";
    case "paused":
      return "Paused";
    case "complete":
      return "Complete";
    case "error":
      return "Error";
    case "removed":
      return "Removed";
    default:
      return status;
  }
}

async function call<K extends keyof Aria2CommandMap>(
  command: K,
  args?: Aria2CommandMap[K]["args"],
): Promise<Aria2CommandMap[K]["result"]> {
  return invoke<Aria2CommandMap[K]["result"]>(command, args);
}

export default function App() {
  const [url, setUrl] = useState("");
  const [directory, setDirectory] = useState("");
  const [output, setOutput] = useState("");
  const [downloads, setDownloads] = useState<Download[]>([]);
  const [globalSpeed, setGlobalSpeed] = useState("—");
  const [running, setRunning] = useState(false);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("Preparing aria2…");
  const [error, setError] = useState("");
  const [filter, setFilter] = useState<"all" | "active" | "waiting" | "paused" | "complete" | "error">("all");
  const [selectedGid, setSelectedGid] = useState<string | null>(null);
  const [selectedDownload, setSelectedDownload] = useState<Download | null>(null);
  const [showSettings, setShowSettings] = useState(false);
  const [settings, setSettings] = useState({
    maxConcurrentDownloads: 3,
    split: 4,
    maxConnectionPerServer: 4,
    minSplitSize: "20M",
  });

  const refresh = useCallback(async () => {
    try {
      setError("");
      const [result, global] = await Promise.all([
        call("aria2_queue"),
        call("aria2_global"),
      ]);
      setDownloads(result);
      setGlobalSpeed(formatSpeed(global.downloadSpeed));
      setRunning(true);
    } catch (reason) {
      setRunning(false);
      setError(String(reason));
    }
  }, []);

  useEffect(() => {
    const saved = localStorage.getItem("orbuffer.download-settings");
    if (saved) {
      try {
        const parsed = JSON.parse(saved);
        setSettings((current) => ({
          ...current,
          ...parsed,
        }));
        setDirectory(typeof parsed.directory === "string" ? parsed.directory : "");
        setOutput(typeof parsed.output === "string" ? parsed.output : "");
      } catch {
        // Ignore malformed local settings and keep defaults.
      }
    }
  }, []);

  useEffect(() => {
    const initialize = async () => {
      try {
        const saved = localStorage.getItem("orbuffer.download-settings");
        const settings = saved ? JSON.parse(saved) : {};
        await call("aria2_start", {
          port: 6800,
          directory:
            typeof settings.directory === "string" && settings.directory.trim()
              ? settings.directory.trim()
              : null,
          maxConcurrentDownloads: Number(settings.maxConcurrentDownloads) || 3,
          split: Number(settings.split) || 4,
          maxConnectionPerServer: Number(settings.maxConnectionPerServer) || 4,
          minSplitSize:
            typeof settings.minSplitSize === "string" && settings.minSplitSize.trim()
              ? settings.minSplitSize.trim()
              : "20M",
        });
      } catch {
        // An already-running aria2 instance is fine; refresh below will connect to it.
      }
      await refresh();
    };

    void initialize();
    const timer = window.setInterval(() => void refresh(), 1000);
    return () => window.clearInterval(timer);
  }, [refresh]);

  const filteredDownloads = useMemo(
    () =>
      downloads.filter((download) => {
        if (filter === "all") return true;
        return download.status === filter;
      }),
    [downloads, filter],
  );

  const stats = useMemo(
    () => ({
      active: downloads.filter((download) => download.status === "active").length,
      waiting: downloads.filter((download) => download.status === "waiting").length,
      paused: downloads.filter((download) => download.status === "paused").length,
      complete: downloads.filter((download) => download.status === "complete").length,
      error: downloads.filter((download) => download.status === "error").length,
      removed: downloads.filter((download) => download.status === "removed").length,
    }),
    [downloads],
  );

  useEffect(() => {
    if (!selectedGid) {
      setSelectedDownload(null);
      return;
    }

    const current = downloads.find((download) => download.gid === selectedGid);
    if (current) {
      setSelectedDownload(current);
      return;
    }

    void call("aria2_status", { gid: selectedGid })
      .then(setSelectedDownload)
      .catch(() => setSelectedDownload(null));
  }, [downloads, selectedGid]);

  async function openDetails(gid: string) {
    try {
      setSelectedGid(gid);
      setError("");
      const detail = await call("aria2_status", { gid });
      setSelectedDownload(detail);
    } catch (reason) {
      setError(String(reason));
    }
  }

  async function startAria2() {
    try {
      setBusy(true);
      setError("");
      localStorage.setItem(
        "orbuffer.download-settings",
        JSON.stringify({ ...settings, directory, output }),
      );
      await call("aria2_start", {
        port: 6800,
        directory: directory.trim() || null,
        maxConcurrentDownloads: settings.maxConcurrentDownloads,
        split: settings.split,
        maxConnectionPerServer: settings.maxConnectionPerServer,
        minSplitSize: settings.minSplitSize,
      });
      setMessage("Settings saved. They apply the next time OrBuffer starts aria2.");
      await refresh();
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  }

  function updateSetting(key: keyof typeof settings, value: string | number) {
    setSettings((current) => ({
      ...current,
      [key]: value,
    }));
  }

  async function addDownload(event: FormEvent) {
    event.preventDefault();
    const value = url.trim();
    if (!value) return;

    try {
      setBusy(true);
      setError("");
      await call("aria2_add", {
        uri: value,
        directory: directory.trim() || null,
        output: output.trim() || null,
      });
      setUrl("");
      setMessage("Download added to aria2.");
      await refresh();
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  }

  async function clearFinished() {
    try {
      setBusy(true);
      setError("");
      const removed = await call("aria2_clear_finished");
      setMessage(`${removed} finished download${removed === 1 ? "" : "s"} cleared.`);
      if (selectedGid) {
        const selected = downloads.find((download) => download.gid === selectedGid);
        if (selected && ["complete", "error", "removed"].includes(selected.status)) {
          setSelectedGid(null);
        }
      }
      await refresh();
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  }

  async function control(gid: string, command: "aria2_pause" | "aria2_resume" | "aria2_remove") {
    try {
      setError("");
      await call(command, { gid });
      await refresh();
    } catch (reason) {
      setError(String(reason));
    }
  }

  return (
    <div className="app-shell">
      <header className="topbar">
        <div>
          <div className="eyebrow">DOWNLOAD MANAGER</div>
          <h1>OrBuffer</h1>
        </div>
        <div className={`engine-state ${running ? "online" : "offline"}`}>
          <span className="status-dot" />
          aria2 {running ? "connected" : "offline"}
        </div>
      </header>

      <main className="content">
        <section className="hero-card">
          <div>
            <div className="eyebrow">QUEUE</div>
            <div className="hero-number">{stats.active}</div>
            <p>active downloads</p>
          </div>
          <div className="hero-metric">
            <div className="eyebrow">TOTAL SPEED</div>
            <strong>{globalSpeed}</strong>
            <p>aria2 download rate</p>
          </div>
          <div className="hero-actions">
            <button className="secondary-button" onClick={() => setShowSettings((value) => !value)}>
              {showSettings ? "Hide settings" : "Settings"}
            </button>
            <button className="secondary-button" onClick={() => void startAria2()} disabled={busy}>
              {running ? "Save settings" : "Start aria2"}
            </button>
          </div>
        </section>

        {showSettings && (
          <section className="settings-card">
            <div className="section-heading">
              <div>
                <div className="eyebrow">ARIA2 SETTINGS</div>
                <h2>Transfer tuning</h2>
              </div>
            </div>

            <div className="settings-grid">
              <label>
                <span>Concurrent downloads</span>
                <input
                  type="number"
                  min="1"
                  max="64"
                  value={settings.maxConcurrentDownloads}
                  onChange={(event) => updateSetting("maxConcurrentDownloads", Number(event.target.value))}
                />
              </label>

              <label>
                <span>Split connections</span>
                <input
                  type="number"
                  min="1"
                  max="32"
                  value={settings.split}
                  onChange={(event) => updateSetting("split", Number(event.target.value))}
                />
              </label>

              <label>
                <span>Connections / server</span>
                <input
                  type="number"
                  min="1"
                  max="32"
                  value={settings.maxConnectionPerServer}
                  onChange={(event) => updateSetting("maxConnectionPerServer", Number(event.target.value))}
                />
              </label>

              <label>
                <span>Minimum split size</span>
                <input
                  value={settings.minSplitSize}
                  onChange={(event) => updateSetting("minSplitSize", event.target.value)}
                  placeholder="20M"
                  spellCheck={false}
                />
              </label>
            </div>

            <p className="settings-note">
              Settings are stored locally. They apply to the next aria2 start; existing downloads keep their current options.
            </p>
          </section>
        )}

        <section className="add-card">
          <div className="section-heading">
            <div>
              <div className="eyebrow">ADD DOWNLOAD</div>
              <h2>Paste a URL</h2>
            </div>
          </div>

          <form className="add-form" onSubmit={addDownload}>
            <input
              value={url}
              onChange={(event) => setUrl(event.target.value)}
              placeholder="https://example.com/file.zip"
              spellCheck={false}
              autoFocus
            />
            <button className="primary-button" disabled={busy || !url.trim()}>
              Add
            </button>

            <div className="add-options">
              <label>
                <span>Save directory</span>
                <input
                  value={directory}
                  onChange={(event) => setDirectory(event.target.value)}
                  placeholder="~/Downloads"
                  spellCheck={false}
                />
              </label>

              <label>
                <span>Filename (optional)</span>
                <input
                  value={output}
                  onChange={(event) => setOutput(event.target.value)}
                  placeholder="Leave empty for automatic name"
                  spellCheck={false}
                />
              </label>
            </div>
          </form>
        </section>

        <section className="downloads-section">
          <div className="section-heading">
            <div>
              <div className="eyebrow">DOWNLOADS</div>
              <h2>Activity</h2>
            </div>
            <div className="section-actions">
              {stats.complete + stats.error + stats.removed > 0 && (
                <button className="text-button" onClick={() => void clearFinished()} disabled={busy}>
                  Clear finished
                </button>
              )}
              <button className="text-button" onClick={() => void refresh()} disabled={busy}>
                Refresh
              </button>
            </div>
          </div>

          <div className="stats-row">
            <button className={`stat-pill ${filter === "active" ? "selected" : ""}`} onClick={() => setFilter(filter === "active" ? "all" : "active")}>
              <strong>{stats.active}</strong> active
            </button>
            <button className={`stat-pill ${filter === "waiting" ? "selected" : ""}`} onClick={() => setFilter(filter === "waiting" ? "all" : "waiting")}>
              <strong>{stats.waiting}</strong> waiting
            </button>
            <button className={`stat-pill ${filter === "paused" ? "selected" : ""}`} onClick={() => setFilter(filter === "paused" ? "all" : "paused")}>
              <strong>{stats.paused}</strong> paused
            </button>
            <button className={`stat-pill ${filter === "complete" ? "selected" : ""}`} onClick={() => setFilter(filter === "complete" ? "all" : "complete")}>
              <strong>{stats.complete}</strong> complete
            </button>
            <button className={`stat-pill ${filter === "error" ? "selected" : ""}`} onClick={() => setFilter(filter === "error" ? "all" : "error")}>
              <strong>{stats.error}</strong> error
            </button>
          </div>

          {error && <div className="notice error">{error}</div>}
          {!error && message && <div className="notice">{message}</div>}

          {downloads.length === 0 ? (
            <div className="empty-state">
              <div className="empty-title">No downloads yet</div>
              <p>Add a URL above and aria2 will handle the transfer.</p>
            </div>
          ) : filteredDownloads.length === 0 ? (
            <div className="empty-state">
              <div className="empty-title">No matching downloads</div>
              <p>Choose another status filter.</p>
            </div>
          ) : (
            <div className="download-list">
              {filteredDownloads.map((download) => {
                const percent = progress(download);
                return (
                  <article className="download-card" key={download.gid}>
                    <div className="download-main">
                      <button className="download-title-button" onClick={() => void openDetails(download.gid)} title="Open download details">
                      <span className="download-title">{filename(download)}</span>
                    </button>
                      <div className="download-meta">
                        <span>{statusLabel(download.status)}</span>
                        <span>{formatBytes(download.completedLength)} / {formatBytes(download.totalLength)}</span>
                        <span>{formatSpeed(download.downloadSpeed)}</span>
                        <span>{download.connections} conn.</span>
                      </div>
                    </div>

                    <div className="progress-track">
                      <div className="progress-fill" style={{ width: `${percent}%` }} />
                    </div>

                    <div className="download-footer">
                      <span>{percent.toFixed(1)}%</span>
                      <div className="actions">
                        {(download.status === "active" || download.status === "waiting") && (
                          <button onClick={() => void control(download.gid, "aria2_pause")}>Pause</button>
                        )}
                        {download.status === "paused" && (
                          <button onClick={() => void control(download.gid, "aria2_resume")}>Resume</button>
                        )}
                        {download.status !== "complete" && download.status !== "removed" && (
                          <button onClick={() => void control(download.gid, "aria2_remove")}>Remove</button>
                        )}
                      </div>
                    </div>
                  </article>
                );
              })}
            </div>
          )}

          {selectedDownload && (
            <aside className="details-card">
              <div className="section-heading">
                <div>
                  <div className="eyebrow">DETAILS</div>
                  <h2>{filename(selectedDownload)}</h2>
                </div>
                <button className="text-button" onClick={() => setSelectedGid(null)}>
                  Close
                </button>
              </div>

              <div className="details-grid">
                <div>
                  <span>Status</span>
                  <strong>{statusLabel(selectedDownload.status)}</strong>
                </div>
                <div>
                  <span>Progress</span>
                  <strong>{progress(selectedDownload).toFixed(1)}%</strong>
                </div>
                <div>
                  <span>Downloaded</span>
                  <strong>{formatBytes(selectedDownload.completedLength)} / {formatBytes(selectedDownload.totalLength)}</strong>
                </div>
                <div>
                  <span>Download speed</span>
                  <strong>{formatSpeed(selectedDownload.downloadSpeed)}</strong>
                </div>
                <div>
                  <span>Connections</span>
                  <strong>{selectedDownload.connections || "—"}</strong>
                </div>
                {selectedDownload.status === "complete" && (
                  <div>
                    <span>File check</span>
                    <strong>{verificationLabel(selectedDownload.verification)}</strong>
                  </div>
                )}
                <div>
                  <span>GID</span>
                  <strong>{selectedDownload.gid}</strong>
                </div>
              </div>

              {selectedDownload.files?.[0]?.path && (
                <div className="detail-path">
                  <span>Path</span>
                  <code>{selectedDownload.files[0].path}</code>
                </div>
              )}

              {selectedDownload.errorMessage && (
                <div className="notice error">
                  aria2 error {selectedDownload.errorCode || "unknown"}: {selectedDownload.errorMessage}
                </div>
              )}
            </aside>
          )}
        </section>
      </main>
    </div>
  );
}
