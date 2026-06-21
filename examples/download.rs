use java_manager::download::DownloadEvent;
use java_manager::download::download_java;
use std::error::Error;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let url = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "https://github.com/adoptium/temurin21-binaries/releases/download/jdk-21.0.2%2B13/OpenJDK21U-jdk_x64_windows_hotspot_21.0.2_13.zip".to_string());

    let version = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "java-21".to_string());

    let runtimes_dir = Path::new("./runtimes");
    let cancel = Arc::new(AtomicBool::new(false));

    println!("═══ Java Downloader ═══");
    println!("Version: {version}");
    println!("URL:     {url}");
    println!();

    let (rx, handle) = download_java(&url, &version, runtimes_dir, cancel);

    let start = Instant::now();

    while let Ok(event) = rx.recv_async().await {
        match event {
            DownloadEvent::DownloadStarted => {
                println!("▸ Download started");
            }
            DownloadEvent::DownloadProgress { downloaded, total } => {
                let elapsed = start.elapsed();
                let secs = elapsed.as_secs_f64();
                let pct = if total > 0 {
                    (downloaded as f64 / total as f64) * 100.0
                } else {
                    0.0
                };
                let speed = if secs > 0.0 {
                    downloaded as f64 / secs
                } else {
                    0.0
                };
                let mut h = std::io::stdout().lock();
                let _ = write!(
                    h,
                    "\r  {:5.1}%  {} / {}  {:>8}/s  {:.1}s",
                    pct,
                    format_bytes(downloaded),
                    format_bytes(total),
                    format_bytes(speed as u64),
                    secs,
                );
                let _ = h.flush();
            }
            DownloadEvent::DownloadFinished { total } => {
                println!();
                println!(
                    "▸ Download complete – {} total in {:.1}s",
                    format_bytes(total),
                    start.elapsed().as_secs_f64()
                );
            }
            DownloadEvent::Extracting => {
                println!("▸ Extracting archive...");
            }
            DownloadEvent::ExtractProgress { current, total } => {
                let mut h = std::io::stdout().lock();
                let _ = write!(h, "\r  Extracting: {current}/{total} files");
                let _ = h.flush();
            }
            DownloadEvent::Finished { java_bin } => {
                println!();
                println!("▸ Done! Total time: {:.1}s", start.elapsed().as_secs_f64());
                println!("▸ Java binary: {}", java_bin.display());
            }
            DownloadEvent::Failed { message } => {
                println!();
                eprintln!("▸ Failed after {:.1}s", start.elapsed().as_secs_f64());
                eprintln!("▸ Error: {message}");
            }
        }
    }

    println!();

    match handle.await? {
        Ok(path) => {
            println!("✓ Success – Java binary: {}", path.display());
            Ok(())
        }
        Err(e) => {
            eprintln!("✗ Error: {e}");
            Err(e.into())
        }
    }
}

fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;

    if bytes as f64 >= GB {
        format!("{:.2} GiB", bytes as f64 / GB)
    } else if bytes as f64 >= MB {
        format!("{:.2} MiB", bytes as f64 / MB)
    } else if bytes as f64 >= KB {
        format!("{:.2} KiB", bytes as f64 / KB)
    } else {
        format!("{bytes} B")
    }
}
