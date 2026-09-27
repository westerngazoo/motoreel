//! R-0012 — paths and fills: the QA-owned acceptance suite (e2e).
//!
//! Loop step 3, TDD red. Authored before any implementation exists, from
//! `requirements/0012-paths-and-fills.md` (AC1–AC12, with AC6 as amended on
//! 2026-09-27) and SPEC-0012 §6. There is one `mod` per criterion, as
//! SPEC-0012 §3 lays out, and each test's header comment says what it pins.
//! `specs/0012-test-plan.md` holds the criterion → test table, the edge
//! cases, what is left to step 7, the unit tests proposed for `ppm.rs`'s
//! private flattening and coverage code, and the spec ambiguities found
//! while writing this suite.
//!
//! **Red at compile time, not at run time.** R-0006 and R-0007 went red at
//! run time, because their suites landed together with `unimplemented!()`
//! stubs in `src/`. The qa agent writes test code only, so this suite lands
//! without stubs. It does not compile until SPEC-0012's API exists: `Seg`,
//! `Subpath`, `Fill`, `Style::fill`, `Shape::Path`, `Prim2::Path`,
//! `Object::{path, planar}`, `motoreel::path::split_cubic`,
//! `motoreel::shapes::*` and `examples/card/scene.rs`. Every other test
//! target is a separate crate, so each still builds and passes on its own.
//!
//! **Exactness levels.** These follow SPEC-0006 §2.8, and which layer
//! carries a claim is part of the claim:
//!
//! - The SVG grammar is asserted by full-line equality.
//! - AC5's identities are asserted as full-frame byte equality, because
//!   every coverage involved is a multiple of 1/16 and so survives the
//!   byte `round` exactly.
//! - The corner bound and the flattening tolerance are sums and distances,
//!   so at frame level they carry the rounding slack `1/510` per fractional
//!   pixel, and the slack is stated wherever it is used. Their exact `f64`
//!   forms need the private coverage and chord functions, so they are
//!   proposed as unit tests in the test plan.

use std::collections::HashMap;
use std::f64::consts::{SQRT_2, TAU};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use garust::{pga, Motor3, Pga3};
use motoreel::path::split_cubic;
use motoreel::shapes::{arc, circle, polygon, rounded_rect, sector};
use motoreel::{
    Align, Camera, Fill, FrameSink, Object, PpmSink, Prim2, Pt2, Rgb, Scene, Seg, Shape, Style,
    Subpath, SvgSink, Track,
};

/// The shipped card demo, included from the example source itself so the
/// demo and AC12's tests cannot drift apart. This is the convention of
/// SPEC-0003 §2.7 and SPEC-0004 §2.0. `rustfmt::skip` keeps the format gate
/// green while the file does not exist yet (TDD red); once it lands,
/// rustfmt still reaches it through the example's own `mod scene;`.
#[rustfmt::skip]
#[path = "../examples/card/scene.rs"]
mod card;

// --- Shared helpers ---------------------------------------------------------

/// A clean per-test scratch directory under `CARGO_TARGET_TMPDIR`, the
/// R-0003 convention. Stale frames are removed so file counts are exact,
/// and the directory itself is left for the sink under test to create.
fn tmp_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("clean the stale test directory");
    }
    dir
}

/// Sorted file names in a directory.
fn dir_names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .expect("read the test output directory")
        .map(|e| {
            e.expect("directory entry")
                .file_name()
                .into_string()
                .expect("UTF-8 file name")
        })
        .collect();
    names.sort();
    names
}

fn p(x: f64, y: f64) -> Pt2 {
    Pt2 { x, y }
}

const WHITE: Rgb = Rgb::WHITE;
const BLACK: Rgb = Rgb::BLACK;
const RED: Rgb = Rgb {
    r: 0xff,
    g: 0x00,
    b: 0x00,
};
/// The golden scene's colours (SPEC-0012 §2.10), reused elsewhere.
const GOLD: Rgb = Rgb {
    r: 0xe9,
    g: 0xb2,
    b: 0x3f,
};
const INK: Rgb = Rgb {
    r: 0x14,
    g: 0x14,
    b: 0x17,
};
const STEEL: Rgb = Rgb {
    r: 0xce,
    g: 0xd1,
    b: 0xd9,
};
const SIGNAL: Rgb = Rgb {
    r: 0xe0,
    g: 0x32,
    b: 0x2a,
};

fn rgb(c: Rgb) -> [u8; 3] {
    [c.r, c.g, c.b]
}

fn paint(stroke: Rgb, width: f64, alpha: f64, fill: Option<Fill>) -> Style {
    Style {
        stroke,
        width,
        alpha,
        fill,
    }
}

fn solid(colour: Rgb, alpha: f64) -> Option<Fill> {
    Some(Fill::solid(colour, alpha))
}

/// Fill only. A stroke width of 0 is how SPEC-0012 §2.7 authors that.
fn filled(colour: Rgb, alpha: f64) -> Style {
    paint(colour, 0.0, 1.0, solid(colour, alpha))
}

/// Stroke only: no fill.
fn stroked(colour: Rgb, width: f64, alpha: f64) -> Style {
    paint(colour, width, alpha, None)
}

fn path_prim(subpaths: Vec<Subpath<Pt2>>, style: Style) -> Prim2 {
    Prim2::Path { subpaths, style }
}

/// The P6 header for a raster size.
fn header(size: (u32, u32)) -> Vec<u8> {
    format!("P6\n{} {}\n255\n", size.0, size.1).into_bytes()
}

/// A decoded P6 frame: its size and its pixel bytes, header stripped.
struct Ppm {
    size: (u32, u32),
    pixels: Vec<u8>,
}

impl Ppm {
    fn at(&self, x: u32, y: u32) -> [u8; 3] {
        let i = (y as usize * self.size.0 as usize + x as usize) * 3;
        [self.pixels[i], self.pixels[i + 1], self.pixels[i + 2]]
    }

    /// The green channel. White ink on black makes all three equal.
    fn grey(&self, x: u32, y: u32) -> u8 {
        self.at(x, y)[1]
    }

    fn is_pure(&self, background: [u8; 3]) -> bool {
        self.pixels
            .as_chunks::<3>()
            .0
            .iter()
            .all(|px| *px == background)
    }
}

/// Render one frame of `prims` through a `PpmSink` and return the whole
/// file, after checking its header and length.
fn render_ppm_bytes(name: &str, size: (u32, u32), view: (f64, f64), prims: &[Prim2]) -> Vec<u8> {
    let dir = tmp_dir(name);
    let mut sink = PpmSink::with_view(&dir, size, view).expect("ppm sink");
    sink.frame(0, prims).expect("write the ppm frame");
    let bytes = fs::read(dir.join("frame_00000.ppm")).expect("read the ppm frame");
    let head = header(size);
    assert_eq!(&bytes[..head.len()], &head[..], "P6 header");
    assert_eq!(
        bytes.len(),
        head.len() + size.0 as usize * size.1 as usize * 3,
        "P6 length"
    );
    bytes
}

fn render_ppm(name: &str, size: (u32, u32), view: (f64, f64), prims: &[Prim2]) -> Ppm {
    let bytes = render_ppm_bytes(name, size, view, prims);
    Ppm {
        size,
        pixels: bytes[header(size).len()..].to_vec(),
    }
}

fn render_svg(name: &str, size: (u32, u32), view: (f64, f64), prims: &[Prim2]) -> String {
    let dir = tmp_dir(name);
    let mut sink = SvgSink::with_view(&dir, size, view).expect("svg sink");
    sink.frame(0, prims).expect("write the svg frame");
    fs::read_to_string(dir.join("frame_00000.svg")).expect("read the svg frame")
}

/// The `<path` elements of a rendered document, in order.
fn path_lines(svg: &str) -> Vec<&str> {
    svg.lines().filter(|l| l.starts_with("<path")).collect()
}

/// The `d` attribute SPEC-0012 §2.8 pins: absolute `M`, `L`, `C`, `Z`,
/// space-separated, each point written `x,y` with Rust's `{}`, and `Z` only
/// where `closed` is set.
fn d_of(subpaths: &[Subpath<Pt2>]) -> String {
    let mut parts = Vec::new();
    for sub in subpaths {
        parts.push(format!("M {},{}", sub.start.x, sub.start.y));
        for seg in &sub.segs {
            parts.push(match seg {
                Seg::Line(e) => format!("L {},{}", e.x, e.y),
                Seg::Cubic(h1, h2, e) => {
                    format!("C {},{} {},{} {},{}", h1.x, h1.y, h2.x, h2.y, e.x, e.y)
                }
            });
        }
        if sub.closed {
            parts.push("Z".to_string());
        }
    }
    parts.join(" ")
}

/// Pixel space (x right, y down) → image space (x right, y up, centred).
/// The inverse of SPEC-0006 §2.3's `to_pixel`. It is exact whenever `s` is a
/// power of two and the pixel coordinates are dyadic, which every caller
/// arranges.
fn img(size: (u32, u32), s: f64, u: f64, v: f64) -> Pt2 {
    p(
        (u - f64::from(size.0) / 2.0) / s,
        (f64::from(size.1) / 2.0 - v) / s,
    )
}

/// SPEC-0006 §2.3's pinned image → pixel mapping.
fn to_px(size: (u32, u32), s: f64, q: Pt2) -> (f64, f64) {
    (
        f64::from(size.0) / 2.0 + s * q.x,
        f64::from(size.1) / 2.0 - s * q.y,
    )
}

/// A pixel-space rectangle `[x0, x1] × [y0, y1]` (y down) as one closed
/// subpath, counter-clockwise in the y-up image space.
fn px_rect(size: (u32, u32), s: f64, (x0, y0): (f64, f64), (x1, y1): (f64, f64)) -> Subpath<Pt2> {
    Subpath::new(img(size, s, x0, y1))
        .line_to(img(size, s, x1, y1))
        .line_to(img(size, s, x1, y0))
        .line_to(img(size, s, x0, y0))
        .close()
}

/// The workhorse canvas: 64 × 64 px over a 4 × 4 view, so `s = 16` px per
/// unit exactly. Every sixteenth of a pixel is then a dyadic image
/// coordinate, which is what lets AC3 and AC5 assert bytes with `==`.
const GRID: (u32, u32) = (64, 64);
const GRID_VIEW: (f64, f64) = (4.0, 4.0);
const GS: f64 = 16.0;

fn g_rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Subpath<Pt2> {
    px_rect(GRID, GS, (x0, y0), (x1, y1))
}

/// A square of half-side `h` centred at `(cx, cy)`, counter-clockwise.
fn square(cx: f64, cy: f64, h: f64) -> [Pt2; 4] {
    [
        p(cx - h, cy - h),
        p(cx + h, cy - h),
        p(cx + h, cy + h),
        p(cx - h, cy + h),
    ]
}

/// The cubic Bézier through `c` at parameter `t`, in Bernstein form.
fn cubic_at(c: [Pt2; 4], t: f64) -> Pt2 {
    let u = 1.0 - t;
    let (b0, b1, b2, b3) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
    p(
        b0 * c[0].x + b1 * c[1].x + b2 * c[2].x + b3 * c[3].x,
        b0 * c[0].y + b1 * c[1].y + b2 * c[2].y + b3 * c[3].y,
    )
}

/// A polyline through `sub`: its start, the end of each line, and
/// `per_cubic` evenly spaced parameter samples along each cubic, the end
/// included.
fn sample(sub: &Subpath<Pt2>, per_cubic: usize) -> Vec<Pt2> {
    let mut out = vec![sub.start];
    let mut cur = sub.start;
    for seg in &sub.segs {
        match *seg {
            Seg::Line(e) => {
                out.push(e);
                cur = e;
            }
            Seg::Cubic(h1, h2, e) => {
                for k in 1..=per_cubic {
                    out.push(cubic_at([cur, h1, h2, e], k as f64 / per_cubic as f64));
                }
                cur = e;
            }
        }
    }
    out
}

/// Signed area by the shoelace formula over dense samples: positive means
/// counter-clockwise in y-up coordinates.
fn signed_area(subpaths: &[Subpath<Pt2>]) -> f64 {
    let mut twice = 0.0;
    for sub in subpaths {
        let pts = sample(sub, 256);
        for (i, a) in pts.iter().enumerate() {
            let b = pts[(i + 1) % pts.len()];
            twice += a.x * b.y - b.x * a.y;
        }
    }
    twice / 2.0
}

/// The on-curve points of a subpath: its start and every segment's end.
fn on_curve(sub: &Subpath<Pt2>) -> Vec<Pt2> {
    let mut out = vec![sub.start];
    out.extend(sub.segs.iter().map(|seg| match *seg {
        Seg::Line(e) | Seg::Cubic(_, _, e) => e,
    }));
    out
}

fn dist(a: Pt2, b: Pt2) -> f64 {
    ((a.x - b.x) * (a.x - b.x) + (a.y - b.y) * (a.y - b.y)).sqrt()
}

