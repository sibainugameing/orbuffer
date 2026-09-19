use std::{
    env,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::PathBuf,
    process,
};

use reqwest::{
    blocking::{Client, Response},
    header::{CONTENT_RANGE, ETAG, IF_RANGE, LAST_MODIFIED, RANGE},
    StatusCode,
};
use url::Url;

fn main() {
    if let Err(error) = run() {
        eprintln!("orbuffer: {error}");
        process::exit(1);
    }
}

#[derive(Debug, PartialEq, Eq)]
struct ContentRange { start: u64, end: u64, total: Option<u64> }

fn parse_content_range(value: &str) -> Option<ContentRange> {
    let value = value.strip_prefix("bytes ")?;
    let (range, total) = value.split_once('/')?;
    let (start, end) = range.split_once('-')?;
    let start = start.parse::<u64>().ok()?;
    let end = end.parse::<u64>().ok()?;
    let total = if total == "*" { None } else { Some(total.parse::<u64>().ok()?) };
    if end < start || total.is_some_and(|size| size <= end) { return None; }
    Some(ContentRange { start, end, total })
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Validator { Etag(String), LastModified(String) }

fn response_validator(response: &Response) -> Option<Validator> {
    if let Some(value) = response.headers().get(ETAG).and_then(|v| v.to_str().ok()) {
        if !value.trim_start().starts_with("W/") && !value.contains(['\r', '\n']) {
            return Some(Validator::Etag(value.to_owned()));
        }
    }
    response.headers().get(LAST_MODIFIED).and_then(|v| v.to_str().ok())
        .filter(|v| !v.contains(['\r', '\n']))
        .map(|v| Validator::LastModified(v.to_owned()))
}

fn read_state(path: &PathBuf, url: &str) -> Option<Validator> {
    let text = fs::read_to_string(path).ok()?;
    let mut lines = text.lines();
    if lines.next()? != url { return None; }
    match lines.next()? {
        "etag" => Some(Validator::Etag(lines.next()?.to_owned())),
        "last-modified" => Some(Validator::LastModified(lines.next()?.to_owned())),
        _ => None,
    }
}

fn write_state(path: &PathBuf, url: &str, validator: &Validator) -> io::Result<()> {
    let (kind, value) = match validator {
        Validator::Etag(value) => ("etag", value),
        Validator::LastModified(value) => ("last-modified", value),
    };
    if url.contains(['\r', '\n']) || value.contains(['\r', '\n']) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid validator metadata"));
    }
    fs::write(path, format!("{url}\n{kind}\n{value}\n"))
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let input = args.next().ok_or("usage: orbuffer <http-or-https-url> [output-file]")?;
    let url = Url::parse(&input)?;
    if !matches!(url.scheme(), "http" | "https") { return Err("only HTTP and HTTPS URLs are supported".into()); }
    let output = args.next().map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(url.path_segments().and_then(|mut p| p.next_back()).filter(|s| !s.is_empty()).unwrap_or("download.bin"))
    });
    if output.exists() { return Err(format!("destination already exists: {}", output.display()).into()); }

    let temporary = output.with_extension("orbuffer-part");
    let state_path = output.with_extension("orbuffer-state");
    let mut offset = if temporary.exists() { fs::metadata(&temporary)?.len() } else { 0 };
    let client = Client::builder().user_agent(concat!("OrBuffer/", env!("CARGO_PKG_VERSION"))).build()?;

    let mut response = if offset > 0 {
        match read_state(&state_path, url.as_str()) {
            Some(validator) => {
                let mut request = client.get(url.clone()).header(RANGE, format!("bytes={offset}-"));
                request = match &validator {
                    Validator::Etag(value) | Validator::LastModified(value) => request.header(IF_RANGE, value),
                };
                let candidate = request.send()?;
                if candidate.status() == StatusCode::PARTIAL_CONTENT {
                    let header = candidate.headers().get(CONTENT_RANGE).and_then(|v| v.to_str().ok()).ok_or("206 response missing Content-Range")?;
                    let range = parse_content_range(header).ok_or_else(|| format!("malformed Content-Range: {header}"))?;
                    if range.start != offset { return Err(format!("unexpected Content-Range start: expected {offset}, got {}", range.start).into()); }
                    if let (Some(total), Some(length)) = (range.total, candidate.content_length()) {
                        let expected = range.end - range.start + 1;
                        if length != expected || range.end >= total { return Err("inconsistent Content-Range and Content-Length".into()); }
                    }
                    if response_validator(&candidate).is_some_and(|received| received != validator) {
                        return Err("server validator changed during resumed response".into());
                    }
                    candidate
                } else if candidate.status().is_success() {
                    eprintln!("Server declined safe resume; restarting from byte zero.");
                    offset = 0;
                    candidate
                } else {
                    candidate.error_for_status()?
                }
            }
            None => {
                eprintln!("No matching strong validator metadata; restarting from byte zero.");
                offset = 0;
                client.get(url.clone()).send()?.error_for_status()?
            }
        }
    } else { client.get(url.clone()).send()?.error_for_status()? };

    let validator = response_validator(&response);
    if offset == 0 {
        if let Some(ref validator) = validator { write_state(&state_path, url.as_str(), validator)?; }
        else { let _ = fs::remove_file(&state_path); }
    }
    let remaining = response.content_length();
    let total = remaining.and_then(|length| length.checked_add(offset));
    let mut file = if offset > 0 { OpenOptions::new().append(true).open(&temporary)? } else { File::create(&temporary)? };
    let mut downloaded = offset;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = response.read(&mut buffer)?;
        if count == 0 { break; }
        file.write_all(&buffer[..count])?;
        downloaded += count as u64;
        match total {
            Some(size) if size > 0 => eprint!("\rDownloaded {downloaded}/{size} bytes ({:.1}%)", downloaded as f64 * 100.0 / size as f64),
            _ => eprint!("\rDownloaded {downloaded} bytes"),
        }
    }
    file.flush()?;
    drop(file);
    if let Some(size) = total {
        if downloaded != size { return Err(io::Error::new(io::ErrorKind::UnexpectedEof, format!("incomplete response: expected {size} bytes, received {downloaded}; partial file kept at {}", temporary.display())).into()); }
    }
    eprintln!("\nSaving to {}", output.display());
    fs::rename(&temporary, &output)?;
    let _ = fs::remove_file(state_path);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{parse_content_range, ContentRange};
    #[test]
    fn parses_valid_range_with_known_total() {
        assert_eq!(parse_content_range("bytes 100-199/500"), Some(ContentRange { start: 100, end: 199, total: Some(500) }));
    }
    #[test]
    fn parses_valid_range_with_unknown_total() {
        assert_eq!(parse_content_range("bytes 10-19/*"), Some(ContentRange { start: 10, end: 19, total: None }));
    }
    #[test]
    fn rejects_wrong_unit_and_malformed_values() {
        assert_eq!(parse_content_range("items 0-9/10"), None);
        assert_eq!(parse_content_range("bytes 0/10"), None);
        assert_eq!(parse_content_range("bytes x-9/10"), None);
        assert_eq!(parse_content_range("bytes 0-9/x"), None);
    }
    #[test]
    fn rejects_reversed_or_out_of_bounds_ranges() {
        assert_eq!(parse_content_range("bytes 10-9/20"), None);
        assert_eq!(parse_content_range("bytes 0-10/10"), None);
    }
}
