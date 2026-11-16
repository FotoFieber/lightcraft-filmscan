//! Convert Negative's Zap! on one photo, apart from the session: finds the picture on the scan,
//! straightens and crops to it, takes the film base from the rebate and measures the picture.
//! `develop.negativeZap` runs it on each target photo; the app runs it off the UI thread for a
//! selection and commits the results the same way, as one undo step.

use lightcraft_catalog::{Photo, PhotoId};
use lightcraft_develop::{DevelopSettings, NegativeMeasurement};
use lightcraft_geom::{CropGeometry, MIN_CROP_FRACTION, Orientation, Rect};
use lightcraft_pipeline::negative::film::ValueHistogram;
use lightcraft_pipeline::negative_frame::{self, Frame, Margin};
use lightcraft_raster::Rgb32f;
use serde_json::{Value, json};
use std::sync::Arc;

use crate::Session;
use crate::media::{SourceLevel, SourceRef};

/// The label of Zap!'s undo step and History entries.
pub const LABEL: &str = "Zap";

/// Whether the crop keeps 35 mm film's sprocket holes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Sprockets {
    /// The crop ends at the picture; the holes stay out.
    #[default]
    Remove,
    /// The long sides go to the holes (just outside whole ones, just inside ones the holder cuts).
    Keep,
}

/// What was scanned.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Film {
    /// A negative: cropped, and converted with the base from its rebate.
    #[default]
    Negative,
    /// A black-and-white negative: found, cropped and measured as a colour one, converted with
    /// the monochrome film profile ([`MONOCHROME`]). The base still comes from the rebate, never
    /// assumed grey: a chromogenic film (Ilford XP2) has a coloured base.
    BwNegative,
    /// A slide (a positive): only cropped, never converted.
    Slide,
}

/// The film profile a black-and-white negative converts with.
pub const MONOCHROME: &str = "Monochrome";

/// The film profile a colour negative falls back to when it had a monochrome one.
const STANDARD: &str = "Standard";

/// Where the crop sits, the film format, the sprocket holes and the kind of film, the same for
/// every photo of a run.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub margin: Margin,
    /// The picture's long / short side, `None` for no format assumed.
    pub ratio: Option<f64>,
    pub sprockets: Sprockets,
    pub film: Film,
    /// The photos are one roll: their negatives are measured together ([`pool`]).
    pub roll: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self { margin: Margin::Inside, ratio: None, sprockets: Sprockets::Remove, film: Film::Negative, roll: false }
    }
}

impl Options {
    /// From the command's `margin` and `format` parameters.
    pub fn from_params(p: &Value) -> Result<Self, String> {
        let margin = match p.get("margin") {
            None | Some(Value::Null) => Margin::Inside,
            Some(Value::String(m)) => Margin::parse(m).ok_or_else(|| format!("unknown margin `{m}` (inside, edge, border or film)"))?,
            Some(v) => return Err(format!("`margin` must be inside, edge, border or film, not {v}")),
        };
        let ratio = match p.get("format") {
            None | Some(Value::Null) => None,
            Some(Value::String(f)) => negative_frame::format_ratio(f)?,
            Some(v) => return Err(format!("`format` must be a film format id, not {v}")),
        };
        let sprockets = match p.get("sprockets") {
            None | Some(Value::Null) => Sprockets::Remove,
            Some(Value::String(v)) if v == "remove" => Sprockets::Remove,
            Some(Value::String(v)) if v == "keep" => Sprockets::Keep,
            Some(v) => return Err(format!("`sprockets` must be remove or keep, not {v}")),
        };
        let film = match p.get("film") {
            None | Some(Value::Null) => Film::Negative,
            Some(Value::String(v)) if v == "negative" => Film::Negative,
            Some(Value::String(v)) if v == "bw" => Film::BwNegative,
            Some(Value::String(v)) if v == "slide" => Film::Slide,
            Some(v) => return Err(format!("`film` must be negative, bw or slide, not {v}")),
        };
        let roll = match p.get("roll") {
            None | Some(Value::Null) => false,
            Some(Value::Bool(b)) => *b,
            Some(v) => return Err(format!("`roll` must be true or false, not {v}")),
        };
        Ok(Self { margin, ratio, sprockets, film, roll })
    }
}

