# OrBuffer MVP

## Scope

The initial MVP is a desktop download manager built around aria2.

The MVP includes:

- Tauri 2 desktop application
- React + TypeScript frontend
- aria2 JSON-RPC control
- OrBuffer-managed local aria2 process
- persistent aria2 session restore
- HTTP, HTTPS, FTP, FTPS, SFTP, and magnet URL validation
- download queue status and progress
- pause, resume, and remove controls
- completed/error queue cleanup
- configurable concurrent downloads and segmented transfer settings
- optional save directory and output filename
- Linux x86_64 package targets: Debian package and AppImage

The MVP does not include:

- cloud synchronization
- browser extensions
- download scheduling
- bandwidth scheduling
- checksum verification UI
- cross-device queue synchronization
- macOS or Windows packaging
- automated real-network download integration tests

## Acceptance criteria

### Application startup

- OrBuffer starts without requiring a separately launched aria2 process when aria2c is installed.
- If an aria2 RPC endpoint is already running on the configured localhost port, OrBuffer can connect to it without claiming ownership.
- If OrBuffer starts aria2, it shuts that process down when the application exits.

### Queue management

- A supported URL can be added from the desktop UI.
- The queue displays active, waiting, paused, complete, error, and removed states when returned by aria2.
- A download can be paused, resumed, and removed from the UI.
- Finished results can be cleared without affecting active or waiting downloads.

### Persistence

- aria2 session state is written to the Tauri application data directory.
- Unfinished session entries can be loaded when OrBuffer starts a new local aria2 process.
- User transfer settings and the last save-directory/output values are retained locally by the frontend.

### Transfer settings

- The UI can configure maximum concurrent downloads.
- The UI can configure split count.
- The UI can configure maximum connections per server.
- The UI can configure minimum split size.
- Invalid aria2 process parameters are rejected by the Rust backend.

### Validation and errors

- Unsupported URL schemes are rejected before an aria2 add operation.
- Invalid RPC endpoints are rejected by the RPC client.
- JSON-RPC errors are surfaced as stable backend errors.
- Frontend loading, empty, and error states are represented in the UI.

### Verification

The repository currently has automated coverage for:

- frontend build
- frontend component/interaction tests
- Rust formatting
- Rust unit tests
- Tauri Rust formatting
- Tauri Rust tests
- RPC request/response behavior through a local mock server

Real networked aria2 integration and release packaging remain separate verification tasks.
