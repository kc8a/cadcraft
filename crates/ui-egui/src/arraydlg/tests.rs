use cadcraft_doc::{Handle, Space};
use cadcraft_engine::Session;
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
fn open_defaults_and_draws() {
    let (mut app, r) = app_with_rect();
    app.session.set_selection(vec![r]);
    open(&mut app, ArrayKind::Rectangular);
    assert_eq!(app.ui.dialog.as_deref(), Some("array"));
    let st = app.array_dlg.clone().unwrap();
    assert_eq!(st.kind, ArrayKind::Rectangular);
    assert_eq!((st.rows, st.cols, st.levels, st.count), (3, 4, 1, 6));
    assert_eq!((st.col_spacing, st.row_spacing, st.axis_angle, st.level_spacing), (1.0, 1.0, 0.0, 1.0));
    assert_eq!((st.fill_angle, st.rotate), (360.0, true));
    assert_eq!(st.center, [0.0, 0.0], "rectangular leaves the centre at zero");
    assert!(st.associative);
    assert!(!st.picking);
}

#[test]
fn open_polar_centres_on_the_selection() {
    let (mut app, r) = app_with_rect();
    app.session.set_selection(vec![r]);
    open(&mut app, ArrayKind::Polar);
    assert_eq!(app.array_dlg.clone().unwrap().center, [5.0, 5.0]);
    // An empty selection gives [0, 0].
    let (mut app, _) = app_with_rect();
    open(&mut app, ArrayKind::Polar);
    assert_eq!(app.array_dlg.clone().unwrap().center, [0.0, 0.0]);
}

#[test]
fn open_reads_the_header_for_associative() {
    let (mut s, _) = session_with_rect();
    s.execute("setvar", &json!({ "name": "ARRAYASSOCIATIVITY", "value": 0 })).unwrap();
    let mut app = CadApp::new(s, Services::default());
    open(&mut app, ArrayKind::Rectangular);
    assert!(!app.array_dlg.clone().unwrap().associative);
}

#[test]
fn open_without_a_drawing_changes_nothing() {
    let mut app = CadApp::new(Session::empty(), Services::default());
    app.ui.dialog = Some("layers".into());
    open(&mut app, ArrayKind::Rectangular);
    assert_eq!(app.ui.dialog.as_deref(), Some("layers"));
    assert!(app.array_dlg.is_none());
}

#[test]
fn params_rectangular_and_polar() {
    let mut st = ArrayDlgState {
        kind: ArrayKind::Rectangular,
        rows: 3,
        cols: 4,
        col_spacing: 2.0,
        row_spacing: 1.5,
        axis_angle: 30.0,
        levels: 2,
        level_spacing: 0.5,
        associative: true,
        count: 6,
        fill_angle: 360.0,
        rotate: true,
        center: [1.0, 2.0],
        picking: false,
        error: None,
        uid: 1,
        space: Space::Model,
    };
    let h = Handle::parse_hex("1F").unwrap();
    let (cmd, p) = params(&st, &[h]);
    assert_eq!(cmd, "arrayrect");
    assert_eq!(p["handles"], json!([h.hex()]));
    assert_eq!((p["rows"].as_u64(), p["cols"].as_u64(), p["levels"].as_u64()), (Some(3), Some(4), Some(2)));
    assert_eq!(
        (p["colSpacing"].as_f64(), p["rowSpacing"].as_f64(), p["axisAngle"].as_f64(), p["levelSpacing"].as_f64()),
        (Some(2.0), Some(1.5), Some(30.0), Some(0.5))
    );
    assert_eq!(p["associative"], json!(true));
    assert!(p.get("center").is_none() && p.get("count").is_none() && p.get("base").is_none() && p.get("keyPoint").is_none());

    st.kind = ArrayKind::Polar;
    st.center = [4.0, 5.0];
    st.count = 6;
    st.fill_angle = 180.0;
    st.rotate = false;
    let (cmd, p) = params(&st, &[h]);
    assert_eq!(cmd, "arraypolar");
    assert_eq!(p["center"], json!([4.0, 5.0]));
    assert_eq!(p["count"], json!(6));
    assert_eq!(p["angle"], json!(180.0));
    assert_eq!(p["rotate"], json!(false));
    assert_eq!(p["rowSpacing"], json!(1.5));
    assert!(p.get("cols").is_none() && p.get("colSpacing").is_none() && p.get("base").is_none());
}

