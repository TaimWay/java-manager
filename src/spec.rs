//! Human-readable Java specification parsing and matching.
//!
//! A spec string like `"Eclipse Adoptium v^17.0.0 ax86_64"` describes a
//! desired Java installation by vendor, version constraint, and architecture.
//!
//! Format: `<vendor> v<constraint> a<arch>`
//!
//! | Part | Example | Meaning |
//! |---|---|---|
//! | Vendor | `Eclipse Adoptium` | Fuzzy-matched against name / vendor |
//! | Constraint | `^17.0.0` | Compatible with 17.0.0 (semver caret) |
//! | Architecture | `x86_64` | Architecture substring match |
//!
//! Supported constraint operators:
//! - `^17.0.0` — compatible (≥17.0.0, <18.0.0)
//! - `~17.0.0` — approximately (≥17.0.0, <17.1.0)
//! - `>=17.0.0`, `<=17.0.0`, `>17.0.0`, `<17.0.0`
//! - `17.0.0` — exact (no prefix)

use std::fmt;

/// A parsed Java installation query spec.
#[derive(Debug, Clone)]
pub struct JavaSpec {
    /// Vendor substring to match (case-insensitive).
    pub vendor: Option<String>,
    /// Version requirement.
    pub version: Option<VersionReq>,
    /// Architecture substring to match (case-insensitive).
    pub arch: Option<String>,
    /// The raw input string.
    pub raw: String,
}

/// A version constraint with an operator and target version.
#[derive(Debug, Clone, PartialEq)]
pub enum VersionReq {
    /// `^major.minor.patch` — compatible: >= target and < next-major.
    Compatible { major: u64, minor: u64, patch: u64 },
    /// `~major.minor.patch` — approximately: >= target and < next-minor.
    Approx { major: u64, minor: u64, patch: u64 },
    /// `>=major.minor.patch`
    Gte { major: u64, minor: u64, patch: u64 },
    /// `<=major.minor.patch`
    Lte { major: u64, minor: u64, patch: u64 },
    /// `>major.minor.patch`
    Gt { major: u64, minor: u64, patch: u64 },
    /// `<major.minor.patch`
    Lt { major: u64, minor: u64, patch: u64 },
    /// `major.minor.patch` — exact match.
    Exact { major: u64, minor: u64, patch: u64 },
}

impl fmt::Display for VersionReq {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            VersionReq::Compatible {
                major,
                minor,
                patch,
            } => {
                write!(f, "^{}.{}.{}", major, minor, patch)
            }
            VersionReq::Approx {
                major,
                minor,
                patch,
            } => {
                write!(f, "~{}.{}.{}", major, minor, patch)
            }
            VersionReq::Gte {
                major,
                minor,
                patch,
            } => write!(f, ">={}.{}.{}", major, minor, patch),
            VersionReq::Lte {
                major,
                minor,
                patch,
            } => write!(f, "<={}.{}.{}", major, minor, patch),
            VersionReq::Gt {
                major,
                minor,
                patch,
            } => write!(f, ">{}.{}.{}", major, minor, patch),
            VersionReq::Lt {
                major,
                minor,
                patch,
            } => write!(f, "<{}.{}.{}", major, minor, patch),
            VersionReq::Exact {
                major,
                minor,
                patch,
            } => write!(f, "{}.{}.{}", major, minor, patch),
        }
    }
}

impl JavaSpec {
    /// Parse a human-readable Java spec string.
    ///
    /// # Format
    ///
    /// `<vendor> v<constraint> a<arch>`
    ///
    /// Both `v` and `a` prefixes and their values are optional.
    ///
    /// # Examples
    ///
    /// ```
    /// use java_manager::JavaSpec;
    ///
    /// let spec = JavaSpec::parse("Eclipse Adoptium v^17.0.0 ax86_64").unwrap();
    /// assert_eq!(spec.vendor.as_deref(), Some("Eclipse Adoptium"));
    /// assert!(spec.version.is_some());
    /// assert_eq!(spec.arch.as_deref(), Some("x86_64"));
    /// ```
    pub fn parse(input: &str) -> Result<Self, String> {
        let trimmed = input.trim();
        let raw = trimmed.to_string();

        let (vendor, rest) = split_vendor(trimmed);
        let (version, rest) = parse_version(rest);

        // If no version was found and there was no vendor split,
        // the remaining text could be a vendor name or an architecture.
        if vendor.is_none() && version.is_none() && !rest.is_empty() {
            // Check if it's an architecture pattern first (short, no spaces).
            if looks_like_arch(rest) {
                let arch = parse_arch(rest);
                return Ok(JavaSpec {
                    vendor: None,
                    version: None,
                    arch,
                    raw,
                });
            }
            // Otherwise treat as vendor name.
            return Ok(JavaSpec {
                vendor: Some(rest.to_string()),
                version: None,
                arch: None,
                raw,
            });
        }

        let arch = parse_arch(rest);

        Ok(JavaSpec {
            vendor,
            version,
            arch,
            raw,
        })
    }

