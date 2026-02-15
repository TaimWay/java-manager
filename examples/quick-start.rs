use java_manager::{quick_search};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    println!("Discovering Java installations...");

    let installations = quick_search()?;

    println!("Found {} Java installation(s):", installations.len());
    println!("{}", "=".repeat(50));

    for (i, java) in installations.iter().enumerate() {
        println!("{}.\tname: {}", i + 1, java.name);
        println!("\tPath: {}", java.path.as_path().display());
        println!("\tVendor: {}", java.vendor);
        println!("\tArchitecture: {}", java.architecture);
        println!("\tJava Home: {}", java.java_home.as_path().display());
        println!("{}", "-".repeat(40));
    }

    Ok(())
}
