use std::{
    env,
    fs::{self, File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    path::PathBuf,
    process,
};

use reqwest::{blocking::Client, header::{CONTENT_RANGE, RANGE}};
use url::Url;

fn main() {
    if let Err(error) = run() {
        eprintln!("orbuffer: {error}");
        process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let input = args.next().ok_or("usage: orbuffer <http-or-https-url> [output-file]")?;
    let url = Url::parse(&input)?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("only HTTP and HTTPS URLs are supported".into());
    }

    let output = args.next().map(PathBuf::from).unwrap_or_else(|| {
        let name = url.path_segments().and_then(|mut parts| parts.next_back())
            .filter(|name| !name.is_empty()).unwrap_or("download.bin");
        PathBuf::from(name)
    });
    if output.exists() {
        return Err(format!("destination already exists: {}", output.display()).into());
    }

    // Keep partial bytes beside the destination so a later invocation can resume.
    let temporary = output.with_extension("orbuffer-part");
    let mut offset = if temporary.exists() { fs::metadata(&temporary)?.len() } else { 0 };
    let client = Client::builder()
        .user_agent(concat!("OrBuffer/", env!("CARGO_PKG_VERSION")))
        .build()?;

    let mut request = client.get(url.clone());
    if offset > 0 {
        request = request.header(RANGE, format!("bytes={offset}-"));
    }
    let mut response = request.send()?.error_for_status()?;

    // Append only when the server confirms that it honored the requested byte range.
    if offset > 0 && response.status() != reqwest::StatusCode::PARTIAL_CONTENT {
        eprintln!("Server did not honor Range; restarting from byte zero.");
        offset = 0;
        response = client.get(url).send()?.error_for_status()?;
    }

    if offset > 0 {
        let content_range = response.headers().get(CONTENT_RANGE)
            .and_then(|value| value.to_str().ok())
            .ok_or("server returned 206 without a valid Content-Range header")?;
        let expected_prefix = format!("bytes {offset}-");
        if !content_range.starts_with(&expected_prefix) {
            return Err(format!("unexpected Content-Range: {content_range}").into());
        }
    }

    let remaining = response.content_length();
    let total = remaining.and_then(|length| length.checked_add(offset));
    let mut file = if offset > 0 {
        let mut file = OpenOptions::new().write(true).open(&temporary)?;
        file.seek(SeekFrom::End(0))?;
        file
    } else {
        File::create(&temporary)?
    };

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
        if downloaded != size {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof,
                format!("incomplete response: expected {size} bytes, received {downloaded}; partial file kept at {}", temporary.display())).into());
        }
    }

    eprintln!("\nSaving to {}", output.display());
    fs::rename(&temporary, &output)?;
    Ok(())
}
