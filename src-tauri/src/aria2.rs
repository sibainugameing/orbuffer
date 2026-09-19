use std::{
    path::Path,
    process::{Child, Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
};

use reqwest::blocking::Client;
use serde_json::{json, Value};
use url::Url;

const DEFAULT_ENDPOINT: &str = "http://127.0.0.1:6800/jsonrpc";

#[derive(Debug)]
pub enum Aria2RpcError {
    InvalidEndpoint(String),
    Http(reqwest::Error),
    InvalidResponse(String),
    Rpc { code: i64, message: String },
}

impl std::fmt::Display for Aria2RpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidEndpoint(message) => write!(f, "invalid aria2 RPC endpoint: {message}"),
            Self::Http(error) => write!(f, "aria2 RPC request failed: {error}"),
            Self::InvalidResponse(message) => write!(f, "invalid aria2 RPC response: {message}"),
            Self::Rpc { code, message } => write!(f, "aria2 RPC error {code}: {message}"),
        }
    }
}

impl std::error::Error for Aria2RpcError {}

impl From<reqwest::Error> for Aria2RpcError {
    fn from(error: reqwest::Error) -> Self {
        Self::Http(error)
    }
}

pub struct Aria2Process {
    child: Child,
}

impl Aria2Process {
    pub fn start(
        port: u16,
        secret: Option<&str>,
        directory: Option<&Path>,
        session_file: Option<&Path>,
        load_session: bool,
        max_concurrent_downloads: u32,
        split: u32,
        max_connection_per_server: u32,
        min_split_size: &str,
    ) -> Result<Self, std::io::Error> {
        if !(1024..=65535).contains(&port) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "aria2 RPC port must be between 1024 and 65535",
            ));
        }
        if !(1..=64).contains(&max_concurrent_downloads) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "max concurrent downloads must be between 1 and 64",
            ));
        }
        if !(1..=32).contains(&split) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "split must be between 1 and 32",
            ));
        }
        if !(1..=32).contains(&max_connection_per_server) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "max connections per server must be between 1 and 32",
            ));
        }
        if min_split_size.trim().is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "min split size must not be empty",
            ));
        }

        let mut command = Command::new("aria2c");
        command
            .arg("--enable-rpc=true")
            .arg("--rpc-listen-all=false")
            .arg(format!("--rpc-listen-port={port}"))
            .arg("--continue=true")
            .arg(format!(
                "--max-concurrent-downloads={max_concurrent_downloads}"
            ))
            .arg(format!("--split={split}"))
            .arg(format!(
                "--max-connection-per-server={max_connection_per_server}"
            ))
            .arg(format!("--min-split-size={min_split_size}"))
            .arg("--summary-interval=1")
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        if let Some(secret) = secret {
            if !secret.is_empty() {
                command.arg(format!("--rpc-secret={secret}"));
            }
        }

        if let Some(directory) = directory {
            command.arg("--dir").arg(directory);
        }

        if let Some(session_file) = session_file {
            command
                .arg("--save-session")
                .arg(session_file)
                .arg("--save-session-interval=5");

            if load_session && session_file.exists() {
                command.arg("--input-file").arg(session_file);
            }
        }

        let child = command.spawn()?;
        Ok(Self { child })
    }

    pub fn is_running(&mut self) -> Result<bool, std::io::Error> {
        Ok(self.child.try_wait()?.is_none())
    }
}

impl Drop for Aria2Process {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub struct Aria2Client {
    endpoint: Url,
    secret: Option<String>,
    http: Client,
    next_id: AtomicU64,
}

impl Aria2Client {
    pub fn new(endpoint: &str, secret: Option<String>) -> Result<Self, Aria2RpcError> {
        let endpoint = Url::parse(endpoint)
            .map_err(|error| Aria2RpcError::InvalidEndpoint(error.to_string()))?;

        if !matches!(endpoint.scheme(), "http" | "https") {
            return Err(Aria2RpcError::InvalidEndpoint(
                "scheme must be http or https".to_string(),
            ));
        }

        Ok(Self {
            endpoint,
            secret,
            http: Client::new(),
            next_id: AtomicU64::new(1),
        })
    }

    pub fn localhost(secret: Option<String>) -> Result<Self, Aria2RpcError> {
        Self::new(DEFAULT_ENDPOINT, secret)
    }

    pub fn add_uri(
        &self,
        uri: &str,
        directory: Option<&str>,
        output: Option<&str>,
    ) -> Result<String, Aria2RpcError> {
        let mut options = serde_json::Map::new();

        if let Some(directory) = directory {
            options.insert("dir".to_string(), Value::String(directory.to_string()));
        }

        if let Some(output) = output {
            options.insert("out".to_string(), Value::String(output.to_string()));
        }

        let mut params = vec![json!([uri])];
        if !options.is_empty() {
            params.push(Value::Object(options));
        }

        self.call("aria2.addUri", params)?
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| {
                Aria2RpcError::InvalidResponse(
                    "aria2.addUri result was not a GID string".to_string(),
                )
            })
    }

