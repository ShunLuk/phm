use crate::ShimAction;
use crate::shim;
use anyhow::Result;
use colored_text::Colorize;

pub fn run(action: ShimAction) -> Result<()> {
    match action {
        ShimAction::Create => {
            let path = shim::create_shims()?;
            println!(
                "Shims created in {}",
                path.display().to_string().hex("#777BB3").bold()
            );

            match shim::inject_zshenv() {
                Ok(true) => println!(
                    "Added shim PATH to {}",
                    "~/.zshenv".bold()
                ),
                Ok(false) => println!(
                    "Shim PATH already in {}",
                    "~/.zshenv".bold()
                ),
                Err(e) => {
                    eprintln!("phm: warning: could not update ~/.zshenv: {}", e);
                    println!(
                        "Add manually to {}:",
                        "~/.zshenv".bold()
                    );
                    println!(
                        "  export PATH=\"{}:$PATH\"",
                        path.display()
                    );
                }
            }
        }
        ShimAction::Path => {
            let path = shim::shim_bin_dir()?;
            println!("{}", path.display());
        }
        ShimAction::Remove => {
            shim::remove_shims()?;
            if shim::remove_zshenv()? {
                println!("Shims removed and cleaned up {}", "~/.zshenv".bold());
            } else {
                println!("Shims removed");
            }
        }
    }
    Ok(())
}
