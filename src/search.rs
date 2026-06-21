//! System-level discovery of Java installations.
//!
//! Provides three tiers of search:
//!
//! | Function | Scope |
//! |---|---|
//! | [`quick_search`] | Walks every directory in `$PATH` looking for `java` (fastest) |
//! | [`deep_search`] | Windows: Everything SDK (falls back to [`full_search`]).<br>Linux/macOS: delegates to [`full_search`] |
//! | [`full_search`] | Registry, keyword BFS, Microsoft Store, `where` command,<br>package managers (Chocolatey, Scoop, SDKMAN, Homebrew,<br>JBang, asdf-vm), JetBrains IDE-bundled JDKs,<br>JVM directories, Minecraft runtime |
//!
//! Search results are automatically deduplicated: when both a JDK and its
//! bundled JRE are found, only the JDK-level entry is returned.

use crate::{JavaError, JavaInfo};
use log::debug;
use std::collections::VecDeque;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const MAX_SEARCH_DEPTH: usize = 6;

const JAVA_KEYWORDS: &[&str] = &["java", "jdk", "jre", "jvm", "openjdk", "graalvm"];

const EXCLUDE_FOLDERS: &[&str] = &[
    "node_modules",
    ".git",
    "__pycache__",
    "vendor",
    "cache",
    "temp",
    "tmp",
    "logs",
    "log",
];

/// Searches for Java installations by scanning every directory in `$PATH`.
///
/// This is the fastest search method — it only checks what is immediately
/// available on the system `PATH`. On Windows it looks for `java.exe`;
/// on Linux and macOS it looks for `java`.
///
/// # Returns
///
/// A `Vec<JavaInfo>` for every valid `java` executable found in `PATH`.
///
/// # Errors
///
/// Returns [`JavaError::IoError`] if a file-system operation fails when
/// inspecting a candidate path. Individual invalid or unreadable paths are
/// skipped silently.
///
/// # Example
///
/// ```no_run
/// use java_manager::quick_search;
///
/// let javas = quick_search()?;
/// for java in javas {
///     println!("{} at {}", java.version, java.path.display());
/// }
/// # Ok::<_, java_manager::JavaError>(())
/// ```
pub fn quick_search() -> Result<Vec<JavaInfo>, JavaError> {
    let mut results: Vec<JavaInfo> = Vec::new();

    if let Ok(paths) = env::var("PATH") {
        for path in env::split_paths(&paths) {
            let java_exe = if cfg!(windows) { "java.exe" } else { "java" };
            let java_path = path.join(java_exe);

            debug!("quick_search: checking PATH entry {:?}", java_path);
            if java_path.is_file()
                && let Ok(info) = JavaInfo::new(java_path.to_string_lossy().to_string())
            {
                debug!(
                    "quick_search: found {} at {}",
                    info.version,
                    java_path.display()
                );
                results.push(info);
            }
        }
    }

    debug!("quick_search: found {} Java installation(s)", results.len());
    Ok(results)
}

/// Remove nested JRE entries that are bundled under a parent JDK.
///
/// When both `jdk-xxx/bin/java.exe` and `jdk-xxx/jre/bin/java.exe` are
/// discovered, only the JDK-level installation is kept. Standalone JREs
/// (whose `java_home` is not a subdirectory of another entry) are unaffected.
fn dedup_nested(javas: Vec<JavaInfo>) -> Vec<JavaInfo> {
    let mut keep = vec![true; javas.len()];
    for i in 0..javas.len() {
        for j in 0..javas.len() {
            if i != j && javas[i].java_home.starts_with(&javas[j].java_home) {
                keep[i] = false;
            }
        }
    }
    javas
        .into_iter()
        .enumerate()
        .filter(|(i, _)| keep[*i])
        .map(|(_, j)| j)
        .collect()
}

/// Performs a deep, platform-aware search for Java installations.
///
/// **Windows (with `everything` feature)**: Uses the Everything SDK for
/// near-instant results. If the Everything service is not running or the
/// SDK is unavailable, automatically falls back to [`full_search`].
///
/// **Windows (without `everything`)**: Delegates directly to [`full_search`].
///
/// **Linux / macOS**: Delegates directly to [`full_search`], which walks
/// standard JVM directories and checks SDKMAN, JBang, asdf-vm, Homebrew,
/// JetBrains IDE-bundled JDKs, and Minecraft runtimes.
///
/// # Returns
///
/// A `Vec<JavaInfo>` for every valid Java installation found.
///
/// # Errors
///
/// Returns [`JavaError::IoError`] if file-system operations fail.
/// On Windows (with the `everything` feature), an Everything IPC error is
/// silently caught and falls back to [`full_search`] — this never propagates
/// to the caller.
///
/// # Example
///
/// ```no_run
/// use java_manager::deep_search;
///
/// let javas = deep_search()?;
/// println!("Found {} Java installation(s)", javas.len());
/// # Ok::<_, java_manager::JavaError>(())
/// ```
pub fn deep_search() -> Result<Vec<JavaInfo>, JavaError> {
    #[cfg(target_os = "windows")]
    {
        #[cfg(feature = "everything")]
        match deep_search_everything() {
            Ok(results) => return Ok(results),
            Err(e) => {
                debug!(
                    "deep_search: Everything SDK unavailable ({}), falling back to full_search",
                    e
                );
            }
        }
        full_search()
    }

    #[cfg(target_os = "linux")]
    {
        full_search()
    }

    #[cfg(target_os = "macos")]
    {
        full_search()
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        Ok(Vec::new())
    }
}