    pub fn tell_status(&self, gid: &str) -> Result<Value, Aria2RpcError> {
        self.call(
            "aria2.tellStatus",
            vec![
                json!(gid),
                json!([
                    "gid",
                    "status",
                    "totalLength",
                    "completedLength",
                    "downloadSpeed",
                    "uploadSpeed",
                    "connections",
                    "errorCode",
                    "errorMessage",
                    "files",
                ]),
            ],
        )
    }

    pub fn tell_active(&self) -> Result<Value, Aria2RpcError> {
        self.call("aria2.tellActive", vec![])
    }

    pub fn tell_waiting(&self, offset: i64, num: i64) -> Result<Value, Aria2RpcError> {
        self.call("aria2.tellWaiting", vec![json!(offset), json!(num)])
    }

    pub fn tell_stopped(&self, offset: i64, num: i64) -> Result<Value, Aria2RpcError> {
        self.call("aria2.tellStopped", vec![json!(offset), json!(num)])
    }

    pub fn pause(&self, gid: &str) -> Result<String, Aria2RpcError> {
        self.call_result_as_gid("aria2.pause", vec![json!(gid)])
    }

    pub fn unpause(&self, gid: &str) -> Result<String, Aria2RpcError> {
        self.call_result_as_gid("aria2.unpause", vec![json!(gid)])
    }

    pub fn remove(&self, gid: &str) -> Result<String, Aria2RpcError> {
        self.call_result_as_gid("aria2.remove", vec![json!(gid)])
    }

    pub fn remove_download_result(&self, gid: &str) -> Result<String, Aria2RpcError> {
        self.call_result_as_gid("aria2.removeDownloadResult", vec![json!(gid)])
    }

    pub fn get_global_stat(&self) -> Result<Value, Aria2RpcError> {
        self.call("aria2.getGlobalStat", vec![])
    }

    pub fn shutdown(&self) -> Result<String, Aria2RpcError> {
        self.call("aria2.shutdown", vec![])?
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| {
                Aria2RpcError::InvalidResponse(
                    "aria2.shutdown result was not an OK string".to_string(),
                )
            })
    }

    fn call_result_as_gid(
        &self,
        method: &str,
        params: Vec<Value>,
    ) -> Result<String, Aria2RpcError> {
        self.call(method, params)?
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| {
                Aria2RpcError::InvalidResponse(format!("{method} result was not a GID string"))
            })
    }

    fn call(&self, method: &str, mut params: Vec<Value>) -> Result<Value, Aria2RpcError> {
        if let Some(secret) = &self.secret {
            params.insert(0, Value::String(format!("token:{secret}")));
        }

        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let request = json!({
            "jsonrpc": "2.0",
            "id": id.to_string(),
            "method": method,
            "params": params,
        });

        let response: Value = self
            .http
            .post(self.endpoint.clone())
            .header("content-type", "application/json")
            .json(&request)
            .send()?
            .error_for_status()?
            .json()?;

        if let Some(error) = response.get("error") {
            let code = error.get("code").and_then(Value::as_i64).unwrap_or(-1);
            let message = error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("unknown aria2 RPC error");

            return Err(Aria2RpcError::Rpc {
                code,
                message: message.to_string(),
            });
        }

        response.get("result").cloned().ok_or_else(|| {
            Aria2RpcError::InvalidResponse("response did not contain result".to_string())
        })
    }
}

impl Default for Aria2Client {
    fn default() -> Self {
        Self::localhost(None).expect("the default localhost RPC endpoint is valid")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_rpc_port() {
        let result = Aria2Process::start(0, None, None, None, false, 3, 4, 4, "20M");
        assert!(matches!(
            result,
            Err(error) if error.kind() == std::io::ErrorKind::InvalidInput
        ));
    }

    #[test]
    fn rejects_invalid_download_settings() {
        let result = Aria2Process::start(6800, None, None, None, false, 0, 4, 4, "20M");
        assert!(matches!(
            result,
            Err(error) if error.kind() == std::io::ErrorKind::InvalidInput
        ));
    }

    #[test]
    fn localhost_endpoint_is_valid() {
        let client = Aria2Client::localhost(Some("secret".to_string())).unwrap();
        assert_eq!(client.endpoint.as_str(), DEFAULT_ENDPOINT);
        assert_eq!(client.secret.as_deref(), Some("secret"));
    }

    #[test]
    fn rejects_non_http_endpoint() {
        let result = Aria2Client::new("ftp://127.0.0.1:6800/jsonrpc", None);
        assert!(matches!(result, Err(Aria2RpcError::InvalidEndpoint(_))));
    }

    #[test]
    fn gid_result_is_rejected_when_not_a_string() {
        let error = Aria2RpcError::InvalidResponse("test".to_string());
        assert_eq!(error.to_string(), "invalid aria2 RPC response: test");
    }
}
