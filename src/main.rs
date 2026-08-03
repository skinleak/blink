mod app;
mod capture;
mod cli;
mod error;
mod notification;
mod save;
mod tray;

use clap::Parser;

use crate::{cli::Cli, error::Result};

#[tokio::main(flavor = "current_thread")]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("Blink: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    match Cli::parse().command {
        Some(command) => match app::capture(command.into(), false).await {
            Err(error) if error.is_cancelled() => Ok(()),
            result => result.map(|_| ()),
        },
        None => tray::run().await,
    }
}
