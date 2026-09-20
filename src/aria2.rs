use std::sync::atomic::{AtomicU64, Ordering};

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

    pub fn get_global_stat(&self) -> Result<Value, Aria2RpcError> {
        self.call("aria2.getGlobalStat", vec![])
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

    fn spawn_mock_rpc(
        expected_method: &'static str,
        expected_params: Value,
        response: Value,
    ) -> (Url, std::thread::JoinHandle<()>) {
        use std::{
            io::{Read, Write},
            net::TcpListener,
            thread,
        };

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();

            let mut request = Vec::new();
            let mut byte = [0_u8; 1];
            loop {
                stream.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
                if request.ends_with(b"\r\n\r\n") {
                    break;
                }
            }

            let headers = String::from_utf8(request).unwrap();
            let content_length = headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().unwrap())
                })
                .unwrap();

            let mut body = vec![0_u8; content_length];
            stream.read_exact(&mut body).unwrap();
            let request: Value = serde_json::from_slice(&body).unwrap();

            assert_eq!(request.get("jsonrpc").and_then(Value::as_str), Some("2.0"));
            assert_eq!(
                request.get("method").and_then(Value::as_str),
                Some(expected_method)
            );
            assert_eq!(request.get("params"), Some(&expected_params));

            let body = serde_json::to_vec(&response).unwrap();
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .unwrap();
            stream.write_all(&body).unwrap();
        });

        (
            Url::parse(&format!("http://{address}/jsonrpc")).unwrap(),
            handle,
        )
    }

    #[test]
    fn add_uri_sends_token_and_options_to_rpc() {
        let (endpoint, handle) = spawn_mock_rpc(
            "aria2.addUri",
            json!([
                "token:secret",
                ["https://example.com/file.zip"],
                {"dir": "/tmp/downloads", "out": "file.zip"}
            ]),
            json!({"jsonrpc": "2.0", "id": "1", "result": "gid-123"}),
        );
        let client = Aria2Client::new(endpoint.as_str(), Some("secret".to_string())).unwrap();

        let gid = client
            .add_uri(
                "https://example.com/file.zip",
                Some("/tmp/downloads"),
                Some("file.zip"),
            )
            .unwrap();

        handle.join().unwrap();
        assert_eq!(gid, "gid-123");
    }

    #[test]
    fn rpc_error_is_mapped_to_aria2_error() {
        let (endpoint, handle) = spawn_mock_rpc(
            "aria2.getGlobalStat",
            json!([]),
            json!({
                "jsonrpc": "2.0",
                "id": "1",
                "error": {"code": 1, "message": "unauthorized"}
            }),
        );
        let client = Aria2Client::new(endpoint.as_str(), None).unwrap();

        let error = client.get_global_stat().unwrap_err();

        handle.join().unwrap();
        assert!(matches!(
            error,
            Aria2RpcError::Rpc { code: 1, message } if message == "unauthorized"
        ));
    }

    #[derive(Clone, Copy)]
    enum FixtureMode {
        Full,
        WithoutRangeSupport,
        UnknownLength,
        FailFirst { cutoff: usize },
    }

    fn spawn_http_fixture(
        body: std::sync::Arc<Vec<u8>>,
        mode: FixtureMode,
    ) -> (
        String,
        std::sync::Arc<std::sync::atomic::AtomicBool>,
        std::thread::JoinHandle<()>,
    ) {
        use std::{
            io::{Read, Write},
            net::TcpListener,
            sync::atomic::AtomicBool,
            thread,
            time::{Duration, Instant},
        };

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();

        let stop = std::sync::Arc::new(AtomicBool::new(false));
        let stop_for_thread = std::sync::Arc::clone(&stop);

        let handle = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(20);
            let request_count = std::sync::atomic::AtomicUsize::new(0);

            while !stop_for_thread.load(std::sync::atomic::Ordering::Relaxed)
                && Instant::now() < deadline
            {
                let Ok((mut stream, _)) = listener.accept() else {
                    thread::sleep(Duration::from_millis(10));
                    continue;
                };

                let mut request = [0_u8; 4096];
                let _ = stream.read(&mut request);
                let request_number =
                    request_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

                match mode {
                    FixtureMode::Full => {
                        let headers = format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            body.len()
                        );
                        stream.write_all(headers.as_bytes()).unwrap();
                        stream.write_all(&body).unwrap();
                    }
                    FixtureMode::WithoutRangeSupport => {
                        let headers = format!(
                            "HTTP/1.1 200 OK\r\nAccept-Ranges: none\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            body.len()
                        );
                        stream.write_all(headers.as_bytes()).unwrap();
                        stream.write_all(&body).unwrap();
                    }
                    FixtureMode::UnknownLength => {
                        stream
                            .write_all(
                                b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n",
                            )
                            .unwrap();
                        stream.write_all(&body).unwrap();
                    }
                    FixtureMode::FailFirst { cutoff } if request_number == 0 => {
                        let headers = format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            body.len()
                        );
                        stream.write_all(headers.as_bytes()).unwrap();
                        stream.write_all(&body[..cutoff.min(body.len())]).unwrap();
                    }
                    FixtureMode::FailFirst { .. } => {
                        let headers = format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            body.len()
                        );
                        stream.write_all(headers.as_bytes()).unwrap();
                        stream.write_all(&body).unwrap();
                    }
                }
            }
        });

        (
            format!("http://{address}/fixture.bin"),
            stop,
            handle,
        )
    }

    fn start_local_aria2(
        temp_dir: &std::path::Path,
        split: u32,
        min_split_size: Option<&str>,
    ) -> (std::process::Child, Aria2Client) {
        use std::{
            net::TcpListener,
            process::{Child, Command, Stdio},
            thread,
            time::{Duration, Instant},
        };

        let rpc_probe = TcpListener::bind("127.0.0.1:0").unwrap();
        let rpc_port = rpc_probe.local_addr().unwrap().port();
        drop(rpc_probe);

        let mut aria2: Child = Command::new("aria2c")
            .arg("--enable-rpc=true")
            .arg("--rpc-listen-all=false")
            .arg(format!("--rpc-listen-port={rpc_port}"))
            .arg("--max-concurrent-downloads=1")
            .arg(format!("--split={split}"))
            .args(min_split_size.into_iter().flat_map(|value| ["--min-split-size", value]))
            .arg("--continue=true")
            .arg("--allow-overwrite=true")
            .arg("--auto-file-renaming=false")
            .arg("--max-tries=3")
            .arg("--retry-wait=0")
            .arg("--connect-timeout=2")
            .arg("--timeout=5")
            .arg("--console-log-level=warn")
            .arg("--summary-interval=0")
            .arg("--dir")
            .arg(temp_dir)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("aria2c must be installed to run this ignored integration test");

        let endpoint = format!("http://127.0.0.1:{rpc_port}/jsonrpc");
        let client = Aria2Client::new(&endpoint, None).unwrap();

        let startup_deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if client.get_global_stat().is_ok() {
                break;
            }

            if aria2.try_wait().unwrap().is_some() {
                panic!("aria2c exited before its RPC endpoint became ready");
            }

            if Instant::now() >= startup_deadline {
                panic!("aria2 RPC endpoint did not become ready");
            }

            thread::sleep(Duration::from_millis(50));
        }

        (aria2, client)
    }

    fn wait_for_completion(client: &Aria2Client, gid: &str, timeout: std::time::Duration) -> Value {
        use std::{
            thread,
            time::Instant,
        };

        let deadline = Instant::now() + timeout;

        loop {
            let status = client.tell_status(gid).unwrap();
            match status.get("status").and_then(Value::as_str) {
                Some("complete") => return status,
                Some("error") => {
                    panic!(
                        "aria2 download failed: {}",
                        status
                            .get("errorMessage")
                            .and_then(Value::as_str)
                            .unwrap_or("unknown error")
                    );
                }
                _ if Instant::now() >= deadline => {
                    panic!("aria2 download did not complete within the timeout");
                }
                _ => thread::sleep(std::time::Duration::from_millis(50)),
            }
        }
    }

    fn temp_download_dir() -> std::path::PathBuf {
        use std::time::{SystemTime, UNIX_EPOCH};

        std::env::temp_dir().join(format!(
            "orbuffer-aria2-e2e-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn cleanup_integration(
        mut aria2: std::process::Child,
        stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
        server: std::thread::JoinHandle<()>,
        temp_dir: std::path::PathBuf,
    ) {
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
        let _ = aria2.kill();
        let _ = aria2.wait();
        server.join().unwrap();
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    #[ignore = "requires aria2c"]
    fn downloads_file_through_local_aria2_rpc() {
        let body = std::sync::Arc::new(
            b"OrBuffer local aria2 integration test\n".to_vec(),
        );
        let (url, stop, server) = spawn_http_fixture(
            std::sync::Arc::clone(&body),
            FixtureMode::Full,
        );
        let temp_dir = temp_download_dir();
        std::fs::create_dir_all(&temp_dir).unwrap();
        let (aria2, client) = start_local_aria2(&temp_dir, 1, None);

        let gid = client.add_uri(&url, None, Some("fixture.bin")).unwrap();
        let final_status = wait_for_completion(&client, &gid, std::time::Duration::from_secs(15));

        assert_eq!(
            final_status.get("completedLength").and_then(Value::as_str),
            Some(body.len().to_string().as_str())
        );
        assert_eq!(std::fs::read(temp_dir.join("fixture.bin")).unwrap(), *body);

        cleanup_integration(aria2, stop, server, temp_dir);
    }

    #[test]
    #[ignore = "requires aria2c"]
    fn downloads_from_server_without_range_support() {
        let body = std::sync::Arc::new(vec![b'R'; 16 * 1024]);
        let (url, stop, server) = spawn_http_fixture(
            std::sync::Arc::clone(&body),
            FixtureMode::WithoutRangeSupport,
        );
        let temp_dir = temp_download_dir();
        std::fs::create_dir_all(&temp_dir).unwrap();
        let (aria2, client) = start_local_aria2(&temp_dir, 4, Some("1K"));

        let gid = client.add_uri(&url, None, Some("fixture.bin")).unwrap();
        wait_for_completion(&client, &gid, std::time::Duration::from_secs(15));

        assert_eq!(std::fs::read(temp_dir.join("fixture.bin")).unwrap(), *body);
        cleanup_integration(aria2, stop, server, temp_dir);
    }

    #[test]
    #[ignore = "requires aria2c"]
    fn downloads_file_with_unknown_content_length() {
        let body = std::sync::Arc::new(b"OrBuffer unknown length integration test\n".repeat(2048));
        let (url, stop, server) = spawn_http_fixture(
            std::sync::Arc::clone(&body),
            FixtureMode::UnknownLength,
        );
        let temp_dir = temp_download_dir();
        std::fs::create_dir_all(&temp_dir).unwrap();
        let (aria2, client) = start_local_aria2(&temp_dir, 1, None);

        let gid = client.add_uri(&url, None, Some("fixture.bin")).unwrap();
        wait_for_completion(&client, &gid, std::time::Duration::from_secs(15));

        assert_eq!(std::fs::read(temp_dir.join("fixture.bin")).unwrap(), *body);
        cleanup_integration(aria2, stop, server, temp_dir);
    }

    #[test]
    #[ignore = "requires aria2c"]
    fn recovers_after_source_connection_is_interrupted() {
        let body = std::sync::Arc::new(vec![b'N'; 64 * 1024]);
        let cutoff = body.len() / 2;
        let (url, stop, server) = spawn_http_fixture(
            std::sync::Arc::clone(&body),
            FixtureMode::FailFirst { cutoff },
        );
        let temp_dir = temp_download_dir();
        std::fs::create_dir_all(&temp_dir).unwrap();
        let (aria2, client) = start_local_aria2(&temp_dir, 1, None);

        let gid = client.add_uri(&url, None, Some("fixture.bin")).unwrap();
        wait_for_completion(&client, &gid, std::time::Duration::from_secs(20));

        assert_eq!(std::fs::read(temp_dir.join("fixture.bin")).unwrap(), *body);
        cleanup_integration(aria2, stop, server, temp_dir);
    }

}
