# OrBuffer

OrBuffer is an open-source download-manager project. The planned desktop application uses a Rust download engine with a Tauri 2 + React + TypeScript interface.

## Current prototype

The repository currently contains an initial Rust command-line downloader for HTTP and HTTPS URLs. It streams response bytes to a temporary file, prints byte progress (and a percentage when the server reports a content length), then renames the completed file to the destination.

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

The program refuses to overwrite an existing destination. This prototype does not yet provide a desktop GUI, pause/resume, retries, SQLite persistence, segmented downloads, or a complete cleanup/recovery strategy for interrupted temporary files.

See [TODO.md](TODO.md) for the roadmap.