#[test]
fn ok_with_no_state_returns_true() {
    let mut app = CadApp::new(Session::new(), Services::default());
    assert!(ok(&mut app));
}

#[test]
fn ok_with_an_empty_selection_keeps_the_dialog_with_an_error() {
    let (mut app, _) = app_with_rect();
    open(&mut app, ArrayKind::Rectangular);
    let n = app.session.doc().unwrap().model.len();
    assert!(!ok(&mut app));
    let st = app.array_dlg.clone().unwrap();
    assert_eq!(st.error.as_deref(), Some("Select objects first"));
    assert_eq!(app.session.doc().unwrap().model.len(), n, "drawing unchanged");
}

#[test]
fn ok_with_a_non_finite_value_keeps_the_dialog() {
    let (mut app, r) = app_with_rect();
    app.session.set_selection(vec![r]);
    open(&mut app, ArrayKind::Rectangular);
    if let Some(st) = app.array_dlg.as_mut() {
        st.row_spacing = f64::NAN;
    }
    let n = app.session.doc().unwrap().model.len();
    assert!(!ok(&mut app));
    assert_eq!(app.array_dlg.clone().unwrap().error.as_deref(), Some("Values must be finite"));
    assert_eq!(app.session.doc().unwrap().model.len(), n, "drawing unchanged");
}

#[test]
fn ok_runs_and_makes_copies() {
    let (mut app, r) = app_with_rect();
    app.session.set_selection(vec![r]);
    open(&mut app, ArrayKind::Rectangular);
    if let Some(st) = app.array_dlg.as_mut() {
        st.associative = false;
    }
    let before = app.session.doc().unwrap().model.len();
    assert!(ok(&mut app));
    assert!(app.array_dlg.is_none());
    assert!(app.session.doc().unwrap().model.len() > before, "copies created");
}

#[test]
fn ok_refuses_a_zero_fill_angle_with_the_engine_error() {
    let (mut app, r) = app_with_rect();
    app.session.set_selection(vec![r]);
    open(&mut app, ArrayKind::Polar);
    if let Some(st) = app.array_dlg.as_mut() {
        st.fill_angle = 0.0;
    }
    let n = app.session.doc().unwrap().model.len();
    assert!(!ok(&mut app));
    let st = app.array_dlg.clone().unwrap();
    assert!(st.error.as_deref().is_some_and(|e| e.to_lowercase().contains("fill angle")), "{:?}", st.error);
    assert_eq!(app.session.doc().unwrap().model.len(), n, "drawing unchanged");
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
    let (mut app, r) = app_with_rect();
    app.session.set_selection(vec![r]);
    open(&mut app, ArrayKind::Rectangular);
    frames(&mut app);
    assert_eq!(app.ui.dialog.as_deref(), Some("array"));
    if let Some(st) = app.array_dlg.as_mut() {
        st.kind = ArrayKind::Polar;
    }
    frames(&mut app);
    assert_eq!(app.ui.dialog.as_deref(), Some("array"));
}

#[test]
fn dialog_drops_on_a_layout_switch() {
    let (mut app, r) = app_with_rect();
    app.session.set_selection(vec![r]);
    open(&mut app, ArrayKind::Rectangular);
    app.run("layout", json!({ "option": "set", "name": "Layout1" })).unwrap();
    frames(&mut app);
    assert_eq!(app.array_dlg, None);
}

