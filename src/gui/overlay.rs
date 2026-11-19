//! The frame and the toolbar: four border windows around the area to record, and the
//! main window as a toolbar beside them.

use eframe::egui::{self, CursorIcon, Id, Sense, Ui, ViewportBuilder, ViewportId};

use super::geometry::{self, BORDER, Edge, GRIP, TOOLBAR_POINTS};
use super::state::{App, Event, Screen};
use crate::model::Rect;

const IDLE: egui::Color32 = egui::Color32::from_rgb(0, 120, 215);
const RECORDING: egui::Color32 = egui::Color32::from_rgb(220, 0, 0);
/// Pixels between the recorded area and the line. On Windows 11 the outermost pixel or
/// two of an undecorated window is covered by the outline egui-winit asks the system to
/// draw (its "undecorated shadow"), so a line at the very edge would not show.
const GAP: u32 = 2;
/// Room for the target menu's list, in points. egui keeps a popup inside its window, so
/// the toolbar window grows by this much while the list is open.
const LIST_POINTS: f32 = 240.0;
/// Where the target menu's widget id is kept between passes.
const MENU: &str = "target-menu";

/// What a drag remembers from its press (decision 13).
#[derive(Clone)]
struct Press {
    inside: Rect,
    pointer: (i32, i32),
    edge: Option<Edge>,
}

pub fn show(ui: &mut Ui, app: &App, now_ms: u64) -> Vec<Event> {
    let mut events = Vec::new();
    let can_change = matches!(
        app.screen,
        Screen::Idle {
            starting: false,
            ..
        }
    );
    let offset = outer_offset(ui);
    for (side, rect) in geometry::borders(&app.inside).into_iter().enumerate() {
        let place = points(&rect, app.scale).translate(-offset);
        border(ui.ctx(), app, side, &rect, place, can_change, &mut events);
    }
    let row = place_toolbar(ui, app, offset);
    toolbar(ui, app, row, now_ms, can_change, &mut events);
    events
}

/// The pointer's position on the screen, in pixels: the window's place on the screen
/// plus the pointer's place in the window.
fn pointer_on_screen(ui: &Ui) -> Option<(i32, i32)> {
    let scale = f64::from(ui.ctx().pixels_per_point());
    ui.input(|input| {
        let inner = input.viewport().inner_rect?;
        let pointer = input.pointer.latest_pos()?;
        Some((
            geometry::to_pixels(f64::from(inner.min.x + pointer.x), scale),
            geometry::to_pixels(f64::from(inner.min.y + pointer.y), scale),
        ))
    })
}

/// How far the inside of the main window is from its outer corner. A window is placed by
/// its outer corner, and on Windows 11 an undecorated window's inside starts a pixel
/// lower, so every window is placed this much up and left of where its inside goes. The
/// border windows have the same style as the main window in the overlay.
fn outer_offset(ui: &Ui) -> egui::Vec2 {
    ui.input(|input| {
        let viewport = input.viewport();
        match (viewport.inner_rect, viewport.outer_rect) {
            (Some(inner), Some(outer)) => inner.min - outer.min,
            _ => egui::Vec2::ZERO,
        }
    })
}

fn points(rect: &Rect, scale: f64) -> egui::Rect {
    let at = |pixels: i64| geometry::to_points(pixels as f64, scale) as f32;
    egui::Rect::from_min_size(
        egui::pos2(at(rect.x.into()), at(rect.y.into())),
        egui::vec2(at(rect.width.into()), at(rect.height.into())),
    )
}

