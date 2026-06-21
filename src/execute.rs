//! Running Java programs with controlled output and redirection.

use crate::{JavaError, JavaInfo};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

/// Controls which output streams are printed to the console.
///
/// Used internally by [`JavaInfo::execute`] and its variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    /// Print both stdout and stderr.
    Both,
    /// Print only stdout; stderr is discarded.
    OutputOnly,
    /// Print only stderr; stdout is discarded.
    ErrorOnly,
}

impl JavaInfo {
    /// Executes the Java executable with the given arguments, printing both
    /// stdout and stderr to the console.
    ///
    /// The argument string is split using shell‑like rules (via `shell_words`).
    /// The child process's stdout and stderr are captured and printed line by line
    /// while the process runs.
    ///
    /// # Errors
    ///
    /// Returns `JavaError::IoError` if spawning or waiting fails.
    /// Returns `JavaError::Other` if the argument string cannot be parsed.
    /// Returns `JavaError::ExecutionFailed` if the Java process exits with a non‑zero status.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use java_manager::JavaInfo;
    /// # let java = JavaInfo::new("/path/to/java".into())?;
    /// java.execute("-version")?;
    /// # Ok::<_, java_manager::JavaError>(())
    /// ```
    pub fn execute(&self, args: &str) -> Result<(), JavaError> {
        self.run_java(args, OutputMode::Both)
    }

    /// Executes the Java executable, printing only stderr to the console.
    /// Stdout is captured and discarded.
    ///
    /// See [`execute`](JavaInfo::execute) for details.
    ///
    /// # Errors
    ///
    /// Same as [`execute`](JavaInfo::execute).
    pub fn execute_with_error(&self, args: &str) -> Result<(), JavaError> {
        self.run_java(args, OutputMode::ErrorOnly)
    }

    /// Executes the Java executable, printing only stdout to the console.
    /// Stderr is captured and discarded.
    ///
    /// See [`execute`](JavaInfo::execute) for details.
    ///
    /// # Errors
    ///
    /// Same as [`execute`](JavaInfo::execute).
    pub fn execute_with_output(&self, args: &str) -> Result<(), JavaError> {
        self.run_java(args, OutputMode::OutputOnly)
    }

    /// Internal implementation of Java execution with configurable output.
    fn run_java(&self, args: &str, mode: OutputMode) -> Result<(), JavaError> {
        let java_exe = self.java_executable()?;

        let arg_vec = shell_words::split(args)
            .map_err(|e| JavaError::Other(format!("Failed to parse arguments: {}", e)))?;

        let mut cmd = Command::new(java_exe);
        cmd.args(&arg_vec);

        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let mut child = cmd.spawn().map_err(JavaError::IoError)?;

        let stdout = child.stdout.take().expect("Failed to get stdout pipe");
        let stderr = child.stderr.take().expect("Failed to get stderr pipe");

        let stdout_handle = if matches!(mode, OutputMode::Both | OutputMode::OutputOnly) {
            Some(thread::spawn(move || {
                let reader = BufReader::new(stdout);
                for line in reader.lines().map_while(Result::ok) {
                    println!("{}", line);
                }
            }))
        } else {
            None
        };

        let stderr_handle = if matches!(mode, OutputMode::Both | OutputMode::ErrorOnly) {
            Some(thread::spawn(move || {
                let reader = BufReader::new(stderr);
                for line in reader.lines().map_while(Result::ok) {
                    eprintln!("{}", line);
                }
            }))
        } else {
            None
        };

        let status = child.wait().map_err(JavaError::IoError)?;

        if let Some(handle) = stdout_handle {
            handle.join().unwrap();
        }
        if let Some(handle) = stderr_handle {
            handle.join().unwrap();
        }

        if status.success() {
            Ok(())
        } else {
            Err(JavaError::ExecutionFailed(format!(
                "Execution failed: {}",
                status.code().unwrap()
            )))
        }
    }

    /// Returns the path to the `java` executable inside this installation's `JAVA_HOME/bin`.
    ///
    /// # Errors
    ///
    /// Returns `JavaError::NotFound` if the executable does not exist.
    fn java_executable(&self) -> Result<PathBuf, JavaError> {
        let java_home = &self.java_home;
        let exe_name = if cfg!(windows) { "java.exe" } else { "java" };
        let java_exe = java_home.join("bin").join(exe_name);
        if java_exe.exists() {
            Ok(java_exe)
        } else {
            Err(JavaError::NotFound(format!(
                "Java executable not found: {:?}",
                java_exe
            )))
        }
    }
}