/// Performs a comprehensive, multi-strategy scan for Java installations.
///
/// This is the most thorough search method. It checks `$JAVA_HOME` first,
/// then applies every available discovery strategy for the current platform:
///
/// **Windows**: Registry (HKLM + HKCU for JavaSoft, Azul, BellSoft, Temurin,
/// Corretto, GraalVM) → Keyword BFS on all drives → Microsoft Store →
/// `where java` → Chocolatey → Scoop → JetBrains IDE-bundled JDK.
///
/// **Linux**: `/usr/lib/jvm` → `/usr/java` → `/opt` → `/usr/local` →
/// SDKMAN → JBang → asdf-vm → JetBrains IDE-bundled JDK → Minecraft runtime.
///
/// **macOS**: `/Library/Java/JavaVirtualMachines` →
/// `/usr/libexec/java_home` → SDKMAN → JBang → asdf-vm → Homebrew →
/// JetBrains IDE-bundled JDK → Minecraft runtime.
///
/// Results are automatically deduplicated — when both a JDK and its bundled
/// JRE are discovered, only the JDK-level entry is kept.
///
/// # Returns
///
/// A `Vec<JavaInfo>` for every valid Java installation found.
///
/// # Errors
///
/// Returns [`JavaError::IoError`] if file-system operations fail.
/// Individual unreadable paths are skipped silently.
///
/// # Example
///
/// ```no_run
/// use java_manager::full_search;
///
/// let javas = full_search()?;
/// for java in &javas {
///     println!("{:25} {}", java.name, java.version);
/// }
/// # Ok::<_, java_manager::JavaError>(())
/// ```
pub fn full_search() -> Result<Vec<JavaInfo>, JavaError> {
    let mut paths = Vec::new();

    if let Some(jh) = env::var_os("JAVA_HOME") {
        let java_exe =
            Path::new(&jh)
                .join("bin")
                .join(if cfg!(windows) { "java.exe" } else { "java" });
        let jh_str = java_exe.to_string_lossy();
        debug!("full_search: checking JAVA_HOME at {jh_str}");
        if java_exe.exists() {
            debug!("full_search: JAVA_HOME -> {jh_str}");
            paths.push(java_exe);
        }
    }

    #[cfg(target_os = "windows")]
    {
        paths.extend(scan_registry());
        paths.extend(scan_default_paths());
        paths.extend(scan_chocolatey());
        paths.extend(scan_scoop());
        paths.extend(scan_jetbrains_windows());
        paths.extend(scan_microsoft_store());
        paths.extend(scan_where_command());
    }

    #[cfg(target_os = "linux")]
    {
        paths.extend(scan_linux());
        paths.extend(scan_sdkman_linux());
        paths.extend(scan_jbang_linux());
        paths.extend(scan_asdf_linux());
        paths.extend(scan_jetbrains_linux());
    }

    #[cfg(target_os = "macos")]
    {
        paths.extend(scan_macos());
        paths.extend(scan_sdkman_macos());
        paths.extend(scan_jbang_macos());
        paths.extend(scan_asdf_macos());
        paths.extend(scan_homebrew_macos());
        paths.extend(scan_jetbrains_macos());
    }

    paths.sort();
    paths.dedup();

    debug!(
        "full_search: {} candidate path(s) before JavaInfo resolution",
        paths.len()
    );

    let javas: Vec<JavaInfo> = paths
        .into_iter()
        .filter_map(|p| {
            let result = JavaInfo::new(p.to_string_lossy().to_string());
            if result.is_err() {
                debug!("full_search: skipping invalid path {}", p.display());
            }
            result.ok()
        })
        .collect();

    let count_before = javas.len();
    let javas = dedup_nested(javas);
    let removed = count_before - javas.len();
    if removed > 0 {
        debug!("full_search: removed {removed} nested JRE entr(ies)");
    }
    Ok(javas)
}

/// Same as [`full_search`] but parallelises `JavaInfo` resolution with rayon.
///
/// The search strategy is identical to [`full_search`]. The difference is that
/// `JavaInfo::new()` calls (which spawn a `java -version` process for each
/// candidate) run concurrently via rayon, significantly speeding up scans
/// when many candidates are discovered.
///
/// Only available with the `parallel` feature enabled.
///
/// # Returns
///
/// A `Vec<JavaInfo>` for every valid Java installation found.
///
/// # Errors
///
/// Returns [`JavaError::IoError`] if file-system operations fail.
///
/// # Example
///
/// ```no_run
/// # #[cfg(feature = "parallel")]
/// # {
/// use java_manager::parallel_full_search;
///
/// let javas = parallel_full_search()?;
/// println!("Found {} Java installation(s)", javas.len());
/// # }
/// # Ok::<_, java_manager::JavaError>(())
/// ```
#[cfg(feature = "parallel")]
pub fn parallel_full_search() -> Result<Vec<JavaInfo>, JavaError> {
    use rayon::prelude::*;

    let mut paths = Vec::new();

    if let Some(jh) = env::var_os("JAVA_HOME") {
        let java_exe =
            Path::new(&jh)
                .join("bin")
                .join(if cfg!(windows) { "java.exe" } else { "java" });
        let jh_str = java_exe.to_string_lossy();
        debug!("full_search: checking JAVA_HOME at {jh_str}");
        if java_exe.exists() {
            debug!("full_search: JAVA_HOME -> {jh_str}");
            paths.push(java_exe);
        }
    }

    #[cfg(target_os = "windows")]
    {
        paths.par_extend(scan_registry());
        paths.par_extend(scan_default_paths());
        paths.par_extend(scan_chocolatey());
        paths.par_extend(scan_scoop());
        paths.par_extend(scan_jetbrains_windows());
        paths.par_extend(scan_microsoft_store());
        paths.par_extend(scan_where_command());
    }

    #[cfg(target_os = "linux")]
    {
        paths.par_extend(scan_linux());
        paths.par_extend(scan_sdkman_linux());
        paths.par_extend(scan_jbang_linux());
        paths.par_extend(scan_asdf_linux());
        paths.par_extend(scan_jetbrains_linux());
    }

    #[cfg(target_os = "macos")]
    {
        paths.par_extend(scan_macos());
        paths.par_extend(scan_sdkman_macos());
        paths.par_extend(scan_jbang_macos());
        paths.par_extend(scan_asdf_macos());
        paths.par_extend(scan_homebrew_macos());
        paths.par_extend(scan_jetbrains_macos());
    }

    paths.par_sort_unstable();
    paths.dedup();

    debug!(
        "parallel_full_search: {} candidate path(s) before JavaInfo resolution",
        paths.len()
    );

    let javas: Vec<JavaInfo> = paths
        .into_par_iter()
        .filter_map(|p| {
            let result = JavaInfo::new(p.to_string_lossy().to_string());
            if result.is_err() {
                debug!(
                    "parallel_full_search: skipping invalid path {}",
                    p.display()
                );
            }
            result.ok()
        })
        .collect();

    let count_before = javas.len();
    let javas = dedup_nested(javas);
    let removed = count_before - javas.len();
    if removed > 0 {
        debug!("parallel_full_search: removed {removed} nested JRE entr(ies)");
    }
    Ok(javas)
}

