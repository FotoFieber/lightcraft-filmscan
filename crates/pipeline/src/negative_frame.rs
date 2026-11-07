//! Convert Negative's "Zap!": finds the picture on a scan of a film negative, the straighten angle
//! and the crop that keep it and leave out the scanner's holder, the film's rebate (the unexposed
//! border, with its sprocket holes, whole or cut) and the film base colour that rebate shows.
//!
//! The scan is analysed small (about [`ANALYSIS_EDGE`] pixels on the long edge), in linear sRGB:
//!
//! 1. **Film and holder.** The holder (or the scanner bed around a strip) is far darker than any
//!    film; what is brighter than a fraction of the scan's highlights is film or light.
//! 2. **Angle.** The holder's window has straight edges: on each side, the first film pixel along
//!    every line across the central part of the frame, and a robust line through them. Sides that
//!    disagree with the others or have too few points are left out.
//! 3. **Film base.** In the straightened scan, the bands just inside the film's edges are rebate
//!    or picture; their median colour, light through the sprocket holes left out (far bluer or
//!    brighter than an orange base), is the first guess of the base.
//! 4. **The picture.** For each side, a profile from the film's edge inward: the median distance
//!    (log colour) to the base along the side. Rebate is flat and near the base; the picture starts
//!    where it leaves the base for good. Where the picture is as thin as the base (a scene shadow
//!    beside the frame line), the straight edge the camera's gate leaves (a step across the whole
//!    side) is used instead.
//! 5. **Choice.** Each side offers its colour step and its strongest straight edges; the choice of
//!    one per side (or none) weighs how sure each is, how alike the rebates are (a holder masks the
//!    film evenly, though never exactly alike) and, when a format is chosen, the picture's aspect.
//!    A side left out is placed by the format, else as deep as the side across from it.
//! 6. **Base.** The median of the rebate (film outside the picture, light left out).
//! 7. **Sprocket holes.** Along each long side of 35 mm film, the lines that light crosses in a run
//!    of holes. The crop's long sides go by them rather than by the picture: just outside holes
//!    that show whole (they stay in the picture, film between them and the film's edge), just
//!    inside holes the holder cuts (they go; the rebate with the edge print stays). Holes are far
//!    surer to find than a picture's edge.
//!
//! Nothing here panics on any input: a scan with no film frame to find is an error.

use lightcraft_color::{Mat3, REC2020, SRGB};
use lightcraft_geom::{Affine, Point, Rect};
use lightcraft_raster::resample::{Filter, resize};
use lightcraft_raster::{Plane, Rgb32f};

/// Long edge of the image the analysis runs on.
pub const ANALYSIS_EDGE: usize = 1000;
/// Most the scan is turned (degrees): a holder sits nearly straight in the scanner.
pub const MAX_ANGLE: f64 = 10.0;
/// Film is brighter than this fraction of the scan's highlights; the holder is darker.
const FILM_FRACTION: f32 = 0.08;
/// Width of the bands the first base guess reads, as a fraction of the shorter side.
const BAND: f64 = 0.03;
/// The part of each side a profile reads (the middle, away from the corners).
const CENTRAL: f64 = 0.6;
/// How far in from the film's edge a picture edge is looked for (fraction of the extent).
const SEARCH: f64 = 0.3;
/// Distance to the base (sum of three log ratios) above the rebate that starts the picture.
const STEP: f32 = 0.5;
/// Rebate is at most this far from the base; more, and the picture reaches the film's edge.
const REBATE_MAX: f32 = 0.6;
/// Smallest log step a pixel pair counts for a gate edge.
const EDGE_LOG_STEP: f32 = 0.02;
/// The fewest whole holes a line along a row of sprocket holes crosses.
const HOLE_RUNS_MIN: usize = 4;
/// How far the holes' pitch and length may vary along a row (fraction of the median), and the
/// share of the pitch a hole takes (35 mm: about 2 mm of 4.75 mm, more across a hole's middle).
const HOLE_PITCH_TOL: f32 = 0.2;
const HOLE_LEN_TOL: f32 = 0.35;
const HOLE_DUTY_MIN: f32 = 0.2;
const HOLE_DUTY_MAX: f32 = 0.8;
/// Lines from the film's edge that may be its blurred border before whole holes start.
const HOLE_EDGE_SLACK: usize = 1;
/// Whole holes are at least this deep across the film for their length along it (35 mm holes are
/// about as deep as long; a hole the holder half covers is a low slit).
const HOLE_SQUARE_MIN: f32 = 0.6;
/// The crop's distance from the sprocket holes (fraction of the film's extent across).
const HOLE_MARGIN: f64 = 0.015;

/// How the crop sits on the picture's edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Margin {
    /// A little inside, so no rebate shows (1 % of the shorter side).
    Inside,
    /// On the edge.
    Edge,
    /// A little rebate around the picture (2 % of the shorter side, within the film).
    Border,
}

impl Margin {
    pub fn parse(s: &str) -> Option<Margin> {
        match s {
            "inside" => Some(Margin::Inside),
            "edge" => Some(Margin::Edge),
            "border" => Some(Margin::Border),
            _ => None,
        }
    }

    fn offset(self) -> f64 {
        match self {
            Margin::Inside => -0.01,
            Margin::Edge => 0.0,
            Margin::Border => 0.02,
        }
    }
}

/// Film formats a frame can be placed by: id, label, and the picture's length along the film over
/// its height across it. "auto" assumes none.
pub const FORMATS: [(&str, &str, Option<f64>); 11] = [
    ("auto", "Auto", None),
    ("35mm", "35 mm (3:2)", Some(36.0 / 24.0)),
    ("half", "Half Frame", Some(18.0 / 24.0)),
    ("xpan", "XPan (65×24)", Some(65.0 / 24.0)),
    ("645", "6×4.5", Some(41.5 / 56.0)),
    ("66", "6×6", Some(1.0)),
    ("67", "6×7", Some(69.5 / 56.0)),
    ("68", "6×8", Some(77.0 / 56.0)),
    ("69", "6×9", Some(84.0 / 56.0)),
    ("612", "6×12", Some(118.0 / 56.0)),
    ("617", "6×17", Some(168.0 / 56.0)),
];

/// The along-the-film aspect of format `id` (`None` for auto); `Err` for an unknown id.
pub fn format_ratio(id: &str) -> Result<Option<f64>, String> {
    FORMATS.iter().find(|f| f.0 == id).map(|f| f.2).ok_or_else(|| format!("unknown film format `{id}`"))
}

