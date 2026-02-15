use java_manager::{JavaInfo, deep_search, quick_search};
use std::io::{self, Write};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let installations: Vec<JavaInfo>;

    let mut input = String::new();
    loop {
        print!("Do you want to search for Java installations using quick or deep search? (q/d) ");
        io::stdout().flush().unwrap();
        input.clear();
        io::stdin().read_line(&mut input)?;
        match input.to_lowercase().as_str().trim() {
            "q" => installations = quick_search()?,
            "d" => installations = deep_search()?,
            _ => {
                println!("Invalid input. Please enter 'q' or 'd'.");
                continue;
            }
        }
        break;
    }

    println!("Found {} Java installations:", installations.len());
    println!("{}", "=".repeat(50));

    for (i, java) in installations.iter().enumerate() {
        println!("{}. {}", i + 1, java.name);
        println!("\tPath: {}", java.path.display());
        println!("\tVendor: {}", java.vendor);
        println!("\tArchitecture: {}", java.architecture);
        println!("\tJava Home: {}", java.java_home.display());
        println!("{}", "-".repeat(40));
    }

    Ok(())
}