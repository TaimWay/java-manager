use crate::JavaError;
use is_executable::is_executable;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str;

const UNKNOWN: &str = "UNKNOWN";

#[derive(Debug)]
pub struct JavaInfo {
    pub name: String,
    pub version: String,
    pub path: PathBuf,
    pub vendor: String,
    pub architecture: String,
    pub java_home: PathBuf,
}

impl JavaInfo {
    pub fn new(path: String) -> Result<Self, JavaError> {
        let path_obj = Path::new(&path);
        if !path_obj.exists() {
            return Err(JavaError::InvalidJavaPath(format!(
                "Path does not exist: {}",
                path
            )));
        }

        // Resolve symlinks to get the real absolute path
        let canonical_path = fs::canonicalize(path_obj)
            .map_err(|e| JavaError::IoError(e))?;

        let (java_home, exec_path) = if canonical_path.is_file() && is_executable(&canonical_path) {
            // It's an executable – locate JAVA_HOME by walking up the tree
            let home = find_java_home_from_exe(&canonical_path)
                .ok_or_else(|| JavaError::InvalidJavaPath(format!(
                    "Unable to determine JAVA_HOME from executable: {}",
                    canonical_path.display()
                )))?;
            (home, Some(canonical_path))
        } else {
            // Assume it's a directory (JAVA_HOME itself)
            (canonical_path, None)
        };

        // Path to the java executable inside JAVA_HOME
        let java_exe = java_home.join("bin").join(if cfg!(windows) { "java.exe" } else { "java" });
        // Store either the original executable path or the default one from bin
        let stored_path = exec_path.unwrap_or_else(|| java_exe.clone());

        let mut info = JavaInfo {
            name: UNKNOWN.to_string(),
            version: UNKNOWN.to_string(),
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

        // If all fields are known, we are done
        if info.is_complete() {
            return Ok(info);
        }

        // --- Step 2: fill missing fields from `java -version` ---
        let version_info = read_version(&java_exe)?;

        if info.name == UNKNOWN {
            if let Some(name) = version_info.name {
                info.name = name;
            }
        }
        if info.version == UNKNOWN {
            if let Some(ver) = version_info.version {
                info.version = ver;
            }
        }
        if info.vendor == UNKNOWN {
            if let Some(vend) = version_info.vendor {
                info.vendor = vend;
            }
        }
        if info.architecture == UNKNOWN {
            if let Some(arch) = version_info.arch {
                info.architecture = arch;
            }
        }

        Ok(info)
    }

    /// Check if all fields (except path/java_home) have been determined
    fn is_complete(&self) -> bool {
        self.name != UNKNOWN
            && self.version != UNKNOWN
            && self.vendor != UNKNOWN
            && self.architecture != UNKNOWN
    }

    pub fn default() -> Self {
        Self {
            name: UNKNOWN.to_string(),
            version: UNKNOWN.to_string(),
            path: PathBuf::new(),
            vendor: UNKNOWN.to_string(),
            architecture: UNKNOWN.to_string(),
            java_home: PathBuf::new(),
        }
    }
}

// -----------------------------------------------------------------------------
// Helper: locate JAVA_HOME from a java executable path
// -----------------------------------------------------------------------------
fn find_java_home_from_exe(exec_path: &Path) -> Option<PathBuf> {
    let mut current = exec_path.parent()?;
    loop {
        let bin_java = current.join("bin").join(if cfg!(windows) { "java.exe" } else { "java" });
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

    let stderr = str::from_utf8(&output.stderr)
        .map_err(|e| JavaError::RuntimeError(format!("Failed to decode java -version output: {}", e)))?;

    let mut version = None;
    let mut vendor = None;
    let mut arch = None;

    for line in stderr.lines() {
        // Extract version from lines like `openjdk version "11.0.2" 2019-01-15`
        if line.contains(" version ") {
            if let Some(start) = line.find('"') {
                if let Some(end) = line[start + 1..].find('"') {
                    version = Some(line[start + 1..start + 1 + end].to_string());
                }
            }
        }

        // Extract vendor from "Runtime Environment" line
        if line.contains("Runtime Environment") {
            if let Some(idx) = line.find("Runtime Environment") {
                let rest = &line[idx + "Runtime Environment".len()..];
                let vendor_part = rest.split_whitespace().next().unwrap_or("");
                let vendor_cleaned = vendor_part
                    .split(|c| c == '-' || c == '(')
                    .next()
                    .unwrap_or("")
                    .to_string();
                if !vendor_cleaned.is_empty() {
                    vendor = Some(vendor_cleaned);
                }
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
    if vendor.is_none() {
        if let Some(first_line) = stderr.lines().next() {
            if first_line.starts_with("openjdk") {
                vendor = Some("OpenJDK".to_string());
            } else if first_line.starts_with("java") {
                vendor = Some("Oracle".to_string());
            }
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