/// One photo's Zap!: its new settings and what was found.
#[derive(Clone, Debug)]
pub struct Zapped {
    pub settings: DevelopSettings,
    /// `{angle, rect, base, sprockets, sidesFound}`.
    pub report: Value,
    /// For a roll ([`Options::roll`]): the samples its measurement read, for [`pool`].
    pub samples: Option<Box<RollSamples>>,
}

/// One frame's share of a roll's measurement.
#[derive(Clone, Debug)]
pub struct RollSamples {
    /// The picture's samples, relative to `base` when there is one.
    pub samples: ValueHistogram,
    /// The film base its rebate showed (linear sRGB), `None` when none showed.
    pub base: Option<[f64; 3]>,
}

/// The photo's size with its orientation applied: the frame its crop is normalized to.
pub fn oriented_dims(p: &Photo) -> (f64, f64) {
    let (w, h) = (p.width.max(1) as f64, p.height.max(1) as f64);
    if p.develop.orientation.swaps_axes() { (h, w) } else { (w, h) }
}

/// The border a measurement inside the user's own crop leaves out when Zap! finds no frame to
/// measure on (as Convert Negative's own measurement: holder, holes, rebate).
const OWN_CROP_BORDER: f64 = 0.08;

/// The photo's crop is the user's own: set, and not the one Zap! set last (which a later Zap! may
/// set again). An unchanged full-frame crop is nobody's.
pub fn own_crop(d: &DevelopSettings) -> bool {
    let g = d.crop.geometry;
    let same = |a: &CropGeometry, b: &CropGeometry| {
        let (p, q) = (a.rect, b.rect);
        [(p.x0, q.x0), (p.y0, q.y0), (p.x1, q.x1), (p.y1, q.y1), (a.angle, b.angle)].iter().all(|(x, y)| (x - y).abs() < 1e-6)
    };
    !same(&g, &CropGeometry::default()) && !d.negative.zap_crop.is_some_and(|z| same(&z, &g))
}

/// Zap! on `scan` (the photo's source, before orientation) of a photo with settings `before` and
/// oriented size `dims`. A crop of the user's own ([`own_crop`]) stays as it is: Zap! then only
/// takes the film base and measures the picture (inside that crop when no frame is found). A
/// slide is only cropped.
pub fn zap(scan: &Rgb32f, before: &DevelopSettings, dims: (f64, f64), o: Options) -> Result<Zapped, String> {
    let oriented = scan.oriented(before.orientation);
    let (ow, oh) = (oriented.width as f64, oriented.height as f64);
    let found = match o.film {
        _ if o.margin == Margin::Film => negative_frame::detect_strip(&oriented),
        Film::Slide => negative_frame::detect_slide(&oriented, o.ratio),
        Film::Negative | Film::BwNegative => negative_frame::detect(&oriented, o.ratio),
    };
    // measured on the picture itself (inside its edge, never the holes or the rebate)
    let picture = |f: &Frame| Frame { holes: [None, None], ..*f }.crop(Margin::Inside, ow, oh);
    if o.film == Film::Slide {
        return slide(before, found, dims, o, (ow, oh));
    }
    // the picture as the crop shows it: in the frame turned by the frame's angle
    let picture_of = |f: &Frame| CropGeometry { rect: picture(f), angle: f.angle };
    if own_crop(before) {
        let geometry = before.crop.geometry;
        let (g, border, frame) = match &found {
            Ok(f) => (picture_of(f), 0.0, Some(f)),
            Err(_) => (geometry, OWN_CROP_BORDER, None),
        };
        let base = frame.and_then(|f| f.base);
        let (measured, samples) = measure_and_sample(scan, before, g, border, base, o.roll)?;
        let settings = converted(before, o.film, measured, base);
        return Ok(Zapped { settings, report: report(&geometry, frame, true), samples });
    }
    let frame = found?;
    let geometry = crop_of(&frame, dims, o, (ow, oh))?;
    let (measured, samples) = measure_and_sample(scan, before, picture_of(&frame), 0.0, frame.base, o.roll)?;
    let mut settings = converted(before, o.film, measured, frame.base);
    settings.crop.geometry = geometry;
    settings.negative.zap_crop = Some(geometry);
    Ok(Zapped { settings, report: report(&geometry, Some(&frame), false), samples })
}

