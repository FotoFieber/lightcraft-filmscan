//! Convert Negative: switch the active photo's conversion on or off, measure the scan, choose the
//! film preset and the advanced choices, and pick the film base or a gray point from the scan. The
//! sliders are develop controls (`negative.*`, set with `develop.set`).

use lightcraft_develop::{DevelopSettings, Negative};
use lightcraft_geom::{CropGeometry, MIN_CROP_FRACTION, Rect};
use lightcraft_pipeline::negative::film::{Look, Preset, Tone, WbPreset};
use lightcraft_pipeline::negative_frame::{self, Margin};
use serde_json::{Value, json};

use super::{CommandSpec, bad, cmd, f64_req, has_active, str_param};
use crate::{Result, Session, media::SourceLevel};

/// Fraction of each edge the measurement leaves out by default (film holder, sprocket holes,
/// the border itself).
const DEFAULT_BORDER: f64 = 0.08;

/// The photo's conversion as the panel and the control channel see it.
fn state(n: &Negative) -> Value {
    let mut v = serde_json::to_value(n).unwrap_or(Value::Null);
    if let Some(o) = v.as_object_mut() {
        o.insert("presetName".into(), json!(lightcraft_pipeline::negative::preset_of(n).name));
    }
    v
}

/// A choice parameter: `None` when absent, `Some("")` for the preset's own, else a name `known`
/// accepts.
fn choice(p: &Value, key: &str, c: &str, known: impl Fn(&str) -> bool) -> Result<Option<String>> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) if s.is_empty() || known(s) => Ok(Some(s.clone())),
        Some(v) => Err(bad(c, format!("unknown `{key}` {v}"))),
    }
}

/// Measures photo `id` inside its crop (the crop's box, straightening left aside), `border` in
/// from each edge.
fn measure(s: &mut Session, id: crate::PhotoId, border: f64, c: &str) -> Result<lightcraft_develop::NegativeMeasurement> {
    let src = s.source_now(id, SourceLevel::Preview).map_err(|e| bad(c, e))?;
    let d = s.develop_of(id).unwrap_or_default();
    measure_in(&src.oriented(d.orientation), d.crop.geometry.rect, border, c)
}

/// Measures the oriented scan `o` inside `r` (normalized), `border` in from each edge.
fn measure_in(o: &lightcraft_raster::Rgb32f, r: Rect, border: f64, c: &str) -> Result<lightcraft_develop::NegativeMeasurement> {
    let (w, h) = (o.width as f64, o.height as f64);
    let at = |v: f64, n: f64| if v.is_finite() { (v.clamp(0.0, 1.0) * n).round() as usize } else { 0 };
    let (x0, y0, x1, y1) = (at(r.x0, w), at(r.y0, h), at(r.x1, w), at(r.y1, h));
    let framed = if x1 > x0 && y1 > y0 { o.crop(x0, y0, x1 - x0, y1 - y0) } else { o.clone() };
    lightcraft_pipeline::negative::measure(&framed, border).map_err(|e| bad(c, e))
}

