use crate::{JavaError, JavaInfo};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutputMode {
    Both,
    OutputOnly,
    ErrorOnly,
}

impl JavaInfo {
    pub fn execute(&self, args: &str) -> Result<(), JavaError> {
        self.run_java(args, OutputMode::Both)
    }

    pub fn execute_with_error(&self, args: &str) -> Result<(), JavaError> {
        self.run_java(args, OutputMode::ErrorOnly)
    }

    pub fn execute_with_output(&self, args: &str) -> Result<(), JavaError> {
        self.run_java(args, OutputMode::OutputOnly)
    }

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
                for line in reader.lines() {
                    if let Ok(line) = line {
                        println!("{}", line);
                    }
                }
            }))
        } else {
            None
        };

        let stderr_handle = if matches!(mode, OutputMode::Both | OutputMode::ErrorOnly) {
            Some(thread::spawn(move || {
                let reader = BufReader::new(stderr);
                for line in reader.lines() {
                    if let Ok(line) = line {
                        eprintln!("{}", line);
                    }
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
            Err(JavaError::ExecutionFailed(format!("Execution failed: {}", status.code().unwrap())))
        }
    }

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

#[derive(Debug, Default)]
pub struct JavaRunner {
    java: Option<JavaInfo>,
    jar: Option<PathBuf>,
    min_memory: Option<String>,
    max_memory: Option<String>,
    main_class: Option<String>,
    args: Vec<String>,
    redirect: JavaRedirect,
}

#[derive(Debug, Default)]
pub struct JavaRedirect {
    output: Option<PathBuf>,
    error: Option<PathBuf>,
    input: Option<PathBuf>,
}

impl JavaRedirect {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn output(mut self, path: impl AsRef<Path>) -> Self {
        self.output = Some(path.as_ref().to_path_buf());
        self
    }

    pub fn error(mut self, path: impl AsRef<Path>) -> Self {
        self.error = Some(path.as_ref().to_path_buf());
        self
    }

    pub fn input(mut self, path: impl AsRef<Path>) -> Self {
        self.input = Some(path.as_ref().to_path_buf());
        self
    }
}

impl JavaRunner {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn java(mut self, java: JavaInfo) -> Self {
        self.java = Some(java);
        self
    }

    pub fn jar(mut self, jar: impl AsRef<Path>) -> Self {
        self.jar = Some(jar.as_ref().to_path_buf());
        self
    }

    pub fn min_memory(mut self, bytes: usize) -> Self {
        self.min_memory = Some(format_memory(bytes));
        self
    }

    pub fn max_memory(mut self, bytes: usize) -> Self {
        self.max_memory = Some(format_memory(bytes));
        self
    }

    pub fn main_class(mut self, class: impl Into<String>) -> Self {
        self.main_class = Some(class.into());
        self
    }

    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    pub fn redirect(mut self, redirect: JavaRedirect) -> Self {
        self.redirect = redirect;
        self
    }

    pub fn execute(self) -> Result<(), JavaError> {
        let java = self.java.ok_or_else(|| {
            JavaError::Other("Must set Java environment via `.java(...)`".to_string())
        })?;
        let java_exe = java.java_executable()?;

        let mut cmd = Command::new(java_exe);

        if let Some(min) = &self.min_memory {
            cmd.arg(format!("-Xms{}", min));
        }
        if let Some(max) = &self.max_memory {
            cmd.arg(format!("-Xmx{}", max));
        }

        if let Some(jar) = self.jar {
            cmd.arg("-jar");
            cmd.arg(jar);
        } else if let Some(main) = self.main_class {
            cmd.arg(main);
        } else {
            return Err(JavaError::Other("Must specify JAR file or main class".into()));
        }

        cmd.args(&self.args);

        // Configure redirection
        if let Some(output) = self.redirect.output {
            let file = File::create(output).map_err(JavaError::IoError)?;
            cmd.stdout(Stdio::from(file));
        } else {
            cmd.stdout(Stdio::inherit());
        }

        if let Some(error) = self.redirect.error {
            let file = File::create(error).map_err(JavaError::IoError)?;
            cmd.stderr(Stdio::from(file));
        } else {
            cmd.stderr(Stdio::inherit());
        }

        if let Some(input) = self.redirect.input {
            let file = File::open(input).map_err(JavaError::IoError)?;
            cmd.stdin(Stdio::from(file));
        } else {
            cmd.stdin(Stdio::inherit());
        }

        let status = cmd.status().map_err(JavaError::IoError)?;

        if status.success() {
            Ok(())
        } else {
            Err(JavaError::ExecutionFailed(format!("Execution failed: {}", status.code().unwrap())))
        }
    }
}

fn format_memory(bytes: usize) -> String {
    const MB: usize = 1024 * 1024;
    const GB: usize = MB * 1024;

    if bytes % GB == 0 {
        format!("{}g", bytes / GB)
    } else if bytes % MB == 0 {
        format!("{}m", bytes / MB)
    } else {
        let mb = (bytes + MB / 2) / MB;
        format!("{}m", mb)
    }
}