/// [`measure_in`] and, for a roll's frame (`roll`), the samples a roll's measurement reads there
/// relative to `base` ([`RollSamples`]): the crop's pixels are cut out once for both.
fn measure_and_sample(
    scan: &Rgb32f,
    d: &DevelopSettings,
    g: CropGeometry,
    border: f64,
    base: Option<[f64; 3]>,
    roll: bool,
) -> Result<(NegativeMeasurement, Option<Box<RollSamples>>), String> {
    let px = framed(scan, d, g);
    let measured = lightcraft_pipeline::negative::measure(&px, border)?;
    let samples = if roll { Some(Box::new(RollSamples { samples: lightcraft_pipeline::negative::sample(&px, border, base)?, base })) } else { None };
    Ok((measured, samples))
}

/// The crop Zap! sets for `frame` on a scan of oriented size `dims` (analysed at `analysed`).
fn crop_of(frame: &Frame, dims: (f64, f64), o: Options, analysed: (f64, f64)) -> Result<CropGeometry, String> {
    let (w, h) = dims;
    let cut = match o.sprockets {
        Sprockets::Keep => *frame,
        Sprockets::Remove => Frame { holes: [None, None], ..*frame },
    };
    let rect = cut.crop(o.margin, analysed.0, analysed.1);
    let geometry = CropGeometry { rect, angle: frame.angle }.constrained(w, h);
    let min = MIN_CROP_FRACTION * w.min(h);
    if geometry.rect.width() * w < min || geometry.rect.height() * h < min {
        return Err("no film frame found".into());
    }
    Ok(geometry)
}

/// Zap! on a slide: only the crop, the conversion left as it is. A crop of the user's own stays.
fn slide(before: &DevelopSettings, found: Result<Frame, String>, dims: (f64, f64), o: Options, analysed: (f64, f64)) -> Result<Zapped, String> {
    if own_crop(before) {
        let geometry = before.crop.geometry;
        return Ok(Zapped { settings: before.clone(), report: report(&geometry, found.as_ref().ok(), true), samples: None });
    }
    let frame = found?;
    let geometry = crop_of(&frame, dims, o, analysed)?;
    let mut settings = before.clone();
    settings.crop.geometry = geometry;
    settings.negative.zap_crop = Some(geometry);
    Ok(Zapped { settings, report: report(&geometry, Some(&frame), false), samples: None })
}

/// `before` converted with `measured` and, when the rebate showed, the film base; with the
/// monochrome profile for a black-and-white negative, and away from it for a colour one (another
/// colour profile the user chose stays).
fn converted(before: &DevelopSettings, film: Film, measured: NegativeMeasurement, base: Option<[f64; 3]>) -> DevelopSettings {
    let mut settings = before.clone();
    let n = &mut settings.negative;
    let monochrome = lightcraft_pipeline::negative::preset_of(n).monochrome;
    if film == Film::BwNegative && !monochrome {
        n.preset = MONOCHROME.into();
    } else if film == Film::Negative && monochrome {
        n.preset = STANDARD.into();
    }
    n.enabled = true;
    n.measured = Some(measured);
    // measured on its own, out of any roll it was in
    n.roll = None;
    if let Some(b) = base {
        // the picked base only counts in Advanced
        n.advanced = true;
        n.base = Some(b);
    }
    settings
}