/// `develop.negativeZap`: finds the picture on the active scan, then straightens and crops to it,
/// takes the film base from the rebate and measures the picture, as one edit.
fn zap(s: &mut Session, p: &Value) -> Result<Value> {
    let c = "develop.negativeZap";
    let margin = match p.get("margin") {
        None | Some(Value::Null) => Margin::Inside,
        Some(Value::String(m)) => Margin::parse(m).ok_or_else(|| bad(c, format!("unknown margin `{m}` (inside, edge or border)")))?,
        Some(v) => return Err(bad(c, format!("`margin` must be inside, edge or border, not {v}"))),
    };
    let ratio = match p.get("format") {
        None | Some(Value::Null) => None,
        Some(Value::String(f)) => negative_frame::format_ratio(f).map_err(|e| bad(c, e))?,
        Some(v) => return Err(bad(c, format!("`format` must be a film format id, not {v}"))),
    };
    let id = s.active().ok_or_else(|| bad(c, "no active photo"))?;
    let src = s.source_now(id, SourceLevel::Preview).map_err(|e| bad(c, e))?;
    let d = s.develop_of(id).unwrap_or_default();
    let o = src.oriented(d.orientation);
    let frame = negative_frame::detect(&o, ratio).map_err(|e| bad(c, e))?;
    let (w, h) = super::develop::crop_dims(s, id);
    let rect = frame.crop(margin, o.width as f64, o.height as f64);
    let geometry = CropGeometry { rect, angle: frame.angle }.constrained(w, h);
    let min = MIN_CROP_FRACTION * w.min(h);
    if geometry.rect.width() * w < min || geometry.rect.height() * h < min {
        return Err(bad(c, "no film frame found"));
    }
    // measured on the picture itself: nothing of the rebate left to leave out
    let measured = measure_in(&o, geometry.rect, 0.0, c)?;
    let mut out = Value::Null;
    super::develop::edit(s, c, "Zap", |d: &mut DevelopSettings| {
        d.crop.geometry = geometry;
        let n = &mut d.negative;
        n.enabled = true;
        n.measured = Some(measured);
        if let Some(b) = frame.base {
            // the picked base only counts in Advanced
            n.advanced = true;
            n.base = Some(b);
        }
        out = state(n);
        Ok(())
    })?;
    if let Some(o) = out.as_object_mut() {
        o.insert(
            "zap".into(),
            json!({
                "angle": geometry.angle,
                "rect": [geometry.rect.x0, geometry.rect.y0, geometry.rect.x1, geometry.rect.y1],
                "base": frame.base,
                "sprockets": frame.sprockets,
                "sidesFound": frame.sides_found,
            }),
        );
    }
    Ok(out)
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "develop.negative",
            "Convert Negative",
            [],
            None,
            "{on?: bool (default true), advanced?: bool, preset?: film preset name, tone?, look?, wb?: names (\"\" = the preset's), measure?: bool (measure the scan again), border?: 0..45 (% of each edge the measurement leaves out, default 8), clearBase?: bool, clearGray?: bool} — the photo is a film negative, converted to a positive; switching it on measures the scan → the conversion",
            has_active,
            |s, p| {
                let c = "develop.negative";
                let id = s.active().ok_or_else(|| bad(c, "no active photo"))?;
                let preset = match str_param(p, "preset") {
                    Some(name) => Some(Preset::parse(name).ok_or_else(|| bad(c, format!("unknown film preset `{name}`")))?.name),
                    None if p.get("preset").is_some_and(|v| !v.is_null()) => return Err(bad(c, "`preset` must be a name")),
                    None => None,
                };
                let tone = choice(p, "tone", c, |n| Tone::parse(n).is_some())?;
                let look = choice(p, "look", c, |n| Look::parse(n).is_some())?;
                let wb = choice(p, "wb", c, |n| WbPreset::parse(n).is_some())?;
                let border = match p.get("border") {
                    None | Some(Value::Null) => DEFAULT_BORDER,
                    Some(v) => match v.as_f64() {
                        Some(b) if b.is_finite() && (0.0..=45.0).contains(&b) => b / 100.0,
                        _ => return Err(bad(c, "`border` must be a number from 0 to 45 (%)")),
                    },
                };
                let current = s.develop_of(id).unwrap_or_default().negative.clone();
                let on = p.get("on").and_then(Value::as_bool).unwrap_or(true);
                let remeasure = p.get("measure").and_then(Value::as_bool).unwrap_or(false);
                // Measured when the conversion goes on without a measurement, or on request.
                let measured = if on && (remeasure || current.measured.is_none()) { Some(measure(s, id, border, c)?) } else { None };
                let mut out = Value::Null;
                super::develop::edit(s, c, "Convert Negative", |d: &mut DevelopSettings| {
                    let n = &mut d.negative;
                    n.enabled = on;
                    if let Some(a) = p.get("advanced").and_then(Value::as_bool) {
                        n.advanced = a;
                    }
                    if let Some(name) = preset {
                        n.preset = name.to_string();
                    }
                    if let Some(t) = tone {
                        n.tone = t;
                    }
                    if let Some(l) = look {
                        n.look = l;
                    }
                    if let Some(w) = wb {
                        n.wb = w;
                    }
                    if p.get("clearBase").and_then(Value::as_bool).unwrap_or(false) {
                        n.base = None;
                    }
                    if p.get("clearGray").and_then(Value::as_bool).unwrap_or(false) {
                        n.gray = None;
                    }
                    if measured.is_some() {
                        n.measured = measured;
                    }
                    out = state(n);
                    Ok(())
                })?;
                Ok(out)
            }
        ),
        cmd!(
            "develop.negativePick",
            "Film Base Selector",
            [],
            None,
            "{x, y: normalized image point, target?: base|gray (default base)} — the colour of the scan there is the film base (the unexposed border) or a gray point; switches the conversion on and to Advanced",
            has_active,
            |s, p| {
                let c = "develop.negativePick";
                let (x, y) = (f64_req(p, "x", c)?, f64_req(p, "y", c)?);
                if !(x.is_finite() && y.is_finite()) {
                    return Err(bad(c, "`x` and `y` must be numbers"));
                }
                let gray = match str_param(p, "target") {
                    None | Some("base") => false,
                    Some("gray") => true,
                    Some(t) => return Err(bad(c, format!("unknown target `{t}` (base or gray)"))),
                };
                let id = s.active().ok_or_else(|| bad(c, "no active photo"))?;
                let src = s.source_now(id, SourceLevel::Thumb).map_err(|e| bad(c, e))?;
                let d = s.develop_of(id).unwrap_or_default();
                // A small neighbourhood of the oriented scan (the negative, before any conversion).
                let o = src.oriented(d.orientation);
                if o.width == 0 || o.height == 0 {
                    return Err(bad(c, "the photo has no pixels"));
                }
                let (cx, cy) = ((x.clamp(0.0, 1.0) * o.width as f64) as isize, (y.clamp(0.0, 1.0) * o.height as f64) as isize);
                let mut acc = [0.0f32; 3];
                for dy in -2..=2 {
                    for dx in -2..=2 {
                        let px = o.get_clamped(cx + dx, cy + dy);
                        for i in 0..3 {
                            acc[i] += px[i] / 25.0;
                        }
                    }
                }
                let colour = lightcraft_pipeline::negative::scan_colour(acc);
                let measured = if d.negative.measured.is_none() { Some(measure(s, id, DEFAULT_BORDER, c)?) } else { None };
                let mut out = Value::Null;
                super::develop::edit(s, c, if gray { "Gray Point" } else { "Film Base" }, |d: &mut DevelopSettings| {
                    let n = &mut d.negative;
                    n.enabled = true;
                    n.advanced = true;
                    if measured.is_some() {
                        n.measured = measured;
                    }
                    if gray {
                        n.gray = Some(colour);
                    } else {
                        n.base = Some(colour);
                    }
                    out = state(n);
                    Ok(())
                })?;
                Ok(out)
            }
        ),
        cmd!(
            "develop.negativeZap",
            "Zap!",
            [],
            None,
            "{margin?: inside|edge|border (default inside), format?: auto|35mm|half|xpan|645|66|67|68|69|612|617 (default auto: no aspect assumed)} — finds the picture on the scan of a negative (the holder, the film's rebate and its sprocket holes left out), straightens and crops to it, takes the film base from the rebate and measures the picture; one undo step",
            has_active,
            zap
        ),
    ]
}
