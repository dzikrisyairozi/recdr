use std::path::PathBuf;

use clap::error::ErrorKind;
use clap::{Args, Parser, Subcommand};
use serde::Serialize;

use crate::control;
use crate::error::{Error, Result};
use crate::request::RecordRequest;

#[derive(Parser)]
#[command(name = "recdr", version, about = "Screen recorder for AI agents")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

// Parsed once per process, so the size of the largest variant does not matter.
#[allow(clippy::large_enum_variant)]
#[derive(Subcommand)]
enum Command {
    /// List visible top-level windows
    Windows,
    /// List monitors
    Monitors,
    /// Record for a fixed time, then print the result
    Record(RecordCmd),
    /// Run the MCP server on stdio
    Mcp,
}

/// Exactly one of these picks what to record.
#[derive(Args)]
struct TargetFlags {
    /// Window id from `recdr windows`
    #[arg(long)]
    window_id: Option<u64>,
    /// Window whose title contains this text, ignoring case
    #[arg(long)]
    window: Option<String>,
    /// Window owned by a process with this executable name, e.g. chrome.exe
    #[arg(long)]
    process: Option<String>,
    /// Rectangle x,y,width,height in physical pixels, inside one monitor
    #[arg(long, allow_hyphen_values = true)]
    region: Option<String>,
    /// Monitor index from `recdr monitors`, starting at 0
    #[arg(long)]
    monitor: Option<usize>,
}

#[derive(Args)]
struct RecordFlags {
    #[command(flatten)]
    target: TargetFlags,
    /// Capture rate limit, 1 to 60 [default: 15]
    #[arg(long)]
    fps: Option<u32>,
    /// The recording stops itself at this length [default: 5m]
    #[arg(long)]
    max_duration: Option<String>,
    /// The recording stops itself at this directory size [default: 2GB]
    #[arg(long)]
    max_size: Option<String>,
    /// gif, mp4, or gif,mp4 [default: gif]
    #[arg(long)]
    format: Option<String>,
    /// Leave the mouse cursor out of the frames
    #[arg(long)]
    no_cursor: bool,
    /// Do not record clicks or draw click rings
    #[arg(long)]
    no_click_highlight: bool,
    /// Output root [default: .recdr]
    #[arg(long)]
    out: Option<PathBuf>,
}

#[derive(Args)]
struct RecordCmd {
    #[command(flatten)]
    flags: RecordFlags,
    /// Stop after this long, e.g. 10s
    #[arg(long)]
    duration: Option<String>,
}

impl RecordFlags {
    fn into_request(self) -> RecordRequest {
        RecordRequest {
            window_id: self.target.window_id,
            window: self.target.window,
            process: self.target.process,
            region: self.target.region,
            monitor: self.target.monitor,
            fps: self.fps,
            max_duration: self.max_duration,
            max_size: self.max_size,
            format: self.format,
            no_cursor: self.no_cursor,
            no_click_highlight: self.no_click_highlight,
            out: self.out,
            duration: None,
            command: None,
        }
    }
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
        Command::Record(cmd) => {
            let request = RecordRequest {
                duration: cmd.duration,
                ..cmd.flags.into_request()
            };
            text(request.into_options().and_then(control::record))
        }
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
