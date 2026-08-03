use clap::{Parser, Subcommand};

use crate::capture::CaptureMode;

#[derive(Debug, Parser)]
#[command(
    name = "blink",
    version,
    about = "A fast, minimal Linux screenshot utility"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Clone, Copy, Debug, Subcommand)]
pub enum Command {
    Area,
    Screen,
    Window,
}

impl From<Command> for CaptureMode {
    fn from(command: Command) -> Self {
        match command {
            Command::Area => Self::Area,
            Command::Screen => Self::Screen,
            Command::Window => Self::Window,
        }
    }
}
