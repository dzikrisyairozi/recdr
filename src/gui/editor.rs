//! The editor: the recording's frames with a timeline to trim and a rectangle to crop,
//! and the buttons that save the result.

use std::path::PathBuf;

use eframe::egui::{self, Id, Sense, Ui};

use super::geometry;
use super::state::{App, Edit, Event, Screen};

/// A press this close to a handle, in points, drags the handle.
const HANDLE_REACH: f32 = 6.0;
const TIMELINE_HEIGHT: f32 = 24.0;
/// Room under the picture for the timeline and the row of buttons, in points.
const CONTROLS_HEIGHT: f32 = 80.0;

#[derive(Clone, Copy)]
enum Grab {
    From,
    Until,
    Seek,
}

pub fn show(ui: &mut Ui, app: &App, picture: Option<&egui::TextureHandle>) -> Vec<Event> {
    let mut events = Vec::new();
    let Screen::Editing(edit) = &app.screen else {
        return events;
    };
    if ui.input(|input| input.key_pressed(egui::Key::Space)) {
        events.push(Event::PlayPressed);
    }
    ui.painter()
        .rect_filled(ui.max_rect(), 0.0, ui.visuals().panel_fill);
    egui::Frame::central_panel(ui.style()).show(ui, |ui| {
        preview(ui, edit, picture, &mut events);
        timeline(ui, edit, &mut events);
        buttons(ui, app, edit, &mut events);
    });
    events
}

fn preview(
    ui: &mut Ui,
    edit: &Edit,
    picture: Option<&egui::TextureHandle>,
    events: &mut Vec<Event>,
) {
    let room = ui.available_size() - egui::vec2(0.0, CONTROLS_HEIGHT);
    let picture_size = egui::vec2(edit.width as f32, edit.height as f32);
    let fit = (room.x / picture_size.x)
        .min(room.y / picture_size.y)
        .max(0.01);
    let shown = picture_size * fit;
    let (rect, response) = ui
        .vertical_centered(|ui| ui.allocate_exact_size(shown, Sense::drag()))
        .inner;
    match picture {
        Some(texture) => {
            let whole = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
            ui.painter()
                .image(texture.id(), rect, whole, egui::Color32::WHITE);
        }
        None => {
            ui.painter().rect_filled(rect, 0.0, egui::Color32::BLACK);
        }
    }
    if let Some(crop) = &edit.crop {
        let at = |x: i32, y: i32| rect.min + egui::vec2(x as f32 * fit, y as f32 * fit);
        let right = crop.x + crop.width as i32;
        let bottom = crop.y + crop.height as i32;
        let shape = egui::Rect::from_min_max(at(crop.x, crop.y), at(right, bottom));
        let stroke = egui::Stroke::new(2.0, egui::Color32::from_rgb(255, 200, 0));
        ui.painter()
            .rect_stroke(shape, 0.0, stroke, egui::StrokeKind::Inside);
    }
    let key = Id::unique("crop-start");
    let relative = |at: egui::Pos2| ((at.x - rect.min.x), (at.y - rect.min.y));
    if response.drag_started() {
        if let Some(at) = response.interact_pointer_pos() {
            ui.data_mut(|data| data.insert_temp(key, relative(at)));
        }
    } else if response.dragged() {
        let start: Option<(f32, f32)> = ui.data(|data| data.get_temp(key));
        if let (Some(start), Some(now)) = (start, response.interact_pointer_pos()) {
            let crop = geometry::crop_of(
                start,
                relative(now),
                (shown.x, shown.y),
                (edit.width, edit.height),
            );
            events.push(Event::Crop(crop));
        }
    }
}