fn dist_to_segment(c: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let (abx, aby) = (b.0 - a.0, b.1 - a.1);
    let (acx, acy) = (c.0 - a.0, c.1 - a.1);
    let len2 = abx * abx + aby * aby;
    let t = if len2 > 0.0 {
        ((acx * abx + acy * aby) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let (dx, dy) = (acx - t * abx, acy - t * aby);
    (dx * dx + dy * dy).sqrt()
}

/// Nearest-distance queries against a set of short line segments,
/// bucketed on a uniform grid so that a dense reference curve stays cheap
/// to query.
///
/// The answer is exact whenever the nearest segment lies within one `cell`
/// of the query. Otherwise it may be too large or `+∞`, and every caller
/// treats anything that far as a failure anyway.
struct Nearest {
    cell: f64,
    buckets: HashMap<(i64, i64), Vec<usize>>,
    segs: Vec<((f64, f64), (f64, f64))>,
}

impl Nearest {
    fn new(polylines: &[Vec<(f64, f64)>], cell: f64) -> Self {
        let mut segs = Vec::new();
        for line in polylines {
            if let [only] = line.as_slice() {
                segs.push((*only, *only));
            }
            segs.extend(line.windows(2).map(|w| (w[0], w[1])));
        }
        let mut buckets: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
        for (i, &(a, b)) in segs.iter().enumerate() {
            let gx =
                ((a.0.min(b.0) / cell).floor() as i64)..=((a.0.max(b.0) / cell).floor() as i64);
            for x in gx {
                let gy =
                    ((a.1.min(b.1) / cell).floor() as i64)..=((a.1.max(b.1) / cell).floor() as i64);
                for y in gy {
                    buckets.entry((x, y)).or_default().push(i);
                }
            }
        }
        Nearest {
            cell,
            buckets,
            segs,
        }
    }

    fn distance(&self, c: (f64, f64)) -> f64 {
        let (gx, gy) = (
            (c.0 / self.cell).floor() as i64,
            (c.1 / self.cell).floor() as i64,
        );
        let mut best = f64::INFINITY;
        for dx in -1..=1 {
            for dy in -1..=1 {
                if let Some(ids) = self.buckets.get(&(gx + dx, gy + dy)) {
                    for &i in ids {
                        let (a, b) = self.segs[i];
                        best = best.min(dist_to_segment(c, a, b));
                    }
                }
            }
        }
        best
    }
}

/// A fixed-seed linear congruential generator: reproducible "random"
/// geometry with no new dev-dependency (AC11).
struct Lcg(u64);

impl Lcg {
    /// Uniform in `[0, 1)`.
    fn unit(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }

    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.unit()
    }
}

/// One of the crate's source files, for the scoped greps SPEC-0012 asks QA
/// to run.
fn src(file: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join(file);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// Source with every `//` comment removed and every string or char
/// literal's contents blanked, so greps and brace matching see code only.
/// Doc comments go too: `mul_add` in a sentence is not a call.
fn scrub(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '/' && chars.get(i + 1) == Some(&'/') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
        } else if c == '"' {
            out.push('"');
            i += 1;
            while i < chars.len() && chars[i] != '"' {
                if chars[i] == '\\' {
                    i += 1;
                }
                out.push(' ');
                i += 1;
            }
            out.push('"');
            i += 1;
        } else if c == '\'' && chars.get(i + 1) == Some(&'\\') {
            out.push_str("' '");
            i += 2;
            while i < chars.len() && chars[i] != '\'' {
                i += 1;
            }
            i += 1;
        } else if c == '\'' && chars.get(i + 2) == Some(&'\'') {
            out.push_str("' '");
            i += 3;
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Char index of each whole-word occurrence of `word` in `chars`.
fn word_positions(chars: &[char], word: &str) -> Vec<usize> {
    let w: Vec<char> = word.chars().collect();
    (0..chars.len().saturating_sub(w.len() - 1))
        .filter(|&i| {
            chars[i..i + w.len()] == w[..]
                && (i == 0 || !is_ident(chars[i - 1]))
                && chars.get(i + w.len()).is_none_or(|&c| !is_ident(c))
        })
        .collect()
}

/// The char range of the braced block that opens at the first `{` at or
/// after `from`, both braces included.
fn block_at(chars: &[char], from: usize) -> Option<(usize, usize)> {
    let open = (from..chars.len()).find(|&k| chars[k] == '{')?;
    let mut depth = 0usize;
    for (k, &c) in chars.iter().enumerate().skip(open) {
        if c == '{' {
            depth += 1;
        } else if c == '}' {
            depth -= 1;
            if depth == 0 {
                return Some((open, k));
            }
        }
    }
    None
}

/// The body of every `fn name` in scrubbed `code`, in order.
fn fn_bodies(code: &str, name: &str) -> Vec<String> {
    let chars: Vec<char> = code.chars().collect();
    word_positions(&chars, "fn")
        .into_iter()
        .filter_map(|at| {
            let rest: String = chars[at + 2..].iter().take(name.len() + 2).collect();
            let named = rest.trim_start().strip_prefix(name)?;
            if !named.starts_with(['(', '<']) {
                return None;
            }
            let (open, close) = block_at(&chars, at)?;
            Some(chars[open..=close].iter().collect())
        })
        .collect()
}

/// Scrubbed `code` with the bodies of every `fn name` blanked out.
fn without_fns(code: &str, name: &str) -> String {
    let mut out = code.to_string();
    for body in fn_bodies(code, name) {
        out = out.replacen(&body, "{}", 1);
    }
    out
}

/// The arm patterns of every `match` in scrubbed `code` whose arms name
/// `needle` (such as `"Prim2::"`), one `Vec` per match.
///
/// Only text at the match block's own brace depth is kept. That leaves the
/// arm patterns (minus their `{ .. }` field lists) and any unbraced arm
/// bodies, which is enough to find the patterns: each follows the last
/// top-level `,` before its `=>`, or the start of the block.
fn match_arms_naming(code: &str, needle: &str) -> Vec<Vec<String>> {
    let chars: Vec<char> = code.chars().collect();
    let mut out = Vec::new();
    for at in word_positions(&chars, "match") {
        let Some((open, close)) = block_at(&chars, at) else {
            continue;
        };
        let mut depth = 0usize;
        let mut top = String::new();
        for &c in &chars[open..=close] {
            match c {
                '{' => {
                    depth += 1;
                    top.push(' ');
                }
                '}' => {
                    depth -= 1;
                    top.push(' ');
                }
                _ if depth == 1 => top.push(c),
                _ => {}
            }
        }
        if !top.contains(needle) {
            continue;
        }
        let pieces: Vec<&str> = top.split("=>").collect();
        let patterns = pieces[..pieces.len() - 1]
            .iter()
            .map(|piece| {
                piece
                    .rsplit_once(',')
                    .map_or(*piece, |(_, pat)| pat)
                    .trim()
                    .to_string()
            })
            .collect();
        out.push(patterns);
    }
    out
}

/// True when an arm pattern matches anything: `_`, `_ if …`, or a bare
/// binding such as `other` or `p if …`.
fn is_catch_all(pattern: &str) -> bool {
    pattern.split('|').map(str::trim).any(|alt| {
        let head = alt.split_whitespace().next().unwrap_or("");
        let head = head.strip_prefix("ref").map_or(head, str::trim);
        head == "_"
            || (!head.is_empty()
                && head.chars().all(is_ident)
                && head.starts_with(|c: char| c.is_lowercase() || c == '_'))
    })
}

/// `Some(path)` when an ffmpeg with libx264 is available, as R-0006 AC6
/// does it; `None`, with a printed note, otherwise. AC12's encode **skips**
/// rather than fails: a creator's machine has ffmpeg, a dev box may not,
/// and a missing encoder is not a defect in our frames.
fn ffmpeg_with_libx264() -> Option<String> {
    let which = Command::new("which").arg("ffmpeg").output().ok()?;
    if !which.status.success() {
        eprintln!("AC12 skipped: no ffmpeg on PATH");
        return None;
    }
    let path = String::from_utf8_lossy(&which.stdout).trim().to_string();
    let encoders = Command::new(&path)
        .args(["-hide_banner", "-encoders"])
        .output()
        .ok()?;
    if !String::from_utf8_lossy(&encoders.stdout).contains("libx264") {
        eprintln!("AC12 skipped: ffmpeg at {path} has no libx264");
        return None;
    }
    Some(path)
}

/// A face for the text primitive, as R-0007's suite finds one. Since R-0009
/// a `PpmSink` with no faces refuses text, so the one test that sets text
/// in PPM has to name a face. Panicking when none is found is deliberate.
#[cfg(feature = "text")]
fn a_face() -> motoreel_typeset::Fonts {
    const CANDIDATES: &[&str] = &[
        "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
        "/System/Library/Fonts/Helvetica.ttc",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
    ];
    for path in CANDIDATES {
        if let Ok(bytes) = fs::read(path) {
            if let Ok(face) = motoreel_typeset::Face::load(bytes, *path) {
                let mut fonts = motoreel_typeset::Fonts::new();
                fonts.add(face);
                return fonts;
            }
        }
    }
    panic!("no usable face found; tried {CANDIDATES:?}");
}

// --- AC1: the path shape ----------------------------------------------------

mod ac1 {
    use super::*;

    fn sample_subpath() -> Subpath<Pt2> {
        Subpath::new(p(0.0, 0.0)).line_to(p(1.0, 0.0)).cubic_to(
            p(1.0, 0.5),
            p(0.5, 1.0),
            p(0.0, 1.0),
        )
    }

    // AC1 — a subpath is plain data: a start, the segments in order, and
    // `closed` set only by `close()`. The builder and the literal agree.
    #[test]
    fn the_builder_produces_exactly_the_documented_data() {
        let open = sample_subpath();
        assert_eq!(
            open,
            Subpath {
                start: p(0.0, 0.0),
                segs: vec![
                    Seg::Line(p(1.0, 0.0)),
                    Seg::Cubic(p(1.0, 0.5), p(0.5, 1.0), p(0.0, 1.0)),
                ],
                closed: false,
            }
        );
        assert!(open.clone().close().closed, "close() sets the flag");
        assert_eq!(open.close().segs.len(), 2, "close() adds no segment");
    }

    // AC1 / AC3 support — `reversed()` walks the segments backwards and
    // swaps each cubic's handles, keeps `closed`, and is an involution.
    #[test]
    fn reversed_walks_backwards_swapping_handles() {
        let s = sample_subpath().close();
        let want = Subpath::new(p(0.0, 1.0))
            .cubic_to(p(0.5, 1.0), p(1.0, 0.5), p(1.0, 0.0))
            .line_to(p(0.0, 0.0))
            .close();
        assert_eq!(s.reversed(), want);
        assert_eq!(s.reversed().reversed(), s, "reversing twice is identity");
        assert!(
            !sample_subpath().reversed().closed,
            "an open subpath stays open"
        );
    }

    // AC1 — `map` is the one way a subpath changes point type (§2.1). It
    // must reach every control point, handles included, and keep the
    // structure.
    #[test]
    fn map_touches_every_control_point_and_keeps_the_structure() {
        let m: Subpath<(f64, f64)> = sample_subpath().close().map(|q| (2.0 * q.x, q.y + 1.0));
        assert_eq!(
            m,
            Subpath {
                start: (0.0, 1.0),
                segs: vec![
                    Seg::Line((2.0, 1.0)),
                    Seg::Cubic((2.0, 1.5), (1.0, 2.0), (0.0, 2.0)),
                ],
                closed: true,
            }
        );
    }

    // AC1 / §2.1 — `split_cubic` is de Casteljau halving at t = ½ with the
    // caller's midpoint. On dyadic input every average is exact, so the
    // halves are asserted with `==`. It is generic, so 3-tuples work too
    // (the pinhole path uses view-space tuples).
    #[test]
    fn split_cubic_is_de_casteljau_at_one_half() {
        let mid2 = |a: &(f64, f64), b: &(f64, f64)| ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
        let (l, r) = split_cubic([(0.0, 0.0), (0.0, 4.0), (4.0, 4.0), (4.0, 0.0)], mid2);
        assert_eq!(l, [(0.0, 0.0), (0.0, 2.0), (1.0, 3.0), (2.0, 3.0)]);
        assert_eq!(r, [(2.0, 3.0), (3.0, 3.0), (4.0, 2.0), (4.0, 0.0)]);

        let mid3 = |a: &(f64, f64, f64), b: &(f64, f64, f64)| {
            ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0, (a.2 + b.2) / 2.0)
        };
        let (l3, r3) = split_cubic(
            [
                (0.0, 0.0, -8.0),
                (8.0, 0.0, -8.0),
                (8.0, 8.0, -4.0),
                (0.0, 8.0, -4.0),
            ],
            mid3,
        );
        assert_eq!(l3[3], r3[0], "the halves meet at B(1/2)");
        assert_eq!(l3[3], (6.0, 4.0, -6.0), "B(1/2) of this cubic");
        assert_eq!((l3[0], r3[3]), ((0.0, 0.0, -8.0), (0.0, 8.0, -4.0)));
    }

    // AC1 — `Object::planar` lifts each (x, y) to `pga::Point::new(x, y, 0)`
    // into a `Shape::Path`. `Object::path` takes model points as they are.
    // Both start from the default style and an identity hold, like every
    // other constructor.
    #[test]
    fn planar_and_path_build_the_same_shape_path() {
        let sub = sample_subpath().close();
        let lifted = Subpath::new(pga::Point::new(0.0, 0.0, 0.0))
            .line_to(pga::Point::new(1.0, 0.0, 0.0))
            .cubic_to(
                pga::Point::new(1.0, 0.5, 0.0),
                pga::Point::new(0.5, 1.0, 0.0),
                pga::Point::new(0.0, 1.0, 0.0),
            )
            .close();
        let planar = Object::planar(vec![sub]);
        assert_eq!(planar.shape, Shape::Path(vec![lifted.clone()]));
        assert_eq!(planar.style, Style::default());
        assert_eq!(planar.track.eval(2.0), Motor3::identity());
        assert_eq!(Object::path(vec![lifted]).shape, planar.shape);
    }

    // AC1 — `Scene::eval` emits a `Prim2::Path`, posed by the object's track
    // like every other shape, with its style (fill included) carried
    // verbatim. Dyadic inputs under a translator are exact.
    #[test]
    fn eval_emits_a_posed_path_carrying_its_style_verbatim() {
        let style = paint(STEEL, 0.02, 0.5, solid(GOLD, 0.25));
        let second = Subpath::new(p(-1.0, -0.5)).line_to(p(-0.5, -0.5));
        let mut scene = Scene::new(1.0);
        scene.add(
            Object::planar(vec![sample_subpath().close(), second])
                .with_style(style)
                .at(Motor3::translator(0.5, 0.25, 0.0)),
        );
        let prims = scene.eval(0.0);
        assert_eq!(prims.len(), 1, "one object, one primitive");
        let want = vec![
            Subpath::new(p(0.5, 0.25))
                .line_to(p(1.5, 0.25))
                .cubic_to(p(1.5, 0.75), p(1.0, 1.25), p(0.5, 1.25))
                .close(),
            Subpath::new(p(-0.5, -0.25)).line_to(p(0.0, -0.25)),
        ];
        assert_eq!(
            prims[0],
            path_prim(want, style),
            "subpath order, segment kinds, `closed` and style all survive"
        );
    }

    // AC1 — `Prim2` has exactly the five old variants plus `Path`. A match
    // with no `_` arm compiles only if that is so; a seventh variant would
    // break it. The same holds for `Shape`.
    #[test]
    fn prim2_and_shape_gain_exactly_one_variant_each() {
        fn prim_kind(prim: &Prim2) -> &'static str {
            match prim {
                Prim2::Point { .. } => "point",
                Prim2::Segment { .. } => "segment",
                Prim2::Polyline { .. } => "polyline",
                Prim2::Edges { .. } => "edges",
                Prim2::Text { .. } => "text",
                Prim2::Path { .. } => "path",
            }
        }
        fn shape_kind(shape: &Shape) -> &'static str {
            match shape {
                Shape::Point(_) => "point",
                Shape::Segment(..) => "segment",
                Shape::Polyline(_) => "polyline",
                Shape::Edges(_) => "edges",
                Shape::Path(_) => "path",
            }
        }
        assert_eq!(prim_kind(&path_prim(Vec::new(), Style::default())), "path");
        assert_eq!(shape_kind(&Shape::Path(Vec::new())), "path");
    }

    // AC1 — both sinks: every `match` over `Prim2` names `Prim2::Path` and
    // has no catch-all arm (`_`, or a bare binding), so an eighth variant
    // stays a compile error. This is the grep SPEC-0012 §6 AC1 asks for;
    // compilation is its other half.
    #[test]
    fn every_prim2_match_in_both_sinks_is_exhaustive_by_name() {
        for file in ["svg.rs", "ppm.rs"] {
            let matches = match_arms_naming(&scrub(&src(file)), "Prim2::");
            assert!(!matches.is_empty(), "{file}: no match over Prim2 found");
            for arms in &matches {
                assert!(
                    arms.iter().any(|a| a.contains("Prim2::Path")),
                    "{file}: a Prim2 match does not name Prim2::Path: {arms:?}"
                );
                assert!(
                    !arms.iter().any(|a| is_catch_all(a)),
                    "{file}: a Prim2 match has a catch-all arm: {arms:?}"
                );
            }
        }
    }

    // AC1 — the eval side too: the `Shape` match in `scene.rs` names
    // `Shape::Path` and has no catch-all, so the path is projected on
    // purpose rather than falling through.
    #[test]
    fn the_shape_match_in_eval_is_exhaustive_by_name() {
        let matches = match_arms_naming(&scrub(&src("scene.rs")), "Shape::");
        assert!(!matches.is_empty(), "scene.rs: no match over Shape found");
        for arms in &matches {
            assert!(
                arms.iter().any(|a| a.contains("Shape::Path")),
                "scene.rs: a Shape match does not name Shape::Path: {arms:?}"
            );
            assert!(
                !arms.iter().any(|a| is_catch_all(a)),
                "scene.rs: a Shape match has a catch-all arm: {arms:?}"
            );
        }
    }
}

// --- AC2: fill paint --------------------------------------------------------

mod ac2 {
    use super::*;

    /// A 32 × 32 px square on pixel boundaries in the middle of `GRID`.
    fn square_px() -> Subpath<Pt2> {
        g_rect(16.0, 16.0, 48.0, 48.0)
    }

    /// A curved outline: lines and a cubic, closed.
    fn blob() -> Vec<Subpath<Pt2>> {
        vec![Subpath::new(p(-1.0, -1.0))
            .line_to(p(1.0, -1.0))
            .cubic_to(p(1.5, 0.0), p(1.0, 1.0), p(0.0, 1.25))
            .line_to(p(-1.0, 1.0))
            .close()]
    }

    // AC2 — fill only: every interior pixel is the fill colour, every
    // exterior pixel is background, and no stroke is drawn. The edges sit
    // on pixel boundaries, so the whole frame is asserted byte by byte.
    #[test]
    fn fill_only_paints_the_interior_and_no_stroke() {
        let f = render_ppm(
            "r0012_ac2_fill_only",
            GRID,
            GRID_VIEW,
            &[path_prim(vec![square_px()], filled(WHITE, 1.0))],
        );
        for y in 0..64 {
            for x in 0..64 {
                let inside = (16..48).contains(&x) && (16..48).contains(&y);
                let want = if inside { 255 } else { 0 };
                assert_eq!(f.at(x, y), [want; 3], "pixel ({x}, {y})");
            }
        }
    }

    // AC2 — stroke only: the boundary is lit and the interior is not.
    #[test]
    fn stroke_only_leaves_the_interior_unpainted() {
        // width 0.125 at s = 16 is r = 1 px on the x = 16 boundary.
        let f = render_ppm(
            "r0012_ac2_stroke_only",
            GRID,
            GRID_VIEW,
            &[path_prim(vec![square_px()], stroked(WHITE, 0.125, 1.0))],
        );
        assert_eq!(f.at(15, 32), [255; 3], "outside half of the stroke");
        assert_eq!(f.at(16, 32), [255; 3], "inside half of the stroke");
        assert_eq!(f.at(32, 32), [0; 3], "no fill without a fill");
        assert_eq!(f.at(14, 32), [0; 3], "nothing beyond the stroke");
    }

    // AC2 / SPEC-0012 §6 — with both, the fill composites first: a
    // white-filled, red-stroked square has red on its boundary pixels, not
    // white, and white in its interior.
    #[test]
    fn with_both_paints_the_stroke_lands_on_top_of_the_fill() {
        let f = render_ppm(
            "r0012_ac2_both",
            GRID,
            GRID_VIEW,
            &[path_prim(
                vec![square_px()],
                paint(RED, 0.125, 1.0, solid(WHITE, 1.0)),
            )],
        );
        assert_eq!(f.at(16, 32), rgb(RED), "boundary, inside: stroke on top");
        assert_eq!(f.at(15, 32), rgb(RED), "boundary, outside");
        assert_eq!(f.at(32, 32), rgb(WHITE), "interior: the fill");
        assert_eq!(f.at(14, 32), rgb(BLACK), "exterior");
    }

    // AC2 — "fill first, stroke second, each with its own opacity, each
    // exactly once" is one exact identity: a path with both paints renders
    // byte-identically to the same path drawn twice, fill-only and then
    // stroke-only. The opposite order must differ, or the test would not
    // be looking at the order at all.
    #[test]
    fn both_paints_are_exactly_fill_then_stroke() {
        let both = paint(STEEL, 0.25, 0.7, solid(GOLD, 0.6));
        let fill_part = paint(STEEL, 0.0, 0.7, solid(GOLD, 0.6));
        let stroke_part = paint(STEEL, 0.25, 0.7, None);
        let one = render_ppm(
            "r0012_ac2_order_one",
            GRID,
            GRID_VIEW,
            &[path_prim(blob(), both)],
        );
        let split = render_ppm(
            "r0012_ac2_order_split",
            GRID,
            GRID_VIEW,
            &[path_prim(blob(), fill_part), path_prim(blob(), stroke_part)],
        );
        let reversed = render_ppm(
            "r0012_ac2_order_reversed",
            GRID,
            GRID_VIEW,
            &[path_prim(blob(), stroke_part), path_prim(blob(), fill_part)],
        );
        assert!(
            one.pixels == split.pixels,
            "one path with both paints must equal fill-then-stroke"
        );
        assert!(
            one.pixels != reversed.pixels,
            "stroke-then-fill must differ, or the order is not being tested"
        );
    }

    // AC2 — exactly one composite per pixel per paint, even where two
    // subpaths of one path overlap (winding 2) and where one subpath
    // retraces itself. White at alpha 0.5 over black is 128 once; twice it
    // would be 191.
    #[test]
    fn each_paint_composites_once_per_pixel_where_geometry_overlaps() {
        let overlap = vec![g_rect(8.0, 8.0, 40.0, 40.0), g_rect(24.0, 24.0, 56.0, 56.0)];
        let f = render_ppm(
            "r0012_ac2_once_fill",
            GRID,
            GRID_VIEW,
            &[path_prim(overlap, filled(WHITE, 0.5))],
        );
        assert_eq!(f.grey(32, 32), 128, "overlap: composited once");
        assert_eq!(f.grey(12, 12), 128, "single cover");

        // An open subpath out and back along the same line, stroked at
        // r = 1 px: the chords overlap exactly.
        let retrace = vec![Subpath::new(img(GRID, GS, 8.0, 32.0))
            .line_to(img(GRID, GS, 56.0, 32.0))
            .line_to(img(GRID, GS, 8.0, 32.0))];
        let g = render_ppm(
            "r0012_ac2_once_stroke",
            GRID,
            GRID_VIEW,
            &[path_prim(retrace, stroked(WHITE, 0.125, 0.5))],
        );
        assert_eq!(g.grey(32, 31), 128, "retraced stroke: composited once");
        assert_eq!(g.grey(32, 32), 128, "retraced stroke: composited once");
    }

    // AC2 — an object with neither paint paints nothing: no fill with a
    // zero width or a zero alpha, and a fill whose own alpha is 0, NaN or
    // negative (SPEC-0012 §2.2: read through the total `unit` clamp).
    #[test]
    fn neither_paint_leaves_pure_background() {
        let cases = [
            ("no fill, width 0", paint(WHITE, 0.0, 1.0, None)),
            ("no fill, alpha 0", paint(WHITE, 0.25, 0.0, None)),
            ("fill alpha 0", filled(WHITE, 0.0)),
            ("fill alpha NaN", filled(WHITE, f64::NAN)),
            ("fill alpha negative", filled(WHITE, -0.5)),
        ];
        for (i, (what, style)) in cases.into_iter().enumerate() {
            let f = render_ppm(
                &format!("r0012_ac2_neither_{i}"),
                GRID,
                GRID_VIEW,
                &[path_prim(blob(), style)],
            );
            assert!(f.is_pure([0, 0, 0]), "{what} must paint nothing");
        }
    }

    // AC2 / §2.2 — the fill alpha is read through `unit`: above 1 is 1, and
    // a fill that paints nothing leaves the stroke exactly as a stroke-only
    // path draws it.
    #[test]
    fn fill_alpha_is_clamped_and_a_dead_fill_leaves_the_stroke_alone() {
        let over = render_ppm(
            "r0012_ac2_alpha_over",
            GRID,
            GRID_VIEW,
            &[path_prim(blob(), filled(WHITE, 1.5))],
        );
        let one = render_ppm(
            "r0012_ac2_alpha_one",
            GRID,
            GRID_VIEW,
            &[path_prim(blob(), filled(WHITE, 1.0))],
        );
        assert!(over.pixels == one.pixels, "alpha 1.5 must render as 1");

        let stroke_only = render_ppm(
            "r0012_ac2_dead_fill_ref",
            GRID,
            GRID_VIEW,
            &[path_prim(blob(), stroked(STEEL, 0.25, 0.7))],
        );
        for (i, alpha) in [f64::NAN, 0.0].into_iter().enumerate() {
            let f = render_ppm(
                &format!("r0012_ac2_dead_fill_{i}"),
                GRID,
                GRID_VIEW,
                &[path_prim(
                    blob(),
                    paint(STEEL, 0.25, 0.7, solid(GOLD, alpha)),
                )],
            );
            assert!(
                f.pixels == stroke_only.pixels,
                "fill alpha {alpha} must leave exactly the stroke-only frame"
            );
        }
    }

