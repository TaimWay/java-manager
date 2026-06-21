//! Core type representing a Java installation and its metadata.

use crate::JavaError;
use is_executable::is_executable;
use log::debug;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str;

const UNKNOWN: &str = "UNKNOWN";

/// Parsed Java version with relaxed extraction of major/minor/patch.
///
/// Handles formats like `11.0.2`, `1.8.0_202-ea`, `17.0.2+13-LTS`,
/// `11-ea`, and `17.0.8.1` (extra segments are ignored).
///
/// The [`Display`](std::fmt::Display) implementation renders as `{major}.{minor}.{patch}`,
/// except when all three are zero — in that case the [`raw`](JavaVersion::raw) string
/// is shown as a fallback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JavaVersion {
    /// Major version number (e.g. `11` in `11.0.2`).
    ///
    /// For legacy Java 8 this is the second component — `1.8.0_202` becomes
    /// `major = 8`.
    pub major: u64,

    /// Minor version number (e.g. `0` in `11.0.2`).
    ///
    /// Defaults to `0` when only the major component is specified.
    pub minor: u64,

    /// Patch / security-update version number (e.g. `2` in `11.0.2`).
    ///
    /// Defaults to `0` when only major and minor are specified.
    pub patch: u64,

    /// The raw version string as produced by `java -version`.
    ///
    /// Preserved for display when automatic parsing falls back to `0.0.0`.
    pub raw: String,
}

impl JavaVersion {
    /// Relaxed parse: strips `1.` prefix, splits on non-digits,
    /// takes first three numeric parts as major/minor/patch.
    pub fn parse(raw: &str) -> Option<Self> {
        debug!("JavaVersion::parse: raw input \"{raw}\"");
        let body = if raw.len() > 2
            && raw.starts_with("1.")
            && raw.as_bytes().get(2).is_some_and(|b| b.is_ascii_digit())
        {
            &raw[2..]
        } else {
            raw
        };

        let parts: Vec<&str> = body
            .split(|c: char| !c.is_ascii_digit())
            .filter(|s| !s.is_empty())
            .collect();

        let major = parts.first()?.parse().ok()?;
        let minor = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
        let patch = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);

        let result = JavaVersion {
            major,
            minor,
            patch,
            raw: raw.to_string(),
        };
        debug!("JavaVersion::parse: \"{raw}\" -> {result}");
        Some(result)
    }
}

impl fmt::Display for JavaVersion {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.major == 0 && self.minor == 0 && self.patch == 0 && !self.raw.is_empty() {
            return write!(f, "{}", self.raw);
        }
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl PartialOrd for JavaVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for JavaVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.major, self.minor, self.patch).cmp(&(other.major, other.minor, other.patch))
    }
}

/// Represents a discovered Java installation.
///
/// This struct holds metadata about a Java runtime, such as its version,
/// vendor, architecture, and the location of its `java` executable and
/// `JAVA_HOME` directory.
#[derive(Debug)]
pub struct JavaInfo {
    /// Human-readable name of the Java implementation (e.g., "OpenJDK").
    pub name: String,
    /// Raw version string (e.g., "11.0.2", "1.8.0_202").
    pub version: String,
    /// Parsed structured version (major/minor/patch). `None` if parsing failed.
    pub parsed_version: Option<JavaVersion>,
    /// Full path to the `java` executable (or the path originally provided).
    pub path: PathBuf,
    /// Vendor name (e.g., "Oracle", "OpenJDK").
    pub vendor: String,
    /// Architecture (e.g., "64-Bit", "32-Bit").
    pub architecture: String,
    /// The `JAVA_HOME` directory corresponding to this installation.
    pub java_home: PathBuf,
}