/// What [`detect`] found.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    /// The straighten angle (degrees, as [`lightcraft_geom::CropGeometry::angle`]).
    pub angle: f64,
    /// The picture, normalized, in the frame turned by `angle` (as the crop's rect).
    pub picture: Rect,
    /// The film inside the holder, the same way.
    pub film: Rect,
    /// The film base (linear sRGB, as `develop.negativePick` stores it), when rebate shows.
    pub base: Option<[f64; 3]>,
    /// Sprocket holes were seen (35 mm film).
    pub sprockets: bool,
    /// How many of the four sides the scan showed (the rest are placed).
    pub sides_found: u8,
    /// The film runs along x (across the scan): its long sides, with the sprocket holes, are the
    /// top and the bottom; else they are the left and the right.
    pub along_x: bool,
    /// The sprocket holes along each long side: `[top, bottom]` when [`Frame::along_x`], else
    /// `[left, right]`.
    pub holes: [Option<Holes>; 2],
}

/// A row of sprocket holes along one long side of the film, across the film normalized as
/// [`Frame::picture`] (y for a film along x).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Holes {
    /// The holes' edge toward the film's edge.
    pub outer: f64,
    /// The holes' edge toward the picture.
    pub inner: f64,
    /// The whole holes show: film is seen between them and the film's edge. Else the holder
    /// covers part of them.
    pub whole: bool,
}

impl Frame {
    /// The crop for `margin`: the picture moved out or in by the margin, kept on the film. A long
    /// side with sprocket holes is cut by them instead: just outside whole holes (they show), just
    /// inside holes the holder cuts (they don't; the rebate with the edge print does).
    pub fn crop(&self, margin: Margin, w: f64, h: f64) -> Rect {
        let (w, h) = (w.max(1.0), h.max(1.0));
        let m = margin.offset() * w.min(h);
        let (mx, my) = (m / w, m / h);
        let p = self.picture;
        let r = Rect::new(p.x0 - mx, p.y0 - my, p.x1 + mx, p.y1 + my);
        let f = self.film;
        let r = if margin == Margin::Border { Rect::new(r.x0.max(f.x0), r.y0.max(f.y0), r.x1.min(f.x1), r.y1.min(f.y1)) } else { r };
        let r = self.by_holes(r);
        Rect::new(r.x0.clamp(0.0, 1.0), r.y0.clamp(0.0, 1.0), r.x1.clamp(0.0, 1.0), r.y1.clamp(0.0, 1.0))
    }

    /// `r` with its long sides moved to the sprocket holes, where they show.
    fn by_holes(&self, r: Rect) -> Rect {
        let f = self.film;
        // the long sides' positions across the film, the film's edges there and its extent across
        let (mut near, mut far, f0, f1) = if self.along_x { (r.y0, r.y1, f.y0, f.y1) } else { (r.x0, r.x1, f.x0, f.x1) };
        let gap = (f1 - f0).max(0.0) * HOLE_MARGIN;
        if let Some(hl) = self.holes[0] {
            near = if hl.whole { (hl.outer - gap).max(f0) } else { hl.inner + gap };
        }
        if let Some(hl) = self.holes[1] {
            far = if hl.whole { (hl.outer + gap).min(f1) } else { hl.inner - gap };
        }
        if far <= near {
            return r;
        }
        if self.along_x { Rect::new(r.x0, near, r.x1, far) } else { Rect::new(near, r.y0, far, r.y1) }
    }
}

/// Finds the picture on `src` (linear Rec.2020, oriented as shown). `ratio` (the picture's length
/// along the film over its height across it, see [`FORMATS`]) places a side the scan doesn't show
/// and weighs the choice of sides; without it, sides are placed by the rebate across from them.
pub fn detect(src: &Rgb32f, ratio: Option<f64>) -> Result<Frame, String> {
    if src.width < 16 || src.height < 16 || src.data.len() != src.width.saturating_mul(src.height) {
        return Err("the scan is too small to find a film frame on".into());
    }
    let small = analysis_image(src);
    let peak = quantile(small.data.iter().map(|p| luma(*p)).collect(), 0.99).unwrap_or(0.0);
    if peak.is_nan() || peak <= 1e-6 {
        return Err("no film frame found: the scan is black".into());
    }
    let threshold = peak * FILM_FRACTION;
    let angle = level_angle(&small, threshold).unwrap_or(0.0);
    let turned = turn(&small, angle);
    let (w, h) = (turned.width, turned.height);
    let film = film_box(&turned, threshold).ok_or("no film frame found: no film inside the holder")?;
    let guess = band_base(&turned, film, threshold).ok_or("no film frame found: no film base")?;
    let light = Light::of(&guess);
    let sprockets = sprocket_share(&turned, film, threshold, &light) > 0.02;
    let holes = if sprockets { hole_rows(&turned, film, threshold, &light) } else { [None, None] };
    let dist = distance(&turned, &guess, threshold, &light);
    let lum = turned.map(|p| (luma(p).max(0.0) + 1e-4).ln());
    let scan = Scan { dist: &dist, lum: &lum, img: &turned, light: &light, threshold, film };
    let cands = Side::ALL.map(|side| scan.candidates(side));
    // the film runs along the sprocket holes, else along the film's longer side
    let along_x = film.width() >= film.height();
    let ratio = ratio.filter(|r| r.is_finite() && *r > 0.0).map(|r| if along_x { r } else { 1.0 / r });
    let (picture, sides_found) = choose(&cands, film, ratio).ok_or("no film frame found")?;
    if sides_found < MIN_SIDES {
        return Err("no film frame found: too little of the frame's edges shows".into());
    }
    plausible(&picture, w, h)?;
    let base = rebate_base(&turned, film, picture, threshold, &light);
    let norm = |r: Px| Rect::new(r.x0 as f64 / w as f64, r.y0 as f64 / h as f64, r.x1 as f64 / w as f64, r.y1 as f64 / h as f64);
    let across = if along_x { h } else { w } as f64;
    let holes = holes.map(|o| o.map(|b| Holes { outer: b.outer as f64 / across, inner: b.inner as f64 / across, whole: b.whole }));
    Ok(Frame { angle, picture: norm(picture), film: norm(film), base, sprockets: holes.iter().any(Option::is_some), sides_found, along_x, holes })
}

// ---- the analysis image

/// `src` at the analysis size, in linear sRGB, with anything not finite as black.
fn analysis_image(src: &Rgb32f) -> Rgb32f {
    let long = src.width.max(src.height);
    let s = if long > ANALYSIS_EDGE { ANALYSIS_EDGE as f64 / long as f64 } else { 1.0 };
    let (w, h) = (((src.width as f64 * s).round() as usize).max(16), ((src.height as f64 * s).round() as usize).max(16));
    let small = resize(src, w, h, Filter::Box);
    let m: Mat3 = REC2020.to_space(&SRGB);
    let m = m.to_f32();
    small.map(|v| {
        let p = m.map(|r| r[0] * v[0] + r[1] * v[1] + r[2] * v[2]);
        p.map(|c| if c.is_finite() { c.max(0.0) } else { 0.0 })
    })
}

fn luma(p: [f32; 3]) -> f32 {
    0.2126 * p[0] + 0.7152 * p[1] + 0.0722 * p[2]
}