    /// Check whether a [`JavaInfo`] matches this spec.
    ///
    /// All present criteria (vendor, version, architecture) must match.
    /// Missing criteria are ignored (i.e., a spec with no vendor matches
    /// any vendor).
    ///
    /// Vendor matching is case-insensitive substring.
    /// Architecture matching is case-insensitive substring.
    pub fn matches(&self, info: &crate::JavaInfo) -> bool {
        if let Some(ref v) = self.vendor {
            let name_match = info.name.to_lowercase().contains(&v.to_lowercase());
            let vendor_match = info.vendor.to_lowercase().contains(&v.to_lowercase());
            if !name_match && !vendor_match {
                return false;
            }
        }

        if let Some(ref req) = self.version
            && !matches_version_req(&info.parsed_version, req)
        {
            return false;
        }

        if let Some(ref a) = self.arch {
            let arch_lower = info.architecture.to_lowercase();
            if !arch_lower.contains(&a.to_lowercase()) {
                return false;
            }
        }

        true
    }
}

/// Split input into vendor portion (before ` v`) and the rest.
///
/// Only splits on ` v` when what follows looks like a version expression
/// (starts with `v^`, `v~`, `v>=`, `v<=`, `v>`, `v<`, or a digit after `v`).
fn split_vendor(input: &str) -> (Option<String>, &str) {
    // Look for " v" where the v is followed by version-like characters
    if let Some(idx) = find_version_prefix(input) {
        let vendor = input[..idx].trim();
        let rest = input[idx + 1..].trim(); // skip " v"
        let vendor = if vendor.is_empty() {
            None
        } else {
            Some(vendor.to_string())
        };
        (vendor, rest)
    } else {
        (None, input)
    }
}

/// Find the position of ` v` that introduces a version expression.
fn find_version_prefix(input: &str) -> Option<usize> {
    let bytes = input.as_bytes();
    // We need to find " v" followed by version-indicating characters
    for i in 0..bytes.len().saturating_sub(2) {
        if bytes[i] == b' ' && bytes[i + 1] == b'v' {
            // Check what follows the 'v'
            let after = &bytes[i + 2..];
            if after.is_empty() {
                continue;
            }
            // Version can start with operator: ^ ~ >= <= > < or digit
            let next = after[0];
            if next.is_ascii_digit() || next == b'^' || next == b'~' || next == b'>' || next == b'<'
            {
                return Some(i);
            }
            // If 'v' is followed by >= or <=
            if after.len() >= 2 && (next == b'>' || next == b'<') && after[1] == b'=' {
                return Some(i);
            }
        }
    }
    None
}

