//! The `eframe` app: collects events from the windows and the worker threads, runs each
//! through `state::step`, and starts a thread per action.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

use eframe::egui;

use super::geometry::{self, TOOLBAR_POINTS};
use super::state::{self, Action, Event, Screen};
use super::{editor, overlay, worker};
use crate::error::{Error, Result};

/// The editor's size when it opens, in points.
const EDITOR_POINTS: (f32, f32) = (960.0, 640.0);

pub fn run(out: PathBuf) -> Result<()> {
    let (app, actions) = state::App::new(out);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("recdr")
            .with_decorations(false)
            // glow picks one pixel format for every window from this one; without
            // alpha the border windows draw black where they are transparent (macOS).
            .with_transparent(true)
            .with_always_on_top()
            .with_resizable(false)
            .with_inner_size([TOOLBAR_POINTS.0 as f32, TOOLBAR_POINTS.1 as f32]),
        ..Default::default()
    };
    eframe::run_native(
        "recdr",
        options,
        Box::new(move |creation| {
            let gui = Gui::new(app);
            for action in actions {
                gui.perform(&creation.egui_ctx, action);
            }
            Ok(Box::new(gui))
        }),
    )
    .map_err(|error| Error::Io(std::io::Error::other(error.to_string())))
}

struct Gui {
    app: state::App,
    sender: Sender<Event>,
    receiver: Receiver<Event>,
    /// The picture on show and the stored frame it came from.
    picture: Option<(PathBuf, egui::TextureHandle)>,
    quitting: bool,
    /// Whether the main window is dressed as the editor; None before the first pass.
    dressed_as_editor: Option<bool>,
}

impl Gui {
    fn new(app: state::App) -> Gui {
        let (sender, receiver) = channel();
        Gui {
            app,
            sender,
            receiver,
            picture: None,
            quitting: false,
            dressed_as_editor: None,
        }
    }

    fn perform(&self, ctx: &egui::Context, action: Action) {
        if matches!(action, Action::Quit) {
            return;
        }
        let (out, sender, ctx) = (self.app.out.clone(), self.sender.clone(), ctx.clone());
        std::thread::spawn(move || {
            if let Some(event) = worker::perform(&out, action) {
                // The receiver is gone only when the app has closed.
                let _ = sender.send(event);
                ctx.request_repaint();
            }
        });
    }

    /// The main window is a bare toolbar over the frame, or a normal window for the editor.
    fn dress(&mut self, ctx: &egui::Context) {
        let editing = matches!(self.app.screen, Screen::Editing(_));
        if self.dressed_as_editor == Some(editing) {
            return;
        }
        self.dressed_as_editor = Some(editing);
        let (size, level) = if editing {
            (EDITOR_POINTS, egui::WindowLevel::Normal)
        } else {
            (
                (TOOLBAR_POINTS.0 as f32, TOOLBAR_POINTS.1 as f32),
                egui::WindowLevel::AlwaysOnTop,
            )
        };
        ctx.send_viewport_cmd(egui::ViewportCommand::Decorations(editing));
        ctx.send_viewport_cmd(egui::ViewportCommand::Resizable(editing));
        ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(level));
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size.into()));
        // The toolbar sits at the screen's edge, so the editor would run off it.
        if editing && let Some(monitor) = geometry::monitor_of(&self.app.inside, &self.app.monitors)
        {
            let at = |pixels: i32, points: f32| {
                geometry::to_points(f64::from(pixels), self.app.scale) as f32 - points / 2.0
            };
            let b = &monitor.bounds;
            let center = (b.x + b.width as i32 / 2, b.y + b.height as i32 / 2);
            let corner = egui::pos2(at(center.0, size.0), at(center.1, size.1));
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(corner));
        }
    }

    /// Loads the stored frame named by the screen into a texture, once per path.
    fn load_picture(&mut self, ctx: &egui::Context) {
        let Screen::Editing(edit) = &self.app.screen else {
            self.picture = None;
            return;
        };
        let Some((_, path)) = &edit.picture else {
            return;
        };
        if self
            .picture
            .as_ref()
            .is_some_and(|(shown, _)| shown == path)
        {
            return;
        }
        // A frame that cannot be read leaves the last picture on show.
        if let Ok(image) = image::open(path) {
            let rgba = image.to_rgba8();
            let size = [rgba.width() as usize, rgba.height() as usize];
            let color = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
            let texture = ctx.load_texture("picture", color, egui::TextureOptions::LINEAR);
            self.picture = Some((path.clone(), texture));
        }
    }
}

impl eframe::App for Gui {
    /// Every way out ends here, also the ones that send no close request first: Quit
    /// from the Dock or the app menu on macOS ends the event loop straight away. A
    /// recording still running is stopped here, as a close request would.
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if self.quitting {
            return;
        }
        for action in state::step(&mut self.app, Event::CloseRequested, 0) {
            if matches!(action, Action::Stop(_)) {
                let _ = worker::perform(&self.app.out, action);
            }
        }
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        // The border windows are transparent but for the line they draw.
        [0.0; 4]
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let now_ms = (ctx.input(|input| input.time) * 1000.0) as u64;
        let mut events: Vec<Event> = self.receiver.try_iter().collect();
        let scale = f64::from(ctx.pixels_per_point());
        if scale != self.app.scale {
            events.push(Event::Scale(scale));
        }
        if ctx.input(|input| input.viewport().close_requested()) && !self.quitting {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            events.push(Event::CloseRequested);
        }
        self.dress(&ctx);
        self.load_picture(&ctx);
        events.extend(match &self.app.screen {
            Screen::Editing(_) => editor::show(
                ui,
                &self.app,
                self.picture.as_ref().map(|(_, texture)| texture),
            ),
            _ => overlay::show(ui, &self.app, now_ms),
        });
        events.push(Event::Tick);
        for event in events {
            for action in state::step(&mut self.app, event, now_ms) {
                if matches!(action, Action::Quit) {
                    self.quitting = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                } else {
                    self.perform(&ctx, action);
                }
            }
        }
        let moving = match &self.app.screen {
            Screen::Recording { .. } => true,
            Screen::Editing(edit) => edit.playing,
            Screen::Idle { .. } => false,
        };
        if moving {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }
}