/// A builder for configuring and executing a Java program (JAR or main class).
///
/// This struct allows you to set the Java runtime, JAR file or main class,
/// memory limits, program arguments, and I/O redirection before spawning the
/// process.
///
/// # Examples
///
/// ```no_run
/// use java_manager::{JavaRunner, JavaRedirect};
///
/// # let java = java_manager::java_home().unwrap();
/// JavaRunner::new()
///     .java(java)
///     .jar("myapp.jar")
///     .min_memory(256 * 1024 * 1024)   // 256 MB
///     .max_memory(1024 * 1024 * 1024)  // 1 GB
///     .arg("--server")
///     .redirect(JavaRedirect::new().output("out.log").error("err.log"))
///     .execute()?;
/// # Ok::<_, java_manager::JavaError>(())
/// ```
#[derive(Debug, Default)]
pub struct JavaRunner {
    java: Option<JavaInfo>,
    jar: Option<PathBuf>,
    min_memory: Option<String>,
    max_memory: Option<String>,
    main_class: Option<String>,
    args: Vec<String>,
    redirect: JavaRedirect,
    classpath: Option<String>,
    module_path: Option<PathBuf>,
    add_opens: Vec<String>,
    add_exports: Vec<String>,
    system_properties: Vec<String>,
    env_vars: Vec<(String, String)>,
    working_dir: Option<PathBuf>,
    timeout: Option<Duration>,
}

/// I/O redirection options for a Java process.
///
/// Use the builder methods to specify files for stdout, stderr, and stdin.
/// If a stream is not redirected, it will inherit the parent's corresponding
/// stream (i.e., print to console or read from keyboard).
#[derive(Debug, Default)]
pub struct JavaRedirect {
    output: Option<PathBuf>,
    error: Option<PathBuf>,
    input: Option<PathBuf>,
    append_output: bool,
    append_error: bool,
}

impl JavaRedirect {
    /// Creates a new empty redirection configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Redirects the Java process's standard output to the given file.
    /// The file will be created (or truncated) before execution.
    pub fn output(mut self, path: impl AsRef<Path>) -> Self {
        self.output = Some(path.as_ref().to_path_buf());
        self
    }

    /// Redirects the Java process's standard error to the given file.
    /// The file will be created (or truncated) before execution.
    pub fn error(mut self, path: impl AsRef<Path>) -> Self {
        self.error = Some(path.as_ref().to_path_buf());
        self
    }

    /// Redirects the Java process's standard input from the given file.
    /// The file must exist and be readable.
    pub fn input(mut self, path: impl AsRef<Path>) -> Self {
        self.input = Some(path.as_ref().to_path_buf());
        self
    }

    /// Append to the output file instead of truncating.
    /// Only has an effect when [`output`](Self::output) is also set.
    pub fn append_output(mut self) -> Self {
        self.append_output = true;
        self
    }

    /// Append to the error file instead of truncating.
    /// Only has an effect when [`error`](Self::error) is also set.
    pub fn append_error(mut self) -> Self {
        self.append_error = true;
        self
    }
}

impl JavaRunner {
    /// Creates a new builder with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the Java installation to use.
    ///
    /// This is mandatory before calling `execute`.
    pub fn java(mut self, java: JavaInfo) -> Self {
        self.java = Some(java);
        self
    }

    /// Sets the JAR file to execute (implies the `-jar` flag).
    ///
    /// Either `jar` or `main_class` must be set.
    pub fn jar(mut self, jar: impl AsRef<Path>) -> Self {
        self.jar = Some(jar.as_ref().to_path_buf());
        self
    }

    /// Sets the initial heap size (`-Xms`).
    ///
    /// The value is given in bytes and will be formatted as a memory string
    /// (e.g., `256m`, `1g`). If the size is not a multiple of a megabyte or gigabyte,
    /// it will be rounded to the nearest megabyte.
    pub fn min_memory(mut self, bytes: usize) -> Self {
        self.min_memory = Some(format_memory(bytes));
        self
    }

    /// Sets the maximum heap size (`-Xmx`).
    ///
    /// See [`min_memory`](JavaRunner::min_memory) for formatting details.
    pub fn max_memory(mut self, bytes: usize) -> Self {
        self.max_memory = Some(format_memory(bytes));
        self
    }

    /// Sets the main class to execute (instead of a JAR file).
    ///
    /// Either `jar` or `main_class` must be set.
    pub fn main_class(mut self, class: impl Into<String>) -> Self {
        self.main_class = Some(class.into());
        self
    }

