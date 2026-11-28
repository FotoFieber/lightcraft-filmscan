//! Headless tests of this build's film scan additions: Zap! from the Film Scan panel, the Photo ▸
//! Film Scan ▸ Zap!… dialog, a selection zapped in the background or as one roll, the tool strip
//! button, and the panel fitting the right panel. The conversion's own tests are in
//! `tests/negative.rs`.

use std::time::Duration;

use serde_json::json;

use crate::headless::Headless;
use crate::state::RightPanel;
use crate::{LightcraftApp, Services};

const T: Duration = Duration::from_secs(20);
const SETTLE: Duration = Duration::from_secs(120);

/// A PNG of a straight 900 × 600 scan of one 35 mm negative frame in a holder: holder, orange base,
/// sprocket holes, and a denser, textured picture on 120..780 × 80..520.
fn negative_png(path: &std::path::Path) {
    let base = [0.62f32, 0.26, 0.07];
    let img = lightcraft_raster::Rgb32f::from_fn(900, 600, |x, y| {
        let (x, y) = (x as f64, y as f64);
        if !(60.0..840.0).contains(&x) || !(40.0..560.0).contains(&y) {
            return [0.004, 0.003, 0.003];
        }
        // (35 mm holes: deeper across the film than long along it)
        let hole = |cy: f64| ((x - 40.0).rem_euclid(56.0) - 28.0).powi(2) / 100.0 + (y - cy).powi(2) / 196.0 < 1.0;
        if hole(38.0) || hole(542.0) {
            return [0.92, 0.93, 0.97];
        }
        if !(120.0..780.0).contains(&x) || !(80.0..520.0).contains(&y) {
            return base;
        }
        let t = 0.2 + 0.25 * ((x / 37.0).sin() * (y / 23.0).cos()).abs() + if ((x / 90.0) as i32 + (y / 70.0) as i32) % 2 == 0 { 0.2 } else { 0.0 };
        base.map(|c| c * t as f32)
    });
    let png = lightcraft_codecs::encode::encode_png(&lightcraft_codecs::encode::EncodeImage::rgba8(&img.to_srgb8()), &Default::default()).unwrap();
    std::fs::write(path, png).unwrap();
}

/// Zap!'s crop of the test scan (`want` its picture) with the sprocket holes removed: on the
/// picture all round.
fn on_frame(g: lightcraft_geom::Rect, want: [f64; 4]) -> bool {
    [(g.x0, want[0]), (g.y0, want[1]), (g.x1, want[2]), (g.y1, want[3])].iter().all(|(a, b)| (a - b).abs() < 0.015)
}