    // AC2 / §2.2 — only `Path` honours `fill`. On `Point`, `Segment`,
    // `Polyline`, `Edges` and `Text`, a `Some` fill leaves the bytes of
    // both sinks unchanged.
    #[test]
    fn some_fill_is_ignored_by_every_other_variant_in_both_sinks() {
        let with = |fill: Option<Fill>| -> Vec<(&'static str, Prim2)> {
            let style = paint(STEEL, 0.2, 0.8, fill);
            vec![
                (
                    "point",
                    Prim2::Point {
                        at: p(0.25, 0.5),
                        style,
                    },
                ),
                (
                    "segment",
                    Prim2::Segment {
                        a: p(-1.0, 0.0),
                        b: p(1.0, 0.5),
                        style,
                    },
                ),
                (
                    "polyline",
                    Prim2::Polyline {
                        points: vec![p(-1.0, -1.0), p(1.0, -1.0), p(0.0, 1.0), p(-1.0, -1.0)],
                        style,
                    },
                ),
                (
                    "edges",
                    Prim2::Edges {
                        segments: vec![(p(-1.0, -1.0), p(1.0, 1.0)), (p(-1.0, 1.0), p(1.0, -1.0))],
                        style,
                    },
                ),
                (
                    "text",
                    Prim2::Text {
                        at: p(0.0, 0.0),
                        text: "Ab".to_string(),
                        size: 0.5,
                        align: Align::Center,
                        face: 0,
                        style,
                    },
                ),
            ]
        };
        let plain = with(None);
        let filled = with(solid(RED, 1.0));
        for ((name, a), (_, b)) in plain.iter().zip(&filled) {
            let svg_a = render_svg(
                &format!("r0012_ac2_ignore_svg_a_{name}"),
                GRID,
                GRID_VIEW,
                std::slice::from_ref(a),
            );
            let svg_b = render_svg(
                &format!("r0012_ac2_ignore_svg_b_{name}"),
                GRID,
                GRID_VIEW,
                std::slice::from_ref(b),
            );
            assert_eq!(svg_a, svg_b, "{name}: SVG bytes must ignore the fill");
            if *name == "text" {
                continue; // the raster sink needs a face; see below
            }
            let ppm_a = render_ppm(
                &format!("r0012_ac2_ignore_ppm_a_{name}"),
                GRID,
                GRID_VIEW,
                std::slice::from_ref(a),
            );
            let ppm_b = render_ppm(
                &format!("r0012_ac2_ignore_ppm_b_{name}"),
                GRID,
                GRID_VIEW,
                std::slice::from_ref(b),
            );
            assert!(
                ppm_a.pixels == ppm_b.pixels,
                "{name}: PPM bytes must ignore the fill"
            );
        }

        #[cfg(feature = "text")]
        {
            let text = |i: usize| -> Vec<u8> {
                let dir = tmp_dir(&format!("r0012_ac2_ignore_ppm_text_{i}"));
                let mut sink = PpmSink::with_view(&dir, GRID, GRID_VIEW)
                    .expect("ppm sink")
                    .with_fonts(a_face());
                let prim = if i == 0 { &plain[4].1 } else { &filled[4].1 };
                sink.frame(0, std::slice::from_ref(prim))
                    .expect("text frame");
                fs::read(dir.join("frame_00000.ppm")).expect("read text frame")
            };
            assert!(text(0) == text(1), "text: PPM bytes must ignore the fill");
        }
    }

    // AC2 / §2.2 — `Fill` and `Style` are plain `Copy` data, `Fill::solid`
    // is the `const` constructor, and the default style gains `fill: None`
    // while keeping R-0002's adjudicated stroke.
    #[test]
    fn fill_is_plain_copy_data_and_the_default_is_none() {
        const HALF_WHITE: Fill = Fill::solid(Rgb::WHITE, 0.5);
        assert_eq!(
            HALF_WHITE,
            Fill {
                colour: Rgb::WHITE,
                alpha: 0.5
            }
        );
        let d = Style::default();
        assert_eq!(d.fill, None, "today's behaviour: no fill");
        assert_eq!((d.stroke, d.width, d.alpha), (Rgb::WHITE, 0.01, 1.0));
        fn copy<T: Copy>(t: T) -> (T, T) {
            (t, t)
        }
        let (a, b) = copy(paint(WHITE, 0.1, 1.0, Some(HALF_WHITE)));
        assert_eq!(a, b);
    }
}

// --- AC3: nonzero winding, matching SVG -------------------------------------

mod ac3 {
    use super::*;

    const STAR: (u32, u32) = (200, 200);
    const STAR_VIEW: (f64, f64) = (2.0, 2.0);
    const STAR_S: f64 = 100.0;

    /// The five tips of a pentagram of radius 0.9, in drawing order: each
    /// step turns 2/5 of a revolution, so the one closed subpath crosses
    /// itself five times.
    fn tips() -> [Pt2; 5] {
        std::array::from_fn(|k| {
            let a = TAU / 4.0 + k as f64 * (2.0 * TAU / 5.0);
            p(0.9 * a.cos(), 0.9 * a.sin())
        })
    }

    fn pentagram() -> Vec<Subpath<Pt2>> {
        let t = tips();
        vec![Subpath::new(t[0])
            .line_to(t[1])
            .line_to(t[2])
            .line_to(t[3])
            .line_to(t[4])
            .close()]
    }

    fn intersect(a: (f64, f64), b: (f64, f64), c: (f64, f64), d: (f64, f64)) -> (f64, f64) {
        let (rx, ry, sx, sy) = (b.0 - a.0, b.1 - a.1, d.0 - c.0, d.1 - c.1);
        let t = ((c.0 - a.0) * sy - (c.1 - a.1) * sx) / (rx * sy - ry * sx);
        (a.0 + t * rx, a.1 + t * ry)
    }

    fn inside_polygon(c: (f64, f64), poly: &[(f64, f64)]) -> bool {
        let mut inside = false;
        for (i, &a) in poly.iter().enumerate() {
            let b = poly[(i + 1) % poly.len()];
            if (a.1 > c.1) != (b.1 > c.1) && c.0 < a.0 + (c.1 - a.1) * (b.0 - a.0) / (b.1 - a.1) {
                inside = !inside;
            }
        }
        inside
    }

    // AC3 / SPEC-0012 §6 — the pentagram's interior is the fill colour, byte
    // for byte, with no seam along the chords that cross it. "Interior" is
    // the spec's definition: at least 0.5 px from the 10-vertex outer
    // outline. That is not "winding number 2", because pixels near the
    // concave inner vertices rightly take the ramp. Pixels at least 0.5 px
    // outside the outline are background.
    #[test]
    fn a_pentagram_fills_its_interior_with_no_seam() {
        let f = render_ppm(
            "r0012_ac3_pentagram",
            STAR,
            STAR_VIEW,
            &[path_prim(pentagram(), filled(SIGNAL, 1.0))],
        );

        // Tips in angle order: tip k sits at 90° + 144°·k, so the tip at
        // 90° + 72°·j is tip (3j mod 5).
        let t = tips().map(|q| to_px(STAR, STAR_S, q));
        let around: [(f64, f64); 5] = std::array::from_fn(|j| t[(3 * j) % 5]);
        let mut outline = Vec::new();
        for j in 0..5 {
            // The inner vertex between tips j and j+1 is where the chord
            // from tip j-1 to tip j+1 crosses the chord from tip j to j+2.
            let inner = intersect(
                around[(j + 4) % 5],
                around[(j + 1) % 5],
                around[j],
                around[(j + 2) % 5],
            );
            outline.push(around[j]);
            outline.push(inner);
        }
        let chords: Vec<((f64, f64), (f64, f64))> =
            (0..5).map(|k| (t[k], t[(k + 1) % 5])).collect();

        let mut near_a_chord = 0;
        for y in 0..STAR.1 {
            for x in 0..STAR.0 {
                let c = (f64::from(x) + 0.5, f64::from(y) + 0.5);
                let to_outline = (0..10)
                    .map(|i| dist_to_segment(c, outline[i], outline[(i + 1) % 10]))
                    .fold(f64::INFINITY, f64::min);
                if to_outline < 0.5 {
                    continue; // the ramp's territory
                }
                if inside_polygon(c, &outline) {
                    assert_eq!(f.at(x, y), rgb(SIGNAL), "interior pixel ({x}, {y})");
                    if chords.iter().any(|&(a, b)| dist_to_segment(c, a, b) < 0.5) {
                        near_a_chord += 1;
                    }
                } else {
                    assert_eq!(f.at(x, y), rgb(BLACK), "exterior pixel ({x}, {y})");
                }
            }
        }
        assert!(
            near_a_chord > 100,
            "the seam check is vacuous unless interior pixels lie on the \
             internal chords; found {near_a_chord}"
        );
    }

    // AC3 — the same pentagram in SVG is one `<path>` whose data runs
    // through the five tips in drawing order and closes, with
    // `fill-rule="nonzero"`.
    #[test]
    fn the_pentagram_svg_is_one_nonzero_path_through_the_tips() {
        let star = pentagram();
        let svg = render_svg(
            "r0012_ac3_pentagram_svg",
            STAR,
            STAR_VIEW,
            &[path_prim(star.clone(), filled(SIGNAL, 1.0))],
        );
        assert_eq!(
            path_lines(&svg),
            [format!(
                "<path d=\"{}\" fill=\"#e0322a\" fill-opacity=\"1\" \
                 fill-rule=\"nonzero\" stroke=\"none\"/>",
                d_of(&star)
            )]
        );
    }

    /// The outer square, on pixel boundaries.
    fn outer() -> Subpath<Pt2> {
        g_rect(8.0, 8.0, 56.0, 56.0)
    }

    /// The inner square. Its edges run through pixel centres, so a seam
    /// would show as a 128 there rather than hide in a fringe.
    fn inner() -> Subpath<Pt2> {
        g_rect(20.5, 20.5, 43.5, 43.5)
    }

    // AC3 — a square inside a square with the same orientation fills solid.
    // Every pixel of the outer square is exactly 255, including the ones
    // the inner edges pass through.
    #[test]
    fn nested_squares_with_the_same_orientation_fill_solid() {
        let subs = vec![outer(), inner()];
        let f = render_ppm(
            "r0012_ac3_nested_same",
            GRID,
            GRID_VIEW,
            &[path_prim(subs.clone(), filled(WHITE, 1.0))],
        );
        for y in 0..64 {
            for x in 0..64 {
                let inside = (8..56).contains(&x) && (8..56).contains(&y);
                assert_eq!(f.grey(x, y), if inside { 255 } else { 0 }, "({x}, {y})");
            }
        }
        let svg = render_svg(
            "r0012_ac3_nested_same_svg",
            GRID,
            GRID_VIEW,
            &[path_prim(subs.clone(), filled(WHITE, 1.0))],
        );
        let line = path_lines(&svg)[0];
        assert!(line.contains(&format!("d=\"{}\"", d_of(&subs))), "{line}");
        assert!(line.contains("fill-rule=\"nonzero\""), "{line}");
    }

    // AC3 — with the opposite orientation the inner square is a hole: its
    // inside is background, its edges take the ramp at exactly 128, and the
    // ring is solid. The four inner-corner pixels are centred exactly on
    // vertices, where §2.6 pins no single value, so they are not asserted.
    #[test]
    fn nested_squares_with_opposite_orientation_leave_a_hole() {
        let subs = vec![outer(), inner().reversed()];
        let f = render_ppm(
            "r0012_ac3_nested_hole",
            GRID,
            GRID_VIEW,
            &[path_prim(subs.clone(), filled(WHITE, 1.0))],
        );
        for y in 0..64u32 {
            for x in 0..64u32 {
                let on_x = x == 20 || x == 43;
                let on_y = y == 20 || y == 43;
                let want = if !((8..56).contains(&x) && (8..56).contains(&y)) {
                    Some(0)
                } else if on_x && on_y {
                    None // a vertex pixel
                } else if (on_x && (21..43).contains(&y)) || (on_y && (21..43).contains(&x)) {
                    Some(128)
                } else if (21..43).contains(&x) && (21..43).contains(&y) {
                    Some(0)
                } else {
                    Some(255)
                };
                if let Some(want) = want {
                    assert_eq!(f.grey(x, y), want, "({x}, {y})");
                }
            }
        }
        let svg = render_svg(
            "r0012_ac3_nested_hole_svg",
            GRID,
            GRID_VIEW,
            &[path_prim(subs.clone(), filled(WHITE, 1.0))],
        );
        let line = path_lines(&svg)[0];
        assert!(
            line.contains(&format!("d=\"{}\"", d_of(&subs))),
            "the hole's reversed order must reach the SVG: {line}"
        );
        assert!(line.contains("fill-rule=\"nonzero\""), "{line}");
    }

    // AC3 / AC7 — the generators' orientation is what makes `reversed()` a
    // hole: a reversed circle inside a circle leaves background at the
    // centre and ink in the ring.
    #[test]
    fn a_reversed_generator_inside_a_generator_is_a_hole() {
        let mut ring = circle(p(0.0, 0.0), 1.5);
        ring.extend(circle(p(0.0, 0.0), 0.75).iter().map(Subpath::reversed));
        let f = render_ppm(
            "r0012_ac3_ring",
            GRID,
            GRID_VIEW,
            &[path_prim(ring, filled(WHITE, 1.0))],
        );
        assert_eq!(f.grey(32, 32), 0, "the hole");
        assert_eq!(f.grey(32, 12), 255, "the ring: 1.125 units above centre");
        assert_eq!(f.grey(2, 2), 0, "outside");
    }

    // AC3 — an open subpath fills as though closed by a straight segment,
    // which is SVG's behaviour. Fill-only frames of the open and the closed
    // form are byte-identical. In SVG the open form has no `Z` and still
    // carries its fill.
    #[test]
    fn an_open_subpath_fills_exactly_as_if_closed() {
        let open = Subpath::new(img(GRID, GS, 8.0, 56.0))
            .line_to(img(GRID, GS, 56.0, 56.0))
            .cubic_to(
                img(GRID, GS, 60.0, 30.0),
                img(GRID, GS, 40.0, 4.0),
                img(GRID, GS, 20.0, 12.0),
            )
            .line_to(img(GRID, GS, 8.0, 8.0));
        let closed = open.clone().close();
        let a = render_ppm(
            "r0012_ac3_open",
            GRID,
            GRID_VIEW,
            &[path_prim(vec![open.clone()], filled(WHITE, 1.0))],
        );
        let b = render_ppm(
            "r0012_ac3_closed",
            GRID,
            GRID_VIEW,
            &[path_prim(vec![closed], filled(WHITE, 1.0))],
        );
        assert!(
            a.pixels == b.pixels,
            "open and closed must fill identically"
        );
        assert_eq!(a.grey(32, 32), 255, "and the fill is really there");

        let svg = render_svg(
            "r0012_ac3_open_svg",
            GRID,
            GRID_VIEW,
            &[path_prim(vec![open.clone()], filled(WHITE, 1.0))],
        );
        assert_eq!(
            path_lines(&svg),
            [format!(
                "<path d=\"{}\" fill=\"#ffffff\" fill-opacity=\"1\" \
                 fill-rule=\"nonzero\" stroke=\"none\"/>",
                d_of(&[open])
            )]
        );
        assert!(!svg.contains(" Z"), "an open subpath writes no Z");
    }

    // AC3 / §2.7 — the implicit close is for the fill only. An open subpath
    // strokes no closing segment; the same subpath closed does.
    #[test]
    fn an_open_subpath_strokes_no_closing_segment() {
        let u = Subpath::new(img(GRID, GS, 8.0, 56.0))
            .line_to(img(GRID, GS, 56.0, 56.0))
            .line_to(img(GRID, GS, 56.0, 8.0))
            .line_to(img(GRID, GS, 8.0, 8.0));
        let style = stroked(WHITE, 0.125, 1.0);
        let open = render_ppm(
            "r0012_ac3_open_stroke",
            GRID,
            GRID_VIEW,
            &[path_prim(vec![u.clone()], style)],
        );
        let closed = render_ppm(
            "r0012_ac3_closed_stroke",
            GRID,
            GRID_VIEW,
            &[path_prim(vec![u.close()], style)],
        );
        assert_eq!(open.grey(8, 32), 0, "the missing side is not stroked");
        assert_eq!(open.grey(7, 32), 0, "the missing side is not stroked");
        assert_eq!(closed.grey(8, 32), 255, "the closing side is stroked");
        assert_eq!(closed.grey(7, 32), 255, "the closing side is stroked");
    }

    // AC3 / §2.6 — winding and coverage do not depend on edge order:
    // reordering subpaths, and reversing a lines-only subpath (exact in
    // floating point), leave fill and stroke bytes unchanged.
    #[test]
    fn edge_order_and_single_loop_orientation_change_nothing() {
        let a = g_rect(6.0, 6.0, 30.0, 26.5);
        let b = Subpath::new(img(GRID, GS, 36.25, 40.0))
            .line_to(img(GRID, GS, 58.0, 44.5))
            .line_to(img(GRID, GS, 40.0, 60.0))
            .close();
        let style = paint(STEEL, 0.1875, 0.75, solid(GOLD, 0.8));
        let base = render_ppm(
            "r0012_ac3_order_base",
            GRID,
            GRID_VIEW,
            &[path_prim(vec![a.clone(), b.clone()], style)],
        );
        let swapped = render_ppm(
            "r0012_ac3_order_swapped",
            GRID,
            GRID_VIEW,
            &[path_prim(vec![b.clone(), a.clone()], style)],
        );
        let flipped = render_ppm(
            "r0012_ac3_order_flipped",
            GRID,
            GRID_VIEW,
            &[path_prim(vec![a.reversed(), b.reversed()], style)],
        );
        assert!(
            base.pixels == swapped.pixels,
            "subpath order must not matter"
        );
        assert!(
            base.pixels == flipped.pixels,
            "a lone loop's orientation must not matter"
        );
    }

    // AC3 / §2.6 — zero-length edges add nothing: a repeated vertex leaves
    // fill and stroke bytes unchanged.
    #[test]
    fn zero_length_edges_change_nothing() {
        let (q0, q1, q2) = (
            img(GRID, GS, 10.25, 50.0),
            img(GRID, GS, 54.0, 44.75),
            img(GRID, GS, 30.5, 9.5),
        );
        let style = paint(STEEL, 0.1875, 0.75, solid(GOLD, 0.8));
        let plain = Subpath::new(q0).line_to(q1).line_to(q2).close();
        let doubled = Subpath::new(q0)
            .line_to(q0)
            .line_to(q1)
            .line_to(q1)
            .line_to(q2)
            .close();
        let a = render_ppm(
            "r0012_ac3_zero_len_a",
            GRID,
            GRID_VIEW,
            &[path_prim(vec![plain], style)],
        );
        let b = render_ppm(
            "r0012_ac3_zero_len_b",
            GRID,
            GRID_VIEW,
            &[path_prim(vec![doubled], style)],
        );
        assert!(a.pixels == b.pixels, "zero-length edges must add nothing");
    }