/// The `q` quantile of `v` (none when empty).
fn quantile(mut v: Vec<f32>, q: f64) -> Option<f32> {
    v.retain(|x| x.is_finite());
    if v.is_empty() {
        return None;
    }
    let k = ((v.len() - 1) as f64 * q.clamp(0.0, 1.0)).round() as usize;
    let k = k.min(v.len() - 1);
    let (_, x, _) = v.select_nth_unstable_by(k, f32::total_cmp);
    Some(*x)
}

fn median(v: Vec<f32>) -> Option<f32> {
    quantile(v, 0.5)
}

/// `img` turned for a crop angle of `angle` degrees: pixel `p` of the result is the scan at the
/// point the render takes for it ([`lightcraft_geom::CropGeometry::output_to_source`]); outside
/// the scan it is black.
fn turn(img: &Rgb32f, angle: f64) -> Rgb32f {
    if angle == 0.0 {
        return img.clone();
    }
    let (w, h) = (img.width as f64, img.height as f64);
    let back = Affine::rotate_about(-angle.to_radians(), Point::new(w / 2.0, h / 2.0));
    Rgb32f::from_fn(img.width, img.height, |x, y| {
        let q = back.apply(Point::new(x as f64 + 0.5, y as f64 + 0.5));
        if q.x < 0.0 || q.y < 0.0 || q.x > w || q.y > h { [0.0; 3] } else { img.sample_bilinear(q.x as f32, q.y as f32) }
    })
}

// ---- sides and boxes

/// A box in analysis pixels (end exclusive).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Px {
    x0: usize,
    y0: usize,
    x1: usize,
    y1: usize,
}

