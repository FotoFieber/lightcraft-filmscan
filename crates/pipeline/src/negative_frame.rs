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
//!    film evenly) and, when a format is chosen, the picture's aspect. A side left out is placed by
//!    the format, else as deep as the side across from it.
//! 6. **Base.** The median of the rebate (film outside the picture, light left out).
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
}

impl Frame {
    /// The crop for `margin`: the picture moved out or in by the margin, kept on the film.
    pub fn crop(&self, margin: Margin, w: f64, h: f64) -> Rect {
        let (w, h) = (w.max(1.0), h.max(1.0));
        let m = margin.offset() * w.min(h);
        let (mx, my) = (m / w, m / h);
        let p = self.picture;
        let r = Rect::new(p.x0 - mx, p.y0 - my, p.x1 + mx, p.y1 + my);
        let f = self.film;
        let r = if margin == Margin::Border { Rect::new(r.x0.max(f.x0), r.y0.max(f.y0), r.x1.min(f.x1), r.y1.min(f.y1)) } else { r };
        Rect::new(r.x0.clamp(0.0, 1.0), r.y0.clamp(0.0, 1.0), r.x1.clamp(0.0, 1.0), r.y1.clamp(0.0, 1.0))
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
    Ok(Frame { angle, picture: norm(picture), film: norm(film), base, sprockets, sides_found })
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
        mirror(&mut top, &mut bottom, across_y);
        mirror(&mut left, &mut right, across_x);
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
/// edge there; then as deep as the other axis's rebate (`other`), else on the film's edge.
fn mirror(a: &mut Option<f64>, b: &mut Option<f64>, other: Option<f64>) {
    let like = |v: f64| if v > 0.0 { v } else { other.unwrap_or(0.0) };
    match (*a, *b) {
        (Some(x), None) => *b = Some(like(x)),
        (None, Some(y)) => *a = Some(like(y)),
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
    let depth: f64 = fracs.iter().map(|f| f64::from((f - m).abs())).sum::<f64>() * DEPTH_WEIGHT;
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
        let hole = |cy: f64| ((x - 40.0).rem_euclid(56.0) - 28.0).powi(2) / 196.0 + (y - cy).powi(2) / 64.0 < 1.0;
        if hole(38.0) || hole(548.0) {
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
        let f = detect(&scan(0.0, 0.0), None).unwrap();
        let (w, h) = (W as f64, H as f64);
        let [inside, edge, border] = [Margin::Inside, Margin::Edge, Margin::Border].map(|m| f.crop(m, w, h));
        assert_eq!(edge, f.picture);
        assert!(inside.x0 > edge.x0 && inside.y1 < edge.y1 && inside.width() < edge.width(), "{inside:?}");
        assert!(border.x0 < edge.x0 && border.y1 > edge.y1, "{border:?}");
        assert!(border.x0 >= f.film.x0 && border.y0 >= f.film.y0 && border.x1 <= f.film.x1 && border.y1 <= f.film.y1);
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
}
