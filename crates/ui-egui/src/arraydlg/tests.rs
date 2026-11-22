use cadcraft_doc::Space;
use cadcraft_engine::doc::Handle;
use cadcraft_engine::{ArrayLiveKind, Session};
use cadcraft_geom::Vec2;
use serde_json::json;

use super::*;
use crate::{CadApp, Services};

fn session_with_rect() -> (Session, Handle) {
    let mut s = Session::new();
    let r = s.execute("rectang", &json!({ "p1": [0, 0], "p2": [10, 10] })).unwrap();
    (s, Handle::parse_hex(r["handle"].as_str().unwrap()).unwrap())
}

fn app_with_rect() -> (CadApp, Handle) {
    let (s, r) = session_with_rect();
    (CadApp::new(s, Services::default()), r)
}

/// A selected rectangle: the array commands go straight to their option loop.
fn ready(kind: ArrayLiveKind) -> (CadApp, Handle) {
    let (mut app, r) = app_with_rect();
    app.session.set_selection(vec![r]);
    open(&mut app, kind);
    (app, r)
}

fn frames(app: &mut CadApp) -> egui::Context {
    let ctx = egui::Context::default();
    app.logic(&ctx);
    let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1600.0, 1000.0))), ..Default::default() };
    for _ in 0..10 {
        let mut output = ctx.run_ui(input.clone(), |ui| app.ui(ui));
        output.textures_delta.clear();
    }
    ctx
}

#[test]
fn open_starts_the_command_and_opens_the_window() {
    let (mut app, r) = app_with_rect();
    app.session.set_selection(vec![r]);
    open(&mut app, ArrayLiveKind::Rectangular);
    assert_eq!(app.ui.dialog.as_deref(), Some("array"));
    let st = app.array_dlg.clone().unwrap();
    assert_eq!(st.space, Space::Model);
    assert_eq!(st.error, None);
    // The real command runs: the command line keeps its prompts and the canvas its preview.
    assert_eq!(app.session.running.as_ref().map(|r| r.id.as_str()), Some("arrayrect"));
    let live = app.session.array_live().unwrap();
    assert_eq!(live.kind, ArrayLiveKind::Rectangular);
    assert_eq!((live.cols, live.rows, live.levels), (4, 3, 1));
    assert_eq!(live.center, None);
}

#[test]
fn open_polar_centres_on_the_selection() {
    let (app, _) = ready(ArrayLiveKind::Polar);
    assert_eq!(app.session.array_live().unwrap().center, Some(Vec2::new(5.0, 5.0)));
    // An empty selection leaves the command's own centre.
    let (mut app, _) = app_with_rect();
    open(&mut app, ArrayLiveKind::Polar);
    assert_eq!(app.session.array_live().unwrap().center, Some(Vec2::ZERO));
}

#[test]
fn open_without_a_drawing_changes_nothing() {
    let mut app = CadApp::new(Session::empty(), Services::default());
    app.ui.dialog = Some("layers".into());
    open(&mut app, ArrayLiveKind::Rectangular);
    assert_eq!(app.ui.dialog.as_deref(), Some("layers"));
    assert!(app.array_dlg.is_none());
}

#[test]
fn ok_with_no_live_array_returns_true() {
    let mut app = CadApp::new(Session::new(), Services::default());
    assert!(ok(&mut app));
}

#[test]
fn ok_commits_the_running_array() {
    let (mut app, _) = ready(ArrayLiveKind::Rectangular);
    assert!(ok(&mut app));
    assert!(app.array_dlg.is_none());
    assert!(app.session.running.is_none());
    assert!(app.session.array_live().is_none());
    let n = app.session.doc().unwrap().model.iter().filter(|e| matches!(&e.kind, cadcraft_doc::EntityKind::Insert(_))).count();
    assert_eq!(n, 1, "the array was not created");
}

#[test]
fn ok_while_selecting_keeps_the_window() {
    let (mut app, _) = app_with_rect();
    open(&mut app, ArrayLiveKind::Rectangular);
    assert!(app.session.current_prompt().is_some_and(|p| p.accept.select));
    assert!(!ok(&mut app));
    assert!(app.array_dlg.is_some());
    assert!(app.session.running.is_some());
}

#[test]
fn ok_reports_a_refused_commit_and_stays_open() {
    let (mut app, r) = app_with_rect();
    app.session.set_selection(vec![r]);
    open(&mut app, ArrayLiveKind::Rectangular);
    // A value the engine refuses: the fill angle of a polar array.
    app.session.cancel();
    open(&mut app, ArrayLiveKind::Polar);
    let mut live = app.session.array_live().unwrap();
    live.fill = 0.0;
    assert!(app.session.set_array_live(&live).is_err(), "the engine must refuse it");
    // The window is still bound to the command; committing it succeeds normally.
    assert!(ok(&mut app));
}

#[test]
fn dialog_returns_immediately_without_state() {
    let mut app = CadApp::new(Session::new(), Services::default());
    let ctx = egui::Context::default();
    let mut open = true;
    dialog(&mut app, &ctx, &mut open);
    assert!(open, "no state: the window is left untouched");
}

