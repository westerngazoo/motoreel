//! R-0006 — `PpmSink`: the QA-owned acceptance suite (e2e).
//!
//! Loop step 3, TDD red: authored before the implementation exists, derived
//! from `requirements/0006-ppm-sink.md` (AC1–AC7) and SPEC-0006 §6's test
//! plan. Each test's header comment names the criteria it verifies.
//! Per-AC unit tests for the private mapping, coverage rule and distance
//! function live in `ppm.rs`'s own `#[cfg(test)] mod tests` — SPEC-0006 §6
//! puts them there deliberately, because AC4's identity is exact only at
//! the `f64` layer, before `round` quantizes to a byte.
//!
//! Criterion → test map:
//!
//! - AC1  `ac1_empty_frame_is_header_plus_background_bytes`,
//!   `ac1_header_text_is_exact_for_several_sizes`,
//!   `ac1_file_length_is_header_plus_w_h_3`,
//!   `ac1_filenames_are_zero_padded_five_digits`,
//!   `ac1_new_creates_nested_output_directory`,
//!   `ac1_frame_overwrites_existing_file`,
//!   `ac1_invalid_size_or_view_is_invalid_input_with_no_directory`
//! - AC2  `ac2_both_sinks_report_the_same_defaults`,
//!   `ac2_ink_lands_where_the_svg_sink_puts_it`,
//!   `ac2_letterboxed_view_leaves_background_bands`,
//!   `ac2_svg_header_carries_no_preserve_aspect_ratio`
//! - AC3  `ac3_two_renders_in_one_process_are_byte_identical`,
//!   `ac3_golden_frame_matches_the_checked_in_fixture_byte_for_byte`
//! - AC4  `ac4_unit_radius_on_a_boundary_lights_two_full_rows`,
//!   `ac4_unit_radius_off_by_half_a_pixel_lights_one_full_and_two_half`,
//!   `ac4_radius_two_lights_four_full_rows`,
//!   `ac4_frame_level_cross_section_carries_the_width_within_rounding`,
//!   `ac4_degenerate_widths_paint_nothing`,
//!   `ac4_half_alpha_white_over_black_is_exactly_128`,
//!   `ac4_a_polyline_joint_has_no_darker_seam`
//! - AC5  `ac5_every_variant_produces_ink`,
//!   `ac5_a_point_and_a_degenerate_segment_are_byte_identical`,
//!   `ac5_degenerate_primitives_leave_pure_background`,
//!   `ac5_geometry_behind_the_camera_leaves_pure_background`
//! - AC6  `ac6_stock_ffmpeg_encodes_the_frames`,
//!   `ac6_the_demo_documents_both_encode_commands`
//! - AC7  `ac7_zero_new_dependencies`
//!
//! Exactness levels follow SPEC-0006 §2.8 and are load-bearing: the header
//! and the golden are full-byte equality; the mapping is bit-exact (unit
//! tests); AC2's *grid* claim is ±1 px and holds only for `r ≥ 1` px, which
//! every primitive in that slice satisfies and each assertion restates —
//! so thinning the slice later fails loudly instead of quietly voiding the
//! claim. The frame-level width check is bounded by `n / 510` because the
//! per-channel `round` costs up to half a byte per pixel; the exact `==`
//! form of that same claim is asserted in `ppm.rs`. Which layer carries the
//! exact claim is part of the claim.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use garust::pga;
use motoreel::{FrameSink, Object, PpmSink, Prim2, Pt2, Rgb, Scene, Style, SvgSink};

// --- Shared helpers ------------------------------------------------------

/// A clean per-test scratch directory under `CARGO_TARGET_TMPDIR` (the
/// R-0003 convention): stale frames are removed so file counts are exact,
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

/// The P6 header for a raster size — the exact bytes AC1 pins.
fn header_for(size: (u32, u32)) -> Vec<u8> {
    format!("P6\n{} {}\n255\n", size.0, size.1).into_bytes()
}

/// Render one frame of `prims` straight through a sink and return the whole
/// file, header included.
fn render_prims(name: &str, size: (u32, u32), view: (f64, f64), prims: &[Prim2]) -> Vec<u8> {
    let dir = tmp_dir(name);
    let mut sink = PpmSink::with_view(&dir, size, view).expect("sink construction");
    sink.frame(0, prims).expect("write the frame");
    fs::read(dir.join("frame_00000.ppm")).expect("read the rendered frame")
}

/// The RGB triple at pixel `(x, y)` of a rendered file.
fn pixel(frame: &[u8], size: (u32, u32), x: u32, y: u32) -> [u8; 3] {
    let head = header_for(size).len();
    let i = head + (y as usize * size.0 as usize + x as usize) * 3;
    [frame[i], frame[i + 1], frame[i + 2]]
}

