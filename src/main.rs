use std::{
    env,
    fs::{self, File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    path::PathBuf,
    process,
};

use reqwest::{
    blocking::Client,
    header::{CONTENT_RANGE, RANGE},
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
struct ContentRange {
    start: u64,
    end: u64,
    total: Option<u64>,
}

fn parse_content_range(value: &str) -> Option<ContentRange> {
    let value = value.strip_prefix("bytes ")?;
    let (range, total) = value.split_once('/')?;
    let (start, end) = range.split_once('-')?;
    let start = start.parse::<u64>().ok()?;
    let end = end.parse::<u64>().ok()?;
    let total = if total == "*" {
        None
    } else {
        Some(total.parse::<u64>().ok()?)
    };

    if end < start || total.is_some_and(|size| size <= end) {
        return None;
    }

    Some(ContentRange { start, end, total })
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let input = args
        .next()
        .ok_or("usage: orbuffer <http-or-https-url> [output-file]")?;
    let url = Url::parse(&input)?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("only HTTP and HTTPS URLs are supported".into());
    }

    let output = args.next().map(PathBuf::from).unwrap_or_else(|| {
        let name = url
            .path_segments()
            .and_then(|mut parts| parts.next_back())
            .filter(|name| !name.is_empty())
            .unwrap_or("download.bin");
        PathBuf::from(name)
    });
    if output.exists() {
        return Err(format!("destination already exists: {}", output.display()).into());
    }

    // Keep partial bytes beside the destination so a later invocation can resume.
    let temporary = output.with_extension("orbuffer-part");
    let mut offset = if temporary.exists() {
        fs::metadata(&temporary)?.len()
    } else {
        0
    };
    let client = Client::builder()
        .user_agent(concat!("OrBuffer/", env!("CARGO_PKG_VERSION")))
        .build()?;

    let mut request = client.get(url.clone());
    if offset > 0 {
        request = request.header(RANGE, format!("bytes={offset}-"));
    }
    let mut response = request.send()?.error_for_status()?;

    // Append only when the server confirms that it honored the requested byte range.
    if offset > 0 && response.status() != StatusCode::PARTIAL_CONTENT {
        eprintln!("Server did not honor Range; restarting from byte zero.");
        offset = 0;
        response = client.get(url).send()?.error_for_status()?;
    }

    if offset > 0 {
        let header = response
            .headers()
            .get(CONTENT_RANGE)
            .and_then(|value| value.to_str().ok())
            .ok_or("server returned 206 without a valid Content-Range header")?;
        let range = parse_content_range(header)
            .ok_or_else(|| format!("malformed Content-Range: {header}"))?;
        if range.start != offset {
            return Err(format!(
                "unexpected Content-Range start: requested {offset}, received {}",
                range.start
            )
            .into());
        }
        if let (Some(total), Some(length)) = (range.total, response.content_length()) {
            let expected_length = range
                .end
                .checked_sub(range.start)
                .and_then(|value| value.checked_add(1))
                .ok_or("invalid Content-Range length")?;
            if length != expected_length || range.end >= total {
                return Err(format!(
                    "inconsistent Content-Range and Content-Length: {header}, {length} bytes"
                )
                .into());
            }
        }
    }

    let remaining = response.content_length();
    let total = remaining.and_then(|length| length.checked_add(offset));
    let mut file = if offset > 0 {
        let mut file = OpenOptions::new().append(true).open(&temporary)?;
        file.seek(SeekFrom::End(0))?;
        file
    } else {
        File::create(&temporary)?
    };

    let mut downloaded = offset;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = response.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        file.write_all(&buffer[..count])?;
        downloaded += count as u64;
        match total {
            Some(size) if size > 0 => eprint!(
                "\rDownloaded {downloaded}/{size} bytes ({:.1}%)",
                downloaded as f64 * 100.0 / size as f64
            ),
            _ => eprint!("\rDownloaded {downloaded} bytes"),
        }
    }
    file.flush()?;
    drop(file);

    if let Some(size) = total {
        if downloaded != size {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                format!(
                    "incomplete response: expected {size} bytes, received {downloaded}; partial file kept at {}",
                    temporary.display()
                ),
            )
            .into());
        }
    }

    eprintln!("\nSaving to {}", output.display());
    fs::rename(&temporary, &output)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{parse_content_range, ContentRange};

    #[test]
    fn parses_valid_range_with_known_total() {
        assert_eq!(
            parse_content_range("bytes 100-199/500"),
            Some(ContentRange {
                start: 100,
                end: 199,
                total: Some(500),
            })
        );
    }

    #[test]
    fn parses_valid_range_with_unknown_total() {
        assert_eq!(
            parse_content_range("bytes 10-19/*"),
            Some(ContentRange {
                start: 10,
                end: 19,
                total: None,
            })
        );
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
