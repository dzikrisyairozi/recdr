//! Turns raw user input, from either front end, into validated record options.

use std::path::PathBuf;

use schemars::JsonSchema;
use serde::Deserialize;

use crate::error::{Error, Result};
use crate::model::{
    Endpoint, ExportOptions, FakeKind, Format, FrameSelection, RecordOptions, Rect, TabQuery,
    TabTarget, Target,
};
use crate::time::parse_time;

pub const DEFAULT_FPS: u32 = 15;
pub const DEFAULT_MAX_DURATION: &str = "5m";
pub const DEFAULT_MAX_SIZE: &str = "2GB";
pub const DEFAULT_FORMAT: &str = "gif";
pub const DEFAULT_OUT: &str = ".recdr";
pub const FAKE_SOURCE_ENV: &str = "RECDR_FAKE_SOURCE";

/// What a user asked for, before validation. Every field is optional so the CLI
/// and the MCP server can fill it from their own input shapes.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, JsonSchema)]
#[serde(default)]
pub struct RecordRequest {
    pub window_id: Option<u64>,
    pub window: Option<String>,
    pub process: Option<String>,
    pub region: Option<String>,
    pub monitor: Option<usize>,
    /// A port, an http:// or ws:// URL, or "chrome"; goes with tab_id or tab.
    pub cdp: Option<String>,
    /// Tab id from list_tabs.
    pub tab_id: Option<String>,
    /// Tab whose title or URL contains this text, ignoring case.
    pub tab: Option<String>,
    pub fps: Option<u32>,
    pub max_duration: Option<String>,
    pub max_size: Option<String>,
    /// gif, mp4, "gif,mp4" or none; default gif
    pub format: Option<String>,
    pub no_cursor: bool,
    pub no_click_highlight: bool,
    pub out: Option<PathBuf>,
    pub duration: Option<String>,
    #[serde(skip)]
    #[schemars(skip)]
    pub command: Option<Vec<String>>,
}

impl RecordRequest {
    pub fn into_options(self) -> Result<RecordOptions> {
        let cdp = self.cdp.as_deref().map(parse_endpoint).transpose()?;
        let mut targets = Vec::new();
        if let Some(id) = self.window_id {
            targets.push(Target::WindowId(id));
        }
        if let Some(text) = self.window {
            targets.push(Target::WindowTitle(text));
        }
        if let Some(name) = self.process {
            targets.push(Target::Process(name));
        }
        if let Some(text) = self.region {
            targets.push(Target::Region(parse_region(&text)?));
        }
        if let Some(index) = self.monitor {
            targets.push(Target::Monitor(index));
        }
        let mut queries = Vec::new();
        if let Some(id) = self.tab_id {
            queries.push(TabQuery::Id(id));
        }
        if let Some(text) = self.tab {
            queries.push(TabQuery::Text(text));
        }
        if targets.len() + queries.len() != 1 {
            return Err(usage(
                "give exactly one of --window-id, --window, --process, --region, --monitor, --tab-id, --tab",
            ));
        }
        let mut target = match (queries.pop(), cdp) {
            (Some(query), Some(cdp)) => Target::Tab(TabTarget { cdp, query }),
            (Some(_), None) => return Err(usage("--tab-id and --tab need --cdp")),
            (None, Some(_)) => return Err(usage("--cdp goes with --tab-id or --tab")),
            (None, None) => targets.remove(0),
        };
        if let Ok(value) = std::env::var(FAKE_SOURCE_ENV) {
            target = Target::Fake(fake_kind(&value)?);
        }

        let fps = self.fps.unwrap_or(DEFAULT_FPS);
        if !(1..=60).contains(&fps) {
            return Err(usage("--fps must be between 1 and 60"));
        }
        let max_duration_ms =
            parse_time(self.max_duration.as_deref().unwrap_or(DEFAULT_MAX_DURATION))?;
        let duration_ms = self.duration.as_deref().map(parse_time).transpose()?;
        if duration_ms.is_some_and(|d| d > max_duration_ms) {
            return Err(usage("--duration is longer than --max-duration"));
        }
        if duration_ms.is_some() && self.command.is_some() {
            return Err(usage("give --duration or a command after --, not both"));
        }

        Ok(RecordOptions {
            target,
            fps,
            max_duration_ms,
            max_size_bytes: parse_size(self.max_size.as_deref().unwrap_or(DEFAULT_MAX_SIZE))?,
            formats: parse_formats(self.format.as_deref().unwrap_or(DEFAULT_FORMAT))?,
            cursor: !self.no_cursor,
            clicks: !self.no_click_highlight,
            out: resolve_out(self.out)?,
            duration_ms,
            command: self.command,
        })
    }
}

