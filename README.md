# OrBuffer

OrBuffer is an open-source download-manager project. The planned desktop application uses a Tauri 2 + React + TypeScript interface and delegates transfer work to aria2.

## Current prototype

The Rust prototype now has two paths:

1. A direct launcher that starts `aria2c` for a single download.
2. An aria2 JSON-RPC client for queue management.

aria2 owns the actual transfer work, including continuation, retries, segmented connections, and progress. OrBuffer does not implement a second HTTP download engine.

### Requirements

- Rust toolchain (Cargo)
- aria2 (`aria2c`) installed and available on `PATH`
- For RPC commands, an aria2 instance with JSON-RPC enabled

### Direct download

```sh
cargo run -- "https://example.com/file.zip"
```

Choose an explicit output path:

```sh
cargo run -- "https://example.com/file.zip" "./file.zip"
```

The launcher passes `--continue=true`, disables overwriting, enables automatic filename collision handling, configures up to five attempts, and shows aria2's console progress.

### aria2 JSON-RPC

Start a local-only aria2 RPC server:

```sh
aria2c \
  --enable-rpc=true \
  --rpc-listen-all=false \
  --rpc-listen-port=6800
```

With a secret:

```sh
aria2c \
  --enable-rpc=true \
  --rpc-listen-all=false \
  --rpc-listen-port=6800 \
  --rpc-secret=change-this-secret
```

When a secret is configured, set the same value for OrBuffer:

```sh
export ORBUFFER_ARIA2_SECRET="change-this-secret"
```

Add a download:

```sh
cargo run -- rpc add "https://example.com/file.zip"
```

Inspect active downloads:

```sh
cargo run -- rpc active
```

Inspect one download:

```sh
cargo run -- rpc status <gid>
```

Pause, resume, or remove a download:

```sh
cargo run -- rpc pause <gid>
cargo run -- rpc resume <gid>
cargo run -- rpc remove <gid>
```

The RPC endpoint defaults to `http://127.0.0.1:6800/jsonrpc`. Override it with `ORBUFFER_ARIA2_RPC`.

### Supported URLs

The launcher and RPC add command accept HTTP, HTTPS, FTP, FTPS, SFTP, and magnet URLs. Actual support depends on the installed aria2 build.

### Verification

The earlier launcher commit passed the Rust GitHub Actions formatting and test workflow. The RPC implementation has its own Rust unit tests; the corresponding GitHub Actions run is the authoritative build check. Networked aria2 integration is not yet tested in CI.

## Architecture direction

```
React + TypeScript
        |
     Tauri 2
        |
    Rust commands
        |
  aria2 JSON-RPC
        |
      aria2c
        |
      network
```

The next major step is to move the existing Rust prototype into the standard Tauri project layout and expose these RPC operations as Tauri commands. Tauri's current documentation uses a top-level frontend project with a `src-tauri/` Rust project and documents invoking Rust commands from the frontend. citeturn650746search0turn650746search3

See [TODO.md](TODO.md) for the roadmap.
