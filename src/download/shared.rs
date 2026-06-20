use std::fs;
use std::path::{Path, PathBuf};

pub(super) fn resolve_install_source(temp_dir: &Path) -> PathBuf {
    if let Ok(entries) = fs::read_dir(temp_dir) {
        let entries: Vec<_> = entries.filter_map(|e| e.ok()).collect();
        if entries.len() == 1 && entries[0].path().is_dir() {
            return entries[0].path();
        }
    }
    temp_dir.to_path_buf()
}

pub fn resolve_java_binary_path(target_dir: &Path) -> PathBuf {
    if cfg!(target_os = "windows") {
        target_dir.join("bin").join("java.exe")
    } else {
        target_dir.join("bin").join("java")
    }
}

pub(super) fn read_resume_offset(dir: &Path) -> Option<u64> {
    let path = dir.join("download.resume");
    let content = fs::read_to_string(path).ok()?;
    content.trim().parse().ok()
}

pub(super) fn write_resume_offset(dir: &Path, offset: u64) -> Result<(), String> {
    let path = dir.join("download.resume");
    fs::write(&path, offset.to_string()).map_err(|e| format!("写入续传信息失败：{}", e))
}

pub(super) fn remove_resume_file(dir: &Path) {
    let path = dir.join("download.resume");
    let _ = fs::remove_file(&path);
}

pub(super) fn move_directory(src: &Path, dst: &Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        copy_recursively(src, dst)?;
        fs::remove_dir_all(src).map_err(|e| format!("清理源目录失败：{}", e))?;
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        fs::rename(src, dst).map_err(|e| format!("移动目录失败：{}", e))
    }
}

fn copy_recursively(src: &Path, dst: &Path) -> Result<(), String> {
    if src.is_file() {
        fs::copy(src, dst).map_err(|e| format!("复制文件失败：{}", e))?;
        return Ok(());
    }
    fs::create_dir_all(dst).map_err(|e| format!("创建目录失败：{}", e))?;
    for entry in fs::read_dir(src).map_err(|e| format!("读取目录失败：{}", e))? {
        let entry = entry.map_err(|e| format!("读取目录项失败：{}", e))?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            copy_recursively(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)
                .map_err(|e| format!("复制文件失败：{}", e))?;
        }
    }
    Ok(())
}

pub(super) fn concatenate_files(output: &Path, inputs: &[PathBuf]) -> Result<(), String> {
    let mut out =
        fs::File::create(output).map_err(|e| format!("创建合并文件失败：{}", e))?;
    for input in inputs {
        let mut file =
            fs::File::open(input).map_err(|e| format!("打开分块文件失败：{}", e))?;
        std::io::copy(&mut file, &mut out)
            .map_err(|e| format!("合并分块文件失败：{}", e))?;
        let _ = fs::remove_file(input);
    }
    Ok(())
}
