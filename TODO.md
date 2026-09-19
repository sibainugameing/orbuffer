# OrBuffer — TODO

> Status: planning checklist. Items are unchecked until implemented and verified.

## Project definition

- [ ] Define the MVP scope and acceptance criteria.
- [ ] Confirm supported operating systems and minimum system requirements.
- [ ] Document architecture: Rust download engine, Tauri 2 desktop shell, React + TypeScript UI.

## Phase 1 — Application foundation

- [ ] Create the Tauri 2 + React + TypeScript project structure.
- [ ] Establish Rust/TypeScript command and event contracts.
- [ ] Add SQLite persistence and database migrations.
- [ ] Add application logging and structured error reporting.

## Phase 2 — Download MVP

- [ ] Accept and validate an HTTP/HTTPS URL.
- [ ] Select a destination directory and filename.
- [ ] Implement a reliable single-stream download.
- [ ] Display progress, downloaded bytes, transfer speed, and estimated time remaining when calculable.
- [ ] Implement pause/stop, resume, and cancel behavior.
- [ ] Persist download state and restore the queue after restart.
- [ ] Handle HTTP errors, network interruptions, retries, and invalid URLs.
- [ ] Handle filename conflicts and incomplete temporary files safely.
- [ ] Detect server support for byte ranges before attempting segmented downloads.
- [ ] Implement configurable multi-connection Range downloads with safe fallback to single-stream mode.
- [ ] Verify completed downloads where size or integrity information is available.

## Phase 3 — Desktop GUI

- [ ] Build the Graphite background and Cyan accent visual system.
- [ ] Add polished transitions and responsive interaction states.
- [ ] Implement Overview dashboard.
- [ ] Implement Downloads queue and status filters.
- [ ] Implement Add Download flow.
- [ ] Implement a download Details panel.
- [ ] Implement Settings, including default save location and download behavior.
- [ ] Implement Analytics for download activity and transfer statistics.
- [ ] Add accessible labels, keyboard navigation, and empty/loading/error states.

## Phase 4 — Quality and release

- [ ] Add Rust unit tests for URL handling, state transitions, retries, and Range logic.
- [ ] Add frontend component and interaction tests.
- [ ] Test pause/resume across application restarts and network failures.
- [ ] Test servers that do not support Range requests and servers with unknown file sizes.
- [ ] Check resource usage and performance with large downloads and multiple tasks.
- [ ] Update README with features, setup, development, and troubleshooting instructions.
- [ ] Add licensing and third-party dependency notices as needed.
- [ ] Configure release builds and platform packaging.
- [ ] Document known limitations and the initial release checklist.

## Backlog / later

- [ ] Evaluate optional checksum verification and user-provided hashes.
- [ ] Evaluate download scheduling and bandwidth limits.
- [ ] Evaluate browser integration or clipboard URL detection.
- [ ] Evaluate additional protocols only after HTTP/HTTPS is stable.
