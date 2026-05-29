use crate::multishell::PHP_BINARIES;
use crate::{composer, config, discover};
use crate::version::{PhpVersion, VersionConstraint};
use anyhow::{Context, Result, bail};
use std::path::PathBuf;

pub fn shim_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().context("could not determine home directory")?;
    Ok(home.join(".local/state/phm/shims"))
}

pub fn shim_bin_dir() -> Result<PathBuf> {
    Ok(shim_dir()?.join("bin"))
}

pub fn create_shims() -> Result<PathBuf> {
    let bin_dir = shim_bin_dir()?;
    std::fs::create_dir_all(&bin_dir)?;

    let phm_binary = find_phm_binary()?;

    for binary in PHP_BINARIES {
        let shim_path = bin_dir.join(binary);
        let _ = std::fs::remove_file(&shim_path);
        std::os::unix::fs::symlink(&phm_binary, &shim_path)
            .with_context(|| format!("failed to create shim: {}", shim_path.display()))?;
    }

    Ok(bin_dir)
}

pub fn ensure_shims() -> Result<()> {
    let bin_dir = shim_bin_dir()?;
    let php_shim = bin_dir.join("php");

    if php_shim.is_symlink() {
        if let Ok(target) = std::fs::read_link(&php_shim) {
            if let Ok(our_binary) = find_phm_binary() {
                if target == our_binary {
                    return Ok(());
                }
            }
        }
    }

    create_shims()?;
    Ok(())
}

pub fn remove_shims() -> Result<()> {
    let bin_dir = shim_bin_dir()?;
    if bin_dir.exists() {
        std::fs::remove_dir_all(&bin_dir)?;
    }
    Ok(())
}

pub fn exec_shim(binary_name: &str, args: &[String]) -> Result<()> {
    let cwd = std::env::current_dir()?;

    let constraint = match composer::find_version(&cwd)? {
        Some(c) => c,
        None => match config::get_default()? {
            Some(ver_str) => {
                let v = PhpVersion::parse(&ver_str)
                    .ok_or_else(|| anyhow::anyhow!("invalid default version: {}", ver_str))?;
                VersionConstraint::exact(v)
            }
            None => bail!(
                "no PHP version detected for {}.\n\
                 Add .php-version, composer.json require.php, or run: phm default <version>",
                cwd.display()
            ),
        },
    };

    let installations = discover::discover_versions()?;
    let versions: Vec<PhpVersion> = installations.iter().map(|i| i.version).collect();
    let resolved = constraint
        .resolve(&versions)
        .ok_or_else(|| anyhow::anyhow!("no installed PHP satisfies constraint for {}", constraint.target()))?;

    let installation = installations.iter().find(|i| i.version == resolved).unwrap();
    let real_binary = installation.bin_dir.join(binary_name);

    if !real_binary.exists() {
        bail!(
            "'{}' not found in PHP {} ({})",
            binary_name,
            resolved,
            installation.bin_dir.display()
        );
    }

    use std::os::unix::process::CommandExt;
    let err = std::process::Command::new(&real_binary)
        .args(args)
        .exec();

    Err(err.into())
}

const ZSHENV_MARKER: &str = "# phm shims";

pub fn inject_zshenv() -> Result<bool> {
    let home = dirs::home_dir().context("could not determine home directory")?;
    let zshenv = home.join(".zshenv");
    let bin_dir = shim_bin_dir()?;
    let line = format!("export PATH=\"{}:$PATH\" {}", bin_dir.display(), ZSHENV_MARKER);

    if let Ok(content) = std::fs::read_to_string(&zshenv) {
        if content.contains(ZSHENV_MARKER) {
            return Ok(false);
        }
    }

    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&zshenv)?;

    writeln!(file)?;
    writeln!(file, "{}", line)?;

    Ok(true)
}

pub fn remove_zshenv() -> Result<bool> {
    let home = dirs::home_dir().context("could not determine home directory")?;
    let zshenv = home.join(".zshenv");

    if !zshenv.exists() {
        return Ok(false);
    }

    let content = std::fs::read_to_string(&zshenv)?;
    if !content.contains(ZSHENV_MARKER) {
        return Ok(false);
    }

    let filtered: Vec<&str> = content
        .lines()
        .filter(|line| !line.contains(ZSHENV_MARKER))
        .collect();
    std::fs::write(&zshenv, filtered.join("\n") + "\n")?;

    Ok(true)
}

fn find_phm_binary() -> Result<PathBuf> {
    if let Ok(output) = std::process::Command::new("which")
        .arg("phm")
        .output()
    {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Ok(PathBuf::from(path));
            }
        }
    }

    std::env::current_exe().context("could not determine phm binary path")
}
