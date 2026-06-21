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

A comprehensive Rust library for discovering, managing, and interacting with Java installations.

</div>

---

## Features

- **Cross‑platform** – Works on Windows, macOS, and Linux.
- **Java discovery** – Find Java via `PATH` (`quick_search`), Everything SDK on Windows (`deep_search`, requires `everything` feature), or multi‑strategy full scan (`full_search`):
  - **Windows**: Registry (HKLM + HKCU, including Azul, BellSoft, Temurin, Corretto, GraalVM), keyword BFS, Microsoft Store, `where` command, Chocolatey, Scoop, JetBrains bundled JDK.
  - **Linux**: walkdir over `/usr/lib/jvm`, `/usr/java`, `/opt`, `/usr/local`, plus SDKMAN, JBang, asdf‑vm, JetBrains bundled JDK, Minecraft runtime.
  - **macOS**: `/Library/Java/JavaVirtualMachines`, `/usr/libexec/java_home`, plus SDKMAN, JBang, asdf‑vm, Homebrew, JetBrains bundled JDK, Minecraft runtime.
  - **All platforms**: `$JAVA_HOME` is always checked first.
- **Structured metadata** – `JavaInfo` provides name, version, vendor, architecture, `JAVA_HOME`, parsed `JavaVersion`, and JDK capabilities.
- **Version matching** – `best_match()` and `filter_by_version()` let you pick the right Java by version requirement (`"17"`, `"17.0"`, `"17.0.2"`).
- **Execution control** – Run JARs or main classes with `JavaRunner`: classpath, module path, `--add-opens`/`--add-exports`, system properties, memory limits, environment variables, working directory, process timeout, I/O redirection with append mode.
- **TTL caching** – `JavaCache` avoids redundant full‑disk scans (default 300 s TTL).
- **Parallel search** – Optional `parallel` feature enables rayon‑powered concurrent `JavaInfo` resolution.
- **Download** – Optional `download` feature for async, resumable, parallel‑chunked JDK downloads with automatic extraction (ZIP / tar.gz).
- **Debug logging** – Built‑in `log` crate integration.
- **Nested JRE dedup** – Automatically removes bundled JREs that are sub‑directories of a parent JDK.

## Installation

```toml
[dependencies]
java-manager = "0.4"
```

Or:

```bash
cargo add java-manager
```

### Optional features

| Feature | Description |
|---|---|
| `parallel` | Enables `parallel_full_search()` via rayon |
| `download` | Async JDK download with resume, parallel chunks, and archive extraction (ZIP / tar.gz) |
| `everything` | Use Everything SDK for near-instant Windows search (falls back to `full_search` on error) |

## Usage

### Locate Java installations

```rust
use java_manager::{quick_search, deep_search, full_search, java_home};

// Quick search: look for 'java' in every directory in PATH
let javas = quick_search()?;
for java in javas {
    println!("Found Java at {} (version {})", java.path.display(), java.version);
}

// Deep search: Everything SDK (Windows, falls back to full_search) or walkdir (Linux/macOS)
let all_javas = deep_search()?;

// Full search: registry + BFS + MS Store + where + package managers + IDE paths
let all_javas = full_search()?;

// Check JAVA_HOME environment variable
if let Some(java) = java_home() {
    println!("JAVA_HOME points to Java version {}", java.version);
}
```

### Filter by version requirement

```rust
use java_manager::{quick_search, best_match, filter_by_version};

let javas = quick_search()?;

// Best match for Java 17 (highest patch version)
if let Some(java17) = best_match(javas.clone(), "17") {
    println!("Best Java 17: {} (version {})", java17.path.display(), java17.version);
}

// All Java 11 installations
let java11_installs = filter_by_version(javas, "11");
```

### Cached search

```rust
use java_manager::{JavaCache, full_search};
use std::time::Duration;

let mut cache = JavaCache::new(Duration::from_secs(300));

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

// Run a JAR file with memory limits and I/O redirection
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

### Advanced JavaRunner

```rust
use java_manager::{JavaRunner, java_home};
use std::time::Duration;

let java = java_home().expect("JAVA_HOME not set");

JavaRunner::new()
    .java(java)
    .jar("app.jar")
    .classpath(&["lib/*", "config/"])
    .add_opens("java.base", "java.lang", "ALL-UNNAMED")
    .add_exports("java.base", "com.sun.internal", "ALL-UNNAMED")
    .system_property("myapp.config", "prod")
    .env("MY_VAR", "value")
    .working_dir("/opt/myapp")
    .timeout(Duration::from_secs(30))
    .redirect(
        JavaRedirect::new()
            .output("app.log")
            .append_output()
            .error("app.err")
            .append_error()
    )
    .execute()?;
```

### JDK detection and capabilities

```rust
use java_manager::JavaInfo;

let info = JavaInfo::new("/usr/lib/jvm/java-17-openjdk/bin/java".into())?;

// Check if this is a full JDK (has javac)
if info.is_jdk() {
    println!("Full JDK detected");
}

