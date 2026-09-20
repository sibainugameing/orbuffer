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

    #[test]
    #[ignore = "requires aria2c"]
    fn downloads_file_through_local_aria2_rpc() {
        use std::{
            fs,
            io::{Read, Write},
            net::{TcpListener, TcpStream},
            process::{Child, Command, Stdio},
            thread,
            time::{Duration, Instant},
        };

        const BODY: &[u8] = b"OrBuffer local aria2 integration test\n";

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let http_address = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();

        let server = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(10);

            loop {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let mut request = [0_u8; 4096];
                        let _ = stream.read(&mut request);

                        let headers = format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            BODY.len()
                        );
                        stream.write_all(headers.as_bytes()).unwrap();
                        stream.write_all(BODY).unwrap();
                        return;
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        if Instant::now() >= deadline {
                            return;
                        }
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(_) => return,
                }
            }
        });

        let rpc_probe = TcpListener::bind("127.0.0.1:0").unwrap();
        let rpc_port = rpc_probe.local_addr().unwrap().port();
        drop(rpc_probe);

        let temp_dir = std::env::temp_dir().join(format!(
            "orbuffer-aria2-e2e-{}-{}",
            std::process::id(),
            Instant::now().elapsed().as_nanos()
        ));
        fs::create_dir_all(&temp_dir).unwrap();

        let mut aria2: Child = Command::new("aria2c")
            .arg("--enable-rpc=true")
            .arg("--rpc-listen-all=false")
            .arg(format!("--rpc-listen-port={rpc_port}"))
            .arg("--max-concurrent-downloads=1")
            .arg("--split=1")
            .arg("--continue=true")
            .arg("--allow-overwrite=true")
            .arg("--auto-file-renaming=false")
            .arg("--console-log-level=warn")
            .arg("--summary-interval=0")
            .arg("--dir")
            .arg(&temp_dir)
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

        let url = format!("http://{http_address}/fixture.bin");
        let gid = client
            .add_uri(&url, None, Some("fixture.bin"))
            .unwrap();

        let download_deadline = Instant::now() + Duration::from_secs(15);
        let final_status = loop {
            let status = client.tell_status(&gid).unwrap();
            let state = status.get("status").and_then(Value::as_str).unwrap_or("unknown");

            match state {
                "complete" => break status,
                "error" => {
                    panic!(
                        "aria2 download failed: {}",
                        status
                            .get("errorMessage")
                            .and_then(Value::as_str)
                            .unwrap_or("unknown error")
                    );
                }
                _ if Instant::now() >= download_deadline => {
                    panic!("aria2 download did not complete within the timeout");
                }
                _ => thread::sleep(Duration::from_millis(50)),
            }
        };

        let expected_length = BODY.len().to_string();
        assert_eq!(
            final_status
                .get("completedLength")
                .and_then(Value::as_str),
            Some(expected_length.as_str())
        );
        assert_eq!(fs::read(temp_dir.join("fixture.bin")).unwrap(), BODY);

        let _ = aria2.kill();
        let _ = aria2.wait();
        server.join().unwrap();
        let _ = fs::remove_dir_all(temp_dir);
    }
}
