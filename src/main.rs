use java_manager;

fn main() -> java_manager::Result<()> {
    // Get detailed information about the default Java installation
    let java_info = java_manager::get_local_java_home()?;
    println!("Default Java: {}", java_info);

    // Find all Java installations on the system
    let installations = java_manager::find_all_java_installations()?;
    println!("Found {} Java installations", installations.len());

    // Execute a Java command
    let output = java_info.execute_with_output(&["-version"])?;
    println!("Java version output:\n{}", output);

    Ok(())
}