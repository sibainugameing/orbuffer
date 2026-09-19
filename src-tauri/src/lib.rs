use std::{sync::Mutex, thread::sleep, time::Duration};

use serde_json::Value;
use tauri::{AppHandle, Manager, RunEvent, State};
use url::Url;

mod aria2;

const DEFAULT_ENDPOINT: &str = "http://127.0.0.1:6800/jsonrpc";
const DEFAULT_PORT: u16 = 6800;

struct AppState {
    client: Mutex<aria2::Aria2Client>,
    process: Mutex<Option<aria2::Aria2Process>>,
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
        }
    }
}

#[tauri::command]
fn aria2_start(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    port: Option<u16>,
    directory: Option<String>,
) -> Result<bool, String> {
    let port = port.unwrap_or(DEFAULT_PORT);
    let secret = std::env::var("ORBUFFER_ARIA2_SECRET")
        .ok()
        .filter(|value| !value.is_empty());

    let mut process = state
        .process
        .lock()
        .map_err(|_| "failed to lock aria2 process state".to_string())?;

    if let Some(existing) = process.as_mut() {
        if existing
            .is_running()
            .map_err(|error| error.to_string())?
        {
            return Ok(false);
        }
    }

    let endpoint = format!("http://127.0.0.1:{port}/jsonrpc");
    let client = aria2::Aria2Client::new(&endpoint, secret.clone())
        .map_err(|error| error.to_string())?;

    if client.get_global_stat().is_ok() {
        *state
            .client
            .lock()
            .map_err(|_| "failed to lock aria2 client state".to_string())? = client;
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
            return Ok(true);
        }
        sleep(Duration::from_millis(50));
    }

    Err("aria2c started but its JSON-RPC endpoint did not become ready".to_string())
}

#[tauri::command]
fn aria2_add(
    state: State<'_, AppState>,
    uri: String,
    directory: Option<String>,
    output: Option<String>,
) -> Result<String, String> {
    validate_url(&uri)?;
    state
        .client
        .lock()
        .map_err(|_| "failed to lock aria2 client state".to_string())?
        .add_uri(&uri, directory.as_deref(), output.as_deref())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn aria2_active(state: State<'_, AppState>) -> Result<Value, String> {
    state
        .client
        .lock()
        .map_err(|_| "failed to lock aria2 client state".to_string())?
        .tell_active()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn aria2_queue(state: State<'_, AppState>) -> Result<Value, String> {
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
            Value::Array(items) => downloads.extend(items),
            _ => return Err("aria2 returned a non-array queue response".to_string()),
        }
    }

    Ok(Value::Array(downloads))
}

#[tauri::command]
fn aria2_status(state: State<'_, AppState>, gid: String) -> Result<Value, String> {
    state
        .client
        .lock()
        .map_err(|_| "failed to lock aria2 client state".to_string())?
        .tell_status(&gid)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn aria2_pause(state: State<'_, AppState>, gid: String) -> Result<String, String> {
    state
        .client
        .lock()
        .map_err(|_| "failed to lock aria2 client state".to_string())?
        .pause(&gid)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn aria2_resume(state: State<'_, AppState>, gid: String) -> Result<String, String> {
    state
        .client
        .lock()
        .map_err(|_| "failed to lock aria2 client state".to_string())?
        .unpause(&gid)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn aria2_remove(state: State<'_, AppState>, gid: String) -> Result<String, String> {
    state
        .client
        .lock()
        .map_err(|_| "failed to lock aria2 client state".to_string())?
        .remove(&gid)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn aria2_global(state: State<'_, AppState>) -> Result<Value, String> {
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
            aria2_global,
        ])
        .build(tauri::generate_context!())
        .expect("error while building OrBuffer")
        .run(|app, event| {
            if matches!(event, RunEvent::Exit) {
                let state = app.state::<AppState>();
                if let Ok(client) = state.client.lock() {
                    let _ = client.shutdown();
                }
                if let Ok(mut process) = state.process.lock() {
                    process.take();
                }
            }
        });
}
