use std::{path::PathBuf, sync::Mutex, thread::sleep, time::Duration};

use serde_json::Value;
use tauri::{AppHandle, Manager, RunEvent, State};
use url::Url;

mod aria2;

const DEFAULT_ENDPOINT: &str = "http://127.0.0.1:6800/jsonrpc";
const DEFAULT_PORT: u16 = 6800;

#[derive(Clone)]
struct Aria2LaunchConfig {
    port: u16,
    secret: Option<String>,
    directory: Option<PathBuf>,
    session_file: PathBuf,
    max_concurrent_downloads: u32,
    split: u32,
    max_connection_per_server: u32,
    min_split_size: String,
}

struct AppState {
    client: Mutex<aria2::Aria2Client>,
    process: Mutex<Option<aria2::Aria2Process>>,
    launch_config: Mutex<Option<Aria2LaunchConfig>>,
}

impl AppState {
    fn new() -> Self {
        let secret = std::env::var("ORBUFFER_ARIA2_SECRET")
            .ok()
            .filter(|value| !value.is_empty());
        let client = aria2::Aria2Client::new(DEFAULT_ENDPOINT, secret)
            .expect("default aria2 endpoint must be valid");

        Self {
            client: Mutex::new(client),
            process: Mutex::new(None),
            launch_config: Mutex::new(None),
        }
    }
}

#[tauri::command]
fn aria2_start(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    port: Option<u16>,
    directory: Option<String>,
    max_concurrent_downloads: Option<u32>,
    split: Option<u32>,
    max_connection_per_server: Option<u32>,
    min_split_size: Option<String>,
) -> Result<bool, String> {
    let port = port.unwrap_or(DEFAULT_PORT);
    let max_concurrent_downloads = max_concurrent_downloads.unwrap_or(3);
    let split = split.unwrap_or(4);
    let max_connection_per_server = max_connection_per_server.unwrap_or(4);
    let min_split_size = min_split_size.unwrap_or_else(|| "20M".to_string());

    let secret = std::env::var("ORBUFFER_ARIA2_SECRET")
        .ok()
        .filter(|value| !value.is_empty());

    let mut process = state
        .process
        .lock()
        .map_err(|_| "failed to lock aria2 process state".to_string())?;

    if let Some(existing) = process.as_mut() {
        if existing.is_running().map_err(|error| error.to_string())? {
            return Ok(false);
        }
    }

    let endpoint = format!("http://127.0.0.1:{port}/jsonrpc");
    let client =
        aria2::Aria2Client::new(&endpoint, secret.clone()).map_err(|error| error.to_string())?;

    if client.get_global_stat().is_ok() {
        *state
            .client
            .lock()
            .map_err(|_| "failed to lock aria2 client state".to_string())? = client;
        *state
            .launch_config
            .lock()
            .map_err(|_| "failed to lock aria2 launch configuration".to_string())? = None;
        return Ok(false);
    }

    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|error| format!("failed to resolve app data directory: {error}"))?;
    std::fs::create_dir_all(&app_data_dir)
        .map_err(|error| format!("failed to create app data directory: {error}"))?;
    let session_file = app_data_dir.join("aria2.session");

    let directory_path = directory.as_deref().map(std::path::Path::new);
    let child = aria2::Aria2Process::start(
        port,
        secret.as_deref(),
        directory_path,
        Some(&session_file),
        true,
        max_concurrent_downloads,
        split,
        max_connection_per_server,
        &min_split_size,
    )
    .map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            "aria2c was not found. Install aria2 and ensure aria2c is on PATH.".to_string()
        } else {
            format!("failed to start aria2c: {error}")
        }
    })?;

    for _ in 0..40 {
        if client.get_global_stat().is_ok() {
            *state
                .client
                .lock()
                .map_err(|_| "failed to lock aria2 client state".to_string())? = client;
            *process = Some(child);
            *state
                .launch_config
                .lock()
                .map_err(|_| "failed to lock aria2 launch configuration".to_string())? =
                Some(Aria2LaunchConfig {
                    port,
                    secret,
                    directory: directory_path.map(PathBuf::from),
                    session_file,
                    max_concurrent_downloads,
                    split,
                    max_connection_per_server,
                    min_split_size,
                });
            return Ok(true);
        }
        sleep(Duration::from_millis(50));
    }

    Err("aria2c started but its JSON-RPC endpoint did not become ready".to_string())
}