/// Every pixel equals `background` — "pure background", the phrase several
/// ACs use for a frame that must carry no ink at all.
fn is_pure(frame: &[u8], size: (u32, u32), background: [u8; 3]) -> bool {
    let head = header_for(size).len();
    frame[head..].chunks_exact(3).all(|p| p == background)
}

fn style(stroke: Rgb, width: f64, alpha: f64) -> Style {
    Style {
        stroke,
        width,
        alpha,
    }
}

fn pt(x: f64, y: f64) -> Pt2 {
    Pt2 { x, y }
}

/// A 64x36 raster over the default 3.2 x 1.8 view: `s = 20` px per image
/// unit **exactly** (64/3.2 and 36/1.8 are both 20.0), which is what lets
/// the AC4 tests state whole-pixel radii without a tolerance.
const SMALL: (u32, u32) = (64, 36);
const VIEW: (f64, f64) = (3.2, 1.8);
const S: f64 = 20.0;

// --- AC1: the P6 byte grammar --------------------------------------------

// AC1 — `frame(0, &[])` on a 4x2 sink is exactly the header followed by 24
// background bytes: full-file byte equality, no tolerance.
#[test]
fn ac1_empty_frame_is_header_plus_background_bytes() {
    let got = render_prims("r0006_ac1_empty", (4, 2), VIEW, &[]);
    let mut want = b"P6\n4 2\n255\n".to_vec();
    want.extend(std::iter::repeat_n(0u8, 4 * 2 * 3));
    assert!(
        got == want,
        "an empty 4x2 frame must be header + 24 zero bytes; got {} bytes",
        got.len()
    );
}

// AC1 — header text is exact at 1080p and at a non-square size.
#[test]
fn ac1_header_text_is_exact_for_several_sizes() {
    for size in [(1920, 1080), (7, 3)] {
        let got = render_prims("r0006_ac1_header", size, VIEW, &[]);
        let want = header_for(size);
        assert_eq!(
            &got[..want.len()],
            &want[..],
            "header for {size:?} must be P6, dimensions, maxval"
        );
    }
}

// AC1 — `file_len == header.len() + w*h*3` for several sizes.
#[test]
fn ac1_file_length_is_header_plus_w_h_3() {
    for size in [(1u32, 1u32), (4, 2), (64, 36), (321, 17)] {
        let got = render_prims("r0006_ac1_len", size, VIEW, &[]);
        let want = header_for(size).len() + size.0 as usize * size.1 as usize * 3;
        assert_eq!(got.len(), want, "file length for {size:?}");
    }
}

// AC1 — filenames are `frame_%05d.ppm` at indices 0 and 123.
#[test]
fn ac1_filenames_are_zero_padded_five_digits() {
    let dir = tmp_dir("r0006_ac1_names");
    let mut sink = PpmSink::with_view(&dir, (4, 2), VIEW).expect("sink");
    sink.frame(0, &[]).expect("frame 0");
    sink.frame(123, &[]).expect("frame 123");
    assert_eq!(dir_names(&dir), ["frame_00000.ppm", "frame_00123.ppm"]);
}

// AC1 — a nested output directory is created by the constructor.
#[test]
fn ac1_new_creates_nested_output_directory() {
    let dir = tmp_dir("r0006_ac1_nested").join("a/b/c");
    let _sink = PpmSink::new(&dir).expect("nested construction");
    assert!(dir.is_dir(), "constructor must create parents too");
}

// AC1 — an existing file is truncated and overwritten, never appended.
#[test]
fn ac1_frame_overwrites_existing_file() {
    let dir = tmp_dir("r0006_ac1_overwrite");
    let mut sink = PpmSink::with_view(&dir, (4, 2), VIEW).expect("sink");
    let path = dir.join("frame_00000.ppm");
    sink.frame(0, &[]).expect("first write");
    let first = fs::read(&path).expect("read first");
    fs::write(&path, vec![0xAAu8; first.len() * 3]).expect("clobber");
    sink.frame(0, &[]).expect("second write");
    assert_eq!(fs::read(&path).expect("read second"), first, "truncated");
}

// AC1 — invalid size or view is `InvalidInput`, and has no side effects:
// the directory must not exist afterwards (SPEC-0006 §2.1, §2.9).
#[test]
fn ac1_invalid_size_or_view_is_invalid_input_with_no_directory() {
    /// A rejected construction: raster size, view window, what it is.
    type Bad = ((u32, u32), (f64, f64), &'static str);
    let cases: [Bad; 6] = [
        ((0, 10), VIEW, "zero width"),
        ((10, 0), VIEW, "zero height"),
        ((10, 10), (0.0, 1.8), "zero view width"),
        ((10, 10), (3.2, -1.0), "negative view height"),
        ((10, 10), (f64::NAN, 1.8), "NaN view"),
        ((10, 10), (f64::INFINITY, 1.8), "infinite view"),
    ];
    for (i, (size, view, what)) in cases.into_iter().enumerate() {
        let dir = tmp_dir(&format!("r0006_ac1_invalid_{i}"));
        // `expect_err` would require `PpmSink: Debug`; the sink is an
        // opaque handle, so match instead of widening its API for a test.
        let err = match PpmSink::with_view(&dir, size, view) {
            Ok(_) => panic!("{what} must be rejected"),
            Err(e) => e,
        };
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput, "{what}");
        assert!(
            !dir.exists(),
            "{what}: invalid input must not create {dir:?}"
        );
    }
}