/// One border window: the line along the inside, and the grip that resizes the frame.
fn border(
    ctx: &egui::Context,
    app: &App,
    side: usize,
    rect: &Rect,
    place: egui::Rect,
    can_change: bool,
    events: &mut Vec<Event>,
) {
    let recording = matches!(app.screen, Screen::Recording { .. });
    let builder = ViewportBuilder::default()
        .with_title("recdr frame")
        .with_decorations(false)
        .with_transparent(true)
        .with_always_on_top()
        .with_resizable(false)
        .with_taskbar(false)
        .with_mouse_passthrough(recording)
        .with_position(place.min)
        .with_inner_size(place.size());
    let (width, height) = (app.inside.width, app.inside.height);
    let (b, g, d) = (BORDER as i32, GRIP as i32, GAP as i32);
    let across = width + 2 * (BORDER + GAP);
    let line = match side {
        0 => (g - b - d, g - b - d, across, BORDER),
        1 => (g - b - d, d, across, BORDER),
        2 => (g - b - d, 0, BORDER, height),
        _ => (d, 0, BORDER, height),
    };
    let line = Rect {
        x: line.0,
        y: line.1,
        width: line.2,
        height: line.3,
    };
    let id = ViewportId::from_hash_of(("recdr-border", side));
    ctx.show_viewport_immediate(id, builder, |ui, _| {
        let color = if recording { RECORDING } else { IDLE };
        ui.painter()
            .rect_filled(points(&line, app.scale), 0.0, color);
        if !can_change {
            return;
        }
        let grip = ui.interact(ui.max_rect(), Id::unique(("grip", side)), Sense::drag());
        let length = if side < 2 { rect.width } else { rect.height };
        let along = |position: egui::Pos2| {
            let along = if side < 2 { position.x } else { position.y };
            geometry::to_pixels(f64::from(along), app.scale).max(0) as u32
        };
        let hovered = grip
            .hover_pos()
            .map(|at| geometry::edge_at(side, along(at), length));
        let grip = grip.on_hover_cursor(hovered.map_or(CursorIcon::Default, cursor));
        let key = Id::unique(("press", side));
        if grip.drag_started() {
            let edge = grip
                .interact_pointer_pos()
                .map(|at| geometry::edge_at(side, along(at), length));
            if let Some(pointer) = pointer_on_screen(ui) {
                let press = Press {
                    inside: app.inside,
                    pointer,
                    edge,
                };
                ui.data_mut(|data| data.insert_temp(key, press));
            }
        } else if grip.dragged() {
            let press: Option<Press> = ui.data(|data| data.get_temp(key));
            if let (Some(press), Some(now)) = (press, pointer_on_screen(ui))
                && let Some(edge) = press.edge
            {
                let (dx, dy) = (now.0 - press.pointer.0, now.1 - press.pointer.1);
                events.push(Event::Resized(geometry::resized(
                    &press.inside,
                    edge,
                    dx,
                    dy,
                )));
            }
        }
    });
}

fn cursor(edge: Edge) -> CursorIcon {
    match edge {
        Edge::Top | Edge::Bottom => CursorIcon::ResizeVertical,
        Edge::Left | Edge::Right => CursorIcon::ResizeHorizontal,
        Edge::TopLeft | Edge::BottomRight => CursorIcon::ResizeNwSe,
        Edge::TopRight | Edge::BottomLeft => CursorIcon::ResizeNeSw,
    }
}

/// Whether the target menu's list was open at the end of the last pass.
fn menu_open(ctx: &egui::Context) -> bool {
    ctx.data(|data| data.get_temp::<Id>(Id::unique(MENU)))
        .is_some_and(|id| egui::ComboBox::is_open(ctx, id))
}

/// Puts the main window where `geometry::toolbar_rect` says, grown for the menu's list
/// while that is open, and returns where in the window the toolbar row goes. The window's
/// real place is compared, so it comes back after the editor moved it.
fn place_toolbar(ui: &Ui, app: &App, offset: egui::Vec2) -> egui::Rect {
    let row = egui::vec2(TOOLBAR_POINTS.0 as f32, TOOLBAR_POINTS.1 as f32);
    let Some(monitor) = geometry::monitor_of(&app.inside, &app.monitors) else {
        return egui::Rect::from_min_size(egui::Pos2::ZERO, row);
    };
    let toolbar = geometry::toolbar_size(app.scale);
    let rect = points(
        &geometry::toolbar_rect(&app.inside, &monitor.bounds, toolbar),
        app.scale,
    );
    let room_under = rect.max.y + LIST_POINTS <= points(&monitor.bounds, app.scale).max.y;
    let (window, row_top) = match (menu_open(ui.ctx()), room_under) {
        (false, _) => (rect, 0.0),
        (true, true) => (rect.with_max_y(rect.max.y + LIST_POINTS), 0.0),
        (true, false) => (rect.with_min_y(rect.min.y - LIST_POINTS), LIST_POINTS),
    };
    let at = window.min - offset;
    let (outer, inner) =
        ui.input(|input| (input.viewport().outer_rect, input.viewport().inner_rect));
    if outer.is_none_or(|outer| (outer.min - at).length() > 0.5) {
        ui.ctx()
            .send_viewport_cmd(egui::ViewportCommand::OuterPosition(at));
    }
    if inner.is_none_or(|inner| (inner.size() - window.size()).length() > 0.5) {
        ui.ctx()
            .send_viewport_cmd(egui::ViewportCommand::InnerSize(window.size()));
    }
    egui::Rect::from_min_size(egui::pos2(0.0, row_top), row)
}

