use crate::{JavaError, JavaInfo};
use std::env;

pub fn quick_search() -> Result<Vec<JavaInfo>, JavaError> {
    let mut results: Vec<JavaInfo> = Vec::new();

    // Search for 'java' in all directories listed in PATH
    if let Ok(paths) = env::var("PATH") {
        for path in env::split_paths(&paths) {
            let java_exe = if cfg!(windows) { "java.exe" } else { "java" };
            let java_path = path.join(java_exe);

            if java_path.is_file() {
                // Attempt to build JavaInfo; skip if it fails (e.g., missing release file)
                if let Ok(info) = JavaInfo::new(java_path.to_string_lossy().to_string()) {
                    results.push(info);
                }
            }
        }
    }

    // Additional common locations can be added here if desired

    Ok(results)
}

pub fn deep_search() -> Result<Vec<JavaInfo>, JavaError> {
    let mut results: Vec<JavaInfo> = Vec::new();

    // Windows
    #[cfg(target_os = "windows")]
    {
        use everything_sdk::*;

        // Acquire lock on Everything global state
        let mut everything = global().try_lock().map_err(|_| {
            JavaError::RuntimeError("Failed to lock Everything global state".to_string())
        })?;

        // Check if database is loaded
        match everything.is_db_loaded() {
            Ok(false) => {
                return Err(JavaError::ExecuteError(
                    "Everything database is not fully loaded".to_string(),
                ));
            }
            Err(EverythingError::Ipc) => {
                return Err(JavaError::ExecuteError(
                    "Everything is not running in the background. Please start Everything.exe"
                        .to_string(),
                ));
            }
            _ => {}
        }

        // Create searcher and set search criteria
        let mut searcher = everything.searcher();
        searcher.set_search("\"java.exe\" !C:\\Windows\\");
        searcher.set_request_flags(
            RequestFlags::EVERYTHING_REQUEST_FILE_NAME
                | RequestFlags::EVERYTHING_REQUEST_PATH
                | RequestFlags::EVERYTHING_REQUEST_SIZE,
        );
        searcher.set_sort(SortType::EVERYTHING_SORT_NAME_ASCENDING);

        // Ensure case-insensitive matching (default)
        assert_eq!(searcher.get_match_case(), false);

        // Execute query
        let query_results = searcher.query();

        // Collect results
        
        for item in query_results.iter() {
            if let Ok(path) = item.filepath() {
                if let Ok(info) = JavaInfo::new(path.to_string_lossy().to_string()) {
                    results.push(info);
                }
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        use walkdir::WalkDir;

        // Standard locations where Java is commonly installed on Linux
        let search_dirs = vec![
            "/usr/lib/jvm",
            "/usr/java",
            "/opt",
            "/usr/local",
        ];

        for dir in search_dirs {
            let path = std::path::Path::new(dir);
            if !path.exists() {
                continue;
            }

            for entry in WalkDir::new(path)
                .follow_links(true)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                let entry_path = entry.path();
                // Look for files exactly named "java"
                if entry_path.file_name() != Some(std::ffi::OsStr::new("java")) {
                    continue;
                }
                if !entry_path.is_file() {
                    continue;
                }

                // On Unix, also verify the file is executable
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Ok(metadata) = entry_path.metadata() {
                        let permissions = metadata.permissions();
                        if permissions.mode() & 0o111 != 0 {
                            if let Ok(info) = JavaInfo::new(entry_path.to_string_lossy().to_string()) {
                                results.push(info);
                            }
                        }
                    }
                }
                #[cfg(not(unix))]
                {
                    if let Ok(info) = JavaInfo::new(entry_path.to_string_lossy().to_string()) {
                        results.push(info);
                    }
                }
            }
        }
    }

    Ok(results)
}