/// What Zap! did: `{angle, rect, base, sprockets, sidesFound, picture, holes, cropKept}`.
fn report(geometry: &CropGeometry, frame: Option<&Frame>, kept: bool) -> Value {
    let r = |r: Rect| json!([r.x0, r.y0, r.x1, r.y1]);
    json!({
        "angle": geometry.angle,
        "rect": r(geometry.rect),
        "base": frame.and_then(|f| f.base),
        "sprockets": frame.is_some_and(|f| f.sprockets),
        "sidesFound": frame.map_or(0, |f| f.sides_found),
        "picture": frame.map(|f| r(f.picture)),
        "holes": frame.map(|f| f.holes.map(|h| h.map(|h| json!({"outer": h.outer, "inner": h.inner, "whole": h.whole})))),
        "cropKept": kept,
    })
}

/// Measures `scan` (the photo's source, before orientation, with settings `d`) inside the crop
/// `g`, straightened as the render shows it, `border` in from each edge. Measure Again and Zap!
/// both measure here, so a turned crop reads the same pixels either way.
pub fn measure_in(scan: &Rgb32f, d: &DevelopSettings, g: CropGeometry, border: f64) -> Result<NegativeMeasurement, String> {
    lightcraft_pipeline::negative::measure(&framed(scan, d, g), border)
}

/// The pixels of `scan` inside the crop `g` of a photo with settings `d`. A turned crop is not a
/// box of the scan: it is sampled straightened, as the render does. Else the plain box of the
/// oriented scan (all of it when the box holds no pixel).
fn framed(scan: &Rgb32f, d: &DevelopSettings, g: CropGeometry) -> Rgb32f {
    if g.angle.is_finite() && g.angle != 0.0 {
        let mut d = d.clone();
        d.crop.geometry = g;
        let frame = lightcraft_pipeline::frame_for(scan, &lightcraft_pipeline::SourceInfo::default(), &d, true);
        let r = frame.crop.rect_px(frame.ow, frame.oh);
        // (a NaN side rounds to 0 and so becomes 1)
        let side = |v: f64| (v.round() as usize).clamp(1, 16384);
        return frame.sample(scan, side(r.x1 - r.x0), side(r.y1 - r.y0));
    }
    let o = scan.oriented(d.orientation);
    let r = g.rect;
    let (w, h) = (o.width as f64, o.height as f64);
    let at = |v: f64, n: f64| if v.is_finite() { (v.clamp(0.0, 1.0) * n).round() as usize } else { 0 };
    let (x0, y0, x1, y1) = (at(r.x0, w), at(r.y0, h), at(r.x1, w), at(r.y1, h));
    if x1 > x0 && y1 > y0 { o.crop(x0, y0, x1 - x0, y1 - y0) } else { o }
}

/// Fewest frames of one orientation that [`agree`] compares.
pub const AGREE_MIN: usize = 4;
/// How far a frame's crop edge may lie from the roll's (normalized) before [`agree`] moves it.
pub const AGREE_TOLERANCE: f64 = 0.05;
/// The share of the frames that must lie within [`AGREE_TOLERANCE`] of an edge's median for the
/// selection to agree on it (one holder, one scanner setup); a mixed selection (other films,
/// formats or holders) agrees on none and is left as it is.
pub const AGREE_SHARE: f64 = 0.75;