/// What a user asked `export` for, before validation. Also the MCP tool's arguments.
#[derive(Debug, Clone, PartialEq, Deserialize, JsonSchema)]
pub struct ExportRequest {
    /// Recording id
    pub id: String,
    /// Where to write; the extension picks the format, .gif or .mp4
    pub file: PathBuf,
    /// Start of the part to keep, such as "1500ms" or "2s"; default the start of the recording
    pub from: Option<String>,
    /// End of the part to keep; default the end of the recording
    pub until: Option<String>,
    /// Rectangle to keep, "x,y,width,height" in pixels; default the whole picture
    pub crop: Option<String>,
    /// Output root; default .recdr in the working directory
    pub out: Option<PathBuf>,
}

impl ExportRequest {
    pub fn into_options(self) -> Result<ExportOptions> {
        let extension = self
            .file
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase());
        let format = match extension.as_deref() {
            Some("gif") => Format::Gif,
            Some("mp4") => Format::Mp4,
            _ => return Err(usage("--file must end in .gif or .mp4")),
        };
        Ok(ExportOptions {
            id: self.id,
            out: resolve_out(self.out)?,
            file: std::path::absolute(self.file)?,
            format,
            from_ms: self.from.as_deref().map(parse_time).transpose()?,
            until_ms: self.until.as_deref().map(parse_time).transpose()?,
            crop: self.crop.as_deref().map(parse_region).transpose()?,
        })
    }
}

fn usage(message: &str) -> Error {
    Error::Usage(message.to_owned())
}

fn fake_kind(value: &str) -> Result<FakeKind> {
    match value {
        "static" => Ok(FakeKind::Static),
        "moving" => Ok(FakeKind::Moving),
        "closing" => Ok(FakeKind::Closing),
        "broken" => Ok(FakeKind::Broken),
        "failing" => Ok(FakeKind::Failing),
        other => Err(Error::Usage(format!(
            "{FAKE_SOURCE_ENV}={other:?} is not static, moving, closing, broken or failing"
        ))),
    }
}

/// `500MB` or `2GB` to bytes, with 1 MB = 1,000,000 bytes.
pub fn parse_size(text: &str) -> Result<u64> {
    let bad = || {
        Error::Usage(format!(
            "{text:?} is not a size; use a whole number with MB or GB"
        ))
    };
    let (digits, factor) = if let Some(n) = text.strip_suffix("MB") {
        (n, 1_000_000)
    } else if let Some(n) = text.strip_suffix("GB") {
        (n, 1_000_000_000)
    } else {
        return Err(bad());
    };
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(bad());
    }
    digits
        .parse::<u64>()
        .ok()
        .and_then(|n| n.checked_mul(factor))
        .ok_or_else(bad)
}

/// `x,y,w,h` in physical pixels.
pub fn parse_region(text: &str) -> Result<Rect> {
    let bad = || Error::Usage(format!("{text:?} is not a region; use x,y,width,height"));
    let parts: Vec<&str> = text.split(',').map(str::trim).collect();
    let [x, y, w, h] = parts.as_slice() else {
        return Err(bad());
    };
    let rect = Rect {
        x: x.parse().map_err(|_| bad())?,
        y: y.parse().map_err(|_| bad())?,
        width: w.parse().map_err(|_| bad())?,
        height: h.parse().map_err(|_| bad())?,
    };
    if rect.width == 0 || rect.height == 0 {
        return Err(bad());
    }
    Ok(rect)
}

/// `gif`, `mp4` or both, comma separated, or `none` alone. A repeated format counts once.
pub fn parse_formats(text: &str) -> Result<Vec<Format>> {
    let parts: Vec<&str> = text.split(',').map(str::trim).collect();
    if parts == ["none"] {
        return Ok(Vec::new());
    }
    let mut formats = Vec::new();
    for part in parts {
        let format = match part {
            "gif" => Format::Gif,
            "mp4" => Format::Mp4,
            "none" => return Err(usage("none cannot be combined with gif or mp4")),
            other => return Err(Error::Usage(format!("{other:?} is not gif, mp4 or none"))),
        };
        if !formats.contains(&format) {
            formats.push(format);
        }
    }
    Ok(formats)
}

