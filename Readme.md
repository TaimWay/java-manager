<div align="center">

# java-manager

![Rust.Crate](https://img.shields.io/badge/Crate-java--manager-brightgreen?style=for-the-badge&logo=Rust&logoColor=orange
)
[![Crates.io](https://img.shields.io/crates/v/java-manager.svg?style=for-the-badge)](https://crates.io/crates/java-manager)
[![License](https://img.shields.io/github/license/TaimWay/java-manager?style=for-the-badge&logo=apachelucene&logoColor=white
)](https://github.com/TaimWay/java-manager/blob/main/LICENSE-APACHE.txt)

[![Github](https://img.shields.io/badge/Github-TaimWay%2Fjava--manager-black?style=for-the-badge&logo=Github&logoColor=white
)](https://github.com/TaimWay/java-manager)
[![Author](https://img.shields.io/badge/Author-TaimWay-green?style=for-the-badge&logo=devdotto&logoColor=white
)](https://github.com/TaimWay)
![DevState](https://img.shields.io/badge/DevState-Debug%2FIndev-red?style=for-the-badge&logo=devbox&logoColor=red
)

A comprehensive Rust library for discovering, managing, and interacting with Java installations.

</div>

---

> **The project is currently under development. All bugs related to the project can be reported by submitting issues on GitHub, and we will regularly fix the reported problems**

## Features

- **Cross‑platform** – Works on Windows, macOS, and Linux.
- **Java discovery** – Find Java via `PATH` (`quick_search`), Everything SDK on Windows (`deep_search`), or multi-strategy full scan (`full_search`): registry (HKLM + HKCU, including Azul, BellSoft, Temurin, Corretto, GraalVM), keyword BFS, Microsoft Store, `where`/`/usr/libexec/java_home`, and walkdir.
- **Structured metadata** – `JavaInfo` provides name, version, vendor, architecture, `JAVA_HOME`, and a parsed `JavaVersion` (major/minor/patch).
- **Version matching** – `best_match()` and `filter_by_version()` let you pick the right Java for your project by version requirement.
- **Execution control** – Run JARs or main classes with configurable memory limits, arguments, and I/O redirection.
- **TTL caching** – `JavaCache` avoids redundant full-disk scans (default 10 s TTL).
- **Parallel search** – Optional `parallel` feature enables rayon-powered concurrent scanning.
- **Debug logging** – Built-in `log` crate integration for troubleshooting search paths.
- **Error handling** – Comprehensive `JavaError` enum including `StaleRegistryEntry` for broken registry references.

## Installation

```toml
[dependencies]
java-manager = "0.3"
```

Or use the `cargo` command:

```bash
cargo add java-manager
```

### Optional features

| Feature | Description |
|---|---|
| `parallel` | Enables `parallel_full_search()` via rayon (requires Rust edition 2024 compatible version of rayon) |

## Usage

### Locate Java installations

```rust
use java_manager::{quick_search, deep_search, full_search, java_home};

// Quick search: look for 'java' in every directory in PATH
let javas = quick_search()?;
for java in javas {
    println!("Found Java at {} (version {})", java.path.display(), java.version);
}

// Deep search: Everything SDK (Windows) or walkdir (Linux/macOS)
let all_javas = deep_search()?;

// Full search: registry + BFS + MS Store + where + JVM directories
let all_javas = full_search()?;

// Check JAVA_HOME environment variable
if let Some(java) = java_home() {
    println!("JAVA_HOME points to Java version {}", java.version);
}
```

### Filter by version requirement

```rust
use java_manager::{quick_search, best_match, filter_by_version, JavaInfo};

let javas = quick_search()?;

// Best match for Java 17 (highest patch version)
if let Some(java17) = best_match(javas.clone(), "17") {
    println!("Best Java 17: {} (version {})", java17.path.display(), java17.version);
}

// All Java 11 installations
let java11_installs = filter_by_version(javas, "11");
```

### Cached search (avoids repeated full scans)

```rust
use java_manager::{JavaCache, full_search};
use std::time::Duration;

let mut cache = JavaCache::new(Duration::from_secs(30));

// First call runs the search, subsequent calls are cached
let javas = cache.get_or_refresh(|| full_search())?;

// Force a refresh
let javas = cache.force_refresh(|| full_search())?;
```

### Parallel search (requires `parallel` feature)

```rust
use java_manager::parallel_full_search;

let javas = parallel_full_search()?;
```

### Execute a Java program

```rust
use java_manager::{JavaRunner, JavaRedirect, java_home};

let java = java_home().expect("JAVA_HOME not set");

// Run a JAR file
JavaRunner::new()
    .java(java.clone())
    .jar("myapp.jar")
    .min_memory(256 * 1024 * 1024)   // 256 MB
    .max_memory(1024 * 1024 * 1024)  // 1 GB
    .arg("--server")
    .redirect(JavaRedirect::new().output("out.log").error("err.log"))
    .execute()?;

// Or run a main class
JavaRunner::new()
    .java(java)
    .main_class("com.example.Main")
    .arg("arg1")
    .arg("arg2")
    .execute()?;
```

### Metadata from a specific Java path

```rust
use java_manager::JavaInfo;

let info = JavaInfo::new("/usr/lib/jvm/java-11-openjdk/bin/java".into())?;
println!("Name: {}", info.name);
println!("Version: {}", info.version);
println!("Parsed version: {}", info.parsed_version.unwrap()); // e.g. "11.0.2"
println!("Vendor: {}", info.vendor);
println!("Architecture: {}", info.architecture);
println!("JAVA_HOME: {}", info.java_home.display());
```

## API Overview

| Function | Description |
|---|---|
| `quick_search()` | Walks `$PATH` — fastest, catches the default Java |
| `deep_search()` | Windows: Everything SDK. Linux/macOS: delegates to `full_search()` |
| `full_search()` | Registry (HKLM + HKCU), BFS, Microsoft Store, `where`, JVM directories |
| `parallel_full_search()` | Same as `full_search()` but uses rayon (feature `parallel`) |
| `java_home()` | Returns the Java pointed to by `$JAVA_HOME` |
| `filter_by_version(javas, req)` | Returns installations matching a version requirement |
| `best_match(javas, req)` | Returns the highest-versioned match |
| `JavaCache::new(ttl)` | TTL cache wrapper for search results |
| `JavaInfo::matches_version(req)` | Checks if this installation matches a version requirement |

### Version requirement syntax

| `req` | Matches |
|---|---|
| `"17"` | Any Java 17 (major == 17) |
| `"17.0"` | Any Java 17.0.x |
| `"17.0.2"` | Exact version 17.0.2 |

## Search Strategy Details

`full_search()` attempts multiple discovery strategies on each platform:

- **Windows**: Registry (`HKLM` + `HKCU` for JavaSoft, Azul, BellSoft, Eclipse Temurin, Amazon Corretto, GraalVM) → Keyword BFS on all drives → Microsoft Store → `where java`
- **Linux**: Walkdir over `/usr/lib/jvm`, `/usr/java`, `/opt`, `/usr/local`, `~/.minecraft/runtime`
- **macOS**: Walkdir over `/Library/Java/JavaVirtualMachines`, `~/Library/Java/JavaVirtualMachines`, `~/.minecraft/runtime`, plus `/usr/libexec/java_home`

All platforms also check `$JAVA_HOME`.

**Nested JRE deduplication**: When both a JDK and its bundled JRE are
discovered (e.g. `jdk-xxx/jre/bin/java.exe`), only the JDK-level installation
is kept. Standalone JREs (whose `java_home` is not a subdirectory of another
entry) are unaffected. This avoids duplicate entries in results.

## Debug Logging

Enable logging to see what the library is doing behind the scenes:

```bash
RUST_LOG=debug cargo run
```

This will output scan paths, registry keys checked, stale entries skipped, and version parsing results.

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.