/// Frames of one roll scanned in one holder sit alike on their scans (for a selection zapped as one
/// roll, [`Options::roll`]): where most of its frames agree on a crop edge, a frame whose edge lies far from the others' (a dark scene taken
/// for rebate, a beam taken for the frame's edge) gets the roll's edge. Per orientation, among the
/// frames whose crop Zap! set (not a crop of the user's own or a slide's), at least [`AGREE_MIN`]
/// of them. A frame keeps its own angle. The ids whose crop moved, to be measured again.
pub fn agree(zapped: &mut [(PhotoId, Zapped)]) -> Vec<PhotoId> {
    let mut groups: Vec<(Orientation, Vec<usize>)> = Vec::new();
    for (i, (_, z)) in zapped.iter().enumerate() {
        let d = &z.settings;
        if d.negative.zap_crop != Some(d.crop.geometry) {
            continue;
        }
        match groups.iter_mut().find(|(o, _)| *o == d.orientation) {
            Some((_, v)) => v.push(i),
            None => groups.push((d.orientation, vec![i])),
        }
    }
    let mut moved = Vec::new();
    for (_, members) in groups.into_iter().filter(|(_, m)| m.len() >= AGREE_MIN) {
        for k in 0..4 {
            let mut v: Vec<f64> = members
                .iter()
                .filter_map(|i| zapped.get(*i))
                .map(|(_, z)| edges(z.settings.crop.geometry.rect)[k])
                .filter(|e| e.is_finite())
                .collect();
            v.sort_by(f64::total_cmp);
            let Some(median) = v.get(v.len() / 2).copied() else { continue };
            let close = v.iter().filter(|e| (*e - median).abs() <= AGREE_TOLERANCE).count();
            if (close as f64) < AGREE_SHARE * members.len() as f64 {
                continue;
            }
            for i in &members {
                let Some((id, z)) = zapped.get_mut(*i) else { continue };
                let g = &mut z.settings.crop.geometry;
                let mut e = edges(g.rect);
                if (e[k] - median).abs() > AGREE_TOLERANCE {
                    e[k] = median;
                    g.rect = Rect::new(e[0], e[1], e[2], e[3]);
                    z.settings.negative.zap_crop = Some(*g);
                    if !moved.contains(id) {
                        moved.push(*id);
                    }
                }
            }
        }
    }
    moved
}

fn edges(r: Rect) -> [f64; 4] {
    [r.x0, r.y0, r.x1, r.y1]
}

/// [`agree`], each moved frame measured again inside its new crop by `measure` (as [`remeasure`],
/// given the frame's own crop). A frame that can't be measured there keeps its own crop and
/// measurement: one frame never fails the selection. The ids whose crop moved.
pub fn agree_measured(zapped: &mut [(PhotoId, Zapped)], mut measure: impl FnMut(PhotoId, &mut Zapped, Rect) -> Result<(), String>) -> Vec<PhotoId> {
    let before: Vec<Zapped> = zapped.iter().map(|(_, z)| z.clone()).collect();
    let mut kept = Vec::new();
    for id in agree(zapped) {
        let Some(i) = zapped.iter().position(|(z, _)| *z == id) else { continue };
        let (Some((_, z)), Some(old)) = (zapped.get_mut(i), before.get(i)) else { continue };
        match measure(id, z, old.settings.crop.geometry.rect) {
            Ok(()) => kept.push(id),
            Err(_) => *z = old.clone(),
        }
    }
    kept
}

/// A strip of film this even (natural log of luminance, 10 % to 90 %) has no picture on it: the
/// base, or the fogged leader past a roll's last frame (about 0.4 on a camera scan, its noise in
/// the dark; the darkest scene beside it, thin trees on a B&W negative, 0.8).
pub const BLANK_SPREAD: f32 = 0.5;
/// Thinnest strip (normalized) [`widens_onto_blank`] reads.
const BLANK_STRIP_MIN: f64 = 0.005;

