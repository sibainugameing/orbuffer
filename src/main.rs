use std::{env, path::PathBuf, process::{self, Command, ExitCode}};
use url::Url;

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
    let input = args.next().ok_or("usage: orbuffer <http-or-https-url> [output-file]")?;
    let url = Url::parse(&input)?;
    if !matches!(url.scheme(), "http" | "https" | "ftp" | "ftps" | "sftp" | "magnet") {
        return Err(format!("unsupported URL scheme: {}", url.scheme()).into());
    }

    let output = args.next().map(PathBuf::from).unwrap_or_else(|| {
        let name = url.path_segments()
            .and_then(|mut parts| parts.next_back())
            .filter(|name| !name.is_empty())
            .unwrap_or("download.bin");
        PathBuf::from(name)
    });
    if args.next().is_some() {
        return Err("too many arguments; usage: orbuffer <url> [output-file]".into());
    }

    let filename = output.file_name().ok_or("output path must include a filename")?;
    let directory = output.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or_else(|| std::path::Path::new("."));

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