    // AC3 / §2.6 — the crossing rule is half-open, so a vertex lying exactly
    // on a pixel-centre row is counted once. The triangle's left vertex is
    // the only vertex on its row: one edge ends there and the next begins.
    // A closed-interval rule counts both, and nothing on that row cancels
    // the extra crossing, so the pixel 0.5 px left of the vertex would light
    // up. (A diamond would not show it: its right vertex, on the same row,
    // double-counts in the opposite sense.) Every pixel at least 0.5 px from
    // the outline is exactly 255 inside and 0 outside.
    #[test]
    fn a_vertex_on_a_pixel_centre_row_is_counted_once() {
        let v = [(10.0, 32.5), (44.0, 50.25), (40.0, 10.25)];
        let triangle = Subpath::new(img(GRID, GS, v[0].0, v[0].1))
            .line_to(img(GRID, GS, v[1].0, v[1].1))
            .line_to(img(GRID, GS, v[2].0, v[2].1))
            .close();
        let f = render_ppm(
            "r0012_ac3_vertex_row",
            GRID,
            GRID_VIEW,
            &[path_prim(vec![triangle], filled(WHITE, 1.0))],
        );
        for y in 0..64 {
            for x in 0..64 {
                let c = (f64::from(x) + 0.5, f64::from(y) + 0.5);
                let d = (0..3)
                    .map(|i| dist_to_segment(c, v[i], v[(i + 1) % 3]))
                    .fold(f64::INFINITY, f64::min);
                if d < 0.5 {
                    continue;
                }
                let want = if inside_polygon(c, &v) { 255 } else { 0 };
                assert_eq!(f.grey(x, y), want, "pixel ({x}, {y})");
            }
        }
        assert_eq!(f.grey(9, 32), 0, "0.5 px left of the vertex, on its row");
        assert_eq!(f.grey(30, 32), 255, "inside, on the same row");
    }

    // AC3 — a subpath that encloses no area fills nothing. A horizontal
    // line out and back is the exact case: it has no crossings at all.
    #[test]
    fn a_zero_area_subpath_fills_nothing() {
        let line = Subpath::new(img(GRID, GS, 8.0, 32.0))
            .line_to(img(GRID, GS, 56.0, 32.0))
            .close();
        let f = render_ppm(
            "r0012_ac3_zero_area",
            GRID,
            GRID_VIEW,
            &[path_prim(vec![line], filled(WHITE, 1.0))],
        );
        assert!(f.is_pure([0, 0, 0]), "no area, no fill");
    }

    // AC3 — disjoint subpaths of one path each fill.
    #[test]
    fn disjoint_subpaths_in_one_path_each_fill() {
        let f = render_ppm(
            "r0012_ac3_disjoint",
            GRID,
            GRID_VIEW,
            &[path_prim(
                vec![g_rect(4.0, 4.0, 20.0, 20.0), g_rect(40.0, 40.0, 60.0, 60.0)],
                filled(WHITE, 1.0),
            )],
        );
        assert_eq!(f.grey(12, 12), 255);
        assert_eq!(f.grey(50, 50), 255);
        assert_eq!(f.grey(30, 30), 0, "nothing between them");
    }
}

// --- AC4: geometry agreement, the pinhole case, the cull rule ---------------

mod ac4 {
    use super::*;

    /// A model-space path with lines and cubics, off the z = 0 plane.
    fn model_path() -> Vec<Subpath<pga::Point>> {
        vec![
            Subpath::new(pga::Point::new(-0.7, -0.3, 0.1))
                .line_to(pga::Point::new(0.4, -0.35, -0.2))
                .cubic_to(
                    pga::Point::new(0.9, 0.1, 0.3),
                    pga::Point::new(0.2, 0.8, -0.1),
                    pga::Point::new(-0.3, 0.6, 0.25),
                )
                .close(),
            Subpath::new(pga::Point::new(0.5, -0.6, 0.0)).cubic_to(
                pga::Point::new(0.8, -0.9, 0.2),
                pga::Point::new(1.1, -0.2, -0.3),
                pga::Point::new(1.3, -0.5, 0.1),
            ),
        ]
    }

    fn the_path(prims: &[Prim2]) -> &[Subpath<Pt2>] {
        match prims {
            [Prim2::Path { subpaths, .. }] => subpaths,
            other => panic!("expected exactly one Prim2::Path, got {other:?}"),
        }
    }

    // AC4 / SPEC-0012 §6 — orthographic: every emitted control point is the
    // projected model control point, bit for bit, and there is exactly one
    // emitted cubic per authored cubic. The expected points go through the
    // same public steps eval takes (§2.4): `to_view`, then
    // `Projection::project`.
    #[test]
    fn orthographic_control_points_are_the_projected_model_points_bit_for_bit() {
        let mut scene = Scene::new(1.0);
        scene.camera = Camera::orthographic(
            Motor3::translator(0.1, -0.2, 5.0).compose(&Motor3::rotor(0.3, Pga3::basis(0b0011))),
        );
        let pose =
            Motor3::translator(0.3, -0.1, 0.5).compose(&Motor3::rotor(0.7, Pga3::basis(0b0110)));
        scene.add(Object::path(model_path()).at(pose));

        let prims = scene.eval(0.0);
        let got = the_path(&prims);
        let to_view = scene
            .camera
            .pose
            .inverse()
            .compose(&scene.objects[0].track.eval(0.0));
        let project = |q: &pga::Point| {
            scene
                .camera
                .projection
                .project(&q.transform(&to_view))
                .expect("in front of the camera")
        };
        let want: Vec<Subpath<Pt2>> = model_path().iter().map(|s| s.map(project)).collect();
        assert_eq!(got.len(), want.len(), "one image subpath per model subpath");
        for (g, w) in got.iter().zip(&want) {
            let bits = |s: &Subpath<Pt2>| -> Vec<(u64, u64)> {
                sample(s, 0)
                    .iter()
                    .map(|q| (q.x.to_bits(), q.y.to_bits()))
                    .collect()
            };
            assert_eq!(
                g.segs.len(),
                w.segs.len(),
                "no subdivision under an affine camera"
            );
            assert_eq!(g.closed, w.closed);
            for (a, b) in g.segs.iter().zip(&w.segs) {
                assert_eq!(
                    std::mem::discriminant(a),
                    std::mem::discriminant(b),
                    "a line stays a line and a cubic a cubic"
                );
                if let (Seg::Cubic(a1, a2, _), Seg::Cubic(b1, b2, _)) = (a, b) {
                    assert_eq!(
                        (
                            a1.x.to_bits(),
                            a1.y.to_bits(),
                            a2.x.to_bits(),
                            a2.y.to_bits()
                        ),
                        (
                            b1.x.to_bits(),
                            b1.y.to_bits(),
                            b2.x.to_bits(),
                            b2.y.to_bits()
                        ),
                        "handles, bit for bit"
                    );
                }
            }
            assert_eq!(bits(g), bits(w), "on-curve points, bit for bit");
        }
    }

    // AC4 / §2.4 — under the pinhole camera a `Seg::Line` stays one line
    // whose end points are the projected model points exactly: a
    // projective map sends lines to lines.
    #[test]
    fn pinhole_lines_project_their_end_points_exactly() {
        let mut scene = Scene::new(1.0);
        scene.camera = Camera::pinhole(Motor3::translator(0.0, 0.0, 3.0), 1.5);
        let model = vec![Subpath::new(pga::Point::new(-0.5, -0.4, 0.3))
            .line_to(pga::Point::new(0.6, -0.2, -0.4))
            .line_to(pga::Point::new(0.1, 0.7, 0.2))
            .close()];
        scene.add(Object::path(model.clone()));
        let prims = scene.eval(0.0);
        let to_view = scene
            .camera
            .pose
            .inverse()
            .compose(&scene.objects[0].track.eval(0.0));
        let want: Vec<Subpath<Pt2>> = model
            .iter()
            .map(|s| {
                s.map(|q| {
                    scene
                        .camera
                        .projection
                        .project(&q.transform(&to_view))
                        .expect("in front")
                })
            })
            .collect();
        assert_eq!(the_path(&prims), &want[..]);
    }

    /// The true perspective image of planar model subpaths, sampled at
    /// `per_cubic` points per cubic, as one polyline per subpath: every
    /// sample is evaluated in model space and then projected.
    fn true_image(scene: &Scene, model: &[Subpath<Pt2>], per_cubic: usize) -> Vec<Vec<Pt2>> {
        let to_view = scene
            .camera
            .pose
            .inverse()
            .compose(&scene.objects[0].track.eval(0.0));
        let project = |q: Pt2| {
            scene
                .camera
                .projection
                .project(&pga::Point::new(q.x, q.y, 0.0).transform(&to_view))
                .expect("in front of the camera")
        };
        model
            .iter()
            .map(|s| sample(s, per_cubic).into_iter().map(project).collect())
            .collect()
    }

    fn tuples(lines: &[Vec<Pt2>]) -> Vec<Vec<(f64, f64)>> {
        lines
            .iter()
            .map(|l| l.iter().map(|q| (q.x, q.y)).collect())
            .collect()
    }

    /// A unit circle tilted 60° about x, seen by a pinhole camera `depth`
    /// back: strongly foreshortened, nearest depth `depth − 0.866`.
    fn tilted_circle_scene(depth: f64, style: Style) -> (Scene, Vec<Subpath<Pt2>>) {
        let model = circle(p(0.0, 0.0), 1.0);
        let mut scene = Scene::new(1.0);
        scene.camera = Camera::pinhole(Motor3::translator(0.0, 0.0, depth), 1.5);
        scene.add(
            Object::planar(model.clone())
                .with_style(style)
                .at(Motor3::rotor(TAU / 6.0, Pga3::basis(0b0110))),
        );
        (scene, model)
    }

    // AC4 / SPEC-0012 §6 — pinhole: the geometric nearest-point distance
    // between the true projected curve (10³ samples per cubic) and the
    // emitted pieces is ≤ τ in BOTH directions, where
    // τ = 5·10⁻⁵ · min(view) = 9·10⁻⁵ image units. The same-parameter
    // distance is deliberately not used (§2.4). The two cases put the
    // nearest point at depth 1.63 and at depth 0.63.
    #[test]
    fn pinhole_pieces_lie_within_tau_of_the_true_perspective_image() {
        let tau = 5e-5 * 3.2_f64.min(1.8);
        for depth in [2.5, 1.5] {
            let (scene, model) = tilted_circle_scene(depth, Style::default());
            let prims = scene.eval(0.0);
            let emitted: Vec<Vec<Pt2>> = the_path(&prims).iter().map(|s| sample(s, 64)).collect();
            let truth = true_image(&scene, &model, 1000);

            let to_truth = Nearest::new(&tuples(&truth), 0.01);
            let to_emitted = Nearest::new(&tuples(&emitted), 0.01);
            let worst_out = emitted
                .iter()
                .flatten()
                .map(|q| to_truth.distance((q.x, q.y)))
                .fold(0.0, f64::max);
            let worst_in = truth
                .iter()
                .flatten()
                .map(|q| to_emitted.distance((q.x, q.y)))
                .fold(0.0, f64::max);
            assert!(
                worst_out <= tau,
                "depth {depth}: an emitted point lies {worst_out:e} from the true image (τ = {tau:e})"
            );
            assert!(
                worst_in <= tau,
                "depth {depth}: a true point lies {worst_in:e} from the emitted pieces (τ = {tau:e})"
            );
        }
    }

    // AC4 — pinhole through the raster sink: the rendered outline lies
    // within the compound bound τ + 0.1 px of the true perspective image
    // (§2.4). It is read from the pixels themselves. A stroke of radius r
    // has coverage `r + 0.5 − d` on its fringe, so every fractional pixel
    // reports its distance `d` to the rendered polyline within 1/510 (the
    // byte rounding). That distance must match the distance to the true
    // curve within τ + 0.1 px, plus 10⁻³ px for sampling the truth.
    #[test]
    fn pinhole_ppm_outline_lies_within_tau_plus_a_tenth_of_a_pixel() {
        let (size, s, r) = ((320, 180), 100.0, 1.0);
        let (scene, model) = tilted_circle_scene(2.5, stroked(WHITE, 2.0 * r / s, 1.0));
        let f = render_ppm("r0012_ac4_pinhole_ppm", size, (3.2, 1.8), &scene.eval(0.0));
        let truth: Vec<Vec<(f64, f64)>> = true_image(&scene, &model, 1000)
            .iter()
            .map(|l| l.iter().map(|&q| to_px(size, s, q)).collect())
            .collect();
        let near = Nearest::new(&truth, 4.0);
        let bound = 5e-5 * 1.8 * s + 0.1 + 1.0 / 510.0 + 1e-3;
        let mut fractional = 0;
        for y in 0..size.1 {
            for x in 0..size.0 {
                let b = f.grey(x, y);
                if b == 0 || b == 255 {
                    continue;
                }
                fractional += 1;
                let d_rendered = r + 0.5 - f64::from(b) / 255.0;
                let d_true = near.distance((f64::from(x) + 0.5, f64::from(y) + 0.5));
                assert!(
                    (d_rendered - d_true).abs() <= bound,
                    "pixel ({x}, {y}): rendered distance {d_rendered}, true {d_true}"
                );
            }
        }
        assert!(fractional > 100, "the fringe must be there to be measured");
    }

    /// A scene with one reference segment and one path under `camera`.
    fn cull_scene(camera: Camera, path: Vec<Subpath<pga::Point>>) -> Vec<Prim2> {
        let mut scene = Scene::new(1.0);
        scene.camera = camera;
        scene.add(Object::segment(
            pga::Point::new(-1.0, 0.0, 0.0),
            pga::Point::new(1.0, 0.0, 0.0),
        ));
        scene.add(Object::path(path));
        scene.eval(0.0)
    }

    fn with_handle_at(z: f64) -> Vec<Subpath<pga::Point>> {
        vec![Subpath::new(pga::Point::new(0.0, 0.0, 0.0)).cubic_to(
            pga::Point::new(0.5, 0.0, z),
            pga::Point::new(1.0, 0.0, 0.0),
            pga::Point::new(1.0, 1.0, 0.0),
        )]
    }

    fn cameras() -> [(&'static str, Camera); 2] {
        [
            ("orthographic", Camera::default()),
            (
                "pinhole",
                Camera::pinhole(Motor3::translator(0.0, 0.0, 5.0), 2.0),
            ),
        ]
    }

    // AC4 — a single control point behind the camera, even a handle whose
    // end points are both in front, culls the whole path. Nothing else in
    // the frame is affected. The camera sits at z = 5 looking along −z.
    #[test]
    fn a_control_point_behind_the_camera_culls_the_whole_path() {
        for (name, camera) in cameras() {
            let prims = cull_scene(camera, with_handle_at(10.0));
            assert_eq!(prims.len(), 1, "{name}: only the segment survives");
            assert!(matches!(prims[0], Prim2::Segment { .. }), "{name}");

            let fine = cull_scene(camera, with_handle_at(4.5));
            assert_eq!(fine.len(), 2, "{name}: a handle in front is not culled");
        }
    }

    // AC4 — a control point exactly on the camera plane (view depth 0)
    // culls the whole path.
    #[test]
    fn a_control_point_on_the_camera_plane_culls_the_whole_path() {
        for (name, camera) in cameras() {
            let prims = cull_scene(camera, with_handle_at(5.0));
            assert_eq!(prims.len(), 1, "{name}: depth 0 culls");
        }
    }

    // AC4 / §2.3 — a non-finite control point anywhere, on a curve or on a
    // line, culls the whole path.
    #[test]
    fn a_non_finite_control_point_culls_the_whole_path() {
        for (name, camera) in cameras() {
            for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
                let handle = vec![Subpath::new(pga::Point::new(0.0, 0.0, 0.0)).cubic_to(
                    pga::Point::new(bad, 0.0, 0.0),
                    pga::Point::new(1.0, 0.0, 0.0),
                    pga::Point::new(1.0, 1.0, 0.0),
                )];
                assert_eq!(
                    cull_scene(camera, handle).len(),
                    1,
                    "{name}: handle x = {bad}"
                );
                let line = vec![
                    Subpath::new(pga::Point::new(0.0, 0.0, 0.0))
                        .line_to(pga::Point::new(1.0, 0.0, 0.0)),
                    Subpath::new(pga::Point::new(0.0, 1.0, 0.0))
                        .line_to(pga::Point::new(1.0, bad, 0.0)),
                ];
                assert_eq!(
                    cull_scene(camera, line).len(),
                    1,
                    "{name}: a line end y = {bad} in the second subpath"
                );
            }
        }
    }

    // AC4 / §2.3 — empty subpaths are dropped at eval, and a path with
    // nothing left is not emitted at all.
    #[test]
    fn empty_subpaths_are_dropped_and_an_empty_path_is_not_emitted() {
        let real = Subpath::new(p(0.0, 0.0)).line_to(p(0.5, 0.25));
        let eval_of = |subs: Vec<Subpath<Pt2>>| {
            let mut scene = Scene::new(1.0);
            scene.add(Object::planar(subs));
            scene.eval(0.0)
        };
        let kept = eval_of(vec![
            Subpath::new(p(1.0, 1.0)),
            real.clone(),
            Subpath::new(p(-1.0, 0.0)),
        ]);
        assert_eq!(the_path(&kept), &[real][..], "only the non-empty subpath");
        assert!(eval_of(Vec::new()).is_empty(), "no subpaths: no primitive");
        assert!(
            eval_of(vec![
                Subpath::new(p(0.0, 0.0)),
                Subpath::new(p(1.0, 0.0)).close()
            ])
            .is_empty(),
            "only empty subpaths: no primitive"
        );
    }

    // AC4 / §2.4 — pinhole subdivision terminates at depth 12, so one
    // authored cubic becomes at most 4096 pieces even where the estimate is
    // never met. The case is a ground-plane quarter circle of radius 3 whose
    // near end sits 10⁻⁵ in front of the camera. That forces the limit: a
    // spec-faithful probe needed 805 pieces at depth 12 and 18 893 at depth
    // 22. Every emitted coordinate stays finite.
    #[test]
    fn pinhole_subdivision_stops_at_4096_pieces_per_authored_cubic() {
        let (near, k) = (1e-5, 3.0 * 4.0 * (SQRT_2 - 1.0) / 3.0);
        let quarter = vec![Subpath::new(pga::Point::new(0.0, -0.3, -near)).cubic_to(
            pga::Point::new(k, -0.3, -near),
            pga::Point::new(3.0, -0.3, -3.0 - near + k),
            pga::Point::new(3.0, -0.3, -3.0 - near),
        )];
        let mut scene = Scene::new(1.0);
        scene.camera = Camera::pinhole(Motor3::identity(), 1.5);
        scene.add(Object::path(quarter));
        let prims = scene.eval(0.0);
        let subs = the_path(&prims);
        assert_eq!(subs.len(), 1);
        let pieces = subs[0].segs.len();
        assert!(
            pieces > 100,
            "a curve this close must be subdivided: {pieces} pieces"
        );
        assert!(
            pieces <= 4096,
            "at most 4096 pieces per authored cubic: {pieces}"
        );
        assert!(
            sample(&subs[0], 0)
                .iter()
                .all(|q| q.x.is_finite() && q.y.is_finite()),
            "every emitted coordinate is finite"
        );
    }

    // AC4 / §2.3 — the invariants eval upholds for `Prim2::Path`: every
    // coordinate finite, at least one subpath, no subpath without segments.
    // Checked on the pinhole output, where subdivision could break them.
    #[test]
    fn emitted_paths_uphold_the_prim2_invariants() {
        for depth in [2.5, 1.5, 1.0] {
            let (scene, _) = tilted_circle_scene(depth, Style::default());
            let prims = scene.eval(0.0);
            let subs = the_path(&prims);
            assert!(!subs.is_empty(), "depth {depth}: at least one subpath");
            for s in subs {
                assert!(!s.segs.is_empty(), "depth {depth}: no empty subpath");
                let mut pts = vec![s.start];
                for seg in &s.segs {
                    match *seg {
                        Seg::Line(e) => pts.push(e),
                        Seg::Cubic(h1, h2, e) => pts.extend([h1, h2, e]),
                    }
                }
                assert!(
                    pts.iter().all(|q| q.x.is_finite() && q.y.is_finite()),
                    "depth {depth}: every coordinate finite"
                );
            }
        }
    }
}