impl Px {
    fn width(&self) -> usize {
        self.x1.saturating_sub(self.x0)
    }
    fn height(&self) -> usize {
        self.y1.saturating_sub(self.y0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Side {
    Top,
    Bottom,
    Left,
    Right,
}

impl Side {
    const ALL: [Side; 4] = [Side::Top, Side::Bottom, Side::Left, Side::Right];

    fn horizontal(self) -> bool {
        matches!(self, Side::Top | Side::Bottom)
    }
}

/// The pixel `i` in from `side` of a `w × h` image, at `j` along the side.
fn at(side: Side, i: usize, j: usize, w: usize, h: usize) -> Option<(usize, usize)> {
    let p = match side {
        Side::Top => (j, i),
        Side::Bottom => (j, h.checked_sub(1)?.checked_sub(i)?),
        Side::Left => (i, j),
        Side::Right => (w.checked_sub(1)?.checked_sub(i)?, j),
    };
    (p.0 < w && p.1 < h).then_some(p)
}

fn px<T: Copy>(img: &lightcraft_raster::Image<T>, x: usize, y: usize) -> Option<T> {
    img.data.get(y.checked_mul(img.width)?.checked_add(x)?).copied()
}

/// The central part of `0..n`.
fn central(n: usize) -> std::ops::Range<usize> {
    let m = ((n as f64) * (1.0 - CENTRAL) / 2.0) as usize;
    m..n.saturating_sub(m).max(m)
}

// ---- the angle

/// The angle that levels the holder's window, from the sides that show it.
fn level_angle(img: &Rgb32f, threshold: f32) -> Option<f64> {
    let mut found: Vec<(f64, usize)> = Side::ALL.iter().filter_map(|s| side_angle(img, threshold, *s)).collect();
    if found.is_empty() {
        return None;
    }
    // the sides that agree with their weighted median
    found.sort_by(|a, b| a.0.total_cmp(&b.0));
    let total: usize = found.iter().map(|f| f.1).sum();
    let mut acc = 0;
    let mid = found.iter().find(|f| {
        acc += f.1;
        acc * 2 >= total
    })?;
    let mid = mid.0;
    let near: Vec<&(f64, usize)> = found.iter().filter(|f| (f.0 - mid).abs() < 0.5).collect();
    let n: usize = near.iter().map(|f| f.1).sum();
    if n == 0 {
        return None;
    }
    let a = near.iter().map(|f| f.0 * f.1 as f64).sum::<f64>() / n as f64;
    (a.is_finite() && a.abs() <= MAX_ANGLE && a.abs() > 0.02).then_some(a)
}

/// The angle (degrees) and the number of points of the holder's edge on `side`: for each line
/// across the central part, the first film pixel after holder.
fn side_angle(img: &Rgb32f, threshold: f32, side: Side) -> Option<(f64, usize)> {
    let (w, h) = (img.width, img.height);
    let (along, across) = if side.horizontal() { (w, h) } else { (h, w) };
    let reach = (across as f64 * SEARCH) as usize;
    let film = |i: usize, j: usize| at(side, i, j, w, h).and_then(|(x, y)| px(img, x, y)).is_some_and(|p| luma(p) > threshold);
    let pts: Vec<(f64, f64)> = central(along)
        .filter_map(|j| {
            // holder at the scan's edge, then film
            if film(0, j) {
                return None;
            }
            (1..reach).find(|i| film(*i, j)).map(|i| (j as f64, i as f64))
        })
        .collect();
    if pts.len() < along / 4 {
        return None;
    }
    let slope = theil_sen(&pts)?;
    // residuals: a straight edge, not the ragged end of a strip
    let mid_j = pts.iter().map(|p| p.0).sum::<f64>() / pts.len() as f64;
    let off = median(pts.iter().map(|p| (p.1 - slope * (p.0 - mid_j)) as f32).collect())? as f64;
    let res: Vec<f32> = pts.iter().map(|p| (p.1 - off - slope * (p.0 - mid_j)).abs() as f32).collect();
    let inliers = res.iter().filter(|r| **r < 2.0).count();
    if inliers * 3 < pts.len() * 2 {
        return None;
    }
    // the distance in from the side grows along it: which way that turns the scan depends on the side
    let deg = slope.atan().to_degrees();
    let a = match side {
        Side::Top | Side::Right => -deg,
        Side::Bottom | Side::Left => deg,
    };
    Some((a, inliers))
}

/// The median slope between points half the run apart (Theil–Sen on pairs).
fn theil_sen(pts: &[(f64, f64)]) -> Option<f64> {
    let half = pts.len() / 2;
    if half == 0 {
        return None;
    }
    let slopes: Vec<f32> = pts
        .iter()
        .zip(pts.iter().skip(half))
        .filter_map(|(a, b)| {
            let dx = b.0 - a.0;
            (dx.abs() > 1e-9).then(|| ((b.1 - a.1) / dx) as f32)
        })
        .collect();
    median(slopes).map(f64::from)
}

// ---- film, base and light

/// The film inside the holder: the rows and columns more than half film.
fn film_box(img: &Rgb32f, threshold: f32) -> Option<Px> {
    let (w, h) = (img.width, img.height);
    let is_film = |p: [f32; 3]| luma(p) > threshold;
    let rows: Vec<bool> = (0..h).map(|y| img.row(y).iter().filter(|p| is_film(**p)).count() * 2 > w).collect();
    let cols: Vec<bool> = (0..w).map(|x| (0..h).filter(|y| px(img, x, *y).is_some_and(is_film)).count() * 2 > h).collect();
    let span = |v: &[bool]| Some((v.iter().position(|b| *b)?, v.len() - v.iter().rev().position(|b| *b)?));
    let (x0, x1) = span(&cols)?;
    let (y0, y1) = span(&rows)?;
    let b = Px { x0, y0, x1, y1 };
    (b.width() * 4 >= w && b.height() * 4 >= h).then_some(b)
}

/// What light through the film's holes (sprockets, a torn edge) looks like next to `base`.
struct Light {
    blue: f32,
    lum: f32,
}

impl Light {
    fn of(base: &[f32; 3]) -> Light {
        Light { blue: base[2].max(1e-4) * 2.5, lum: luma(*base).max(1e-4) * 1.8 }
    }
    fn is(&self, p: [f32; 3]) -> bool {
        p[2] > self.blue && p[2] > p[0] * 0.5 || luma(p) > self.lum
    }
}

/// The median colour of the film in the bands just inside the film's edges, light left out.
fn band_base(img: &Rgb32f, film: Px, threshold: f32) -> Option<[f32; 3]> {
    let band = ((film.width().min(film.height()) as f64 * BAND) as usize).max(2);
    let mut px_in: Vec<[f32; 3]> = Vec::new();
    for y in film.y0..film.y1 {
        for x in film.x0..film.x1 {
            let edge = y < film.y0 + band || y + band >= film.y1 || x < film.x0 + band || x + band >= film.x1;
            if let Some(p) = px(img, x, y).filter(|p| edge && luma(*p) > threshold) {
                px_in.push(p);
            }
        }
    }
    let first = channel_median(&px_in)?;
    let light = Light::of(&first);
    let kept: Vec<[f32; 3]> = px_in.into_iter().filter(|p| !light.is(*p)).collect();
    channel_median(&kept)
}

fn channel_median(px: &[[f32; 3]]) -> Option<[f32; 3]> {
    Some([median(px.iter().map(|p| p[0]).collect())?, median(px.iter().map(|p| p[1]).collect())?, median(px.iter().map(|p| p[2]).collect())?])
}

/// The share of light in the bands along the film's long sides (sprocket holes).
fn sprocket_share(img: &Rgb32f, film: Px, threshold: f32, light: &Light) -> f64 {
    let band = ((film.width().min(film.height()) as f64 * BAND * 2.0) as usize).max(2);
    let long_horizontal = film.width() >= film.height();
    let (mut n, mut lit) = (0usize, 0usize);
    for y in film.y0..film.y1 {
        for x in film.x0..film.x1 {
            let edge = if long_horizontal { y < film.y0 + band || y + band >= film.y1 } else { x < film.x0 + band || x + band >= film.x1 };
            if let Some(p) = px(img, x, y).filter(|p| edge && luma(*p) > threshold) {
                n += 1;
                lit += usize::from(light.is(p));
            }
        }
    }
    if n == 0 { 0.0 } else { lit as f64 / n as f64 }
}

/// A row of sprocket holes in analysis pixels across the film (from the image's edge).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct HoleRow {
    outer: usize,
    inner: usize,
    whole: bool,
}

/// The sprocket holes along the film's two long sides (`[top, bottom]` or `[left, right]`). On
/// each, the lines along the film (rows for a film along x) over its central length that light
/// crosses in a run of holes: a good share of the line, in several separate runs (a gap between
/// the holder and the film's edge is one long run). Whole holes have film between them and the
/// film's edge; holes that start at the film's edge are cut by the holder.
fn hole_rows(img: &Rgb32f, film: Px, threshold: f32, light: &Light) -> [Option<HoleRow>; 2] {
    let along_x = film.width() >= film.height();
    let (across0, across1, len0, len1) = if along_x { (film.y0, film.y1, film.x0, film.x1) } else { (film.x0, film.x1, film.y0, film.y1) };
    let extent = across1.saturating_sub(across0);
    let along: Vec<usize> = {
        let c = central(len1.saturating_sub(len0));
        (len0..len1).skip(c.start).take(c.len()).collect()
    };
    let at = |a: usize, l: usize| if along_x { px(img, l, a) } else { px(img, a, l) };
    // a line across the film at `a` crosses a row of holes: the holes' length along it
    let holes_at = |a: usize| {
        let lit: Vec<bool> = along.iter().map(|l| at(a, *l).is_some_and(|p| luma(p) > threshold && light.is(p))).collect();
        regular_holes(&lit)
    };
    let holey = |a: usize| holes_at(a).is_some();
    let reach = ((extent as f64) * SEARCH) as usize;
    let side = |inward: &dyn Fn(usize) -> Option<usize>| -> Option<HoleRow> {
        // the first holey line from the film's edge, then as far in as the holes go
        let first = (0..reach).find(|i| inward(*i).is_some_and(holey))?;
        let last = (first..reach).take_while(|i| inward(*i).is_some_and(holey)).last()?;
        let (outer, inner) = (inward(first)?, inward(last + 1)?);
        // whole: film between them and the film's edge (a pixel or two there are its blurred
        // border), and about as deep across the film as they are long along it (a hole the
        // holder covers shows as a low slit)
        let length = holes_at(inward((first + last) / 2)?)?;
        let deep = (last + 1 - first) as f32 >= length * HOLE_SQUARE_MIN;
        Some(HoleRow { outer, inner, whole: first > HOLE_EDGE_SLACK && deep })
    };
    let near = side(&|i| (across0 + i < across1).then_some(across0 + i));
    let far = side(&|i| across1.checked_sub(1 + i).filter(|a| *a >= across0)).map(|r| HoleRow { outer: r.outer + 1, inner: r.inner + 1, ..r });
    [near, far]
}

/// `lit` (light or not, pixel by pixel along a line) crosses a row of sprocket holes: several holes
/// of one length, one pitch apart, each a fair part of the pitch. Light along a holder's ragged
/// edge or through a torn film is not that regular. The holes' length along the line.
fn regular_holes(lit: &[bool]) -> Option<f32> {
    // the runs of light, those cut by the line's ends left out
    let mut runs: Vec<(usize, usize)> = Vec::new();
    let mut start = None;
    for (i, b) in lit.iter().enumerate() {
        match (*b, start) {
            (true, None) => start = Some(i),
            (false, Some(s0)) => {
                if s0 > 0 {
                    runs.push((s0, i - s0));
                }
                start = None;
            }
            _ => {}
        }
    }
    if runs.len() < HOLE_RUNS_MIN {
        return None;
    }
    let pitches: Vec<f32> = runs.windows(2).map(|w| (w[1].0 - w[0].0) as f32).collect();
    let lens: Vec<f32> = runs.iter().map(|r| r.1 as f32).collect();
    let (pitch, len) = (median(pitches.clone())?, median(lens.clone())?);
    if pitch < 2.0 || len < 1.0 {
        return None;
    }
    let near = |v: &[f32], m: f32, tol: f32| v.iter().all(|x| (x - m).abs() <= m * tol + 1.0);
    let duty = len / pitch;
    (near(&pitches, pitch, HOLE_PITCH_TOL) && near(&lens, len, HOLE_LEN_TOL) && (HOLE_DUTY_MIN..=HOLE_DUTY_MAX).contains(&duty)).then_some(len)
}

/// Each pixel's distance to the base (sum of the three log ratios); NaN for holder and light.
fn distance(img: &Rgb32f, base: &[f32; 3], threshold: f32, light: &Light) -> Plane {
    let lb = base.map(|c| (c.max(0.0) + 1e-4).ln());
    img.map(|p| {
        if luma(p) <= threshold || light.is(p) {
            return f32::NAN;
        }
        (0..3).map(|i| ((p[i].max(0.0) + 1e-4).ln() - lb[i]).abs()).sum()
    })
}

// ---- the picture's sides

/// A place the picture may start on one side: `depth` pixels in from the film's edge, and how
/// little the scan backs it (0 = sure).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Cand {
    depth: usize,
    cost: f64,
}

/// The fewest sides the scan must show; the rest are placed from them.
const MIN_SIDES: u8 = 2;
/// What a side not found costs a choice of sides (it is placed from the others).
const MISSING_COST: f64 = 1.0;
/// Weight of a side's rebate differing from the others' (per fraction of the film's extent).
const DEPTH_WEIGHT: f64 = 10.0;
/// How much the rebates may differ for free (fraction of the film's extent): a holder never masks
/// the sides exactly alike, and a roll film's frame gap is narrower than its edge. A step deep in
/// the scene (a quarter of the frame) still costs.
const DEPTH_TOLERANCE: f64 = 0.04;
/// Weight of a picture differing from the chosen format's aspect (per unit of log aspect).
const ASPECT_WEIGHT: f64 = 20.0;
/// Most candidates kept per side (besides the colour step).
const MAX_CANDS: usize = 4;
/// Weakest share of a line stepping the same way that is still a candidate.
const WEAK_SHARE: f32 = 0.5;

/// The film's edge on `side`, in pixels from the image's edge, and its extent across the side.
fn film_edge(film: Px, side: Side, (w, h): (usize, usize)) -> (usize, usize) {
    match side {
        Side::Top => (film.y0, film.height()),
        Side::Bottom => (h.saturating_sub(film.y1), film.height()),
        Side::Left => (film.x0, film.width()),
        Side::Right => (w.saturating_sub(film.x1), film.width()),
    }
}

/// Everything a side's picture edge reads.
struct Scan<'a> {
    dist: &'a Plane,
    lum: &'a Plane,
    img: &'a Rgb32f,
    light: &'a Light,
    threshold: f32,
    film: Px,
}