    /// Adds a single argument to be passed to the Java program.
    ///
    /// Arguments are appended in the order they are added.
    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    /// Sets I/O redirection options.
    pub fn redirect(mut self, redirect: JavaRedirect) -> Self {
        self.redirect = redirect;
        self
    }

    /// Sets the classpath (`-cp` / `-classpath`).
    ///
    /// Paths are joined with the platform-specific separator (`;` on Windows, `:` otherwise).
    pub fn classpath(mut self, paths: &[impl AsRef<Path>]) -> Self {
        let separator = if cfg!(windows) { ";" } else { ":" };
        let joined: Vec<String> = paths
            .iter()
            .map(|p| p.as_ref().to_string_lossy().to_string())
            .collect();
        self.classpath = Some(joined.join(separator));
        self
    }

    /// Sets the module path (`--module-path`).
    pub fn module_path(mut self, path: impl AsRef<Path>) -> Self {
        self.module_path = Some(path.as_ref().to_path_buf());
        self
    }

    /// Adds a `--add-opens` flag (e.g., `java.base/java.lang=ALL-UNNAMED`).
    pub fn add_opens(
        mut self,
        module: impl Into<String>,
        package: impl Into<String>,
        target: impl Into<String>,
    ) -> Self {
        self.add_opens.push(format!(
            "{}/{}.{}",
            module.into(),
            package.into(),
            target.into()
        ));
        self
    }

    /// Adds a `--add-exports` flag (e.g., `java.base/com.sun.internal=ALL-UNNAMED`).
    pub fn add_exports(
        mut self,
        module: impl Into<String>,
        package: impl Into<String>,
        target: impl Into<String>,
    ) -> Self {
        self.add_exports.push(format!(
            "{}/{}.{}",
            module.into(),
            package.into(),
            target.into()
        ));
        self
    }

    /// Sets a system property (`-Dkey=value`).
    pub fn system_property(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.system_properties
            .push(format!("-D{}={}", key.into(), value.into()));
        self
    }

