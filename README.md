# OrBuffer

OrBuffer is an open-source download-manager project. The planned desktop application uses a Rust download engine with a Tauri 2 + React + TypeScript interface.

## Current prototype

The repository contains a Rust command-line downloader for HTTP and HTTPS URLs. It streams response bytes to a temporary file, prints byte progress (and a percentage when the server reports a content length), and renames a completed file to the destination.

If a transfer is interrupted, the `.orbuffer-part` file is kept. Running the same command again attempts to resume with an HTTP `Range` request. The server must honor the requested range and return a valid `206 Partial Content` response; otherwise the program restarts from byte zero. Existing completed destination files are not overwritten.

**Verification status:** The resume implementation has been committed but has not yet been compiled or tested in this environment. Treat it as experimental.

### Requirements

- Rust toolchain (Cargo)

### Run

```sh
cargo run -- "https://example.com/file.zip"
```

Choose an explicit output path:

```sh
cargo run -- "https://example.com/file.zip" "./file.zip"
```

Use the same command and destination path to retry a partial transfer.

This prototype does not yet provide a desktop GUI, user-controlled pause/resume, retries, SQLite persistence, segmented downloads, or a complete recovery strategy for changed remote files.

See [TODO.md](TODO.md) for the roadmap.