// --- AC5: the fill coverage rule --------------------------------------------

mod ac5 {
    use super::*;

    // AC5 — an axis-aligned rectangle whose edges lie on pixel boundaries
    // paints its interior at exactly 1 and its exterior at exactly 0, with
    // no fringe. Asserted for the whole frame.
    #[test]
    fn a_rectangle_on_pixel_boundaries_has_no_fringe() {
        let f = render_ppm(
            "r0012_ac5_boundaries",
            GRID,
            GRID_VIEW,
            &[path_prim(
                vec![g_rect(16.0, 8.0, 48.0, 40.0)],
                filled(WHITE, 1.0),
            )],
        );
        for y in 0..64 {
            for x in 0..64 {
                let inside = (16..48).contains(&x) && (8..40).contains(&y);
                assert_eq!(f.at(x, y), [if inside { 255 } else { 0 }; 3], "({x}, {y})");
            }
        }
    }

    // AC5 — an edge through a column (or row) of pixel centres paints it at
    // coverage exactly 0.5, which is byte round(127.5) = 128 for white on
    // black. That is the same byte a stroke gives a pixel centred on its
    // boundary (R-0006 AC4). The other edges sit on pixel boundaries, so
    // the whole frame is exact.
    #[test]
    fn an_edge_through_pixel_centres_paints_exactly_128() {
        let vertical = render_ppm(
            "r0012_ac5_centres_v",
            GRID,
            GRID_VIEW,
            &[path_prim(
                vec![g_rect(10.5, 8.0, 40.0, 56.0)],
                filled(WHITE, 1.0),
            )],
        );
        let horizontal = render_ppm(
            "r0012_ac5_centres_h",
            GRID,
            GRID_VIEW,
            &[path_prim(
                vec![g_rect(8.0, 20.5, 56.0, 50.0)],
                filled(WHITE, 1.0),
            )],
        );
        for y in 0..64 {
            for x in 0..64 {
                let v = if !(8..56).contains(&y) || !(10..40).contains(&x) {
                    0
                } else if x == 10 {
                    128
                } else {
                    255
                };
                assert_eq!(vertical.grey(x, y), v, "vertical edge, ({x}, {y})");
                let h = if !(8..56).contains(&x) || !(20..50).contains(&y) {
                    0
                } else if y == 20 {
                    128
                } else {
                    255
                };
                assert_eq!(horizontal.grey(x, y), h, "horizontal edge, ({x}, {y})");
            }
        }
    }

    // AC5 / §2.6 table — an axis-aligned edge at any sub-pixel offset, away
    // from corners, gives coverage 0.5 ± d, which is exactly the box-filtered
    // area. All 17 sixteenth-pixel offsets on each of the four sides, whole
    // frame, with the pinned byte `round(255 · area)`.
    #[test]
    fn an_axis_aligned_edge_at_any_sixteenth_offset_is_the_box_filtered_area() {
        let byte = |cov: f64| (255.0 * cov).round() as u8;
        for k in 0..=16 {
            let f = f64::from(k) / 16.0;
            // (name, rect, the fractional line, its coverage)
            let cases = [
                ("left", g_rect(10.0 + f, 8.0, 40.0, 56.0), 1.0 - f),
                ("right", g_rect(10.0, 8.0, 40.0 + f, 56.0), f),
                ("top", g_rect(8.0, 10.0 + f, 56.0, 40.0), 1.0 - f),
                ("bottom", g_rect(8.0, 10.0, 56.0, 40.0 + f), f),
            ];
            for (name, rect, cov) in cases {
                let frame = render_ppm(
                    &format!("r0012_ac5_offset_{name}_{k}"),
                    GRID,
                    GRID_VIEW,
                    &[path_prim(vec![rect], filled(WHITE, 1.0))],
                );
                // Per-axis coverage of pixel i; only one axis is fractional.
                let axis = |i: u32, lo: u32, hi: u32, frac: Option<(u32, f64)>| -> f64 {
                    match frac {
                        Some((j, c)) if i == j => c,
                        _ if (lo..hi).contains(&i) => 1.0,
                        _ => 0.0,
                    }
                };
                for y in 0..64 {
                    for x in 0..64 {
                        let (cx, cy) = match name {
                            "left" => (axis(x, 11, 40, Some((10, cov))), axis(y, 8, 56, None)),
                            "right" => (axis(x, 10, 40, Some((40, cov))), axis(y, 8, 56, None)),
                            "top" => (axis(x, 8, 56, None), axis(y, 11, 40, Some((10, cov)))),
                            _ => (axis(x, 8, 56, None), axis(y, 10, 40, Some((40, cov)))),
                        };
                        assert_eq!(
                            frame.grey(x, y),
                            byte(cx * cy),
                            "{name} edge at offset {k}/16, pixel ({x}, {y})"
                        );
                    }
                }
            }
        }
    }

    // AC5 / SPEC-0012 §6 — the corner bound, on the exhaustive grid: every
    // one of the 16 × 16 sixteenth-pixel offsets, with w and h each in
    // [1, 3] px in 1/16 steps (278 784 rectangles). The summed coverage
    // must equal the area within 0.25 px² per convex corner.
    //
    // At frame level each fractional pixel may be off by 1/510 from its
    // f64 coverage, so the bound carries `n/510`, where n counts the pixels
    // whose centre is within 0.5 px of the outline. Those are the only ones
    // that can be fractional. The architect measured 0.984 against the
    // pinned 1.0, a margin of 1.6 %, so the slack cannot hide a real miss:
    // the inclusive tie rule gives 1.559 (§2.6).
    #[test]
    fn corner_error_is_at_most_a_quarter_px2_per_corner_on_the_exhaustive_grid() {
        const CELL: u32 = 8;
        let size = (256 * CELL, 33 * CELL);
        let view = (f64::from(size.0) / 16.0, f64::from(size.1) / 16.0);
        let mut worst = 0.0_f64;
        for a in 16..=48u32 {
            let w = f64::from(a) / 16.0;
            let mut prims = Vec::new();
            let mut rects = Vec::new();
            for (row, b) in (16..=48u32).enumerate() {
                let h = f64::from(b) / 16.0;
                for o in 0..256u32 {
                    let (ox, oy) = (f64::from(o % 16) / 16.0, f64::from(o / 16) / 16.0);
                    let x0 = f64::from(o * CELL) + 2.0 + ox;
                    let y0 = row as f64 * f64::from(CELL) + 2.0 + oy;
                    let (x1, y1) = (x0 + w, y0 + h);
                    prims.push(path_prim(
                        vec![px_rect(size, 16.0, (x0, y0), (x1, y1))],
                        filled(WHITE, 1.0),
                    ));
                    rects.push((o, row as u32, x0, y0, x1, y1));
                }
            }
            let f = render_ppm(&format!("r0012_ac5_corners_{a}"), size, view, &prims);
            for (o, row, x0, y0, x1, y1) in rects {
                let (mut sum, mut n) = (0.0, 0);
                for y in row * CELL..(row + 1) * CELL {
                    for x in o * CELL..(o + 1) * CELL {
                        sum += f64::from(f.grey(x, y)) / 255.0;
                        if in_fringe((x0, y0, x1, y1), x, y) {
                            n += 1;
                        }
                    }
                }
                let err = (sum - (x1 - x0) * (y1 - y0)).abs();
                worst = worst.max(err - f64::from(n) / 510.0);
                assert!(
                    err <= 4.0 * 0.25 + f64::from(n) / 510.0,
                    "w = {} px, h = {} px at offset ({}, {})/16: |Σcov − area| = {err}, \
                     over 1.0 + {n}/510",
                    x1 - x0,
                    y1 - y0,
                    o % 16,
                    o / 16
                );
            }
        }
        eprintln!("AC5 corner grid: worst |Σcov − area| − n/510 = {worst}");
    }

    /// Whether pixel `(x, y)` has its centre within 0.5 px of the outline of
    /// the pixel-space rectangle `(x0, y0, x1, y1)`. Only those pixels can
    /// have fractional coverage, so only they can be rounded.
    fn in_fringe((x0, y0, x1, y1): (f64, f64, f64, f64), x: u32, y: u32) -> bool {
        let (cx, cy) = (f64::from(x) + 0.5, f64::from(y) + 0.5);
        let inside = (x0..=x1).contains(&cx) && (y0..=y1).contains(&cy);
        let d = if inside {
            (cx - x0).min(x1 - cx).min(cy - y0).min(y1 - cy)
        } else {
            let dx = (x0 - cx).max(cx - x1).max(0.0);
            let dy = (y0 - cy).max(cy - y1).max(0.0);
            (dx * dx + dy * dy).sqrt()
        };
        d < 0.5
    }

    // AC5 — "at any sub-pixel offset": the same corner bound on 20 000
    // rectangles at random, non-dyadic offsets and sizes (fixed seed), with
    // w and h in [1, 3] px. This covers the gaps between the sixteenths of
    // the exhaustive grid, and leaves no pixel centre exactly on an edge
    // line.
    #[test]
    fn corner_error_bound_holds_at_random_non_dyadic_offsets() {
        const CELL: u32 = 8;
        let (cols, rows) = (200u32, 100u32);
        let size = (cols * CELL, rows * CELL);
        let view = (f64::from(size.0) / 16.0, f64::from(size.1) / 16.0);
        let mut rng = Lcg(0x5eed_0012);
        let mut prims = Vec::new();
        let mut rects = Vec::new();
        for row in 0..rows {
            for col in 0..cols {
                let x0 = f64::from(col * CELL) + 2.0 + rng.unit();
                let y0 = f64::from(row * CELL) + 2.0 + rng.unit();
                let (x1, y1) = (x0 + rng.range(1.0, 3.0), y0 + rng.range(1.0, 3.0));
                prims.push(path_prim(
                    vec![px_rect(size, 16.0, (x0, y0), (x1, y1))],
                    filled(WHITE, 1.0),
                ));
                rects.push((col, row, x0, y0, x1, y1));
            }
        }
        let f = render_ppm("r0012_ac5_corners_random", size, view, &prims);
        let mut worst = 0.0_f64;
        for (col, row, x0, y0, x1, y1) in rects {
            let (mut sum, mut n) = (0.0, 0);
            for y in row * CELL..(row + 1) * CELL {
                for x in col * CELL..(col + 1) * CELL {
                    sum += f64::from(f.grey(x, y)) / 255.0;
                    if in_fringe((x0, y0, x1, y1), x, y) {
                        n += 1;
                    }
                }
            }
            let err = (sum - (x1 - x0) * (y1 - y0)).abs();
            worst = worst.max(err - f64::from(n) / 510.0);
            assert!(
                err <= 1.0 + f64::from(n) / 510.0,
                "rect [{x0}, {x1}] × [{y0}, {y1}]: |Σcov − area| = {err}, over 1.0 + {n}/510"
            );
        }
        eprintln!("AC5 random corners: worst |Σcov − area| − n/510 = {worst}");
    }

    // AC5 / §2.6 — the strict half-plane tie rule is load-bearing. A pixel
    // centre that lies exactly on an edge's line, 0.25 px beyond the edge's
    // end, is outside the rectangle. The strict rule gives it s = −d, so its
    // coverage is 0.5 − 0.25 (byte 64), or 0 if no edge at that vertex
    // classifies as boundary. The inclusive reading gives +d, so 0.75
    // (byte 191), which is the reading that breaks the corner bound. There
    // are eight such pixels, two per corner of each orientation. Their
    // neighbours pin the surrounding identities: 128 on the edge line and
    // 64 on the column 0.25 px outside.
    #[test]
    fn a_centre_on_an_edge_line_beyond_its_end_takes_the_outside_sign() {
        let f = render_ppm(
            "r0012_ac5_tie_rule",
            GRID,
            GRID_VIEW,
            &[
                path_prim(vec![g_rect(10.75, 10.5, 20.25, 20.5)], filled(WHITE, 1.0)),
                path_prim(vec![g_rect(30.5, 10.75, 40.5, 20.25)], filled(WHITE, 1.0)),
            ],
        );
        for (x, y) in [
            (10, 10),
            (20, 10),
            (10, 20),
            (20, 20),
            (30, 10),
            (40, 10),
            (30, 20),
            (40, 20),
        ] {
            let b = f.grey(x, y);
            assert!(
                b == 0 || b == 64,
                "pixel ({x}, {y}) lies on an edge's line beyond its end: the strict \
                 rule gives 64 or 0, got {b} (191 is the inclusive reading)"
            );
        }
        for i in 11..20 {
            assert_eq!(f.grey(i, 10), 128, "rectangle 1, top edge through centres");
            assert_eq!(
                f.grey(i, 20),
                128,
                "rectangle 1, bottom edge through centres"
            );
            assert_eq!(
                f.grey(10, i),
                64,
                "rectangle 1, 0.25 px outside the left edge"
            );
            assert_eq!(
                f.grey(20, i),
                64,
                "rectangle 1, 0.25 px outside the right edge"
            );
            assert_eq!(f.grey(30, i), 128, "rectangle 2, left edge through centres");
            assert_eq!(
                f.grey(i + 20, 10),
                64,
                "rectangle 2, 0.25 px above the top edge"
            );
        }
    }

    /// A lines-only outline with dyadic vertices whose edge lines avoid
    /// every pixel centre, and a stroke on it.
    fn lopsided(dx: f64, dy: f64) -> Prim2 {
        let v = [
            (12.0625, 14.1875),
            (30.3125, 18.4375),
            (22.5625, 33.0625),
            (15.1875, 27.8125),
        ];
        let q = |i: usize| img(GRID, GS, v[i].0 + dx, v[i].1 + dy);
        path_prim(
            vec![Subpath::new(q(0))
                .line_to(q(1))
                .line_to(q(2))
                .line_to(q(3))
                .close()],
            paint(STEEL, 0.1875, 0.75, solid(GOLD, 0.8)),
        )
    }

    // AC5 — coverage is a closed-form function of each pixel centre and the
    // outline, with no accumulation across pixels. It is therefore
    // translation-equivariant: moving the outline by whole pixels moves the
    // frame by whole pixels, byte for byte. Every difference involved is
    // exact on dyadic input, so any mismatch is state carried between
    // pixels.
    #[test]
    fn coverage_is_translation_equivariant() {
        let a = render_ppm("r0012_ac5_shift_a", GRID, GRID_VIEW, &[lopsided(0.0, 0.0)]);
        let b = render_ppm("r0012_ac5_shift_b", GRID, GRID_VIEW, &[lopsided(13.0, 9.0)]);
        let mut ink = 0;
        for y in 4..44 {
            for x in 4..40 {
                assert_eq!(a.at(x, y), b.at(x + 13, y + 9), "pixel ({x}, {y}) moved");
                if a.at(x, y) != [0, 0, 0] {
                    ink += 1;
                }
            }
        }
        assert!(ink > 200, "the shape must be in the compared region");
    }

    // AC5 — distant geometry in the same primitive, in the same rows, does
    // not move a pixel near the first shape. A closed loop adds zero winding
    // outside itself, and the per-row edge list is a filter, not an
    // accumulator (§2.6).
    #[test]
    fn distant_geometry_in_the_same_path_moves_no_pixel() {
        let Prim2::Path { subpaths, style } = lopsided(0.0, 0.0) else {
            unreachable!("lopsided builds a path")
        };
        let mut more = subpaths.clone();
        more.push(g_rect(44.0, 6.0, 60.0, 40.0));
        let alone = render_ppm(
            "r0012_ac5_alone",
            GRID,
            GRID_VIEW,
            &[path_prim(subpaths, style)],
        );
        let with = render_ppm(
            "r0012_ac5_with_distant",
            GRID,
            GRID_VIEW,
            &[path_prim(more, style)],
        );
        for y in 0..64 {
            for x in 0..40 {
                assert_eq!(alone.at(x, y), with.at(x, y), "pixel ({x}, {y})");
            }
        }
        assert_ne!(with.at(52, 20), [0, 0, 0], "the distant square is drawn");
    }
}

// --- AC6: flattening tolerance ----------------------------------------------

mod ac6 {
    use super::*;

    // AC6 as amended — a stroked circle of radius R px and stroke radius r
    // lights only pixels whose centres lie within
    // R ± (r + 0.5 + 0.1 + 3·10⁻⁴·R) of the true centre, and lights every
    // pixel within R ± (r − 0.5). This holds for R ∈ {10, 100, 500} and
    // r ∈ {1, 2, 4}. The canvas is at s = 1 px per unit, and the centre is
    // off the pixel grid.
    #[test]
    fn a_stroked_circle_lights_only_its_amended_band_and_all_of_its_core() {
        let size = (1024, 1024);
        let view = (1024.0, 1024.0);
        let centre = p(0.3, -0.2);
        let c_px = to_px(size, 1.0, centre);
        for big_r in [10.0, 100.0, 500.0] {
            for r in [1.0, 2.0, 4.0] {
                let f = render_ppm(
                    &format!("r0012_ac6_band_{big_r}_{r}"),
                    size,
                    view,
                    &[path_prim(
                        circle(centre, big_r),
                        stroked(WHITE, 2.0 * r, 1.0),
                    )],
                );
                let outer = r + 0.5 + 0.1 + 3e-4 * big_r;
                let core = r - 0.5;
                for y in 0..size.1 {
                    for x in 0..size.0 {
                        let (cx, cy) = (f64::from(x) + 0.5, f64::from(y) + 0.5);
                        let e = (((cx - c_px.0) * (cx - c_px.0) + (cy - c_px.1) * (cy - c_px.1))
                            .sqrt()
                            - big_r)
                            .abs();
                        let lit = f.grey(x, y) > 0;
                        assert!(
                            !lit || e <= outer,
                            "R = {big_r}, r = {r}: pixel ({x}, {y}) is lit {e} px from \
                             the circle, beyond the band {outer}"
                        );
                        assert!(
                            lit || e > core,
                            "R = {big_r}, r = {r}: pixel ({x}, {y}) is {e} px from the \
                             circle, inside the core {core}, and unlit"
                        );
                    }
                }
            }
        }
    }

    /// Random cubics in [-100, 100]² (fixed seed), plus the hard cases: a
    /// cusp, a loop, a collinear back-and-forth, a point, and a nearly
    /// straight curve.
    fn cubics() -> Vec<[Pt2; 4]> {
        let mut rng = Lcg(0x0012_0012_0012_0012);
        let mut out: Vec<[Pt2; 4]> = (0..120)
            .map(|_| std::array::from_fn(|_| p(rng.range(-100.0, 100.0), rng.range(-100.0, 100.0))))
            .collect();
        out.extend([
            // B'(1/2) = 0: a true cusp.
            [
                p(-80.0, -80.0),
                p(80.0, 80.0),
                p(-80.0, 80.0),
                p(80.0, -80.0),
            ],
            [p(-80.0, 0.0), p(120.0, 80.0), p(-120.0, 80.0), p(80.0, 0.0)],
            [p(-80.0, 0.0), p(80.0, 0.0), p(-80.0, 0.0), p(80.0, 0.0)],
            [p(12.5, -7.25); 4],
            [
                p(-90.0, -30.0),
                p(-30.0, -29.0),
                p(30.0, -31.0),
                p(90.0, -30.0),
            ],
        ]);
        out
    }

