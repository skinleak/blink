mod app;
mod capture;
mod cli;
mod error;
mod notification;
mod save;
mod settings;
mod tray;
mod ui;

use clap::Parser;

use crate::{cli::Cli, error::Result};

fn main() {
    if let Err(error) = run() {
        eprintln!("Blink: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    match Cli::parse().command {
        Some(command) => {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(error::BlinkError::Runtime)?;
            match runtime.block_on(app::capture(command.into(), false)) {
                Err(error) if error.is_cancelled() => Ok(()),
                result => result.map(|_| ()),
            }
        }
        None => ui::run(),
    }
}
