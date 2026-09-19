use std::{env, fs::File, io::{self, Read, Write}, path::PathBuf, process};
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

    let client = reqwest::blocking::Client::builder()
        .user_agent(concat!("OrBuffer/", env!("CARGO_PKG_VERSION")))
        .build()?;
    let mut response = client.get(url.clone()).send()?.error_for_status()?;
    let total = response.content_length();
    let temporary = output.with_extension("orbuffer-part");
    let mut file = File::create(&temporary)?;
    let mut downloaded: u64 = 0;
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
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, format!("incomplete response: expected {size} bytes, received {downloaded}")).into());
        }
    }
    eprintln!("\nSaving to {}", output.display());
    std::fs::rename(&temporary, &output)?;
    Ok(())
}
