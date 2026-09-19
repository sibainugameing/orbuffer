use std::{
    env,
    path::PathBuf,
    process::{Command, ExitCode},
};

use serde_json::Value;
use url::Url;

mod aria2;

fn main() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("orbuffer: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<u8, Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);

    match args.next().as_deref() {
        Some("rpc") => run_rpc(args),
        Some(input) => run_download(input, args),
        None => Err(usage().into()),
    }
}

fn run_download(
    input: &str,
    mut args: impl Iterator<Item = String>,
) -> Result<u8, Box<dyn std::error::Error>> {
    let url = parse_supported_url(input)?;

    let output = args.next().map(PathBuf::from).unwrap_or_else(|| {
        let name = url
            .path_segments()
            .and_then(|mut parts| parts.next_back())
            .filter(|name| !name.is_empty())
            .unwrap_or("download.bin");
        PathBuf::from(name)
    });

    if args.next().is_some() {
        return Err("too many arguments; usage: orbuffer <url> [output-file]".into());
    }

    let filename = output
        .file_name()
        .ok_or("output path must include a filename")?;
    let directory = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| std::path::Path::new("."));

    // aria2 owns transfer, resume, retries, segmented connections, and progress.
    // --continue=true reuses aria2's .aria2 control file when available.
    let status = Command::new("aria2c")
        .arg("--continue=true")
        .arg("--allow-overwrite=false")
        .arg("--auto-file-renaming=true")
        .arg("--max-tries=5")
        .arg("--retry-wait=3")
        .arg("--connect-timeout=15")
        .arg("--timeout=60")
        .arg("--summary-interval=1")
        .arg("--console-log-level=notice")
        .arg("--dir")
        .arg(directory)
        .arg("--out")
        .arg(filename)
        .arg(url.as_str())
        .status()
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                "aria2c was not found. Install aria2 and ensure aria2c is on PATH.".to_string()
            } else {
                format!("failed to launch aria2c: {error}")
            }
        })?;

    Ok(status.code().unwrap_or(1).clamp(0, 255) as u8)
}

fn run_rpc(mut args: impl Iterator<Item = String>) -> Result<u8, Box<dyn std::error::Error>> {
    let command = args
        .next()
        .ok_or("missing RPC command; use `orbuffer rpc help`")?;

    let endpoint =
        env::var("ORBUFFER_ARIA2_RPC").unwrap_or_else(|_| "http://127.0.0.1:6800/jsonrpc".into());
    let secret = env::var("ORBUFFER_ARIA2_SECRET")
        .ok()
        .filter(|value| !value.is_empty());
    let client = aria2::Aria2Client::new(&endpoint, secret)?;

    let result = match command.as_str() {
        "add" => {
            let uri = args
                .next()
                .ok_or("usage: orbuffer rpc add <url> [dir] [out]")?;
            parse_supported_url(&uri)?;
            let directory = args.next();
            let output = args.next();
            if args.next().is_some() {
                return Err("too many arguments; usage: orbuffer rpc add <url> [dir] [out]".into());
            }
            Value::String(client.add_uri(&uri, directory.as_deref(), output.as_deref())?)
        }
        "active" => {
            reject_extra_args(&mut args, "orbuffer rpc active")?;
            client.tell_active()?
        }
        "waiting" => {
            let offset = parse_i64(args.next(), 0, "offset")?;
            let num = parse_i64(args.next(), 100, "num")?;
            reject_extra_args(&mut args, "orbuffer rpc waiting [offset] [num]")?;
            client.tell_waiting(offset, num)?
        }
        "stopped" => {
            let offset = parse_i64(args.next(), 0, "offset")?;
            let num = parse_i64(args.next(), 100, "num")?;
            reject_extra_args(&mut args, "orbuffer rpc stopped [offset] [num]")?;
            client.tell_stopped(offset, num)?
        }
        "status" => {
            let gid = args.next().ok_or("usage: orbuffer rpc status <gid>")?;
            reject_extra_args(&mut args, "orbuffer rpc status <gid>")?;
            client.tell_status(&gid)?
        }
        "pause" => rpc_gid_command(args, "pause", |gid| client.pause(gid))?,
        "resume" | "unpause" => rpc_gid_command(args, "resume", |gid| client.unpause(gid))?,
        "remove" => rpc_gid_command(args, "remove", |gid| client.remove(gid))?,
        "global" => {
            reject_extra_args(&mut args, "orbuffer rpc global")?;
            client.get_global_stat()?
        }
        "help" => {
            println!("{}", rpc_usage());
            return Ok(0);
        }
        other => return Err(format!("unknown RPC command: {other}\n\n{}", rpc_usage()).into()),
    };

    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(0)
}

fn rpc_gid_command(
    mut args: impl Iterator<Item = String>,
    name: &str,
    action: impl FnOnce(&str) -> Result<String, aria2::Aria2RpcError>,
) -> Result<Value, Box<dyn std::error::Error>> {
    let gid = args
        .next()
        .ok_or_else(|| format!("usage: orbuffer rpc {name} <gid>"))?;
    reject_extra_args(&mut args, &format!("orbuffer rpc {name} <gid>"))?;
    Ok(Value::String(action(&gid)?))
}

fn reject_extra_args(
    args: &mut impl Iterator<Item = String>,
    usage: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if args.next().is_some() {
        return Err(format!("too many arguments; usage: {usage}").into());
    }
    Ok(())
}

fn parse_i64(
    value: Option<String>,
    default: i64,
    name: &str,
) -> Result<i64, Box<dyn std::error::Error>> {
    match value {
        Some(value) => value
            .parse::<i64>()
            .map_err(|_| format!("invalid {name}: {value}").into()),
        None => Ok(default),
    }
}

fn parse_supported_url(input: &str) -> Result<Url, Box<dyn std::error::Error>> {
    let url = Url::parse(input)?;
    if !matches!(
        url.scheme(),
        "http" | "https" | "ftp" | "ftps" | "sftp" | "magnet"
    ) {
        return Err(format!("unsupported URL scheme: {}", url.scheme()).into());
    }
    Ok(url)
}

fn usage() -> &'static str {
    "usage:\n  orbuffer <url> [output-file]\n  orbuffer rpc <command> [arguments]\n\nRun `orbuffer rpc help` for RPC commands."
}

fn rpc_usage() -> &'static str {
    "usage:\n  orbuffer rpc add <url> [dir] [out]\n  orbuffer rpc active\n  orbuffer rpc waiting [offset] [num]\n  orbuffer rpc stopped [offset] [num]\n  orbuffer rpc status <gid>\n  orbuffer rpc pause <gid>\n  orbuffer rpc resume <gid>\n  orbuffer rpc remove <gid>\n  orbuffer rpc global\n\nEnvironment:\n  ORBUFFER_ARIA2_RPC     RPC endpoint (default: http://127.0.0.1:6800/jsonrpc)\n  ORBUFFER_ARIA2_SECRET  aria2 RPC secret, when configured"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_supported_http_url() {
        assert!(parse_supported_url("https://example.com/file.zip").is_ok());
    }

    #[test]
    fn accepts_magnet_url() {
        assert!(parse_supported_url("magnet:?xt=urn:btih:test").is_ok());
    }

    #[test]
    fn rejects_unsupported_url_scheme() {
        let result = parse_supported_url("javascript:alert(1)");
        assert!(result.is_err());
    }
}