fn toolbar(
    ui: &mut Ui,
    app: &App,
    row: egui::Rect,
    now_ms: u64,
    can_change: bool,
    events: &mut Vec<Event>,
) {
    ui.painter()
        .rect_filled(ui.max_rect(), 0.0, ui.visuals().panel_fill);
    if can_change {
        let background = ui.interact(row, Id::unique("toolbar-move"), Sense::drag());
        let key = Id::unique("toolbar-press");
        if background.drag_started() {
            if let Some(pointer) = pointer_on_screen(ui) {
                let press = Press {
                    inside: app.inside,
                    pointer,
                    edge: None,
                };
                ui.data_mut(|data| data.insert_temp(key, press));
            }
        } else if background.dragged() {
            let press: Option<Press> = ui.data(|data| data.get_temp(key));
            if let (Some(press), Some(now)) = (press, pointer_on_screen(ui)) {
                events.push(Event::Moved {
                    x: press.inside.x + now.0 - press.pointer.0,
                    y: press.inside.y + now.1 - press.pointer.1,
                });
            }
        }
    }
    let mut row_ui = ui.new_child(egui::UiBuilder::new().max_rect(row));
    row_ui.horizontal_centered(|ui| {
        ui.add_space(8.0);
        match &app.screen {
            Screen::Recording {
                started_ms,
                stopping,
                ..
            } => {
                let seconds = now_ms.saturating_sub(*started_ms) / 1000;
                ui.label(format!("{}:{:02}", seconds / 60, seconds % 60));
                if ui
                    .add_enabled(!stopping, egui::Button::new("Stop"))
                    .clicked()
                {
                    events.push(Event::StopPressed);
                }
            }
            _ => {
                ui.add_enabled_ui(can_change, |ui| idle(ui, app, events))
                    .inner
            }
        }
        if let Some(message) = &app.message {
            ui.add(egui::Label::new(message.as_str()).truncate())
                .on_hover_text(message.as_str());
        }
    });
}

fn idle(ui: &mut Ui, app: &App, events: &mut Vec<Event>) {
    ui.label(format!("{} x {}", app.inside.width, app.inside.height));
    let chosen = app
        .window
        .and_then(|id| app.windows.iter().find(|window| window.id == id))
        .map_or("Region", |window| window.title.as_str());
    let menu = egui::ComboBox::from_id_salt("target")
        .width(140.0)
        .selected_text(chosen)
        .truncate()
        .show_ui(ui, |ui| {
            if ui
                .selectable_label(app.window.is_none(), "Region")
                .clicked()
            {
                events.push(Event::Picked(None));
            }
            for window in &app.windows {
                let label = ui.selectable_label(app.window == Some(window.id), &window.title);
                if label.clicked() {
                    events.push(Event::Picked(Some(window.id)));
                }
            }
        });
    ui.data_mut(|data| data.insert_temp(Id::unique(MENU), menu.response.id));
    if menu.response.clicked() {
        events.push(Event::MenuOpened);
    }
    let mut fps = app.fps;
    let fps_value = egui::DragValue::new(&mut fps).range(1..=60).suffix(" fps");
    if ui.add(fps_value).changed() {
        events.push(Event::Fps(fps));
    }
    let mut cursor = app.cursor;
    if ui.checkbox(&mut cursor, "Cursor").changed() {
        events.push(Event::Cursor(cursor));
    }
    let mut clicks = app.clicks;
    if ui.checkbox(&mut clicks, "Clicks").changed() {
        events.push(Event::Clicks(clicks));
    }
    if ui.button("Record").clicked() {
        events.push(Event::RecordPressed);
    }
    if ui.button("Close").clicked() {
        events.push(Event::CloseRequested);
    }
}
