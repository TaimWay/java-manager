use std::fs;
use std::io::BufReader;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[cfg(not(target_os = "windows"))]
use {flate2::read::GzDecoder, tar::Archive};

#[cfg(target_os = "windows")]
use zip::ZipArchive;

pub(super) fn extract_downloaded_archive(
    temp_file_path: &Path,
    temp_dir: &Path,
    read_len: usize,
    magic: [u8; 2],
    cancel_flag: &Arc<AtomicBool>,
    on_progress: &mut dyn FnMut(u64, u64),
) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        if read_len >= 2 && &magic == b"PK" {
            return extract_zip(temp_file_path, temp_dir, cancel_flag, on_progress);
        }
        Err("not a valid ZIP archive".to_string())
    }

    #[cfg(not(target_os = "windows"))]
    {
        if read_len >= 2 && magic[0] == 0x1f && magic[1] == 0x8b {
            return extract_tar_gz(temp_file_path, temp_dir, cancel_flag);
        }
        Err("not a valid tar.gz archive".to_string())
    }
}

#[cfg(target_os = "windows")]
fn extract_zip(
    zip_path: &Path,
    target_dir: &Path,
    cancel_flag: &Arc<AtomicBool>,
    on_progress: &mut dyn FnMut(u64, u64),
) -> Result<(), String> {
    let file = fs::File::open(zip_path).map_err(|e| format!("open zip file: {e}"))?;
    let reader = BufReader::new(file);
    let mut archive = ZipArchive::new(reader).map_err(|e| format!("parse zip: {e}"))?;

    let total = archive.len() as u64;

    for i in 0..total as usize {
        if cancel_flag.load(Ordering::Relaxed) {
            return Err("cancelled by user".to_string());
        }
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("read zip entry: {e}"))?;
        let outpath = target_dir.join(file.mangled_name());

        if file.name().ends_with('/') {
            fs::create_dir_all(&outpath).map_err(|e| format!("create dir: {e}"))?;
        } else {
            if let Some(p) = outpath.parent() {
                if !p.exists() {
                    fs::create_dir_all(p).map_err(|e| format!("create parent dir: {e}"))?;
                }
            }
            let mut outfile =
                fs::File::create(&outpath).map_err(|e| format!("create file: {e}"))?;
            std::io::copy(&mut file, &mut outfile)
                .map_err(|e| format!("write file: {e}"))?;
        }

        on_progress(i as u64 + 1, total);
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn extract_tar_gz(
    archive_path: &Path,
    target_dir: &Path,
    cancel_flag: &Arc<AtomicBool>,
) -> Result<(), String> {
    let file =
        fs::File::open(archive_path).map_err(|e| format!("open archive: {e}"))?;
    let buf_reader = BufReader::new(file);
    let tar = GzDecoder::new(buf_reader);
    let mut archive = Archive::new(tar);

    if cancel_flag.load(Ordering::Relaxed) {
        return Err("cancelled by user".to_string());
    }
    archive
        .unpack(target_dir)
        .map_err(|e| format!("extract: {e}"))?;
    Ok(())
}
