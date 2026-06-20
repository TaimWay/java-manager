mod archive;
mod shared;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use flume::Receiver;
use futures_util::StreamExt;
use tokio::io::AsyncWriteExt;

use crate::JavaError;

use archive::extract_downloaded_archive;
use shared::{
    concatenate_files, move_directory, read_resume_offset, remove_resume_file,
    resolve_install_source, resolve_java_binary_path, write_resume_offset,
};

/// Events emitted during the Java download and installation process.
#[derive(Debug, Clone)]
pub enum DownloadEvent {
    /// The download has started.
    DownloadStarted,
    /// Progress update with current and total bytes.
    DownloadProgress { downloaded: u64, total: u64 },
    /// The download has finished.
    DownloadFinished { total: u64 },
    /// The archive is being extracted.
    Extracting,
    /// Per‑file extraction progress (current file, total files).
    ExtractProgress { current: u64, total: u64 },
    /// Installation is complete, providing the path to the `java` binary.
    Finished { java_bin: PathBuf },
    /// An error occurred during the process.
    Failed { message: String },
}

const DOWNLOAD_TIMEOUT_SECS: u64 = 300;
const DOWNLOAD_RETRY_LIMIT: usize = 3;
const DOWNLOAD_TEMP_FILE_NAME: &str = "java_download.tmp";

const NUM_PARALLEL_CHUNKS: u64 = 4;
const MIN_PARALLEL_SIZE: u64 = 50 * 1024 * 1024; // 50 MB

/// Download a Java runtime from a URL and install it into
/// `runtimes_dir/<version_name>`.
///
/// Returns a [`Receiver`] that yields [`DownloadEvent`] progress updates,
/// and a [`tokio::task::JoinHandle`] that resolves to the path of the
/// installed `java` binary on success.
///
/// If the target directory already contains a valid `java` binary,
/// the download is skipped and a `Finished` event is emitted immediately.
///
/// Downloads support:
/// - **Parallel chunking**: when the server supports `Range` and the file is
///   larger than 50 MB, the download is split into 4 concurrent stream tasks.
/// - **Resume**: if interrupted, a resume marker is saved so the next attempt
///   continues from where it left off (requires server `Range` support).
/// - **Platform‑safe move**: on Windows, extracted files are copied instead of
///   renamed to avoid cross‑volume / permission errors.
///
/// # Example
///
/// ```no_run
/// use std::sync::atomic::AtomicBool;
/// use std::sync::Arc;
/// use java_manager::download::download_java;
/// use std::path::Path;
///
/// # async fn example() {
/// let cancel = Arc::new(AtomicBool::new(false));
/// let (rx, handle) = download_java(
///     "https://example.com/java.tar.gz",
///     "java-21",
///     Path::new("/tmp/runtimes"),
///     cancel,
/// );
///
/// while let Ok(event) = rx.recv_async().await {
///     println!("{event:?}");
/// }
///
/// let result = handle.await.unwrap();
/// match result {
///     Ok(path) => println!("Installed at: {}", path.display()),
///     Err(e) => eprintln!("Failed: {e}"),
/// }
/// # }
/// ```
pub fn download_java(
    url: &str,
    version_name: &str,
    runtimes_dir: &Path,
    cancel_flag: Arc<AtomicBool>,
) -> (Receiver<DownloadEvent>, tokio::task::JoinHandle<Result<PathBuf, JavaError>>) {
    let (tx, rx) = flume::unbounded::<DownloadEvent>();

    let url = url.to_string();
    let version_name = version_name.to_string();
    let runtimes_dir = runtimes_dir.to_path_buf();

    let handle = tokio::spawn(async move {
        let result =
            download_inner(&url, &version_name, &runtimes_dir, &cancel_flag, &tx).await;
        if let Err(ref e) = result {
            let _ = tx.send(DownloadEvent::Failed {
                message: e.to_string(),
            });
        }
        result
    });

    (rx, handle)
}

// ---------------------------------------------------------------------------
// Internal download orchestrator
// ---------------------------------------------------------------------------