/// A port, an http:// URL, a ws:// URL, or `chrome`.
pub fn parse_endpoint(text: &str) -> Result<Endpoint> {
    if text == "chrome" {
        return Ok(Endpoint::Chrome);
    }
    if !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit()) {
        return match text.parse::<u16>() {
            Ok(port) if port > 0 => Ok(Endpoint::Port(port)),
            _ => Err(Error::Usage(format!(
                "{text} is not a port between 1 and 65535"
            ))),
        };
    }
    if let Some(rest) = text.strip_prefix("http://") {
        let authority = rest.strip_suffix('/').unwrap_or(rest);
        if authority.is_empty() || authority.contains('/') {
            return Err(Error::Usage(format!(
                "{text} is not an http:// endpoint; use http://host:port"
            )));
        }
        return Ok(Endpoint::Http(if authority.contains(':') {
            authority.to_owned()
        } else {
            format!("{authority}:80")
        }));
    }
    if let Some(rest) = text.strip_prefix("ws://") {
        if rest.split('/').next().unwrap_or_default().is_empty() {
            return Err(Error::Usage(format!(
                "{text} is not a ws:// endpoint; use ws://host:port/path"
            )));
        }
        return Ok(Endpoint::Ws(text.to_owned()));
    }
    if text.starts_with("https://") || text.starts_with("wss://") {
        return Err(usage(
            "recdr does not connect over TLS; use an http:// or ws:// endpoint",
        ));
    }
    Err(Error::Usage(format!(
        "{text} is not a CDP endpoint; use a port, an http:// URL, a ws:// URL, or chrome"
    )))
}

/// The output root, made absolute so a detached recorder and later commands agree on it.
pub fn resolve_out(out: Option<PathBuf>) -> Result<PathBuf> {
    let out = out.unwrap_or_else(|| PathBuf::from(DEFAULT_OUT));
    Ok(std::path::absolute(out)?)
}