#[cfg(target_os = "windows")]
#[cfg(feature = "everything")]
fn deep_search_everything() -> Result<Vec<JavaInfo>, JavaError> {
    use everything_sdk::*;

    let mut results: Vec<JavaInfo> = Vec::new();
    let mut everything = global().try_lock().map_err(|_| {
        JavaError::RuntimeError("Failed to lock Everything global state".to_string())
    })?;

    match everything.is_db_loaded() {
        Ok(false) => {
            debug!("deep_search_everything: database not fully loaded");
            return Err(JavaError::ExecuteError(
                "Everything database is not fully loaded".to_string(),
            ));
        }
        Err(EverythingError::Ipc) => {
            debug!("deep_search_everything: Everything IPC unavailable (not running)");
            return Err(JavaError::ExecuteError(
                "Everything is not running in the background. Please start Everything.exe"
                    .to_string(),
            ));
        }
        _ => {}
    }

    debug!("deep_search_everything: querying Everything SDK");
    let mut searcher = everything.searcher();
    searcher.set_search("\"java.exe\" !C:\\Windows\\");
    searcher.set_request_flags(
        RequestFlags::EVERYTHING_REQUEST_FILE_NAME
            | RequestFlags::EVERYTHING_REQUEST_PATH
            | RequestFlags::EVERYTHING_REQUEST_SIZE,
    );
    searcher.set_sort(SortType::EVERYTHING_SORT_NAME_ASCENDING);

    assert!(!searcher.get_match_case());

    let query_results = searcher.query();

    for item in query_results.iter() {
        if let Ok(path) = item.filepath()
            && let Ok(info) = JavaInfo::new(path.to_string_lossy().to_string())
        {
            debug!(
                "deep_search_everything: found {} at {}",
                info.version,
                path.display()
            );
            results.push(info);
        }
    }

    debug!(
        "deep_search_everything: found {} Java installation(s)",
        results.len()
    );
    Ok(results)
}

#[cfg(target_os = "windows")]
fn scan_registry() -> Vec<PathBuf> {
    use winreg::RegKey;
    use winreg::enums::*;

    let mut results = Vec::new();

    let entries: &[(*mut std::ffi::c_void, &str, &str)] = &[
        // JavaSoft — HKLM
        (
            HKEY_LOCAL_MACHINE,
            r"SOFTWARE\JavaSoft\Java Development Kit",
            "JavaHome",
        ),
        (
            HKEY_LOCAL_MACHINE,
            r"SOFTWARE\JavaSoft\Java Runtime Environment",
            "JavaHome",
        ),
        (
            HKEY_LOCAL_MACHINE,
            r"SOFTWARE\WOW6432Node\JavaSoft\Java Development Kit",
            "JavaHome",
        ),
        (
            HKEY_LOCAL_MACHINE,
            r"SOFTWARE\WOW6432Node\JavaSoft\Java Runtime Environment",
            "JavaHome",
        ),
        // JavaSoft — HKCU
        (
            HKEY_CURRENT_USER,
            r"SOFTWARE\JavaSoft\Java Development Kit",
            "JavaHome",
        ),
        (
            HKEY_CURRENT_USER,
            r"SOFTWARE\JavaSoft\Java Runtime Environment",
            "JavaHome",
        ),
        (
            HKEY_CURRENT_USER,
            r"SOFTWARE\WOW6432Node\JavaSoft\Java Development Kit",
            "JavaHome",
        ),
        (
            HKEY_CURRENT_USER,
            r"SOFTWARE\WOW6432Node\JavaSoft\Java Runtime Environment",
            "JavaHome",
        ),
        // Brand-specific
        (
            HKEY_LOCAL_MACHINE,
            r"SOFTWARE\Azul Systems\Zulu",
            "InstallationPath",
        ),
        (
            HKEY_LOCAL_MACHINE,
            r"SOFTWARE\BellSoft\Liberica",
            "InstallationPath",
        ),
        (
            HKEY_LOCAL_MACHINE,
            r"SOFTWARE\Eclipse Foundation\Temurin",
            "InstallationPath",
        ),
        (
            HKEY_CURRENT_USER,
            r"SOFTWARE\Eclipse Foundation\Temurin",
            "InstallationPath",
        ),
        (
            HKEY_LOCAL_MACHINE,
            r"SOFTWARE\Amazon Corretto",
            "InstallationPath",
        ),
        (
            HKEY_CURRENT_USER,
            r"SOFTWARE\Amazon Corretto",
            "InstallationPath",
        ),
        (HKEY_LOCAL_MACHINE, r"SOFTWARE\GraalVM", "InstallationPath"),
        (HKEY_CURRENT_USER, r"SOFTWARE\GraalVM", "InstallationPath"),
    ];

    for &(root, subpath, value_name) in entries {
        debug!("scan_registry: checking {subpath}");
        let key = match RegKey::predef(root).open_subkey(subpath) {
            Ok(k) => k,
            Err(_) => continue,
        };
        for subkey_name in key.enum_keys().filter_map(|k| k.ok()) {
            let subkey = match key.open_subkey(subkey_name) {
                Ok(sk) => sk,
                Err(_) => continue,
            };
            let mut install_path: String = match subkey.get_value(value_name) {
                Ok(v) => v,
                Err(_) => continue,
            };
            install_path = install_path.trim_end_matches('\\').to_string();
            let java_exe = Path::new(&install_path).join("bin").join("java.exe");
            if java_exe.exists() {
                debug!("scan_registry: found valid Java at {install_path}");
                results.push(java_exe);
            } else {
                debug!("scan_registry: stale registry entry at {install_path}");
            }
        }
    }

    results
}

