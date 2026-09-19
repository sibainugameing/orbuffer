# OrBuffer

OrBuffer is an open-source download-manager project. The planned desktop application uses a Tauri 2 + React + TypeScript interface and delegates transfer work to aria2.

## Current prototype

The Rust command-line prototype validates a URL and launches `aria2c`. aria2 handles the actual transfer, continuation/resume metadata, retries, and progress output. OrBuffer currently acts as a small launcher; it is not yet a graphical interface or an aria2 RPC client.

### Requirements

- Rust toolchain (Cargo)
- aria2 (`aria2c`) installed and available on `PATH`

### Run

```sh
cargo run -- "https://example.com/file.zip"
```

Choose an explicit output path:

```sh
cargo run -- "https://example.com/file.zip" "./file.zip"
```

The launcher passes `--continue=true`, disables overwriting, enables automatic filename collision handling, configures up to five attempts, and shows aria2's console progress. If aria2 is missing, OrBuffer prints an installation/PATH hint.

Supported URL schemes accepted by the launcher: HTTP, HTTPS, FTP, FTPS, SFTP, and magnet. Actual protocol support depends on the installed aria2 build.

**Verification status:** The aria2 launcher change has been committed, but it has not yet been compiled or tested in this environment. Check the GitHub Actions result before treating it as verified.

## Roadmap

The next integration step is an aria2 RPC service so the desktop UI can add, inspect, pause, resume, and remove downloads without parsing console output. The planned GUI is Tauri 2 + React + TypeScript.

See [TODO.md](TODO.md) for the roadmap.
