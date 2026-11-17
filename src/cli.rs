use clap::error::ErrorKind;
use clap::{Parser, Subcommand};
use serde::Serialize;

use crate::control;
use crate::error::{Error, Result};

#[derive(Parser)]
#[command(name = "recdr", version, about = "Screen recorder for AI agents")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List visible top-level windows
    Windows,
    /// List monitors
    Monitors,
    /// Run the MCP server on stdio
    Mcp,
}

/// Parses the command line, runs the command and prints its JSON. Returns the exit code.
pub fn run(serve_mcp: fn() -> Result<()>) -> i32 {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) if matches!(e.kind(), ErrorKind::DisplayHelp | ErrorKind::DisplayVersion) => {
            let _ = e.print();
            return 0;
        }
        Err(e) => return fail(&Error::Usage(e.to_string().trim().to_owned())),
    };
    // Some(json) is printed on stdout; None prints nothing.
    let outcome: Result<Option<String>> = match cli.command {
        Command::Windows => text(control::list_windows()),
        Command::Monitors => text(control::list_monitors()),
        Command::Mcp => serve_mcp().map(|()| None),
    };
    match outcome {
        Ok(Some(text)) => {
            println!("{text}");
            0
        }
        Ok(None) => 0,
        Err(e) => fail(&e),
    }
}

fn text<T: Serialize>(value: Result<T>) -> Result<Option<String>> {
    Ok(Some(serde_json::to_string(&value?)?))
}

fn fail(error: &Error) -> i32 {
    eprintln!("{}", error.to_json());
    error.exit_code()
}
