//! `develop.negativeZap` through the command layer: the frame found, straightened and cropped, the
//! film base picked, a crop of the user's own kept, sprocket holes, slides, a selection in one undo
//! step and the roll mode, plus the opt-in runs on real scans. The conversion's own command tests
//! are in `tests/negative.rs`.

use serde_json::json;

use crate::Session;

/// A PNG of one frame of 35 mm colour negative in a holder, put in askew (`angle` degrees):
/// holder, orange base, sprocket holes along the long edges and a textured, denser picture on
/// 120..780 × 80..520 of the straight 900 × 600 scan.
fn negative_scan(dir: &std::path::Path, angle: f64) -> std::path::PathBuf {
    negative_scan_of(dir, &format!("negative-{angle}"), angle, [0.62, 0.26, 0.07], 1.0)
}

/// [`negative_scan`] of a film with this `base`, its picture `density` times as dense (in
/// transmittance: below 1 is denser, a frame given more light), saved as `name`.png.
fn negative_scan_of(dir: &std::path::Path, name: &str, angle: f64, base: [f32; 3], density: f64) -> std::path::PathBuf {
    let (w, h) = (900usize, 600usize);
    let fwd = lightcraft_geom::Affine::rotate_about(angle.to_radians(), lightcraft_geom::Point::new(w as f64 / 2.0, h as f64 / 2.0));
    let img = lightcraft_raster::Rgb32f::from_fn(w, h, |x, y| {
        let q = fwd.apply(lightcraft_geom::Point::new(x as f64 + 0.5, y as f64 + 0.5));
        let (x, y) = (q.x, q.y);
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
        base.map(|c| c * (t * density) as f32)
    });
    // (linear sRGB, as the PNG's sRGB encoding gives it back)
    let png = lightcraft_codecs::encode::encode_png(&lightcraft_codecs::encode::EncodeImage::rgba8(&img.to_srgb8()), &Default::default()).unwrap();
    let path = dir.join(format!("{name}.png"));
    std::fs::write(&path, png).unwrap();
    path
}

