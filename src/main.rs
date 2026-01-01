#![windows_subsystem = "windows"]

mod logic;
mod ui;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // Initial setup (root check on linux)
    #[cfg(unix)]
    if is_running_as_root() {
        eprintln!("Error: This application should not be run as root or using sudo.");
        eprintln!("The program will request sudo permissions when needed.");
        eprintln!("Please run without sudo.");
        std::process::exit(1);
    }

    // Launch UI
    ui::run_app().await?;

    Ok(())
}

#[cfg(unix)]
fn is_running_as_root() -> bool {
    unsafe { libc::geteuid() == 0 }
}
