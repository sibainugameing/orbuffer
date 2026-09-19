import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import App from "./App";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

const mockedInvoke = vi.mocked(invoke);

const queue = [
  {
    gid: "gid-active",
    status: "active",
    totalLength: "2048",
    completedLength: "1024",
    downloadSpeed: "2048",
    uploadSpeed: "0",
    connections: "2",
    files: [{ path: "/tmp/example.zip" }],
  },
  {
    gid: "gid-complete",
    status: "complete",
    totalLength: "4096",
    completedLength: "4096",
    downloadSpeed: "0",
    uploadSpeed: "0",
    connections: "0",
    files: [{ path: "/tmp/done.zip" }],
  },
];

beforeEach(() => {
  localStorage.clear();
  vi.spyOn(window, "setInterval").mockReturnValue(1);
  vi.spyOn(window, "clearInterval").mockImplementation(() => {});

  mockedInvoke.mockImplementation(async (command) => {
    switch (command) {
      case "aria2_start":
        return false;
      case "aria2_queue":
        return queue;
      case "aria2_global":
        return { downloadSpeed: "2048" };
      case "aria2_add":
        return "gid-new";
      default:
        return "OK";
    }
  });
});

afterEach(() => {
  vi.restoreAllMocks();
  mockedInvoke.mockReset();
});

async function renderReadyApp() {
  render(<App />);
  await waitFor(() => {
    expect(screen.getByText("example.zip")).toBeTruthy();
  });
}

describe("OrBuffer", () => {
  test("renders queue information and opens settings", async () => {
    await renderReadyApp();

    expect(screen.getByText("Downloading")).toBeTruthy();
    expect(screen.getByText("2.00 KiB/s")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Settings" }));

    expect(screen.getByText("Transfer tuning")).toBeTruthy();
    expect(screen.getByDisplayValue("20M")).toBeTruthy();
  });

  test("filters the queue by completion status", async () => {
    await renderReadyApp();

    fireEvent.click(screen.getByRole("button", { name: "1 complete" }));

    expect(screen.getByText("done.zip")).toBeTruthy();
    expect(screen.queryByText("example.zip")).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "1 complete" }));

    expect(screen.getByText("example.zip")).toBeTruthy();
  });

  test("submits a download with directory and filename options", async () => {
    await renderReadyApp();

    const urlInput = screen.getByPlaceholderText("https://example.com/file.zip");
    const directoryInput = screen.getByPlaceholderText("~/Downloads");
    const outputInput = screen.getByPlaceholderText("Leave empty for automatic name");

    fireEvent.change(urlInput, {
      target: { value: "https://example.com/archive.zip" },
    });
    fireEvent.change(directoryInput, {
      target: { value: "/tmp/downloads" },
    });
    fireEvent.change(outputInput, {
      target: { value: "archive.zip" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Add" }));

    await waitFor(() => {
      expect(screen.getByText("Download added to aria2.")).toBeTruthy();
    });

    expect(mockedInvoke).toHaveBeenCalledWith("aria2_add", {
      uri: "https://example.com/archive.zip",
      directory: "/tmp/downloads",
      output: "archive.zip",
    });
  });
});