    // AC6 / SPEC-0012 §6 — chords stay within 0.1 px of the curve, measured
    // over random cubics including cusps, and measured from the pixels
    // (flattening is private to `ppm.rs`). Each is stroked at r = 1.5 px.
    // On the stroke's fringe the coverage is `r + 0.5 − d`, where d is the
    // distance to the chords, so every fractional pixel reports d within
    // 1/510. The distance to the chords and the distance to the true curve
    // differ by at most the chord deviation, so the two must agree within
    // 0.1 px, plus 1/510 for the byte and 10⁻³ for sampling the truth.
    #[test]
    fn chords_stay_within_a_tenth_of_a_pixel_of_random_cubics_including_cusps() {
        let (size, r) = ((256, 256), 1.5);
        let bound = 0.1 + 1.0 / 510.0 + 1e-3;
        let mut worst = 0.0_f64;
        for (i, c) in cubics().into_iter().enumerate() {
            let sub = Subpath::new(c[0]).cubic_to(c[1], c[2], c[3]);
            let f = render_ppm(
                &format!("r0012_ac6_cubic_{i}"),
                size,
                (256.0, 256.0),
                &[path_prim(vec![sub.clone()], stroked(WHITE, 2.0 * r, 1.0))],
            );
            let truth: Vec<(f64, f64)> = sample(&sub, 4096)
                .into_iter()
                .map(|q| to_px(size, 1.0, q))
                .collect();
            let near = Nearest::new(&[truth], 4.0);
            let mut fractional = 0;
            for y in 0..size.1 {
                for x in 0..size.0 {
                    let b = f.grey(x, y);
                    if b == 0 || b == 255 {
                        continue;
                    }
                    fractional += 1;
                    let d_chords = r + 0.5 - f64::from(b) / 255.0;
                    let d_true = near.distance((f64::from(x) + 0.5, f64::from(y) + 0.5));
                    let err = (d_chords - d_true).abs();
                    worst = worst.max(err);
                    assert!(
                        err <= bound,
                        "cubic {i} {c:?}: pixel ({x}, {y}) is {d_chords} px from the \
                         chords but {d_true} px from the curve"
                    );
                }
            }
            assert!(fractional > 0, "cubic {i}: nothing to measure");
        }
        eprintln!("AC6 chord sweep: worst |d_chords − d_curve| = {worst} px");
    }
}

// --- AC7: shape generators --------------------------------------------------

mod ac7 {
    use super::*;

    /// SPEC-0012 §2.9's golden constant, `4(√2 − 1)/3` in f64.
    const K: f64 = 0.5522847498307936;

    fn cubic_segments(sub: &Subpath<Pt2>) -> Vec<[Pt2; 4]> {
        let mut out = Vec::new();
        let mut cur = sub.start;
        for seg in &sub.segs {
            match *seg {
                Seg::Line(e) => cur = e,
                Seg::Cubic(h1, h2, e) => {
                    out.push([cur, h1, h2, e]);
                    cur = e;
                }
            }
        }
        out
    }

    fn line_count(sub: &Subpath<Pt2>) -> usize {
        sub.segs
            .iter()
            .filter(|s| matches!(s, Seg::Line(_)))
            .count()
    }

    fn close_to(a: Pt2, b: Pt2, tol: f64) -> bool {
        dist(a, b) <= tol
    }

    /// No segment has zero length: a line never ends where it starts, and a
    /// cubic never has all four control points equal.
    fn no_zero_length_segment(sub: &Subpath<Pt2>) -> bool {
        let mut cur = sub.start;
        sub.segs.iter().all(|seg| {
            let ok = match *seg {
                Seg::Line(e) => e != cur,
                Seg::Cubic(h1, h2, e) => !(h1 == cur && h2 == cur && e == cur),
            };
            cur = match *seg {
                Seg::Line(e) | Seg::Cubic(_, _, e) => e,
            };
            ok
        })
    }

    // AC7 — the circle is trig-free: its handle is `K·r` with K pinned at
    // 0.5522847498307936, bit for bit. The first quarter of the unit circle
    // at the origin exposes K exactly, and K is the closed form evaluated
    // in f64.
    #[test]
    fn the_circle_handle_is_the_pinned_constant_bit_for_bit() {
        assert_eq!(
            (4.0 * (SQRT_2 - 1.0) / 3.0_f64).to_bits(),
            K.to_bits(),
            "the pinned literal is the closed form"
        );
        let unit = circle(p(0.0, 0.0), 1.0);
        assert_eq!(unit.len(), 1, "one subpath");
        let s = &unit[0];
        assert_eq!(s.start, p(1.0, 0.0), "starts at (c.x + r, c.y)");
        match s.segs[0] {
            Seg::Cubic(h1, h2, e) => {
                assert_eq!((h1.x, h1.y.to_bits()), (1.0, K.to_bits()), "first handle");
                assert_eq!((h2.x.to_bits(), h2.y), (K.to_bits(), 1.0), "second handle");
                assert_eq!(e, p(0.0, 1.0), "a quarter turn counter-clockwise");
            }
            Seg::Line(_) => panic!("a circle is made of cubics"),
        }
        // Scaling by a power of two is exact, so the handle is still K·r.
        let two = circle(p(0.0, 0.0), 2.0);
        match two[0].segs[0] {
            Seg::Cubic(h1, _, _) => assert_eq!(h1.y.to_bits(), (2.0 * K).to_bits()),
            Seg::Line(_) => panic!("a circle is made of cubics"),
        }
    }

    // AC7 / §2.9 — a circle is one closed subpath of four cubic quarters,
    // counter-clockwise, through c ± r on both axes, ending where it began.
    // Every handle lies K·r from its on-curve point.
    #[test]
    fn a_circle_is_four_counter_clockwise_quarters() {
        for (c, r) in [
            (p(0.3, -0.7), 0.5),
            (p(-2.0, 1.25), 3.0),
            (p(1e3, 0.0), 1e-3),
        ] {
            let out = circle(c, r);
            assert_eq!(out.len(), 1);
            let s = &out[0];
            assert!(s.closed, "closed");
            assert_eq!(s.segs.len(), 4, "four quarters");
            let quarters = cubic_segments(s);
            assert_eq!(quarters.len(), 4, "all cubics");
            let tol = 1e-12 * (r + c.x.abs() + c.y.abs());
            let want = [
                p(c.x + r, c.y),
                p(c.x, c.y + r),
                p(c.x - r, c.y),
                p(c.x, c.y - r),
            ];
            for (i, q) in quarters.iter().enumerate() {
                assert!(
                    close_to(q[0], want[i], tol),
                    "quarter {i} starts at {:?}",
                    q[0]
                );
                assert!(
                    close_to(q[3], want[(i + 1) % 4], tol),
                    "quarter {i} ends at {:?}",
                    q[3]
                );
                assert!(
                    (dist(q[0], q[1]) - K * r).abs() <= tol,
                    "quarter {i}: first handle K·r"
                );
                assert!(
                    (dist(q[3], q[2]) - K * r).abs() <= tol,
                    "quarter {i}: second handle K·r"
                );
            }
            assert_eq!(
                quarters[3][3], s.start,
                "the last quarter ends at the start"
            );
            assert!(signed_area(&out) > 0.0, "counter-clockwise");
        }
    }

    // AC7 — the circle's radial deviation is at most 3·10⁻⁴·R by dense
    // sampling (10⁵ points), at several scales. It is also outward only,
    // which is the fact the AC6 amendment rests on.
    #[test]
    fn circle_radial_error_is_within_3e_4_r_and_only_outward() {
        for (c, r) in [
            (p(0.3, -0.7), 2.0),
            (p(0.0, 0.0), 500.0),
            (p(-1.0, 4.0), 1e-3),
        ] {
            let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
            for q in cubic_segments(&circle(c, r)[0]) {
                for k in 0..=25_000 {
                    let e = dist(cubic_at(q, f64::from(k) / 25_000.0), c) - r;
                    lo = lo.min(e);
                    hi = hi.max(e);
                }
            }
            assert!(hi <= 3e-4 * r, "R = {r}: bulges {} R", hi / r);
            assert!(lo >= -1e-12 * r, "R = {r}: dips inside by {} R", -lo / r);
            assert!(
                hi >= 2.5e-4 * r,
                "R = {r}: this is the 4-cubic circle ({})",
                hi / r
            );
        }
    }

    // AC7 / §2.9 — an arc is open and uses m = ceil(|sweep| / (τ/4)) cubics.
    // The sweeps here make that ratio exact or clearly non-integral, so the
    // count does not hinge on rounding.
    #[test]
    fn an_arc_uses_one_cubic_per_started_quarter_turn() {
        let c = p(0.25, -0.5);
        for (sweep, m) in [
            (0.1, 1),
            (TAU / 4.0, 1),
            (TAU / 4.0 + 1e-9, 2),
            (TAU / 2.0, 2),
            (0.7 * TAU, 3),
            (TAU, 4),
            (-TAU / 3.0, 2),
            (-TAU, 4),
        ] {
            let out = arc(c, 1.5, 0.4, sweep);
            assert_eq!(out.len(), 1, "sweep {sweep}: one subpath");
            assert!(!out[0].closed, "sweep {sweep}: an arc is open");
            assert_eq!(out[0].segs.len(), m, "sweep {sweep}: {m} cubics");
            assert_eq!(
                cubic_segments(&out[0]).len(),
                m,
                "sweep {sweep}: all cubics"
            );
        }
    }

    // AC7 — arc end points are exact: they lie on the circle at the start
    // and end angles, up to the rounding of `cos`/`sin`. The interior stays
    // within 3·10⁻⁴·r of the circle and turns the way the sweep's sign says.
    #[test]
    fn arc_endpoints_lie_on_the_circle_and_the_arc_turns_the_right_way() {
        let angle = |q: Pt2, c: Pt2| (q.y - c.y).atan2(q.x - c.x);
        let same_angle =
            |a: f64, b: f64| ((a - b).rem_euclid(TAU)).min((b - a).rem_euclid(TAU)) < 1e-9;
        for (c, r, start, sweep) in [
            (p(0.25, -0.5), 1.5, 0.4, 2.0),
            (p(-3.0, 2.0), 0.75, -1.0, -4.5),
            (p(0.0, 0.0), 10.0, 3.0, TAU),
            (p(1.0, 1.0), 2.0, 0.0, 0.3),
        ] {
            let s = &arc(c, r, start, sweep)[0];
            let tol = 1e-12 * (r + c.x.abs() + c.y.abs());
            let want_start = p(c.x + r * start.cos(), c.y + r * start.sin());
            let want_end = p(
                c.x + r * (start + sweep).cos(),
                c.y + r * (start + sweep).sin(),
            );
            let end = *on_curve(s).last().expect("an end point");
            assert!(
                close_to(s.start, want_start, tol),
                "start {:?} vs {want_start:?}",
                s.start
            );
            assert!(close_to(end, want_end, tol), "end {end:?} vs {want_end:?}");

            let quarters = cubic_segments(s);
            let m = quarters.len() as f64;
            for (i, q) in quarters.iter().enumerate() {
                for k in 0..=1000 {
                    let e = dist(cubic_at(*q, f64::from(k) / 1000.0), c) - r;
                    assert!(e.abs() <= 3e-4 * r, "piece {i}: radial error {e}");
                }
                let mid = angle(cubic_at(*q, 0.5), c);
                let want = start + sweep * (i as f64 + 0.5) / m;
                assert!(
                    same_angle(mid, want),
                    "piece {i} of sweep {sweep}: midpoint at angle {mid}, want {want}"
                );
            }
        }
    }

    // AC7 / §2.9 — |sweep| beyond a full turn is clamped to one turn, in
    // both directions.
    #[test]
    fn a_sweep_beyond_a_full_turn_is_clamped_to_one_turn() {
        let c = p(0.5, 0.25);
        assert_eq!(arc(c, 1.0, 0.3, 3.0 * TAU), arc(c, 1.0, 0.3, TAU));
        assert_eq!(arc(c, 1.0, 0.3, -5.5 * TAU), arc(c, 1.0, 0.3, -TAU));
        assert_eq!(sector(c, 1.0, 0.3, 7.0), sector(c, 1.0, 0.3, TAU));
    }

    // AC7 / §2.9 — a sector is `M c`, `L` to the arc's start, the arc's
    // cubics, `Z`. It is exactly the arc with the centre prepended and the
    // subpath closed.
    #[test]
    fn a_sector_is_the_centre_a_line_to_the_arc_the_arc_and_a_close() {
        for (c, r, start, sweep) in [
            (p(0.25, -0.5), 1.5, 0.4, 2.0),
            (p(-1.0, 0.5), 0.5, 1.0, TAU),
            (p(0.0, 0.0), 2.0, -0.5, -1.0),
        ] {
            let s = sector(c, r, start, sweep);
            let a = arc(c, r, start, sweep);
            assert_eq!(s.len(), 1);
            assert_eq!(s[0].start, c, "it starts at the centre");
            assert_eq!(s[0].segs[0], Seg::Line(a[0].start), "a line to the arc");
            assert_eq!(s[0].segs[1..], a[0].segs[..], "then the arc itself");
            assert!(s[0].closed, "and closes back to the centre");
        }
    }

    // AC7 / §2.9 — `rad = 0` gives a plain rectangle: exactly three lines
    // plus `close()`. The closing side is implicit, so no zero-length
    // closing chord exists. The four on-curve points are the corners.
    #[test]
    fn a_rounded_rect_with_zero_radius_is_three_lines_and_a_close() {
        let (c, w, h) = (p(0.25, -0.5), 2.0, 1.0);
        let out = rounded_rect(c, w, h, 0.0);
        assert_eq!(out.len(), 1);
        let s = &out[0];
        assert!(s.closed);
        assert_eq!((s.segs.len(), line_count(s)), (3, 3), "three lines");
        let pts = on_curve(s);
        for corner in [
            p(c.x - w / 2.0, c.y - h / 2.0),
            p(c.x + w / 2.0, c.y - h / 2.0),
            p(c.x + w / 2.0, c.y + h / 2.0),
            p(c.x - w / 2.0, c.y + h / 2.0),
        ] {
            assert!(
                pts.iter().any(|&q| close_to(q, corner, 1e-12)),
                "corner {corner:?} missing from {pts:?}"
            );
        }
        assert_ne!(
            *pts.last().expect("end"),
            s.start,
            "no zero-length closing chord"
        );
    }

    // AC7 / §2.9 — each corner is a quarter-circle cubic of radius `rad`
    // with handle K·rad: a chord of rad·√2 and each handle K·rad from its
    // end point. With 0 < rad < min(w, h)/2 all four straight sides remain,
    // either four lines or three plus the implicit close.
    #[test]
    fn rounded_rect_corners_are_quarter_circles_with_handle_k_rad() {
        let (c, w, h, rad) = (p(-0.5, 0.75), 3.0, 2.0, 0.4);
        let s = &rounded_rect(c, w, h, rad)[0];
        let corners = cubic_segments(s);
        assert_eq!(corners.len(), 4, "four corners");
        assert!((3..=4).contains(&line_count(s)), "all four sides: {s:?}");
        for q in corners {
            assert!(
                (dist(q[0], q[3]) - rad * SQRT_2).abs() <= 1e-12,
                "chord of {q:?}"
            );
            assert!(
                (dist(q[0], q[1]) - K * rad).abs() <= 1e-12,
                "first handle of {q:?}"
            );
            assert!(
                (dist(q[3], q[2]) - K * rad).abs() <= 1e-12,
                "second handle of {q:?}"
            );
        }
    }

    // AC7 — the corner radius is clamped to [0, min(w, h)/2]: anything
    // larger is the half-side case, anything negative is the rectangle. At
    // the clamp the vanished straight sides are omitted, not emitted at
    // zero length.
    #[test]
    fn rounded_rect_radius_is_clamped_to_half_the_shorter_side() {
        let c = p(0.5, 0.5);
        assert_eq!(
            rounded_rect(c, 2.0, 1.0, 5.0),
            rounded_rect(c, 2.0, 1.0, 0.5)
        );
        assert_eq!(
            rounded_rect(c, 2.0, 1.0, -1.0),
            rounded_rect(c, 2.0, 1.0, 0.0)
        );

        let wide = &rounded_rect(c, 2.0, 1.0, 0.5)[0];
        assert_eq!(cubic_segments(wide).len(), 4);
        assert!(
            (1..=2).contains(&line_count(wide)),
            "only the long sides remain: {wide:?}"
        );
        assert!(no_zero_length_segment(wide), "{wide:?}");

        let round = &rounded_rect(c, 1.0, 1.0, 0.5)[0];
        assert_eq!(
            (cubic_segments(round).len(), line_count(round)),
            (4, 0),
            "{round:?}"
        );
    }

    // AC7 / §2.9 — a polygon is its vertices joined by lines and closed,
    // with the closing side implicit.
    #[test]
    fn a_polygon_is_its_vertices_as_lines_closed_implicitly() {
        let [a, b, c, d] = square(0.25, -0.5, 0.75);
        assert_eq!(
            polygon(&[a, b, c, d]),
            vec![Subpath::new(a).line_to(b).line_to(c).line_to(d).close()]
        );
        assert_eq!(
            polygon(&[a, b, c]),
            vec![Subpath::new(a).line_to(b).line_to(c).close()]
        );
    }

    // AC7 / §2.9 — generators are counter-clockwise in y-up coordinates
    // (positive signed area), stated once and tested here. For `sector`
    // that holds for a positive sweep, and for `polygon` for
    // counter-clockwise input; see the test plan for the other cases.
    #[test]
    fn generators_are_counter_clockwise() {
        let c = p(0.3, -0.2);
        let cases = [
            ("circle", circle(c, 1.5)),
            ("rounded_rect rad 0", rounded_rect(c, 2.0, 1.0, 0.0)),
            ("rounded_rect rad 0.3", rounded_rect(c, 2.0, 1.0, 0.3)),
            ("rounded_rect rad max", rounded_rect(c, 2.0, 1.0, 0.5)),
            ("sector", sector(c, 1.0, 0.3, 2.0)),
            ("sector full turn", sector(c, 1.0, 0.3, TAU)),
            ("polygon", polygon(&square(0.0, 0.0, 1.0))),
        ];
        for (name, out) in cases {
            assert!(signed_area(&out) > 0.0, "{name} must be counter-clockwise");
        }
    }

    // AC7 / §2.9 — no generator emits a zero-length side, and the
    // all-line ones have no zero-length closing chord either.
    #[test]
    fn no_generator_emits_a_zero_length_side() {
        let c = p(0.3, -0.2);
        let curved = [
            circle(c, 1.5),
            arc(c, 1.0, 0.3, 2.0),
            arc(c, 1.0, 0.3, TAU),
            sector(c, 1.0, 0.3, TAU),
            rounded_rect(c, 2.0, 1.0, 0.3),
            rounded_rect(c, 2.0, 1.0, 0.5),
            rounded_rect(c, 1.0, 1.0, 0.5),
        ];
        for out in curved.iter().flatten() {
            assert!(no_zero_length_segment(out), "{out:?}");
        }
        for out in [
            rounded_rect(c, 2.0, 1.0, 0.0),
            polygon(&square(0.0, 0.0, 1.0)),
        ] {
            let s = &out[0];
            assert!(no_zero_length_segment(s), "{s:?}");
            assert_ne!(*on_curve(s).last().expect("end"), s.start, "{s:?}");
        }
    }