async fn download_inner(
    url: &str,
    version_name: &str,
    runtimes_dir: &Path,
    cancel_flag: &Arc<AtomicBool>,
    tx: &flume::Sender<DownloadEvent>,
) -> Result<PathBuf, JavaError> {
    if !runtimes_dir.exists() {
        fs::create_dir_all(runtimes_dir)
            .map_err(|e| JavaError::DownloadError(format!("create runtimes dir: {e}")))?;
    }

    // Idempotency check
    let target_dir = runtimes_dir.join(version_name);
    let java_bin = resolve_java_binary_path(&target_dir);
    if java_bin.exists() {
        let _ = tx.send(DownloadEvent::Finished {
            java_bin: java_bin.clone(),
        });
        return Ok(java_bin);
    }

    let _ = tx.send(DownloadEvent::DownloadStarted);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(DOWNLOAD_TIMEOUT_SECS))
        .build()
        .map_err(|e| JavaError::DownloadError(format!("build http client: {e}")))?;

    // Prepare temp dir
    let temp_dir = runtimes_dir.join(format!("temp_{}", version_name));
    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir)
            .map_err(|e| JavaError::DownloadError(format!("clean temp dir: {e}")))?;
    }
    fs::create_dir_all(&temp_dir)
        .map_err(|e| JavaError::DownloadError(format!("create temp dir: {e}")))?;

    // HEAD request to probe server capabilities
    let head_response = retry(|| client.head(url).send(), cancel_flag).await?;
    let total_size = head_response.content_length().unwrap_or(0);
    let accept_ranges = head_response
        .headers()
        .get(reqwest::header::ACCEPT_RANGES)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.contains("bytes"))
        .unwrap_or(false);

    // Choose download strategy
    let temp_file_path = temp_dir.join(DOWNLOAD_TEMP_FILE_NAME);

    let resume_offset = read_resume_offset(&temp_dir).unwrap_or(0);
    let downloaded = if resume_offset > 0 && accept_ranges && total_size > 0 {
        resume_download(
            &client, url, resume_offset, total_size, &temp_file_path, cancel_flag, tx,
        )
        .await?
    } else if accept_ranges && total_size >= MIN_PARALLEL_SIZE {
        parallel_download(
            &client, url, total_size, &temp_dir, &temp_file_path, cancel_flag, tx,
        )
        .await?
    } else {
        stream_download(&client, url, total_size, &temp_file_path, cancel_flag, tx).await?
    };

    remove_resume_file(&temp_dir);

    let _ = tx.send(DownloadEvent::DownloadFinished { total: downloaded });

    if cancel_flag.load(Ordering::Relaxed) {
        return Err(JavaError::DownloadError("cancelled by user".to_string()));
    }

    // Extract archive
    let _ = tx.send(DownloadEvent::Extracting);

    let mut magic = [0u8; 2];
    let mut magic_file =
        fs::File::open(&temp_file_path)
            .map_err(|e| JavaError::DownloadError(format!("open temp file: {e}")))?;
    let read_len = std::io::Read::read(&mut magic_file, &mut magic)
        .map_err(|e| JavaError::DownloadError(format!("read magic bytes: {e}")))?;
    drop(magic_file);

    {
        let mut emit_progress = |current: u64, total: u64| {
            let _ = tx.send(DownloadEvent::ExtractProgress { current, total });
        };
        extract_downloaded_archive(
            &temp_file_path,
            &temp_dir,
            read_len,
            magic,
            cancel_flag,
            &mut emit_progress,
        )
        .map_err(JavaError::ExtractError)?;
    }

    // Remove temp download file so resolve_install_source sees only extracted content
    let _ = fs::remove_file(&temp_file_path);

    if cancel_flag.load(Ordering::Relaxed) {
        let _ = fs::remove_dir_all(&temp_dir);
        return Err(JavaError::DownloadError("cancelled by user".to_string()));
    }

    // Move to target directory
    let install_source = resolve_install_source(&temp_dir);

    if target_dir.exists() {
        fs::remove_dir_all(&target_dir)
            .map_err(|e| JavaError::DownloadError(format!("remove old install: {e}")))?;
    }

    move_directory(&install_source, &target_dir)
        .map_err(|e| JavaError::DownloadError(format!("move to target: {e}")))?;

    if install_source != temp_dir {
        let _ = fs::remove_dir_all(&temp_dir);
    }

    let java_bin = resolve_java_binary_path(&target_dir);
    if !java_bin.exists() {
        return Err(JavaError::DownloadError(format!(
            "java binary not found at {}",
            java_bin.display()
        )));
    }

    #[cfg(not(target_os = "windows"))]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = fs::metadata(&java_bin) {
            let mut perms = metadata.permissions();
            perms.set_mode(0o755);
            let _ = fs::set_permissions(&java_bin, perms);
        }
    }

    let _ = tx.send(DownloadEvent::Finished {
        java_bin: java_bin.clone(),
    });

    Ok(java_bin)
}