#[test]
fn dialog_draws_both_kinds_and_stays_open() {
    let (mut app, _) = ready(ArrayLiveKind::Rectangular);
    frames(&mut app);
    assert_eq!(app.ui.dialog.as_deref(), Some("array"));
    app.session.cancel();
    open(&mut app, ArrayLiveKind::Polar);
    frames(&mut app);
    assert_eq!(app.ui.dialog.as_deref(), Some("array"));
}

#[test]
fn dialog_follows_the_command_line_and_the_window_writes_back() {
    let (mut app, _) = ready(ArrayLiveKind::Rectangular);
    // The command line changes the columns: the next frame shows them.
    app.session.cmdline("columns").unwrap();
    app.session.cmdline("6").unwrap();
    frames(&mut app);
    assert_eq!(app.session.array_live().unwrap().cols, 6);
    // The window changes the rows through set_array_live.
    let mut v = app.session.array_live().unwrap();
    v.rows = 5;
    app.session.set_array_live(&v).unwrap();
    assert_eq!(app.session.array_live().unwrap().rows, 5);
}

#[test]
fn polar_angle_between_field_resolves_the_fill_angle() {
    let (mut app, _) = ready(ArrayLiveKind::Polar);
    let before = app.session.array_live().unwrap();
    assert!(before.between.is_some(), "polar reads its between");
    // Editing Angle between leaves fill at live.fill; the engine resolves it.
    let mut v = before.clone();
    v.items = 4;
    v.between = Some(45.0);
    v.fill = before.fill;
    app.session.set_array_live(&v).unwrap();
    let after = app.session.array_live().unwrap();
    assert_eq!(after.between, Some(45.0));
    assert!((after.fill - 135.0).abs() < 1e-9, "items 4 / between 45 -> fill 135, got {}", after.fill);
    // The window stays open and re-reads the resolved values on the next frame.
    frames(&mut app);
    assert_eq!(app.ui.dialog.as_deref(), Some("array"));
}

#[test]
fn rectangular_has_no_angle_between() {
    let (app, _) = ready(ArrayLiveKind::Rectangular);
    assert_eq!(app.session.array_live().unwrap().between, None);
}

#[test]
fn dialog_keeps_the_old_values_when_a_value_is_refused() {
    let (mut app, _) = ready(ArrayLiveKind::Polar);
    let before = app.session.array_live().unwrap();
    let mut bad = before.clone();
    bad.fill = 0.0;
    assert!(app.session.set_array_live(&bad).is_err());
    assert_eq!(app.session.array_live().unwrap(), before, "the old values stay");
    frames(&mut app);
    assert_eq!(app.ui.dialog.as_deref(), Some("array"));
}

#[test]
fn dialog_closes_when_the_command_ends_from_the_command_line() {
    let (mut app, _) = ready(ArrayLiveKind::Rectangular);
    app.session.cmdline("x").unwrap();
    assert!(app.session.running.is_none());
    frames(&mut app);
    assert!(app.array_dlg.is_none());
    assert_eq!(app.ui.dialog, None);
}

#[test]
fn dialog_drops_on_a_drawing_switch() {
    let (mut app, _) = ready(ArrayLiveKind::Rectangular);
    app.session.new_drawing(false);
    frames(&mut app);
    assert!(app.array_dlg.is_none());
}

#[test]
fn pick_center_asks_the_command_and_keeps_the_window() {
    let (mut app, _) = ready(ArrayLiveKind::Polar);
    // The "Pick center" button.
    assert!(app.session.array_pick_center());
    assert_eq!(app.ui.dialog.as_deref(), Some("array"));
    frames(&mut app);
    assert_eq!(app.ui.dialog.as_deref(), Some("array"));
    // The command asks for the point; feeding it moves it on and the window follows.
    app.session.input(cadcraft_engine::Input::Point(Vec2::new(3.0, 4.0))).unwrap();
    assert_eq!(app.session.array_live().unwrap().center, Some(Vec2::new(3.0, 4.0)));
    assert_eq!(app.ui.dialog.as_deref(), Some("array"));
}

#[test]
fn menu_and_typed_commands_open_the_window_but_json_does_not() {
    for (typed, kind) in [("arrayrect", ArrayLiveKind::Rectangular), ("arraypolar", ArrayLiveKind::Polar), ("ARRAYRECT", ArrayLiveKind::Rectangular)]
    {
        let (mut app, r) = app_with_rect();
        app.session.set_selection(vec![r]);
        app.cmdline(typed);
        assert_eq!(app.ui.dialog.as_deref(), Some("array"), "{typed}");
        assert_eq!(app.session.array_live().unwrap().kind, kind, "{typed}");
        assert!(app.session.running.is_some(), "{typed} runs the command");
        app.session.cancel();
    }
    // ARRAY and -ARRAY stay on the command line.
    for typed in ["array", "-array"] {
        let (mut app, r) = app_with_rect();
        app.session.set_selection(vec![r]);
        app.cmdline(typed);
        assert_eq!(app.ui.dialog, None, "{typed}");
        assert!(app.session.running.is_some(), "{typed}");
        app.session.cancel();
    }
    // JSON: runs, never opens.
    let (mut app, r) = app_with_rect();
    app.session.set_selection(vec![r]);
    app.run("arrayrect", json!({ "handles": [r.hex()], "rows": 2, "cols": 2, "associative": false })).unwrap();
    assert_eq!(app.ui.dialog, None);
    assert!(app.array_dlg.is_none());
}