// -----------------------------------------------------------------------------
// Chocolatey (Windows)
// -----------------------------------------------------------------------------
#[cfg(target_os = "windows")]
fn scan_chocolatey() -> Vec<PathBuf> {
    let prog_data = match env::var_os("ProgramData") {
        Some(v) => Path::new(&v).join("chocolatey").join("lib"),
        None => return Vec::new(),
    };

    debug!("scan_chocolatey: checking {}", prog_data.display());
    if !prog_data.exists() {
        return Vec::new();
    }

    let mut results = Vec::new();
    if let Ok(entries) = fs::read_dir(&prog_data) {
        for entry in entries.flatten() {
            let dir = entry.path();
            if !dir.is_dir() {
                continue;
            }
            let java_exe = dir.join("bin").join("java.exe");
            if java_exe.exists() {
                debug!("scan_chocolatey: found Java at {}", java_exe.display());
                results.push(java_exe);
            }
        }
    }
    results
}

// -----------------------------------------------------------------------------
// Scoop (Windows)
// -----------------------------------------------------------------------------
#[cfg(target_os = "windows")]
fn scan_scoop() -> Vec<PathBuf> {
    let scoop_home = if let Some(v) = env::var_os("SCOOP") {
        Path::new(&v).to_path_buf()
    } else if let Some(v) = env::var_os("USERPROFILE") {
        Path::new(&v).join("scoop").join("apps")
    } else {
        return Vec::new();
    };

    debug!("scan_scoop: checking {}", scoop_home.display());
    if !scoop_home.exists() {
        return Vec::new();
    }

    let mut results = Vec::new();
    if let Ok(entries) = fs::read_dir(&scoop_home) {
        for entry in entries.flatten() {
            let dir_name = entry.file_name().to_string_lossy().to_lowercase();
            if !JAVA_KEYWORDS.iter().any(|kw| dir_name.contains(kw)) {
                continue;
            }
            let current_java = entry.path().join("current").join("bin").join("java.exe");
            if current_java.exists() {
                debug!("scan_scoop: found Java at {}", current_java.display());
                results.push(current_java);
            }
        }
    }
    results
}

// -----------------------------------------------------------------------------
// JetBrains IDE-bundled JDK (Windows)
// -----------------------------------------------------------------------------
#[cfg(target_os = "windows")]
fn scan_jetbrains_windows() -> Vec<PathBuf> {
    let mut results = Vec::new();

    let search_roots: Vec<PathBuf> = {
        let mut roots = Vec::new();
        if let Some(localappdata) = env::var_os("LOCALAPPDATA") {
            roots.push(Path::new(&localappdata).join("JetBrains"));
        }
        if let Some(prog_files) = env::var_os("ProgramFiles") {
            roots.push(Path::new(&prog_files).join("JetBrains"));
        }
        if let Some(prog_files_x86) = env::var_os("ProgramFiles(x86)") {
            roots.push(Path::new(&prog_files_x86).join("JetBrains"));
        }
        roots
    };

    for root in search_roots {
        if !root.exists() {
            continue;
        }
        debug!("scan_jetbrains_windows: checking {}", root.display());
        if let Ok(entries) = fs::read_dir(&root) {
            for entry in entries.flatten() {
                let jbr = entry.path().join("jbr").join("bin").join("java.exe");
                if jbr.exists() {
                    debug!("scan_jetbrains_windows: found Java at {}", jbr.display());
                    results.push(jbr);
                }
            }
        }
    }
    results
}

// -----------------------------------------------------------------------------
// SDKMAN (Linux)
// -----------------------------------------------------------------------------
#[cfg(target_os = "linux")]
fn scan_sdkman_linux() -> Vec<PathBuf> {
    let home = match env::var("HOME") {
        Ok(h) => h,
        Err(_) => return Vec::new(),
    };
    let sdkman_candidates = Path::new(&home)
        .join(".sdkman")
        .join("candidates")
        .join("java");
    if !sdkman_candidates.exists() {
        return Vec::new();
    }

    debug!(
        "scan_sdkman_linux: checking {}",
        sdkman_candidates.display()
    );
    let mut results = Vec::new();

    if let Ok(entries) = fs::read_dir(&sdkman_candidates) {
        for entry in entries.flatten() {
            let java_exe = entry.path().join("bin").join("java");
            if java_exe.exists() {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Ok(metadata) = java_exe.metadata() {
                        if metadata.permissions().mode() & 0o111 != 0 {
                            debug!("scan_sdkman_linux: found Java at {}", java_exe.display());
                            results.push(java_exe);
                        }
                    }
                }
                #[cfg(not(unix))]
                {
                    results.push(java_exe);
                }
            }
        }
    }
    results
}

// -----------------------------------------------------------------------------
// JetBrains IDE-bundled JDK (Linux)
// -----------------------------------------------------------------------------
#[cfg(target_os = "linux")]
fn scan_jetbrains_linux() -> Vec<PathBuf> {
    let mut results = Vec::new();

    let search_roots: Vec<PathBuf> = {
        let mut roots: Vec<PathBuf> = vec![Path::new("/opt").join("JetBrains")];
        if let Ok(home) = env::var("HOME") {
            roots.push(
                Path::new(&home)
                    .join(".local")
                    .join("share")
                    .join("JetBrains"),
            );
        }
        roots
    };

    for root in &search_roots {
        if !root.exists() {
            continue;
        }
        debug!("scan_jetbrains_linux: checking {}", root.display());
        if let Ok(entries) = fs::read_dir(root) {
            for entry in entries.flatten() {
                let jbr = entry.path().join("jbr").join("bin").join("java");
                if jbr.exists() {
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        if let Ok(metadata) = jbr.metadata() {
                            if metadata.permissions().mode() & 0o111 != 0 {
                                debug!("scan_jetbrains_linux: found Java at {}", jbr.display());
                                results.push(jbr);
                            }
                        }
                    }
                    #[cfg(not(unix))]
                    {
                        results.push(jbr);
                    }
                }
            }
        }
    }

    results
}

