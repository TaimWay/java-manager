# Java Manager

A comprehensive Rust library and command-line tool for discovering, managing, and interacting with Java installations.

## Features

- **Cross-platform**: Works on Windows, macOS, and Linux/Unix
- **Comprehensive discovery**: Finds Java installations in common locations
- **Detailed information**: Extracts version, architecture, supplier information
- **Command execution**: Execute Java commands and capture output
- **Multiple installation management**: Manage and switch between different Java versions
- **File searching**: Find files within Java installations with wildcard support
- **Documentation location**: Locate Java documentation directories

## Installation

### As a Library

Add to your `Cargo.toml`:

```toml
[dependencies]
java-manager = "0.1"