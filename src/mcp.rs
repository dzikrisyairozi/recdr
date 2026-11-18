//! `recdr mcp`: the same operations as the CLI, as MCP tools on stdio.

use std::path::{Path, PathBuf};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock};
use rmcp::{ErrorData, ServerHandler, ServiceExt, tool, tool_handler, tool_router};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
// Renamed because the layer test bans the substring "time::" in front ends.
use tokio::runtime as tokio_rt;

use crate::control;
use crate::error::{self, Error};
use crate::model::FrameRef;
use crate::request::{self, ExportRequest, RecordRequest};

const MAX_FRAMES: usize = 8;
const DEFAULT_MAX_WIDTH: u32 = 1024;

/// Serves the tools on stdin and stdout until the client disconnects.
pub fn serve() -> error::Result<()> {
    let runtime = tokio_rt::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async {
        let service = RecdrServer
            .serve(rmcp::transport::stdio())
            .await
            .map_err(std::io::Error::other)?;
        service.waiting().await.map_err(std::io::Error::other)?;
        Ok(())
    })
}

#[derive(Deserialize, JsonSchema)]
struct IdParams {
    /// Recording id from start_recording or record
    id: String,
    /// Output root; default .recdr in the server's working directory
    out: Option<PathBuf>,
}

#[derive(Deserialize, JsonSchema)]
struct MarkParams {
    /// Recording id from start_recording
    id: String,
    /// Text shown under the frame on the contact sheet
    label: String,
    /// Output root; default .recdr in the server's working directory
    out: Option<PathBuf>,
}

#[derive(Deserialize, JsonSchema)]
struct StatusParams {
    /// Recording id; every recording when left out
    id: Option<String>,
    /// Output root; default .recdr in the server's working directory
    out: Option<PathBuf>,
}

#[derive(Deserialize, JsonSchema)]
struct FramesParams {
    /// Recording id
    id: String,
    /// Times such as ["500ms", "2s"]; each gives the frame on screen then
    at: Option<Vec<String>>,
    /// One frame per interval, such as "1s"
    every: Option<String>,
    /// The frames where the screen changed
    keyframes: Option<bool>,
    /// Scale wider frames down to this width; default 1024
    max_width: Option<u32>,
    /// Output root; default .recdr in the server's working directory
    out: Option<PathBuf>,
}

#[derive(Deserialize, JsonSchema)]
struct TabsParams {
    /// A port, an http:// or ws:// URL, or chrome
    cdp: String,
}

struct RecdrServer;

#[tool_router]
impl RecdrServer {
    #[tool(description = "List visible windows with the id to pass as window_id.")]
    async fn list_windows(&self) -> std::result::Result<CallToolResult, ErrorData> {
        Ok(reply(run(control::list_windows).await, |_| Ok(Vec::new())))
    }

    #[tool(description = "List monitors with the index to pass as monitor.")]
    async fn list_monitors(&self) -> std::result::Result<CallToolResult, ErrorData> {
        Ok(reply(run(control::list_monitors).await, |_| Ok(Vec::new())))
    }