// -----------------------------------------------------------------------------
// SDKMAN (macOS)
// -----------------------------------------------------------------------------
#[cfg(target_os = "macos")]
fn scan_sdkman_macos() -> Vec<PathBuf> {
    let home = match env::var("HOME") {
        Ok(h) => h,
        Err(_) => return Vec::new(),
    };
    let sdkman_candidates = Path::new(&home)
        .join(".sdkman")
        .join("candidates")
        .join("java");
    if !sdkman_candidates.exists() {
        return Vec::new();
    }

    debug!(
        "scan_sdkman_macos: checking {}",
        sdkman_candidates.display()
    );
    let mut results = Vec::new();

    if let Ok(entries) = fs::read_dir(&sdkman_candidates) {
        for entry in entries.flatten() {
            let java_exe = entry.path().join("bin").join("java");
            if java_exe.exists() {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Ok(metadata) = java_exe.metadata() {
                        if metadata.permissions().mode() & 0o111 != 0 {
                            debug!("scan_sdkman_macos: found Java at {}", java_exe.display());
                            results.push(java_exe);
                        }
                    }
                }
                #[cfg(not(unix))]
                {
                    results.push(java_exe);
                }
            }
        }
    }
    results
}

// -----------------------------------------------------------------------------
// Homebrew (macOS)
// -----------------------------------------------------------------------------
#[cfg(target_os = "macos")]
fn scan_homebrew_macos() -> Vec<PathBuf> {
    let mut results = Vec::new();

    let brew_roots: &[&str] = &["/opt/homebrew/opt", "/usr/local/opt"];

    for root in brew_roots {
        let root_path = Path::new(root);
        if !root_path.exists() {
            continue;
        }
        debug!("scan_homebrew_macos: checking {}", root);
        if let Ok(entries) = fs::read_dir(root_path) {
            for entry in entries.flatten() {
                let dir_name = entry.file_name().to_string_lossy().to_lowercase();
                if dir_name.starts_with("openjdk") || dir_name.starts_with("java") {
                    let java_exe = entry.path().join("bin").join("java");
                    if java_exe.exists() {
                        #[cfg(unix)]
                        {
                            use std::os::unix::fs::PermissionsExt;
                            if let Ok(metadata) = java_exe.metadata() {
                                if metadata.permissions().mode() & 0o111 != 0 {
                                    debug!(
                                        "scan_homebrew_macos: found Java at {}",
                                        java_exe.display()
                                    );
                                    results.push(java_exe);
                                }
                            }
                        }
                        #[cfg(not(unix))]
                        {
                            results.push(java_exe);
                        }
                    }
                }
            }
        }
    }
    results
}

// -----------------------------------------------------------------------------
// JetBrains IDE-bundled JDK (macOS)
// -----------------------------------------------------------------------------
#[cfg(target_os = "macos")]
fn scan_jetbrains_macos() -> Vec<PathBuf> {
    let mut results = Vec::new();

    // /Applications/*.app/Contents/jbr/Contents/Home/bin/java
    debug!("scan_jetbrains_macos: checking /Applications");
    if let Ok(entries) = fs::read_dir("/Applications") {
        for entry in entries.flatten() {
            let app_name = entry.file_name().to_string_lossy().to_lowercase();
            if !app_name.contains("jetbrains")
                && !app_name.contains("intellij")
                && !app_name.contains("android studio")
            {
                continue;
            }
            let jbr_home = entry
                .path()
                .join("Contents")
                .join("jbr")
                .join("Contents")
                .join("Home");
            let java_exe = jbr_home.join("bin").join("java");
            if java_exe.exists() {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Ok(metadata) = java_exe.metadata() {
                        if metadata.permissions().mode() & 0o111 != 0 {
                            debug!("scan_jetbrains_macos: found Java at {}", java_exe.display());
                            results.push(java_exe);
                        }
                    }
                }
                #[cfg(not(unix))]
                {
                    results.push(java_exe);
                }
            }
        }
    }

    // ~/Library/Application Support/JetBrains/*/jbr/Contents/Home/bin/java
    if let Ok(home) = env::var("HOME") {
        let jetbrains_support = Path::new(&home)
            .join("Library")
            .join("Application Support")
            .join("JetBrains");
        if jetbrains_support.exists() {
            debug!(
                "scan_jetbrains_macos: checking {}",
                jetbrains_support.display()
            );
            if let Ok(entries) = fs::read_dir(&jetbrains_support) {
                for entry in entries.flatten() {
                    let jbr_home = entry.path().join("jbr").join("Contents").join("Home");
                    let java_exe = jbr_home.join("bin").join("java");
                    if java_exe.exists() {
                        #[cfg(unix)]
                        {
                            use std::os::unix::fs::PermissionsExt;
                            if let Ok(metadata) = java_exe.metadata() {
                                if metadata.permissions().mode() & 0o111 != 0 {
                                    debug!(
                                        "scan_jetbrains_macos: found Java at {}",
                                        java_exe.display()
                                    );
                                    results.push(java_exe);
                                }
                            }
                        }
                        #[cfg(not(unix))]
                        {
                            results.push(java_exe);
                        }
                    }
                }
            }
        }
    }

    results
}

// -----------------------------------------------------------------------------
// JBang (Linux)
// -----------------------------------------------------------------------------
#[cfg(target_os = "linux")]
fn scan_jbang_linux() -> Vec<PathBuf> {
    let home = match env::var("HOME") {
        Ok(h) => h,
        Err(_) => return Vec::new(),
    };
    let jbang_jdks = Path::new(&home).join(".jbang").join("cache").join("jdks");
    if !jbang_jdks.exists() {
        return Vec::new();
    }

    debug!("scan_jbang_linux: checking {}", jbang_jdks.display());
    let mut results = Vec::new();

    if let Ok(entries) = fs::read_dir(&jbang_jdks) {
        for entry in entries.flatten() {
            let java_exe = entry.path().join("bin").join("java");
            if java_exe.exists() {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Ok(metadata) = java_exe.metadata() {
                        if metadata.permissions().mode() & 0o111 != 0 {
                            debug!("scan_jbang_linux: found Java at {}", java_exe.display());
                            results.push(java_exe);
                        }
                    }
                }
                #[cfg(not(unix))]
                {
                    results.push(java_exe);
                }
            }
        }
    }
    results
}

