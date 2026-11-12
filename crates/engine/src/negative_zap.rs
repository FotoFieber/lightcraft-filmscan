//! Convert Negative's Zap! on one photo, apart from the session: finds the picture on the scan,
//! straightens and crops to it, takes the film base from the rebate and measures the picture.
//! `develop.negativeZap` runs it on each target photo; the app runs it off the UI thread for a
//! selection and commits the results the same way, as one undo step.

use lightcraft_catalog::{Photo, PhotoId};
use lightcraft_develop::{DevelopSettings, NegativeMeasurement};
use lightcraft_geom::{CropGeometry, MIN_CROP_FRACTION, Rect};
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
    /// A slide (a positive): only cropped, never converted.
    Slide,
}

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
            Some(Value::String(v)) if v == "slide" => Film::Slide,
            Some(v) => return Err(format!("`film` must be negative or slide, not {v}")),
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
        Film::Negative => negative_frame::detect(&oriented, o.ratio),
    };
    // measured on the picture itself (inside its edge, never the holes or the rebate)
    let picture = |f: &Frame| Frame { holes: [None, None], ..*f }.crop(Margin::Inside, ow, oh);
    if o.film == Film::Slide {
        return slide(before, found, dims, o, (ow, oh));
    }
    // a roll's frames keep what their measurement read, to be measured together
    let samples = |r: Rect, border: f64, base: Option<[f64; 3]>| -> Result<Option<Box<RollSamples>>, String> {
        if !o.roll {
            return Ok(None);
        }
        Ok(Some(Box::new(RollSamples { samples: sample_in(&oriented, r, border, base)?, base })))
    };
    if own_crop(before) {
        let geometry = before.crop.geometry;
        let (r, border, frame) = match &found {
            Ok(f) => (picture(f), 0.0, Some(f)),
            Err(_) => (geometry.rect, OWN_CROP_BORDER, None),
        };
        let settings = converted(before, measure_in(&oriented, r, border)?, frame.and_then(|f| f.base));
        let samples = samples(r, border, frame.and_then(|f| f.base))?;
        return Ok(Zapped { settings, report: report(&geometry, frame, true), samples });
    }
    let frame = found?;
    let geometry = crop_of(&frame, dims, o, (ow, oh))?;
    let measured = measure_in(&oriented, picture(&frame), 0.0)?;
    let mut settings = converted(before, measured, frame.base);
    settings.crop.geometry = geometry;
    settings.negative.zap_crop = Some(geometry);
    Ok(Zapped { settings, report: report(&geometry, Some(&frame), false), samples: samples(picture(&frame), 0.0, frame.base)? })
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

/// `before` converted with `measured` and, when the rebate showed, the film base.
fn converted(before: &DevelopSettings, measured: NegativeMeasurement, base: Option<[f64; 3]>) -> DevelopSettings {
    let mut settings = before.clone();
    let n = &mut settings.negative;
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

/// Measures the oriented scan `o` inside `r` (normalized), `border` in from each edge.
pub fn measure_in(o: &Rgb32f, r: Rect, border: f64) -> Result<NegativeMeasurement, String> {
    lightcraft_pipeline::negative::measure(&framed(o, r), border)
}

/// The samples [`measure_in`] reads, at most a roll frame's share, relative to `base`.
fn sample_in(o: &Rgb32f, r: Rect, border: f64, base: Option<[f64; 3]>) -> Result<ValueHistogram, String> {
    lightcraft_pipeline::negative::sample(&framed(o, r), border, base)
}

/// The oriented scan `o` inside `r` (normalized; all of it when `r` holds no pixel).
fn framed(o: &Rgb32f, r: Rect) -> Rgb32f {
    let (w, h) = (o.width as f64, o.height as f64);
    let at = |v: f64, n: f64| if v.is_finite() { (v.clamp(0.0, 1.0) * n).round() as usize } else { 0 };
    let (x0, y0, x1, y1) = (at(r.x0, w), at(r.y0, h), at(r.x1, w), at(r.y1, h));
    if x1 > x0 && y1 > y0 { o.crop(x0, y0, x1 - x0, y1 - y0) } else { o.clone() }
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
    if roll.frames.len() < 2 {
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
        let source = self.source.load_source()?;
        zap(&source.image, &self.settings, self.dims, o)
    }
}

/// A run's outcome as `develop.negativeZap` reports it: how many photos changed, and each photo
/// left out with why.
pub fn summary(changed: usize, failed: &[(PhotoId, String)]) -> Value {
    json!({
        "changed": changed,
        "failed": failed.iter().map(|(id, e)| json!({"id": id.0, "error": e})).collect::<Vec<_>>(),
    })
}