impl JavaInfo {
    /// Creates a new `JavaInfo` from a path pointing either to a `java` executable
    /// or directly to a `JAVA_HOME` directory.
    ///
    /// The path is canonicalized, and if it is an executable, the `JAVA_HOME` is
    /// located by walking up the directory tree until a `bin/java` (or `java.exe`)
    /// is found. Metadata is then extracted from the `release` file inside
    /// `JAVA_HOME`, and any missing fields are filled by running `java -version`.
    ///
    /// # Errors
    ///
    /// Returns `JavaError::InvalidJavaPath` if the path does not exist,
    /// or if `JAVA_HOME` cannot be determined from an executable.
    /// Returns other `JavaError` variants if I/O or command execution fails.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use java_manager::JavaInfo;
    ///
    /// let info = JavaInfo::new("/usr/lib/jvm/java-11-openjdk/bin/java".into())?;
    /// println!("Java version: {}", info.version);
    /// # Ok::<_, java_manager::JavaError>(())
    /// ```
    pub fn new(path: String) -> Result<Self, JavaError> {
        debug!("JavaInfo::new: resolving path \"{path}\"");
        let path_obj = Path::new(&path);
        if !path_obj.exists() {
            debug!("JavaInfo::new: path does not exist: {path}");
            return Err(JavaError::InvalidJavaPath(format!(
                "Path does not exist: {}",
                path
            )));
        }

        // Resolve symlinks to get the real absolute path
        let canonical_path = fs::canonicalize(path_obj).map_err(JavaError::IoError)?;
        debug!(
            "JavaInfo::new: canonical path: {}",
            canonical_path.display()
        );

        let (java_home, exec_path) = if canonical_path.is_file() && is_executable(&canonical_path) {
            // It's an executable – locate JAVA_HOME by walking up the tree
            let home = find_java_home_from_exe(&canonical_path).ok_or_else(|| {
                JavaError::InvalidJavaPath(format!(
                    "Unable to determine JAVA_HOME from executable: {}",
                    canonical_path.display()
                ))
            })?;
            (home, Some(canonical_path))
        } else {
            // Assume it's a directory (JAVA_HOME itself)
            (canonical_path, None)
        };

        // Path to the java executable inside JAVA_HOME
        let java_exe = java_home
            .join("bin")
            .join(if cfg!(windows) { "java.exe" } else { "java" });
        // Store either the original executable path or the default one from bin
        let stored_path = exec_path.unwrap_or_else(|| java_exe.clone());

        let mut info = JavaInfo {
            name: UNKNOWN.to_string(),
            version: UNKNOWN.to_string(),
            parsed_version: None,
            path: stored_path,
            vendor: UNKNOWN.to_string(),
            architecture: UNKNOWN.to_string(),
            java_home,
        };

        // --- Step 1: read from release file (if possible) ---
        if let Some(release) = read_release(&info.java_home) {
            if let Some(name) = release.name {
                info.name = name;
            }
            if let Some(version) = release.version {
                info.version = version;
            }
            if let Some(vendor) = release.vendor {
                info.vendor = vendor;
            }
            if let Some(arch) = release.arch {
                info.architecture = arch;
            }
        }

        if info.version != UNKNOWN {
            info.parsed_version = JavaVersion::parse(&info.version);
        }

        // If all fields are known, we are done
        if info.is_complete() {
            return Ok(info);
        }

        // --- Step 2: fill missing fields from `java -version` ---
        let version_info = read_version(&java_exe)?;

        if info.name == UNKNOWN
            && let Some(name) = version_info.name
        {
            info.name = name;
        }
        if info.version == UNKNOWN
            && let Some(ver) = version_info.version
        {
            info.version = ver;
        }
        if info.vendor == UNKNOWN
            && let Some(vend) = version_info.vendor
        {
            info.vendor = vend;
        }
        if info.architecture == UNKNOWN
            && let Some(arch) = version_info.arch
        {
            info.architecture = arch;
        }

        if info.version != UNKNOWN {
            info.parsed_version = JavaVersion::parse(&info.version);
        }

        Ok(info)
    }
}

impl Default for JavaInfo {
    fn default() -> Self {
        Self {
            name: UNKNOWN.to_string(),
            version: UNKNOWN.to_string(),
            parsed_version: None,
            path: PathBuf::new(),
            vendor: UNKNOWN.to_string(),
            architecture: UNKNOWN.to_string(),
            java_home: PathBuf::new(),
        }
    }
}

