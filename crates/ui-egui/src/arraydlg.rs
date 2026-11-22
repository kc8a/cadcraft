//! The Array dialog (ARRAY, ARRAYRECT, ARRAYPOLAR): one window with the rectangular and polar
//! settings. It never edits the drawing itself: a centre pick goes through HATCHPICK, and OK runs
//! one ARRAYRECT or ARRAYPOLAR call on the current selection.

use cadcraft_doc::{Handle, Space};
use serde_json::{Value, json};

use crate::CadApp;
use crate::theme::Tokens;

/// Which array the dialog builds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ArrayKind {
    #[default]
    Rectangular,
    Polar,
}

/// The dialog while open (and while a centre pick it started runs).
#[derive(Clone, Debug, PartialEq)]
pub struct ArrayDlgState {
    pub kind: ArrayKind,
    pub rows: u32,
    pub cols: u32,
    pub col_spacing: f64,
    pub row_spacing: f64,
    pub axis_angle: f64,
    pub levels: u32,
    pub level_spacing: f64,
    pub associative: bool,
    pub count: u32,
    pub fill_angle: f64,
    pub rotate: bool,
    pub center: [f64; 2],
    /// A HATCHPICK (oRigin) is running.
    pub picking: bool,
    /// The last command error, shown in the dialog.
    pub error: Option<String>,
    /// The drawing the dialog belongs to (`DocState::uid`).
    pub uid: u64,
    /// The space it belongs to (`DocState::edit_space`).
    pub space: Space,
}

/// The centre of the selection's union bounds, or `[0.0, 0.0]` when empty or non-finite.
fn selection_center(doc: &cadcraft_doc::Drawing, handles: &[Handle]) -> [f64; 2] {
    let mut b = cadcraft_geom::Bounds2::EMPTY;
    for h in handles {
        if let Some(e) = doc.entity(*h) {
            b = b.union(&cadcraft_doc::entity_bounds(doc, e, 0));
        }
    }
    let c = b.center();
    if c.x.is_finite() && c.y.is_finite() { [c.x, c.y] } else { [0.0, 0.0] }
}

/// Open the window on the current drawing's selection.
pub fn open(app: &mut CadApp, kind: ArrayKind) {
    let Ok(st) = app.session.state() else { return };
    let mut dlg = ArrayDlgState {
        kind,
        rows: 3,
        cols: 4,
        col_spacing: 1.0,
        row_spacing: 1.0,
        axis_angle: 0.0,
        levels: 1,
        level_spacing: 1.0,
        associative: st.doc.header.i64("ARRAYASSOCIATIVITY", 1) != 0,
        count: 6,
        fill_angle: 360.0,
        rotate: true,
        center: [0.0, 0.0],
        picking: false,
        error: None,
        uid: st.uid,
        space: st.edit_space(),
    };
    if kind == ArrayKind::Polar {
        let handles = st.shown_selection();
        dlg.center = selection_center(&st.doc, &handles);
    }
    app.array_dlg = Some(dlg);
    app.ui.dialog = Some("array".into());
}

/// Hide the dialog and run HATCHPICK for the centre; [`resume`] brings the dialog back.
fn start_pick(app: &mut CadApp) {
    let Some(st) = app.array_dlg.as_mut() else { return };
    st.picking = true;
    st.error = None;
    app.session.hatch_picks = None;
    app.ui.dialog = None;
    app.start("hatchpick");
    if app.session.running.is_some()
        && let Err(e) = app.session.input(cadcraft_engine::Input::Keyword("oRigin".into()))
    {
        app.session.echo(e.to_string());
    }
}