// -----------------------------------------------------------------------------
// asdf-vm (Linux)
// -----------------------------------------------------------------------------
#[cfg(target_os = "linux")]
fn scan_asdf_linux() -> Vec<PathBuf> {
    let home = match env::var("HOME") {
        Ok(h) => h,
        Err(_) => return Vec::new(),
    };
    let asdf_java = Path::new(&home).join(".asdf").join("installs").join("java");
    if !asdf_java.exists() {
        return Vec::new();
    }

    debug!("scan_asdf_linux: checking {}", asdf_java.display());
    let mut results = Vec::new();

    if let Ok(entries) = fs::read_dir(&asdf_java) {
        for entry in entries.flatten() {
            let java_exe = entry.path().join("bin").join("java");
            if java_exe.exists() {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Ok(metadata) = java_exe.metadata() {
                        if metadata.permissions().mode() & 0o111 != 0 {
                            debug!("scan_asdf_linux: found Java at {}", java_exe.display());
                            results.push(java_exe);
                        }
                    }
                }
                #[cfg(not(unix))]
                {
                    results.push(java_exe);
                }
            }
        }
    }
    results
}

// -----------------------------------------------------------------------------
// JBang (macOS)
// -----------------------------------------------------------------------------
#[cfg(target_os = "macos")]
fn scan_jbang_macos() -> Vec<PathBuf> {
    let home = match env::var("HOME") {
        Ok(h) => h,
        Err(_) => return Vec::new(),
    };
    let jbang_jdks = Path::new(&home).join(".jbang").join("cache").join("jdks");
    if !jbang_jdks.exists() {
        return Vec::new();
    }

    debug!("scan_jbang_macos: checking {}", jbang_jdks.display());
    let mut results = Vec::new();

    if let Ok(entries) = fs::read_dir(&jbang_jdks) {
        for entry in entries.flatten() {
            let java_exe = entry.path().join("bin").join("java");
            if java_exe.exists() {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Ok(metadata) = java_exe.metadata() {
                        if metadata.permissions().mode() & 0o111 != 0 {
                            debug!("scan_jbang_macos: found Java at {}", java_exe.display());
                            results.push(java_exe);
                        }
                    }
                }
                #[cfg(not(unix))]
                {
                    results.push(java_exe);
                }
            }
        }
    }
    results
}

// -----------------------------------------------------------------------------
// asdf-vm (macOS)
// -----------------------------------------------------------------------------
#[cfg(target_os = "macos")]
fn scan_asdf_macos() -> Vec<PathBuf> {
    let home = match env::var("HOME") {
        Ok(h) => h,
        Err(_) => return Vec::new(),
    };
    let asdf_java = Path::new(&home).join(".asdf").join("installs").join("java");
    if !asdf_java.exists() {
        return Vec::new();
    }

    debug!("scan_asdf_macos: checking {}", asdf_java.display());
    let mut results = Vec::new();

    if let Ok(entries) = fs::read_dir(&asdf_java) {
        for entry in entries.flatten() {
            let java_exe = entry.path().join("bin").join("java");
            if java_exe.exists() {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Ok(metadata) = java_exe.metadata() {
                        if metadata.permissions().mode() & 0o111 != 0 {
                            debug!("scan_asdf_macos: found Java at {}", java_exe.display());
                            results.push(java_exe);
                        }
                    }
                }
                #[cfg(not(unix))]
                {
                    results.push(java_exe);
                }
            }
        }
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dedup_nested_no_duplicates() {
        let javas = vec![
            JavaInfo {
                java_home: PathBuf::from("/usr/lib/jvm/java-11"),
                ..Default::default()
            },
            JavaInfo {
                java_home: PathBuf::from("/usr/lib/jvm/java-17"),
                ..Default::default()
            },
        ];
        let result = dedup_nested(javas);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_dedup_nested_removes_jre() {
        let javas = vec![
            JavaInfo {
                java_home: PathBuf::from("/usr/lib/jvm/java-11"),
                ..Default::default()
            },
            JavaInfo {
                java_home: PathBuf::from("/usr/lib/jvm/java-11/jre"),
                ..Default::default()
            },
        ];
        let result = dedup_nested(javas);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].java_home, PathBuf::from("/usr/lib/jvm/java-11"));
    }

    #[test]
    fn test_dedup_nested_same_path() {
        let javas = vec![
            JavaInfo {
                java_home: PathBuf::from("/usr/lib/jvm/java-11"),
                ..Default::default()
            },
            JavaInfo {
                java_home: PathBuf::from("/usr/lib/jvm/java-11"),
                ..Default::default()
            },
        ];
        let result = dedup_nested(javas);
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_dedup_nested_standalone_jre() {
        let javas = vec![
            JavaInfo {
                java_home: PathBuf::from("/usr/lib/jvm/java-11"),
                ..Default::default()
            },
            JavaInfo {
                java_home: PathBuf::from("/usr/lib/jvm/jre-8"),
                ..Default::default()
            },
        ];
        let result = dedup_nested(javas);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_is_version_like_valid() {
        assert!(is_version_like("11.0.2"));
        assert!(is_version_like("1.8.0_202"));
        assert!(is_version_like("17_3"));
    }

    #[test]
    fn test_is_version_like_invalid() {
        assert!(!is_version_like(""));
        assert!(!is_version_like(&"a".repeat(21)));
        assert!(!is_version_like("no_digits"));
    }

    #[test]
    fn test_is_version_like_more() {
        assert!(is_version_like("8"));
        assert!(is_version_like("11.0"));
        assert!(is_version_like("1.8"));
        assert!(is_version_like("20.0.0.0"));
        assert!(is_version_like(".1"));
        assert!(!is_version_like(".."));
        assert!(!is_version_like("a-b-c"));
    }

    #[test]
    fn test_dedup_nested_deeply_nested() {
        let javas = vec![
            JavaInfo {
                java_home: PathBuf::from("/usr/lib/jvm/java-11"),
                ..Default::default()
            },
            JavaInfo {
                java_home: PathBuf::from("/usr/lib/jvm/java-11/jre"),
                ..Default::default()
            },
            JavaInfo {
                java_home: PathBuf::from("/usr/lib/jvm/java-11/jre/lib"),
                ..Default::default()
            },
        ];
        let result = dedup_nested(javas);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].java_home, PathBuf::from("/usr/lib/jvm/java-11"));
    }

    #[test]
    fn test_filter_by_version() {
        let javas = vec![
            JavaInfo {
                version: "11.0.2".into(),
                parsed_version: crate::JavaVersion::parse("11.0.2"),
                ..Default::default()
            },
            JavaInfo {
                version: "17.0.1".into(),
                parsed_version: crate::JavaVersion::parse("17.0.1"),
                ..Default::default()
            },
        ];
        let filtered = crate::filter_by_version(javas, "17");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].version, "17.0.1");
    }

    #[test]
    fn test_best_match() {
        let javas = vec![
            JavaInfo {
                version: "11.0.2".into(),
                parsed_version: crate::JavaVersion::parse("11.0.2"),
                ..Default::default()
            },
            JavaInfo {
                version: "17.0.1".into(),
                parsed_version: crate::JavaVersion::parse("17.0.1"),
                ..Default::default()
            },
            JavaInfo {
                version: "8.0_202".into(),
                parsed_version: crate::JavaVersion::parse("1.8.0_202"),
                ..Default::default()
            },
        ];
        let best = crate::best_match(javas, "8");
        assert!(best.is_some());
        assert_eq!(best.unwrap().version, "8.0_202");
    }

    #[test]
    fn test_filter_by_version_no_match() {
        let javas = vec![JavaInfo {
            version: "11.0.2".into(),
            parsed_version: crate::JavaVersion::parse("11.0.2"),
            ..Default::default()
        }];
        let filtered = crate::filter_by_version(javas, "17");
        assert!(filtered.is_empty());
    }

    #[test]
    fn test_best_match_no_match() {
        let javas = vec![JavaInfo {
            version: "11.0.2".into(),
            parsed_version: crate::JavaVersion::parse("11.0.2"),
            ..Default::default()
        }];
        let best = crate::best_match(javas, "17");
        assert!(best.is_none());
    }
}