/// Parse version constraint from the start of `input`.
///
/// Returns the parsed requirement (if any) and the remaining string.
fn parse_version(input: &str) -> (Option<VersionReq>, &str) {
    // Strip optional leading 'v' prefix
    let input = input.strip_prefix('v').unwrap_or(input).trim_start();

    if input.is_empty() {
        return (None, input);
    }

    // Check if it starts with a known operator
    if let Some(rest) = input.strip_prefix("^") {
        let (v, rest) = parse_version_number(rest);
        let req = v.map(|(major, minor, patch)| VersionReq::Compatible {
            major,
            minor,
            patch,
        });
        return (req, rest);
    }
    if let Some(rest) = input.strip_prefix("~") {
        let (v, rest) = parse_version_number(rest);
        let req = v.map(|(major, minor, patch)| VersionReq::Approx {
            major,
            minor,
            patch,
        });
        return (req, rest);
    }
    if let Some(rest) = input.strip_prefix(">=") {
        let (v, rest) = parse_version_number(rest);
        let req = v.map(|(major, minor, patch)| VersionReq::Gte {
            major,
            minor,
            patch,
        });
        return (req, rest);
    }
    if let Some(rest) = input.strip_prefix("<=") {
        let (v, rest) = parse_version_number(rest);
        let req = v.map(|(major, minor, patch)| VersionReq::Lte {
            major,
            minor,
            patch,
        });
        return (req, rest);
    }
    if let Some(rest) = input.strip_prefix(">") {
        let (v, rest) = parse_version_number(rest);
        let req = v.map(|(major, minor, patch)| VersionReq::Gt {
            major,
            minor,
            patch,
        });
        return (req, rest);
    }
    if let Some(rest) = input.strip_prefix("<") {
        let (v, rest) = parse_version_number(rest);
        let req = v.map(|(major, minor, patch)| VersionReq::Lt {
            major,
            minor,
            patch,
        });
        return (req, rest);
    }

    // No operator — try parsing as exact version
    let (v, rest) = parse_version_number(input);
    (
        v.map(|(maj, min, patch)| VersionReq::Exact {
            major: maj,
            minor: min,
            patch,
        }),
        rest,
    )
}

/// Parse a version number in the form `major.minor.patch` from the start.
///
/// Returns the parsed components and the remaining string.
fn parse_version_number(input: &str) -> (Option<(u64, u64, u64)>, &str) {
    let trimmed = input.trim_start();

    // Collect digits and dots until we hit a non-version character
    let version_end = trimmed
        .find(|c: char| !c.is_ascii_digit() && c != '.')
        .unwrap_or(trimmed.len());
    let version_part = &trimmed[..version_end];
    let rest = trimmed[version_end..].trim_start();

    let parts: Vec<&str> = version_part.split('.').collect();
    if parts.is_empty() || parts[0].is_empty() {
        return (None, input);
    }

    let major = match parts[0].parse() {
        Ok(m) => m,
        Err(_) => return (None, input),
    };
    let minor = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    let patch = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);

    (Some((major, minor, patch)), rest)
}

/// Parse architecture from the input (may start with `a` prefix).
fn parse_arch(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    // Strip leading 'a' prefix if present (e.g. "ax86_64" -> "x86_64")
    // But keep "aarch64" as-is since that's the actual arch name
    let arch = if trimmed.len() > 1 && trimmed.starts_with('a') && trimmed != "aarch64" {
        trimmed[1..].to_string()
    } else {
        trimmed.to_string()
    };

    if arch.is_empty() { None } else { Some(arch) }
}

/// Check if a string looks like an architecture name.
fn looks_like_arch(input: &str) -> bool {
    let trimmed = input.trim();
    if trimmed.contains(' ') {
        return false; // architectures don't have spaces
    }
    let lower = trimmed.to_lowercase();
    let known = ["x86_64", "amd64", "aarch64", "arm64", "x86", "i386", "i686"];
    if known.contains(&lower.as_str()) {
        return true;
    }
    // Pattern: optional 'a' prefix + known arch name
    if trimmed.starts_with('a') || trimmed.starts_with('A') {
        let rest = &trimmed[1..];
        let rest_lower = rest.to_lowercase();
        if known.contains(&rest_lower.as_str()) {
            return true;
        }
    }
    false
}