    // AC7 — degenerate inputs never panic; each returns an empty path:
    // non-finite arguments, r ≤ 0, sweep = 0, w ≤ 0 or h ≤ 0, fewer than
    // three polygon vertices.
    #[test]
    fn degenerate_inputs_return_an_empty_path_without_panicking() {
        let (c, nan, inf) = (p(0.0, 0.0), f64::NAN, f64::INFINITY);
        let cases: Vec<(&str, Vec<Subpath<Pt2>>)> = vec![
            ("circle r = 0", circle(c, 0.0)),
            ("circle r = -0", circle(c, -0.0)),
            ("circle r < 0", circle(c, -1.0)),
            ("circle r NaN", circle(c, nan)),
            ("circle r inf", circle(c, inf)),
            ("circle c NaN", circle(p(nan, 0.0), 1.0)),
            ("circle c inf", circle(p(0.0, -inf), 1.0)),
            ("arc sweep 0", arc(c, 1.0, 0.3, 0.0)),
            ("arc sweep -0", arc(c, 1.0, 0.3, -0.0)),
            ("arc r = 0", arc(c, 0.0, 0.3, 1.0)),
            ("arc r < 0", arc(c, -1.0, 0.3, 1.0)),
            ("arc start NaN", arc(c, 1.0, nan, 1.0)),
            ("arc sweep NaN", arc(c, 1.0, 0.3, nan)),
            ("arc sweep inf", arc(c, 1.0, 0.3, inf)),
            ("arc c NaN", arc(p(0.0, nan), 1.0, 0.3, 1.0)),
            ("sector sweep 0", sector(c, 1.0, 0.3, 0.0)),
            ("sector r = 0", sector(c, 0.0, 0.3, 1.0)),
            ("sector start inf", sector(c, 1.0, inf, 1.0)),
            ("sector sweep NaN", sector(c, 1.0, 0.3, nan)),
            ("rounded_rect w = 0", rounded_rect(c, 0.0, 1.0, 0.1)),
            ("rounded_rect h = 0", rounded_rect(c, 1.0, 0.0, 0.1)),
            ("rounded_rect w < 0", rounded_rect(c, -1.0, 1.0, 0.1)),
            ("rounded_rect h < 0", rounded_rect(c, 1.0, -1.0, 0.1)),
            ("rounded_rect w NaN", rounded_rect(c, nan, 1.0, 0.1)),
            ("rounded_rect rad NaN", rounded_rect(c, 1.0, 1.0, nan)),
            ("rounded_rect rad inf", rounded_rect(c, 1.0, 1.0, inf)),
            (
                "rounded_rect c inf",
                rounded_rect(p(inf, 0.0), 1.0, 1.0, 0.1),
            ),
            ("polygon of 0", polygon(&[])),
            ("polygon of 1", polygon(&[c])),
            ("polygon of 2", polygon(&[c, p(1.0, 0.0)])),
            ("polygon with NaN", polygon(&[c, p(1.0, 0.0), p(nan, 1.0)])),
        ];
        for (name, out) in cases {
            assert!(
                out.is_empty(),
                "{name} must return an empty path, got {out:?}"
            );
        }
        assert_eq!(
            circle(c, 5e-324).len(),
            1,
            "a tiny positive radius is still a circle"
        );
    }
}

// --- AC8: SvgSink renders paths ---------------------------------------------

mod ac8 {
    use super::*;

    /// The fixture: a closed line-and-cubic subpath and an open line.
    fn fixture() -> Vec<Subpath<Pt2>> {
        vec![
            Subpath::new(p(0.0, 0.0))
                .line_to(p(1.0, 0.0))
                .cubic_to(p(1.0, 0.5), p(0.5, 1.0), p(0.0, 1.0))
                .close(),
            Subpath::new(p(-1.5, -0.25)).line_to(p(-0.75, 0.125)),
        ]
    }

    const D: &str = "M 0,0 L 1,0 C 1,0.5 0.5,1 0,1 Z M -1.5,-0.25 L -0.75,0.125";

    fn only_path_line(name: &str, style: Style) -> String {
        let svg = render_svg(name, GRID, GRID_VIEW, &[path_prim(fixture(), style)]);
        let lines = path_lines(&svg);
        assert_eq!(lines.len(), 1, "exactly one <path> element");
        lines[0].to_string()
    }

    // AC8 / §2.8 — the whole document for one filled-and-stroked path:
    // the R-0003 header, one `<path>` in the pinned attribute order with
    // absolute `M`/`L`/`C`/`Z`, and the R-0003 footer. Byte for byte.
    #[test]
    fn the_path_element_grammar_is_pinned_byte_for_byte() {
        let svg = render_svg(
            "r0012_ac8_grammar",
            GRID,
            GRID_VIEW,
            &[path_prim(
                fixture(),
                paint(STEEL, 0.02, 0.5, solid(GOLD, 0.25)),
            )],
        );
        let want = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <svg xmlns=\"http://www.w3.org/2000/svg\" width=\"64\" height=\"64\" \
             viewBox=\"-2 -2 4 4\">\n\
             <g transform=\"scale(1 -1)\">\n\
             <path d=\"{D}\" fill=\"#e9b23f\" fill-opacity=\"0.25\" fill-rule=\"nonzero\" \
             stroke=\"#ced1d9\" stroke-width=\"0.02\" stroke-opacity=\"0.5\" \
             stroke-linecap=\"round\" stroke-linejoin=\"round\"/>\n\
             </g>\n</svg>\n"
        );
        assert_eq!(svg, want);
        assert_eq!(
            d_of(&fixture()),
            D,
            "the helper agrees with the pinned grammar"
        );
    }

    // AC8 / §2.8 — `fill = None` writes `fill="none"` and omits
    // `fill-opacity` and `fill-rule`.
    #[test]
    fn a_stroke_only_path_writes_fill_none_and_no_fill_attributes() {
        assert_eq!(
            only_path_line("r0012_ac8_stroke_only", stroked(STEEL, 0.02, 0.5)),
            format!(
                "<path d=\"{D}\" fill=\"none\" stroke=\"#ced1d9\" stroke-width=\"0.02\" \
                 stroke-opacity=\"0.5\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/>"
            )
        );
    }

    // AC8 / §2.8 — a path whose stroke paints nothing, by the PPM guard's
    // own test (width not both finite and > 0, or `unit(alpha)` = 0),
    // writes `stroke="none"` and omits the other stroke attributes.
    #[test]
    fn a_path_whose_stroke_paints_nothing_writes_stroke_none() {
        let want = format!(
            "<path d=\"{D}\" fill=\"#e9b23f\" fill-opacity=\"0.25\" \
             fill-rule=\"nonzero\" stroke=\"none\"/>"
        );
        let cases = [
            (0.0, 1.0),
            (-1.0, 1.0),
            (f64::NAN, 1.0),
            (f64::INFINITY, 1.0),
            (0.02, 0.0),
            (0.02, f64::NAN),
            (0.02, -0.5),
        ];
        for (i, (width, alpha)) in cases.into_iter().enumerate() {
            assert_eq!(
                only_path_line(
                    &format!("r0012_ac8_no_stroke_{i}"),
                    paint(STEEL, width, alpha, solid(GOLD, 0.25))
                ),
                want,
                "width {width}, alpha {alpha}"
            );
        }
    }

    // AC8 / AC2 — with neither paint the element says so on both counts.
    #[test]
    fn neither_paint_writes_fill_none_and_stroke_none() {
        assert_eq!(
            only_path_line("r0012_ac8_neither", paint(STEEL, 0.0, 1.0, None)),
            format!("<path d=\"{D}\" fill=\"none\" stroke=\"none\"/>")
        );
    }

    // AC8 — numbers use SPEC-0003's rule, Rust's `{}` for f64 (shortest
    // round-trip, never an exponent), so `-0` stays `-0`.
    #[test]
    fn numbers_use_shortest_round_trip_display() {
        let subs = vec![Subpath::new(p(0.1 + 0.2, -0.0))
            .line_to(p(1e-7, 1e21))
            .cubic_to(p(1.0 / 3.0, 2.5), p(-7.0, 0.0), p(0.5, -0.125))];
        let svg = render_svg(
            "r0012_ac8_numbers",
            GRID,
            GRID_VIEW,
            &[path_prim(subs, stroked(WHITE, 0.02, 1.0))],
        );
        let line = path_lines(&svg)[0];
        assert!(
            line.starts_with(
                "<path d=\"M 0.30000000000000004,-0 L 0.0000001,1000000000000000000000 \
                 C 0.3333333333333333,2.5 -7,0 0.5,-0.125\" "
            ),
            "{line}"
        );
    }

    /// The d attribute of a `<path>` line, parsed back into sample points
    /// along every drawn piece (the implicit close included).
    fn samples_of_d(line: &str) -> Vec<Pt2> {
        let d = line
            .split("d=\"")
            .nth(1)
            .and_then(|rest| rest.split('"').next())
            .expect("a d attribute");
        let pt = |tok: &str| {
            let (x, y) = tok.split_once(',').expect("x,y");
            p(x.parse().expect("x"), y.parse().expect("y"))
        };
        let mut toks = d.split_whitespace();
        let (mut out, mut start, mut cur) = (Vec::new(), p(0.0, 0.0), p(0.0, 0.0));
        let ts = [0.0, 0.25, 0.5, 0.75];
        while let Some(cmd) = toks.next() {
            match cmd {
                "M" => {
                    start = pt(toks.next().expect("M point"));
                    cur = start;
                }
                "L" | "Z" => {
                    let e = if cmd == "L" {
                        pt(toks.next().expect("L point"))
                    } else {
                        start
                    };
                    out.extend(ts.map(|t| p(cur.x + t * (e.x - cur.x), cur.y + t * (e.y - cur.y))));
                    cur = e;
                }
                "C" => {
                    let (h1, h2, e) = (
                        pt(toks.next().expect("h1")),
                        pt(toks.next().expect("h2")),
                        pt(toks.next().expect("end")),
                    );
                    out.extend(ts.map(|t| cubic_at([cur, h1, h2, e], t)));
                    cur = e;
                }
                other => panic!("unexpected path command {other:?}"),
            }
        }
        out
    }

    // AC8 — PPM and SVG place a fill boundary within ±1 px of each other,
    // in the regime R-0006 AC2 covers. Points along the outline are parsed
    // back out of the SVG and mapped by SPEC-0006 §2.3's formula. In the
    // PPM, the 3 × 3 neighbourhood of each must hold both a pixel at least
    // half covered and one less than half covered: the 50 % contour passes
    // within one pixel. Both shapes are many pixels across.
    #[test]
    fn ppm_and_svg_place_a_fill_boundary_within_one_pixel() {
        let (size, view, s) = ((128, 128), (4.0, 4.0), 32.0);
        let prims = [
            path_prim(circle(p(0.25, -0.125), 1.0), filled(WHITE, 1.0)),
            path_prim(
                polygon(&[p(-1.75, 1.0), p(-0.5, 1.8), p(-1.0, 0.4)]),
                filled(WHITE, 1.0),
            ),
        ];
        let svg = render_svg("r0012_ac8_agree_svg", size, view, &prims);
        let f = render_ppm("r0012_ac8_agree_ppm", size, view, &prims);
        let mut checked = 0;
        for line in path_lines(&svg) {
            for q in samples_of_d(line) {
                let (u, v) = to_px(size, s, q);
                let (i, j) = (u.floor() as i64, v.floor() as i64);
                let window: Vec<u8> = (-1..=1)
                    .flat_map(|dy| (-1..=1).map(move |dx| (i + dx, j + dy)))
                    .filter(|&(x, y)| (0..128).contains(&x) && (0..128).contains(&y))
                    .map(|(x, y)| f.grey(x as u32, y as u32))
                    .collect();
                assert!(
                    window.iter().any(|&b| b >= 128) && window.iter().any(|&b| b < 128),
                    "no fill boundary within 1 px of SVG point {q:?} (pixel {u}, {v}): {window:?}"
                );
                checked += 1;
            }
        }
        assert!(
            checked >= 20,
            "circle and triangle outlines sampled: {checked}"
        );
    }
}

// --- AC9: determinism -------------------------------------------------------

mod ac9 {
    use super::*;

    /// SPEC-0012 §2.10's pinned, trig-free golden scene. The canvas is
    /// 320 × 180 over the default 3.2 × 1.8 view (s = 100), black
    /// background, `Camera::default()`, identity holds, every vertex at
    /// z = 0. The objects, in insertion order:
    ///
    /// 1. a circle at (−1, 0), r 0.5, filled #e9b23f, with no stroke;
    /// 2. a rounded rect at (0.2, 0), 1 × 0.8, radius 0.2, filled #141417
    ///    and stroked #ced1d9 at width 0.02 (r = 1 px);
    /// 3. a square of half-side 0.35 at (1.15, 0) with the half-side 0.15
    ///    square reversed inside it, in one path: a nonzero-rule hole,
    ///    filled #e0322a, with no stroke.
    fn golden_scene() -> Scene {
        let mut scene = Scene::new(1.0);
        scene.add(Object::planar(circle(p(-1.0, 0.0), 0.5)).with_style(filled(GOLD, 1.0)));
        scene.add(
            Object::planar(rounded_rect(p(0.2, 0.0), 1.0, 0.8, 0.2)).with_style(paint(
                STEEL,
                0.02,
                1.0,
                solid(INK, 1.0),
            )),
        );
        let mut hole = polygon(&square(1.15, 0.0, 0.35));
        hole.extend(
            polygon(&square(1.15, 0.0, 0.15))
                .iter()
                .map(Subpath::reversed),
        );
        scene.add(Object::planar(hole).with_style(filled(SIGNAL, 1.0)));
        scene
    }

    const SIZE: (u32, u32) = (320, 180);
    const VIEW: (f64, f64) = (3.2, 1.8);

    fn render_golden_ppm(name: &str, fps: f64) -> PathBuf {
        let dir = tmp_dir(name);
        let mut sink = PpmSink::with_view(&dir, SIZE, VIEW).expect("ppm sink");
        golden_scene().render(fps, &mut sink).expect("render");
        dir
    }

    fn render_golden_svg(name: &str, fps: f64) -> PathBuf {
        let dir = tmp_dir(name);
        let mut sink = SvgSink::with_view(&dir, SIZE, VIEW).expect("svg sink");
        golden_scene().render(fps, &mut sink).expect("render");
        dir
    }

    /// A trig-bearing, moving, pinhole scene: arcs, sectors and a spinning
    /// track. Same-machine identity is all AC9 claims for such content.
    fn trig_scene() -> Scene {
        let mut scene = Scene::new(1.0);
        scene.camera = Camera::pinhole(Motor3::translator(0.0, 0.0, 3.0), 1.5);
        scene.add(
            Object::planar(arc(p(-0.5, 0.0), 0.6, 0.3, 4.0)).with_style(stroked(STEEL, 0.03, 1.0)),
        );
        scene.add(
            Object::planar(sector(p(0.6, 0.1), 0.5, -0.2, 2.2))
                .with_style(paint(WHITE, 0.02, 0.8, solid(GOLD, 0.4)))
                .with_track(Track::spin(1.0, Pga3::basis(0b0011), 1.0).expect("valid spin")),
        );
        scene
    }

    // AC9 — two renders of the same scene produce byte-identical files in
    // both sinks: the pinned golden scene and a trig-bearing pinhole scene
    // in motion.
    #[test]
    fn two_renders_are_byte_identical_in_both_sinks() {
        let mut pairs = Vec::new();
        for (kind, ext) in [("ppm", "ppm"), ("svg", "svg")] {
            let dirs: Vec<PathBuf> = ["a", "b"]
                .iter()
                .map(|run| {
                    let name = format!("r0012_ac9_twice_{kind}_{run}");
                    if kind == "ppm" {
                        render_golden_ppm(&name, 3.0)
                    } else {
                        render_golden_svg(&name, 3.0)
                    }
                })
                .collect();
            pairs.push((dirs, ext));
            let trig: Vec<PathBuf> = ["a", "b"]
                .iter()
                .map(|run| {
                    let dir = tmp_dir(&format!("r0012_ac9_trig_{kind}_{run}"));
                    if kind == "ppm" {
                        let mut sink = PpmSink::with_view(&dir, SIZE, VIEW).expect("sink");
                        trig_scene().render(4.0, &mut sink).expect("render");
                    } else {
                        let mut sink = SvgSink::with_view(&dir, SIZE, VIEW).expect("sink");
                        trig_scene().render(4.0, &mut sink).expect("render");
                    }
                    dir
                })
                .collect();
            pairs.push((trig, ext));
        }
        for (dirs, ext) in pairs {
            let names = dir_names(&dirs[0]);
            assert!(!names.is_empty(), "frames were written");
            assert_eq!(names, dir_names(&dirs[1]), "same file list");
            for name in &names {
                assert!(name.ends_with(ext), "{name}");
                let (a, b) = (
                    fs::read(dirs[0].join(name)).expect("read a"),
                    fs::read(dirs[1].join(name)).expect("read b"),
                );
                assert!(a == b, "{name} differs between two renders in one process");
            }
        }
    }

    /// Decode a byte offset of the golden PPM into a pixel and a channel,
    /// so that a drift fails legibly (the R-0006 AC3 convention).
    fn locate(offset: usize) -> String {
        let head = header(SIZE).len();
        if offset < head {
            return format!("header byte {offset}");
        }
        let i = offset - head;
        let (px, channel) = (i / 3, i % 3);
        let (x, y) = (px as u32 % SIZE.0, px as u32 / SIZE.0);
        format!("pixel ({x}, {y}) channel {}", ["R", "G", "B"][channel])
    }

    /// Bless or compare one golden fixture, the `MOTOREEL_BLESS=1` scheme of
    /// SPEC-0003. A missing or empty fixture fails with the command that
    /// makes it. It is blessed once, from a reviewed implementation, and
    /// then reviewed like code.
    fn check_golden(fixture: &str, got: &[u8], describe: impl Fn(usize) -> String) {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/golden")
            .join(fixture);
        if std::env::var_os("MOTOREEL_BLESS").is_some_and(|v| v == "1") {
            fs::write(&path, got).expect("bless the golden fixture");
            eprintln!("blessed {} — review it like code", path.display());
            return;
        }
        let want = match fs::read(&path) {
            Ok(bytes) if !bytes.is_empty() => bytes,
            _ => panic!(
                "{} is missing or empty. Bless it once the implementation is \
                 reviewed: MOTOREEL_BLESS=1 cargo test -p motoreel --test \
                 r0012_paths_fills ac9, then review the frame like code \
                 (SPEC-0012 §2.10)",
                path.display()
            ),
        };
        assert_eq!(want.len(), got.len(), "{fixture}: wrong length");
        if let Some(at) = (0..got.len()).find(|&i| got[i] != want[i]) {
            panic!(
                "{fixture} drifted at byte {at} — {}: rendered {}, fixture {}",
                describe(at),
                got[at],
                want[at]
            );
        }
    }

    // AC9 — the PPM golden. Before any comparison, and before any bless, the
    // rendered frame is sanity-checked against the scene it claims to be,
    // so a bad bless cannot slip through. The checks cover the circle's
    // centre, the panel's interior and its stroke on the right side, the
    // hole and the ring.
    #[test]
    fn the_ppm_golden_matches_byte_for_byte() {
        let dir = render_golden_ppm("r0012_ac9_golden_ppm", 1.0);
        assert_eq!(dir_names(&dir), ["frame_00000.ppm"], "exactly one frame");
        let got = fs::read(dir.join("frame_00000.ppm")).expect("read the frame");
        assert_eq!(got.len(), 15 + 320 * 180 * 3, "15 header bytes + 172 800");
        let f = Ppm {
            size: SIZE,
            pixels: got[15..].to_vec(),
        };
        assert_eq!(f.at(60, 90), rgb(GOLD), "the circle's centre");
        assert_eq!(f.at(180, 90), rgb(INK), "the panel's interior");
        assert_eq!(f.at(229, 90), rgb(STEEL), "the panel's stroke, inside");
        assert_eq!(f.at(230, 90), rgb(STEEL), "the panel's stroke, outside");
        assert_eq!(f.at(275, 90), rgb(BLACK), "the hole");
        assert_eq!(f.at(250, 90), rgb(SIGNAL), "the ring");
        assert_eq!(f.at(0, 0), rgb(BLACK), "the background");
        check_golden("r0012_paths.ppm", &got, locate);
    }