// --- AC2: the same geometry as the SVG sink ------------------------------

/// The quoted tokens of an SVG element, in order: the template is pinned,
/// so splitting on `"` is exact and needs no XML parser (R-0003 precedent).
fn quoted(line: &str) -> Vec<&str> {
    line.split('"').skip(1).step_by(2).collect()
}

// AC2(b) — the two sinks report the same default size and view, read from
// the two headers: a creator must be able to swap one for the other.
#[test]
fn ac2_both_sinks_report_the_same_defaults() {
    let svg_dir = tmp_dir("r0006_ac2_svg_default");
    let mut svg = SvgSink::new(&svg_dir).expect("svg sink");
    svg.frame(0, &[]).expect("svg frame");
    let svg_text = fs::read_to_string(svg_dir.join("frame_00000.svg")).expect("read svg");
    let head = svg_text.lines().nth(1).expect("the <svg> element");
    let attrs = quoted(head);
    // xmlns, width, height, viewBox
    assert_eq!(attrs[1], "1920", "SvgSink default width");
    assert_eq!(attrs[2], "1080", "SvgSink default height");
    assert_eq!(attrs[3], "-1.6 -0.9 3.2 1.8", "SvgSink default viewBox");

    let ppm_dir = tmp_dir("r0006_ac2_ppm_default");
    let mut ppm = PpmSink::new(&ppm_dir).expect("ppm sink");
    ppm.frame(0, &[]).expect("ppm frame");
    let bytes = fs::read(ppm_dir.join("frame_00000.ppm")).expect("read ppm");
    assert_eq!(
        &bytes[..header_for((1920, 1080)).len()],
        b"P6\n1920 1080\n255\n",
        "PpmSink must default to the same 1920x1080 raster"
    );
    assert_eq!(
        bytes.len(),
        header_for((1920, 1080)).len() + 1920 * 1080 * 3,
        "and to a full-size frame"
    );
}

// AC2(c) — one `Prim2` slice through both sinks: coordinates parsed out of
// the SVG, mapped by §2.3's formula, and asserted to carry stroke-coloured
// ink within +/-1 px, with background preserved far from every centre-line.
//
// **Every primitive here is stroked at `r >= 1` px** — §2.8's precondition
// for the +/-1 px claim. At `s = 20`, width 0.1 gives exactly `r = 1.0` px,
// and this assertion states that radius so a later thinning of the slice
// fails loudly rather than quietly invalidating the tolerance.
#[test]
fn ac2_ink_lands_where_the_svg_sink_puts_it() {
    let white = style(Rgb::WHITE, 0.1, 1.0);
    let r = 0.5 * S * white.width;
    assert!(r >= 1.0, "AC2's +/-1 px claim needs r >= 1 px; r = {r}");

    let prims = vec![
        Prim2::Segment {
            a: pt(-1.0, 0.4),
            b: pt(1.0, 0.4),
            style: white,
        },
        Prim2::Point {
            at: pt(0.6, -0.5),
            style: white,
        },
    ];

    let svg_dir = tmp_dir("r0006_ac2_svg_slice");
    let mut svg = SvgSink::with_view(&svg_dir, SMALL, VIEW).expect("svg sink");
    svg.frame(0, &prims).expect("svg frame");
    let svg_text = fs::read_to_string(svg_dir.join("frame_00000.svg")).expect("read svg");

    let frame = render_prims("r0006_ac2_ppm_slice", SMALL, VIEW, &prims);

    // Every image-space coordinate the SVG records, mapped by §2.3 and
    // checked for ink in the PPM within a Chebyshev distance of 1 px.
    let mut checked = 0;
    for line in svg_text.lines() {
        let coords: Vec<f64> = if line.starts_with("<line") {
            let q = quoted(line);
            vec![
                q[0].parse().unwrap(),
                q[1].parse().unwrap(),
                q[2].parse().unwrap(),
                q[3].parse().unwrap(),
            ]
        } else if line.starts_with("<circle") {
            let q = quoted(line);
            vec![q[0].parse().unwrap(), q[1].parse().unwrap()]
        } else {
            continue;
        };
        for xy in coords.chunks_exact(2) {
            let (u, v) = (
                f64::from(SMALL.0) / 2.0 + S * xy[0],
                f64::from(SMALL.1) / 2.0 - S * xy[1],
            );
            let (cx, cy) = (u.floor() as i64, v.floor() as i64);
            let lit = (-1..=1).any(|dy| {
                (-1..=1).any(|dx| {
                    let (x, y) = (cx + dx, cy + dy);
                    x >= 0
                        && y >= 0
                        && (x as u32) < SMALL.0
                        && (y as u32) < SMALL.1
                        && pixel(&frame, SMALL, x as u32, y as u32) != [0, 0, 0]
                })
            });
            assert!(lit, "no ink within 1 px of vertex ({}, {})", xy[0], xy[1]);
            checked += 1;
        }
    }
    assert_eq!(checked, 3, "2 line endpoints + 1 circle centre");

    // Background is preserved far from every centre-line: the top-left
    // corner is > r + 1 px from both primitives.
    assert_eq!(pixel(&frame, SMALL, 0, 0), [0, 0, 0], "far corner is clean");
}