/// Whether the crop of `d` reaches past `own` (the frame's own crop) onto blank film on some side
/// ([`BLANK_SPREAD`]): a roll's last frame, half exposed, ends where its picture ends, whatever the
/// other frames' edges.
fn widens_onto_blank(scan: &Rgb32f, d: &DevelopSettings, own: Rect) -> bool {
    let g = d.crop.geometry;
    let n = g.rect;
    let strips = [
        (n.x0 < own.x0).then(|| Rect::new(n.x0, n.y0, own.x0, n.y1)),
        (n.y0 < own.y0).then(|| Rect::new(n.x0, n.y0, n.x1, own.y0)),
        (n.x1 > own.x1).then(|| Rect::new(own.x1, n.y0, n.x1, n.y1)),
        (n.y1 > own.y1).then(|| Rect::new(n.x0, own.y1, n.x1, n.y1)),
    ];
    strips.into_iter().flatten().filter(|r| r.width() >= BLANK_STRIP_MIN && r.height() >= BLANK_STRIP_MIN).any(|r| {
        let px = framed(scan, d, CropGeometry { rect: r, angle: g.angle });
        let mut v: Vec<f32> =
            px.data.iter().map(|p| (0.2627 * p[0] + 0.678 * p[1] + 0.0593 * p[2]).max(0.0) + 1e-4).map(f32::ln).filter(|v| v.is_finite()).collect();
        if v.len() < 50 {
            return false;
        }
        v.sort_by(f32::total_cmp);
        let q = |f: f64| v.get(((v.len() - 1) as f64 * f) as usize).copied().unwrap_or(0.0);
        q(0.9) - q(0.1) < BLANK_SPREAD
    })
}

/// A frame whose crop [`agree`] moved, measured again inside it (`scan`: its source, as [`zap`]
/// read it), its roll samples too. An error when the move reaches past its own crop (`own`) onto
/// blank film ([`widens_onto_blank`]): the frame then keeps its own crop.
pub fn remeasure(scan: &Rgb32f, z: &mut Zapped, own: Rect) -> Result<(), String> {
    if widens_onto_blank(scan, &z.settings, own) {
        return Err("the roll's crop reaches onto blank film".into());
    }
    let g = z.settings.crop.geometry;
    let px = framed(scan, &z.settings, g);
    let measured = lightcraft_pipeline::negative::measure(&px, 0.0)?;
    if let Some(r) = z.samples.as_deref_mut() {
        r.samples = lightcraft_pipeline::negative::sample(&px, 0.0, r.base)?;
    }
    z.settings.negative.measured = Some(measured);
    Ok(())
}

/// Film bases whose colour differs by more than this (in any channel, as a ratio, after their
/// brightness is set aside) are another film or another light: such a frame is measured on its own.
pub const ROLL_BASE_TOLERANCE: f64 = 1.15;

/// What [`pool`] did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Roll {
    /// The frames measured together.
    pub frames: Vec<PhotoId>,
    /// How many of them showed their film base on the rebate.
    pub bases: usize,
    /// Frames whose film base has another colour than the roll's ([`ROLL_BASE_TOLERANCE`]): left
    /// as Zap! measured them, on their own.
    pub apart: Vec<PhotoId>,
}

impl Roll {
    pub fn json(&self) -> Value {
        json!({
            "frames": self.frames.iter().map(|id| id.0).collect::<Vec<_>>(),
            "bases": self.bases,
            "apart": self.apart.iter().map(|id| id.0).collect::<Vec<_>>(),
        })
    }
}

/// A film base's colour without its brightness: each channel's log over the three's mean.
fn base_colour(b: [f64; 3]) -> [f64; 3] {
    let l = b.map(|v| v.max(1e-6).ln());
    let mean = (l[0] + l[1] + l[2]) / 3.0;
    l.map(|v| v - mean)
}

