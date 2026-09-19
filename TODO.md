# OrBuffer — TODO

> Status: implementation roadmap. Items are marked complete only after implementation and verification.

## Project definition

- [ ] Define the MVP scope and acceptance criteria.
- [ ] Confirm supported operating systems and minimum system requirements.
- [x] Document the target architecture: Tauri 2 + React + TypeScript UI, Rust integration layer, and aria2 download engine.

## Phase 1 — Application foundation

- [ ] Create the standard Tauri 2 + React + TypeScript project structure.
- [ ] Establish Rust/TypeScript command and event contracts.
- [ ] Add SQLite persistence and database migrations.
- [ ] Add application logging and structured error reporting.

## Phase 2 — aria2 integration

- [x] Accept and validate supported download URLs.
- [x] Launch aria2 for direct downloads.
- [x] Add an aria2 JSON-RPC client.
- [x] Support add, status, active, waiting, stopped, pause, resume, remove, and global-stat RPC operations.
- [ ] Keep aria2 RPC bound to localhost by default.
- [ ] Start and supervise the aria2 RPC process from the desktop application.
- [ ] Define the download settings passed from the UI to aria2.
- [x] Configure aria2 segmented downloads and concurrent-download limits.
- [ ] Persist queue metadata and restore it after application restart.
- [ ] Map aria2 states and errors to stable UI-facing states.
- [ ] Verify completed downloads where size or integrity information is available.

## Phase 3 — Desktop GUI

- [ ] Build the Graphite background and Cyan accent visual system.
- [ ] Add polished transitions and responsive interaction states.
- [ ] Implement Overview dashboard.
- [x] Implement Downloads queue and status filters.
- [ ] Implement Add Download flow.
- [x] Implement a download Details panel.
- [ ] Implement Settings, including default save location and aria2 behavior.
- [ ] Implement Analytics for download activity and transfer statistics.
- [ ] Add accessible labels, keyboard navigation, and empty/loading/error states.

## Phase 4 — Quality and release

- [x] Add Rust unit tests for URL validation and RPC endpoint validation.
- [ ] Add RPC request/response tests with a local mock server.
- [ ] Add frontend component and interaction tests.
- [ ] Test pause/resume across application restarts and network failures.
- [ ] Test servers that do not support Range requests and servers with unknown file sizes through aria2.
- [ ] Check resource usage and performance with large downloads and multiple tasks.
- [ ] Update README with features, setup, development, and troubleshooting instructions.
- [ ] Add licensing and third-party dependency notices as needed.
- [ ] Configure release builds and platform packaging.
- [ ] Document known limitations and the initial release checklist.

## Backlog / later

- [ ] Evaluate optional checksum verification and bandwidth limits.
- [ ] Evaluate download scheduling.
- [ ] Evaluate browser integration or clipboard URL detection.
- [ ] Evaluate additional protocols only after HTTP/HTTPS is stable.