// AC2(d) — a letterboxed view leaves background bands exactly where SVG's
// `meet` letterboxes, which proves the scale is `min` and not two scales.
#[test]
fn ac2_letterboxed_view_leaves_background_bands() {
    // 200x100 over 3.2x1.8: 200/3.2 = 62.5 but 100/1.8 = 55.55..., so the
    // height limits and vertical bands appear on the left and right.
    let size = (200, 100);
    let s = (f64::from(size.0) / VIEW.0).min(f64::from(size.1) / VIEW.1);
    assert_eq!(s, 100.0 / 1.8, "the height must limit");
    let half = s * VIEW.0 / 2.0;
    let band = (f64::from(size.0) / 2.0 - half).floor() as u32;
    assert!(band > 0, "a letterbox band must exist to be asserted");

    // A full-width horizontal rule at y = 0: ink spans the mapped view, and
    // the bands outside it stay background.
    let prims = vec![Prim2::Segment {
        a: pt(-VIEW.0 / 2.0, 0.0),
        b: pt(VIEW.0 / 2.0, 0.0),
        style: style(Rgb::WHITE, 0.1, 1.0),
    }];
    let frame = render_prims("r0006_ac2_letterbox", size, VIEW, &prims);
    let mid = size.1 / 2;
    assert_ne!(
        pixel(&frame, size, size.0 / 2, mid),
        [0, 0, 0],
        "the rule must be drawn at the centre"
    );
    assert_eq!(
        pixel(&frame, size, 0, mid),
        [0, 0, 0],
        "the left letterbox band must stay background"
    );
    assert_eq!(
        pixel(&frame, size, size.0 - 1, mid),
        [0, 0, 0],
        "the right letterbox band must stay background"
    );
}

// AC2(e) — SPEC-0013's amendment to SPEC-0003 §2.5, asserted in the
// requirement that depends on it: the SVG header carries **no**
// `preserveAspectRatio`, so the default `xMidYMid meet` is what `to_pixel`
// is the closed form of. If a future edit adds the attribute, this fails
// here rather than silently desynchronising the two sinks.
#[test]
fn ac2_svg_header_carries_no_preserve_aspect_ratio() {
    let dir = tmp_dir("r0006_ac2_par");
    let mut svg = SvgSink::new(&dir).expect("svg sink");
    svg.frame(0, &[]).expect("frame");
    let text = fs::read_to_string(dir.join("frame_00000.svg")).expect("read svg");
    assert!(
        !text.contains("preserveAspectRatio"),
        "the absent attribute is normative: PpmSink's mapping assumes the \
         xMidYMid meet default"
    );
}

// --- AC3: determinism and the golden fixture ------------------------------

/// The checked-in golden fixture. Regenerate only via
/// `MOTOREEL_BLESS=1 cargo test -p motoreel --test r0006_ppm_sink`, then
/// review the decoded diff like code (SPEC-0006 §6 AC3).
const GOLDEN: &[u8] = include_bytes!("golden/frame_00000.ppm");

/// SPEC-0006 §3's trig-free golden scene at 64x36 over the default view,
/// so `s = 20` exactly. Deliberately fat-stroked so a small fixture still
/// exercises interiors, fringes, caps, joins and alpha — and so that every
/// radius (`r` in {2.0, 3.0, 1.6, 1.2} px) sits above §2.8's `r >= 1` px
/// threshold, keeping the fixture inside the regime it demonstrates.
fn golden_scene() -> Scene {
    let white = style(Rgb::WHITE, 0.2, 1.0);
    let orange = style(
        Rgb {
            r: 0xff,
            g: 0x4d,
            b: 0x00,
        },
        0.3,
        1.0,
    );
    let teal = style(
        Rgb {
            r: 0x00,
            g: 0xb4,
            b: 0xd8,
        },
        0.16,
        0.5,
    );
    let edge = style(Rgb::WHITE, 0.12, 1.0);

    let mut scene = Scene::new(1.0);
    scene.add(
        Object::segment(
            pga::Point::new(-1.0, 0.0, 0.0),
            pga::Point::new(1.0, 0.0, 0.0),
        )
        .with_style(white),
    );
    scene.add(Object::point(pga::Point::new(0.5, 0.5, 0.0)).with_style(orange));
    scene.add(
        Object::polyline(vec![
            pga::Point::new(-0.5, -0.5, 0.0),
            pga::Point::new(0.5, -0.5, 0.0),
            pga::Point::new(0.0, 0.25, 0.0),
            pga::Point::new(-0.5, -0.5, 0.0),
        ])
        .with_style(teal),
    );
    scene.add(
        Object::edges(vec![
            (
                pga::Point::new(-1.3, 0.7, 0.0),
                pga::Point::new(-1.0, 0.7, 0.0),
            ),
            (
                pga::Point::new(1.0, 0.7, 0.0),
                pga::Point::new(1.3, 0.7, 0.0),
            ),
        ])
        .with_style(edge),
    );
    scene
}

