use anyhow::Result;
use colored_text::Colorize;

pub fn run() -> Result<()> {
    #[cfg(target_os = "linux")]
    return run_linux();

    #[cfg(not(target_os = "linux"))]
    return run_macos();
}

#[cfg(target_os = "linux")]
fn run_linux() -> Result<()> {
    println!("Fetching available PHP versions...");
    let minors = crate::downloader::fetch_available_minors()?;

    if minors.is_empty() {
        eprintln!("No versions found from static-php-cli");
        return Ok(());
    }

    let installed = crate::discover::discover_versions()?;

    for minor in &minors {
        let is_installed = installed.iter().any(|i| i.version.to_string() == *minor);
        let marker = if is_installed { "*" } else { " " };
        let display = if is_installed {
            minor.hex("#777BB3").bold().to_string()
        } else {
            minor.clone()
        };
        println!("{} {}", marker, display);
    }

    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn run_macos() -> Result<()> {
    eprintln!(
        "{}",
        "list-remote is only available on Linux. On macOS, use: brew search php".yellow()
    );
    Ok(())
}