    // AC9 — the SVG golden, with the same guard: three `<path>` elements in
    // insertion order, with the colours, stroke and `stroke="none"` that
    // §2.10 pins, and the hole as two closed subpaths.
    #[test]
    fn the_svg_golden_matches_byte_for_byte() {
        let dir = render_golden_svg("r0012_ac9_golden_svg", 1.0);
        assert_eq!(dir_names(&dir), ["frame_00000.svg"], "exactly one frame");
        let got = fs::read(dir.join("frame_00000.svg")).expect("read the frame");
        let text = String::from_utf8(got.clone()).expect("UTF-8");
        let paths = path_lines(&text);
        assert_eq!(paths.len(), 3, "three objects, three paths");
        assert!(paths[0].contains("fill=\"#e9b23f\"") && paths[0].ends_with("stroke=\"none\"/>"));
        assert!(
            paths[1].contains("fill=\"#141417\"")
                && paths[1].contains("stroke=\"#ced1d9\" stroke-width=\"0.02\"")
        );
        assert!(paths[2].contains("fill=\"#e0322a\"") && paths[2].ends_with("stroke=\"none\"/>"));
        assert_eq!(
            paths[2].matches(" Z").count(),
            2,
            "the hole is two closed subpaths"
        );
        assert_eq!(
            paths[2].matches("M ").count(),
            2,
            "the hole is two subpaths"
        );
        check_golden("r0012_paths.svg", &got, |at| {
            format!("line {}", text[..at].matches('\n').count() + 1)
        });
    }

    const BANNED_FLOPS: [&str; 2] = ["mul_add", "hypot"];
    const TRANSCENDENTALS: [&str; 19] = [
        "sin", "cos", "tan", "sin_cos", "exp", "exp2", "exp_m1", "ln", "ln_1p", "log", "log2",
        "log10", "powf", "atan", "atan2", "asin", "acos", "sinh", "cbrt",
    ];

    fn calls(code: &str, name: &str) -> bool {
        code.contains(&format!(".{name}(")) || code.contains(&format!("::{name}("))
    }

    // AC9 / §2.10 — the scoped grep: no `mul_add` and no `hypot` in the new
    // path, flattening and fill code, wherever it lives. The scope is
    // `path.rs`, `shapes.rs`, `scene.rs`, and `ppm.rs` minus
    // `blend_pixel`, the R-0009 text path's one recorded exception. The
    // grep is over code, with comments and strings scrubbed.
    #[test]
    fn no_mul_add_or_hypot_in_path_flattening_or_fill_code() {
        let scoped = [
            ("path.rs", scrub(&src("path.rs"))),
            ("shapes.rs", scrub(&src("shapes.rs"))),
            ("scene.rs", scrub(&src("scene.rs"))),
            (
                "ppm.rs (minus blend_pixel)",
                without_fns(&scrub(&src("ppm.rs")), "blend_pixel"),
            ),
        ];
        for (name, code) in &scoped {
            for banned in BANNED_FLOPS {
                assert!(
                    !code.contains(banned),
                    "{name} uses `{banned}` (SPEC-0012 §2.10)"
                );
            }
        }
    }

    // AC9 / §2.10 — no transcendental function enters the raster path
    // (`ppm.rs` outside the text path), `path.rs`, or the trig-free
    // generators `circle`, `rounded_rect` and `polygon`. Trig lives in the
    // scene: `arc` and `sector` are allowed it.
    #[test]
    fn no_transcendental_in_the_raster_path_or_the_trig_free_generators() {
        let ppm = without_fns(
            &without_fns(&scrub(&src("ppm.rs")), "draw_text"),
            "blend_pixel",
        );
        let shapes = scrub(&src("shapes.rs"));
        let mut scoped = vec![
            ("ppm.rs (minus the text path)".to_string(), ppm),
            ("path.rs".to_string(), scrub(&src("path.rs"))),
        ];
        for generator in ["circle", "rounded_rect", "polygon"] {
            let bodies = fn_bodies(&shapes, generator);
            assert_eq!(bodies.len(), 1, "shapes.rs defines `fn {generator}` once");
            scoped.push((format!("shapes::{generator}"), bodies[0].clone()));
        }
        for (name, code) in &scoped {
            for f in TRANSCENDENTALS {
                assert!(!calls(code, f), "{name} calls `{f}` (SPEC-0012 §2.10)");
            }
        }
    }
}

// --- AC10: nothing already rendered moves -----------------------------------

mod ac10 {
    use super::*;

    /// FNV-1a, 64-bit: enough to pin a file's bytes with no dependency.
    fn fnv1a(bytes: &[u8]) -> u64 {
        bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| {
            (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
        })
    }

    // AC10 — every golden fixture that existed before R-0012 is
    // byte-unchanged, pinned by length and fingerprint as of this suite's
    // commit. Re-blessing one to make a suite pass fails here. The suites
    // that render against these fixtures carry the rest of AC10, and they
    // must pass unmodified: r0003 ac3, r0006 ac3, r0007 ac5, and r0004 ac6.
    #[test]
    fn every_pre_existing_golden_is_byte_unchanged() {
        let pinned: [(&str, &str, usize, u64); 3] = [
            ("frame_00000.ppm", "R-0006", 6925, 0x45af_8751_ed6a_ab48),
            ("frame_00000.svg", "R-0003", 535, 0xbb54_f0b3_0572_c3a3),
            (
                "labels_00000.svg",
                "R-0007 / R-0009",
                962,
                0x659a_6526_e1f3_86c1,
            ),
        ];
        for (file, owner, len, hash) in pinned {
            let path = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/golden")
                .join(file);
            let bytes = fs::read(&path).unwrap_or_else(|e| panic!("{owner}'s {file}: {e}"));
            assert_eq!(bytes.len(), len, "{owner}'s {file} changed length");
            assert_eq!(fnv1a(&bytes), hash, "{owner}'s {file} changed bytes");
        }
    }
}

// --- AC11: the kernel stays thin --------------------------------------------

mod ac11 {
    use super::*;

    /// The keys of a `Cargo.toml` section, in order (the R-0003 helper).
    fn section_keys(manifest: &str, section: &str) -> Vec<String> {
        let header = format!("[{section}]");
        let mut keys = Vec::new();
        let mut inside = false;
        for line in manifest.lines() {
            let line = line.trim();
            if line.starts_with('[') {
                inside = line == header;
            } else if inside && !line.is_empty() && !line.starts_with('#') {
                if let Some((key, _)) = line.split_once('=') {
                    keys.push(key.trim().to_string());
                }
            }
        }
        keys
    }

    // AC11 — zero new dependencies. The manifest declares exactly what it
    // declared before R-0012: garust, plus the optional typesetter behind
    // `text`, and proptest for dev. There are no build or target tables.
    // This asserts the manifest, as R-0007 AC7 does, because it is a claim
    // about what motoreel declares.
    #[test]
    fn the_manifest_declares_no_new_dependency() {
        let manifest = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"))
            .expect("read the crate manifest");
        assert_eq!(
            section_keys(&manifest, "dependencies"),
            ["garust", "motoreel-typeset"]
        );
        assert!(
            manifest
                .contains("motoreel-typeset = { path = \"../motoreel-typeset\", optional = true }"),
            "the typesetter stays optional"
        );
        assert!(manifest.contains("text = [\"dep:motoreel-typeset\"]"));
        assert_eq!(section_keys(&manifest, "dev-dependencies"), ["proptest"]);
        assert!(!manifest.contains("[build-dependencies]"), "no build deps");
        assert!(!manifest.contains(".dependencies]"), "no target tables");
    }

    // AC11 / §2.1 / §2.11 — paths and fills live in the core. The `path` and
    // `shapes` modules are public and not behind a feature. Both are
    // std-only, and `path.rs` names no concrete point type and imports
    // nothing from `prim.rs`, so the module graph has no cycle. Unit tests
    // are excluded from the scan.
    #[test]
    fn paths_and_shapes_are_std_only_core_modules() {
        let lib = src("lib.rs");
        let lines: Vec<&str> = lib.lines().map(str::trim).collect();
        for module in ["pub mod path;", "pub mod shapes;"] {
            let at = lines
                .iter()
                .position(|l| *l == module)
                .unwrap_or_else(|| panic!("lib.rs must declare `{module}`"));
            let before = lines[..at]
                .iter()
                .rev()
                .find(|l| !l.is_empty() && !l.starts_with("//"))
                .unwrap_or(&"");
            assert!(
                !before.starts_with("#[cfg"),
                "`{module}` must not be feature-gated"
            );
        }
        let code_of = |file: &str| {
            let code = scrub(&src(file));
            code.split("#[cfg(test)]").next().unwrap_or("").to_string()
        };
        let path_rs = code_of("path.rs");
        for banned in [
            "garust",
            "motoreel_typeset",
            "cfg(feature",
            "prim",
            "Pt2",
            "pga::",
        ] {
            assert!(
                !path_rs.contains(banned),
                "path.rs must not mention `{banned}`"
            );
        }
        let shapes_rs = code_of("shapes.rs");
        for banned in ["garust", "motoreel_typeset", "cfg(feature"] {
            assert!(
                !shapes_rs.contains(banned),
                "shapes.rs must not mention `{banned}`"
            );
        }
    }
}

// --- AC12: a demo a creator can run -----------------------------------------

mod ac12 {
    use super::*;

    const CARD_VIEW: (f64, f64) = (1.8, 3.2);
    /// The documented raster, 600 px per unit.
    const CARD_SIZE: (u32, u32) = (1080, 1920);
    const CARD_S: f64 = 600.0;
    /// The same picture at a quarter of the size (150 px per unit), so that
    /// the whole clip stays cheap to render in a test.
    const PREVIEW: (u32, u32) = (270, 480);

    fn example_file(file: &str) -> String {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("examples/card")
            .join(file);
        fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
    }

    // AC12 / §2.12 — the card is a 3 s portrait clip, and its scene view
    // matches the sink view the example documents, so `render` accepts the
    // pair.
    #[test]
    fn the_card_is_a_three_second_portrait_clip() {
        let scene = card::scene();
        assert_eq!(scene.view, CARD_VIEW, "portrait view (finding 14)");
        assert_eq!(scene.duration, 3.0, "3 s");
    }

    // AC12 — the card renders through the raster sink: every frame of the
    // 30 fps clip at preview size, frames that change over time, and one
    // frame at the full 1080 × 1920.
    #[test]
    fn the_card_renders_every_frame_through_the_raster_sink() {
        let dir = tmp_dir("r0012_ac12_preview");
        let mut sink = PpmSink::with_view(&dir, PREVIEW, CARD_VIEW).expect("sink");
        card::scene()
            .render(30.0, &mut sink)
            .expect("render the card");
        let want: Vec<String> = (0..90).map(|i| format!("frame_{i:05}.ppm")).collect();
        assert_eq!(dir_names(&dir), want, "3 s at 30 fps is 90 frames");
        let first = fs::read(dir.join(&want[0])).expect("first frame");
        let middle = fs::read(dir.join(&want[45])).expect("middle frame");
        assert!(first != middle, "something moves");

        let full = tmp_dir("r0012_ac12_full");
        let mut sink = PpmSink::with_view(&full, CARD_SIZE, CARD_VIEW).expect("sink");
        sink.frame(0, &card::scene().eval(0.0))
            .expect("a full-size frame");
        let bytes = fs::read(full.join("frame_00000.ppm")).expect("read");
        assert_eq!(bytes.len(), header(CARD_SIZE).len() + 1080 * 1920 * 3);
    }

    fn stroke_visible(style: &Style) -> bool {
        style.width.is_finite() && style.width > 0.0 && style.alpha > 0.0
    }

    /// The centre and radius of a closed all-cubic subpath whose on-curve
    /// points are equidistant from their mean: a generated circle.
    fn as_circle(sub: &Subpath<Pt2>) -> Option<(Pt2, f64)> {
        if !sub.closed || sub.segs.len() < 4 || sub.segs.iter().any(|s| matches!(s, Seg::Line(_))) {
            return None;
        }
        let pts = on_curve(sub);
        let n = (pts.len() - 1) as f64;
        let c = p(
            pts[..pts.len() - 1].iter().map(|q| q.x).sum::<f64>() / n,
            pts[..pts.len() - 1].iter().map(|q| q.y).sum::<f64>() / n,
        );
        let r = dist(pts[0], c);
        pts.iter()
            .all(|&q| (dist(q, c) - r).abs() <= 1e-9 * r.max(1.0))
            .then_some((c, r))
    }

    fn one_subpath(prim: &Prim2) -> Option<(&Subpath<Pt2>, &Style)> {
        match prim {
            Prim2::Path { subpaths, style } if subpaths.len() == 1 => Some((&subpaths[0], style)),
            _ => None,
        }
    }

    // AC12 — what the card shows, read from eval:
    //
    // - a panel, a filled rounded rectangle with a hairline stroke of at
    //   most 2 px;
    // - a friction circle, filled at low opacity and stroked;
    // - a dot that moves while staying on the friction circle's rim;
    // - a shaded area, a filled lines-only polygon, with a stroke-only curve
    //   drawn on top of it.
    #[test]
    fn the_card_shows_a_panel_a_friction_circle_a_dot_on_its_rim_and_an_area() {
        let scene = card::scene();
        let times = [0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 2.9];
        let frames: Vec<Vec<Prim2>> = times.iter().map(|&t| scene.eval(t)).collect();
        let first = &frames[0];
        assert!(
            frames.iter().all(|f| f.len() == first.len()),
            "nothing is culled as the clip plays"
        );
        let moving = |i: usize| frames.iter().any(|f| f[i] != first[i]);

        let panel = first.iter().any(|prim| {
            one_subpath(prim).is_some_and(|(s, st)| {
                st.fill.is_some()
                    && stroke_visible(st)
                    && st.width * CARD_S <= 2.0
                    && s.segs
                        .iter()
                        .filter(|g| matches!(g, Seg::Cubic(..)))
                        .count()
                        == 4
                    && s.segs.iter().filter(|g| matches!(g, Seg::Line(_))).count() >= 3
            })
        });
        assert!(panel, "a filled panel with a hairline border");

        let rims: Vec<(Pt2, f64)> = (0..first.len())
            .filter(|&i| !moving(i))
            .filter_map(|i| one_subpath(&first[i]))
            .filter(|(_, st)| st.fill.is_some_and(|f| f.alpha <= 0.5) && stroke_visible(st))
            .filter_map(|(s, _)| as_circle(s))
            .collect();
        assert!(
            !rims.is_empty(),
            "a friction circle, filled at low opacity and stroked"
        );

        let dot_on_a_rim = (0..first.len()).filter(|&i| moving(i)).any(|i| {
            let centres: Option<Vec<Pt2>> = frames
                .iter()
                .map(|f| {
                    one_subpath(&f[i])
                        .filter(|(_, st)| st.fill.is_some())
                        .and_then(|(s, _)| as_circle(s))
                        .map(|(c, _)| c)
                })
                .collect();
            centres.is_some_and(|cs| {
                rims.iter()
                    .any(|&(rc, rr)| cs.iter().all(|&c| (dist(c, rc) - rr).abs() <= 0.02 * rr))
                    && cs.windows(2).any(|w| w[0] != w[1])
            })
        });
        assert!(
            dot_on_a_rim,
            "a filled dot moving on the friction circle's rim"
        );

        let area = first.iter().position(|prim| {
            one_subpath(prim).is_some_and(|(s, st)| {
                st.fill.is_some()
                    && s.segs.len() >= 4
                    && s.segs.iter().all(|g| matches!(g, Seg::Line(_)))
            })
        });
        let area = area.expect("a shaded area: a filled polygon");
        let curve_on_top = first[area + 1..].iter().any(|prim| match prim {
            Prim2::Path { style, .. } => style.fill.is_none() && stroke_visible(style),
            Prim2::Polyline { style, .. } => stroke_visible(style),
            _ => false,
        });
        assert!(curve_on_top, "the curve is stroked on top of its area");
    }

    // AC12 — colours are passed in by the example and are not built into
    // the engine. No colour the card paints (other than white and black)
    // appears in `src/`, in hex or as an `Rgb` literal.
    #[test]
    fn the_card_colours_live_in_the_example_not_the_engine() {
        let mut colours = Vec::new();
        for prim in card::scene().eval(0.0) {
            if let Prim2::Path { style, .. } = prim {
                if let Some(f) = style.fill {
                    colours.push(f.colour);
                }
                if stroke_visible(&style) {
                    colours.push(style.stroke);
                }
            }
        }
        colours.retain(|&c| c != WHITE && c != BLACK);
        colours.dedup();
        assert!(
            colours.len() >= 2,
            "the card is not monochrome: {colours:?}"
        );

        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        for entry in fs::read_dir(&dir).expect("read src/") {
            let path = entry.expect("entry").path();
            let code: String = fs::read_to_string(&path)
                .expect("read a source file")
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect::<String>()
                .to_lowercase();
            for c in &colours {
                for needle in [
                    format!("{:02x}{:02x}{:02x}", c.r, c.g, c.b),
                    format!("r:0x{:02x},g:0x{:02x},b:0x{:02x}", c.r, c.g, c.b),
                    format!("r:{},g:{},b:{}", c.r, c.g, c.b),
                ] {
                    assert!(
                        !code.contains(&needle),
                        "{} contains the card colour {needle}",
                        path.display()
                    );
                }
            }
        }
    }

    // AC12 — the example documents its stock-ffmpeg command and the raster
    // it renders, and builds its scene from the shared `scene.rs`.
    #[test]
    fn the_card_documents_its_stock_ffmpeg_command() {
        let main_rs = example_file("main.rs");
        assert!(
            main_rs.contains("mod scene;"),
            "the demo renders the tested scene"
        );
        assert!(main_rs.contains("(1080, 1920)"), "the documented raster");
        assert!(
            main_rs.contains("ffmpeg -framerate 30 -i "),
            "the encode command"
        );
        assert!(
            main_rs.contains("frame_%05d.ppm -c:v libx264 -pix_fmt yuv420p"),
            "the stock-ffmpeg encode of the PPM frames"
        );
    }

    // AC12 — the clip encodes with stock ffmpeg, as R-0006 AC6 does it. It
    // skips, not fails, when ffmpeg or libx264 is absent. `-y -nostdin` is
    // harness hygiene, not part of the documented command.
    #[test]
    fn stock_ffmpeg_encodes_the_card() {
        let Some(ffmpeg) = ffmpeg_with_libx264() else {
            return;
        };
        let dir = tmp_dir("r0012_ac12_encode");
        let mut sink = PpmSink::with_view(&dir, PREVIEW, CARD_VIEW).expect("sink");
        card::scene().render(30.0, &mut sink).expect("render");
        let out = dir.join("card.mp4");
        let status = Command::new(&ffmpeg)
            .args(["-y", "-nostdin", "-framerate", "30", "-i"])
            .arg(dir.join("frame_%05d.ppm"))
            .args(["-c:v", "libx264", "-pix_fmt", "yuv420p"])
            .arg(&out)
            .status()
            .expect("run ffmpeg");
        assert!(status.success(), "stock ffmpeg must encode the card");
        let mp4 = fs::read(&out).expect("read the encode");
        assert_eq!(&mp4[4..8], b"ftyp", "an ISO-BMFF file, not garbage");
    }
}