fn timeline(ui: &mut Ui, edit: &Edit, events: &mut Vec<Event>) {
    let size = egui::vec2(ui.available_width(), TIMELINE_HEIGHT);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let width = rect.width().max(1.0);
    let x_of = |t_ms: u64| rect.min.x + width * t_ms as f32 / edit.duration_ms.max(1) as f32;
    let painter = ui.painter();
    painter.rect_filled(rect, 2.0, ui.visuals().extreme_bg_color);
    let kept =
        egui::Rect::from_x_y_ranges(x_of(edit.from_ms)..=x_of(edit.until_ms), rect.y_range());
    painter.rect_filled(kept, 2.0, ui.visuals().selection.bg_fill);
    for t_ms in [edit.from_ms, edit.until_ms] {
        let handle = egui::Rect::from_center_size(
            egui::pos2(x_of(t_ms), rect.center().y),
            egui::vec2(4.0, TIMELINE_HEIGHT),
        );
        painter.rect_filled(handle, 1.0, ui.visuals().strong_text_color());
    }
    let position = x_of(edit.position_ms);
    painter.line_segment(
        [
            egui::pos2(position, rect.min.y),
            egui::pos2(position, rect.max.y),
        ],
        egui::Stroke::new(2.0, egui::Color32::from_rgb(220, 0, 0)),
    );
    let time = |at: egui::Pos2| geometry::time_at(at.x - rect.min.x, width, edit.duration_ms);
    let grab_at = |at: egui::Pos2| {
        if (at.x - x_of(edit.from_ms)).abs() <= HANDLE_REACH {
            Grab::From
        } else if (at.x - x_of(edit.until_ms)).abs() <= HANDLE_REACH {
            Grab::Until
        } else {
            Grab::Seek
        }
    };
    let key = Id::unique("timeline-grab");
    let Some(at) = response.interact_pointer_pos() else {
        return;
    };
    let grab = if response.drag_started() || response.clicked() {
        let grab = grab_at(at);
        ui.data_mut(|data| data.insert_temp(key, grab));
        Some(grab)
    } else if response.dragged() {
        ui.data(|data| data.get_temp(key))
    } else {
        None
    };
    match grab {
        Some(Grab::From) => events.push(Event::From(time(at))),
        Some(Grab::Until) => events.push(Event::Until(time(at))),
        Some(Grab::Seek) => events.push(Event::Seek(time(at))),
        None => {}
    }
}

fn buttons(ui: &mut Ui, app: &App, edit: &Edit, events: &mut Vec<Event>) {
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        let play = if edit.playing { "Pause" } else { "Play" };
        if ui.button(play).clicked() {
            events.push(Event::PlayPressed);
        }
        if ui.button("Clear crop").clicked() {
            events.push(Event::Crop(None));
        }
        let seconds = edit.until_ms.saturating_sub(edit.from_ms) as f64 / 1000.0;
        let (width, height) = edit
            .crop
            .map_or((edit.width, edit.height), |crop| (crop.width, crop.height));
        ui.label(format!("{seconds:.1} s, {width} x {height}"));
        ui.separator();
        let idle = !edit.exporting;
        if ui
            .add_enabled(idle, egui::Button::new("Save GIF"))
            .clicked()
            && let Some(file) = save_as("GIF", "gif")
        {
            events.push(Event::Save(file));
        }
        if ui
            .add_enabled(idle, egui::Button::new("Save MP4"))
            .clicked()
            && let Some(file) = save_as("MP4", "mp4")
        {
            events.push(Event::Save(file));
        }
        if ui
            .add_enabled(idle, egui::Button::new("New recording"))
            .clicked()
        {
            events.push(Event::NewRecording);
        }
        let line = if edit.exporting {
            Some("exporting")
        } else {
            app.message.as_deref()
        };
        if let Some(line) = line {
            ui.add(egui::Label::new(line).truncate())
                .on_hover_text(line);
        }
    });
}

/// Asks where to save. The file gets the extension when the person left it off.
fn save_as(kind: &str, extension: &str) -> Option<PathBuf> {
    let file = rfd::FileDialog::new()
        .add_filter(kind, &[extension])
        .set_file_name(format!("export.{extension}"))
        .save_file()?;
    let has_it = file
        .extension()
        .is_some_and(|given| given.eq_ignore_ascii_case(extension));
    Some(if has_it {
        file
    } else {
        let mut name = file.into_os_string();
        name.push(format!(".{extension}"));
        PathBuf::from(name)
    })
}
