# OrBuffer SQLite database

OrBuffer stores application-side download metadata in SQLite under the Tauri application data directory:

```
orbuffer.db
```

The database is created when the desktop application starts. SQLite is bundled through the Rust dependency, so users do not need to install a separate SQLite runtime.

## Migration model

Schema changes are tracked in:

```text
schema_migrations(version, applied_at)
```

Migrations are applied in ascending version order and run inside a SQLite transaction. The current schema version is `1`.

Migration 1 creates:

- `downloads` — one row per aria2 GID, including the original add metadata when available and the latest observed aria2 status.
- `settings` — key/value storage reserved for application settings that will move out of frontend-only storage.

Indexes cover download status and the last update timestamp.

## Download persistence

When a download is added successfully, OrBuffer stores its GID, URL, directory, filename, and initial state.

Queue and status refreshes update the persisted snapshot with:

- status
- total and completed length
- download/upload speed
- connection count
- error information
- completed-file verification result
- file metadata as JSON
- creation and update timestamps

Clearing finished results from aria2 does not delete the SQLite record. This keeps the application metadata available for future history and analytics features.

## Relationship with aria2 session data

SQLite is application metadata storage. It is not a replacement for aria2's transfer/session state.

OrBuffer continues to use:

```
aria2.session
```

for aria2 queue restoration. SQLite records are synchronized from the live aria2 state whenever OrBuffer reads the queue or an individual download status.

This separation means aria2 remains responsible for transfers and resumable download state, while OrBuffer owns application metadata and future history/analytics data.