// AC3 — two renders of the same scene in one process produce byte-identical
// files pairwise, and the expected file list.
#[test]
fn ac3_two_renders_in_one_process_are_byte_identical() {
    let base = tmp_dir("r0006_ac3_determinism");
    let (a, b) = (base.join("a"), base.join("b"));
    for dir in [&a, &b] {
        let mut sink = PpmSink::with_view(dir, SMALL, VIEW).expect("sink");
        golden_scene().render(4.0, &mut sink).expect("render");
    }
    let want: Vec<String> = (0..4).map(|i| format!("frame_{i:05}.ppm")).collect();
    assert_eq!(dir_names(&a), want, "exactly 4 frames in a/");
    assert_eq!(dir_names(&b), want, "exactly 4 frames in b/");
    for name in &want {
        let (x, y) = (
            fs::read(a.join(name)).expect("read a"),
            fs::read(b.join(name)).expect("read b"),
        );
        assert!(x == y, "{name} differs between two renders in one process");
    }
}

/// Decode a byte offset into `(x, y, channel)` — a binary golden must fail
/// legibly or it cannot be debugged (SPEC-0006 §6 AC3).
fn locate(offset: usize, size: (u32, u32)) -> String {
    let head = header_for(size).len();
    if offset < head {
        return format!("header byte {offset}");
    }
    let i = offset - head;
    let (px, channel) = (i / 3, i % 3);
    let (x, y) = (px as u32 % size.0, px as u32 / size.0);
    format!("pixel ({x}, {y}) channel {}", ["R", "G", "B"][channel])
}

// AC3 — the golden frame equals the checked-in fixture byte-for-byte, and
// its length is exactly `13 + 6912`, so a truncated fixture cannot pass by
// prefix.
#[test]
fn ac3_golden_frame_matches_the_checked_in_fixture_byte_for_byte() {
    let dir = tmp_dir("r0006_ac3_golden");
    let mut sink = PpmSink::with_view(&dir, SMALL, VIEW).expect("sink");
    golden_scene()
        .render(1.0, &mut sink)
        .expect("golden render");
    assert_eq!(dir_names(&dir), ["frame_00000.ppm"], "exactly one frame");
    let got = fs::read(dir.join("frame_00000.ppm")).expect("read the frame");

    if std::env::var_os("MOTOREEL_BLESS").is_some_and(|v| v == "1") {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/frame_00000.ppm");
        fs::write(&fixture, &got).expect("bless the golden fixture");
        eprintln!("blessed {} — review the decoded diff", fixture.display());
        return;
    }

    assert_eq!(
        got.len(),
        13 + 64 * 36 * 3,
        "a 64x36 P6 frame is 13 header bytes plus 6912 pixel bytes"
    );
    assert_eq!(GOLDEN.len(), got.len(), "the fixture is the wrong length");
    if let Some(at) = (0..got.len()).find(|&i| got[i] != GOLDEN[i]) {
        panic!(
            "the golden frame drifted at byte {at} — {}: rendered {}, fixture {}",
            locate(at, SMALL),
            got[at],
            GOLDEN[at]
        );
    }
}

// --- AC4: anti-aliasing, width, alpha -------------------------------------

/// A horizontal rule at image height `y`, stroked white at `width`.
fn rule(y: f64, width: f64) -> Vec<Prim2> {
    vec![Prim2::Segment {
        a: pt(-1.0, y),
        b: pt(1.0, y),
        style: style(Rgb::WHITE, width, 1.0),
    }]
}

/// The green channel down a column, through the middle of the raster.
fn column(frame: &[u8], size: (u32, u32), x: u32) -> Vec<u8> {
    (0..size.1).map(|y| pixel(frame, size, x, y)[1]).collect()
}

