//! Shared file-system helpers for download extraction and resume.

use std::fs;
use std::path::{Path, PathBuf};

/// If `temp_dir` contains a single subdirectory, return that subdirectory.
///
/// This handles archives that wrap their contents in a top-level folder
/// (e.g. `jdk-17.0.2/...`), returning the inner folder so the JDK root
/// is placed directly in the runtime directory.
pub(super) fn resolve_install_source(temp_dir: &Path) -> PathBuf {
    if let Ok(entries) = fs::read_dir(temp_dir) {
        let entries: Vec<_> = entries.filter_map(|e| e.ok()).collect();
        if entries.len() == 1 && entries[0].path().is_dir() {
            return entries[0].path();
        }
    }
    temp_dir.to_path_buf()
}

/// Returns the platform-specific path to the `java` executable inside a
/// JDK installation directory.
///
/// On Windows this is `bin\java.exe`; on other platforms it is `bin/java`.
pub fn resolve_java_binary_path(target_dir: &Path) -> PathBuf {
    if cfg!(target_os = "windows") {
        target_dir.join("bin").join("java.exe")
    } else {
        target_dir.join("bin").join("java")
    }
}

/// Reads a previously-persisted download resume offset for the given directory.
///
/// The offset is stored in a file named `download.resume` inside `dir`.
pub(super) fn read_resume_offset(dir: &Path) -> Option<u64> {
    let path = dir.join("download.resume");
    let content = fs::read_to_string(path).ok()?;
    content.trim().parse().ok()
}

/// Writes a download resume offset to `download.resume` inside `dir`.
pub(super) fn write_resume_offset(dir: &Path, offset: u64) -> Result<(), String> {
    let path = dir.join("download.resume");
    fs::write(&path, offset.to_string()).map_err(|e| format!("failed to write resume info: {}", e))
}

/// Removes the resume file (`download.resume`) from the given directory.
pub(super) fn remove_resume_file(dir: &Path) {
    let path = dir.join("download.resume");
    let _ = fs::remove_file(&path);
}

