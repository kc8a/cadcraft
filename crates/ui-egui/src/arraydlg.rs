//! The Array window (ARRAYRECT, ARRAYPOLAR): a second view of the **running** command. The command
//! line keeps its prompts and the canvas keeps its preview; the window reads and writes the live
//! parameters ([`cadcraft_engine::ArrayLive`]), so an edit on either side shows on the other.

use cadcraft_doc::Space;
use cadcraft_engine::{ArrayLive, ArrayLiveKind};
use cadcraft_geom::Vec2;

use crate::CadApp;
use crate::theme::Tokens;

/// The window while it is open.
#[derive(Clone, Debug, Default)]
pub struct ArrayDlgState {
    /// The last refused value, shown in the window.
    pub error: Option<String>,
    /// The drawing the window belongs to (`DocState::uid`).
    pub uid: u64,
    /// The space it belongs to (`DocState::edit_space`).
    pub space: Space,
}

/// The command each array kind runs.
fn command(kind: ArrayLiveKind) -> &'static str {
    match kind {
        ArrayLiveKind::Rectangular => "arrayrect",
        ArrayLiveKind::Polar => "arraypolar",
    }
}

/// The centre of the selection's union bounds, when it is finite.
fn selection_center(app: &CadApp) -> Option<Vec2> {
    let st = app.session.state().ok()?;
    let mut b = cadcraft_geom::Bounds2::EMPTY;
    for h in st.shown_selection() {
        if let Some(e) = st.doc.entity(h) {
            b = b.union(&cadcraft_doc::entity_bounds(&st.doc, e, 0));
        }
    }
    let c = b.center();
    (c.x.is_finite() && c.y.is_finite()).then_some(c)
}

/// Start the real command and open the window on it.
pub fn open(app: &mut CadApp, kind: ArrayLiveKind) {
    let (uid, space, center) = {
        let Ok(st) = app.session.state() else { return };
        (st.uid, st.edit_space(), selection_center(app))
    };
    // The session, not `CadApp::start`: that one would come back here through the UI commands.
    if let Err(e) = app.session.start(command(kind)) {
        app.session.echo(e.to_string());
        return;
    }
    if kind == ArrayLiveKind::Polar
        && let (Some(c), Some(mut live)) = (center, app.session.array_live())
    {
        live.center = Some(c);
        if let Err(e) = app.session.set_array_live(&live) {
            app.session.echo(e.to_string());
        }
    }
    app.array_dlg = Some(ArrayDlgState { error: None, uid, space });
    app.ui.dialog = Some("array".into());
}

/// OK: finish the running array. True when the window closes.
pub fn ok(app: &mut CadApp) -> bool {
    if app.session.array_live().is_none() {
        return true;
    }
    match app.session.commit_array() {
        Ok(true) => {
            app.array_dlg = None;
            true
        }
        Ok(false) => false,
        Err(e) => {
            if let Some(st) = app.array_dlg.as_mut() {
                st.error = Some(e.to_string());
            }
            false
        }
    }
}

enum Action {
    None,
    Ok,
    Cancel,
    PickCenter,
}

/// A clamped 1..=1_000_000 DragValue.
fn count_field(ui: &mut egui::Ui, v: &mut u32) {
    ui.add(egui::DragValue::new(v).range(1..=1_000_000));
}

/// The fields both kinds have; the counts come back through the caller's locals.
fn common_fields(ui: &mut egui::Ui, rows: &mut u32, levels: &mut u32, live: &mut ArrayLive) {
    egui::Grid::new("array_common").num_columns(2).spacing(egui::vec2(12.0, 6.0)).show(ui, |ui| {
        ui.label(crate::tl!("Rows"));
        count_field(ui, rows);
        ui.end_row();
        ui.label(crate::tl!("Levels"));
        count_field(ui, levels);
        ui.end_row();
        ui.label(crate::tl!("Row spacing"));
        ui.add(egui::DragValue::new(&mut live.row_spacing).speed(0.1));
        ui.end_row();
        ui.label(crate::tl!("Level spacing"));
        ui.add(egui::DragValue::new(&mut live.level_spacing).speed(0.1));
        ui.end_row();
        ui.label(crate::tl!("Row elevation"));
        ui.add(egui::DragValue::new(&mut live.row_elevation).speed(0.1));
        ui.end_row();
    });
    ui.checkbox(&mut live.associative, crate::tl!("Associative"));
}

/// The rectangular-only fields.
fn rect_fields(ui: &mut egui::Ui, cols: &mut u32, live: &mut ArrayLive) {
    egui::Grid::new("array_rect").num_columns(2).spacing(egui::vec2(12.0, 6.0)).show(ui, |ui| {
        ui.label(crate::tl!("Columns"));
        count_field(ui, cols);
        ui.end_row();
        ui.label(crate::tl!("Column spacing"));
        ui.add(egui::DragValue::new(&mut live.col_spacing).speed(0.1));
        ui.end_row();
    });
}