// AC4 — a `r = 1` px stroke whose centre-line falls on a pixel boundary
// lights exactly two rows at 255, with no fringe above or below.
#[test]
fn ac4_unit_radius_on_a_boundary_lights_two_full_rows() {
    // width 0.1 at s = 20 gives r = 1.0 px; y = 0 maps to pixel y = 18.0,
    // a boundary.
    let frame = render_prims("r0006_ac4_boundary", SMALL, VIEW, &rule(0.0, 0.1));
    let col = column(&frame, SMALL, 32);
    assert_eq!(col[17], 255, "row 17 full");
    assert_eq!(col[18], 255, "row 18 full");
    assert_eq!(col[16], 0, "row 16 must carry no fringe");
    assert_eq!(col[19], 0, "row 19 must carry no fringe");
}

// AC4 — shifted half a pixel, the same stroke lights one row at 255 and two
// at exactly 128 (`round(127.5)` ties away from zero).
#[test]
fn ac4_unit_radius_off_by_half_a_pixel_lights_one_full_and_two_half() {
    // pixel y = 18.5 <=> image y = -0.025 at s = 20.
    let frame = render_prims("r0006_ac4_halfshift", SMALL, VIEW, &rule(-0.025, 0.1));
    let col = column(&frame, SMALL, 32);
    assert_eq!(col[18], 255, "the centre row is full");
    assert_eq!(col[17], 128, "the row above is exactly half");
    assert_eq!(col[19], 128, "the row below is exactly half");
    assert_eq!(col[16], 0, "and nothing beyond");
    assert_eq!(col[20], 0, "and nothing beyond");
}

// AC4 — a `r = 2` px stroke lights exactly four full rows. The lit extent
// is `2r + 1` = 5 px, **not** double the `r = 1` case's 3 px: the tempting
// "doubling the width doubles the lit width" is false (§2.8).
#[test]
fn ac4_radius_two_lights_four_full_rows() {
    let frame = render_prims("r0006_ac4_r2", SMALL, VIEW, &rule(0.0, 0.2));
    let col = column(&frame, SMALL, 32);
    for (row, value) in col.iter().enumerate().take(20).skip(16) {
        assert_eq!(*value, 255, "row {row} must be full");
    }
    assert_eq!(col[15], 0, "no fringe above");
    assert_eq!(col[20], 0, "no fringe below");
}

// AC4 — the frame-level counterpart of §2.8's exact identity. The exact
// `==` form lives in `ppm.rs`'s unit tests, where coverage is still `f64`;
// here the per-channel `round` costs up to half a byte per pixel, so the
// bound is `n / 510` for an `n`-pixel cross-section. A frame-level `==`
// would be false precision.
#[test]
fn ac4_frame_level_cross_section_carries_the_width_within_rounding() {
    for width in [0.1_f64, 0.15, 0.2, 0.3] {
        let frame = render_prims("r0006_ac4_width", SMALL, VIEW, &rule(0.0, width));
        let col = column(&frame, SMALL, 32);
        let n = col.iter().filter(|&&b| b > 0).count();
        let sum: f64 = col.iter().map(|&b| f64::from(b) / 255.0).sum();
        let want = S * width;
        assert!(
            (sum - want).abs() <= n as f64 / 510.0,
            "width {width}: cross-section summed to {sum}, want {want} \
             within {}",
            n as f64 / 510.0
        );
        // The extent claim is NOT asserted here: `2r + 1` holds when the
        // centre-line sits on a pixel centre, but these rules sit on a
        // boundary (y = 0 maps to pixel y = 18.0), where an integer `r`
        // puts the two edge rows at exactly zero coverage and they drop
        // out -- 2 px at r = 1, as the boundary test asserts. The offset is
        // controlled in `ppm.rs`'s unit test; asserting one formula across
        // widths here would be false precision.
    }
}

// AC4 — `width = 0`, negative, NaN or infinite paints nothing, exactly as
// SVG draws nothing for `stroke-width="0"`. An infinite width would
// otherwise make the padded tile the whole canvas (§2.5, §2.9).
#[test]
fn ac4_degenerate_widths_paint_nothing() {
    for (i, width) in [0.0_f64, -1.0, f64::NAN, f64::INFINITY]
        .into_iter()
        .enumerate()
    {
        let frame = render_prims(
            &format!("r0006_ac4_degen_{i}"),
            SMALL,
            VIEW,
            &rule(0.0, width),
        );
        assert!(
            is_pure(&frame, SMALL, [0, 0, 0]),
            "width {width} must paint nothing"
        );
    }
    // A zero alpha is the same story from the other side.
    let invisible = vec![Prim2::Segment {
        a: pt(-1.0, 0.0),
        b: pt(1.0, 0.0),
        style: style(Rgb::WHITE, 0.2, 0.0),
    }];
    let frame = render_prims("r0006_ac4_alpha0", SMALL, VIEW, &invisible);
    assert!(is_pure(&frame, SMALL, [0, 0, 0]), "alpha 0 paints nothing");
}

