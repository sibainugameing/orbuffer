# OrBuffer

OrBuffer is an open-source download-manager project. The desktop application uses a Tauri 2 + React + TypeScript interface and delegates transfer work to aria2.

## Current prototype

The project has two main paths:

1. A Rust command-line launcher for a single download.
2. A Tauri desktop GUI backed by an aria2 JSON-RPC client.

aria2 owns the actual transfer work, including continuation, retries, segmented connections, queueing, and progress. OrBuffer does not implement a second HTTP download engine.

## Requirements

For the Rust prototype:

- Rust toolchain (Cargo)
- aria2 (aria2c) installed and available on PATH
- For RPC commands, an aria2 instance with JSON-RPC enabled

For the desktop GUI:

- Node.js 22 or newer
- Rust toolchain
- Linux desktop development dependencies required by Tauri on Linux
- aria2 (aria2c) installed and available on PATH

## Direct download

```sh
cargo run -- "https://example.com/file.zip"
```

Choose an explicit output file:

```sh
cargo run -- "https://example.com/file.zip" "./file.zip"
```

The launcher passes continuation, overwrite protection, automatic filename collision handling, retry, timeout, and progress options to aria2.

## aria2 JSON-RPC

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

The RPC endpoint defaults to http://127.0.0.1:6800/jsonrpc. Override it with ORBUFFER_ARIA2_RPC.

## Desktop development

Install frontend dependencies:

```sh
npm install
```

Build the frontend:

```sh
npm run build
```

Run the frontend test suite:

```sh
npm test
```

Start the Tauri desktop application in development mode:

```sh
npm run tauri dev
```

The desktop prototype starts a local aria2 RPC process when needed. aria2 session state is stored in the Tauri app-data directory as aria2.session and loaded on startup.

The Settings panel exposes aria2's concurrent-download limit, split count, per-server connection limit, and minimum split size. These values are saved locally by the frontend and applied when OrBuffer starts an aria2 session. They do not retroactively change options of already-running downloads.

The Add Download form accepts an optional save directory and filename. These fields are saved locally for convenience.

## Supported URLs

The launcher and RPC add command accept:

- HTTP
- HTTPS
- FTP
- FTPS
- SFTP
- magnet

Actual protocol support depends on the installed aria2 build.

## Testing and CI

The CI workflow is intentionally not run on every push to main.

It runs for:

- pull requests that change application files
- v* tags
- v* verification branches used by connected automation
- manual dispatches

The CI checks:

- frontend production build
- frontend component and interaction tests
- Rust formatting
- Rust unit tests
- Tauri Rust formatting
- Tauri Rust tests
- a Tauri package build for v* verification branches

Networked downloads through a real aria2 instance are not yet part of the automated test suite.

## Release flow

Pushing a real v* Git tag starts the Release workflow.

The Release workflow uses Tauri Action to build Linux packages configured by src-tauri/tauri.conf.json and creates a draft GitHub Release with the generated packages attached. The normal v* tag path does not duplicate that package build in CI.

The repository currently targets:

- Debian package (.deb)
- AppImage

Release packaging is configured, but a real tag-triggered package release has not yet been verified.

## Architecture

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

See [MVP.md](docs/MVP.md) for the MVP scope and acceptance criteria. See [COMMANDS.md](docs/COMMANDS.md) for the Tauri command contracts. See [TODO.md](TODO.md) for the implementation roadmap.