/// The polar-only fields. "Pick center" asks the running command for the point; the window stays.
fn polar_fields(ui: &mut egui::Ui, center: &mut Vec2, items: &mut u32, live: &mut ArrayLive, action: &mut Action) {
    egui::Grid::new("array_polar").num_columns(2).spacing(egui::vec2(12.0, 6.0)).show(ui, |ui| {
        ui.label(crate::tl!("Center X"));
        ui.add(egui::DragValue::new(&mut center.x).speed(0.1));
        ui.end_row();
        ui.label(crate::tl!("Center Y"));
        ui.add(egui::DragValue::new(&mut center.y).speed(0.1));
        ui.end_row();
        ui.label("");
        if ui.button(crate::tl!("Pick center")).clicked() {
            *action = Action::PickCenter;
        }
        ui.end_row();
        ui.label(crate::tl!("Items"));
        count_field(ui, items);
        ui.end_row();
        let mut between = live.between.unwrap_or(0.0);
        ui.label(crate::tl!("Angle between"));
        let resp = ui.add(egui::DragValue::new(&mut between).speed(0.1).suffix("°"));
        if resp.changed() {
            live.between = Some(between);
        }
        ui.end_row();
        ui.label(crate::tl!("Fill angle"));
        ui.add(egui::DragValue::new(&mut live.fill).speed(1.0).suffix("°"));
        ui.end_row();
        ui.label("");
        ui.checkbox(&mut live.rotate, crate::tl!("Rotate items"));
        ui.end_row();
    });
}

/// Draw the window while `app.ui.dialog == Some("array")`.
pub fn dialog(app: &mut CadApp, ctx: &egui::Context, open: &mut bool) {
    let Some(st) = app.array_dlg.clone() else { return };
    let here = app.session.state().ok().map(|s| (s.uid, s.edit_space()));
    let Some(live) = app.session.array_live() else {
        app.array_dlg = None;
        *open = false;
        return;
    };
    if here != Some((st.uid, st.space)) {
        app.array_dlg = None;
        *open = false;
        return;
    }
    let selecting = app.session.current_prompt().is_some_and(|p| p.accept.select);
    let title = match live.kind {
        ArrayLiveKind::Rectangular => crate::tl!("Rectangular Array"),
        ArrayLiveKind::Polar => crate::tl!("Polar Array"),
    };
    let mut edited = live.clone();
    let mut cols = live.cols.min(u64::from(u32::MAX)) as u32;
    let mut rows = live.rows.min(u64::from(u32::MAX)) as u32;
    let mut levels = live.levels.min(u64::from(u32::MAX)) as u32;
    let mut items = live.items.min(u64::from(u32::MAX)) as u32;
    let mut center = live.center.unwrap_or(Vec2::ZERO);
    let mut action = Action::None;
    egui::Window::new(title).id(egui::Id::unique("array_dialog")).open(open).collapsible(false).resizable(false).show(ctx, |ui| {
        match live.kind {
            ArrayLiveKind::Rectangular => rect_fields(ui, &mut cols, &mut edited),
            ArrayLiveKind::Polar => polar_fields(ui, &mut center, &mut items, &mut edited, &mut action),
        }
        common_fields(ui, &mut rows, &mut levels, &mut edited);
        if let Some(e) = &st.error {
            ui.colored_label(Tokens::get().warn, e);
        }
        ui.separator();
        ui.horizontal(|ui| {
            if ui.add_enabled(!selecting, egui::Button::new(crate::tl!("OK"))).clicked() {
                action = Action::Ok;
            }
            if ui.button(crate::tl!("Cancel")).clicked() {
                action = Action::Cancel;
            }
            if selecting {
                ui.label(crate::tl!("Select objects first"));
            }
        });
    });
    edited.cols = u64::from(cols);
    edited.rows = u64::from(rows);
    edited.levels = u64::from(levels);
    edited.items = u64::from(items);
    if live.kind == ArrayLiveKind::Polar && live.center.is_some() {
        edited.center = Some(center);
    }
    // Only the running command draws; a refused value is reported and the old one stays.
    if edited != live {
        match app.session.set_array_live(&edited) {
            Ok(()) => {
                // Re-read so the next frame draws the stored value (e.g. the Fill angle the
                // engine resolved from Angle between); the array kind never changes on update.
                if let Some(l) = app.session.array_live() {
                    debug_assert_eq!(l.kind, edited.kind);
                }
                if let Some(s) = app.array_dlg.as_mut() {
                    s.error = None;
                }
            }
            Err(e) => {
                if let Some(s) = app.array_dlg.as_mut() {
                    s.error = Some(e.to_string());
                }
            }
        }
    }
    match action {
        Action::Ok => {
            if ok(app) {
                *open = false;
            }
        }
        Action::Cancel => {
            app.session.cancel();
            app.array_dlg = None;
            *open = false;
        }
        Action::PickCenter => {
            app.session.array_pick_center();
        }
        // The window's X: Cancel.
        Action::None if !*open => {
            app.session.cancel();
            app.array_dlg = None;
        }
        Action::None => {}
    }
}

#[cfg(test)]
mod tests;