impl JavaInfo {
    /// Check whether this installation matches a version requirement.
    ///
    /// `req` supports:
    /// - `"17"` — major version match (any 17.x.x)
    /// - `"17.0"` — major.minor match (any 17.0.x)
    /// - `"17.0.2"` — exact major.minor.patch match
    ///
    /// Returns `false` if the version could not be parsed.
    ///
    /// # Examples
    ///
    /// ```
    /// # use java_manager::JavaInfo;
    /// let info = JavaInfo {
    ///     version: "11.0.2".into(),
    ///     parsed_version: java_manager::JavaVersion::parse("11.0.2"),
    ///     ..Default::default()
    /// };
    ///
    /// assert!( info.matches_version("11"));
    /// assert!( info.matches_version("11.0"));
    /// assert!( info.matches_version("11.0.2"));
    /// assert!(!info.matches_version("17"));
    /// ```
    pub fn matches_version(&self, req: &str) -> bool {
        let parsed = match &self.parsed_version {
            Some(v) => v,
            None => return false,
        };

        let req_parts: Vec<&str> = req.split('.').collect();
        match req_parts.len() {
            1 => req_parts[0].parse::<u64>().ok() == Some(parsed.major),
            2 => {
                let major = match req_parts[0].parse::<u64>() {
                    Ok(m) => m,
                    Err(_) => return false,
                };
                let minor = match req_parts[1].parse::<u64>() {
                    Ok(m) => m,
                    Err(_) => return false,
                };
                parsed.major == major && parsed.minor == minor
            }
            3 => {
                let major = match req_parts[0].parse::<u64>() {
                    Ok(m) => m,
                    Err(_) => return false,
                };
                let minor = match req_parts[1].parse::<u64>() {
                    Ok(m) => m,
                    Err(_) => return false,
                };
                let patch = match req_parts[2].parse::<u64>() {
                    Ok(m) => m,
                    Err(_) => return false,
                };
                parsed.major == major && parsed.minor == minor && parsed.patch == patch
            }
            _ => false,
        }
    }

    fn is_complete(&self) -> bool {
        self.name != UNKNOWN
            && self.version != UNKNOWN
            && self.vendor != UNKNOWN
            && self.architecture != UNKNOWN
    }

    /// Returns `true` if this installation is a full JDK (has `javac`).
    ///
    /// Checks for the presence of `javac` (or `javac.exe` on Windows) in the
    /// `JAVA_HOME/bin` directory. A `false` return typically means a JRE.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use java_manager::java_home;
    ///
    /// if let Some(java) = java_home() {
    ///     if java.is_jdk() {
    ///         println!("Full JDK detected");
    ///     } else {
    ///         println!("JRE only");
    ///     }
    /// }
    /// ```
    pub fn is_jdk(&self) -> bool {
        let javac =
            self.java_home
                .join("bin")
                .join(if cfg!(windows) { "javac.exe" } else { "javac" });
        javac.exists()
    }

    /// Returns a list of JDK tools available in this installation.
    ///
    /// Checks for common tools in `JAVA_HOME/bin`: `javac`, `javadoc`,
    /// `javap`, `jar`, `jlink`, `jmod`, `jpackage`, `jshell`, `jconsole`,
    /// `jcmd`, `jmap`, `jstack`, `jstat`, `jhsdb`, `jfr`, `jwebserver`.
    ///
    /// Returns an empty `Vec` if the directory cannot be read.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use java_manager::java_home;
    ///
    /// if let Some(java) = java_home() {
    ///     let tools = java.capabilities();
    ///     println!("Available tools: {:?}", tools);
    /// }
    /// ```
    pub fn capabilities(&self) -> Vec<String> {
        let bin_dir = self.java_home.join("bin");
        let tools = [
            "javac",
            "javadoc",
            "javap",
            "jar",
            "jlink",
            "jmod",
            "jpackage",
            "jshell",
            "jconsole",
            "jcmd",
            "jmap",
            "jstack",
            "jstat",
            "jhsdb",
            "jfr",
            "jwebserver",
        ];

        let mut available = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&bin_dir) {
            let names: Vec<String> = entries
                .filter_map(|e| e.ok())
                .filter_map(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    if cfg!(windows) {
                        // On Windows, strip .exe extension
                        name.strip_suffix(".exe").map(|s| s.to_string())
                    } else {
                        Some(name)
                    }
                })
                .collect();