impl Scan<'_> {
    fn size(&self) -> (usize, usize) {
        (self.dist.width, self.dist.height)
    }

    /// The lines across the central part of `side` (columns for top and bottom, rows for the others).
    fn lines(&self, side: Side) -> Vec<usize> {
        let f = self.film;
        let all = if side.horizontal() { f.x0..f.x1 } else { f.y0..f.y1 };
        let c = central(all.len());
        all.skip(c.start).take(c.len()).collect()
    }

    /// The median distance to the base `i` in from the film's edge on `side` (NaN where too little
    /// of the line is film).
    fn profile(&self, side: Side, start: usize, lines: &[usize], reach: usize) -> Vec<f32> {
        let (w, h) = self.size();
        (0..reach)
            .map(|i| {
                let v: Vec<f32> = lines
                    .iter()
                    .filter_map(|j| at(side, start + i, *j, w, h).and_then(|(x, y)| px(self.dist, x, y)))
                    .filter(|v| v.is_finite())
                    .collect();
                if v.len() * 3 < lines.len() { f32::NAN } else { median(v).unwrap_or(f32::NAN) }
            })
            .collect()
    }

    /// For each line `i` in from the film's edge in `0..reach`, the share of the side that steps
    /// the same way in brightness from `i - 1` to `i` (light and holder left out).
    fn shares(&self, side: Side, start: usize, lines: &[usize], reach: usize) -> Vec<f32> {
        let (w, h) = self.size();
        let ok = |x: usize, y: usize| px(self.img, x, y).is_some_and(|p| luma(p) > self.threshold && !self.light.is(p));
        (0..reach)
            .map(|i| {
                let Some(prev) = i.checked_sub(1) else { return 0.0 };
                let steps: Vec<f32> = lines
                    .iter()
                    .filter_map(|j| {
                        let (xa, ya) = at(side, start + prev, *j, w, h)?;
                        let (xb, yb) = at(side, start + i, *j, w, h)?;
                        (ok(xa, ya) && ok(xb, yb)).then_some(())?;
                        Some(px(self.lum, xb, yb)? - px(self.lum, xa, ya)?)
                    })
                    .collect();
                if steps.len() * 2 < lines.len() {
                    return 0.0;
                }
                let up = steps.iter().filter(|s| **s > EDGE_LOG_STEP).count();
                let down = steps.iter().filter(|s| **s < -EDGE_LOG_STEP).count();
                up.max(down) as f32 / steps.len() as f32
            })
            .collect()
    }

    /// The places the picture may start on `side`, in from the film's edge.
    fn candidates(&self, side: Side) -> Vec<Cand> {
        let (start, across) = film_edge(self.film, side, self.size());
        let lines = self.lines(side);
        let reach = ((across as f64) * SEARCH) as usize;
        let profile = self.profile(side, start, &lines, reach);
        let skip = (reach / 40).max(2);
        let Some(rebate) = median(profile.iter().skip(skip).take(skip * 2).copied().collect()) else { return Vec::new() };
        if rebate > REBATE_MAX {
            // the picture (or something that isn't rebate) reaches the film's edge
            return vec![Cand { depth: 0, cost: 0.0 }];
        }
        let shares = self.shares(side, start, &lines, reach);
        let mut out = Vec::new();
        if let Some(i) = colour_step(&profile, rebate, skip) {
            // the sharpest step just before the profile left the base
            let from = i.saturating_sub(reach / 10).max(skip);
            let best = (from..=i).max_by(|a, b| share_at(&shares, *a).total_cmp(&share_at(&shares, *b)));
            let depth = best.filter(|b| share_at(&shares, *b) >= WEAK_SHARE).unwrap_or(i);
            out.push(Cand { depth, cost: 0.0 });
        }
        out.extend(gate_edges(&shares, skip));
        out
    }
}