/// Measures the zapped negatives of one roll together, as a lab prints a roll: the levels and the
/// balance come from all their pictures at once, so one dark or one snowy frame doesn't set them.
/// Each frame's pictures are read relative to its own film base, so frames scanned brighter or
/// darker pool alike, and each frame gets the roll's measurement on its own base (a frame whose
/// rebate didn't show takes the median base). Each keeps its own crop and sliders. Frames without
/// samples (slides, or zapped without [`Options::roll`]) stay as they are; with fewer than two
/// frames to pool, nothing changes (`None`).
pub fn pool(zapped: &mut [(PhotoId, Zapped)]) -> Result<Option<Roll>, String> {
    fn rolled(z: &Zapped) -> Option<&RollSamples> {
        z.samples.as_deref().filter(|_| z.settings.negative.enabled)
    }
    let bases: Vec<[f64; 3]> = zapped.iter().filter_map(|(_, z)| rolled(z).and_then(|r| r.base)).collect();
    let median = median_base(&bases);
    let tolerance = ROLL_BASE_TOLERANCE.ln();
    let apart_from_roll = |b: Option<[f64; 3]>| match (median, b) {
        (Some(m), Some(b)) => {
            let (m, b) = (base_colour(m), base_colour(b));
            (0..3).any(|c| (b[c] - m[c]).abs() > tolerance)
        }
        _ => false,
    };
    let mut roll = Roll::default();
    let mut samples = ValueHistogram::new();
    for (id, z) in zapped.iter() {
        let Some(r) = rolled(z) else { continue };
        if apart_from_roll(r.base) {
            roll.apart.push(*id);
            continue;
        }
        // with bases to go by, only samples relative to one pool (a frame without takes the median)
        if median.is_none() || r.base.is_some() {
            samples.add(&r.samples);
        }
        roll.frames.push(*id);
        roll.bases += usize::from(r.base.is_some());
    }
    // (nothing to pool: in a mixed selection the frames counted may be only those without a base
    // to read relative to; each then keeps the measurement Zap! took on it)
    if roll.frames.len() < 2 || samples.count() == 0 {
        return Ok(None);
    }
    let pooled = lightcraft_pipeline::negative::measure_samples(&samples)?;
    let frames = u32::try_from(roll.frames.len()).unwrap_or(u32::MAX);
    for (id, z) in zapped.iter_mut() {
        if !roll.frames.contains(id) {
            continue;
        }
        let own = z.samples.as_deref().and_then(|r| r.base).or(median);
        let n = &mut z.settings.negative;
        n.measured = Some(match own {
            // the roll's levels on this frame's base
            Some(b) => {
                let on = |v: [f64; 3]| [v[0] * b[0], v[1] * b[1], v[2] * b[2]];
                NegativeMeasurement { base: on(pooled.base), black: on(pooled.black), white: on(pooled.white), balance: pooled.balance }
            }
            None => pooled,
        });
        if let Some(b) = own {
            // the picked base only counts in Advanced
            n.advanced = true;
            n.base = Some(b);
        }
        n.roll = Some(frames);
    }
    Ok(Some(roll))
}

/// The per-channel median of `bases` (in log, robust to one odd frame); `None` for none.
fn median_base(bases: &[[f64; 3]]) -> Option<[f64; 3]> {
    if bases.is_empty() {
        return None;
    }
    let mut out = [0.0; 3];
    for (c, o) in out.iter_mut().enumerate() {
        let mut v: Vec<f64> = bases.iter().map(|b| b[c].max(1e-6).ln()).filter(|v| v.is_finite()).collect();
        if v.is_empty() {
            return None;
        }
        v.sort_by(f64::total_cmp);
        // the middle one, or the mean of the middle two
        let (lo, hi) = ((v.len() - 1) / 2, v.len() / 2);
        let m = (v.get(lo).copied().unwrap_or(0.0) + v.get(hi).copied().unwrap_or(0.0)) / 2.0;
        *o = m.exp();
    }
    Some(out)
}

/// What a run off the session needs of one photo: read on the UI thread, run on a worker.
pub struct Input {
    pub id: PhotoId,
    /// The settings as they were read (a run that finds them changed is not applied).
    pub settings: Arc<DevelopSettings>,
    source: SourceRef,
    dims: (f64, f64),
}