            for tool in tools {
                let tool_name = tool.to_string();
                if names.contains(&tool_name) || names.contains(&format!("{}.exe", tool_name)) {
                    available.push(tool.to_string());
                }
            }
        }
        available
    }
}

// -----------------------------------------------------------------------------
// Helper: locate JAVA_HOME from a java executable path
// -----------------------------------------------------------------------------
fn find_java_home_from_exe(exec_path: &Path) -> Option<PathBuf> {
    let mut current = exec_path.parent()?;
    loop {
        let bin_java = current
            .join("bin")
            .join(if cfg!(windows) { "java.exe" } else { "java" });
        if bin_java.exists() && is_executable(&bin_java) {
            return Some(current.to_path_buf());
        }
        current = current.parent()?;
    }
}

// -----------------------------------------------------------------------------
// Data extracted from the release file
// -----------------------------------------------------------------------------
struct ReleaseInfo {
    name: Option<String>,
    version: Option<String>,
    vendor: Option<String>,
    arch: Option<String>,
}

fn read_release(java_home: &Path) -> Option<ReleaseInfo> {
    let release_path = java_home.join("release");
    debug!("read_release: checking {}", release_path.display());
    let file = File::open(release_path).ok()?;
    let reader = BufReader::new(file);
    let mut properties = HashMap::new();

    for line in reader.lines() {
        let line = line.ok()?;
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.splitn(2, '=');
        if let (Some(key), Some(value)) = (parts.next(), parts.next()) {
            let key = key.trim().to_string();
            let value = value.trim().trim_matches('"').to_string();
            properties.insert(key, value);
        }
    }

    Some(ReleaseInfo {
        name: properties.get("IMPLEMENTOR").cloned(),
        version: properties.get("JAVA_VERSION").cloned(),
        vendor: properties.get("IMPLEMENTOR").cloned(),
        arch: properties.get("OS_ARCH").cloned(),
    })
}

// -----------------------------------------------------------------------------
// Data extracted from `java -version` output
// -----------------------------------------------------------------------------
struct VersionInfo {
    name: Option<String>,
    version: Option<String>,
    vendor: Option<String>,
    arch: Option<String>,
}

