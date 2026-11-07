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
        let hole = |cy: f64| ((x - 40.0).rem_euclid(56.0) - 28.0).powi(2) / 196.0 + (y - cy).powi(2) / 64.0 < 1.0;
        if hole(38.0) || hole(548.0) {
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
    let r = s.execute("develop.negativeZap", &json!({"margin": "edge", "format": "35mm"})).unwrap();
    let d = s.develop_of(id).unwrap();
    let g = d.crop.geometry;
    assert!((g.angle - 1.5).abs() < 0.25, "{r}");
    let want = [120.0 / 900.0, 80.0 / 600.0, 780.0 / 900.0, 520.0 / 600.0];
    let got = [g.rect.x0, g.rect.y0, g.rect.x1, g.rect.y1];
    assert!(want.iter().zip(got).all(|(a, b)| (a - b).abs() < 0.015), "{got:?} {r}");
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
    assert!(inside.width() < g.rect.width() && inside.height() < g.rect.height(), "{inside:?} vs {:?}", g.rect);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn zap_rejects_bad_arguments_and_photos_without_a_frame() {
    let mut s = Session::with_demo();
    for p in [json!({"margin": "wide"}), json!({"margin": 3}), json!({"format": "8x10"}), json!({"format": true})] {
        assert!(s.execute("develop.negativeZap", &p).is_err(), "{p}");
    }
    // a demo photo is no negative in a holder: an error, nothing changed
    let id = s.active().unwrap();
    let before = s.develop_of(id).unwrap();
    let _ = s.execute("develop.negativeZap", &json!({}));
    let after = s.develop_of(id).unwrap();
    assert!(after == before || after.negative.enabled, "either untouched or a frame was found");
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
        let r = s.execute("develop.negativeZap", &json!({"format": format}));
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
        }
        if r.is_ok() {
            let rendered = s.render_now(id, 900, 900).unwrap();
            write_ppm(&out.join(format!("{name}-zap.ppm")), &rendered.image);
        }
    }
}