fn share_at(shares: &[f32], i: usize) -> f32 {
    shares.get(i).copied().unwrap_or(0.0)
}

/// The first place the profile stays above the rebate by [`STEP`] for a few samples.
fn colour_step(profile: &[f32], rebate: f32, skip: usize) -> Option<usize> {
    let run = 3;
    let above = |v: &f32| v.is_finite() && *v > rebate + STEP;
    (skip..profile.len().saturating_sub(run)).find(|i| profile.get(*i..i + run).is_some_and(|s| s.iter().all(above)))
}

/// Lines across which most of the side steps the same way (the edge of the camera's gate, or a
/// straight edge in the picture): the local peaks, strongest first.
fn gate_edges(shares: &[f32], skip: usize) -> Vec<Cand> {
    let mut peaks: Vec<(usize, f32)> = (skip.max(1)..shares.len())
        .filter_map(|i| {
            let s = share_at(shares, i);
            let peak = s >= WEAK_SHARE && s >= share_at(shares, i - 1) && s > share_at(shares, i + 1);
            peak.then_some((i, s))
        })
        .collect();
    peaks.sort_by(|a, b| b.1.total_cmp(&a.1));
    peaks.truncate(MAX_CANDS);
    // a full edge costs little, one the side only half steps across nearly as much as none
    peaks.into_iter().map(|(i, s)| Cand { depth: i, cost: 0.1 + f64::from(1.0 - s.min(1.0)) * 1.6 }).collect()
}

/// The sides' depths (in from the film's edges) as a picture box; missing sides placed.
fn complete(depths: [Option<f64>; 4], film: Px, ratio: Option<f64>) -> Option<Px> {
    let [mut top, mut bottom, mut left, mut right] = depths;
    let (fw, fh) = (film.width() as f64, film.height() as f64);
    // one missing side of an axis: from the format's aspect (width over height) when the other
    // axis is whole, else as deep as the side across from it; both missing: as deep as the other
    // axis's sides
    for _ in 0..3 {
        if let Some(ratio) = ratio {
            if let (Some(t), Some(b)) = (top, bottom) {
                fill(&mut left, &mut right, fw - (fh - t - b) * ratio);
            }
            if let (Some(l), Some(r)) = (left, right) {
                fill(&mut top, &mut bottom, fh - (fw - l - r) / ratio);
            }
        }
        // (a side whose picture reaches the film's edge says nothing about the rebate)
        let rebate = |a: Option<f64>, b: Option<f64>| {
            let v: Vec<f64> = [a, b].into_iter().flatten().filter(|v| *v > 0.0).collect();
            (!v.is_empty()).then(|| v.iter().sum::<f64>() / v.len() as f64)
        };
        let (across_y, across_x) = (rebate(left, right), rebate(top, bottom));
        // with a format, a side across from one whose picture reaches the film's edge is left to the
        // format, once the other axis is whole
        mirror(&mut top, &mut bottom, across_y, ratio.is_some());
        mirror(&mut left, &mut right, across_x, ratio.is_some());
        if top.is_none()
            && bottom.is_none()
            && let Some(d) = across_y
        {
            (top, bottom) = (Some(d), Some(d));
        }
        if left.is_none()
            && right.is_none()
            && let Some(d) = across_x
        {
            (left, right) = (Some(d), Some(d));
        }
    }
    let d = |v: Option<f64>| v.unwrap_or(0.0).max(0.0);
    let r = Px {
        x0: (film.x0 as f64 + d(left)).round() as usize,
        y0: (film.y0 as f64 + d(top)).round() as usize,
        x1: (film.x1 as f64 - d(right)).max(0.0).round() as usize,
        y1: (film.y1 as f64 - d(bottom)).max(0.0).round() as usize,
    };
    (r.width() > 0 && r.height() > 0).then_some(r)
}

/// Splits `total` (the rebate of both sides of an axis) when one side is known.
fn fill(a: &mut Option<f64>, b: &mut Option<f64>, total: f64) {
    if !total.is_finite() {
        return;
    }
    match (*a, *b) {
        (Some(x), None) => *b = Some(total - x),
        (None, Some(y)) => *a = Some(total - y),
        (None, None) => (*a, *b) = (Some(total / 2.0), Some(total / 2.0)),
        _ => {}
    }
}

/// One side missing: as deep as the side across from it, unless the picture reaches the film's
/// edge there; then as deep as the other axis's rebate (`other`), else on the film's edge. With
/// `by_format`, such a side is left for the format to place.
fn mirror(a: &mut Option<f64>, b: &mut Option<f64>, other: Option<f64>, by_format: bool) {
    let like = |v: f64| {
        if v > 0.0 {
            Some(v)
        } else if by_format {
            None
        } else {
            Some(other.unwrap_or(0.0))
        }
    };
    match (*a, *b) {
        (Some(x), None) => *b = like(x),
        (None, Some(y)) => *a = like(y),
        _ => {}
    }
}

/// The choice of one candidate (or none) per side that the scan backs best: each side's own cost,
/// rebates that differ from each other, and a picture of another aspect than the format's.
fn choose(cands: &[Vec<Cand>; 4], film: Px, ratio: Option<f64>) -> Option<(Px, u8)> {
    let opts: Vec<Vec<Option<Cand>>> = cands.iter().map(|c| std::iter::once(None).chain(c.iter().copied().map(Some)).collect()).collect();
    let extent = |i: usize| (if i < 2 { film.height() } else { film.width() }).max(1) as f64;
    let mut best: Option<(f64, Px, u8)> = None;
    let n = |i: usize| opts.get(i).map_or(1, Vec::len);
    for a in 0..n(0) {
        for b in 0..n(1) {
            for c in 0..n(2) {
                for d in 0..n(3) {
                    let at = [a, b, c, d];
                    let pick = [0, 1, 2, 3].map(|s| opts.get(s).and_then(|o| o.get(at[s]).copied().flatten()));
                    let Some((cost, r)) = score(&pick, film, ratio, &extent) else { continue };
                    if best.as_ref().is_none_or(|b| cost < b.0) {
                        best = Some((cost, r, pick.iter().filter(|p| p.is_some()).count() as u8));
                    }
                }
            }
        }
    }
    best.map(|b| (b.1, b.2))
}

fn score(pick: &[Option<Cand>; 4], film: Px, ratio: Option<f64>, extent: &dyn Fn(usize) -> f64) -> Option<(f64, Px)> {
    let own: f64 = pick.iter().map(|p| p.map_or(MISSING_COST, |c| c.cost)).sum();
    let fracs: Vec<f32> = pick.iter().enumerate().filter_map(|(i, p)| p.map(|c| (c.depth as f64 / extent(i)) as f32)).collect();
    let m = median(fracs.clone()).unwrap_or(0.0);
    let depth: f64 = fracs.iter().map(|f| (f64::from((f - m).abs()) - DEPTH_TOLERANCE).max(0.0)).sum::<f64>() * DEPTH_WEIGHT;
    let r = complete(pick.map(|p| p.map(|c| c.depth as f64)), film, ratio)?;
    let aspect = ratio.map_or(0.0, |want| (r.width() as f64 / r.height().max(1) as f64 / want).ln().abs() * ASPECT_WEIGHT);
    Some((own + depth + aspect, r))
}

