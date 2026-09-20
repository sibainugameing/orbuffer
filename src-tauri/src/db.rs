use std::{
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::{params, Connection, Transaction};
use serde_json::Value;

const CURRENT_SCHEMA_VERSION: i64 = 1;

const MIGRATION_1: &str = r#"
CREATE TABLE IF NOT EXISTS downloads (
    gid TEXT PRIMARY KEY,
    uri TEXT,
    directory TEXT,
    output TEXT,
    status TEXT NOT NULL,
    total_length TEXT NOT NULL DEFAULT '0',
    completed_length TEXT NOT NULL DEFAULT '0',
    download_speed TEXT NOT NULL DEFAULT '0',
    upload_speed TEXT NOT NULL DEFAULT '0',
    connections TEXT NOT NULL DEFAULT '0',
    error_code TEXT,
    error_message TEXT,
    verification TEXT,
    files_json TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_downloads_status
    ON downloads(status);

CREATE INDEX IF NOT EXISTS idx_downloads_updated_at
    ON downloads(updated_at);

CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value_json TEXT NOT NULL,
    updated_at INTEGER NOT NULL
);
"#;

pub struct Database {
    connection: Connection,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self, rusqlite::Error> {
        let connection = Connection::open(path)?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "foreign_keys", "ON")?;

        let mut database = Self { connection };
        database.migrate()?;
        Ok(database)
    }

    #[cfg(test)]
    fn open_in_memory() -> Result<Self, rusqlite::Error> {
        let connection = Connection::open_in_memory()?;
        let mut database = Self { connection };
        database.migrate()?;
        Ok(database)
    }

    fn migrate(&mut self) -> Result<(), rusqlite::Error> {
        self.connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                version INTEGER PRIMARY KEY,
                applied_at INTEGER NOT NULL
            );",
        )?;

        let current_version: i64 = self.connection.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |row| row.get(0),
        )?;

        if current_version < CURRENT_SCHEMA_VERSION {
            let transaction = self.connection.transaction()?;
            transaction.execute_batch(MIGRATION_1)?;
            transaction.execute(
                "INSERT INTO schema_migrations(version, applied_at) VALUES (?1, ?2)",
                params![CURRENT_SCHEMA_VERSION, unix_timestamp()],
            )?;
            transaction.commit()?;
        }

        Ok(())
    }

    pub fn record_added(
        &mut self,
        gid: &str,
        uri: &str,
        directory: Option<&str>,
        output: Option<&str>,
    ) -> Result<(), rusqlite::Error> {
        let now = unix_timestamp();

        self.connection.execute(
            "INSERT INTO downloads (
                gid, uri, directory, output, status, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, 'waiting', ?5, ?5)
            ON CONFLICT(gid) DO UPDATE SET
                uri = excluded.uri,
                directory = excluded.directory,
                output = excluded.output,
                updated_at = excluded.updated_at",
            params![gid, uri, directory, output, now],
        )?;

        Ok(())
    }

    pub fn sync_downloads(&mut self, downloads: &[Value]) -> Result<(), rusqlite::Error> {
        let transaction = self.connection.transaction()?;

        for download in downloads {
            let Some(gid) = string_field(download, "gid") else {
                continue;
            };
            upsert_download(&transaction, gid, download)?;
        }

        transaction.commit()
    }

    pub fn mark_removed(&mut self, gid: &str) -> Result<(), rusqlite::Error> {
        self.connection.execute(
            "UPDATE downloads
             SET status = 'removed', updated_at = ?2
             WHERE gid = ?1",
            params![gid, unix_timestamp()],
        )?;
        Ok(())
    }

    #[cfg(test)]
    fn schema_version(&self) -> Result<i64, rusqlite::Error> {
        self.connection.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |row| row.get(0),
        )
    }

    #[cfg(test)]
    fn download_row(
        &self,
        gid: &str,
    ) -> Result<(Option<String>, String, String, Option<String>), rusqlite::Error> {
        self.connection.query_row(
            "SELECT uri, status, completed_length, files_json
             FROM downloads
             WHERE gid = ?1",
            params![gid],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
    }
}