#[test]
fn zap_crops_to_the_frame_with_the_panels_choices_and_undoes_in_one_step() {
    let dir = std::env::temp_dir().join(format!("lc-ui-zap-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("negative.png");
    negative_png(&path);
    let mut session = lightcraft_engine::Session::new().with_fs();
    session.execute("library.import", &json!({"paths": [path.to_string_lossy()]})).unwrap();
    let app = LightcraftApp::new(session, Services { png: None, ..Default::default() });
    let mut h = Headless::new(app, [1400.0, 900.0], 1.0);
    let r = h.request("ui.menu.invoke", json!({"command": "photo.convertNegative"}), T);
    assert_eq!(r["ok"], true, "{r}");
    h.settle(SETTLE);
    for w in [
        "button:negative-zap",
        "dropdown:negative-zapMargin",
        "dropdown:negative-zapFormat",
        "dropdown:negative-zapSprockets",
        "dropdown:negative-zapStraighten",
        "dropdown:negative-zapFilm",
    ] {
        assert!(h.app.widgets.iter().any(|(id, _)| id == w), "{w} on the panel");
    }
    let id = h.app.session.active().unwrap();
    let before = h.app.session.develop_of(id).unwrap();
    h.app.ui.negative_zap_margin = "edge".into();
    h.app.ui.negative_zap_format = "35mm".into();
    let r = h.request("ui.clickWidget", json!({"id": "button:negative-zap"}), T);
    assert_eq!(r["ok"], true, "{r}");
    h.settle(SETTLE);
    let d = h.app.session.develop_of(id).unwrap();
    let g = d.crop.geometry.rect;
    let want = [120.0 / 900.0, 80.0 / 600.0, 780.0 / 900.0, 520.0 / 600.0];
    assert!(on_frame(g, want), "{g:?} toast {:?}", h.app.ui.toast);
    assert!(d.negative.advanced && d.negative.base.is_some(), "the base shows in the Film Base row");
    // Edit ▸ Undo takes the whole Zap! back
    h.request("engine.execute", json!({"command": "edit.undo"}), T);
    h.settle(SETTLE);
    assert_eq!(h.app.session.develop_of(id).unwrap().crop, before.crop);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_tool_strip_opens_convert_negative() {
    let mut h = Headless::new(
        LightcraftApp::new(lightcraft_engine::Session::with_demo(), Services { png: None, ..Default::default() }),
        [1400.0, 900.0],
        1.0,
    );
    h.settle(SETTLE);
    let r = h.request("ui.clickWidget", json!({"id": "icon:negative"}), T);
    assert_eq!(r["ok"], true, "{r}");
    h.settle(SETTLE);
    assert_eq!(h.app.ui.right, crate::state::RightPanel::Negative);
    assert!(h.app.widgets.iter().any(|(id, _)| id == "button:negative-zap"), "the panel with Zap!");
}

#[test]
fn photo_zap_asks_for_its_choices_and_runs_with_them() {
    let dir = std::env::temp_dir().join(format!("lc-ui-zap-dialog-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("negative.png");
    negative_png(&path);
    let mut session = lightcraft_engine::Session::new().with_fs();
    session.execute("library.import", &json!({"paths": [path.to_string_lossy()]})).unwrap();
    let app = LightcraftApp::new(session, Services { png: None, ..Default::default() });
    let mut h = Headless::new(app, [1400.0, 900.0], 1.0);
    let id = h.app.session.active().unwrap();
    let before = h.app.session.develop_of(id).unwrap();
    h.app.ui.negative_zap_format = "35mm".into();
    // Photo ▸ Film Scan ▸ Zap!… opens the dialog with the last choices, and changes nothing yet
    let r = h.request("ui.menu.invoke", json!({"command": "dialog.negativeZap"}), T);
    assert_eq!(r["ok"], true, "{r}");
    h.settle(SETTLE);
    let Some(crate::state::Dialog::NegativeZap { film, format, sprockets, margin, .. }) = h.app.ui.dialog.as_mut() else {
        panic!("the Zap! dialog: {:?}", h.app.ui.dialog)
    };
    assert_eq!((film.as_str(), format.as_str(), sprockets.as_str(), margin.as_str()), ("negative", "35mm", "remove", "inside"));
    for w in [
        "dropdown:negativeZap-film",
        "dropdown:negativeZap-format",
        "dropdown:negativeZap-sprockets",
        "dropdown:negativeZap-margin",
        "dropdown:negativeZap-straighten",
    ] {
        assert!(h.app.widgets.iter().any(|(id, _)| id == w), "{w} in the dialog");
    }
    assert!(!h.app.widgets.iter().any(|(id, _)| id == "dropdown:negativeZap-roll"), "one photo is no roll");
    assert_eq!(h.app.session.develop_of(id).unwrap(), before);
    // the holes kept this time
    *sprockets = "keep".into();
    *margin = "edge".into();
    let r = h.request("ui.dialog.confirm", json!({}), T);
    assert_eq!(r["ok"], true, "{r}");
    h.settle(SETTLE);
    let d = h.app.session.develop_of(id).unwrap();
    let g = d.crop.geometry.rect;
    assert!(g.y1 > 556.0 / 600.0 - 0.01 && (g.x0 - 120.0 / 900.0).abs() < 0.015, "the whole holes kept: {g:?} toast {:?}", h.app.ui.toast);
    assert!(d.negative.enabled, "{:?}", d.negative);
    // the next Zap! starts from them
    assert_eq!(
        (h.app.ui.negative_zap_film.as_str(), h.app.ui.negative_zap_sprockets.as_str(), h.app.ui.negative_zap_margin.as_str()),
        ("negative", "keep", "edge")
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn zap_on_a_selection_runs_in_the_background_and_commits_once() {
    let dir = std::env::temp_dir().join(format!("lc-ui-zap-many-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let paths: Vec<_> = (0..3).map(|i| dir.join(format!("negative-{i}.png"))).collect();
    for (i, p) in paths.iter().enumerate() {
        negative_png(p);
        // three files, not one imported three times: bytes after the PNG's end tell them apart
        let mut bytes = std::fs::read(p).unwrap();
        bytes.extend(std::iter::repeat_n(0u8, i));
        std::fs::write(p, bytes).unwrap();
    }
    let mut session = lightcraft_engine::Session::new().with_fs();
    session.execute("library.import", &json!({"paths": paths.iter().map(|p| p.to_string_lossy()).collect::<Vec<_>>()})).unwrap();
    let mut app = LightcraftApp::new(session, Services { png: None, ..Default::default() });
    let ids: Vec<_> = app.session.visible_cloned().to_vec();
    assert_eq!(ids.len(), 3);
    app.session.execute("library.select", &json!({"ids": ids.iter().map(|i| i.0).collect::<Vec<_>>(), "active": ids[0].0})).unwrap();
    app.ui.negative_zap_margin = "edge".into();
    let before = app.session.commits();
    // the panel's Zap! button (Film Scan ▸ Zap!… runs the same command after its dialog)
    let reply = app.run("develop.negativeZap", json!({})).unwrap();
    assert_eq!((reply["pending"].as_bool(), reply["photos"].as_u64()), (Some(true), Some(3)), "{reply}");
    assert_eq!(app.session.commits(), before, "nothing changes until the run is done");
    assert!(app.session.activity.list().iter().any(|t| t.kind == "zap"), "listed in the activity stack");
    assert!(crate::tasks::wait_for(&mut app, &egui::Context::default(), "Zap!", SETTLE));
    assert_eq!(app.session.commits(), before + 1, "one undo step; toast {:?}", app.ui.toast);
    let want = [120.0 / 900.0, 80.0 / 600.0, 780.0 / 900.0, 520.0 / 600.0];
    for id in &ids {
        let d = app.session.develop_of(*id).unwrap();
        let g = d.crop.geometry.rect;
        assert!(on_frame(g, want), "{id:?}: {g:?} with the panel's margin");
        assert!(d.negative.enabled && d.negative.base.is_some());
    }
    app.session.execute("edit.undo", &json!({})).unwrap();
    assert!(ids.iter().all(|id| !app.session.develop_of(*id).unwrap().negative.enabled), "undone together");
    std::fs::remove_dir_all(&dir).ok();
}

/// Photo ▸ Film Scan ▸ Zap!… on several photos offers to measure them as one roll; they then share
/// one measurement, the panel says so, and Release measures one photo on its own again.
#[test]
fn zap_as_one_roll_from_the_dialog_and_release_from_the_panel() {
    let dir = std::env::temp_dir().join(format!("lc-ui-zap-roll-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let paths: Vec<_> = (0..3).map(|i| dir.join(format!("negative-{i}.png"))).collect();
    for (i, p) in paths.iter().enumerate() {
        negative_png(p);
        let mut bytes = std::fs::read(p).unwrap();
        bytes.extend(std::iter::repeat_n(0u8, i));
        std::fs::write(p, bytes).unwrap();
    }
    let mut session = lightcraft_engine::Session::new().with_fs();
    session.execute("library.import", &json!({"paths": paths.iter().map(|p| p.to_string_lossy()).collect::<Vec<_>>()})).unwrap();
    let app = LightcraftApp::new(session, Services { png: None, ..Default::default() });
    let mut h = Headless::new(app, [1400.0, 900.0], 1.0);
    let ids: Vec<_> = h.app.session.visible_cloned().to_vec();
    h.app.session.execute("library.select", &json!({"ids": ids.iter().map(|i| i.0).collect::<Vec<_>>(), "active": ids[0].0})).unwrap();
    let r = h.request("ui.menu.invoke", json!({"command": "dialog.negativeZap"}), T);
    assert_eq!(r["ok"], true, "{r}");
    h.settle(SETTLE);
    assert!(h.app.widgets.iter().any(|(id, _)| id == "dropdown:negativeZap-roll"), "several photos can be a roll");
    let Some(crate::state::Dialog::NegativeZap { roll, .. }) = h.app.ui.dialog.as_mut() else { panic!("the Zap! dialog: {:?}", h.app.ui.dialog) };
    assert_eq!(roll, "single", "each on its own until asked");
    *roll = "roll".into();
    let r = h.request("ui.dialog.confirm", json!({}), T);
    assert_eq!(r["ok"], true, "{r}");
    assert!(crate::tasks::wait_for(&mut h.app, &egui::Context::default(), "Zap!", SETTLE));
    assert!(h.app.ui.toast.as_ref().is_some_and(|t| t.0.contains("3 measured as one roll, the film base from 3")), "{:?}", h.app.ui.toast);
    h.settle(SETTLE);
    let negs: Vec<_> = ids.iter().map(|id| h.app.session.develop_of(*id).unwrap().negative.clone()).collect();
    assert!(negs.iter().all(|n| n.roll == Some(3) && n.measured == negs[0].measured && n.base == negs[0].base), "{negs:?}");
    assert_eq!(h.app.ui.negative_zap_roll, "roll", "remembered for the next Zap!…");

    // the panel shows the roll; Release measures the active photo on its own
    h.app.session.execute("library.select", &json!({"ids": [ids[0].0], "active": ids[0].0})).unwrap();
    h.request("ui.menu.invoke", json!({"command": "panel.negative"}), T);
    h.settle(SETTLE);
    let r = h.request("ui.clickWidget", json!({"id": "button:negative-rollRelease"}), T);
    assert_eq!(r["ok"], true, "{r}");
    h.settle(SETTLE);
    assert!(h.app.session.develop_of(ids[0]).unwrap().negative.roll.is_none());
    assert_eq!(h.app.session.develop_of(ids[1]).unwrap().negative.roll, Some(3), "the others stay in the roll");
    std::fs::remove_dir_all(&dir).ok();
}

/// Photo ▸ Film Scan ▸ Zap!… offers black-and-white negatives: a selection of them converts with
/// the monochrome profile, still offered as one roll, and the choice is kept for the next Zap!….
#[test]
fn zap_dialog_converts_black_and_white_negatives() {
    let dir = std::env::temp_dir().join(format!("lc-ui-zap-bw-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let paths: Vec<_> = (0..2).map(|i| dir.join(format!("negative-{i}.png"))).collect();
    for (i, p) in paths.iter().enumerate() {
        negative_png(p);
        let mut bytes = std::fs::read(p).unwrap();
        bytes.extend(std::iter::repeat_n(0u8, i));
        std::fs::write(p, bytes).unwrap();
    }
    let mut session = lightcraft_engine::Session::new().with_fs();
    session.execute("library.import", &json!({"paths": paths.iter().map(|p| p.to_string_lossy()).collect::<Vec<_>>()})).unwrap();
    let app = LightcraftApp::new(session, Services { png: None, ..Default::default() });
    let mut h = Headless::new(app, [1400.0, 900.0], 1.0);
    let ids: Vec<_> = h.app.session.visible_cloned().to_vec();
    h.app.session.execute("library.select", &json!({"ids": ids.iter().map(|i| i.0).collect::<Vec<_>>(), "active": ids[0].0})).unwrap();
    assert!(crate::panels::negative::FILMS.iter().any(|(v, _)| *v == "bw"), "offered in the dialog and the panel");
    let r = h.request("ui.menu.invoke", json!({"command": "dialog.negativeZap"}), T);
    assert_eq!(r["ok"], true, "{r}");
    h.settle(SETTLE);
    let Some(crate::state::Dialog::NegativeZap { film, .. }) = h.app.ui.dialog.as_mut() else { panic!("the Zap! dialog: {:?}", h.app.ui.dialog) };
    *film = "bw".into();
    h.settle(SETTLE);
    assert!(h.app.widgets.iter().any(|(id, _)| id == "dropdown:negativeZap-roll"), "black-and-white negatives can be a roll");
    let r = h.request("ui.dialog.confirm", json!({}), T);
    assert_eq!(r["ok"], true, "{r}");
    assert!(crate::tasks::wait_for(&mut h.app, &egui::Context::default(), "Zap!", SETTLE));
    for id in &ids {
        let n = h.app.session.develop_of(*id).unwrap().negative.clone();
        assert!(n.enabled && n.preset == "Monochrome", "{id:?}: {n:?}");
    }
    assert_eq!(h.app.ui.negative_zap_film, "bw", "remembered for the next Zap!…");
    std::fs::remove_dir_all(&dir).ok();
}

/// The import review of a folder of two negative scans, added in place, with Then Zap!… `zap`.
fn import_scans(tag: &str, zap: bool) -> (Headless, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("lc-ui-zap-import-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for i in 0..2 {
        let p = dir.join(format!("negative-{i}.png"));
        negative_png(&p);
        let mut bytes = std::fs::read(&p).unwrap();
        bytes.extend(std::iter::repeat_n(0u8, i));
        std::fs::write(&p, bytes).unwrap();
    }
    let app = LightcraftApp::new(lightcraft_engine::Session::new().with_fs(), Services { png: None, ..Default::default() });
    let mut h = Headless::new(app, [1300.0, 1400.0], 1.0);
    h.app.ui.view = crate::state::ViewMode::PhotoGrid;
    let r = h.request("engine.execute", json!({"command": "file.addPhotos", "params": {"paths": [dir.to_string_lossy()]}}), T);
    assert_eq!(r["result"]["scanning"], true, "{r}");
    assert!(h.step_until(SETTLE, |h| matches!(h.app.ui.dialog, Some(crate::state::Dialog::Import { .. }))), "the review opens");
    h.settle(SETTLE);
    assert!(h.app.widgets.iter().any(|(id, _)| id == "check:importZap"), "Then Zap!… in the review, adding in place too");
    let Some(crate::state::Dialog::Import { opts }) = &mut h.app.ui.dialog else { panic!("no import review") };
    assert!(!opts.copy && !opts.zap, "added in place, no Zap! unless asked");
    opts.zap = zap;
    let r = h.request("ui.dialog.confirm", json!({}), T);
    assert_eq!(r["ok"], true, "{r}");
    let t0 = std::time::Instant::now();
    while h.app.import.is_some() && t0.elapsed() < Duration::from_secs(60) {
        h.step();
    }
    assert!(h.app.import.is_none(), "the import finished");
    h.settle(SETTLE);
    (h, dir)
}

/// Import with Then Zap!…: when it is done the photos it added are selected and Zap!… asks for
/// them; its choices then zap both. Without it no dialog follows the import.
#[test]
fn import_then_zap_asks_for_the_added_photos() {
    let (mut h, dir) = import_scans("zap", true);
    let ids: Vec<_> = h.app.session.catalog.photos().map(|p| p.id).collect();
    assert_eq!(ids.len(), 2);
    assert!(matches!(h.app.ui.dialog, Some(crate::state::Dialog::NegativeZap { .. })), "Zap!… after the import: {:?}", h.app.ui.dialog);
    let mut selected = h.app.session.selection.ids.clone();
    selected.sort();
    let mut want = ids.clone();
    want.sort();
    assert_eq!(selected, want, "the added photos selected");
    let r = h.request("ui.dialog.confirm", json!({}), T);
    assert_eq!(r["ok"], true, "{r}");
    assert!(crate::tasks::wait_for(&mut h.app, &egui::Context::default(), "Zap!", SETTLE));
    for id in &ids {
        let d = h.app.session.develop_of(*id).unwrap();
        assert!(d.negative.enabled && d.negative.zap_crop.is_some(), "{id:?} zapped: {:?}", d.negative);
    }
    let _ = std::fs::remove_dir_all(&dir);

    let (h, dir) = import_scans("plain", false);
    assert!(h.app.ui.dialog.is_none(), "no dialog without Then Zap!…: {:?}", h.app.ui.dialog);
    assert!(h.app.session.catalog.photos().all(|p| !p.develop.negative.enabled));
    let _ = std::fs::remove_dir_all(&dir);
}

/// The Film Scan panel fits the right panel at every width, in English and in German: Zap!'s
/// choices wrap instead of widening the panel, which pushed the sliders' values out of sight.
#[test]
fn the_film_scan_panel_fits_the_right_panel() {
    let app = LightcraftApp::new(lightcraft_engine::Session::with_demo(), Services { png: None, ..Default::default() });
    let mut h = Headless::new(app, [1400.0, 900.0], 1.0);
    let r = h.request("ui.menu.invoke", json!({"command": "photo.convertNegative"}), T);
    assert_eq!(r["ok"], true, "{r}");
    h.settle(SETTLE);
    assert_eq!(h.app.ui.right, RightPanel::Negative);
    // Advanced shows every row: the film base, the Tone / Color / White Balance choices
    let r = h.request("ui.clickWidget", json!({"id": "button:negativeMode-advanced"}), T);
    assert_eq!(r["ok"], true, "{r}");
    h.settle(SETTLE);
    for language in ["en", "de"] {
        assert_eq!(h.request("ui.set", json!({"language": language}), T)["ok"], true);
        for width in [crate::state::RIGHT_WIDTH.min, crate::state::RIGHT_WIDTH.default] {
            h.request("ui.set", json!({"rightWidth": width}), T);
            h.step();
            h.step();
            let panel = h.app.widgets.iter().find(|(id, _)| id == "panel:right_panel").expect("the right panel").1;
            let ours: Vec<_> = h
                .app
                .widgets
                .iter()
                .filter(|(id, _)| id.contains("negative") && !["panel:", "film:", "icon:"].iter().any(|p| id.starts_with(p)))
                .collect();
            assert!(ours.iter().any(|(id, _)| id == "sliderValue:negative.exposure"), "{language} {width}: the sliders are drawn");
            assert!(ours.iter().any(|(id, _)| id == "dropdown:negative-zapFilm"), "{language} {width}: Zap!'s choices are drawn");
            assert!(ours.iter().any(|(id, _)| id == "dropdown:negative-tone"), "{language} {width}: the Advanced rows are drawn");
            for (id, r) in &ours {
                assert!(r.right() <= panel.right() + 0.5, "{language} {width}: {id} {r:?} sticks out of the panel {panel:?}");
            }
            // and nothing widened the contents: every slider's value ends at the same edge
            let values: Vec<_> = ours.iter().filter(|(id, _)| id.starts_with("sliderValue:negative.")).map(|(id, r)| (id, r.right())).collect();
            let edge = values[0].1;
            assert!(values.iter().all(|(_, right)| (right - edge).abs() < 0.5), "{language} {width}: values end at different edges: {values:?}");
        }
    }
}