/// A picture that is a reasonable part of the scan.
fn plausible(r: &Px, w: usize, h: usize) -> Result<(), String> {
    let area = r.width() as f64 * r.height() as f64 / (w as f64 * h as f64).max(1.0);
    let aspect = r.width() as f64 / r.height().max(1) as f64;
    if area < 0.1 || !(0.25..=4.0).contains(&aspect) {
        return Err("no film frame found".into());
    }
    Ok(())
}

/// The median of the rebate: film outside the picture, light left out.
fn rebate_base(img: &Rgb32f, film: Px, pic: Px, threshold: f32, light: &Light) -> Option<[f64; 3]> {
    let mut v: Vec<[f32; 3]> = Vec::new();
    // a pixel or two of slack around the picture's edge
    let pad = 2;
    for y in film.y0..film.y1 {
        for x in film.x0..film.x1 {
            let inside = x + pad >= pic.x0 && x < pic.x1 + pad && y + pad >= pic.y0 && y < pic.y1 + pad;
            if let Some(p) = px(img, x, y).filter(|p| !inside && luma(*p) > threshold && !light.is(*p)) {
                v.push(p);
            }
        }
    }
    let min = (img.width.min(img.height) * 2).max(100);
    if v.len() < min {
        return None;
    }
    channel_median(&v).map(|c| c.map(|x| f64::from(x).clamp(0.0, 1.0)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: usize = 900;
    const H: usize = 600;
    /// The film inside the holder, and the picture on it (3:2), in pixels of the straight scan.
    const FILM: [f64; 4] = [60.0, 40.0, 840.0, 560.0];
    const PIC: [f64; 4] = [120.0, 80.0, 780.0, 520.0];
    const BASE: [f32; 3] = [0.62, 0.26, 0.07];

    fn inside(r: [f64; 4], x: f64, y: f64) -> bool {
        x >= r[0] && x < r[2] && y >= r[1] && y < r[3]
    }

    /// A straight scan of one frame of 35 mm colour negative (linear sRGB) at `(x, y)`: holder,
    /// orange base, sprocket holes along the top and bottom (those at the top cut by the holder),
    /// and a textured picture whose left `thin` part is as thin as the base.
    fn strip(x: f64, y: f64, thin: f64) -> [f32; 3] {
        if !inside(FILM, x, y) {
            return [0.004, 0.003, 0.003];
        }
        // (35 mm holes: deeper across the film than long along it)
        let hole = |cy: f64| ((x - 40.0).rem_euclid(56.0) - 28.0).powi(2) / 100.0 + (y - cy).powi(2) / 196.0 < 1.0;
        if hole(38.0) || hole(542.0) {
            return [0.92, 0.93, 0.97];
        }
        if !inside(PIC, x, y) || x < PIC[0] + thin {
            return BASE;
        }
        // denser than the base: the scene, a pattern of soft bands and blocks
        let t = 0.2 + 0.25 * ((x / 37.0).sin() * (y / 23.0).cos()).abs() + if ((x / 90.0) as i32 + (y / 70.0) as i32) % 2 == 0 { 0.2 } else { 0.0 };
        BASE.map(|c| c * t as f32)
    }

    /// The scan turned by a crop angle of `angle` degrees (as a holder put in askew), in Rec.2020.
    fn scan(angle: f64, thin: f64) -> Rgb32f {
        let fwd = Affine::rotate_about(angle.to_radians(), Point::new(W as f64 / 2.0, H as f64 / 2.0));
        let m = SRGB.to_space(&REC2020).to_f32();
        Rgb32f::from_fn(W, H, |x, y| {
            let q = fwd.apply(Point::new(x as f64 + 0.5, y as f64 + 0.5));
            let p = strip(q.x, q.y, thin);
            m.map(|r| r[0] * p[0] + r[1] * p[1] + r[2] * p[2])
        })
    }

    fn near(a: Rect, b: [f64; 4], tol: f64) -> bool {
        let b = [b[0] / W as f64, b[1] / H as f64, b[2] / W as f64, b[3] / H as f64];
        [(a.x0, b[0]), (a.y0, b[1]), (a.x1, b[2]), (a.y1, b[3])].iter().all(|(x, y)| (x - y).abs() < tol)
    }

    #[test]
    fn finds_the_picture_on_a_straight_scan() {
        let f = detect(&scan(0.0, 0.0), None).unwrap();
        assert!(f.angle.abs() < 0.05, "{f:?}");
        assert!(near(f.picture, PIC, 0.01), "{f:?}");
        assert!(near(f.film, FILM, 0.01), "{f:?}");
        assert!(f.sprockets && f.sides_found == 4, "{f:?}");
        let b = f.base.expect("the rebate shows");
        assert!((0..3).all(|i| (b[i] - f64::from(BASE[i])).abs() < 0.02), "{b:?}");
    }

    #[test]
    fn straightens_a_scan_put_in_askew() {
        for angle in [2.3, -1.7] {
            let f = detect(&scan(angle, 0.0), None).unwrap();
            assert!((f.angle - angle).abs() < 0.2, "{angle}: {f:?}");
            assert!(near(f.picture, PIC, 0.015), "{angle}: {f:?}");
        }
    }

    #[test]
    fn a_side_as_thin_as_the_base_is_placed_by_the_format_or_the_rebate_across() {
        // the left 200 px of the picture show nothing denser than the base
        let s = scan(0.0, 200.0);
        let f = detect(&s, format_ratio("35mm").unwrap()).unwrap();
        assert!(near(f.picture, PIC, 0.01), "3:2: {f:?}");
        assert_eq!(f.sides_found, 3);
        // auto: as deep as the rebate on the right, which is the same here
        let f = detect(&s, None).unwrap();
        assert!(near(f.picture, PIC, 0.01), "auto: {f:?}");
    }

    #[test]
    fn the_margin_moves_the_crop_in_or_out_and_stays_on_the_film() {
        // (the ends of the frame, along the film: the long sides go by the sprocket holes)
        let f = detect(&scan(0.0, 0.0), None).unwrap();
        let (w, h) = (W as f64, H as f64);
        let [inside, edge, border] = [Margin::Inside, Margin::Edge, Margin::Border].map(|m| f.crop(m, w, h));
        assert_eq!((edge.x0, edge.x1), (f.picture.x0, f.picture.x1));
        assert!(inside.x0 > edge.x0 && inside.x1 < edge.x1, "{inside:?}");
        assert!(border.x0 < edge.x0 && border.x1 > edge.x1, "{border:?}");
        assert!(border.x0 >= f.film.x0 && border.y0 >= f.film.y0 && border.x1 <= f.film.x1 && border.y1 <= f.film.y1);
        // without holes, every side is the picture's
        let mut no_holes = f;
        no_holes.holes = [None, None];
        assert_eq!(no_holes.crop(Margin::Edge, w, h), f.picture);
        let inside = no_holes.crop(Margin::Inside, w, h);
        assert!(inside.y0 > f.picture.y0 && inside.y1 < f.picture.y1, "{inside:?}");
    }

    /// The holder cuts the holes at the top and shows them whole at the bottom: the crop runs just
    /// inside the cut ones (the rebate between them and the picture stays) and just outside the
    /// whole ones, kept on the film; the ends of the frame stay on the picture.
    #[test]
    fn the_crop_goes_by_the_sprocket_holes() {
        let f = detect(&scan(0.0, 0.0), None).unwrap();
        assert!(f.along_x && f.sprockets, "{f:?}");
        let [top, bottom] = f.holes;
        let (top, bottom) = (top.expect("the cut holes at the top"), bottom.expect("the whole holes at the bottom"));
        // the holes span 24..52 (the holder from 40) and 528..556 of the 600 rows
        assert!(!top.whole && (top.outer * 600.0 - 40.0).abs() < 3.0 && (top.inner * 600.0 - 52.0).abs() < 3.0, "{top:?}");
        assert!(bottom.whole && (bottom.outer * 600.0 - 556.0).abs() < 3.0 && (bottom.inner * 600.0 - 528.0).abs() < 3.0, "{bottom:?}");
        let c = f.crop(Margin::Inside, W as f64, H as f64);
        let gap = HOLE_MARGIN * f.film.height();
        assert!((c.y0 - (top.inner + gap)).abs() < 1e-9 && c.y0 < f.picture.y0, "just inside the cut holes: {c:?}");
        assert!((c.y1 - f.film.y1).abs() < 1e-9, "outside the whole holes, on the film: {c:?}");
        assert!(c.x0 > f.picture.x0 && c.x1 < f.picture.x1, "the ends inside the picture: {c:?}");
    }

    #[test]
    fn formats_parse_and_unknown_ones_are_errors() {
        assert_eq!(format_ratio("auto"), Ok(None));
        assert_eq!(format_ratio("66"), Ok(Some(1.0)));
        assert!(format_ratio("617").unwrap().is_some_and(|r| (r - 3.0).abs() < 1e-9));
        assert!(format_ratio("8x10").is_err());
        assert_eq!(Margin::parse("edge"), Some(Margin::Edge));
        assert_eq!(Margin::parse("wide"), None);
    }

    /// Never-crash: scans with nothing to find are errors, not panics.
    #[test]
    fn scans_without_a_frame_are_errors() {
        let bad = [
            Rgb32f::new(0, 0),
            Rgb32f::new(1, 1),
            Rgb32f::filled(64, 48, [0.0; 3]),
            Rgb32f::filled(64, 48, [1.0; 3]),
            Rgb32f::filled(64, 48, [f32::NAN; 3]),
            Rgb32f::filled(64, 48, [f32::INFINITY, -1.0, f32::NAN]),
            Rgb32f::from_fn(300, 200, |x, y| if (x + y) % 2 == 0 { [1.0; 3] } else { [0.0; 3] }),
        ];
        for img in &bad {
            for ratio in [None, Some(1.5), Some(f64::NAN), Some(0.0)] {
                let r = detect(img, ratio);
                assert!(r.is_err(), "{}×{} {:?}: {r:?}", img.width, img.height, img.data.first());
            }
        }
        // a truncated image (data shorter than its size claims)
        let mut short = Rgb32f::filled(64, 48, [0.5; 3]);
        short.data.truncate(10);
        assert!(detect(&short, None).is_err());
    }

    /// A frame of 120 film (6×6) in a holder that masks the film's long edges, on a 900 × 640 scan:
    /// the film shows on 150..770 × 20..600, the picture on 172..748 × 22..598, so the picture
    /// reaches the holder at the top and the bottom and the rebate shows only between frames, left
    /// and right. The sky (top) is dense, the rest about as thin as the base; a dense block from
    /// x = 420 to 640 (mid-height) is a colour step deep inside the picture from the right. The
    /// frame lines step across 47 % of the rows on the left and 60 % on the right (bright foliage
    /// beside them hides the rest).
    fn medium_format(x: f64, y: f64) -> [f32; 3] {
        let film = [150.0, 20.0, 770.0, 600.0];
        let pic = [172.0, 22.0, 748.0, 598.0];
        if !inside(film, x, y) {
            return [0.004, 0.003, 0.003];
        }
        if !inside(pic, x, y) {
            return BASE;
        }
        let row = ((y as usize).wrapping_mul(37) % 100) as f64;
        let steps = if x < 212.0 {
            row < 47.0
        } else if x >= 708.0 {
            row < 60.0
        } else {
            true
        };
        let dense = y < 130.0 || inside([420.0, 200.0, 640.0, 420.0], x, y);
        let t = if dense {
            0.15
        } else if steps {
            0.85
        } else {
            1.0
        };
        BASE.map(|c| c * t as f32)
    }

    #[test]
    fn a_medium_format_frame_whose_picture_meets_the_holder_is_found() {
        let (w, h) = (900usize, 640usize);
        let m = SRGB.to_space(&REC2020).to_f32();
        let s = Rgb32f::from_fn(w, h, |x, y| {
            let p = medium_format(x as f64 + 0.5, y as f64 + 0.5);
            m.map(|r| r[0] * p[0] + r[1] * p[1] + r[2] * p[2])
        });
        let want = [172.0 / w as f64, 22.0 / h as f64, 748.0 / w as f64, 598.0 / h as f64];
        let close = |a: Rect, tol: f64| [(a.x0, want[0]), (a.y0, want[1]), (a.x1, want[2]), (a.y1, want[3])].iter().all(|(x, y)| (x - y).abs() < tol);
        // 6×6: the top and the bottom on the holder, the sides on the frame lines
        let f = detect(&s, format_ratio("66").unwrap()).unwrap();
        assert!(close(f.picture, 0.01), "6×6: {f:?}");
        // (the top on the holder and one frame line; the rest placed from them)
        assert!(f.sides_found >= 2, "{f:?}");
        // auto: the frame lines, never the dense block inside the picture
        let f = detect(&s, None).unwrap();
        assert!((f.picture.x0 - want[0]).abs() < 0.01 && (f.picture.x1 - want[2]).abs() < 0.01, "auto: {f:?}");
        assert!(f.picture.height() > 0.85, "auto: {f:?}");
    }
}