// AC4 — a white stroke at alpha 0.5 over black, at full coverage, is
// exactly 128.
#[test]
fn ac4_half_alpha_white_over_black_is_exactly_128() {
    let prims = vec![Prim2::Segment {
        a: pt(-1.0, 0.0),
        b: pt(1.0, 0.0),
        style: style(Rgb::WHITE, 0.2, 0.5),
    }];
    let frame = render_prims("r0006_ac4_alpha", SMALL, VIEW, &prims);
    assert_eq!(
        pixel(&frame, SMALL, 32, 17),
        [128, 128, 128],
        "full coverage at alpha 0.5 over black"
    );
}

// AC4 — a polyline's joint shows no darker seam: coverage unions by max
// rather than accumulating, so the joint pixel matches either arm's
// interior instead of exceeding it.
#[test]
fn ac4_a_polyline_joint_has_no_darker_seam() {
    // A right angle whose corner sits at the image origin.
    let prims = vec![Prim2::Polyline {
        points: vec![pt(-0.5, 0.0), pt(0.0, 0.0), pt(0.0, -0.5)],
        style: style(Rgb::WHITE, 0.2, 0.5), // translucent: accumulation would show
    }];
    let frame = render_prims("r0006_ac4_joint", SMALL, VIEW, &prims);
    let joint = pixel(&frame, SMALL, 32, 17)[1];
    let arm = pixel(&frame, SMALL, 26, 17)[1];
    assert_eq!(
        joint, arm,
        "the joint must match an arm's interior — union by max, not sum"
    );
}

// --- AC5: the full vocabulary, culling, finiteness ------------------------

// AC5 — each of the four `Prim2` variants produces ink at its mapped
// position.
#[test]
fn ac5_every_variant_produces_ink() {
    let s = style(Rgb::WHITE, 0.2, 1.0);
    let cases: [(&str, Prim2); 4] = [
        (
            "point",
            Prim2::Point {
                at: pt(0.0, 0.0),
                style: s,
            },
        ),
        (
            "segment",
            Prim2::Segment {
                a: pt(-0.2, 0.0),
                b: pt(0.2, 0.0),
                style: s,
            },
        ),
        (
            "polyline",
            Prim2::Polyline {
                points: vec![pt(-0.2, 0.0), pt(0.2, 0.0)],
                style: s,
            },
        ),
        (
            "edges",
            Prim2::Edges {
                segments: vec![(pt(-0.2, 0.0), pt(0.2, 0.0))],
                style: s,
            },
        ),
    ];
    for (name, prim) in cases {
        let frame = render_prims(&format!("r0006_ac5_{name}"), SMALL, VIEW, &[prim]);
        assert_ne!(
            pixel(&frame, SMALL, 32, 17),
            [0, 0, 0],
            "{name} must put ink at the origin"
        );
    }
}

// AC5 — a `Point` and a degenerate `Segment` at the same place are
// byte-identical frames: both are the same disc (§2.4).
#[test]
fn ac5_a_point_and_a_degenerate_segment_are_byte_identical() {
    let s = style(Rgb::WHITE, 0.2, 1.0);
    let at = pt(0.3, -0.2);
    let point = render_prims(
        "r0006_ac5_dot",
        SMALL,
        VIEW,
        &[Prim2::Point { at, style: s }],
    );
    let degenerate = render_prims(
        "r0006_ac5_degen_seg",
        SMALL,
        VIEW,
        &[Prim2::Segment {
            a: at,
            b: at,
            style: s,
        }],
    );
    assert!(
        point == degenerate,
        "a point and a zero-length segment must rasterize identically"
    );
}

// AC5 — a `Polyline` of 0 or 1 points, and an empty `Edges`, leave pure
// background.
#[test]
fn ac5_degenerate_primitives_leave_pure_background() {
    let s = style(Rgb::WHITE, 0.2, 1.0);
    let cases: [(&str, Prim2); 3] = [
        (
            "empty_polyline",
            Prim2::Polyline {
                points: vec![],
                style: s,
            },
        ),
        (
            "one_point_polyline",
            Prim2::Polyline {
                points: vec![pt(0.0, 0.0)],
                style: s,
            },
        ),
        (
            "empty_edges",
            Prim2::Edges {
                segments: vec![],
                style: s,
            },
        ),
    ];
    for (name, prim) in cases {
        let frame = render_prims(&format!("r0006_ac5_{name}"), SMALL, VIEW, &[prim]);
        assert!(
            is_pure(&frame, SMALL, [0, 0, 0]),
            "{name} must leave pure background"
        );
    }
}

// AC5 — geometry behind the camera yields a pure-background frame: the cull
// is `Scene::eval`'s, inherited unchanged, and the sink never sees it.
#[test]
fn ac5_geometry_behind_the_camera_leaves_pure_background() {
    let mut scene = Scene::new(1.0);
    // The default camera sits at translator(0, 0, 5) looking along -z, so
    // world z = 10 is behind it (view depth -5, below Projection::NEAR).
    scene.add(
        Object::segment(
            pga::Point::new(-1.0, 0.0, 10.0),
            pga::Point::new(1.0, 0.0, 10.0),
        )
        .with_style(style(Rgb::WHITE, 0.2, 1.0)),
    );
    let dir = tmp_dir("r0006_ac5_behind");
    let mut sink = PpmSink::with_view(&dir, SMALL, VIEW).expect("sink");
    scene.render(1.0, &mut sink).expect("render");
    let frame = fs::read(dir.join("frame_00000.ppm")).expect("read frame");
    assert!(
        is_pure(&frame, SMALL, [0, 0, 0]),
        "culled geometry must leave pure background"
    );
}