// -----------------------------------------------------------------------------
// BFS scan of default paths (Windows)
// -----------------------------------------------------------------------------
#[cfg(target_os = "windows")]
fn scan_default_paths() -> Vec<PathBuf> {
    let mut results = Vec::new();
    let roots = default_path_roots();

    for root in roots {
        if !root.exists() {
            debug!(
                "scan_default_paths: root does not exist: {}",
                root.display()
            );
            continue;
        }

        debug!("scan_default_paths: starting BFS from {}", root.display());
        let mut queue = VecDeque::new();
        queue.push_back((root, 0));

        while let Some((current, depth)) = queue.pop_front() {
            if depth > MAX_SEARCH_DEPTH {
                continue;
            }

            if let Ok(entries) = fs::read_dir(&current) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if !path.is_dir() {
                        continue;
                    }

                    let java_exe = path.join("java.exe");
                    if java_exe.exists() {
                        debug!("scan_default_paths: found Java at {}", path.display());
                        results.push(java_exe);
                        continue;
                    }

                    if should_explore_deeper(&path) {
                        debug!(
                            "scan_default_paths: exploring {} (depth {})",
                            path.display(),
                            depth + 1
                        );
                        queue.push_back((path, depth + 1));
                    }
                }
            }
        }
    }

    results
}

#[cfg(target_os = "windows")]
fn default_path_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();

    if let Some(appdata) = env::var_os("APPDATA") {
        roots.push(Path::new(&appdata).join(".minecraft").join("runtime"));
    }

    if let Some(localappdata) = env::var_os("LOCALAPPDATA") {
        roots.push(Path::new(&localappdata).to_path_buf());
    }

    if let Some(profile) = env::var_os("USERPROFILE") {
        roots.push(Path::new(&profile).to_path_buf());
    }

    for drive in fixed_drives() {
        let prog_files = Path::new(&drive).join("Program Files");
        if prog_files.exists() {
            roots.push(prog_files);
        }

        let prog_files_x86 = Path::new(&drive).join("Program Files (x86)");
        if prog_files_x86.exists() {
            roots.push(prog_files_x86);
        }

        if let Ok(entries) = fs::read_dir(&drive) {
            for entry in entries.flatten() {
                let dir_name = entry.file_name().to_string_lossy().to_lowercase();
                if JAVA_KEYWORDS.iter().any(|kw| dir_name.contains(kw)) {
                    roots.push(entry.path());
                }
            }
        }
    }

    roots
}

#[cfg(target_os = "windows")]
fn fixed_drives() -> Vec<String> {
    let mut drives = Vec::new();
    for letter in 'A'..='Z' {
        let drive = format!("{letter}:\\");
        let p = Path::new(&drive);
        if p.exists() && fs::read_dir(p).is_ok() {
            drives.push(drive);
        }
    }
    drives
}

#[cfg(target_os = "windows")]
fn should_explore_deeper(path: &Path) -> bool {
    let name = match path.file_name() {
        Some(n) => n.to_string_lossy(),
        None => return false,
    };

    let lower = name.to_lowercase();

    for ex in EXCLUDE_FOLDERS {
        if lower.contains(ex) {
            return false;
        }
    }

    for kw in JAVA_KEYWORDS {
        if lower.contains(kw) {
            return true;
        }
    }

    is_version_like(&name)
}

fn is_version_like(name: &str) -> bool {
    if name.is_empty() || name.len() > 20 {
        return false;
    }

    let has_digit = name.chars().any(|c| c.is_ascii_digit());
    if !has_digit {
        return false;
    }

    name.chars()
        .all(|c| c.is_ascii_digit() || c == '.' || c == '_' || c == '-')
}

