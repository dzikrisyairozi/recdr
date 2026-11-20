use std::path::PathBuf;

use clap::error::ErrorKind;
use clap::{Args, Parser, Subcommand};
use serde::Serialize;

use crate::control;
use crate::error::{Error, Result};
use crate::request::{self, RecordRequest};

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
    /// List the tabs of a browser
    Tabs(TabsCmd),
    /// Record for a fixed time or while a command runs, then print the result
    Record(RecordCmd),
    /// Start recording in the background and print its id
    Start(StartCmd),
    /// Stop a background recording and print the result
    Stop(IdCmd),
    /// Label the current moment of a background recording
    Mark(MarkCmd),
    /// Show one recording, or every recording under the output root
    Status(StatusCmd),
    /// Print paths of stored frames of a finished recording
    Frames(FramesCmd),
    /// Run the MCP server on stdio
    Mcp,
    /// The background recorder that `start` spawns
    #[command(name = "_recorder", hide = true)]
    Recorder { dir: PathBuf },
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
    /// Browser to attach to: a port, an http:// or ws:// URL, or chrome
    #[arg(long)]
    cdp: Option<String>,
    /// Tab id from `recdr tabs`
    #[arg(long)]
    tab_id: Option<String>,
    /// Tab whose title or URL contains this text, ignoring case
    #[arg(long)]
    tab: Option<String>,
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
    /// Record while this command runs, e.g. -- npm test
    #[arg(last = true)]
    command: Vec<String>,
}

#[derive(Args)]
struct StartCmd {
    #[command(flatten)]
    flags: RecordFlags,
}

#[derive(Args)]
struct TabsCmd {
    /// Browser to attach to: a port, an http:// or ws:// URL, or chrome
    #[arg(long)]
    cdp: String,
}

#[derive(Args)]
struct IdCmd {
    /// Recording id
    id: String,
    /// Output root [default: .recdr]
    #[arg(long)]
    out: Option<PathBuf>,
}

#[derive(Args)]
struct MarkCmd {
    /// Recording id
    id: String,
    /// Text shown under the frame on the contact sheet
    label: String,
    /// Output root [default: .recdr]
    #[arg(long)]
    out: Option<PathBuf>,
}

#[derive(Args)]
struct StatusCmd {
    /// Recording id; every recording when left out
    id: Option<String>,
    /// Output root [default: .recdr]
    #[arg(long)]
    out: Option<PathBuf>,
}

#[derive(Args)]
struct FramesCmd {
    /// Recording id
    id: String,
    /// Times such as 500ms,1s; each gives the frame on screen then
    #[arg(long, value_delimiter = ',')]
    at: Vec<String>,
    /// One frame per interval, e.g. 500ms
    #[arg(long)]
    every: Option<String>,
    /// The frames where the screen changed
    #[arg(long)]
    keyframes: bool,
    /// Scale wider frames down to this width into samples/
    #[arg(long)]
    max_width: Option<u32>,
    /// Output root [default: .recdr]
    #[arg(long)]
    out: Option<PathBuf>,
}

impl RecordFlags {
    fn into_request(self) -> RecordRequest {
        RecordRequest {
            window_id: self.target.window_id,
            window: self.target.window,
            process: self.target.process,
            region: self.target.region,
            monitor: self.target.monitor,
            cdp: self.target.cdp,
            tab_id: self.target.tab_id,
            tab: self.target.tab,
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
        Command::Tabs(cmd) => text(
            request::parse_endpoint(&cmd.cdp).and_then(|endpoint| control::list_tabs(&endpoint)),
        ),
        Command::Record(cmd) => {
            let request = RecordRequest {
                duration: cmd.duration,
                command: (!cmd.command.is_empty()).then_some(cmd.command),
                ..cmd.flags.into_request()
            };
            text(request.into_options().and_then(control::record))
        }
        Command::Start(cmd) => text(
            cmd.flags
                .into_request()
                .into_options()
                .and_then(control::start),
        ),
        Command::Stop(cmd) => {
            text(request::resolve_out(cmd.out).and_then(|out| control::stop(&out, &cmd.id)))
        }
        Command::Mark(cmd) => text(
            request::resolve_out(cmd.out).and_then(|out| control::mark(&out, &cmd.id, &cmd.label)),
        ),
        Command::Status(cmd) => text(
            request::resolve_out(cmd.out).and_then(|out| control::status(&out, cmd.id.as_deref())),
        ),
        // The recorder reports its own failure into the log; JSON there would bury it.
        Command::Recorder { dir } => return i32::from(control::run_recorder(dir).is_err()),
        Command::Frames(cmd) => text(
            request::parse_selection(&cmd.at, cmd.every.as_deref(), cmd.keyframes).and_then(
                |selection| {
                    let out = request::resolve_out(cmd.out)?;
                    control::frames(&out, &cmd.id, &selection, cmd.max_width, None)
                },
            ),
        ),
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