/// Every frame: restore the window after its centre pick ended.
pub fn resume(app: &mut CadApp) {
    if !app.array_dlg.as_ref().is_some_and(|s| s.picking) {
        return;
    }
    if app.session.running.is_some() {
        return;
    }
    let picks = app.session.hatch_picks.take();
    let here = app.session.state().ok().map(|s| (s.uid, s.edit_space()));
    let Some(mut st) = app.array_dlg.take() else { return };
    if here != Some((st.uid, st.space.clone())) {
        return;
    }
    st.picking = false;
    if let Some(q) = picks.and_then(|p| p.origin) {
        st.center = [q.x, q.y];
        st.error = None;
    }
    app.array_dlg = Some(st);
    app.ui.dialog = Some("array".into());
}

/// (command id, JSON params) for the state and handles.
fn params(st: &ArrayDlgState, handles: &[Handle]) -> (&'static str, Value) {
    let hx = json!(handles.iter().map(|h| h.hex()).collect::<Vec<_>>());
    match st.kind {
        ArrayKind::Rectangular => (
            "arrayrect",
            json!({
                "handles": hx,
                "rows": st.rows,
                "cols": st.cols,
                "colSpacing": st.col_spacing,
                "rowSpacing": st.row_spacing,
                "axisAngle": st.axis_angle,
                "levels": st.levels,
                "levelSpacing": st.level_spacing,
                "associative": st.associative,
            }),
        ),
        ArrayKind::Polar => (
            "arraypolar",
            json!({
                "handles": hx,
                "center": [st.center[0], st.center[1]],
                "count": st.count,
                "angle": st.fill_angle,
                "rotate": st.rotate,
                "rows": st.rows,
                "rowSpacing": st.row_spacing,
                "levels": st.levels,
                "levelSpacing": st.level_spacing,
                "associative": st.associative,
            }),
        ),
    }
}

/// The numbers `ok` refuses when any is not finite.
fn all_finite(st: &ArrayDlgState) -> bool {
    [st.col_spacing, st.row_spacing, st.axis_angle, st.level_spacing, st.fill_angle, st.center[0], st.center[1]].iter().all(|v| v.is_finite())
}