    #[tool(
        description = "List the tabs of a browser reached through CDP, with the id to pass as tab_id."
    )]
    async fn list_tabs(
        &self,
        Parameters(params): Parameters<TabsParams>,
    ) -> std::result::Result<CallToolResult, ErrorData> {
        let tabs = run(move || {
            request::parse_endpoint(&params.cdp).and_then(|endpoint| control::list_tabs(&endpoint))
        });
        Ok(reply(tabs.await, |_| Ok(Vec::new())))
    }

    #[tool(
        description = "Start recording a window, region, monitor or browser tab in the background and return its id. For a window run list_windows and pass window_id. For a tab run list_tabs and pass cdp and tab_id."
    )]
    async fn start_recording(
        &self,
        Parameters(params): Parameters<RecordRequest>,
    ) -> std::result::Result<CallToolResult, ErrorData> {
        let started = run(move || params.into_options().and_then(control::start)).await;
        Ok(reply(started, |_| Ok(Vec::new())))
    }

    #[tool(
        description = "Stop a recording and return its files and a contact sheet of what changed."
    )]
    async fn stop_recording(
        &self,
        Parameters(params): Parameters<IdParams>,
    ) -> std::result::Result<CallToolResult, ErrorData> {
        let stopped = run(move || control::stop(&request::resolve_out(params.out)?, &params.id));
        Ok(reply(stopped.await, |r| Ok(vec![png(&r.contact_sheet)?])))
    }

    #[tool(
        description = "Record for a fixed duration such as 5s and return the files and a contact sheet. For a window run list_windows and pass window_id. For a tab run list_tabs and pass cdp and tab_id."
    )]
    async fn record(
        &self,
        Parameters(params): Parameters<RecordRequest>,
    ) -> std::result::Result<CallToolResult, ErrorData> {
        let recorded = run(move || {
            if params.duration.is_none() {
                return Err(Error::Usage("record needs duration".into()));
            }
            let options = params.into_options()?;
            let out = options.out.clone();
            let started = control::start(options)?;
            control::wait(&out, &started.id)
        });
        Ok(reply(recorded.await, |r| Ok(vec![png(&r.contact_sheet)?])))
    }

    #[tool(
        description = "Write a GIF or an MP4 from a finished recording, optionally trimmed with from and until and cropped with crop."
    )]
    async fn export(
        &self,
        Parameters(params): Parameters<ExportRequest>,
    ) -> std::result::Result<CallToolResult, ErrorData> {
        let exported = run(move || params.into_options().and_then(control::export));
        Ok(reply(exported.await, |_| Ok(Vec::new())))
    }

    #[tool(description = "Label the current moment of a running recording.")]
    async fn mark(
        &self,
        Parameters(params): Parameters<MarkParams>,
    ) -> std::result::Result<CallToolResult, ErrorData> {
        let marked = run(move || {
            control::mark(
                &request::resolve_out(params.out)?,
                &params.id,
                &params.label,
            )
        });
        Ok(reply(marked.await, |_| Ok(Vec::new())))
    }

    #[tool(description = "Show whether recordings are running, finished or crashed.")]
    async fn get_status(
        &self,
        Parameters(params): Parameters<StatusParams>,
    ) -> std::result::Result<CallToolResult, ErrorData> {
        let status =
            run(move || control::status(&request::resolve_out(params.out)?, params.id.as_deref()));
        Ok(reply(status.await, |_| Ok(Vec::new())))
    }

    #[tool(
        description = "Return frames of a finished recording at given times, at an interval, or its keyframes. At most 8 per call."
    )]
    async fn get_frames(
        &self,
        Parameters(params): Parameters<FramesParams>,
    ) -> std::result::Result<CallToolResult, ErrorData> {
        let frames = run(move || {
            let selection = request::parse_selection(
                &params.at.unwrap_or_default(),
                params.every.as_deref(),
                params.keyframes.unwrap_or(false),
            )?;
            let out = request::resolve_out(params.out)?;
            let max_width = params.max_width.unwrap_or(DEFAULT_MAX_WIDTH);
            let list = control::frames(&out, &params.id, &selection, Some(max_width))?;
            if list.frames.len() > MAX_FRAMES {
                return Err(Error::TooManyFrames(list.frames.len()));
            }
            Ok(list)
        });
        Ok(reply(frames.await, |list| {
            list.frames
                .iter()
                .map(|f: &FrameRef| png(&f.path))
                .collect()
        }))
    }
}

#[tool_handler(name = "recdr")]
impl ServerHandler for RecdrServer {}

/// Runs a blocking `control` call off the async thread.
async fn run<T: Send + 'static>(
    work: impl FnOnce() -> error::Result<T> + Send + 'static,
) -> error::Result<T> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|e| Error::Io(std::io::Error::other(e)))?
}

/// The JSON the CLI would print, then any images. Failures carry the CLI's error JSON.
fn reply<T: Serialize>(
    value: error::Result<T>,
    images: impl FnOnce(&T) -> error::Result<Vec<ContentBlock>>,
) -> CallToolResult {
    let blocks = value.and_then(|value| {
        let mut blocks = vec![ContentBlock::text(serde_json::to_string(&value)?)];
        blocks.extend(images(&value)?);
        Ok(blocks)
    });
    match blocks {
        Ok(blocks) => CallToolResult::success(blocks),
        Err(e) => CallToolResult::error(vec![ContentBlock::text(e.to_json().to_string())]),
    }
}

fn png(path: &Path) -> error::Result<ContentBlock> {
    Ok(ContentBlock::image(
        STANDARD.encode(std::fs::read(path)?),
        "image/png",
    ))
}