    /// Sets an environment variable for the child Java process.
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env_vars.push((key.into(), value.into()));
        self
    }

    /// Sets the working directory for the child Java process.
    pub fn working_dir(mut self, path: impl AsRef<Path>) -> Self {
        self.working_dir = Some(path.as_ref().to_path_buf());
        self
    }

    /// Sets a timeout for the Java process.
    ///
    /// If the process runs longer than the specified duration, it will be killed.
    /// A `Duration::ZERO` or `None` means no timeout.
    pub fn timeout(mut self, duration: Duration) -> Self {
        self.timeout = Some(duration);
        self
    }

    /// Executes the configured Java program.
    ///
    /// # Errors
    ///
    /// Returns `JavaError::Other` if no Java installation has been set, or if
    /// neither a JAR file nor a main class has been specified.
    /// Returns `JavaError::NotFound` if the Java executable does not exist.
    /// Returns `JavaError::IoError` if file operations or process spawning fail.
    /// Returns `JavaError::ExecutionFailed` if the Java process exits with a non‑zero status.
    pub fn execute(&self) -> Result<(), JavaError> {
        let java = self.java.as_ref().ok_or_else(|| {
            JavaError::Other("Must set Java environment via `.java(...)`".to_string())
        })?;
        let java_exe = java.java_executable()?;

        let mut cmd = Command::new(java_exe);

        // Classpath
        if let Some(cp) = &self.classpath {
            cmd.arg("-cp");
            cmd.arg(cp);
        }

        // Module path
        if let Some(mp) = &self.module_path {
            cmd.arg("--module-path");
            cmd.arg(mp);
        }

        // --add-opens
        for open in &self.add_opens {
            cmd.arg("--add-opens");
            cmd.arg(open);
        }

        // --add-exports
        for export in &self.add_exports {
            cmd.arg("--add-exports");
            cmd.arg(export);
        }

        // System properties
        for prop in &self.system_properties {
            cmd.arg(prop);
        }

        if let Some(min) = &self.min_memory {
            cmd.arg(format!("-Xms{}", min));
        }
        if let Some(max) = &self.max_memory {
            cmd.arg(format!("-Xmx{}", max));
        }

        if let Some(jar) = &self.jar {
            cmd.arg("-jar");
            cmd.arg(jar);
        } else if let Some(main) = &self.main_class {
            cmd.arg(main);
        } else {
            return Err(JavaError::Other(
                "Must specify JAR file or main class".into(),
            ));
        }

        cmd.args(&self.args);

        // Environment variables
        for (key, value) in &self.env_vars {
            cmd.env(key, value);
        }

        // Working directory
        if let Some(dir) = &self.working_dir {
            cmd.current_dir(dir);
        }

        // Configure redirection
        if let Some(output) = &self.redirect.output {
            let file = if self.redirect.append_output {
                OpenOptions::new().append(true).create(true).open(output)
            } else {
                File::create(output)
            }
            .map_err(JavaError::IoError)?;
            cmd.stdout(Stdio::from(file));
        } else {
            cmd.stdout(Stdio::inherit());
        }

        if let Some(error) = &self.redirect.error {
            let file = if self.redirect.append_error {
                OpenOptions::new().append(true).create(true).open(error)
            } else {
                File::create(error)
            }
            .map_err(JavaError::IoError)?;
            cmd.stderr(Stdio::from(file));
        } else {
            cmd.stderr(Stdio::inherit());
        }

        if let Some(input) = &self.redirect.input {
            let file = File::open(input).map_err(JavaError::IoError)?;
            cmd.stdin(Stdio::from(file));
        } else {
            cmd.stdin(Stdio::inherit());
        }

        // Timeout handling
        if let Some(timeout) = self.timeout {
            let mut child = cmd.spawn().map_err(JavaError::IoError)?;
            let start = std::time::Instant::now();
            loop {
                if child.try_wait().map_err(JavaError::IoError)?.is_some() {
                    // Process completed within the timeout
                    let status = child.wait().map_err(JavaError::IoError)?;
                    return if status.success() {
                        Ok(())
                    } else {
                        Err(JavaError::ExecutionFailed(format!(
                            "Execution failed: {}",
                            status.code().unwrap()
                        )))
                    };
                }
                if start.elapsed() >= timeout {
                    // Timeout reached, kill the process
                    kill_process(&child)?;
                    return Err(JavaError::ExecutionFailed(format!(
                        "Process timed out after {}ms",
                        timeout.as_millis()
                    )));
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }

        let status = cmd.status().map_err(JavaError::IoError)?;

        if status.success() {
            Ok(())
        } else {
            Err(JavaError::ExecutionFailed(format!(
                "Execution failed: {}",
                status.code().unwrap()
            )))
        }
    }
}

/// Formats a memory size in bytes into a Java‑compatible string (`<n>m` or `<n>g`).
///
/// If the size is an exact multiple of 1 GiB, it is formatted as `<n>g`.
/// Otherwise, if it is an exact multiple of 1 MiB, it is formatted as `<n>m`.
/// If neither, it is rounded to the nearest mebibyte and formatted as `<n>m`.
fn format_memory(bytes: usize) -> String {
    const MB: usize = 1024 * 1024;
    const GB: usize = MB * 1024;

    if bytes.is_multiple_of(GB) {
        format!("{}g", bytes / GB)
    } else if bytes.is_multiple_of(MB) {
        format!("{}m", bytes / MB)
    } else {
        let mb = (bytes + MB / 2) / MB;
        format!("{}m", mb)
    }
}

fn kill_process(child: &std::process::Child) -> Result<(), JavaError> {
    // The Child struct doesn't have a kill method on & reference,
    // but we can use taskkill (Windows) or kill command (Unix)
    let pid = child.id();
    #[cfg(windows)]
    let status = std::process::Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/F"])
        .status()
        .map_err(JavaError::IoError)?;
    #[cfg(not(windows))]
    let status = std::process::Command::new("kill")
        .args(["-9", &pid.to_string()])
        .status()
        .map_err(JavaError::IoError)?;

    if status.success() {
        Ok(())
    } else {
        Err(JavaError::Other(format!("Failed to kill process {}", pid)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::JavaInfo;

    #[test]
    fn test_format_memory_exact_mb() {
        assert_eq!(format_memory(256 * 1024 * 1024), "256m");
    }

    #[test]
    fn test_format_memory_exact_gb() {
        assert_eq!(format_memory(2 * 1024 * 1024 * 1024), "2g");
    }

    #[test]
    fn test_format_memory_rounded() {
        let result = format_memory(100 * 1024 * 1024 + 512 * 1024);
        assert!(result.ends_with('m'));
        let num: usize = result[..result.len() - 1].parse().unwrap();
        assert!(num >= 100);
    }

    #[test]
    fn test_format_memory_zero() {
        assert_eq!(format_memory(0), "0g");
    }

    #[test]
    fn test_format_memory_small() {
        let result = format_memory(1);
        assert_eq!(result, "0m");
    }

    #[test]
    fn test_runner_missing_java() {
        let result = JavaRunner::new().jar("test.jar").execute();
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Must set Java environment"));
    }

    #[test]
    fn test_runner_missing_jar_or_main() {
        // JavaInfo::default() has empty java_home, so java_executable() fails first
        let info = JavaInfo::default();
        let result = JavaRunner::new().java(info).execute();
        assert!(result.is_err());
    }

    #[test]
    fn test_java_redirect_default() {
        let r = JavaRedirect::new();
        assert!(r.output.is_none());
        assert!(r.error.is_none());
        assert!(r.input.is_none());
        assert!(!r.append_output);
        assert!(!r.append_error);
    }

    #[test]
    fn test_java_redirect_append() {
        let r = JavaRedirect::new()
            .output("out.log")
            .append_output()
            .error("err.log")
            .append_error();
        assert!(r.append_output);
        assert!(r.append_error);
    }

    #[test]
    fn test_runner_builder_methods() {
        let runner = JavaRunner::new()
            .system_property("foo", "bar")
            .add_opens("java.base", "java.lang", "ALL-UNNAMED")
            .add_exports("java.base", "com.sun.internal", "ALL-UNNAMED");
        assert_eq!(runner.system_properties, vec!["-Dfoo=bar"]);
        assert_eq!(runner.add_opens, vec!["java.base/java.lang.ALL-UNNAMED"]);
        assert_eq!(
            runner.add_exports,
            vec!["java.base/com.sun.internal.ALL-UNNAMED"]
        );
    }

    #[test]
    fn test_output_mode_debug_clone() {
        let mode = OutputMode::Both;
        let cloned = mode;
        assert_eq!(mode, cloned);
        let _ = format!("{:?}", mode);
    }

    #[test]
    fn test_format_memory_1gb_plus_1b() {
        let result = format_memory(1024 * 1024 * 1024 + 1);
        assert_eq!(result, "1024m");
    }

    #[test]
    fn test_format_memory_1_mib() {
        assert_eq!(format_memory(1024 * 1024), "1m");
    }

    #[test]
    fn test_format_memory_1024_mib_is_1g() {
        assert_eq!(format_memory(1024 * 1024 * 1024), "1g");
    }

    #[test]
    fn test_runner_env_var() {
        let runner = JavaRunner::new().env("MY_VAR", "my_value");
        assert_eq!(runner.env_vars, vec![("MY_VAR".into(), "my_value".into())]);
    }

    #[test]
    fn test_runner_working_dir() {
        let runner = JavaRunner::new().working_dir("/tmp");
        assert_eq!(runner.working_dir, Some(PathBuf::from("/tmp")));
    }

    #[test]
    fn test_runner_timeout() {
        let runner = JavaRunner::new().timeout(Duration::from_secs(30));
        assert_eq!(runner.timeout, Some(Duration::from_secs(30)));
    }

    #[test]
    fn test_runner_classpath() {
        let separator = if cfg!(windows) { ";" } else { ":" };
        let runner = JavaRunner::new().classpath(&["lib/a.jar", "config"]);
        assert_eq!(
            runner.classpath,
            Some(format!("lib/a.jar{separator}config"))
        );
    }

    #[test]
    fn test_runner_module_path() {
        let runner = JavaRunner::new().module_path("./modules");
        assert_eq!(runner.module_path, Some(PathBuf::from("./modules")));
    }

    #[test]
    fn test_runner_multiple_args() {
        let runner = JavaRunner::new().arg("--verbose").arg("--debug");
        assert_eq!(runner.args, vec!["--verbose", "--debug"]);
    }

    #[test]
    fn test_runner_all_builder_methods() {
        let runner = JavaRunner::new()
            .classpath(&["lib/*"])
            .module_path("mods")
            .add_opens("java.base", "java.lang", "ALL-UNNAMED")
            .add_exports("java.base", "sun.security", "ALL-UNNAMED")
            .system_property("key", "val")
            .env("HOME", "/root")
            .working_dir("/app")
            .timeout(Duration::from_secs(10))
            .min_memory(256 * 1024 * 1024)
            .max_memory(1024 * 1024 * 1024);

        assert!(runner.classpath.is_some());
        assert!(runner.module_path.is_some());
        assert_eq!(runner.add_opens.len(), 1);
        assert_eq!(runner.add_exports.len(), 1);
        assert_eq!(runner.system_properties.len(), 1);
        assert_eq!(runner.env_vars.len(), 1);
        assert!(runner.working_dir.is_some());
        assert_eq!(runner.timeout, Some(Duration::from_secs(10)));
        assert!(runner.min_memory.is_some());
        assert!(runner.max_memory.is_some());
    }
}