/// Moves (or copies + deletes on Windows) a directory from `src` to `dst`.
///
/// On Unix this is a simple rename. On Windows, filesystem rename across
/// mount points fails, so a recursive copy-then-delete is used instead.
pub(super) fn move_directory(src: &Path, dst: &Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        copy_recursively(src, dst)?;
        fs::remove_dir_all(src)
            .map_err(|e| format!("failed to clean up source directory: {}", e))?;
        return Ok(());
    }
    #[cfg(not(target_os = "windows"))]
    {
        return fs::rename(src, dst).map_err(|e| format!("failed to move directory: {}", e));
    }
    #[allow(unreachable_code)]
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_install_source_single_subdir() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("jdk-17");
        std::fs::create_dir(&sub).unwrap();
        let result = resolve_install_source(dir.path());
        assert_eq!(result, sub);
    }

    #[test]
    fn test_resolve_install_source_multiple_entries() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("file1.txt"), "").unwrap();
        std::fs::write(dir.path().join("file2.txt"), "").unwrap();
        let result = resolve_install_source(dir.path());
        // Multiple entries → return the dir itself
        assert_eq!(result, dir.path());
    }

    #[test]
    fn test_resolve_install_source_empty() {
        let dir = tempfile::tempdir().unwrap();
        let result = resolve_install_source(dir.path());
        assert_eq!(result, dir.path());
    }

    #[test]
    fn test_resolve_java_binary_path_windows() {
        let dir = Path::new("/opt/java");
        let result = resolve_java_binary_path(dir);
        if cfg!(windows) {
            assert_eq!(result, PathBuf::from("/opt/java/bin/java.exe"));
        } else {
            assert_eq!(result, PathBuf::from("/opt/java/bin/java"));
        }
    }

    #[test]
    fn test_resume_offset_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        write_resume_offset(dir.path(), 12345).unwrap();
        let result = read_resume_offset(dir.path());
        assert_eq!(result, Some(12345));
    }

    #[test]
    fn test_remove_resume_file() {
        let dir = tempfile::tempdir().unwrap();
        write_resume_offset(dir.path(), 42).unwrap();
        remove_resume_file(dir.path());
        assert!(read_resume_offset(dir.path()).is_none());
    }

    #[test]
    fn test_read_resume_offset_no_file() {
        let dir = tempfile::tempdir().unwrap();
        assert!(read_resume_offset(dir.path()).is_none());
    }

    #[test]
    fn test_concatenate_files() {
        let dir = tempfile::tempdir().unwrap();
        let chunk1 = dir.path().join("chunk1");
        let chunk2 = dir.path().join("chunk2");
        std::fs::write(&chunk1, b"hello ").unwrap();
        std::fs::write(&chunk2, b"world").unwrap();
        let output = dir.path().join("merged");
        concatenate_files(&output, &[chunk1, chunk2]).unwrap();
        let content = std::fs::read(&output).unwrap();
        assert_eq!(content, b"hello world");
        // Chunks should be deleted
        assert!(!dir.path().join("chunk1").exists());
        assert!(!dir.path().join("chunk2").exists());
    }

    #[test]
    fn test_copy_recursively_file() {
        let src_dir = tempfile::tempdir().unwrap();
        let dst_dir = tempfile::tempdir().unwrap();
        let src_file = src_dir.path().join("test.txt");
        std::fs::write(&src_file, b"content").unwrap();
        let dst_file = dst_dir.path().join("test.txt");
        copy_recursively(&src_file, &dst_file).unwrap();
        assert_eq!(std::fs::read(&dst_file).unwrap(), b"content");
    }

    #[test]
    fn test_copy_recursively_directory() {
        let src_dir = tempfile::tempdir().unwrap();
        let sub = src_dir.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(sub.join("a.txt"), b"a").unwrap();
        let dst_dir = tempfile::tempdir().unwrap();
        let dst_child = dst_dir.path().join("copied");
        copy_recursively(src_dir.path(), &dst_child).unwrap();
        assert!(dst_child.join("sub").join("a.txt").exists());
    }

    #[test]
    fn test_move_directory() {
        let src_dir = tempfile::tempdir().unwrap();
        let dst_dir = tempfile::tempdir().unwrap();
        let nested = src_dir.path().join("nested");
        std::fs::create_dir(&nested).unwrap();
        std::fs::write(nested.join("file.txt"), b"data").unwrap();
        let dest = dst_dir.path().join("moved");
        move_directory(src_dir.path(), &dest).unwrap();
        assert!(dest.join("nested").join("file.txt").exists());
        assert!(!src_dir.path().exists());
    }
}

/// Recursively copies a file or directory from `src` to `dst`.
///
/// Directories are created as needed. Existing files at `dst` are overwritten.
fn copy_recursively(src: &Path, dst: &Path) -> Result<(), String> {
    if src.is_file() {
        fs::copy(src, dst).map_err(|e| format!("failed to copy file: {}", e))?;
        return Ok(());
    }
    fs::create_dir_all(dst).map_err(|e| format!("failed to create directory: {}", e))?;
    for entry in fs::read_dir(src).map_err(|e| format!("failed to read directory: {}", e))? {
        let entry = entry.map_err(|e| format!("failed to read directory entry: {}", e))?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            copy_recursively(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path).map_err(|e| format!("failed to copy file: {}", e))?;
        }
    }
    Ok(())
}

/// Concatenates multiple chunk files into a single output file, then deletes the chunks.
pub(super) fn concatenate_files(output: &Path, inputs: &[PathBuf]) -> Result<(), String> {
    let mut out = fs::File::create(output)
        .map_err(|e| format!("failed to create concatenated file: {}", e))?;
    for input in inputs {
        let mut file =
            fs::File::open(input).map_err(|e| format!("failed to open chunk file: {}", e))?;
        std::io::copy(&mut file, &mut out)
            .map_err(|e| format!("failed to concatenate chunk files: {}", e))?;
        let _ = fs::remove_file(input);
    }
    Ok(())
}