fn ensure_owned_aria2(app_handle: &AppHandle, state: &AppState) -> Result<(), String> {
    {
        let mut process = state
            .process
            .lock()
            .map_err(|_| "failed to lock aria2 process state".to_string())?;

        if let Some(existing) = process.as_mut() {
            if existing
                .is_running()
                .map_err(|error| format!("failed to inspect aria2 process: {error}"))?
            {
                return Ok(());
            }

            process.take();
        }
    }

    let config = state
        .launch_config
        .lock()
        .map_err(|_| "failed to lock aria2 launch configuration".to_string())?
        .clone();

    let Some(config) = config else {
        return Ok(());
    };

    let endpoint = format!("http://127.0.0.1:{}/jsonrpc", config.port);
    let client = aria2::Aria2Client::new(&endpoint, config.secret.clone())
        .map_err(|error| error.to_string())?;

    if client.get_global_stat().is_ok() {
        return Ok(());
    }

    let child = aria2::Aria2Process::start(
        config.port,
        config.secret.as_deref(),
        config.directory.as_deref(),
        Some(&config.session_file),
        true,
        config.max_concurrent_downloads,
        config.split,
        config.max_connection_per_server,
        &config.min_split_size,
    )
    .map_err(|error| format!("failed to restart aria2c: {error}"))?;

    for _ in 0..40 {
        if client.get_global_stat().is_ok() {
            *state
                .process
                .lock()
                .map_err(|_| "failed to lock aria2 process state".to_string())? = Some(child);
            return Ok(());
        }

        sleep(Duration::from_millis(50));
    }

    Err("aria2c restart was attempted but its JSON-RPC endpoint did not become ready".to_string())
}

#[tauri::command]
fn aria2_add(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    uri: String,
    directory: Option<String>,
    output: Option<String>,
) -> Result<String, String> {
    ensure_owned_aria2(&app_handle, state.inner())?;
    validate_url(&uri)?;
    state
        .client
        .lock()
        .map_err(|_| "failed to lock aria2 client state".to_string())?
        .add_uri(&uri, directory.as_deref(), output.as_deref())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn aria2_active(app_handle: AppHandle, state: State<'_, AppState>) -> Result<Value, String> {
    ensure_owned_aria2(&app_handle, state.inner())?;

    state
        .client
        .lock()
        .map_err(|_| "failed to lock aria2 client state".to_string())?
        .tell_active()
        .map_err(|error| error.to_string())
}

fn annotate_verification(mut download: Value) {
    if download.get("status").and_then(Value::as_str) == Some("complete") {
        let verification = verify_completed_files(&download);
        if let Some(object) = download.as_object_mut() {
            object.insert(
                "verification".to_string(),
                Value::String(verification.to_string()),
            );
        }
    }

    download
}

fn verify_completed_files(download: &Value) -> &'static str {
    let Some(files) = download.get("files").and_then(Value::as_array) else {
        return "unavailable";
    };

    if files.is_empty() {
        return "unavailable";
    }

    for file in files {
        let Some(path) = file.get("path").and_then(Value::as_str) else {
            return "unavailable";
        };
        let Some(length) = file
            .get("length")
            .and_then(Value::as_str)
            .and_then(|value| value.parse::<u64>().ok())
        else {
            return "unavailable";
        };

        match std::fs::metadata(path) {
            Ok(metadata) if metadata.len() == length => {}
            Ok(_) | Err(_) => return "mismatch",
        }
    }

    "verified"
}

#[tauri::command]
fn aria2_queue(app_handle: AppHandle, state: State<'_, AppState>) -> Result<Value, String> {
    ensure_owned_aria2(&app_handle, state.inner())?;

    let client = state
        .client
        .lock()
        .map_err(|_| "failed to lock aria2 client state".to_string())?;

    let mut downloads = Vec::new();

    for result in [
        client.tell_active(),
        client.tell_waiting(0, 1000),
        client.tell_stopped(0, 1000),
    ] {
        match result.map_err(|error| error.to_string())? {
            Value::Array(items) => {
                downloads.extend(items.into_iter().map(annotate_verification));
            }
            _ => return Err("aria2 returned a non-array queue response".to_string()),
        }
    }

    Ok(Value::Array(downloads))
}