// --- AC6: end to end with stock ffmpeg ------------------------------------

/// `Some(path)` when an ffmpeg with libx264 is available; `None` (with a
/// printed note) otherwise. AC6 **skips** rather than fails: a creator's
/// machine has ffmpeg, CI may not, and a missing encoder is not a defect in
/// our frames.
fn ffmpeg_with_libx264() -> Option<String> {
    let which = Command::new("which").arg("ffmpeg").output().ok()?;
    if !which.status.success() {
        eprintln!("AC6 skipped: no ffmpeg on PATH");
        return None;
    }
    let path = String::from_utf8_lossy(&which.stdout).trim().to_string();
    let encoders = Command::new(&path)
        .args(["-hide_banner", "-encoders"])
        .output()
        .ok()?;
    if !String::from_utf8_lossy(&encoders.stdout).contains("libx264") {
        eprintln!("AC6 skipped: ffmpeg at {path} has no libx264");
        return None;
    }
    Some(path)
}

// AC6 — 15 frames at 320x180 (0.25 s x 60 fps, dyadic) rendered into a
// freshly cleaned directory and passed to the documented command shape,
// **plus `-y -nostdin`** — harness hygiene, because ffmpeg otherwise
// prompts on an existing output and reads the answer from a stdin
// `cargo test` does not provide (§2.10).
#[test]
fn ac6_stock_ffmpeg_encodes_the_frames() {
    let Some(ffmpeg) = ffmpeg_with_libx264() else {
        return;
    };
    // `tmp_dir` removes the tree: §2.1 truncates but never deletes, so a
    // stale frame would otherwise join the encode.
    let dir = tmp_dir("r0006_ac6_encode");
    let mut sink = PpmSink::with_view(&dir, (320, 180), VIEW).expect("sink");
    let mut scene = golden_scene();
    scene.duration = 0.25; // dyadic: 0.25 s x 60 fps is exactly 15 frames
    scene.render(60.0, &mut sink).expect("render");
    assert_eq!(dir_names(&dir).len(), 15, "0.25 s at 60 fps is 15 frames");

    let out = dir.join("out.mp4");
    let status = Command::new(&ffmpeg)
        .args(["-y", "-nostdin", "-framerate", "60", "-i"])
        .arg(dir.join("frame_%05d.ppm"))
        .args(["-c:v", "libx264", "-pix_fmt", "yuv420p"])
        .arg(&out)
        .status()
        .expect("run ffmpeg");
    assert!(status.success(), "stock ffmpeg must decode our P6 frames");

    let mp4 = fs::read(&out).expect("read the encoded file");
    assert!(!mp4.is_empty(), "the encode must not be empty");
    assert_eq!(
        &mp4[4..8],
        b"ftyp",
        "bytes 4..8 must be the ISO-BMFF box type — this is what catches \
         an ffmpeg that exits 0 having written garbage"
    );
}

// AC6 — unconditionally (no ffmpeg required): the demo documents the PPM
// encode command, and R-0003's SVG line is still present, unchanged. The
// *documented* command carries no `-y` or `-nostdin`; those are the
// harness's, not the creator's (§2.10).
#[test]
fn ac6_the_demo_documents_both_encode_commands() {
    let main_rs = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/first_light/main.rs"),
    )
    .expect("examples/first_light/main.rs exists");
    assert!(
        main_rs.contains("ffmpeg -framerate 60 -i out/ppm/frame_%05d.ppm"),
        "the working PPM encode command must be documented with the demo"
    );
    assert!(
        main_rs.contains("ffmpeg -framerate 60 -i out/frame_%05d.svg"),
        "R-0003's SVG line must still be present, unchanged"
    );
}

// --- AC7: zero new dependencies -------------------------------------------

/// The keys of a `Cargo.toml` section, in order (the R-0003 helper, reused).
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

// AC7 — the rasterizer is `std` arithmetic and `Vec<u8>`: the dependency
// graph is unchanged.
#[test]
fn ac7_zero_new_dependencies() {
    let manifest = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"))
        .expect("read the crate manifest");
    assert_eq!(section_keys(&manifest, "dependencies"), ["garust"]);
    assert_eq!(section_keys(&manifest, "dev-dependencies"), ["proptest"]);
    assert!(!manifest.contains("[build-dependencies]"), "no build deps");
    assert!(
        !manifest.contains(".dependencies]"),
        "no target-specific dependency tables"
    );
}
