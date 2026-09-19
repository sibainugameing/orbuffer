import { invoke } from "@tauri-apps/api/core";
import { FormEvent, useCallback, useEffect, useMemo, useState } from "react";

type Aria2File = {
  path?: string;
  length?: string;
  completedLength?: string;
  selected?: string;
};

type Download = {
  gid: string;
  status: "active" | "waiting" | "paused" | "error" | "complete" | "removed" | string;
  totalLength: string;
  completedLength: string;
  downloadSpeed: string;
  uploadSpeed: string;
  connections: string;
  errorCode?: string;
  errorMessage?: string;
  files?: Aria2File[];
};

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

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  return invoke<T>(command, args);
}

export default function App() {
  const [url, setUrl] = useState("");
  const [downloads, setDownloads] = useState<Download[]>([]);
  const [running, setRunning] = useState(false);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("aria2 is not started by OrBuffer yet.");
  const [error, setError] = useState("");

  const refresh = useCallback(async () => {
    try {
      setError("");
      const result = await call<Download[]>("aria2_active");
      setDownloads(result);
      setRunning(true);
    } catch (reason) {
      setRunning(false);
      setError(String(reason));
    }
  }, []);

  useEffect(() => {
    void refresh();
    const timer = window.setInterval(() => void refresh(), 1000);
    return () => window.clearInterval(timer);
  }, [refresh]);

  const activeCount = useMemo(
    () => downloads.filter((download) => download.status === "active").length,
    [downloads],
  );

  async function startAria2() {
    try {
      setBusy(true);
      setError("");
      await call<boolean>("aria2_start", { port: 6800 });
      setMessage("aria2 RPC is running on localhost:6800.");
      await refresh();
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  }

  async function addDownload(event: FormEvent) {
    event.preventDefault();
    const value = url.trim();
    if (!value) return;

    try {
      setBusy(true);
      setError("");
      await call<string>("aria2_add", { uri: value });
      setUrl("");
      setMessage("Download added to aria2.");
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
      await call<string>(command, { gid });
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
            <div className="hero-number">{activeCount}</div>
            <p>active downloads</p>
          </div>
          <button className="secondary-button" onClick={() => void startAria2()} disabled={busy && !running}>
            {running ? "Reconnect" : "Start aria2"}
          </button>
        </section>

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
          </form>
        </section>

        <section className="downloads-section">
          <div className="section-heading">
            <div>
              <div className="eyebrow">DOWNLOADS</div>
              <h2>Activity</h2>
            </div>
            <button className="text-button" onClick={() => void refresh()} disabled={busy}>
              Refresh
            </button>
          </div>

          {error && <div className="notice error">{error}</div>}
          {!error && message && <div className="notice">{message}</div>}

          {downloads.length === 0 ? (
            <div className="empty-state">
              <div className="empty-title">Nothing active</div>
              <p>Add a URL above and aria2 will handle the transfer.</p>
            </div>
          ) : (
            <div className="download-list">
              {downloads.map((download) => {
                const percent = progress(download);
                return (
                  <article className="download-card" key={download.gid}>
                    <div className="download-main">
                      <div className="download-title">{filename(download)}</div>
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
                        {download.status === "active" && (
                          <button onClick={() => void control(download.gid, "aria2_pause")}>Pause</button>
                        )}
                        {download.status === "paused" && (
                          <button onClick={() => void control(download.gid, "aria2_resume")}>Resume</button>
                        )}
                        <button onClick={() => void control(download.gid, "aria2_remove")}>Remove</button>
                      </div>
                    </div>
                  </article>
                );
              })}
            </div>
          )}
        </section>
      </main>
    </div>
  );
}