#[cfg(target_os = "windows")]
fn scan_microsoft_store() -> Vec<PathBuf> {
    let localappdata = match env::var_os("LOCALAPPDATA") {
        Some(v) => v,
        None => return Vec::new(),
    };

    let base = Path::new(&localappdata)
        .join(r"Packages\Microsoft.4297127D64EC6_8wekyb3d8bbwe\LocalCache\Local\runtime");

    if !base.exists() {
        debug!("scan_microsoft_store: base path does not exist");
        return Vec::new();
    }

    debug!("scan_microsoft_store: scanning {}", base.display());
    let mut results = Vec::new();

    if let Ok(runtimes) = fs::read_dir(&base) {
        for runtime in runtimes.flatten() {
            let runtime_path = runtime.path();
            let name = runtime_path.file_name().map(|n| n.to_string_lossy());
            if !name
                .as_deref()
                .is_some_and(|n| n.starts_with("java-runtime"))
            {
                continue;
            }

            if let Ok(archs) = fs::read_dir(&runtime_path) {
                for arch in archs.flatten() {
                    let arch_path = arch.path();
                    if !arch_path.is_dir() {
                        continue;
                    }

                    if let Ok(versions) = fs::read_dir(&arch_path) {
                        for version in versions.flatten() {
                            let java_exe = version.path().join("bin").join("java.exe");
                            if java_exe.exists() {
                                results.push(java_exe);
                            }
                        }
                    }
                }
            }
        }
    }

    results
}

#[cfg(target_os = "windows")]
fn scan_where_command() -> Vec<PathBuf> {
    let mut results = Vec::new();

    debug!("scan_where_command: running `where java`");
    if let Ok(output) = std::process::Command::new("where").arg("java").output()
        && output.status.success()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                let p = Path::new(trimmed);
                if p.exists() {
                    results.push(p.to_path_buf());
                }
            }
        }
    }

    results
}

#[cfg(target_os = "linux")]
fn scan_linux() -> Vec<PathBuf> {
    use walkdir::WalkDir;

    let mut results = Vec::new();

    let mut search_dirs: Vec<PathBuf> = vec![
        "/usr/lib/jvm".into(),
        "/usr/java".into(),
        "/opt".into(),
        "/usr/local".into(),
    ];

    if let Ok(home) = env::var("HOME") {
        let mc_runtime = Path::new(&home).join(".minecraft").join("runtime");
        if mc_runtime.exists() {
            search_dirs.push(mc_runtime);
        }
    }

    for dir in search_dirs {
        if !dir.exists() {
            debug!("scan_linux: directory does not exist: {}", dir.display());
            continue;
        }

        debug!("scan_linux: walking {}", dir.display());
        for entry in WalkDir::new(&dir)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let entry_path = entry.path();

            if entry_path.file_name() != Some(std::ffi::OsStr::new("java")) {
                continue;
            }
            if !entry_path.is_file() {
                continue;
            }

            if let Some(parent) = entry_path.parent() {
                if !should_explore_linux(parent) {
                    continue;
                }
            }

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(metadata) = entry_path.metadata() {
                    let permissions = metadata.permissions();
                    if permissions.mode() & 0o111 != 0 {
                        debug!("scan_linux: found Java at {}", entry_path.display());
                        results.push(entry_path.to_path_buf());
                    }
                }
            }
            #[cfg(not(unix))]
            {
                results.push(entry_path.to_path_buf());
            }
        }
    }

    results
}

#[cfg(target_os = "linux")]
fn should_explore_linux(path: &Path) -> bool {
    let name = match path.file_name() {
        Some(n) => n.to_string_lossy(),
        None => return true,
    };

    let lower = name.to_lowercase();

    for ex in EXCLUDE_FOLDERS {
        if lower.contains(ex) {
            return false;
        }
    }

    for kw in JAVA_KEYWORDS {
        if lower.contains(kw) {
            return true;
        }
    }

    is_version_like(&name)
}

#[cfg(target_os = "macos")]
fn scan_macos() -> Vec<PathBuf> {
    use walkdir::WalkDir;

    let mut results = Vec::new();

    let mut search_dirs: Vec<PathBuf> = vec!["/Library/Java/JavaVirtualMachines".into()];

    if let Ok(home) = env::var("HOME") {
        let user_jvm = Path::new(&home).join("Library/Java/JavaVirtualMachines");
        if user_jvm.exists() {
            search_dirs.push(user_jvm);
        }

        let mc_runtime = Path::new(&home).join(".minecraft").join("runtime");
        if mc_runtime.exists() {
            search_dirs.push(mc_runtime);
        }
    }

    // Also try /usr/libexec/java_home for the system default
    debug!("scan_macos: running /usr/libexec/java_home");
    if let Ok(output) = std::process::Command::new("/usr/libexec/java_home").output() {
        if output.status.success() {
            let path_str = String::from_utf8_lossy(&output.stdout);
            let jh_path = Path::new(path_str.trim());
            if jh_path.exists() {
                let java_exe = jh_path.join("bin").join("java");
                if java_exe.exists() {
                    debug!(
                        "scan_macos: /usr/libexec/java_home -> {}",
                        java_exe.display()
                    );
                    results.push(java_exe);
                }
            }
        }
    }

    for dir in search_dirs {
        if !dir.exists() {
            debug!("scan_macos: directory does not exist: {}", dir.display());
            continue;
        }

        debug!("scan_macos: walking {}", dir.display());
        for entry in WalkDir::new(&dir)
            .max_depth(5)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let entry_path = entry.path();

            if entry_path.file_name() != Some(std::ffi::OsStr::new("java")) {
                continue;
            }
            if !entry_path.is_file() {
                continue;
            }

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(metadata) = entry_path.metadata() {
                    let permissions = metadata.permissions();
                    if permissions.mode() & 0o111 != 0 {
                        debug!("scan_macos: found Java at {}", entry_path.display());
                        results.push(entry_path.to_path_buf());
                    }
                }
            }
            #[cfg(not(unix))]
            {
                results.push(entry_path.to_path_buf());
            }
        }
    }

    results
}
