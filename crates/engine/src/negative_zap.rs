//! Convert Negative's Zap! on one photo, apart from the session: finds the picture on the scan,
//! straightens and crops to it, takes the film base from the rebate and measures the picture.
//! `develop.negativeZap` runs it on each target photo; the app runs it off the UI thread for a
//! selection and commits the results the same way, as one undo step.

use lightcraft_catalog::{Photo, PhotoId};
use lightcraft_develop::{DevelopSettings, NegativeMeasurement};
use lightcraft_geom::{CropGeometry, MIN_CROP_FRACTION, Rect};
use lightcraft_pipeline::negative_frame::{self, Margin};
use lightcraft_raster::Rgb32f;
use serde_json::{Value, json};
use std::sync::Arc;

use crate::Session;
use crate::media::{SourceLevel, SourceRef};

/// The label of Zap!'s undo step and History entries.
pub const LABEL: &str = "Zap";

/// Where the crop sits and the film format, the same for every photo of a run.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub margin: Margin,
    /// The picture's long / short side, `None` for no format assumed.
    pub ratio: Option<f64>,
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
        Ok(Self { margin, ratio })
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

/// Zap! on `scan` (the photo's source, before orientation) of a photo with settings `before` and
/// oriented size `dims`.
pub fn zap(scan: &Rgb32f, before: &DevelopSettings, dims: (f64, f64), o: Options) -> Result<Zapped, String> {
    let oriented = scan.oriented(before.orientation);
    let frame = negative_frame::detect(&oriented, o.ratio)?;
    let (w, h) = dims;
    let rect = frame.crop(o.margin, oriented.width as f64, oriented.height as f64);
    let geometry = CropGeometry { rect, angle: frame.angle }.constrained(w, h);
    let min = MIN_CROP_FRACTION * w.min(h);
    if geometry.rect.width() * w < min || geometry.rect.height() * h < min {
        return Err("no film frame found".into());
    }
    // measured on the picture itself: nothing of the rebate left to leave out
    let measured = measure_in(&oriented, geometry.rect, 0.0)?;
    let mut settings = before.clone();
    settings.crop.geometry = geometry;
    let n = &mut settings.negative;
    n.enabled = true;
    n.measured = Some(measured);
    if let Some(b) = frame.base {
        // the picked base only counts in Advanced
        n.advanced = true;
        n.base = Some(b);
    }
    let report = json!({
        "angle": geometry.angle,
        "rect": [geometry.rect.x0, geometry.rect.y0, geometry.rect.x1, geometry.rect.y1],
        "base": frame.base,
        "sprockets": frame.sprockets,
        "sidesFound": frame.sides_found,
    });
    Ok(Zapped { settings, report })
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
