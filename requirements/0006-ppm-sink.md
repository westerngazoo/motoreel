# R-0006 — `PpmSink`: video with a stock ffmpeg

- **Status:** Draft
- **Milestone:** MC (the creator pipeline)
- **Owner:** Gustavo Delgadillo (westerngazoo)
- **Created:** 2026-08-27
- **Depends on:** R-0003 (`FrameSink`, `Scene::render`)
- **Realized by:** SPEC-0006
- **QA:** `qa` agent run scoped to this requirement

## 1. Statement

motoreel must emit **raster frames a stock `ffmpeg` can encode** with no
external rasterizer and no special build: binary P6 PPM, one file per
frame, through the existing `FrameSink` trait. The primitives an SVG
frame describes — segments, polylines, edges, points, and (with R-0007)
text — are rasterized in-crate with an anti-aliased stroke rasterizer.

## 2. Rationale

**This is a bug report, not a feature request.** The encode command
motoreel documents in its own example header does not work on an
ordinary install: ffmpeg ships an `svg_pipe` demuxer but no SVG
*decoder* unless it was built against librsvg, so
`ffmpeg -i out/frame_%05d.svg` fails with "no decoder found for: svg"
(reproduced on the owner's machine, 2026-08-27). Every creator hits this
on their first attempt. PPM is decoded by every ffmpeg build; a 30-frame
P6 sequence was verified to encode to h264 here with no extra tooling.

The raster sink was already planned for M4 as a throughput item. It is
promoted to MC because its real value is *the pipeline working at all*.

## 3. Acceptance criteria

- **AC1.** `PpmSink` implements `FrameSink`, writing valid binary P6
  files named `frame_%05d.ppm`, with the header `P6\n{w} {h}\n255\n`
  followed by exactly `w·h·3` bytes.
- **AC2.** A frame renders the same geometry the SVG sink renders:
  for a known scene, every primitive appears at the mapped pixel
  position, within the rasterizer's documented tolerance.
- **AC3.** Determinism, as R-0003: two renders of the same scene in one
  process produce **byte-identical** files, and a checked-in golden
  frame matches byte-for-byte (fixture kept trig-free and small).
- **AC4.** Strokes are anti-aliased with a documented coverage rule, and
  stroke width is honoured in image units exactly as SVG honours it.
- **AC5.** The primitive vocabulary is covered completely — point,
  segment, polyline, edges — with the same whole-primitive cull
  behaviour and never-non-finite guarantee as R-0002.
- **AC6.** **The pipeline works end to end with stock ffmpeg**: a demo
  renders to PPM and encodes to an h264 mp4 with a single documented
  command, asserted by a test that skips (not fails) when ffmpeg is
  absent, so CI stays honest on machines without it.
- **AC7.** Zero new dependencies; the rasterizer is written in-crate.

## 4. Constraints & non-goals

- No fills, gradients, blending modes, or depth sorting; strokes and
  dots only, matching what `Prim2` can express.
- No PNG, no in-crate video encoding, no audio.
- Colour is 8-bit sRGB written verbatim from `Style` — no colour
  management.
- Alpha is composited against the frame's background colour at write
  time (PPM has no alpha channel); the background becomes explicit,
  where SVG left it transparent.

## 5. Open questions

None — scope settled with the owner on 2026-08-27.

## 6. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-08-27 | Promote the raster sink from M4 to MC | It is not a performance nicety; without it there is no working path from frames to video on an ordinary machine (owner) |
| 2026-08-27 | P6 PPM rather than PNG | PNG needs a compressor (a dependency or ~500 lines of zlib); PPM is trivially correct, decoded by every ffmpeg, and the size cost is irrelevant to an intermediate the user deletes (owner) |
| 2026-08-27 | Alpha composited against an explicit background | PPM has no alpha; making the background explicit is more honest than silently flattening onto black (owner) |

## Changelog

- 2026-08-27 — created; shaped with the owner the same day.
