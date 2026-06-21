//! CLI tool for discovering and inspecting Java installations.
//!
//! Usage:
//!   cjm info <SPEC>         Show matching Java installations
//!   cjm info <SPEC> -i      Interactive selection
//!   cjm info <SPEC> -n <N>  Select the N-th result (0-indexed)

use clap::Parser;
use java_manager::{JavaInfo, JavaSpec, full_search};
use std::process;

#[derive(Parser)]
#[command(name = "cjm", version, about = "Java installation manager")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Parser)]
enum Command {
    /// Show detailed info for Java installations matching a spec.
    Info {
        /// Human-readable Java spec (e.g. "Eclipse Adoptium v^17.0.0 ax86_64")
        spec: String,

        /// Enable interactive selection when multiple results match.
        #[arg(short = 'i', long)]
        interactive: bool,

        /// Auto-select the N-th result (0-indexed) without prompting.
        #[arg(short = 'n', long)]
        index: Option<usize>,
    },
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Command::Info {
            spec,
            interactive,
            index,
        } => cmd_info(&spec, interactive, index),
    }
}

fn cmd_info(spec_str: &str, interactive: bool, index: Option<usize>) {
    let spec = match JavaSpec::parse(spec_str) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error: failed to parse spec: {e}");
            process::exit(1);
        }
    };

    let all_javas = match full_search() {
        Ok(j) => j,
        Err(e) => {
            eprintln!("Error: search failed: {e}");
            process::exit(1);
        }
    };

    let matches: Vec<&JavaInfo> = all_javas.iter().filter(|j| spec.matches(j)).collect();

    if matches.is_empty() {
        println!("No Java installations match: {spec_str}");
        process::exit(1);
    }

    if let Some(idx) = index {
        if idx >= matches.len() {
            eprintln!("Error: index {idx} out of range (0..{})", matches.len() - 1);
            process::exit(1);
        }
        print_java_info(matches[idx]);
        return;
    }

    if interactive && matches.len() > 1 {
        let selection = dialoguer::Select::new()
            .with_prompt("Multiple Java installations found. Select one:")
            .items(
                &matches
                    .iter()
                    .map(|j| {
                        format!(
                            "{} — {} [{}] ({})",
                            j.name,
                            j.version,
                            j.architecture,
                            j.java_home.display()
                        )
                    })
                    .collect::<Vec<_>>(),
            )
            .default(0)
            .interact();

        match selection {
            Ok(idx) => {
                print_java_info(matches[idx]);
                return;
            }
            Err(_) => {
                eprintln!("Selection cancelled.");
                process::exit(1);
            }
        }
    }

    // Default: show all
    for (i, java) in matches.iter().enumerate() {
        println!(
            "[{i}] {} — {} [{}]",
            java.name, java.version, java.architecture
        );
        println!("    JAVA_HOME: {}", java.java_home.display());
    }
}

fn print_java_info(java: &JavaInfo) {
    println!("Java Installation");
    println!("  Name:       {}", java.name);
    println!("  Version:    {}", java.version);
    if let Some(ref v) = java.parsed_version {
        println!("  Parsed:     {v}");
    }
    println!("  Vendor:     {}", java.vendor);
    println!("  Architecture: {}", java.architecture);
    println!("  JAVA_HOME:  {}", java.java_home.display());
    println!("  Path:       {}", java.path.display());
    println!("  Is JDK:     {}", java.is_jdk());
    let caps = java.capabilities();
    if !caps.is_empty() {
        println!("  Tools:      {}", caps.join(", "));
    }
}