/// OK: run ARRAYRECT/ARRAYPOLAR. True when the dialog closes.
pub fn ok(app: &mut CadApp) -> bool {
    let Some(handles) = app.session.state().ok().map(|s| s.shown_selection()) else { return true };
    let Some(st) = app.array_dlg.as_ref() else { return true };
    if handles.is_empty() {
        if let Some(s) = app.array_dlg.as_mut() {
            s.error = Some("Select objects first".into());
        }
        return false;
    }
    if !all_finite(st) {
        if let Some(s) = app.array_dlg.as_mut() {
            s.error = Some("Values must be finite".into());
        }
        return false;
    }
    let (cmd, p) = params(st, &handles);
    match app.run(cmd, p) {
        Ok(_) => {
            app.array_dlg = None;
            true
        }
        Err(e) => {
            if let Some(s) = app.array_dlg.as_mut() {
                s.error = Some(e);
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

/// Draw the window when `app.ui.dialog == Some("array")`.
pub fn dialog(app: &mut CadApp, ctx: &egui::Context, open: &mut bool) {
    if app.array_dlg.is_none() {
        return;
    }
    let here = app.session.state().ok().map(|s| (s.uid, s.edit_space()));
    let Some(mut st) = app.array_dlg.take().filter(|s| here.as_ref() == Some(&(s.uid, s.space.clone()))) else {
        *open = false;
        return;
    };
    let handles = app.session.state().map(|s| s.shown_selection()).unwrap_or_default();
    let mut action = Action::None;
    egui::Window::new(crate::tl!("Array")).id(egui::Id::unique("array_dialog")).open(open).collapsible(false).resizable(false).show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut st.kind, ArrayKind::Rectangular, crate::tl!("Rectangular"));
            ui.selectable_value(&mut st.kind, ArrayKind::Polar, crate::tl!("Polar"));
        });
        ui.separator();
        match st.kind {
            ArrayKind::Rectangular => {
                egui::Grid::new("array_rect").num_columns(2).spacing(egui::vec2(12.0, 6.0)).show(ui, |ui| {
                    ui.label(crate::tl!("Rows"));
                    count_field(ui, &mut st.rows);
                    ui.end_row();
                    ui.label(crate::tl!("Columns"));
                    count_field(ui, &mut st.cols);
                    ui.end_row();
                    ui.label(crate::tl!("Column spacing"));
                    ui.add(egui::DragValue::new(&mut st.col_spacing).speed(0.1));
                    ui.end_row();
                    ui.label(crate::tl!("Row spacing"));
                    ui.add(egui::DragValue::new(&mut st.row_spacing).speed(0.1));
                    ui.end_row();
                    ui.label(crate::tl!("Axis angle"));
                    ui.add(egui::DragValue::new(&mut st.axis_angle).speed(1.0).suffix("°"));
                    ui.end_row();
                    ui.label(crate::tl!("Levels"));
                    count_field(ui, &mut st.levels);
                    ui.end_row();
                    ui.label(crate::tl!("Level spacing"));
                    ui.add(egui::DragValue::new(&mut st.level_spacing).speed(0.1));
                    ui.end_row();
                });
                ui.checkbox(&mut st.associative, crate::tl!("Associative"));
            }
            ArrayKind::Polar => {
                egui::Grid::new("array_polar").num_columns(2).spacing(egui::vec2(12.0, 6.0)).show(ui, |ui| {
                    ui.label(crate::tl!("Center X"));
                    ui.add(egui::DragValue::new(&mut st.center[0]).speed(0.1));
                    ui.end_row();
                    ui.label(crate::tl!("Center Y"));
                    ui.add(egui::DragValue::new(&mut st.center[1]).speed(0.1));
                    ui.end_row();
                    ui.label("");
                    if ui.button(crate::tl!("Pick center")).clicked() {
                        action = Action::PickCenter;
                    }
                    ui.end_row();
                    ui.label(crate::tl!("Items"));
                    count_field(ui, &mut st.count);
                    ui.end_row();
                    ui.label(crate::tl!("Fill angle"));
                    ui.add(egui::DragValue::new(&mut st.fill_angle).speed(1.0).suffix("°"));
                    ui.end_row();
                    ui.label("");
                    ui.checkbox(&mut st.rotate, crate::tl!("Rotate items"));
                    ui.end_row();
                    ui.label(crate::tl!("Rows"));
                    count_field(ui, &mut st.rows);
                    ui.end_row();
                    ui.label(crate::tl!("Row spacing"));
                    ui.add(egui::DragValue::new(&mut st.row_spacing).speed(0.1));
                    ui.end_row();
                    ui.label(crate::tl!("Levels"));
                    count_field(ui, &mut st.levels);
                    ui.end_row();
                    ui.label(crate::tl!("Level spacing"));
                    ui.add(egui::DragValue::new(&mut st.level_spacing).speed(0.1));
                    ui.end_row();
                });
                ui.checkbox(&mut st.associative, crate::tl!("Associative"));
            }
        }
        if let Some(e) = &st.error {
            ui.colored_label(Tokens::get().warn, e);
        }
        ui.separator();
        ui.horizontal(|ui| {
            let ready = !handles.is_empty();
            if ui.add_enabled(ready, egui::Button::new(crate::tl!("OK"))).clicked() {
                action = Action::Ok;
            }
            if ui.button(crate::tl!("Cancel")).clicked() {
                action = Action::Cancel;
            }
            if handles.is_empty() {
                ui.label(crate::tl!("Select objects first"));
            }
        });
    });
    app.array_dlg = Some(st);
    match action {
        Action::Ok => {
            if ok(app) {
                *open = false;
            }
        }
        Action::Cancel => {
            app.array_dlg = None;
            *open = false;
        }
        Action::PickCenter => start_pick(app),
        Action::None => {
            if !*open {
                app.array_dlg = None;
            }
        }
    }
}

#[cfg(test)]
mod tests;