// ---------------------------------------------------------------------------
// Retry helper
// ---------------------------------------------------------------------------

async fn retry<F, Fut, T>(f: F, cancel_flag: &Arc<AtomicBool>) -> Result<T, JavaError>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = reqwest::Result<T>>,
{
    let mut attempt: usize = 0;
    loop {
        if cancel_flag.load(Ordering::Relaxed) {
            return Err(JavaError::DownloadError("cancelled by user".to_string()));
        }
        attempt += 1;
        match f().await {
            Ok(v) => return Ok(v),
            Err(e) => {
                if attempt >= DOWNLOAD_RETRY_LIMIT {
                    return Err(JavaError::DownloadError(format!(
                        "request failed after {attempt} attempts: {e}"
                    )));
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Strategy 1 – Serial streaming download (with optional resume)
// ---------------------------------------------------------------------------

async fn stream_download(
    client: &reqwest::Client,
    url: &str,
    total_size: u64,
    dest: &Path,
    cancel_flag: &Arc<AtomicBool>,
    tx: &flume::Sender<DownloadEvent>,
) -> Result<u64, JavaError> {
    let mut req = client.get(url);
    let resume_offset = read_resume_offset(dest.parent().unwrap_or(Path::new(""))).unwrap_or(0);
    if resume_offset > 0 {
        req = req.header(reqwest::header::RANGE, format!("bytes={resume_offset}-"));
    }

    let response = retry(|| req.try_clone().unwrap().send(), cancel_flag).await?;
    let total = total_size;

    let mut stream = response.bytes_stream();
    let mut downloaded = resume_offset;
    let mut last_emit = Instant::now();

    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dest)
        .await
        .map_err(|e| JavaError::DownloadError(format!("open file for streaming: {e}")))?;

    while let Some(chunk) = stream.next().await {
        if cancel_flag.load(Ordering::Relaxed) {
            return Err(JavaError::DownloadError("cancelled by user".to_string()));
        }
        let chunk = chunk.map_err(|e| JavaError::DownloadError(format!("stream error: {e}")))?;

        file.write_all(&chunk)
            .await
            .map_err(|e| JavaError::DownloadError(format!("write error: {e}")))?;

        downloaded += chunk.len() as u64;

        if total > 0 && last_emit.elapsed().as_millis() > 100 {
            let _ = tx.send(DownloadEvent::DownloadProgress { downloaded, total });
            last_emit = Instant::now();
        }

        // Persist resume offset every ~2 MB
        if downloaded % (2 * 1024 * 1024) < chunk.len() as u64 {
            if let Some(dir) = dest.parent() {
                let _ = write_resume_offset(dir, downloaded);
            }
        }
    }

    Ok(downloaded)
}

// ---------------------------------------------------------------------------
// Strategy 2 – Parallel chunked download (Range required)
// ---------------------------------------------------------------------------

async fn parallel_download(
    client: &reqwest::Client,
    url: &str,
    total_size: u64,
    temp_dir: &Path,
    final_dest: &Path,
    cancel_flag: &Arc<AtomicBool>,
    tx: &flume::Sender<DownloadEvent>,
) -> Result<u64, JavaError> {
    let num_chunks = NUM_PARALLEL_CHUNKS.min(total_size / (1024 * 1024) + 1);
    let chunk_size = total_size / num_chunks;
    let progress = Arc::new(AtomicU64::new(0));
    let mut chunk_paths = Vec::with_capacity(num_chunks as usize);

    let reporter_progress = progress.clone();
    let reporter_cancel = cancel_flag.clone();
    let reporter_tx = tx.clone();
    let reporter_handle = tokio::spawn(async move {
        let mut last = Instant::now();
        loop {
            tokio::time::sleep(Duration::from_millis(100)).await;
            if reporter_cancel.load(Ordering::Relaxed) {
                break;
            }
            let d = reporter_progress.load(Ordering::Relaxed);
            if last.elapsed().as_millis() > 100 {
                let _ = reporter_tx.send(DownloadEvent::DownloadProgress {
                    downloaded: d,
                    total: total_size,
                });
                last = Instant::now();
            }
            if d >= total_size {
                break;
            }
        }
    });

    let mut handles = Vec::with_capacity(num_chunks as usize);
    for i in 0..num_chunks {
        let start = i * chunk_size;
        let end = if i == num_chunks - 1 {
            total_size - 1
        } else {
            start + chunk_size - 1
        };

        let chunk_path = temp_dir.join(format!("chunk_{i:04}"));
        chunk_paths.push(chunk_path.clone());

        let client = client.clone();
        let url = url.to_string();
        let progress = progress.clone();
        let cancel = cancel_flag.clone();

        handles.push(tokio::spawn(async move {
            download_chunk(&client, &url, start, end, &chunk_path, &cancel).await?;
            progress.fetch_add((end - start + 1) as u64, Ordering::Relaxed);
            Ok::<_, JavaError>(())
        }));
    }

    for handle in handles {
        handle
            .await
            .map_err(|e| JavaError::DownloadError(format!("chunk task failed: {e}")))??;
    }

    let _ = reporter_handle.await;

    // Concatenate chunks
    concatenate_files(final_dest, &chunk_paths)
        .map_err(|e| JavaError::DownloadError(format!("concatenate chunks: {e}")))?;

    Ok(total_size)
}

async fn download_chunk(
    client: &reqwest::Client,
    url: &str,
    start: u64,
    end: u64,
    dest: &Path,
    cancel_flag: &Arc<AtomicBool>,
) -> Result<(), JavaError> {
    let response = retry(
        || {
            client
                .get(url)
                .header(reqwest::header::RANGE, format!("bytes={start}-{end}"))
                .send()
        },
        cancel_flag,
    )
    .await?;

    let mut stream = response.bytes_stream();
    let mut file = tokio::fs::File::create(dest)
        .await
        .map_err(|e| JavaError::DownloadError(format!("create chunk file: {e}")))?;

    while let Some(chunk) = stream.next().await {
        if cancel_flag.load(Ordering::Relaxed) {
            return Err(JavaError::DownloadError("cancelled by user".to_string()));
        }
        let chunk =
            chunk.map_err(|e| JavaError::DownloadError(format!("chunk stream error: {e}")))?;
        file.write_all(&chunk)
            .await
            .map_err(|e| JavaError::DownloadError(format!("chunk write error: {e}")))?;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Strategy 3 – Resume an interrupted serial download
// ---------------------------------------------------------------------------

async fn resume_download(
    client: &reqwest::Client,
    url: &str,
    resume_offset: u64,
    total_size: u64,
    dest: &Path,
    cancel_flag: &Arc<AtomicBool>,
    tx: &flume::Sender<DownloadEvent>,
) -> Result<u64, JavaError> {
    let response = retry(
        || {
            client
                .get(url)
                .header(reqwest::header::RANGE, format!("bytes={resume_offset}-"))
                .send()
        },
        cancel_flag,
    )
    .await?;

    let mut stream = response.bytes_stream();
    let mut downloaded = resume_offset;
    let mut last_emit = Instant::now();

    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dest)
        .await
        .map_err(|e| JavaError::DownloadError(format!("open file for resume: {e}")))?;

    while let Some(chunk) = stream.next().await {
        if cancel_flag.load(Ordering::Relaxed) {
            return Err(JavaError::DownloadError("cancelled by user".to_string()));
        }
        let chunk =
            chunk.map_err(|e| JavaError::DownloadError(format!("resume stream error: {e}")))?;

        file.write_all(&chunk)
            .await
            .map_err(|e| JavaError::DownloadError(format!("resume write error: {e}")))?;

        downloaded += chunk.len() as u64;

        if total_size > 0 && last_emit.elapsed().as_millis() > 100 {
            let _ = tx.send(DownloadEvent::DownloadProgress {
                downloaded,
                total: total_size,
            });
            last_emit = Instant::now();
        }

        if downloaded % (2 * 1024 * 1024) < chunk.len() as u64 {
            if let Some(dir) = dest.parent() {
                let _ = write_resume_offset(dir, downloaded);
            }
        }
    }

    Ok(downloaded)
}
