//! Convert Negative's Zap! on one photo, apart from the session: finds the picture on the scan,
//! straightens and crops to it, takes the film base from the rebate and measures the picture.
//! `develop.negativeZap` runs it on each target photo; the app runs it off the UI thread for a
//! selection and commits the results the same way, as one undo step.

use lightcraft_catalog::{Photo, PhotoId};
use lightcraft_develop::{DevelopSettings, NegativeMeasurement};
use lightcraft_geom::{CropGeometry, MIN_CROP_FRACTION, Rect};
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
    /// The picture is the whole film strip ([`negative_frame::STRIP_FORMAT`]).
    pub strip: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self { margin: Margin::Inside, ratio: None, sprockets: Sprockets::Remove, film: Film::Negative, strip: false }
    }
}

impl Options {
    /// From the command's `margin` and `format` parameters.
    pub fn from_params(p: &Value) -> Result<Self, String> {
        let margin = match p.get("margin") {
            None | Some(Value::Null) => Margin::Inside,
            Some(Value::String(m)) => Margin::parse(m).ok_or_else(|| format!("unknown margin `{m}` (inside, edge or border)"))?,
            Some(v) => return Err(format!("`margin` must be inside, edge or border, not {v}")),
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
        let strip = p.get("format").and_then(Value::as_str) == Some(negative_frame::STRIP_FORMAT);
        Ok(Self { margin, ratio, sprockets, film, strip })
    }
}

/// One photo's Zap!: its new settings and what was found.
#[derive(Clone, Debug)]
pub struct Zapped {
    pub settings: DevelopSettings,
    /// `{angle, rect, base, sprockets, sidesFound}`.
    pub report: Value,
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
    let found = if o.strip { negative_frame::detect_strip(&oriented) } else { negative_frame::detect(&oriented, o.ratio) };
    // measured on the picture itself (inside its edge, never the holes or the rebate)
    let picture = |f: &Frame| Frame { holes: [None, None], ..*f }.crop(Margin::Inside, ow, oh);
    if o.film == Film::Slide {
        return slide(before, found, dims, o, (ow, oh));
    }
    if own_crop(before) {
        let geometry = before.crop.geometry;
        let (measured, frame) = match &found {
            Ok(f) => (measure_in(&oriented, picture(f), 0.0)?, Some(f)),
            Err(_) => (measure_in(&oriented, geometry.rect, OWN_CROP_BORDER)?, None),
        };
        let settings = converted(before, measured, frame.and_then(|f| f.base));
        return Ok(Zapped { settings, report: report(&geometry, frame, true) });
    }
    let frame = found?;
    let geometry = crop_of(&frame, dims, o, (ow, oh))?;
    let measured = measure_in(&oriented, picture(&frame), 0.0)?;
    let mut settings = converted(before, measured, frame.base);
    settings.crop.geometry = geometry;
    settings.negative.zap_crop = Some(geometry);
    Ok(Zapped { settings, report: report(&geometry, Some(&frame), false) })
}

/// The crop Zap! sets for `frame` on a scan of oriented size `dims` (analysed at `analysed`).
fn crop_of(frame: &Frame, dims: (f64, f64), o: Options, analysed: (f64, f64)) -> Result<CropGeometry, String> {
    let (w, h) = dims;
    let cut = match o.sprockets {
        // the whole strip: to the film's edges, the holes in it
        _ if o.strip => Frame { picture: frame.film, holes: [None, None], ..*frame },
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
        return Ok(Zapped { settings: before.clone(), report: report(&geometry, found.as_ref().ok(), true) });
    }
    let frame = found?;
    let geometry = crop_of(&frame, dims, o, analysed)?;
    let mut settings = before.clone();
    settings.crop.geometry = geometry;
    settings.negative.zap_crop = Some(geometry);
    Ok(Zapped { settings, report: report(&geometry, Some(&frame), false) })
}

/// `before` converted with `measured` and, when the rebate showed, the film base.
fn converted(before: &DevelopSettings, measured: NegativeMeasurement, base: Option<[f64; 3]>) -> DevelopSettings {
    let mut settings = before.clone();
    let n = &mut settings.negative;
    n.enabled = true;
    n.measured = Some(measured);
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
    let (w, h) = (o.width as f64, o.height as f64);
    let at = |v: f64, n: f64| if v.is_finite() { (v.clamp(0.0, 1.0) * n).round() as usize } else { 0 };
    let (x0, y0, x1, y1) = (at(r.x0, w), at(r.y0, h), at(r.x1, w), at(r.y1, h));
    let framed = if x1 > x0 && y1 > y0 { o.crop(x0, y0, x1 - x0, y1 - y0) } else { o.clone() };
    lightcraft_pipeline::negative::measure(&framed, border)
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