#[test]
fn zap_straightens_crops_and_picks_the_base_in_one_undo_step() {
    let dir = std::env::temp_dir().join(format!("lc-zap-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = negative_scan(&dir, 1.5);
    let mut s = Session::new().with_fs();
    s.execute("library.import", &json!({"paths": [path.to_string_lossy()]})).unwrap();
    let id = s.active().unwrap();
    let before = s.develop_of(id).unwrap();
    let r = s.execute("develop.negativeZap", &json!({"margin": "edge", "format": "35mm", "sprockets": "keep"})).unwrap();
    let d = s.develop_of(id).unwrap();
    let g = d.crop.geometry;
    assert!((g.angle - 1.5).abs() < 0.25, "{r}");
    // the picture found; the crop's ends on it, its long sides by the sprocket holes (the cut ones
    // at the top left out, the rebate below them kept; the whole ones at the bottom kept)
    let want = [120.0 / 900.0, 80.0 / 600.0, 780.0 / 900.0, 520.0 / 600.0];
    let pic: Vec<f64> = r["zap"]["picture"].as_array().unwrap().iter().filter_map(|v| v.as_f64()).collect();
    assert!(want.iter().zip(&pic).all(|(a, b)| (a - b).abs() < 0.015), "{pic:?} {r}");
    assert!((g.rect.x0 - want[0]).abs() < 0.015 && (g.rect.x1 - want[2]).abs() < 0.015, "{r}");
    assert!(g.rect.y0 > 52.0 / 600.0 && g.rect.y0 < want[1], "below the cut holes, above the picture: {r}");
    assert!(g.rect.y1 > 556.0 / 600.0 - 0.01, "the whole holes kept: {r}");
    let n = &d.negative;
    assert!(n.enabled && n.advanced && n.measured.is_some(), "{n:?}");
    let b = n.base.expect("the base from the rebate");
    assert!((b[0] - 0.62).abs() < 0.04 && (b[1] - 0.26).abs() < 0.04 && (b[2] - 0.07).abs() < 0.04, "{b:?}");
    assert_eq!(r["zap"]["sidesFound"], 4, "{r}");
    // one step back undoes all of it
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(s.develop_of(id).unwrap(), before);
    // the margins: inside is smaller than the edge
    s.execute("develop.negativeZap", &json!({})).unwrap();
    let inside = s.develop_of(id).unwrap().crop.geometry.rect;
    assert!(inside.width() < g.rect.width(), "{inside:?} vs {:?}", g.rect);
    std::fs::remove_dir_all(&dir).ok();
}

/// Zap! measures a turned frame straightened, as the render shows it and as Measure Again reads
/// the crop: the box of the scan under a turned crop would take in rebate at its corners.
#[test]
fn zap_measures_a_turned_frame_as_measure_again_does() {
    let dir = std::env::temp_dir().join(format!("lc-zap-turned-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = negative_scan(&dir, 2.5);
    let mut s = Session::new().with_fs();
    s.execute("library.import", &json!({"paths": [path.to_string_lossy()]})).unwrap();
    let id = s.active().unwrap();
    let r = s.execute("develop.negativeZap", &json!({})).unwrap();
    let d = s.develop_of(id).unwrap();
    assert!((d.crop.geometry.angle - 2.5).abs() < 0.25, "{r}");
    let zapped = d.negative.measured.expect("Zap! measures");
    // Measure Again on Zap!'s crop, which is its picture
    s.execute("develop.negative", &json!({"measure": true, "border": 0})).unwrap();
    let again = s.develop_of(id).unwrap().negative.measured.expect("measured again");
    let close = |a: [f64; 3], b: [f64; 3]| a.iter().zip(b).all(|(a, b)| (a - b).abs() <= 0.02 * a.abs().max(b.abs()).max(1e-3));
    assert!(close(zapped.black, again.black) && close(zapped.white, again.white), "{zapped:?} vs {again:?}");
    std::fs::remove_dir_all(&dir).ok();
}

/// Zap! on a black-and-white negative converts it with the monochrome profile, its base still
/// from the rebate: a chromogenic film (XP2) has a coloured base, which must turn black, not tint
/// the picture. A colour negative afterwards leaves the monochrome profile.
#[test]
fn zap_converts_a_black_and_white_negative_on_its_coloured_base() {
    let dir = std::env::temp_dir().join(format!("lc-zap-bw-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // an XP2-like base: purple-brown, not grey
    let base = [0.55, 0.38, 0.45];
    let path = negative_scan_of(&dir, "xp2", 0.0, base, 1.0);
    let mut s = Session::new().with_fs();
    s.execute("library.import", &json!({"paths": [path.to_string_lossy()]})).unwrap();
    let id = s.active().unwrap();
    assert!(s.execute("develop.negativeZap", &json!({"film": "grey"})).is_err(), "an unknown film is refused");
    s.execute("develop.negativeZap", &json!({"film": "bw"})).unwrap();
    let d = s.develop_of(id).unwrap();
    assert_eq!(d.negative.preset, crate::negative_zap::MONOCHROME);
    let b = d.negative.base.expect("the base from the rebate");
    assert!(b.iter().zip(base).all(|(a, w)| (a - w as f64).abs() < 0.04), "the coloured base as it is: {b:?}");
    // the conversion of the scan's own pixels: grey in the picture, black on the rebate
    let src = s.source_now(id, crate::media::SourceLevel::Preview).unwrap();
    let convert = lightcraft_pipeline::negative::pixel_converter(&d);
    let at = |x: f64, y: f64| src.get((x * src.width as f64) as usize, (y * src.height as f64) as usize);
    for (x, y) in [(0.3, 0.3), (0.5, 0.5), (0.7, 0.6)] {
        let p = convert(at(x, y));
        assert!((p[0] - p[1]).abs() < 1e-3 && (p[1] - p[2]).abs() < 1e-3, "grey at {x},{y}: {p:?}");
        assert!(p[1] > 0.02, "the picture is not black at {x},{y}: {p:?}");
    }
    let rebate = convert(at(90.0 / 900.0, 0.5));
    assert!(rebate.iter().all(|v| *v < 0.01), "the coloured base converts to black: {rebate:?}");
    // a colour negative leaves the monochrome profile
    s.execute("develop.negativeZap", &json!({"film": "negative"})).unwrap();
    assert_eq!(s.develop_of(id).unwrap().negative.preset, "Standard");
    std::fs::remove_dir_all(&dir).ok();
}

/// A roll's frames agree on their crop: an edge far off the others' moves to the roll's (a dark
/// scene taken for rebate); the frames that agree, and a selection that agrees on nothing, stay.
#[test]
fn a_roll_agrees_on_its_frames_crop() {
    use crate::negative_zap::{Zapped, agree};
    let frame = |r: [f64; 4]| {
        let mut d = lightcraft_develop::DevelopSettings::default();
        d.crop.geometry = lightcraft_geom::CropGeometry { rect: lightcraft_geom::Rect::new(r[0], r[1], r[2], r[3]), angle: 0.3 };
        d.negative.zap_crop = Some(d.crop.geometry);
        Zapped { settings: d, report: serde_json::Value::Null, samples: None }
    };
    let roll =
        [[0.01, 0.01, 0.99, 0.96], [0.012, 0.01, 0.99, 0.965], [0.01, 0.2, 0.99, 0.96], [0.015, 0.012, 0.985, 0.96], [0.01, 0.01, 0.99, 0.955]];
    let mut zapped: Vec<_> = roll.iter().enumerate().map(|(i, r)| (crate::PhotoId(i as u64 + 1), frame(*r))).collect();
    let before = zapped.clone();
    assert_eq!(agree(&mut zapped), vec![crate::PhotoId(3)]);
    let g = zapped[2].1.settings.crop.geometry;
    assert!((g.rect.y0 - 0.01).abs() < 1e-9 && g.angle == 0.3, "the roll's top, its own angle: {g:?}");
    assert_eq!(zapped[2].1.settings.negative.zap_crop, Some(g), "still Zap!'s crop");
    for i in [0, 1, 3, 4] {
        assert_eq!(zapped[i].1.settings, before[i].1.settings, "frame {i} agrees already");
    }
    // a moved frame that can't be measured in its new crop keeps its own crop: it never fails the rest
    let mut zapped: Vec<_> = roll.iter().enumerate().map(|(i, r)| (crate::PhotoId(i as u64 + 1), frame(*r))).collect();
    let moved = crate::negative_zap::agree_measured(&mut zapped, |_, _, _| Err("no pixels to measure".into()));
    assert!(moved.is_empty());
    assert_eq!(zapped[2].1.settings, before[2].1.settings, "its own crop again");
    // a roll with nothing to pool (no frame's samples count) keeps each frame's own measurement,
    // rather than failing the selection
    let mut zapped: Vec<_> = roll.iter().enumerate().map(|(i, r)| (crate::PhotoId(i as u64 + 1), frame(*r))).collect();
    for (_, z) in &mut zapped {
        z.samples =
            Some(Box::new(crate::negative_zap::RollSamples { samples: lightcraft_pipeline::negative::film::ValueHistogram::new(), base: None }));
        z.settings.negative.enabled = true;
    }
    assert!(crate::negative_zap::pool(&mut zapped).unwrap().is_none());
    // a selection that agrees on nothing (other films, holders) is left as it is
    let mixed = [[0.1, 0.05, 0.9, 0.95], [0.25, 0.2, 0.7, 0.8], [0.0, 0.3, 0.6, 0.99], [0.4, 0.1, 0.95, 0.7], [0.05, 0.45, 0.8, 0.85]];
    let mut zapped: Vec<_> = mixed.iter().enumerate().map(|(i, r)| (crate::PhotoId(i as u64 + 1), frame(*r))).collect();
    assert!(agree(&mut zapped).is_empty());
    // fewer frames than a roll needs
    let mut zapped: Vec<_> = roll.iter().take(3).enumerate().map(|(i, r)| (crate::PhotoId(i as u64 + 1), frame(*r))).collect();
    assert!(agree(&mut zapped).is_empty());
}

/// The roll's crop never takes a frame's crop onto blank film (a roll's last frame, half exposed,
/// beside the fogged leader): measured again there, the move is refused; onto picture, it holds.
#[test]
fn a_roll_never_widens_a_frame_onto_blank_film() {
    use crate::negative_zap::{Zapped, remeasure};
    // the picture on the left 70 %, then even film
    let scan = lightcraft_raster::Rgb32f::from_fn(600, 400, |x, y| {
        if x >= 420 {
            return [0.08, 0.07, 0.06];
        }
        let t = 0.3 + 0.25 * ((x as f64 / 13.0).sin() * (y as f64 / 9.0).cos()).abs();
        [0.5 * t as f32, 0.4 * t as f32, 0.3 * t as f32]
    });
    let zapped = |r: [f64; 4]| {
        let mut d = lightcraft_develop::DevelopSettings::default();
        d.crop.geometry = lightcraft_geom::CropGeometry { rect: lightcraft_geom::Rect::new(r[0], r[1], r[2], r[3]), angle: 0.0 };
        d.negative.zap_crop = Some(d.crop.geometry);
        Zapped { settings: d, report: serde_json::Value::Null, samples: None }
    };
    // the frame's own crop ends with its picture; the roll's reaches on over the leader
    let mut onto_blank = zapped([0.05, 0.05, 0.95, 0.95]);
    assert!(remeasure(&scan, &mut onto_blank, lightcraft_geom::Rect::new(0.05, 0.05, 0.7, 0.95)).is_err(), "the leader is no picture");
    let mut onto_picture = zapped([0.05, 0.05, 0.68, 0.95]);
    remeasure(&scan, &mut onto_picture, lightcraft_geom::Rect::new(0.05, 0.05, 0.5, 0.95)).unwrap();
    assert!(onto_picture.settings.negative.measured.is_some());
}

#[test]
fn zap_removes_the_sprocket_holes_or_crops_to_the_whole_film() {
    let dir = std::env::temp_dir().join(format!("lc-zap-holes-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = negative_scan(&dir, 0.0);
    let mut s = Session::new().with_fs();
    s.execute("library.import", &json!({"paths": [path.to_string_lossy()]})).unwrap();
    let id = s.active().unwrap();
    // by default the crop ends at the picture, the holes and the rebate beside them left out
    s.execute("develop.negativeZap", &json!({"margin": "edge"})).unwrap();
    let g = s.develop_of(id).unwrap().crop.geometry.rect;
    assert!((g.y0 - 80.0 / 600.0).abs() < 0.015 && (g.y1 - 520.0 / 600.0).abs() < 0.015, "{g:?}");
    s.execute("edit.undo", &json!({})).unwrap();
    // the whole film: the crop is the film inside the holder, rebate and holes and all
    s.execute("develop.negativeZap", &json!({"margin": "film"})).unwrap();
    let g = s.develop_of(id).unwrap().crop.geometry.rect;
    let film = [60.0 / 900.0, 40.0 / 600.0, 840.0 / 900.0, 560.0 / 600.0];
    assert!([g.x0, g.y0, g.x1, g.y1].iter().zip(film).all(|(a, b)| (a - b).abs() < 0.012), "{g:?}");
    std::fs::remove_dir_all(&dir).ok();
}

/// A PNG of a slide (a positive) on a 900 × 600 scan: the film across the width on 20..580, a dark
/// rebate, sprocket holes round 45 and 555 with light through them, and a brighter picture on
/// 120..780 × 90..510.
fn slide_scan(dir: &std::path::Path) -> std::path::PathBuf {
    let img = lightcraft_raster::Rgb32f::from_fn(900, 600, |x, y| {
        let (x, y) = (x as f64 + 0.5, y as f64 + 0.5);
        let hole = |cy: f64| ((x - 40.0).rem_euclid(56.0) - 28.0).powi(2) / 100.0 + (y - cy).powi(2) / 196.0 < 1.0;
        let v = if !(20.0..580.0).contains(&y) {
            0.0005
        } else if hole(45.0) || hole(555.0) {
            0.9
        } else if !((120.0..780.0).contains(&x) && (90.0..510.0).contains(&y)) {
            0.012
        } else {
            0.25 + 0.2 * ((x / 41.0).sin() + (y / 29.0).cos() + ((x + y) / 53.0).sin()) / 3.0
        } as f32;
        [v, v, v * 0.9]
    });
    let png = lightcraft_codecs::encode::encode_png(&lightcraft_codecs::encode::EncodeImage::rgba8(&img.to_srgb8()), &Default::default()).unwrap();
    let path = dir.join("slide.png");
    std::fs::write(&path, png).unwrap();
    path
}

#[test]
fn zap_only_crops_a_slide() {
    let dir = std::env::temp_dir().join(format!("lc-zap-slide-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = slide_scan(&dir);
    let mut s = Session::new().with_fs();
    s.execute("library.import", &json!({"paths": [path.to_string_lossy()]})).unwrap();
    let id = s.active().unwrap();
    let before = s.develop_of(id).unwrap();
    s.execute("develop.negativeZap", &json!({"margin": "edge", "film": "slide"})).unwrap();
    let d = s.develop_of(id).unwrap();
    let g = d.crop.geometry.rect;
    let want = [120.0 / 900.0, 90.0 / 600.0, 780.0 / 900.0, 510.0 / 600.0];
    assert!([g.x0, g.y0, g.x1, g.y1].iter().zip(want).all(|(a, b)| (a - b).abs() < 0.012), "{g:?}");
    assert_eq!(d.negative, lightcraft_develop::Negative { zap_crop: d.negative.zap_crop, ..before.negative.clone() }, "no conversion");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn zap_rejects_bad_arguments_and_photos_without_a_frame() {
    let mut s = Session::with_demo();
    let bad = [
        json!({"margin": "wide"}),
        json!({"margin": 3}),
        json!({"format": "8x10"}),
        json!({"format": true}),
        json!({"sprockets": "cut"}),
        json!({"film": "print"}),
    ];
    for p in bad {
        assert!(s.execute("develop.negativeZap", &p).is_err(), "{p}");
    }
    // a demo photo is no negative in a holder: an error, nothing changed
    let id = s.active().unwrap();
    let before = s.develop_of(id).unwrap();
    let _ = s.execute("develop.negativeZap", &json!({}));
    let after = s.develop_of(id).unwrap();
    assert!(after == before || after.negative.enabled, "either untouched or a frame was found");
}

#[test]
fn zap_on_a_selection_gives_each_scan_its_own_frame_in_one_undo_step() {
    let dir = std::env::temp_dir().join(format!("lc-zap-many-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let blank = dir.join("blank.png");
    let gray = lightcraft_raster::Rgb32f::from_fn(300, 200, |_, _| [0.3, 0.3, 0.3]);
    std::fs::write(
        &blank,
        lightcraft_codecs::encode::encode_png(&lightcraft_codecs::encode::EncodeImage::rgba8(&gray.to_srgb8()), &Default::default()).unwrap(),
    )
    .unwrap();
    let paths = [negative_scan(&dir, 1.5), negative_scan(&dir, -2.0), blank];
    let mut s = Session::new().with_fs();
    s.execute("library.import", &json!({"paths": paths.iter().map(|p| p.to_string_lossy()).collect::<Vec<_>>()})).unwrap();
    let ids: Vec<u64> = s.visible_cloned().iter().map(|p| p.0).collect();
    assert_eq!(ids.len(), 3);
    s.execute("library.select", &json!({"ids": ids, "active": ids[0]})).unwrap();
    // Auto Sync on: still each photo's own frame, never the active photo's crop on the others
    s.execute("develop.autoSync", &json!({"on": true})).unwrap();
    let develop = |s: &Session| ids.iter().map(|id| s.develop_of(crate::PhotoId(*id)).unwrap()).collect::<Vec<_>>();
    let before = develop(&s);
    let r = s.execute("develop.negativeZap", &json!({"margin": "edge"})).unwrap();
    assert_eq!(r["changed"], 2, "{r}");
    let failed = r["failed"].as_array().unwrap();
    assert_eq!(failed.len(), 1, "{r}");
    let after = develop(&s);
    let blank_at = after.iter().position(|d| !d.negative.enabled).expect("the scan with no frame is left out");
    assert_eq!(failed[0]["id"], ids[blank_at], "{r}");
    assert_eq!(after[blank_at], before[blank_at], "untouched");
    let mut angles: Vec<f64> = after.iter().filter(|d| d.negative.enabled).map(|d| d.crop.geometry.angle).collect();
    angles.sort_by(f64::total_cmp);
    assert!((angles[0] + 2.0).abs() < 0.25 && (angles[1] - 1.5).abs() < 0.25, "{angles:?} {r}");
    assert!(after.iter().filter(|d| d.negative.enabled).all(|d| d.negative.base.is_some() && d.negative.measured.is_some()));
    // one step back undoes every photo
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(develop(&s), before);
    // named photos, whatever the selection; none with a frame is an error that changes nothing
    s.execute("library.select", &json!({"ids": [ids[blank_at]], "active": ids[blank_at]})).unwrap();
    assert!(s.execute("develop.negativeZap", &json!({"ids": [ids[blank_at]]})).is_err());
    assert!(s.execute("develop.negativeZap", &json!({"ids": [999_999]})).is_err(), "no such photo");
    assert!(s.execute("develop.negativeZap", &json!({"ids": "all"})).is_err());
    assert_eq!(develop(&s), before);
    let other = (blank_at + 1) % 3;
    let r = s.execute("develop.negativeZap", &json!({"ids": [ids[other]]})).unwrap();
    assert_eq!((r["changed"].as_u64(), r["id"].as_u64()), (Some(1), Some(ids[other])), "{r}");
    std::fs::remove_dir_all(&dir).ok();
}

/// A crop the user sets is theirs: a later Zap! keeps it (and its angle) and only takes the film
/// base and measures the picture; Zap! may redo its own crop, and Reset Crop gives it back to
/// Zap!. Undo takes Zap!'s memory of its crop back with the rest.
#[test]
fn zap_keeps_a_crop_the_user_set() {
    let dir = std::env::temp_dir().join(format!("lc-zap-own-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = negative_scan(&dir, 1.5);
    let mut s = Session::new().with_fs();
    s.execute("library.import", &json!({"paths": [path.to_string_lossy()]})).unwrap();
    let id = s.active().unwrap();
    let crop = |s: &Session| s.develop_of(id).unwrap().crop.geometry;
    // Zap!'s own crop, remembered; Zap! again may set it again
    let r = s.execute("develop.negativeZap", &json!({})).unwrap();
    assert_eq!(r["zap"]["cropKept"], false, "{r}");
    let zapped = crop(&s);
    assert_eq!(s.develop_of(id).unwrap().negative.zap_crop, Some(zapped));
    let r = s.execute("develop.negativeZap", &json!({"margin": "edge"})).unwrap();
    assert_eq!(r["zap"]["cropKept"], false, "{r}");
    assert_ne!(crop(&s), zapped, "the edge margin: a new crop");
    // the user's own crop stays, with its angle; the base is still taken
    s.execute("crop.set", &json!({"rect": [0.2, 0.2, 0.7, 0.7], "angle": 0.0})).unwrap();
    let own = crop(&s);
    s.execute("develop.negative", &json!({"clearBase": true})).unwrap();
    let r = s.execute("develop.negativeZap", &json!({})).unwrap();
    assert_eq!(r["zap"]["cropKept"], true, "{r}");
    assert_eq!(crop(&s), own);
    let n = s.develop_of(id).unwrap().negative.clone();
    assert!(n.base.is_some() && n.measured.is_some() && n.enabled, "{n:?}");
    // Reset Crop: Zap! crops again
    s.execute("crop.reset", &json!({})).unwrap();
    let r = s.execute("develop.negativeZap", &json!({})).unwrap();
    assert_eq!(r["zap"]["cropKept"], false, "{r}");
    assert!((crop(&s).angle - 1.5).abs() < 0.25, "{r}");
    // one undo: Zap!'s crop and its memory of it go back together
    let before = s.develop_of(id).unwrap().negative.zap_crop;
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(crop(&s), lightcraft_geom::CropGeometry::default());
    assert_ne!(s.develop_of(id).unwrap().negative.zap_crop, before);
    std::fs::remove_dir_all(&dir).ok();
}

/// Writes `img` as a binary PPM (test output only).
fn write_ppm(path: &std::path::Path, img: &lightcraft_raster::Rgba8) {
    let mut out = format!("P6\n{} {}\n255\n", img.width, img.height).into_bytes();
    out.extend(img.data.iter().flat_map(|p| [p[0], p[1], p[2]]));
    std::fs::write(path, out).unwrap();
}

/// Zap! on real scans the user has (never committed):
///
///   LC_ZAP_DIR=corpus/negatives-london LC_ZAP_OUT=/tmp/zap \
///     cargo test --release -p lightcraft-engine zap_on_real_scans -- --ignored --nocapture
///
/// Prints what it found and writes, per scan, the scan with the film (blue) and the crop (red)
/// drawn on it, and the converted result.
#[test]
#[ignore = "needs LC_ZAP_DIR with scans of negatives"]
fn zap_on_real_scans() {
    let (Ok(dir), Ok(out)) = (std::env::var("LC_ZAP_DIR"), std::env::var("LC_ZAP_OUT")) else {
        lightcraft_testkit::skip!(Input, "set LC_ZAP_DIR=<folder of negative scans> and LC_ZAP_OUT=<folder>");
        return;
    };
    let out = std::path::PathBuf::from(out);
    std::fs::create_dir_all(&out).unwrap();
    let mut s = Session::new().with_fs();
    s.execute("library.import", &json!({"paths": [dir]})).unwrap();
    let ids: Vec<crate::PhotoId> = s.visible_cloned();
    for id in ids {
        let name = s.catalog.photo(id).map(|p| p.file_name.trim_end_matches(".dng").to_string()).unwrap_or_default();
        s.selection.active = Some(id);
        let src = s.source_now(id, crate::media::SourceLevel::Preview).unwrap();
        let format = std::env::var("LC_ZAP_FORMAT").unwrap_or_else(|_| "auto".into());
        let ratio = lightcraft_pipeline::negative_frame::format_ratio(&format).unwrap();
        let frame = lightcraft_pipeline::negative_frame::detect(&src, ratio);
        let sprockets = std::env::var("LC_ZAP_SPROCKETS").unwrap_or_else(|_| "remove".into());
        let film = std::env::var("LC_ZAP_FILM").unwrap_or_else(|_| "negative".into());
        let margin = std::env::var("LC_ZAP_MARGIN").unwrap_or_else(|_| "inside".into());
        let r = s.execute("develop.negativeZap", &json!({"ids": [id.0], "format": format, "sprockets": sprockets, "film": film, "margin": margin}));
        println!("{name}: {frame:?}\n    -> {}", r.as_ref().map(|v| v["zap"].to_string()).unwrap_or_else(|e| e.to_string()));
        // the scan with the film and the picture drawn on it, in the turned frame's coordinates
        if let Ok(f) = frame {
            let (w, h) = (src.width as f64, src.height as f64);
            let fwd = lightcraft_geom::Affine::rotate_about(-f.angle.to_radians(), lightcraft_geom::Point::new(w / 2.0, h / 2.0));
            let mut img = src.to_srgb8();
            for (rect, colour) in
                [(f.film, [0u8, 120, 255, 255]), (f.crop(lightcraft_pipeline::negative_frame::Margin::Inside, w, h), [255, 0, 0, 255])]
            {
                let r = rect.scale(w, h);
                let c = r.corners();
                for k in 0..4 {
                    let (a, b) = (c[k], c[(k + 1) % 4]);
                    for t in 0..2000 {
                        let p = a.lerp(b, t as f64 / 2000.0);
                        let q = fwd.apply(p);
                        for d in 0..3 {
                            let (x, y) = (q.x as isize + d, q.y as isize + d);
                            if x >= 0 && y >= 0 && (x as usize) < img.width && (y as usize) < img.height {
                                img.set(x as usize, y as usize, colour);
                            }
                        }
                    }
                }
            }
            write_ppm(&out.join(format!("{name}-frame.ppm")), &img);
        } else {
            // nothing found: the plain scan, to see why
            write_ppm(&out.join(format!("{name}-frame.ppm")), &src.to_srgb8());
        }
        if r.is_ok() && std::env::var_os("LC_ZAP_NO_RENDER").is_none() {
            let rendered = s.render_now(id, 900, 900).unwrap();
            write_ppm(&out.join(format!("{name}-zap.ppm")), &rendered.image);
        }
    }
}

/// Zap! on a roll: its frames share one measurement, each on its own film base (a frame scanned
/// brighter still belongs), a frame of another film is measured on its own, and measuring a frame
/// again takes it out of the roll.
#[test]
fn zap_as_a_roll_measures_the_frames_together() {
    let dir = std::env::temp_dir().join(format!("lc-zap-roll-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let base = [0.62, 0.26, 0.07];
    let paths = [
        negative_scan_of(&dir, "a", 1.0, base, 1.0),
        negative_scan_of(&dir, "b", -1.0, base, 1.0),
        // a frame given much more light: denser
        negative_scan_of(&dir, "dense", 0.5, base, 0.45),
        // scanned with more light: brighter base and picture alike
        negative_scan_of(&dir, "bright", -0.5, base.map(|c| c * 1.3), 1.0),
        // another film's base, far off
        negative_scan_of(&dir, "other", 0.0, [0.45, 0.33, 0.2], 1.0),
    ];
    let mut s = Session::new().with_fs();
    s.execute("library.import", &json!({"paths": paths.iter().map(|p| p.to_string_lossy()).collect::<Vec<_>>()})).unwrap();
    let ids: Vec<u64> = s.visible_cloned().iter().map(|p| p.0).collect();
    assert_eq!(ids.len(), 5);
    let develop = |s: &Session, id: u64| s.develop_of(crate::PhotoId(id)).unwrap().negative.clone();
    let name_of = |s: &Session, id: u64| s.catalog.photo(crate::PhotoId(id)).unwrap().file_name.clone();
    let by_name = |s: &Session, n: &str| *ids.iter().find(|id| name_of(s, **id).starts_with(n)).unwrap();
    let (a, b, dense, bright, other) = (by_name(&s, "a."), by_name(&s, "b."), by_name(&s, "dense"), by_name(&s, "bright"), by_name(&s, "other"));

    // zapped one by one first: each its own measurement
    s.execute("develop.negativeZap", &json!({"ids": ids})).unwrap();
    let alone = develop(&s, dense).measured.unwrap();
    assert!(develop(&s, a).roll.is_none());
    assert_ne!(develop(&s, a).measured, develop(&s, dense).measured);

    let r = s.execute("develop.negativeZap", &json!({"ids": ids, "roll": true})).unwrap();
    let roll = &r["roll"];
    let frames: Vec<u64> = roll["frames"].as_array().unwrap().iter().map(|v| v.as_u64().unwrap()).collect();
    assert_eq!(frames.len(), 4, "{r}");
    assert!(frames.contains(&bright), "scanned brighter, the same film: {r}");
    assert!(!frames.contains(&other) && roll["apart"] == json!([other]), "the other film is apart: {r}");
    let (na, nb, nd, nr, no) = (develop(&s, a), develop(&s, b), develop(&s, dense), develop(&s, bright), develop(&s, other));
    assert_eq!((na.roll, nb.roll, nd.roll, nr.roll, no.roll), (Some(4), Some(4), Some(4), Some(4), None));
    // one measurement for the roll, on each frame's own film base
    let relative = |n: &lightcraft_develop::Negative| {
        let (m, b) = (n.measured.unwrap(), n.base.unwrap());
        [0, 1, 2].map(|c| (m.black[c] / b[c], m.white[c] / b[c], m.balance[c]))
    };
    for n in [&nb, &nd, &nr] {
        for (x, y) in relative(&na).iter().zip(relative(n)) {
            assert!((x.0 - y.0).abs() < 1e-9 && (x.1 - y.1).abs() < 1e-9 && (x.2 - y.2).abs() < 1e-9, "{na:?} vs {n:?}");
        }
    }
    let base_of = na.base.unwrap();
    assert!((0..3).all(|c| (base_of[c] - f64::from(base[c])).abs() < 0.03), "a frame's own base: {base_of:?}");
    let bright_base = nr.base.unwrap();
    assert!((0..3).all(|c| (bright_base[c] / base_of[c] - 1.3).abs() < 0.08), "the brighter scan's own base: {bright_base:?}");
    // the dense frame alone set its own levels; in the roll it no longer does
    assert_ne!(nd.measured.unwrap(), alone);
    // each frame still has its own crop
    assert_ne!(s.develop_of(crate::PhotoId(a)).unwrap().crop.geometry, s.develop_of(crate::PhotoId(b)).unwrap().crop.geometry);

    // one undo step takes the whole roll back
    s.execute("edit.undo", &json!({})).unwrap();
    assert!(develop(&s, a).roll.is_none() && develop(&s, dense).measured == Some(alone));
    s.execute("edit.redo", &json!({})).unwrap();
    assert_eq!(develop(&s, a).roll, Some(4));

    // measuring a frame again measures it on its own
    s.execute("library.select", &json!({"ids": [a], "active": a})).unwrap();
    s.execute("develop.negative", &json!({"measure": true})).unwrap();
    assert!(develop(&s, a).roll.is_none());
    assert_eq!(develop(&s, b).roll, Some(4), "the rest stay in the roll");
    // zapping a frame alone, too
    s.execute("develop.negativeZap", &json!({"ids": [b]})).unwrap();
    assert!(develop(&s, b).roll.is_none());
    // a roll of one is no roll
    let r = s.execute("develop.negativeZap", &json!({"ids": [dense], "roll": true})).unwrap();
    assert!(r["roll"].is_null() && develop(&s, dense).roll.is_none(), "{r}");
    assert!(s.execute("develop.negativeZap", &json!({"ids": ids, "roll": "yes"})).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

/// Zap! on a roll of real scans, each frame alone and then as one roll: prints each frame's mean
/// L*a*b* and how far the frames spread, and writes both renders side by side. Real scans aren't
/// committed: set LC_ROLL_DIR=<folder of one roll's scans> and LC_ZAP_OUT=<folder>.
#[test]
#[ignore = "needs a folder of real scans"]
fn zap_roll_on_real_scans() {
    let (Ok(dir), Ok(out)) = (std::env::var("LC_ROLL_DIR"), std::env::var("LC_ZAP_OUT")) else {
        lightcraft_testkit::skip!(Input, "set LC_ROLL_DIR=<folder of one roll's scans> and LC_ZAP_OUT=<folder>");
        return;
    };
    let out = std::path::PathBuf::from(out);
    std::fs::create_dir_all(&out).unwrap();
    let mut s = Session::new().with_fs();
    s.execute("library.import", &json!({"paths": [dir]})).unwrap();
    let ids: Vec<u64> = s.visible_cloned().iter().map(|p| p.0).collect();
    let format = std::env::var("LC_ZAP_FORMAT").unwrap_or_else(|_| "auto".into());
    let sprockets = std::env::var("LC_ZAP_SPROCKETS").unwrap_or_else(|_| "remove".into());
    let p = |roll: bool| json!({"ids": ids, "format": format, "sprockets": sprockets, "roll": roll});
    // mean L*a*b* of a render (sRGB, D65)
    let lab = |img: &lightcraft_raster::Rgba8| {
        let lin = |v: u8| {
            let c = f64::from(v) / 255.0;
            if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
        };
        let f = |t: f64| if t > 0.008856 { t.cbrt() } else { 7.787 * t + 16.0 / 116.0 };
        let mut acc = [0.0f64; 3];
        let mut n = 0.0f64;
        for px in &img.data {
            let (r, g, b) = (lin(px[0]), lin(px[1]), lin(px[2]));
            let x = (0.4124 * r + 0.3576 * g + 0.1805 * b) / 0.95047;
            let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
            let z = (0.0193 * r + 0.1192 * g + 0.9505 * b) / 1.08883;
            let (fx, fy, fz) = (f(x), f(y), f(z));
            acc[0] += 116.0 * fy - 16.0;
            acc[1] += 500.0 * (fx - fy);
            acc[2] += 200.0 * (fy - fz);
            n += 1.0;
        }
        acc.map(|v| v / n.max(1.0))
    };
    let mut results = Vec::new();
    for roll in [false, true] {
        let r = s.execute("develop.negativeZap", &p(roll)).unwrap();
        println!("roll={roll}: changed {} failed {} roll {}", r["changed"], r["failed"].as_array().map_or(0, Vec::len), r["roll"]);
        let mut per = Vec::new();
        for id in &ids {
            let id = crate::PhotoId(*id);
            if !s.develop_of(id).unwrap().negative.enabled {
                continue;
            }
            let img = s.render_now(id, 600, 600).unwrap().image;
            per.push((id, lab(&img), img));
        }
        results.push(per);
    }
    let spread = |per: &[(crate::PhotoId, [f64; 3], lightcraft_raster::Rgba8)]| {
        let n = per.len().max(1) as f64;
        let mean: Vec<f64> = (0..3).map(|c| per.iter().map(|p| p.1[c]).sum::<f64>() / n).collect();
        (0..3).map(|c| (per.iter().map(|p| (p.1[c] - mean[c]).powi(2)).sum::<f64>() / n).sqrt()).collect::<Vec<_>>()
    };
    for (single, roll) in results[0].iter().zip(&results[1]) {
        let name = s.catalog.photo(single.0).map(|p| p.file_name.clone()).unwrap_or_default();
        println!(
            "{name}: single L {:.1} a {:.1} b {:.1} | roll L {:.1} a {:.1} b {:.1}",
            single.1[0], single.1[1], single.1[2], roll.1[0], roll.1[1], roll.1[2]
        );
        // side by side: alone | roll
        let (a, b) = (&single.2, &roll.2);
        let (w, h) = (a.width + b.width + 8, a.height.max(b.height));
        let mut img = lightcraft_raster::Rgba8::new(w, h);
        for (src, x0) in [(a, 0), (b, a.width + 8)] {
            for y in 0..src.height {
                for x in 0..src.width {
                    img.set(x0 + x, y, src.get(x, y));
                }
            }
        }
        write_ppm(&out.join(format!("{}.ppm", name.rsplit_once('.').map_or(name.as_str(), |n| n.0))), &img);
    }
    let (a, b) = (spread(&results[0]), spread(&results[1]));
    println!("SPREAD single L {:.2} a {:.2} b {:.2} | roll L {:.2} a {:.2} b {:.2}", a[0], a[1], a[2], b[0], b[1], b[2]);
}

/// Opt-in: Zap! on every scan of a truth file (`LC_ZAP_TRUTH`: `{path: {film, ..}}`, the user's
/// crops), each folder zapped as one selection, the crops written to `LC_ZAP_RESULT` as
/// `{path: {rect, angle}}` for a scorer to compare. `LC_ZAP_ONLY` keeps the paths containing it;
/// `LC_ZAP_ROLL` zaps each folder as one roll. Each photo's conversion (`negative`) is written too, so two
/// builds can be compared for identical results.
#[test]
#[ignore = "needs the user's scans"]
fn zap_eval_on_truth() {
    let (Ok(truth), Ok(result)) = (std::env::var("LC_ZAP_TRUTH"), std::env::var("LC_ZAP_RESULT")) else {
        lightcraft_testkit::skip!(Input, "set LC_ZAP_TRUTH=<truth.json> and LC_ZAP_RESULT=<out.json>");
        return;
    };
    let only = std::env::var("LC_ZAP_ONLY").unwrap_or_default();
    let truth: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&std::fs::read_to_string(truth).unwrap()).unwrap();
    // one selection per folder and film, as a user zaps a roll
    let mut groups: std::collections::BTreeMap<(String, String), Vec<String>> = Default::default();
    for (path, t) in &truth {
        if !path.contains(&only) {
            continue;
        }
        let dir = std::path::Path::new(path).parent().map(|d| d.to_string_lossy().to_string()).unwrap_or_default();
        let film = t["film"].as_str().unwrap_or("negative").to_string();
        groups.entry((dir, film)).or_default().push(path.clone());
    }
    let mut out = serde_json::Map::new();
    for ((dir, film), paths) in groups {
        let mut s = Session::new().with_fs();
        s.execute("library.import", &json!({"paths": paths})).unwrap();
        let ids: Vec<crate::PhotoId> = s.visible_cloned();
        let roll = std::env::var_os("LC_ZAP_ROLL").is_some();
        let r = s.execute("develop.negativeZap", &json!({"ids": ids.iter().map(|i| i.0).collect::<Vec<_>>(), "film": film, "roll": roll}));
        eprintln!("{dir} ({film}, {}): {}", ids.len(), r.as_ref().map(|v| v["failed"].to_string()).unwrap_or_else(|e| e.to_string()));
        for id in ids {
            let (Some(p), Some(d)) = (s.catalog.photo(id), s.develop_of(id)) else { continue };
            let lightcraft_catalog::Source::File { path } = &p.source else { continue };
            let g = d.crop.geometry;
            let zapped = d.negative.zap_crop.is_some();
            out.insert(path.clone(), json!({"rect": [g.rect.x0, g.rect.y0, g.rect.x1, g.rect.y1], "angle": g.angle, "zapped": zapped, "orientation": format!("{:?}", d.orientation), "negative": serde_json::to_value(&d.negative).unwrap()}));
        }
    }
    std::fs::write(result, serde_json::to_string(&out).unwrap()).unwrap();
}

/// Zap! on a selection runs a few photos at once: each gets exactly what Zap! on it alone gives,
/// and the run lists them in the selection's order.
#[test]
fn zap_on_a_selection_matches_zap_one_photo_at_a_time() {
    let dir = std::env::temp_dir().join(format!("lc-zap-parallel-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let base = [0.62, 0.26, 0.07];
    let paths: Vec<String> = [(1.0, 1.0), (-1.0, 1.0), (0.5, 0.45), (-0.5, 0.8), (0.0, 1.2), (0.8, 0.6)]
        .iter()
        .enumerate()
        .map(|(i, (angle, density))| negative_scan_of(&dir, &format!("f{i}"), *angle, base, *density).to_string_lossy().to_string())
        .collect();
    let zapped = |ids: Option<&[u64]>, roll: bool| {
        let mut s = Session::new().with_fs();
        s.execute("library.import", &json!({"paths": paths})).unwrap();
        let all: Vec<u64> = s.visible_cloned().iter().map(|p| p.0).collect();
        match ids {
            // one photo per call
            Some(_) => {
                for id in &all {
                    s.execute("develop.negativeZap", &json!({"ids": [id]})).unwrap();
                }
            }
            None => {
                let r = s.execute("develop.negativeZap", &json!({"ids": all, "roll": roll})).unwrap();
                assert_eq!(r["changed"], json!(all.len()), "{r}");
            }
        }
        all.iter().map(|id| s.develop_of(crate::PhotoId(*id)).unwrap()).map(|d| (d.crop.geometry, d.negative.clone())).collect::<Vec<_>>()
    };
    assert_eq!(zapped(None, false), zapped(Some(&[]), false), "a selection zaps each photo as Zap! on it alone");
    assert_eq!(zapped(None, true), zapped(None, true), "a roll comes out the same every time");
    std::fs::remove_dir_all(&dir).ok();
}

/// The photos of a run come back in their order whatever the number of threads, and a stop
/// leaves the photos not yet started out.
#[test]
fn map_bounded_keeps_the_order_and_stops() {
    let items: Vec<u64> = (0..37).collect();
    let f = |v: &u64| {
        // uneven work, so the threads finish out of order
        std::thread::sleep(std::time::Duration::from_micros((v * 7919) % 500));
        v * v
    };
    let one = crate::negative_zap::map_bounded(&items, 1, &|| false, &|_| {}, &f);
    let done = std::sync::atomic::AtomicUsize::new(0);
    let four = crate::negative_zap::map_bounded(&items, 4, &|| false, &|n| _ = done.fetch_max(n, std::sync::atomic::Ordering::Relaxed), &f);
    assert_eq!(one, four);
    assert_eq!(four, items.iter().map(|v| Some(v * v)).collect::<Vec<_>>());
    assert_eq!(done.into_inner(), items.len());
    let stop = std::sync::atomic::AtomicBool::new(false);
    let stopped = crate::negative_zap::map_bounded(
        &items,
        4,
        &|| stop.load(std::sync::atomic::Ordering::Relaxed),
        &|n| {
            if n >= 5 {
                stop.store(true, std::sync::atomic::Ordering::Relaxed);
            }
        },
        &f,
    );
    let ran = stopped.iter().flatten().count();
    assert!((5..items.len()).contains(&ran), "{ran}");
    assert!(crate::negative_zap::map_bounded(&Vec::<u64>::new(), 4, &|| false, &|_| {}, &f).is_empty());
    assert!((1..=crate::negative_zap::MAX_WORKERS).contains(&crate::negative_zap::workers()));
}
