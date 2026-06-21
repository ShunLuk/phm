use crate::config;
use crate::discover;
use crate::multishell;
use crate::shim;
use anyhow::Result;
use colored_text::Colorize;

#[cfg(not(target_os = "macos"))]
use crate::termux;

pub fn run() -> Result<()> {
    #[cfg(not(target_os = "macos"))]
    return run_linux();

    #[cfg(target_os = "macos")]
    return run_macos();
}

#[cfg(not(target_os = "macos"))]
fn run_linux() -> Result<()> {
    let mut issues = 0;

    // Check: PHP versions found
    let installations = discover::discover_versions()?;
    if installations.is_empty() {
        println!("{} No PHP versions found", "✗".red());
        println!("  Install one with: phm install 8.3");
        println!("  Or via system package manager, then run: phm list");
        issues += 1;
    } else {
        println!(
            "{} {} PHP version(s) found: {}",
            "✓".hex("#777BB3"),
            installations.len(),
            installations
                .iter()
                .map(|i| i.version.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    // Check: managed PHP directory
    let managed = config::managed_php_dir()?;
    let managed_count = if managed.exists() {
        std::fs::read_dir(&managed)
            .map(|d| d.flatten().count())
            .unwrap_or(0)
    } else {
        0
    };
    println!(
        "{} phm-managed PHP dir: {} ({} version(s))",
        "✓".hex("#777BB3"),
        managed.display(),
        managed_count
    );

    // Check: default version set
    match config::get_default()? {
        Some(ver) => {
            if installations.iter().any(|i| i.version.to_string() == ver) {
                println!("{} Default version: {}", "✓".hex("#777BB3"), ver);
            } else {
                println!("{} Default version {} is not installed", "✗".red(), ver);
                issues += 1;
            }
        }
        None => {
            println!("{} No default version set", "✗".red());
            println!("  Set one with: phm default <version>");
            issues += 1;
        }
    }

    // Check: PHM_MULTISHELL_PATH set
    match std::env::var("PHM_MULTISHELL_PATH") {
        Ok(path) => {
            if std::path::Path::new(&path).exists() {
                println!("{} Shell integration active", "✓".hex("#777BB3"));
            } else {
                println!(
                    "{} PHM_MULTISHELL_PATH set but directory missing",
                    "✗".red()
                );
                issues += 1;
            }
        }
        Err(_) => {
            println!("{} Shell integration not loaded", "✗".red());
            println!("  Add to .zshrc/.bashrc: eval \"$(phm env --shell=zsh --use-on-cd)\"");
            issues += 1;
        }
    }

    // Check: composer available
    let composer_check = std::process::Command::new("which").arg("composer").output();
    match composer_check {
        Ok(output) if output.status.success() => {
            println!("{} Composer found", "✓".hex("#777BB3"));
        }
        _ => {
            println!("{} Composer not found", "!".yellow());
            println!("  Install via your package manager or: curl -sS https://getcomposer.org/installer | php");
        }
    }

    // Check: shim setup
    match shim::shim_bin_dir() {
        Ok(shim_bin) if shim_bin.join("php").is_symlink() => {
            let path = std::env::var("PATH").unwrap_or_default();
            println!("{} Shims active in {}", "✓".hex("#777BB3"), shim_bin.display());

            if !path.contains(&shim_bin.display().to_string()) {
                println!("{} Shim directory not in PATH", "!".yellow());
                println!(
                    "  Add to ~/.zshenv or ~/.profile: export PATH=\"{}:$PATH\"",
                    shim_bin.display()
                );
                issues += 1;
            } else {
                println!("{} Shim directory in PATH", "✓".hex("#777BB3"));
            }
        }
        _ => {
            println!(
                "{} No shims configured (recommended for non-interactive shells)",
                "!".yellow()
            );
            println!("  Run: phm shim create");
        }
    }

    // Check: Termux DNS config
    if termux::is_termux() {
        // $PREFIX/etc/resolv.conf — needed by Termux-native PHP
        match termux::resolv_conf_path() {
            Some(path) if path.exists() => {
                let has_nameserver = std::fs::read_to_string(&path)
                    .map(|c| c.contains("nameserver"))
                    .unwrap_or(false);
                if has_nameserver {
                    println!("{} Termux DNS config OK ({})", "✓".hex("#777BB3"), path.display());
                } else {
                    println!(
                        "{} {} exists but has no nameservers — Termux-native PHP DNS will fail",
                        "✗".red(),
                        path.display()
                    );
                    println!("  Fix: echo 'nameserver 8.8.8.8' > {}", path.display());
                    issues += 1;
                }
            }
            Some(path) => {
                println!(
                    "{} {} missing — Termux-native PHP DNS will fail",
                    "✗".red(),
                    path.display()
                );
                println!("  Fix: echo 'nameserver 8.8.8.8' > {}", path.display());
                issues += 1;
            }
            None => {}
        }

        // proot — needed by phm-managed static PHP (musl/c-ares reads /etc/resolv.conf)
        if termux::needs_proot_dns_wrap() {
            println!("{} proot available — static PHP DNS via bind-mount", "✓".hex("#777BB3"));
        } else if termux::proot_bin().is_none() {
            println!("{} proot not found — static PHP (phm-managed) DNS will fail", "✗".red());
            println!("  Fix: pkg install proot");
            issues += 1;
        }
    }

    // Check: stale multishell dirs
    let base = multishell::multishell_base()?;
    if base.exists() {
        let mut stale = 0;
        if let Ok(entries) = std::fs::read_dir(&base) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if let Some(pid_str) = name_str.split('_').next()
                    && let Ok(pid) = pid_str.parse::<i32>()
                    && !multishell::is_process_alive(pid)
                {
                    stale += 1;
                }
            }
        }
        if stale > 0 {
            println!(
                "{} {} stale multishell dir(s) (cleaned up on next shell init)",
                "!".yellow(),
                stale
            );
        } else {
            println!("{} No stale multishell directories", "✓".hex("#777BB3"));
        }
    }

    println!();
    if issues == 0 {
        println!("{}", "All checks passed!".hex("#777BB3").bold());
    } else {
        println!("{} issue(s) found", format!("{}", issues).red().bold());
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn run_macos() -> Result<()> {
    let mut issues = 0;
    let opt_dirs = discover::homebrew_opt_dirs();
    let detected_dirs = opt_dirs
        .iter()
        .filter(|path| path.exists())
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>();

    // Check: PHP versions found
    let installations = discover::discover_versions()?;
    if installations.is_empty() {
        println!("{} No PHP versions found in Homebrew", "✗".red());
        println!("  Install one with: brew install php@8.2");
        issues += 1;
    } else {
        println!(
            "{} {} PHP version(s) found: {}",
            "✓".hex("#777BB3"),
            installations.len(),
            installations
                .iter()
                .map(|i| i.version.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    if detected_dirs.is_empty() {
        println!("{} No Homebrew opt directory detected", "!".yellow());
        println!(
            "  Checked: {}",
            opt_dirs
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
    } else {
        println!(
            "{} Homebrew opt dirs: {}",
            "✓".hex("#777BB3"),
            detected_dirs.join(", ")
        );
    }

    // Check: default version set
    match config::get_default()? {
        Some(ver) => {
            if installations.iter().any(|i| i.version.to_string() == ver) {
                println!("{} Default version: {}", "✓".hex("#777BB3"), ver);
            } else {
                println!("{} Default version {} is not installed", "✗".red(), ver);
                issues += 1;
            }
        }
        None => {
            println!("{} No default version set", "✗".red());
            println!("  Set one with: phm default <version>");
            issues += 1;
        }
    }

    // Check: PHM_MULTISHELL_PATH set
    match std::env::var("PHM_MULTISHELL_PATH") {
        Ok(path) => {
            if std::path::Path::new(&path).exists() {
                println!("{} Shell integration active", "✓".hex("#777BB3"));
            } else {
                println!(
                    "{} PHM_MULTISHELL_PATH set but directory missing",
                    "✗".red()
                );
                issues += 1;
            }
        }
        Err(_) => {
            println!("{} Shell integration not loaded", "✗".red());
            println!("  Add to .zshrc: eval \"$(phm env --shell=zsh --use-on-cd)\"");
            issues += 1;
        }
    }

    // Check: Herd not conflicting
    let path = std::env::var("PATH").unwrap_or_default();
    if path.contains("Herd/bin") {
        println!(
            "{} Herd is still in PATH — may conflict with phm",
            "✗".red()
        );
        println!("  Remove from .zshrc: export PATH=\".../Herd/bin/:$PATH\"");
        issues += 1;
    } else {
        println!("{} No Herd conflict", "✓".hex("#777BB3"));
    }

    // Check: composer available
    let composer_check = std::process::Command::new("which").arg("composer").output();
    match composer_check {
        Ok(output) if output.status.success() => {
            println!("{} Composer found", "✓".hex("#777BB3"));
        }
        _ => {
            println!("{} Composer not found", "✗".red());
            println!("  Install with: brew install composer");
            issues += 1;
        }
    }

    // Check: shim setup
    match shim::shim_bin_dir() {
        Ok(shim_bin) if shim_bin.join("php").is_symlink() => {
            println!("{} Shims active in {}", "✓".hex("#777BB3"), shim_bin.display());

            if !path.contains(&shim_bin.display().to_string()) {
                println!("{} Shim directory not in PATH", "!".yellow());
                println!(
                    "  Add to ~/.zshenv: export PATH=\"{}:$PATH\"",
                    shim_bin.display()
                );
                issues += 1;
            } else {
                println!("{} Shim directory in PATH", "✓".hex("#777BB3"));
            }
        }
        _ => {
            println!(
                "{} No shims configured (recommended for non-interactive shells)",
                "!".yellow()
            );
            println!("  Run: phm shim create");
        }
    }

    // Check: stale multishell dirs
    let base = multishell::multishell_base()?;
    if base.exists() {
        let mut stale = 0;
        if let Ok(entries) = std::fs::read_dir(&base) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if let Some(pid_str) = name_str.split('_').next()
                    && let Ok(pid) = pid_str.parse::<i32>()
                    && !multishell::is_process_alive(pid)
                {
                    stale += 1;
                }
            }
        }
        if stale > 0 {
            println!(
                "{} {} stale multishell dir(s) (cleaned up on next shell init)",
                "!".yellow(),
                stale
            );
        } else {
            println!("{} No stale multishell directories", "✓".hex("#777BB3"));
        }
    }

    println!();
    if issues == 0 {
        println!("{}", "All checks passed!".hex("#777BB3").bold());
    } else {
        println!("{} issue(s) found", format!("{}", issues).red().bold());
    }

    Ok(())
}