#[test]
fn pick_center_hides_the_dialog_and_restores_it_with_the_origin() {
    let (mut app, r) = app_with_rect();
    app.session.set_selection(vec![r]);
    open(&mut app, ArrayKind::Polar);
    let before = app.array_dlg.clone().unwrap();
    // Start the pick the way the "Pick center" button does.
    if let Some(st) = app.array_dlg.as_mut() {
        st.picking = true;
    }
    app.session.hatch_picks = None;
    app.ui.dialog = None;
    app.start("hatchpick");
    app.session.input(cadcraft_engine::Input::Keyword("oRigin".into())).unwrap();
    assert_eq!(app.ui.dialog, None, "hidden while picking");
    resume(&mut app);
    assert_eq!(app.ui.dialog, None, "still picking");
    app.session.input(cadcraft_engine::Input::Point(Vec2::new(3.0, 4.0))).unwrap();
    resume(&mut app);
    assert_eq!(app.ui.dialog.as_deref(), Some("array"));
    let st = app.array_dlg.clone().unwrap();
    assert!(!st.picking);
    assert_eq!(st.center, [3.0, 4.0]);
    assert_eq!(st.rows, before.rows, "the other fields are kept");
}

#[test]
fn pick_center_cancelled_keeps_the_center() {
    let (mut app, r) = app_with_rect();
    app.session.set_selection(vec![r]);
    open(&mut app, ArrayKind::Polar);
    let before = app.array_dlg.clone().unwrap();
    if let Some(st) = app.array_dlg.as_mut() {
        st.picking = true;
    }
    app.session.hatch_picks = None;
    app.ui.dialog = None;
    app.start("hatchpick");
    app.session.input(cadcraft_engine::Input::Keyword("oRigin".into())).unwrap();
    app.session.cancel();
    resume(&mut app);
    assert_eq!(app.ui.dialog.as_deref(), Some("array"));
    assert_eq!(app.array_dlg.clone().unwrap().center, before.center);
}

#[test]
fn pick_center_on_another_drawing_drops_the_state() {
    let (mut app, r) = app_with_rect();
    app.session.set_selection(vec![r]);
    open(&mut app, ArrayKind::Polar);
    if let Some(st) = app.array_dlg.as_mut() {
        st.picking = true;
    }
    app.ui.dialog = None;
    app.session.new_drawing(false);
    resume(&mut app);
    assert_eq!(app.array_dlg, None);
    assert_eq!(app.ui.dialog, None);
}

#[test]
fn typed_and_menu_commands_open_the_dialog_but_dash_and_json_do_not() {
    let (mut app, r) = app_with_rect();
    app.session.set_selection(vec![r]);
    for (typed, kind) in
        [("array", ArrayKind::Rectangular), ("ar", ArrayKind::Rectangular), ("arrayrect", ArrayKind::Rectangular), ("arraypolar", ArrayKind::Polar)]
    {
        app.ui.dialog = None;
        app.array_dlg = None;
        app.cmdline(typed);
        assert_eq!(app.ui.dialog.as_deref(), Some("array"), "{typed}");
        assert_eq!(app.array_dlg.clone().unwrap().kind, kind, "{typed}");
        assert!(app.session.running.is_none(), "{typed} must not start the command line");
    }
    app.ui.dialog = None;
    app.start("arrayrect");
    assert_eq!(app.ui.dialog.as_deref(), Some("array"), "menu");
    // -ARRAY: the command line.
    app.ui.dialog = None;
    app.array_dlg = None;
    app.cmdline("-array");
    assert_eq!(app.ui.dialog, None);
    assert!(app.session.running.is_some());
    app.session.cancel();
    // JSON: runs, never opens.
    app.ui.dialog = None;
    app.array_dlg = None;
    app.run("arrayrect", json!({ "handles": [r.hex()], "rows": 2, "cols": 2, "associative": false })).unwrap();
    assert_eq!(app.ui.dialog, None);
    assert!(app.array_dlg.is_none());
}

#[test]
fn params_never_fails_on_state_built_with_zero_counts() {
    // The widgets clamp to 1..=1_000_000; a state built directly with 0 shows the engine error.
    let (mut app, r) = app_with_rect();
    app.session.set_selection(vec![r]);
    open(&mut app, ArrayKind::Rectangular);
    if let Some(st) = app.array_dlg.as_mut() {
        st.rows = 0;
    }
    assert!(!ok(&mut app));
    assert!(app.array_dlg.clone().unwrap().error.as_deref().is_some_and(|e| !e.is_empty()));
}