/// Exactly one of `--at`, `--every` and `--keyframes`.
pub fn parse_selection(
    at: &[String],
    every: Option<&str>,
    keyframes: bool,
) -> Result<FrameSelection> {
    let given = usize::from(!at.is_empty()) + usize::from(every.is_some()) + usize::from(keyframes);
    if given != 1 {
        return Err(usage("give exactly one of --at, --every, --keyframes"));
    }
    if let Some(every) = every {
        let step = parse_time(every)?;
        if step == 0 {
            return Err(usage("--every must be more than 0"));
        }
        return Ok(FrameSelection::Every(step));
    }
    if keyframes {
        return Ok(FrameSelection::Keyframes);
    }
    Ok(FrameSelection::At(
        at.iter().map(|t| parse_time(t)).collect::<Result<_>>()?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn usage_message(text: &str) -> String {
        let error = parse_endpoint(text).unwrap_err();
        assert_eq!(error.code(), "usage", "{text}");
        error.to_string()
    }

    fn target_of(request: RecordRequest) -> Result<Target> {
        RecordRequest {
            duration: Some("1s".into()),
            ..request
        }
        .into_options()
        .map(|options| options.target)
    }

    #[test]
    fn tab_flags_need_cdp_and_only_with_each_other() {
        let message = |request| target_of(request).unwrap_err().to_string();
        assert_eq!(
            message(RecordRequest {
                tab: Some("x".into()),
                ..Default::default()
            }),
            "--tab-id and --tab need --cdp"
        );
        assert_eq!(
            message(RecordRequest {
                cdp: Some("9222".into()),
                monitor: Some(0),
                ..Default::default()
            }),
            "--cdp goes with --tab-id or --tab"
        );
        assert_eq!(
            message(RecordRequest {
                cdp: Some("1".into()),
                tab_id: Some("a".into()),
                tab: Some("b".into()),
                ..Default::default()
            }),
            "give exactly one of --window-id, --window, --process, --region, --monitor, --tab-id, --tab"
        );
    }

    #[test]
    fn tab_flag_with_cdp_is_a_tab_target() {
        let target = target_of(RecordRequest {
            cdp: Some("9222".into()),
            tab_id: Some("AB".into()),
            ..Default::default()
        });
        assert_eq!(
            target.unwrap(),
            Target::Tab(TabTarget {
                cdp: Endpoint::Port(9222),
                query: TabQuery::Id("AB".into()),
            })
        );
    }

    #[test]
    fn parse_endpoint_chrome() {
        assert_eq!(parse_endpoint("chrome").unwrap(), Endpoint::Chrome);
    }

    #[test]
    fn parse_endpoint_port() {
        assert_eq!(parse_endpoint("9222").unwrap(), Endpoint::Port(9222));
        for text in ["0", "65536", "99999999999"] {
            assert_eq!(
                usage_message(text),
                format!("{text} is not a port between 1 and 65535")
            );
        }
    }

    #[test]
    fn parse_endpoint_http() {
        let http = |text| parse_endpoint(text).unwrap();
        assert_eq!(
            http("http://127.0.0.1:9222"),
            Endpoint::Http("127.0.0.1:9222".into())
        );
        assert_eq!(
            http("http://host:9222/"),
            Endpoint::Http("host:9222".into())
        );
        assert_eq!(http("http://host"), Endpoint::Http("host:80".into()));
        for text in ["http://", "http:///", "http://host:1/json"] {
            assert!(
                usage_message(text).ends_with("is not an http:// endpoint; use http://host:port"),
                "{text}"
            );
        }
    }

    #[test]
    fn parse_endpoint_ws() {
        let url = "ws://127.0.0.1:9222/devtools/browser/abc";
        assert_eq!(parse_endpoint(url).unwrap(), Endpoint::Ws(url.into()));
        assert_eq!(
            usage_message("ws:///x"),
            "ws:///x is not a ws:// endpoint; use ws://host:port/path"
        );
    }

    #[test]
    fn parse_endpoint_refuses_tls() {
        for text in ["https://example.test", "wss://example.test/x"] {
            assert_eq!(
                usage_message(text),
                "recdr does not connect over TLS; use an http:// or ws:// endpoint"
            );
        }
    }

    #[test]
    fn parse_endpoint_anything_else() {
        assert_eq!(
            usage_message("localhost:9222"),
            "localhost:9222 is not a CDP endpoint; use a port, an http:// URL, a ws:// URL, or chrome"
        );
    }

    #[test]
    fn parse_size_units() {
        assert_eq!(parse_size("2GB").unwrap(), 2_000_000_000);
        assert_eq!(parse_size("1MB").unwrap(), 1_000_000);
        for text in ["2", "2KB", "-1MB", "MB"] {
            assert_eq!(parse_size(text).unwrap_err().code(), "usage", "{text}");
        }
    }

    #[test]
    fn parse_region_reads_four_numbers_and_rejects_zero_size() {
        assert_eq!(
            parse_region("10,20,640,480").unwrap(),
            Rect {
                x: 10,
                y: 20,
                width: 640,
                height: 480
            }
        );
        assert_eq!(parse_region("-1920,0,10,10").unwrap().x, -1920);
        for text in ["1,2,3", "1,2,0,4", "a,b,c,d"] {
            assert_eq!(parse_region(text).unwrap_err().code(), "usage", "{text}");
        }
    }

    #[test]
    fn parse_formats_both_and_deduplicated() {
        assert_eq!(
            parse_formats("gif,mp4").unwrap(),
            [Format::Gif, Format::Mp4]
        );
        assert_eq!(parse_formats("gif,gif").unwrap(), [Format::Gif]);
        let webm = parse_formats("webm").unwrap_err();
        assert_eq!(webm.code(), "usage");
        assert!(webm.to_string().contains("none"), "{webm}");
    }

    fn export_to(file: &str) -> Result<ExportOptions> {
        ExportRequest {
            id: "x".into(),
            file: file.into(),
            from: None,
            until: None,
            crop: None,
            out: None,
        }
        .into_options()
    }

    #[test]
    fn export_extension_picks_the_format() {
        assert_eq!(export_to("a.gif").unwrap().format, Format::Gif);
        assert_eq!(export_to("a.GIF").unwrap().format, Format::Gif);
        assert_eq!(export_to("dir/a.mp4").unwrap().format, Format::Mp4);
        assert!(export_to("a.gif").unwrap().file.is_absolute());
        for file in ["a.webm", "a", "gif"] {
            let error = export_to(file).unwrap_err();
            assert_eq!(error.code(), "usage", "{file}");
            assert_eq!(error.to_string(), "--file must end in .gif or .mp4");
        }
    }

    #[test]
    fn parse_formats_none_alone_is_no_format() {
        assert_eq!(parse_formats("none").unwrap(), []);
        for text in ["none,gif", "gif,none", "mp4,none"] {
            let error = parse_formats(text).unwrap_err();
            assert_eq!(error.code(), "usage", "{text}");
            assert_eq!(error.to_string(), "none cannot be combined with gif or mp4");
        }
    }

    #[test]
    fn parse_selection_takes_exactly_one_kind() {
        let at = ["500ms".to_owned(), "1s".to_owned()];
        assert!(matches!(
            parse_selection(&at, None, false).unwrap(),
            FrameSelection::At(times) if times == [500, 1000]
        ));
        assert!(matches!(
            parse_selection(&[], Some("250ms"), false).unwrap(),
            FrameSelection::Every(250)
        ));
        assert!(matches!(
            parse_selection(&[], None, true).unwrap(),
            FrameSelection::Keyframes
        ));
        for (at, every, keyframes) in [
            (&[][..], None, false),
            (&at[..], None, true),
            (&[][..], Some("0ms"), false),
        ] {
            let error = parse_selection(at, every, keyframes).unwrap_err();
            assert_eq!(error.code(), "usage");
        }
    }
}