fn read_version(java_exe: &Path) -> Result<VersionInfo, JavaError> {
    debug!("read_version: running {}", java_exe.display());
    let output = Command::new(java_exe)
        .arg("-version")
        .output()
        .map_err(|e| JavaError::ExecuteError(format!("Failed to execute java -version: {}", e)))?;

    if !output.status.success() {
        return Err(JavaError::ExecuteError(format!(
            "java -version command failed with status: {}",
            output.status
        )));
    }

    let stderr = str::from_utf8(&output.stderr).map_err(|e| {
        JavaError::RuntimeError(format!("Failed to decode java -version output: {}", e))
    })?;

    let mut version = None;
    let mut vendor = None;
    let mut arch = None;

    for line in stderr.lines() {
        // Extract version from lines like `openjdk version "11.0.2" 2019-01-15`
        if line.contains(" version ")
            && let Some(start) = line.find('"')
            && let Some(end) = line[start + 1..].find('"')
        {
            version = Some(line[start + 1..start + 1 + end].to_string());
        }

        // Extract vendor from "Runtime Environment" line
        if line.contains("Runtime Environment")
            && let Some(idx) = line.find("Runtime Environment")
        {
            let rest = &line[idx + "Runtime Environment".len()..];
            let vendor_part = rest.split_whitespace().next().unwrap_or("");
            let vendor_cleaned = vendor_part
                .split(['-', '('])
                .next()
                .unwrap_or("")
                .to_string();
            if !vendor_cleaned.is_empty() {
                vendor = Some(vendor_cleaned);
            }
        }

        // Extract architecture (64‑Bit / 32‑Bit)
        if line.contains("VM") && line.contains("Bit") {
            if line.contains("64-Bit") {
                arch = Some("64-Bit".to_string());
            } else if line.contains("32-Bit") {
                arch = Some("32-Bit".to_string());
            }
        }
    }

    // Fallback vendor from first line
    if vendor.is_none()
        && let Some(first_line) = stderr.lines().next()
    {
        if first_line.starts_with("openjdk") {
            vendor = Some("OpenJDK".to_string());
        } else if first_line.starts_with("java") {
            vendor = Some("Oracle".to_string());
        }
    }

    // Name is derived from vendor (keeps original behaviour)
    let name = vendor.clone();

    Ok(VersionInfo {
        name,
        version,
        vendor,
        arch,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_java_8() {
        let v = JavaVersion::parse("1.8.0_202-ea").unwrap();
        assert_eq!(v.major, 8);
        assert_eq!(v.minor, 0);
        assert_eq!(v.patch, 202);
        assert_eq!(v.raw, "1.8.0_202-ea");
    }

    #[test]
    fn test_parse_java_11() {
        let v = JavaVersion::parse("11.0.2").unwrap();
        assert_eq!(v.major, 11);
        assert_eq!(v.minor, 0);
        assert_eq!(v.patch, 2);
    }

    #[test]
    fn test_parse_java_17() {
        let v = JavaVersion::parse("17.0.2+13-LTS").unwrap();
        assert_eq!(v.major, 17);
        assert_eq!(v.minor, 0);
        assert_eq!(v.patch, 2);
    }

    #[test]
    fn test_parse_java_21() {
        let v = JavaVersion::parse("21.0.1+12").unwrap();
        assert_eq!(v.major, 21);
        assert_eq!(v.minor, 0);
        assert_eq!(v.patch, 1);
    }

    #[test]
    fn test_parse_ea_no_patch() {
        let v = JavaVersion::parse("11-ea").unwrap();
        assert_eq!(v.major, 11);
        assert_eq!(v.minor, 0);
        assert_eq!(v.patch, 0);
    }

    #[test]
    fn test_parse_extra_segments() {
        let v = JavaVersion::parse("17.0.8.1").unwrap();
        assert_eq!(v.major, 17);
        assert_eq!(v.minor, 0);
        assert_eq!(v.patch, 8);
    }

    #[test]
    fn test_parse_no_input() {
        assert!(JavaVersion::parse("").is_none());
    }

    #[test]
    fn test_parse_garbage() {
        assert!(JavaVersion::parse("not.a.version").is_none());
    }

    #[test]
    fn test_parse_letters_only() {
        assert!(JavaVersion::parse("abc.def").is_none());
    }

    #[test]
    fn test_display_normal() {
        let v = JavaVersion::parse("11.0.2").unwrap();
        assert_eq!(v.to_string(), "11.0.2");
    }

    #[test]
    fn test_display_fallback_raw() {
        let v = JavaVersion {
            major: 0,
            minor: 0,
            patch: 0,
            raw: "1.8.0_202".into(),
        };
        assert_eq!(v.to_string(), "1.8.0_202");
    }

    #[test]
    fn test_matches_version_exact() {
        let mut info = JavaInfo::default();
        info.version = "11.0.2".into();
        info.parsed_version = JavaVersion::parse("11.0.2");
        assert!(info.matches_version("11.0.2"));
    }

    #[test]
    fn test_matches_version_major_only() {
        let mut info = JavaInfo::default();
        info.version = "11.0.2".into();
        info.parsed_version = JavaVersion::parse("11.0.2");
        assert!(info.matches_version("11"));
    }

    #[test]
    fn test_matches_version_major_minor() {
        let mut info = JavaInfo::default();
        info.version = "11.0.2".into();
        info.parsed_version = JavaVersion::parse("11.0.2");
        assert!(info.matches_version("11.0"));
    }

    #[test]
    fn test_matches_version_no_match() {
        let mut info = JavaInfo::default();
        info.version = "11.0.2".into();
        info.parsed_version = JavaVersion::parse("11.0.2");
        assert!(!info.matches_version("17"));
    }

    #[test]
    fn test_matches_version_unparseable() {
        let info = JavaInfo::default();
        assert!(!info.matches_version("11"));
    }

    #[test]
    fn test_matches_version_invalid_req() {
        let mut info = JavaInfo::default();
        info.version = "11.0.2".into();
        info.parsed_version = JavaVersion::parse("11.0.2");
        assert!(!info.matches_version("abc"));
    }

    #[test]
    fn test_matches_version_too_many_parts() {
        let mut info = JavaInfo::default();
        info.version = "11.0.2".into();
        info.parsed_version = JavaVersion::parse("11.0.2");
        assert!(!info.matches_version("11.0.2.1"));
    }

    #[test]
    fn test_java_version_ordering() {
        let v8 = JavaVersion::parse("1.8.0_202").unwrap();
        let v11 = JavaVersion::parse("11.0.2").unwrap();
        let v17 = JavaVersion::parse("17.0.1").unwrap();
        assert!(v8 < v11);
        assert!(v11 < v17);
        assert!(v17 > v11);
    }

    #[test]
    fn test_java_version_eq() {
        let a = JavaVersion::parse("11.0.2").unwrap();
        let b = JavaVersion::parse("11.0.2").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn test_java_version_ordering_same_major() {
        let a = JavaVersion::parse("11.0.2").unwrap();
        let b = JavaVersion::parse("11.0.3").unwrap();
        assert!(a < b);
        assert!(b > a);
    }

    #[test]
    fn test_java_info_default() {
        let info = JavaInfo::default();
        assert_eq!(info.name, "UNKNOWN");
        assert_eq!(info.version, "UNKNOWN");
        assert!(info.parsed_version.is_none());
        assert!(info.path.to_string_lossy().is_empty());
    }

    #[test]
    fn test_is_jdk_true() {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("bin");
        std::fs::create_dir(&bin).unwrap();
        let javac_path = bin.join(if cfg!(windows) { "javac.exe" } else { "javac" });
        std::fs::write(&javac_path, "").unwrap();

        let info = JavaInfo {
            java_home: dir.path().to_path_buf(),
            ..Default::default()
        };
        assert!(info.is_jdk());
    }

    #[test]
    fn test_is_jdk_false() {
        let dir = tempfile::tempdir().unwrap();
        let info = JavaInfo {
            java_home: dir.path().to_path_buf(),
            ..Default::default()
        };
        assert!(!info.is_jdk());
    }

    #[test]
    fn test_capabilities_returns_matching_tools() {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("bin");
        std::fs::create_dir(&bin).unwrap();

        let tool = if cfg!(windows) { "javac.exe" } else { "javac" };
        std::fs::write(bin.join(tool), "").unwrap();

        // Also create a tool that is NOT in the known list
        let other = if cfg!(windows) { "other.exe" } else { "other" };
        std::fs::write(bin.join(other), "").unwrap();

        let info = JavaInfo {
            java_home: dir.path().to_path_buf(),
            ..Default::default()
        };
        let caps = info.capabilities();
        assert!(caps.contains(&"javac".to_string()));
        assert!(!caps.contains(&"other".to_string()));
    }

    #[test]
    fn test_capabilities_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let info = JavaInfo {
            java_home: dir.path().to_path_buf(),
            ..Default::default()
        };
        assert!(info.capabilities().is_empty());
    }

    #[test]
    fn test_capabilities_nonexistent_dir() {
        let info = JavaInfo {
            java_home: PathBuf::from("/nonexistent/path"),
            ..Default::default()
        };
        assert!(info.capabilities().is_empty());
    }
}