#[tauri::command]
fn aria2_status(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    gid: String,
) -> Result<Value, String> {
    ensure_owned_aria2(&app_handle, state.inner())?;

    state
        .client
        .lock()
        .map_err(|_| "failed to lock aria2 client state".to_string())?
        .tell_status(&gid)
        .map(annotate_verification)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn aria2_pause(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    gid: String,
) -> Result<String, String> {
    ensure_owned_aria2(&app_handle, state.inner())?;

    state
        .client
        .lock()
        .map_err(|_| "failed to lock aria2 client state".to_string())?
        .pause(&gid)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn aria2_resume(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    gid: String,
) -> Result<String, String> {
    ensure_owned_aria2(&app_handle, state.inner())?;

    state
        .client
        .lock()
        .map_err(|_| "failed to lock aria2 client state".to_string())?
        .unpause(&gid)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn aria2_remove(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    gid: String,
) -> Result<String, String> {
    ensure_owned_aria2(&app_handle, state.inner())?;

    state
        .client
        .lock()
        .map_err(|_| "failed to lock aria2 client state".to_string())?
        .remove(&gid)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn aria2_clear_finished(app_handle: AppHandle, state: State<'_, AppState>) -> Result<u64, String> {
    ensure_owned_aria2(&app_handle, state.inner())?;

    let client = state
        .client
        .lock()
        .map_err(|_| "failed to lock aria2 client state".to_string())?;

    let stopped = client
        .tell_stopped(0, 1000)
        .map_err(|error| error.to_string())?;

    let items = stopped
        .as_array()
        .ok_or_else(|| "aria2 returned a non-array stopped queue".to_string())?;

    let mut removed = 0;
    for item in items {
        let status = item.get("status").and_then(Value::as_str);
        if matches!(status, Some("complete" | "error" | "removed")) {
            if let Some(gid) = item.get("gid").and_then(Value::as_str) {
                client
                    .remove_download_result(gid)
                    .map_err(|error| error.to_string())?;
                removed += 1;
            }
        }
    }

    Ok(removed)
}

#[tauri::command]
fn aria2_global(app_handle: AppHandle, state: State<'_, AppState>) -> Result<Value, String> {
    ensure_owned_aria2(&app_handle, state.inner())?;

    state
        .client
        .lock()
        .map_err(|_| "failed to lock aria2 client state".to_string())?
        .get_global_stat()
        .map_err(|error| error.to_string())
}

fn validate_url(uri: &str) -> Result<(), String> {
    let url = Url::parse(uri).map_err(|error| error.to_string())?;
    if !matches!(
        url.scheme(),
        "http" | "https" | "ftp" | "ftps" | "sftp" | "magnet"
    ) {
        return Err(format!("unsupported URL scheme: {}", url.scheme()));
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            aria2_start,
            aria2_add,
            aria2_active,
            aria2_queue,
            aria2_status,
            aria2_pause,
            aria2_resume,
            aria2_remove,
            aria2_clear_finished,
            aria2_global,
        ])
        .build(tauri::generate_context!())
        .expect("error while building OrBuffer")
        .run(|app, event| {
            if matches!(event, RunEvent::Exit) {
                let state = app.state::<AppState>();
                let owns_aria2 = state
                    .process
                    .lock()
                    .map(|process| process.is_some())
                    .unwrap_or(false);

                if owns_aria2 {
                    if let Ok(client) = state.client.lock() {
                        let _ = client.shutdown();
                    }
                    if let Ok(mut process) = state.process.lock() {
                        process.take();
                    }
                    if let Ok(mut config) = state.launch_config.lock() {
                        config.take();
                    }
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completed_download_is_marked_verified_when_file_sizes_match() {
        let path = std::env::temp_dir().join(format!(
            "orbuffer-verify-{}-{}.bin",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let body = b"verified";
        std::fs::write(&path, body).unwrap();

        let download = serde_json::json!({
            "status": "complete",
            "files": [{
                "path": path.to_string_lossy(),
                "length": body.len().to_string()
            }]
        });

        assert_eq!(verify_completed_files(&download), "verified");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn completed_download_is_marked_mismatch_when_file_size_differs() {
        let path = std::env::temp_dir().join(format!(
            "orbuffer-verify-mismatch-{}-{}.bin",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&path, b"wrong").unwrap();

        let download = serde_json::json!({
            "status": "complete",
            "files": [{
                "path": path.to_string_lossy(),
                "length": "999"
            }]
        });

        assert_eq!(verify_completed_files(&download), "mismatch");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn verification_is_unavailable_without_file_metadata() {
        let download = serde_json::json!({
            "status": "complete",
            "files": [{
                "path": "/tmp/file.bin"
            }]
        });

        assert_eq!(verify_completed_files(&download), "unavailable");
    }
}