// List available JDK tools
let tools = info.capabilities();
println!("Available tools: {:?}", tools);
// e.g. ["javac", "javadoc", "jar", "jlink", "jshell", "jcmd"]
```

### Metadata from a specific Java path

```rust
use java_manager::JavaInfo;

let info = JavaInfo::new("/usr/lib/jvm/java-11-openjdk/bin/java".into())?;
println!("Name: {}", info.name);
println!("Version: {}", info.version);
println!("Parsed version: {}", info.parsed_version.unwrap());
println!("Vendor: {}", info.vendor);
println!("Architecture: {}", info.architecture);
println!("JAVA_HOME: {}", info.java_home.display());
```

## API Overview

| Function / Method | Description |
|---|---|
| `quick_search()` | Walks `$PATH` — fastest, catches the default Java |
| `deep_search()` | Windows: Everything SDK (requires `everything` feature, falls back to `full_search`).<br>Linux/macOS: `full_search()` |
| `full_search()` | Registry, BFS, MS Store, `where`, package managers, IDE paths, JVM directories |
| `parallel_full_search()` | Same as `full_search()` but with rayon‑parallel `JavaInfo` resolution |
| `java_home()` | Returns the Java pointed to by `$JAVA_HOME` |
| `filter_by_version(javas, req)` | Returns installations matching a version requirement |
| `best_match(javas, req)` | Returns the highest‑versioned match |
| `JavaCache::new(ttl)` | TTL cache wrapper for search results |
| `JavaInfo::new(path)` | Create `JavaInfo` from a path to `java` or `JAVA_HOME` |
| `JavaInfo::matches_version(req)` | Checks version requirement match |
| `JavaInfo::is_jdk()` | Returns `true` if `javac` is present (full JDK) |
| `JavaInfo::capabilities()` | Returns list of available JDK tools |
| `JavaInfo::execute(args)` | Run `java` with shell‑word arguments |
| `JavaRunner::new()` | Builder for configuring and executing Java programs |
| `JavaRunner::execute()` | Run the configured program |

### JavaRunner builder methods

| Method | Description |
|---|---|
| `.java(info)` | Set the Java runtime (required) |
| `.jar(path)` | Set the JAR to execute |
| `.main_class(name)` | Set the main class to execute |
| `.classpath(paths)` | Set classpath (`-cp`) |
| `.module_path(path)` | Set module path (`--module-path`) |
| `.add_opens(m, p, t)` | Add `--add-opens` flag |
| `.add_exports(m, p, t)` | Add `--add-exports` flag |
| `.system_property(k, v)` | Set `-Dkey=value` |
| `.min_memory(bytes)` | Set initial heap (`-Xms`) |
| `.max_memory(bytes)` | Set max heap (`-Xmx`) |
| `.arg(val)` | Add a program argument |
| `.env(key, val)` | Set environment variable |
| `.working_dir(path)` | Set working directory |
| `.timeout(duration)` | Kill process after timeout |
| `.redirect(redirect)` | Set I/O redirection |

### JavaRedirect methods

| Method | Description |
|---|---|
| `.output(path)` | Redirect stdout to file (truncate) |
| `.error(path)` | Redirect stderr to file (truncate) |
| `.input(path)` | Redirect stdin from file |
| `.append_output()` | Append to stdout file instead of truncating |
| `.append_error()` | Append to stderr file instead of truncating |

### Version requirement syntax

| `req` | Matches |
|---|---|
| `"17"` | Any Java 17 (major == 17) |
| `"17.0"` | Any Java 17.0.x |
| `"17.0.2"` | Exact version 17.0.2 |

## Search Strategy Details

`full_search()` attempts multiple discovery strategies on each platform:

| Platform | Search locations |
|---|---|
| **Windows** | `$JAVA_HOME` → Registry (HKLM+HKCU for JavaSoft, Azul, BellSoft, Temurin, Corretto, GraalVM) → BFS on all drives → Microsoft Store → `where java` → Chocolatey → Scoop → JetBrains IDE bundled JDK |
| **Linux** | `$JAVA_HOME` → `/usr/lib/jvm` → `/usr/java` → `/opt` → `/usr/local` → SDKMAN → JBang → asdf‑vm → JetBrains bundled JDK → Minecraft runtime |
| **macOS** | `$JAVA_HOME` → `/Library/Java/JavaVirtualMachines` → `~/Library/Java/JavaVirtualMachines` → `/usr/libexec/java_home` → SDKMAN → JBang → asdf‑vm → Homebrew → JetBrains bundled JDK → Minecraft runtime |

**Nested JRE deduplication**: When both a JDK and its bundled JRE are
discovered (e.g. `jdk-xxx/jre/bin/java.exe`), only the JDK-level installation
is kept. Standalone JREs are unaffected.

**`deep_search()` fallback**: On Windows, when the `everything` feature is
enabled and the Everything SDK is not available (service not running / not
installed), `deep_search()` automatically falls back to `full_search()`. Without
the `everything` feature, `deep_search()` always delegates to `full_search()`.

## Debug Logging

Enable logging to see what the library is doing behind the scenes:

```bash
RUST_LOG=debug cargo run
```

This will output scan paths, registry keys checked, stale entries skipped, and version parsing results.

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.