fn upsert_download(
    transaction: &Transaction<'_>,
    gid: &str,
    download: &Value,
) -> Result<(), rusqlite::Error> {
    let now = unix_timestamp();
    let status = string_field(download, "status").unwrap_or("unknown");
    let total_length = string_field(download, "totalLength").unwrap_or("0");
    let completed_length = string_field(download, "completedLength").unwrap_or("0");
    let download_speed = string_field(download, "downloadSpeed").unwrap_or("0");
    let upload_speed = string_field(download, "uploadSpeed").unwrap_or("0");
    let connections = string_field(download, "connections").unwrap_or("0");
    let error_code = string_field(download, "errorCode");
    let error_message = string_field(download, "errorMessage");
    let verification = string_field(download, "verification");
    let files_json = download
        .get("files")
        .and_then(|files| serde_json::to_string(files).ok());

    transaction.execute(
        "INSERT INTO downloads (
            gid, status, total_length, completed_length, download_speed,
            upload_speed, connections, error_code, error_message,
            verification, files_json, created_at, updated_at
        ) VALUES (
            ?1, ?2, ?3, ?4, ?5,
            ?6, ?7, ?8, ?9,
            ?10, ?11, ?12, ?12
        )
        ON CONFLICT(gid) DO UPDATE SET
            status = excluded.status,
            total_length = excluded.total_length,
            completed_length = excluded.completed_length,
            download_speed = excluded.download_speed,
            upload_speed = excluded.upload_speed,
            connections = excluded.connections,
            error_code = excluded.error_code,
            error_message = excluded.error_message,
            verification = excluded.verification,
            files_json = excluded.files_json,
            updated_at = excluded.updated_at",
        params![
            gid,
            status,
            total_length,
            completed_length,
            download_speed,
            upload_speed,
            connections,
            error_code,
            error_message,
            verification,
            files_json,
            now,
        ],
    )?;

    Ok(())
}

fn string_field<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

fn unix_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_are_applied_idempotently() {
        let mut database = Database::open_in_memory().unwrap();

        assert_eq!(database.schema_version().unwrap(), CURRENT_SCHEMA_VERSION);

        database.migrate().unwrap();

        assert_eq!(database.schema_version().unwrap(), CURRENT_SCHEMA_VERSION);
    }

    #[test]
    fn removed_download_is_marked_removed() {
        let mut database = Database::open_in_memory().unwrap();

        database
            .record_added("gid-removed", "https://example.com/file.zip", None, None)
            .unwrap();

        let download = serde_json::json!({
            "gid": "gid-removed",
            "status": "complete",
            "totalLength": "4096",
            "completedLength": "4096",
            "downloadSpeed": "0",
            "uploadSpeed": "0",
            "connections": "0"
        });
        database.sync_downloads(&[download]).unwrap();
        database.mark_removed("gid-removed").unwrap();

        let (_, status, _, _) = database.download_row("gid-removed").unwrap();
        assert_eq!(status, "removed");
    }

    #[test]
    fn added_and_synced_download_is_persisted() {
        let mut database = Database::open_in_memory().unwrap();

        database
            .record_added(
                "gid-123",
                "https://example.com/file.zip",
                Some("/tmp/downloads"),
                Some("file.zip"),
            )
            .unwrap();

        let download = serde_json::json!({
            "gid": "gid-123",
            "status": "complete",
            "totalLength": "4096",
            "completedLength": "4096",
            "downloadSpeed": "0",
            "uploadSpeed": "0",
            "connections": "0",
            "verification": "verified",
            "files": [{"path": "/tmp/downloads/file.zip", "length": "4096"}]
        });

        database.sync_downloads(&[download]).unwrap();

        let (uri, status, completed_length, files_json) = database.download_row("gid-123").unwrap();

        assert_eq!(uri.as_deref(), Some("https://example.com/file.zip"));
        assert_eq!(status, "complete");
        assert_eq!(completed_length, "4096");
        let persisted_files: Value = serde_json::from_str(files_json.as_deref().unwrap()).unwrap();
        assert_eq!(
            persisted_files,
            serde_json::json!([{"path": "/tmp/downloads/file.zip", "length": "4096"}])
        );
    }
}
