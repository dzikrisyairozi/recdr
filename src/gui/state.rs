//! What the app is doing, and what each event does to it. Every event goes through
//! `step` with the time, so nothing here reads a clock or names a window type.

use std::path::PathBuf;

use super::geometry::{self, MIN_KEPT_MS};
use crate::model::{MonitorInfo, Rect, Status, WindowInfo};
use crate::request::{ExportRequest, RecordRequest};

#[derive(Debug, Clone, PartialEq)]
pub struct App {
    pub out: PathBuf,
    pub screen: Screen,
    pub monitors: Vec<MonitorInfo>,
    pub windows: Vec<WindowInfo>,
    /// Pixels per point where the frame is; 1.0 at the start.
    pub scale: f64,
    pub inside: Rect,
    /// The window chosen in the target menu; None is "Region".
    pub window: Option<u64>,
    pub fps: u32,
    pub cursor: bool,
    pub clicks: bool,
    /// An error, or the path of the last export.
    pub message: Option<String>,
    /// The time of the last event.
    pub last_ms: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    Idle {
        starting: bool,
        closing: bool,
    },
    Recording {
        id: String,
        started_ms: u64,
        asked_ms: u64,
        stopping: bool,
        closing: bool,
    },
    Editing(Edit),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Edit {
    pub id: String,
    pub width: u32,
    pub height: u32,
    pub duration_ms: u64,
    pub from_ms: u64,
    pub until_ms: u64,
    pub crop: Option<Rect>,
    pub position_ms: u64,
    pub playing: bool,
    /// The time asked for and the stored frame on show.
    pub picture: Option<(u64, PathBuf)>,
    pub loading: bool,
    pub exporting: bool,
    /// A close came during an export; the app quits once the export reports.
    pub closing: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    // from the windows part
    Tick,
    Scale(f64),
    MenuOpened,
    Picked(Option<u64>),
    Moved {
        x: i32,
        y: i32,
    },
    Resized(Rect),
    Fps(u32),
    Cursor(bool),
    Clicks(bool),
    RecordPressed,
    StopPressed,
    PlayPressed,
    Seek(u64),
    From(u64),
    Until(u64),
    Crop(Option<Rect>),
    Save(PathBuf),
    NewRecording,
    CloseRequested,
    // from the worker
    Monitors(Vec<MonitorInfo>),
    Windows(Vec<WindowInfo>),
    Started(String),
    Status(Status),
    Stopped {
        id: String,
        width: u32,
        height: u32,
        duration_ms: u64,
    },
    Picture {
        t_ms: u64,
        path: PathBuf,
    },
    Exported(PathBuf),
    Failed(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    ListMonitors,
    ListWindows,
    Start(RecordRequest),
    AskStatus(String),
    Stop(String),
    Preview { id: String, t_ms: u64 },
    Export(ExportRequest),
    Quit,
}

const IDLE: Screen = Screen::Idle {
    starting: false,
    closing: false,
};

impl App {
    /// The app as it opens, and the first action to run.
    pub fn new(out: PathBuf) -> (App, Vec<Action>) {
        let app = App {
            out,
            screen: IDLE,
            monitors: Vec::new(),
            windows: Vec::new(),
            scale: 1.0,
            inside: geometry::initial(&[]),
            window: None,
            fps: 15,
            cursor: true,
            clicks: true,
            message: None,
            last_ms: 0,
        };
        (app, vec![Action::ListMonitors])
    }

    fn fit(&self, rect: Rect) -> Rect {
        geometry::keep_inside(rect, &self.monitors, geometry::toolbar_size(self.scale))
    }

    fn start_request(&self) -> RecordRequest {
        let r = &self.inside;
        RecordRequest {
            window_id: self.window,
            region: self
                .window
                .is_none()
                .then(|| format!("{},{},{},{}", r.x, r.y, r.width, r.height)),
            fps: Some(self.fps),
            format: Some("none".into()),
            no_cursor: !self.cursor,
            no_click_highlight: !self.clicks,
            out: Some(self.out.clone()),
            ..Default::default()
        }
    }
}

/// Runs a close request to its end, for when the event loop has already stopped.
/// `perform` runs a stop and returns its outcome; `next` returns the next event from
/// the worker threads, or None when the caller's time limit has passed. Returns at
/// `Quit` or at None.
pub fn wind_down(
    app: &mut App,
    mut next: impl FnMut() -> Option<Event>,
    mut perform: impl FnMut(Action) -> Option<Event>,
) {
    let mut event = Event::CloseRequested;
    loop {
        let mut outcome = None;
        for action in step(app, event, 0) {
            match action {
                Action::Quit => return,
                Action::Stop(_) => outcome = perform(action),
                // A preview or a status poll means nothing to a closing app.
                _ => {}
            }
        }
        match outcome.or_else(&mut next) {
            Some(e) => event = e,
            None => return,
        }
    }
}

/// Applies one event at time `now_ms` and returns what to do about it.
pub fn step(app: &mut App, event: Event, now_ms: u64) -> Vec<Action> {
    let elapsed = now_ms.saturating_sub(app.last_ms);
    app.last_ms = now_ms;
    match event {
        Event::Scale(scale) => {
            app.scale = scale;
            Vec::new()
        }
        Event::Monitors(list) => {
            app.monitors = list;
            if matches!(app.screen, Screen::Idle { .. }) && app.window.is_none() {
                app.inside = geometry::initial(&app.monitors);
            }
            Vec::new()
        }
        Event::Windows(list) => {
            app.windows = list.into_iter().filter(|w| !w.minimized).collect();
            Vec::new()
        }
        event => match app.screen {
            Screen::Idle { starting, closing } => idle(app, event, starting, closing),
            Screen::Recording { .. } => recording(app, event, now_ms),
            Screen::Editing(_) => editing(app, event, elapsed),
        },
    }
}

fn idle(app: &mut App, event: Event, starting: bool, closing: bool) -> Vec<Action> {
    let now_ms = app.last_ms;
    let recording = |id: String, stopping: bool| Screen::Recording {
        id,
        started_ms: now_ms,
        asked_ms: now_ms,
        stopping,
        closing: stopping,
    };
    if !starting {
        match event {
            Event::MenuOpened => return vec![Action::ListWindows],
            Event::Picked(None) => app.window = None,
            Event::Picked(Some(id)) => {
                if let Some(window) = app.windows.iter().find(|w| w.id == id) {
                    app.inside = window.bounds;
                    app.window = Some(id);
                }
            }
            Event::Moved { x, y } => {
                app.inside = app.fit(Rect { x, y, ..app.inside });
                app.window = None;
            }
            Event::Resized(rect) if app.window.is_none() => app.inside = app.fit(rect),
            Event::Fps(n) => app.fps = n.clamp(1, 60),
            Event::Cursor(on) => app.cursor = on,
            Event::Clicks(on) => app.clicks = on,
            Event::RecordPressed => {
                app.screen = Screen::Idle {
                    starting: true,
                    closing: false,
                };
                app.message = None;
                return vec![Action::Start(app.start_request())];
            }
            Event::CloseRequested => return vec![Action::Quit],
            _ => {}
        }
        return Vec::new();
    }
    match (event, closing) {
        (Event::Started(id), false) => app.screen = recording(id, false),
        (Event::Failed(text), false) => {
            app.screen = IDLE;
            app.message = Some(text);
        }
        (Event::CloseRequested, _) => {
            app.screen = Screen::Idle {
                starting: true,
                closing: true,
            }
        }
        (Event::Started(id), true) => {
            app.screen = recording(id.clone(), true);
            return vec![Action::Stop(id)];
        }
        (Event::Failed(_), true) => return vec![Action::Quit],
        _ => {}
    }
    Vec::new()
}

fn recording(app: &mut App, event: Event, now_ms: u64) -> Vec<Action> {
    let Screen::Recording {
        id,
        asked_ms,
        stopping,
        closing,
        ..
    } = &mut app.screen
    else {
        return Vec::new();
    };
    let id = id.clone();
    match event {
        Event::Tick if !*stopping && now_ms.saturating_sub(*asked_ms) >= 1000 => {
            *asked_ms = now_ms;
            vec![Action::AskStatus(id)]
        }
        Event::Status(status) if !*stopping && status != Status::Recording => {
            *stopping = true;
            vec![Action::Stop(id)]
        }
        Event::StopPressed if !*stopping => {
            *stopping = true;
            vec![Action::Stop(id)]
        }
        Event::CloseRequested if !*stopping => {
            (*stopping, *closing) = (true, true);
            vec![Action::Stop(id)]
        }
        Event::CloseRequested => {
            *closing = true;
            Vec::new()
        }
        Event::Stopped { .. } | Event::Failed(_) if *closing => vec![Action::Quit],
        Event::Stopped {
            id,
            width,
            height,
            duration_ms,
        } => {
            app.screen = Screen::Editing(Edit {
                id,
                width,
                height,
                duration_ms,
                from_ms: 0,
                until_ms: duration_ms,
                crop: None,
                position_ms: 0,
                playing: false,
                picture: None,
                loading: false,
                exporting: false,
                closing: false,
            });
            preview(app)
        }
        Event::Failed(text) if *stopping => {
            app.screen = IDLE;
            app.message = Some(text);
            Vec::new()
        }
        Event::Failed(text) => {
            app.message = Some(text);
            Vec::new()
        }
        _ => Vec::new(),
    }
}

fn editing(app: &mut App, event: Event, elapsed: u64) -> Vec<Action> {
    let Screen::Editing(edit) = &mut app.screen else {
        return Vec::new();
    };
    match event {
        Event::Tick if edit.playing => {
            edit.position_ms += elapsed;
            if edit.position_ms >= edit.until_ms {
                edit.position_ms = edit.from_ms;
            }
        }
        Event::PlayPressed => {
            edit.playing = !edit.playing;
            if edit.playing && !(edit.from_ms..edit.until_ms).contains(&edit.position_ms) {
                edit.position_ms = edit.from_ms;
            }
        }
        Event::Seek(t) => {
            edit.position_ms = t.min(edit.duration_ms);
            edit.playing = false;
        }
        Event::From(t) => {
            edit.from_ms = t.min(edit.until_ms.saturating_sub(MIN_KEPT_MS));
            return Vec::new();
        }
        Event::Until(t) => {
            edit.until_ms = if edit.duration_ms < MIN_KEPT_MS {
                edit.duration_ms
            } else {
                t.max(edit.from_ms + MIN_KEPT_MS).min(edit.duration_ms)
            };
            return Vec::new();
        }
        Event::Crop(crop) => {
            edit.crop = crop;
            return Vec::new();
        }
        Event::Picture { t_ms, path } => {
            edit.picture = Some((t_ms, path));
            edit.loading = false;
        }
        Event::Save(file) if !edit.exporting => {
            edit.exporting = true;
            let crop = edit
                .crop
                .map(|r| format!("{},{},{},{}", r.x, r.y, r.width, r.height));
            let request = ExportRequest {
                id: edit.id.clone(),
                file,
                from: Some(format!("{}ms", edit.from_ms)),
                until: Some(format!("{}ms", edit.until_ms)),
                crop,
                out: Some(app.out.clone()),
            };
            app.message = None;
            return vec![Action::Export(request)];
        }
        Event::Exported(path) if edit.exporting => {
            edit.exporting = false;
            if edit.closing {
                return vec![Action::Quit];
            }
            app.message = Some(path.display().to_string());
            return Vec::new();
        }
        Event::Failed(_) if edit.exporting && edit.closing => return vec![Action::Quit],
        Event::Failed(text) => {
            edit.exporting = false;
            edit.loading = false;
            app.message = Some(text);
            return Vec::new();
        }
        Event::NewRecording if !edit.exporting => {
            app.screen = IDLE;
            app.message = None;
            return Vec::new();
        }
        // Quitting now would leave the export's .part file and no result.
        Event::CloseRequested if edit.exporting => {
            edit.closing = true;
            return Vec::new();
        }
        Event::CloseRequested => return vec![Action::Quit],
        _ => return Vec::new(),
    }
    preview(app)
}

/// When no picture is loading and the one on show is not for the position, ask for it.
fn preview(app: &mut App) -> Vec<Action> {
    let Screen::Editing(edit) = &mut app.screen else {
        return Vec::new();
    };
    let shown = edit.picture.as_ref().map(|(t_ms, _)| *t_ms);
    if edit.loading || shown == Some(edit.position_ms) {
        return Vec::new();
    }
    edit.loading = true;
    vec![Action::Preview {
        id: edit.id.clone(),
        t_ms: edit.position_ms,
    }]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: i32, y: i32, width: u32, height: u32) -> Rect {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    fn app() -> App {
        let (mut app, _) = App::new(PathBuf::from("out"));
        app.monitors = vec![MonitorInfo {
            index: 0,
            bounds: rect(0, 0, 1920, 1080),
            primary: true,
        }];
        app
    }

    fn window(id: u64, bounds: Rect, minimized: bool) -> WindowInfo {
        WindowInfo {
            id,
            title: format!("w{id}"),
            process: String::new(),
            pid: 1,
            bounds,
            minimized,
        }
    }

    fn recording_app(id: &str, at_ms: u64) -> App {
        let mut app = app();
        step(&mut app, Event::RecordPressed, at_ms);
        step(&mut app, Event::Started(id.into()), at_ms);
        app
    }

    fn stopped(id: &str, duration_ms: u64) -> Event {
        Event::Stopped {
            id: id.into(),
            width: 320,
            height: 240,
            duration_ms,
        }
    }

    fn editing_app(duration_ms: u64) -> App {
        let mut app = recording_app("r", 0);
        step(&mut app, Event::StopPressed, 10);
        step(&mut app, stopped("r", duration_ms), 20);
        app
    }

    fn edit(app: &App) -> &Edit {
        match &app.screen {
            Screen::Editing(edit) => edit,
            other => panic!("not editing: {other:?}"),
        }
    }

    #[test]
    fn a_new_app_is_idle_and_lists_monitors() {
        let (app, actions) = App::new(PathBuf::from("out"));
        assert_eq!(app.screen, IDLE);
        assert_eq!((app.fps, app.cursor, app.clicks), (15, true, true));
        assert_eq!(actions, [Action::ListMonitors]);
    }

    #[test]
    fn monitors_center_the_frame_and_minimized_windows_are_left_out() {
        let (mut app, _) = App::new(PathBuf::from("out"));
        let monitors = app_monitors();
        step(&mut app, Event::Monitors(monitors.clone()), 0);
        assert_eq!(app.monitors, monitors);
        assert_eq!(app.inside, rect(640, 300, 640, 480));
        let list = vec![
            window(1, rect(0, 0, 10, 10), false),
            window(2, rect(0, 0, 10, 10), true),
        ];
        step(&mut app, Event::Windows(list), 0);
        assert_eq!(app.windows.iter().map(|w| w.id).collect::<Vec<_>>(), [1]);
        step(&mut app, Event::Scale(1.5), 0);
        assert_eq!(app.scale, 1.5);
    }

    fn app_monitors() -> Vec<MonitorInfo> {
        app().monitors
    }

    #[test]
    fn record_asks_for_the_region_or_the_window() {
        let mut app = app();
        app.inside = rect(100, 200, 640, 480);
        let actions = step(&mut app, Event::RecordPressed, 0);
        let [Action::Start(request)] = actions.as_slice() else {
            panic!("{actions:?}");
        };
        assert_eq!(request.region.as_deref(), Some("100,200,640,480"));
        assert_eq!(request.window_id, None);
        assert_eq!(request.format.as_deref(), Some("none"));
        assert_eq!(request.fps, Some(15));
        assert_eq!(request.out, Some(PathBuf::from("out")));
        assert!(!request.no_cursor && !request.no_click_highlight);
        assert_eq!(step(&mut app, Event::RecordPressed, 1), []);

        let mut app = self::app();
        step(
            &mut app,
            Event::Windows(vec![window(9, rect(5, 6, 300, 200), false)]),
            0,
        );
        step(&mut app, Event::Picked(Some(9)), 0);
        step(&mut app, Event::Cursor(false), 0);
        let actions = step(&mut app, Event::RecordPressed, 0);
        let [Action::Start(request)] = actions.as_slice() else {
            panic!("{actions:?}");
        };
        assert_eq!(
            (request.window_id, request.region.as_deref()),
            (Some(9), None)
        );
        assert!(request.no_cursor);
    }

    #[test]
    fn picking_a_window_puts_the_frame_on_it_until_moved() {
        let mut app = app();
        let bounds = rect(5, 6, 300, 200);
        step(&mut app, Event::Windows(vec![window(9, bounds, false)]), 0);
        assert_eq!(step(&mut app, Event::MenuOpened, 0), [Action::ListWindows]);
        step(&mut app, Event::Picked(Some(9)), 0);
        assert_eq!((app.inside, app.window), (bounds, Some(9)));
        step(&mut app, Event::Resized(rect(0, 0, 50, 50)), 0);
        assert_eq!(app.inside, bounds);
        step(&mut app, Event::Moved { x: 100, y: 100 }, 0);
        assert_eq!(app.window, None);
        assert_eq!(app.inside, rect(100, 100, 300, 200));
        step(&mut app, Event::Picked(Some(404)), 0);
        assert_eq!(app.window, None);
        step(&mut app, Event::Fps(99), 0);
        assert_eq!(app.fps, 60);
    }

    #[test]
    fn recording_asks_for_status_once_a_second() {
        let mut app = recording_app("r", 1000);
        assert_eq!(step(&mut app, Event::Tick, 1999), []);
        assert_eq!(
            step(&mut app, Event::Tick, 2000),
            [Action::AskStatus("r".into())]
        );
        assert_eq!(step(&mut app, Event::Tick, 2500), []);
        assert_eq!(
            step(&mut app, Event::Status(Status::Finished), 2600),
            [Action::Stop("r".into())]
        );
        assert_eq!(step(&mut app, Event::Status(Status::Finished), 2700), []);
    }

    #[test]
    fn stop_is_asked_once_and_leads_to_the_editor() {
        let mut app = recording_app("r", 0);
        assert_eq!(
            step(&mut app, Event::StopPressed, 5),
            [Action::Stop("r".into())]
        );
        assert_eq!(step(&mut app, Event::StopPressed, 6), []);
        let actions = step(&mut app, stopped("r", 3000), 7);
        assert_eq!(
            actions,
            [Action::Preview {
                id: "r".into(),
                t_ms: 0
            }]
        );
        let edit = edit(&app);
        assert_eq!(
            (edit.from_ms, edit.until_ms, edit.duration_ms),
            (0, 3000, 3000)
        );
        assert!(edit.loading);
    }

    #[test]
    fn failures_return_to_idle_with_the_message() {
        let mut app = app();
        step(&mut app, Event::RecordPressed, 0);
        step(&mut app, Event::Failed("no".into()), 1);
        assert_eq!(app.screen, IDLE);
        assert_eq!(app.message.as_deref(), Some("no"));

        let mut app = recording_app("r", 0);
        step(&mut app, Event::Failed("hiccup".into()), 1);
        assert!(matches!(app.screen, Screen::Recording { .. }));
        assert_eq!(app.message.as_deref(), Some("hiccup"));
        step(&mut app, Event::StopPressed, 2);
        step(&mut app, Event::Failed("gone".into()), 3);
        assert_eq!(app.screen, IDLE);
        assert_eq!(app.message.as_deref(), Some("gone"));
    }

    #[test]
    fn closing_never_leaves_a_recording_running() {
        let mut app = app();
        assert_eq!(step(&mut app, Event::CloseRequested, 0), [Action::Quit]);
        let mut app = editing_app(3000);
        assert_eq!(step(&mut app, Event::CloseRequested, 30), [Action::Quit]);

        let mut app = recording_app("r", 0);
        assert_eq!(
            step(&mut app, Event::CloseRequested, 1),
            [Action::Stop("r".into())]
        );
        assert_eq!(step(&mut app, stopped("r", 1), 2), [Action::Quit]);

        let mut app = self::app();
        step(&mut app, Event::RecordPressed, 0);
        assert_eq!(step(&mut app, Event::CloseRequested, 1), []);
        assert_eq!(
            step(&mut app, Event::Started("r".into()), 2),
            [Action::Stop("r".into())]
        );
        assert_eq!(step(&mut app, stopped("r", 1), 3), [Action::Quit]);

        let mut app = self::app();
        step(&mut app, Event::RecordPressed, 0);
        step(&mut app, Event::CloseRequested, 1);
        assert_eq!(step(&mut app, Event::Failed("x".into()), 2), [Action::Quit]);
    }

    /// Quit from the Dock ends the event loop at once, so on_exit has to finish what a
    /// close request starts: wait for a starting recording, then stop it.
    #[test]
    fn closing_waits_for_an_export() {
        for outcome in [
            Event::Exported("s.gif".into()),
            Event::Failed("disk full".into()),
        ] {
            let mut app = editing_app(3000);
            let save = step(&mut app, Event::Save("s.gif".into()), 30);
            assert!(matches!(save[..], [Action::Export(_)]), "{save:?}");
            assert_eq!(step(&mut app, Event::CloseRequested, 40), []);
            assert_eq!(step(&mut app, outcome, 50), [Action::Quit]);
        }
    }

    #[test]
    fn winding_down_stops_a_starting_recording() {
        let mut app = self::app();
        step(&mut app, Event::RecordPressed, 0);
        let mut arriving = vec![Event::Started("r".into())].into_iter();
        let mut performed = Vec::new();
        wind_down(
            &mut app,
            || arriving.next(),
            |action| {
                performed.push(action.clone());
                matches!(action, Action::Stop(_)).then(|| stopped("r", 1))
            },
        );
        assert_eq!(performed, [Action::Stop("r".into())]);

        // Nothing arrives before the time limit: winding down returns all the same.
        let mut app = self::app();
        step(&mut app, Event::RecordPressed, 0);
        wind_down(&mut app, || None, |_| None);
    }

    #[test]
    fn playing_moves_the_position_and_loops() {
        let mut app = editing_app(3000);
        step(&mut app, Event::Until(1000), 30);
        step(&mut app, Event::PlayPressed, 40);
        let before = edit(&app).position_ms;
        step(&mut app, Event::Tick, 80);
        assert_eq!(edit(&app).position_ms, before + 40);
        if let Screen::Editing(edit) = &mut app.screen {
            edit.position_ms = 990;
        }
        step(&mut app, Event::Tick, 100);
        assert_eq!(edit(&app).position_ms, 0);
    }

    #[test]
    fn one_preview_at_a_time() {
        let mut app = editing_app(3000);
        assert_eq!(step(&mut app, Event::Seek(500), 30), []);
        let actions = step(
            &mut app,
            Event::Picture {
                t_ms: 0,
                path: "f0.png".into(),
            },
            40,
        );
        assert_eq!(
            actions,
            [Action::Preview {
                id: "r".into(),
                t_ms: 500
            }]
        );
        let actions = step(
            &mut app,
            Event::Picture {
                t_ms: 500,
                path: "f5.png".into(),
            },
            50,
        );
        assert_eq!(actions, []);
        assert!(!edit(&app).loading);
    }

    #[test]
    fn the_handles_keep_a_tenth_of_a_second() {
        let mut app = editing_app(3000);
        step(&mut app, Event::Until(1000), 30);
        step(&mut app, Event::From(2000), 30);
        assert_eq!(edit(&app).from_ms, 900);
        step(&mut app, Event::Until(0), 30);
        assert_eq!(edit(&app).until_ms, 1000);
        step(&mut app, Event::Until(9999), 30);
        assert_eq!(edit(&app).until_ms, 3000);

        let mut short = editing_app(50);
        step(&mut short, Event::Until(10), 30);
        assert_eq!(edit(&short).until_ms, 50);
    }

    #[test]
    fn save_asks_for_one_export() {
        let mut app = editing_app(3000);
        step(&mut app, Event::Crop(Some(rect(1, 2, 30, 40))), 30);
        let actions = step(&mut app, Event::Save("a.gif".into()), 31);
        let [Action::Export(request)] = actions.as_slice() else {
            panic!("{actions:?}");
        };
        assert_eq!(request.from.as_deref(), Some("0ms"));
        assert_eq!(request.until.as_deref(), Some("3000ms"));
        assert_eq!(request.crop.as_deref(), Some("1,2,30,40"));
        assert_eq!(request.out, Some(PathBuf::from("out")));
        assert_eq!(step(&mut app, Event::Save("b.gif".into()), 32), []);
        step(&mut app, Event::NewRecording, 33);
        assert!(matches!(app.screen, Screen::Editing(_)));
        step(&mut app, Event::Exported("a.gif".into()), 34);
        assert_eq!(app.message.as_deref(), Some("a.gif"));
        assert!(!edit(&app).exporting);
        step(&mut app, Event::NewRecording, 35);
        assert_eq!(app.screen, IDLE);
    }

    #[test]
    fn time_going_back_does_not_panic() {
        let mut app = editing_app(3000);
        step(&mut app, Event::PlayPressed, 1000);
        step(&mut app, Event::Tick, 10);
        let mut app = recording_app("r", 5000);
        step(&mut app, Event::Tick, 10);
    }

    fn every_event() -> Vec<Event> {
        vec![
            Event::Tick,
            Event::MenuOpened,
            Event::Picked(None),
            Event::Picked(Some(5)),
            Event::Moved { x: 1, y: 2 },
            Event::Resized(rect(1, 2, 300, 300)),
            Event::Fps(30),
            Event::Cursor(false),
            Event::Clicks(false),
            Event::RecordPressed,
            Event::StopPressed,
            Event::PlayPressed,
            Event::Seek(10),
            Event::From(10),
            Event::Until(500),
            Event::Crop(Some(rect(0, 0, 4, 4))),
            Event::Save("s.gif".into()),
            Event::NewRecording,
            Event::CloseRequested,
            Event::Started("other".into()),
            Event::Status(Status::Finished),
            Event::Status(Status::Recording),
            stopped("other", 100),
            Event::Picture {
                t_ms: 5,
                path: "p.png".into(),
            },
            Event::Exported("e.gif".into()),
            Event::Failed("f".into()),
        ]
    }

    /// An event the table does not list for a screen changes nothing but `last_ms`.
    #[test]
    fn unlisted_events_change_nothing() {
        use std::mem::discriminant as kind;
        let names = |list: &[Event]| list.iter().map(kind).collect::<Vec<_>>();
        let idle = app();
        let mut starting = app();
        step(&mut starting, Event::RecordPressed, 0);
        let mut starting_closing = starting.clone();
        step(&mut starting_closing, Event::CloseRequested, 0);
        let recording = recording_app("r", 0);
        let mut stopping = recording.clone();
        step(&mut stopping, Event::StopPressed, 0);
        let mut editing = editing_app(3000);
        if let Screen::Editing(edit) = &mut editing.screen {
            edit.loading = false;
        }
        let listed = [
            (
                idle,
                names(&[
                    Event::MenuOpened,
                    Event::Picked(None),
                    Event::Moved { x: 0, y: 0 },
                    Event::Resized(rect(0, 0, 1, 1)),
                    Event::Fps(1),
                    Event::Cursor(true),
                    Event::Clicks(true),
                    Event::RecordPressed,
                    Event::CloseRequested,
                ]),
            ),
            (
                starting,
                names(&[
                    Event::Started(String::new()),
                    Event::Failed(String::new()),
                    Event::CloseRequested,
                ]),
            ),
            (
                starting_closing,
                names(&[Event::Started(String::new()), Event::Failed(String::new())]),
            ),
            (
                recording,
                names(&[
                    Event::Tick,
                    Event::Status(Status::Finished),
                    Event::StopPressed,
                    Event::CloseRequested,
                    stopped("", 0),
                    Event::Failed(String::new()),
                ]),
            ),
            (
                stopping,
                names(&[
                    Event::CloseRequested,
                    stopped("", 0),
                    Event::Failed(String::new()),
                ]),
            ),
            (
                editing,
                names(&[
                    Event::PlayPressed,
                    Event::Seek(0),
                    Event::From(0),
                    Event::Until(0),
                    Event::Crop(None),
                    Event::Picture {
                        t_ms: 0,
                        path: PathBuf::new(),
                    },
                    Event::Save(PathBuf::new()),
                    Event::Failed(String::new()),
                    Event::NewRecording,
                    Event::CloseRequested,
                ]),
            ),
        ];
        for (before, handled) in listed {
            for event in every_event() {
                let unlisted =
                    !handled.contains(&kind(&event)) || event == Event::Status(Status::Recording);
                if !unlisted {
                    continue;
                }
                let mut after = before.clone();
                let actions = step(&mut after, event.clone(), 77);
                let mut expected = before.clone();
                expected.last_ms = 77;
                assert_eq!(actions, [], "{event:?} on {:?}", before.screen);
                assert_eq!(after, expected, "{event:?} on {:?}", before.screen);
            }
        }
    }
}