/// Check if parsed_version matches a version requirement.
fn matches_version_req(parsed: &Option<crate::JavaVersion>, req: &VersionReq) -> bool {
    let parsed = match parsed {
        Some(v) => v,
        None => return false,
    };

    let (req_major, req_minor, req_patch) = match *req {
        VersionReq::Compatible {
            major,
            minor,
            patch,
        }
        | VersionReq::Approx {
            major,
            minor,
            patch,
        }
        | VersionReq::Gte {
            major,
            minor,
            patch,
        }
        | VersionReq::Lte {
            major,
            minor,
            patch,
        }
        | VersionReq::Gt {
            major,
            minor,
            patch,
        }
        | VersionReq::Lt {
            major,
            minor,
            patch,
        }
        | VersionReq::Exact {
            major,
            minor,
            patch,
        } => (major, minor, patch),
    };

    let parsed_tuple = (parsed.major, parsed.minor, parsed.patch);

    match *req {
        VersionReq::Compatible { .. } => {
            parsed_tuple >= (req_major, req_minor, req_patch)
                && (parsed.major, 0u64, 0u64) < (req_major + 1, 0, 0)
        }
        VersionReq::Approx { .. } => {
            parsed_tuple >= (req_major, req_minor, req_patch)
                && parsed_tuple < (req_major, req_minor + 1, 0)
        }
        VersionReq::Gte { .. } => parsed_tuple >= (req_major, req_minor, req_patch),
        VersionReq::Lte { .. } => parsed_tuple <= (req_major, req_minor, req_patch),
        VersionReq::Gt { .. } => parsed_tuple > (req_major, req_minor, req_patch),
        VersionReq::Lt { .. } => parsed_tuple < (req_major, req_minor, req_patch),
        VersionReq::Exact { .. } => parsed_tuple == (req_major, req_minor, req_patch),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::JavaInfo;

    // -----------------------------------------------------------------------
    // Parsing
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_full_spec() {
        let spec = JavaSpec::parse("Eclipse Adoptium v^17.0.0 ax86_64").unwrap();
        assert_eq!(spec.vendor.as_deref(), Some("Eclipse Adoptium"));
        assert_eq!(
            spec.version,
            Some(VersionReq::Compatible {
                major: 17,
                minor: 0,
                patch: 0
            })
        );
        assert_eq!(spec.arch.as_deref(), Some("x86_64"));
        assert_eq!(spec.raw, "Eclipse Adoptium v^17.0.0 ax86_64");
    }

    #[test]
    fn test_parse_vendor_only() {
        let spec = JavaSpec::parse("Azul Zulu").unwrap();
        assert_eq!(spec.vendor.as_deref(), Some("Azul Zulu"));
        assert!(spec.version.is_none());
        assert!(spec.arch.is_none());
    }

    #[test]
    fn test_parse_version_only() {
        let spec = JavaSpec::parse("v^11.0.2").unwrap();
        assert!(spec.vendor.is_none());
        assert_eq!(
            spec.version,
            Some(VersionReq::Compatible {
                major: 11,
                minor: 0,
                patch: 2
            })
        );
        assert!(spec.arch.is_none());
    }

    #[test]
    fn test_parse_exact_version() {
        let spec = JavaSpec::parse("v17.0.1").unwrap();
        assert_eq!(
            spec.version,
            Some(VersionReq::Exact {
                major: 17,
                minor: 0,
                patch: 1
            })
        );
    }

    #[test]
    fn test_parse_tilde_version() {
        let spec = JavaSpec::parse("v~21.0.0").unwrap();
        assert_eq!(
            spec.version,
            Some(VersionReq::Approx {
                major: 21,
                minor: 0,
                patch: 0
            })
        );
    }

    #[test]
    fn test_parse_gte_version() {
        let spec = JavaSpec::parse("v>=8.0.0").unwrap();
        assert_eq!(
            spec.version,
            Some(VersionReq::Gte {
                major: 8,
                minor: 0,
                patch: 0
            })
        );
    }

    #[test]
    fn test_parse_arch_only() {
        let spec = JavaSpec::parse("aaarch64").unwrap();
        assert!(spec.vendor.is_none());
        assert!(spec.version.is_none());
        assert_eq!(spec.arch.as_deref(), Some("aarch64"));
    }

    #[test]
    fn test_parse_invalid_version() {
        let spec = JavaSpec::parse("vabc").unwrap();
        assert!(spec.version.is_none());
    }

    #[test]
    fn test_parse_empty() {
        let spec = JavaSpec::parse("").unwrap();
        assert!(spec.vendor.is_none());
        assert!(spec.version.is_none());
        assert!(spec.arch.is_none());
    }

    #[test]
    fn test_parse_arch_x86() {
        let spec = JavaSpec::parse("ax86_64").unwrap();
        assert_eq!(spec.arch.as_deref(), Some("x86_64"));
    }

    // -----------------------------------------------------------------------
    // Matching
    // -----------------------------------------------------------------------

    fn make_info(name: &str, version: &str, arch: &str) -> JavaInfo {
        JavaInfo {
            name: name.to_string(),
            vendor: name.to_string(),
            version: version.to_string(),
            parsed_version: crate::JavaVersion::parse(version),
            architecture: arch.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn test_match_vendor_substring() {
        let spec = JavaSpec::parse("Adoptium v^17.0.0 ax86_64").unwrap();
        let info = make_info("Eclipse Adoptium", "17.0.2", "x86_64");
        assert!(spec.matches(&info));
    }

    #[test]
    fn test_match_vendor_no_match() {
        let spec = JavaSpec::parse("Azul").unwrap();
        let info = make_info("Eclipse Adoptium", "17.0.2", "x86_64");
        assert!(!spec.matches(&info));
    }

    #[test]
    fn test_match_version_compatible() {
        let spec = JavaSpec::parse("v^17.0.0").unwrap();
        // 17.0.2 >= 17.0.0 AND < 18.0.0 → true
        assert!(spec.matches(&make_info("Any", "17.0.2", "x86_64")));
        // 18.0.0 >= 17.0.0 but NOT < 18.0.0 → false
        assert!(!spec.matches(&make_info("Any", "18.0.0", "x86_64")));
        // 11.0.0 < 17.0.0 → false
        assert!(!spec.matches(&make_info("Any", "11.0.0", "x86_64")));
    }

    #[test]
    fn test_match_version_exact() {
        let spec = JavaSpec::parse("v17.0.2").unwrap();
        assert!(spec.matches(&make_info("Any", "17.0.2", "x86_64")));
        assert!(!spec.matches(&make_info("Any", "17.0.3", "x86_64")));
    }

    #[test]
    fn test_match_version_approx() {
        let spec = JavaSpec::parse("v~17.0.0").unwrap();
        assert!(spec.matches(&make_info("Any", "17.0.2", "x86_64")));
        assert!(!spec.matches(&make_info("Any", "17.1.0", "x86_64")));
    }

    #[test]
    fn test_match_version_gte() {
        let spec = JavaSpec::parse("v>=17.0.0").unwrap();
        assert!(spec.matches(&make_info("Any", "17.0.0", "x86_64")));
        assert!(spec.matches(&make_info("Any", "21.0.1", "x86_64")));
        assert!(!spec.matches(&make_info("Any", "11.0.0", "x86_64")));
    }

    #[test]
    fn test_match_arch_substring() {
        let spec = JavaSpec::parse("aarch64").unwrap();
        assert!(spec.matches(&make_info("Any", "17.0.0", "aarch64")));
        assert!(spec.matches(&make_info("Any", "17.0.0", "ARM 64 aarch64")));
        assert!(!spec.matches(&make_info("Any", "17.0.0", "x86_64")));
    }

    #[test]
    fn test_match_all_criteria() {
        let spec = JavaSpec::parse("Eclipse Adoptium v^17.0.0 ax86_64").unwrap();
        let info = make_info("Eclipse Adoptium", "17.0.2", "x86_64");
        assert!(spec.matches(&info));
    }

    #[test]
    fn test_match_all_criteria_fails_vendor() {
        let spec = JavaSpec::parse("Eclipse Adoptium v^17.0.0 ax86_64").unwrap();
        let info = make_info("Azul Zulu", "17.0.2", "x86_64");
        assert!(!spec.matches(&info));
    }

    #[test]
    fn test_match_all_criteria_fails_arch() {
        let spec = JavaSpec::parse("Eclipse Adoptium v^17.0.0 ax86_64").unwrap();
        let info = make_info("Eclipse Adoptium", "17.0.2", "aarch64");
        assert!(!spec.matches(&info));
    }

    #[test]
    fn test_match_empty_spec() {
        let spec = JavaSpec::parse("").unwrap();
        assert!(spec.matches(&make_info("Anything", "17.0.0", "any")));
    }

    #[test]
    fn test_match_no_parsed_version() {
        let spec = JavaSpec::parse("v^17.0.0").unwrap();
        let info = JavaInfo::default();
        assert!(!spec.matches(&info));
    }

    #[test]
    fn test_version_req_display() {
        assert_eq!(
            VersionReq::Compatible {
                major: 17,
                minor: 0,
                patch: 0
            }
            .to_string(),
            "^17.0.0"
        );
        assert_eq!(
            VersionReq::Exact {
                major: 11,
                minor: 0,
                patch: 2
            }
            .to_string(),
            "11.0.2"
        );
        assert_eq!(
            VersionReq::Gte {
                major: 8,
                minor: 0,
                patch: 0
            }
            .to_string(),
            ">=8.0.0"
        );
    }
}
