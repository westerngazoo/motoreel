# R-0003 — `SvgSink` and the first rendered animation

- **Status:** Met
- **Milestone:** M1
- **Owner:** Gustavo Delgadillo (westerngazoo)
- **Created:** 2026-08-20
- **Depends on:** R-0001, R-0002
- **Realized by:** SPEC-0003
- **QA:** `qa` agent run scoped to this requirement

## 1. Statement

motoreel must render a scene to **numbered SVG frame files** through a
`FrameSink` abstraction: `Scene::render(fps, sink)` walks frames, evaluates
the scene (R-0002) at each frame time, and hands the primitives to the sink.
`SvgSink` writes one `frame_%05d.svg` per frame — pure text, zero
dependencies, resolution-independent — which `ffmpeg` encodes to video
outside the crate. With this, the RFC §3.5 worked example (a square riding a
full screw while a point orbits it) becomes motoreel's first light: a real,
watchable explanatory animation.

## 2. Rationale

Frames on disk are the product. SVG first because it is diffable text —
golden-file tests become byte comparisons, honoring the determinism pledge.
(RFC-012 §3.3–§3.5, A3, and A2's golden-frame acceptance.)

## 3. Acceptance criteria

- **AC1.** A `FrameSink` trait receives `(frame index, &[Prim2])` and can
  fail with `io::Error`; `Scene::render(fps, sink)` calls it once per frame
  for `ceil(duration · fps)` frames at evenly spaced times from `t = 0`.
- **AC2.** `SvgSink` writes well-formed SVG (correct header, viewBox mapped
  from image space, one stroke element per primitive honouring
  stroke/width/alpha) to zero-padded `frame_00000.svg`-style names in its
  output directory.
- **AC3.** Golden file: one frame of a known scene matches a checked-in SVG
  **byte-for-byte**.
- **AC4.** First light: the RFC §3.5 screw demo (join-line omitted — M3),
  rendered at 60 fps for 4 s, produces 240 frames; rendering it twice
  produces bit-identical bytes.
- **AC5.** Zero new dependencies; the documented
  `ffmpeg -framerate 60 -i out/frame_%05d.svg …` invocation is recorded with
  the demo (encoding itself stays outside the crate and outside CI).

## 4. Constraints & non-goals

- No rasterization (`PpmSink` is M4/R-0008), no video encoding in-crate, no
  real-time preview.
- Frame timing is derived, not accumulated: `t_i = i / fps` — no float
  drift across long renders.

## 5. Open questions

None open.

## 6. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-08-20 | SVG before raster; golden tests are byte comparisons | Diffable text frames make determinism testable (RFC-012 §3.3) |
| 2026-08-20 | ffmpeg stays outside the crate and CI | Zero-dep discipline; encoding is the user's pipeline (RFC-012 §2) |

## Changelog

- 2026-08-20 — created; accepted.
- 2026-08-21 — implemented (SPEC-0003) and QA-signed-off: PASS, 67/67
  workspace tests, all gates raw exit 0, test suite and golden fixture
  byte-unmoved through red→green. Determinism exceeded AC4: frames from
  `cargo run --example first_light` are bit-identical to the test's own
  render — two processes, two binaries. Merge formalities pending repo
  setup (owner).