impl Input {
    /// Photo `id`'s input, `None` when there is no such photo.
    pub fn of(s: &mut Session, id: PhotoId) -> Option<Self> {
        let photo = s.catalog.photo(id)?.clone();
        let settings = s.develop_of(id)?;
        let source = s.media.source_ref(&photo, SourceLevel::Preview);
        Some(Self { id, settings, source, dims: oriented_dims(&photo) })
    }

    /// Reads the photo's scan and runs Zap! on it.
    pub fn run(&self, o: Options) -> Result<Zapped, String> {
        self.run_with_info(o).0
    }

    /// [`Self::run`], with what the decoder said about the scan (`None` when it could not be read).
    pub fn run_with_info(&self, o: Options) -> (Result<Zapped, String>, Option<lightcraft_pipeline::SourceInfo>) {
        match self.source.load_source() {
            Ok(source) => (zap(&source.image, &self.settings, self.dims, o), source.info.clone()),
            Err(e) => (Err(e), None),
        }
    }

    /// Reads the photo's scan again and measures `z` inside its crop ([`remeasure`]).
    pub fn remeasure(&self, z: &mut Zapped, own: Rect) -> Result<(), String> {
        let source = self.source.load_source()?;
        remeasure(&source.image, z, own)
    }
}

/// Most photos of a selection Zap! reads and analyses at once: each one's decode and analysis
/// already use several cores, so a few side by side fill the rest (measured on 61 MP scans with 20
/// threads: 2.25× faster with 4 at once, little more beyond) without holding many decoded scans in
/// memory.
pub const MAX_WORKERS: usize = 4;

/// How many photos [`map_bounded`] runs at once on this machine: one per four cores, 1 to
/// [`MAX_WORKERS`].
pub fn workers() -> usize {
    (std::thread::available_parallelism().map_or(1, |n| n.get()) / 4).clamp(1, MAX_WORKERS)
}

/// `f` on each of `items`, on up to `workers` threads at once, the results in the order of `items`.
/// Before each item `stop` is asked; an item not started because it said so, or whose thread
/// failed, is `None`. `done` hears how many have finished after each one.
pub fn map_bounded<T: Sync, R: Send>(
    items: &[T],
    workers: usize,
    stop: &(dyn Fn() -> bool + Sync),
    done: &(dyn Fn(usize) + Sync),
    f: &(dyn Fn(&T) -> R + Sync),
) -> Vec<Option<R>> {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let next = AtomicUsize::new(0);
    let finished = AtomicUsize::new(0);
    let work = || {
        let mut out = Vec::new();
        loop {
            if stop() {
                break;
            }
            let i = next.fetch_add(1, Ordering::Relaxed);
            let Some(item) = items.get(i) else { break };
            out.push((i, f(item)));
            done(finished.fetch_add(1, Ordering::Relaxed) + 1);
        }
        out
    };
    let mut results: Vec<Option<R>> = std::iter::repeat_with(|| None).take(items.len()).collect();
    std::thread::scope(|s| {
        let handles: Vec<_> = (0..workers.clamp(1, items.len().max(1))).map(|_| s.spawn(work)).collect();
        for h in handles {
            // (a thread that panicked leaves its items `None`: they count as failed, the run goes on)
            let Ok(part) = h.join() else { continue };
            for (i, r) in part {
                if let Some(slot) = results.get_mut(i) {
                    *slot = Some(r);
                }
            }
        }
    });
    results
}

/// A run's outcome as `develop.negativeZap` reports it: how many photos changed, and each photo
/// left out with why.
pub fn summary(changed: usize, failed: &[(PhotoId, String)]) -> Value {
    json!({
        "changed": changed,
        "failed": failed.iter().map(|(id, e)| json!({"id": id.0, "error": e})).collect::<Vec<_>>(),
    })
}
