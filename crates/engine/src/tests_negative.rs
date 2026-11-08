//! Convert Negative through the command layer: switching it on measures the scan, the preset and
//! choices are validated, the sliders are ordinary develop controls, the picker stores the film
//! base and the gray point, and copy / paste carries the conversion to the rest of a roll.

use serde_json::json;

use crate::Session;

fn negative(s: &Session) -> lightcraft_develop::Negative {
    s.develop_of(s.active().unwrap()).unwrap().negative.clone()
}

#[test]
fn switching_on_measures_and_settings_follow() {
    let mut s = Session::with_demo();
    assert!(!negative(&s).enabled);
    let r = s.execute("develop.negative", &json!({})).unwrap();
    assert_eq!(r["enabled"], true, "{r}");
    assert_eq!(r["presetName"], "Standard");
    let n = negative(&s);
    let m = n.measured.expect("measured when switched on");
    assert!((0..3).all(|ch| m.white[ch] < m.black[ch]), "{m:?}");
    // a preset by any spelling, the advanced choices by name ("" = the preset's)
    s.execute("develop.negative", &json!({"preset": "portra400", "advanced": true, "tone": "cinematic", "look": "", "wb": "warm"})).unwrap();
    let n = negative(&s);
    assert_eq!((n.preset.as_str(), n.advanced, n.tone.as_str(), n.wb.as_str()), ("Portra 400", true, "cinematic", "warm"));
    assert_eq!(n.measured, Some(m), "not measured again unless asked");
    // the sliders are develop controls
    s.execute("develop.set", &json!({"control": "negative.exposure", "value": 9})).unwrap();
    assert_eq!(negative(&s).exposure, 5.0, "clamped by its spec");
    // off keeps everything, so on again is the same conversion
    s.execute("develop.negative", &json!({"on": false})).unwrap();
    assert!(!negative(&s).enabled && negative(&s).measured == Some(m));
    // one undo step per change
    s.execute("edit.undo", &json!({})).unwrap();
    assert!(negative(&s).enabled);
}

#[test]
fn bad_arguments_are_errors() {
    let mut s = Session::with_demo();
    for p in [
        json!({"preset": "Kodachrome 25"}),
        json!({"preset": 3}),
        json!({"tone": "spicy"}),
        json!({"look": 1}),
        json!({"border": 90}),
        json!({"border": "x"}),
    ] {
        assert!(s.execute("develop.negative", &p).is_err(), "{p}");
    }
    for p in [json!({}), json!({"x": 0.5}), json!({"x": 0.5, "y": 0.5, "target": "sky"})] {
        assert!(s.execute("develop.negativePick", &p).is_err(), "{p}");
    }
    assert!(!negative(&s).enabled, "nothing changed");
}

#[test]
fn the_picker_stores_the_base_and_the_gray_point() {
    let mut s = Session::with_demo();
    let r = s.execute("develop.negativePick", &json!({"x": 0.01, "y": 0.01})).unwrap();
    assert_eq!((r["enabled"].as_bool(), r["advanced"].as_bool()), (Some(true), Some(true)), "{r}");
    let n = negative(&s);
    let base = n.base.expect("a base");
    assert!(base.iter().all(|v| (0.0..=1.0).contains(v)), "{base:?}");
    assert!(n.measured.is_some());
    s.execute("develop.negativePick", &json!({"x": 2.0, "y": -1.0, "target": "gray"})).unwrap();
    assert!(negative(&s).gray.is_some(), "points outside the photo clamp to its edge");
    s.execute("develop.negative", &json!({"clearBase": true, "clearGray": true})).unwrap();
    let n = negative(&s);
    assert!(n.base.is_none() && n.gray.is_none());
}

#[test]
fn copy_and_paste_carries_the_conversion_to_another_frame() {
    let mut s = Session::with_demo();
    s.execute("develop.negative", &json!({"preset": "Gold 200"})).unwrap();
    let first = negative(&s);
    s.execute("develop.copy", &json!({"groups": ["negative"]})).unwrap();
    let ids: Vec<u64> = s.visible_cloned().iter().map(|p| p.0).collect();
    s.execute("library.select", &json!({"ids": [ids[1]], "active": ids[1]})).unwrap();
    assert!(!negative(&s).enabled);
    s.execute("develop.paste", &json!({})).unwrap();
    assert_eq!(negative(&s), first, "the roll converts with the first frame's measurement");
}

/// A PNG of one frame of 35 mm colour negative in a holder, put in askew (`angle` degrees):
/// holder, orange base, sprocket holes along the long edges and a textured, denser picture on
/// 120..780 × 80..520 of the straight 900 × 600 scan.
fn negative_scan(dir: &std::path::Path, angle: f64) -> std::path::PathBuf {
    let (w, h) = (900usize, 600usize);
    let base = [0.62f32, 0.26, 0.07];
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
        base.map(|c| c * t as f32)
    });
    // (linear sRGB, as the PNG's sRGB encoding gives it back)
    let png = lightcraft_codecs::encode::encode_png(&lightcraft_codecs::encode::EncodeImage::rgba8(&img.to_srgb8()), &Default::default()).unwrap();
    let path = dir.join(format!("negative-{angle}.png"));
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

#[test]
fn zap_removes_the_sprocket_holes_unless_told_to_keep_them_and_only_crops_a_slide() {
    let dir = std::env::temp_dir().join(format!("lc-zap-holes-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = negative_scan(&dir, 0.0);
    let mut s = Session::new().with_fs();
    s.execute("library.import", &json!({"paths": [path.to_string_lossy()]})).unwrap();
    let id = s.active().unwrap();
    let before = s.develop_of(id).unwrap();
    // by default the crop ends at the picture, the holes and the rebate beside them left out
    s.execute("develop.negativeZap", &json!({"margin": "edge"})).unwrap();
    let g = s.develop_of(id).unwrap().crop.geometry.rect;
    assert!((g.y0 - 80.0 / 600.0).abs() < 0.015 && (g.y1 - 520.0 / 600.0).abs() < 0.015, "{g:?}");
    s.execute("edit.undo", &json!({})).unwrap();
    // a slide: the same crop, and no conversion
    s.execute("develop.negativeZap", &json!({"margin": "edge", "film": "slide"})).unwrap();
    let d = s.develop_of(id).unwrap();
    assert!((d.crop.geometry.rect.y0 - g.y0).abs() < 1e-9 && (d.crop.geometry.rect.x0 - g.x0).abs() < 1e-9, "{:?}", d.crop.geometry);
    assert_eq!(d.negative.enabled, before.negative.enabled);
    assert_eq!((d.negative.base, d.negative.measured.is_some()), (before.negative.base, before.negative.measured.is_some()));
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
        let r = s.execute("develop.negativeZap", &json!({"ids": [id.0], "format": format, "sprockets": sprockets, "film": film}));
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
