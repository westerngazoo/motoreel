# SPEC-0007 — Anchored text labels: `Label`, `Prim2::Text`, and an embedded face

- **Status:** Draft — architect review 2026-08-27 (BLOCK → resolved) applied;
  awaiting owner acceptance
- **Realizes:** R-0007
- **Author:** Claude (engineer session)
- **Created:** 2026-08-27
- **Depends on:** SPEC-0002 (`Shape`/`Prim2`, whole-primitive cull, `Scene::eval`,
  `ObjectId`), SPEC-0003 (`FrameSink`, the SVG byte grammar, the golden/bless
  scheme), SPEC-0004 (`Prim2::Edges` — the precedent for amending the
  vocabulary additively), SPEC-0006 (`PpmSink`, `Canvas`, `src_over`,
  `to_pixel`, `Tile`)
- **Module(s):** `crates/motoreel/src/label.rs` (new),
  `crates/motoreel/src/font.rs` (new); additive amendments to `prim.rs`,
  `object.rs`, `scene.rs`, `sink.rs`, `svg.rs`, `ppm.rs`, `lib.rs`
- **Implementation gate:** R-0006 must be **`Met`** — not merely accepted —
  before any code in this spec is written. That is not a paper dependency:
  §2.9 and §2.13 consume `to_pixel`, `src_over`, `unit`, `Tile` and `Canvas`,
  and **none of them exists in the tree today**; `ppm.rs` itself does not
  exist. Until R-0006 lands, `ppm.rs` is a file this spec plans to amend and
  cannot, and three of §2.13's edits have nothing to edit. Recorded on
  `ROADMAP.md` as "blocked on R-0006 landing first"; stated here so the
  ordering is a gate rather than a note.

## 1. Motivation

R-0007 reverses a documented non-goal because the project acquired an
audience on 2026-08-27: an explanatory video that cannot name an axis, a
quantity, or a body is an animation, not an explainer. The scope the owner
settled is deliberately narrow — **one string, one line, one anchor** — and
this spec keeps it there: no layout engine, no LaTeX, no rich text
(R-0007 §4).

Three things must be true at once, and they pull against each other:

1. A label must **ride a moving body** (the `Pose` anchor), which means
   `Scene::eval` must resolve an object's track at `t` on a label's behalf —
   the first real use of `ObjectId`, reserved and inert since SPEC-0002 §2.6.
2. A label must obey **exactly** R-0002's invariants: whole-primitive cull
   (a label anchored behind the camera is *absent*, never NaN-positioned) and
   never-non-finite coordinates.
3. Both sinks must render it, and they will **not** be pixel-identical —
   SVG names a font family, PPM embeds a face. R-0007 §4 requires this be
   stated. §2.10 states it, in the form of an explicit guaranteed /
   not-guaranteed split rather than a hand-wave.

This spec also breaks one previously-stated invariant, on purpose and in
writing: SPEC-0003 §2.5 says "**No escaping machinery:** every emitted value
is a number or `#hex`… No text content exists". Text content now exists.
§2.6 amends that sentence and pins the replacement rule.

Zero new dependencies (`Cargo.toml` untouched): the face is a `const` byte
table and the escaper is four match arms.

## 2. Design

### 2.0 Module and test layout

```
crates/motoreel/src/
  label.rs                     — `Label`, `Anchor`, `ScreenAnchor`, anchor
                                 resolution, the ASCII rule
  font.rs                      — the embedded 5×7 face: one `const` table and
                                 one lookup function. No types, no traits.
  prim.rs                      — + `Align`, + `Prim2::Text`
  object.rs                    — + `ObjectId` (moved from `scene.rs`, §2.0.1);
                                 doc-comment renumber R-0007 → R-0010 (§2.14)
  scene.rs                     — + `Scene::labels`, `Scene::view`, `add_label`,
                                 the label phase of `eval`; − `ObjectId`;
                                 doc-comment renumber R-0007 → R-0010 (§2.14)
  sink.rs                      — + `FrameSink::view` (defaulted) and the
                                 `Scene::render` view-agreement check (§2.4.3)
  svg.rs                       — + the `<text>` template and the XML escaper,
                                 + a retained `view` field and `SvgSink::view`
  ppm.rs                       — + `Canvas::draw_text` (SPEC-0006 §2.12's
                                 "second entry point beside `draw`"),
                                 + a retained `view` field and `PpmSink::view`
crates/motoreel/tests/
  r0007_anchored_labels.rs     — every AC (one file per requirement)
  golden/labels_00000.svg      — the SVG text fixture
  golden/labels_00000.ppm      — the PPM text fixture (96×48)
  golden/font_specimen.ppm     — all 95 glyphs at k = 1 (96×48); mandatory (§3)
```

`lib.rs` gains `mod font; mod label;` and
`pub use label::{Anchor, Label, ScreenAnchor};`, `pub use prim::Align`. The
`ObjectId` move keeps the public path `motoreel::ObjectId` unchanged; only
the `pub use` line it appears on moves from `scene::` to `object::`.

**`font.rs` is a data module, not an abstraction.** SPEC-0006 §2.0 declined a
`raster.rs` on the rule "a second module for a single caller is the premature
abstraction §2 forbids", and that rule is right about *abstractions*. The face
introduces no type, no trait, no indirection and no second call path: it is
~100 lines of hand-authored `const` bitmap that would otherwise dominate
`ppm.rs` by line count and bury the ~60 lines of blit logic that actually need
reading. §2 asks for readable, well-structured modules; extracting the table
serves that and costs nothing. Recorded as §5 Q1 and adjudicated **yes**, since it
sits beside a rule SPEC-0006 stated in the opposite direction.

#### 2.0.1 `ObjectId` moves to `object.rs` — breaking a module cycle

`Anchor::Pose` names an `ObjectId`, so `label` must see it; `Scene` holds
`Vec<Label>`, so `scene` must see `label`. With `ObjectId` defined in
`scene.rs` that is `label → scene → label` — a circular module dependency,
which compiles in Rust and which constitution §2 forbids anyway.

`ObjectId` moves to `object.rs`, where the type it names already lives. The
resulting graph is acyclic and points inward:

```
prim   ← std only                      Pt2, Rgb, Style, Align, Prim2
object ← prim, track, garust           Shape, Object, ObjectId
camera ← prim, garust                  Camera, Projection
font   ← nothing                       the glyph table
label  ← prim, object, camera, garust  Label, Anchor, ScreenAnchor
scene  ← label, object, camera, prim   Scene, eval
sink   ← prim, scene                   FrameSink, Scene::render
svg    ← prim, sink                    SvgSink
ppm    ← prim, sink, font              PpmSink, Canvas
```

The move is invisible outside the crate (`motoreel::ObjectId` is unchanged,
and `tests/r0002_scene_camera.rs` imports it from the crate root). The
alternative — defining `Label`/`Anchor` inside `scene.rs` — avoids the move
but merges "the label vocabulary" into "the evaluation loop", roughly
doubling `scene.rs` for no gain. §5 Q2, adjudicated **yes**.

### 2.1 Where labels live in a `Scene` — a separate `labels` field

```rust
pub struct Scene {
    pub objects: Vec<Object>,
    pub labels: Vec<Label>,     // new — painted after every object
    pub camera: Camera,
    pub view: (f64, f64),       // new — §2.4.3
    pub duration: f64,
}
```

**Why not a `Shape::Text` variant on `Object`** (the tempting reuse):

- `Object` is `{ shape, style, track }` — geometry *posed by a motor track*.
  A label is not posed by a track; it is **anchored**, and one of its three
  anchor kinds (`Screen`) has no world position at all. A screen-anchored
  `Object` would have to carry a dummy identity `Track` that means nothing —
  a field that lies.
- If text lived in `Shape`, then `Object::track` and `Anchor::Pose` would be
  two different spellings of "follow this motion", with different semantics
  (one poses geometry, the other only relocates an anchor). Two ways to say
  one thing is exactly the ambiguity §2 forbids.
- `Shape` is documented as "model-space geometry as typed PGA points", and
  every variant is a point set that `project_shape` walks vertex by vertex.
  Text is not a point set, and it does **not** scale with perspective: a
  label is drawn at a fixed image size whether its anchor is 1 or 100 units
  away, whereas every `Shape` shrinks with depth. Sharing a code path would
  mean a per-variant exception inside the one function whose uniformity the
  whole cull rule rests on.
- `Prim2::Point`'s style means "a dot of radius `width/2`"; a text style
  means "fill colour and opacity". `Object::style` would mean two things.

**Why not a unified `Vec<Entry>`** (`enum Entry { Object(Object), Label(Label) }`),
which would buy interleaved draw order: it churns `Scene::objects` — a public
field with landed R-0002 acceptance tests against it — and it invalidates
`ObjectId`'s definition as an index into `objects`. Interleaving is not asked
for by R-0007 and nothing here forecloses it (§4 records the promotion path).

**Draw order is two-phase and stated:** `eval` emits **every object, then
every label**, each in insertion order. A label exists to be read; letting a
later-added object paint over it would make legibility depend on insertion
order in a way the author cannot reason about. This needs no sort, no z-index
and no comparator, so SPEC-0003 §2.5's "the sink never sorts, groups, or
dedups" survives untouched — the ordering is decided once, in `eval`, by the
order two loops run in.

`Scene::add_label(&mut self, label: Label)` returns **nothing**. `Scene::add`
returns an `ObjectId` only because R-0002 recorded it as the RFC's target API
and because derived shapes were going to need it; nothing in R-0007
references a label, so minting a `LabelId` now would be the speculative
generality §2 forbids. `labels` is public, so a future requirement that needs
one adds it additively.

### 2.2 The label vocabulary — `label.rs`

```rust
pub struct Label {
    pub text: String,     // author text; normalized to ASCII at eval (§2.5)
    pub anchor: Anchor,
    pub offset: Pt2,      // image units, applied after projection
    pub size: f64,        // em height in image units. Contract: finite, > 0
    pub align: Align,     // horizontal placement of the anchor (§2.9)
    pub style: Style,     // `stroke` is the fill colour; `width` is unused
}

pub enum Anchor {
    Point(pga::Point),                          // world point
    Pose { object: ObjectId, at: pga::Point },  // model point on an object
    Screen(ScreenAnchor),                       // fixed image position
}

/// R-0007 AC4's 3 × 3 grid, in reading order, top row first.
pub enum ScreenAnchor {
    TopLeft,    TopCentre,    TopRight,
    MidLeft,    Centre,       MidRight,
    BottomLeft, BottomCentre, BottomRight,
}
```

**`size` is R-0007 AC1's sixth field, and it is separate from `Style`.** This
spec's first draft argued for it as an *additional* field AC1 did not name;
the owner accepted the argument and **amended AC1 on 2026-08-27** to require
`size` (em height in image units) outright, with the separation from `Style`
written into the criterion and its rationale into R-0007's own decision log.
All six required fields are present. It stays out of `Style` because:

- `Style` is the *stroke* vocabulary shared by every primitive, with a
  documented contract "`width` finite and ≥ 0, in image units" that already
  means a specific thing — half of it is a radius, and SPEC-0006 §2.5 turns it
  into `radius = 0.5 · scale · width`. Reusing it as an em height would make
  one `Style` mean two different things depending on which variant carries it,
  breaking SPEC-0002 AC4's bit-exact passthrough property and requiring a
  variant test inside the rasterizer's hot guard.
- Adding a `size` field to `Style` would change a landed type carried by four
  existing variants, move `Style::default()`, and put a text concept into the
  vocabulary every non-text primitive holds.

`size` is the **em (cell) height in image units**, passed verbatim to SVG's
`font-size` and interpreted by the raster sink as the 8-font-pixel cell height
(§2.8). Default `0.08` — 48 px em, 36 px cap height at 1080p over the default
view (`s = 600`), a readable caption. `Style::width` is carried but ignored by
both sinks for text; it is retained so SPEC-0006's `style_of` stays a total
function returning one type (§2.13).

**The type is `ScreenAnchor`, a 3 × 3 grid — and it is not named `Corner`.**
This spec's first draft shipped exactly four corners on the strength of
R-0007 AC4's then-wording, and flagged the cost: *there is no way to centre a
title*, the single most likely creator request. The owner took the call and
**amended AC4 on 2026-08-27** to the nine-point grid `TopLeft · TopCentre ·
TopRight · MidLeft · Centre · MidRight · BottomLeft · BottomCentre ·
BottomRight`. The requirement now words it as nine, so the four-corner
position is dead — a spec does not get to be one draft behind the requirement
it realizes (constitution §1.2).

The rename follows from the content rather than from taste: "corner" is a
false name for `Centre` and for the three edge midpoints, and a variant
spelled `Corner::Centre` would be precisely the lying identifier §2 forbids.
Nothing structural changes. It is still a fieldless `Copy` enum resolved by one
total function over `Scene::view` (§2.4.3), it still touches no camera and no
`t`, and the four corners are four of the nine — so every corner argument in
this spec survives verbatim, with five more cases under it.

Constructors follow the house builder pattern (`Object::point(…).with_style(…)`):

```rust
impl Label {
    pub fn new(text: impl Into<String>, anchor: Anchor) -> Self;  // size 0.08,
                                    // Align::Left, Style::default()
    #[must_use] pub fn with_offset(self, offset: Pt2) -> Self;
    #[must_use] pub fn with_size(self, size: f64) -> Self;
    #[must_use] pub fn with_align(self, align: Align) -> Self;
    #[must_use] pub fn with_style(self, style: Style) -> Self;

    /// Whether every character renders as authored — `false` when §2.5's
    /// substitution would fire. Pure; the render loop stays total.
    pub fn is_ascii_renderable(&self) -> bool;
}
```

**`is_ascii_renderable` is public API that no acceptance criterion asks for.**
It is kept, because §2.5 leans on it as the answer to `'?'`'s one weakness —
that a substituted character is indistinguishable from an author-typed `?` —
and without it that argument has no exit. But a spec should not mint public
surface on its own authority: a public predicate is a compatibility commitment,
and commitments are the requirement's to make (constitution §1.2). It is
therefore recorded in §7 and flagged for **promotion into R-0007's decision
log at owner acceptance**, alongside the `size` field the owner already
ratified there. Nothing about the design depends on where the row lives; the
point is that the row exists in the requirement, not only here.

`Align` lives in `prim.rs`, not here, because it is part of the **sink-facing**
vocabulary — it tells a sink how to place a run relative to a point, which is
`prim.rs`'s stated job, and putting it in `label.rs` would recreate the
`prim ↔ label` cycle §2.0.1 just removed. It is three unit variants, so
`prim.rs` keeps its "depends on `std` alone" property.

```rust
/// Horizontal placement of a text run relative to its anchor point.
/// Maps 1:1 to SVG `text-anchor`; motoreel text is ASCII, single-line and
/// left-to-right (R-0007 §4), so Left/Center/Right are not a lie.
pub enum Align { Left, Center, Right }   // → start | middle | end
```

**Vertical placement is not an `Align` axis: the anchor sits on the text
baseline**, always. Rationale: SVG's `y` on `<text>` *is* the baseline, so
pinning to it needs no `dominant-baseline` attribute — an attribute whose
renderer support is uneven and which would be a pure source of cross-sink
divergence. Vertical adjustment is `offset.y`, which the author already has.

### 2.3 The output primitive — amending `Prim2` (SPEC-0002 §2.2)

```rust
/// A single line of printable-ASCII text pinned to one image-space point.
Text {
    /// Image position of the alignment point, on the text baseline.
    at: Pt2,
    /// The line to draw. Invariant: every byte is in `0x20..=0x7E`.
    text: String,
    /// Em height in image units. Contract: finite and > 0.
    size: f64,
    /// Horizontal placement of `at` relative to the run.
    align: Align,
    /// Fill colour and opacity; `width` is unused for text.
    style: Style,
}
```

Two invariants, both upheld structurally by `Scene::eval`:

1. **`at` is finite** — SPEC-0002 §2.2's coordinate invariant, unchanged and
   still unconditional. §2.4 shows how the offset add is guarded so it stays
   so.
2. **`text` is printable ASCII** — new, and the exact structural sibling of
   the first. Non-ASCII is normalized *once, upstream* (§2.5), so both sinks
   receive the same bytes and cannot disagree about a substitution. This is
   what makes "same presence, same string, same anchor" a property of the
   pipeline rather than a coincidence between two writers.

`size` is deliberately **not** covered by an eval-stage guard. It follows
`Style::width`'s standing precedent exactly (SPEC-0002 §2.2, qa-run decision
2026-08-20): author data carried verbatim, honoured by each sink's own guard,
never silently sanitized. A NaN `size` therefore reaches SVG as
`font-size="NaN"` — precisely as a NaN `Style::width` already reaches
`stroke-width` today — and paints nothing in PPM.

`Prim2` keeps `#[derive(Clone, Debug, PartialEq)]`; `String` and `Align`
support all three. The variant is added last, so `Prim2::Text` is the fifth
arm of every exhaustive match, and each such match becomes a compile error
until amended — which is the designed behaviour (SPEC-0006 §2.4: "a label that
silently fails to render is the failure mode this is chosen to prevent").

**Four sites, counted against the tree rather than from memory** (the first
draft named three and got both the count and the tense wrong: it missed
`r0004`'s helper entirely and listed `ppm.rs`, which does not exist yet):

| # | site | exists today? | amendment |
|---|---|---|---|
| 1 | `src/svg.rs` `write_prim` (`:82`) | yes | the new `<text>` template (§2.6) |
| 2 | `src/ppm.rs` `push_segments`, `style_of`, `Canvas::draw` | **no** — arrives with SPEC-0006 | §2.13 |
| 3 | `tests/r0002_scene_camera.rs` `prim_parts` (`:60`) | yes | one arm, exactly as R-0004 added for `Edges` — the file already carries that precedent comment verbatim |
| 4 | `tests/r0004_physics_playback.rs` `prim_parts` (`:226–237`) | yes | the same arm again. R-0004 deliberately grew a **second** copy of the helper rather than sharing one across integration binaries, and its module header at `:67–72` predicts precisely this obligation for the *other* file — so the duplicate is by design and must simply be amended twice |

Row 2 is the concrete reason the implementation gate at the top of this spec
is a gate: three of those four arms cannot be written until R-0006 is `Met`.

**`every_f64` and `Text.size` — decided, not left open.** `r0004`'s
`prim_parts` has a companion, `every_f64` (`:242`), which flattens *"every f64
an emitted primitive carries"* and is fed to the never-non-finite property at
`:1291`; `r0002` carries the same pair (`:111`, asserted at `:975`). Both
already push `style.width` and `style.alpha`, which are author passthrough data
under exactly the rule §2.3 gives `size`. **`every_f64` therefore covers
`Text.size` too.** The property those tests assert is not "the sinks reject bad
input" — it is *finite author data in ⇒ finite data out, the pipeline
manufactures no non-finite value*, and a helper named `every_f64` that skipped
one of a variant's f64s would be a lie about its own coverage. The cost is one
constraint on generators, which `style.width` already imposes: a scene fed to
this property supplies a finite `size`, exactly as it supplies a finite `width`.

Mechanically it is one line, and it does **not** widen `prim_parts`:
`every_f64` adds `if let Prim2::Text { size, .. } = prim { out.push(*size) }`
beside its existing `prim_parts` call, so `prim_parts`'s five other call sites
in `r0002` are untouched. `prim_parts` itself returns `(4, vec![*at], *style)`
for `Text` — position and style, the two things `prim_bits` needs as
determinism currency. The string and `size` are deliberately **not** folded
into `prim_bits`: byte-equality of the text is asserted directly by AC7's own
clause ("identical strings"), and pushing a `String` through a tuple built for
bit patterns would buy nothing.

### 2.4 Anchor resolution in `Scene::eval` (AC2, AC3, AC4)

`eval` gains a second phase. The object phase is **byte-for-byte the code it
is today** — no reordering, no new expression in it:

```
eval(t):
  view = camera.pose.inverse()                    # once, unchanged
  for obj in objects:                             # phase 1 — unchanged
      ...exactly as SPEC-0002 §2.6...
  for label in labels:                            # phase 2 — new
      at = label.anchor.resolve(&objects, &view, projection, self.view, t)?
      at = Pt2 { x: at.x + offset.x, y: at.y + offset.y }
      if !(at.x.is_finite() && at.y.is_finite()) { continue }   # cull
      push Prim2::Text { at, text: ascii(&label.text), size, align, style }
```

#### 2.4.1 `Anchor::Point` — AC2

`projection.project(&p.transform(&view))`. This is *the same call* geometry
makes, so the cull rule is inherited rather than restated: a `None` return —
behind the camera, at the plane, non-finite depth, or a non-finite projected
coordinate — drops the whole label. Never a NaN position, never a partial
label. There is no smaller unit than a label to cull, so "whole-primitive"
and "whole-label" are the same statement.

#### 2.4.2 `Anchor::Pose` — AC3, and the first real use of `ObjectId`

```
Anchor::Pose { object, at } =>
    let obj = objects.get(object.index())?;             // absent id → cull
    let to_view = view.compose(&obj.track.eval(t));     // pose, then view
    projection.project(&at.transform(&to_view))
```

The two-step is **character-for-character the one `project_shape` uses** —
`view.compose(&obj.track.eval(t))` then `Point::transform` then
`Projection::project`. That is not stylistic: IEEE-754 f64 is deterministic
only for a fixed expression order (SPEC-0002 §2.7), so writing it the same way
buys a bit-exact, independently checkable property, which is AC3's test:

> A `Pose`-anchored label at model point `p` on object *O* lands at the
> **bit-identical** image position as an `Object::point(p)` carrying *O*'s
> track — for every `t`.

`ObjectId` was minted by `Scene::add` in SPEC-0002 and has been inert since;
this is its first consumer. It needs one crate-internal accessor —
`pub(crate) fn index(self) -> usize` — which keeps the newtype opaque outside
the crate (R-0002's decision) while giving `label.rs` exactly the one thing it
needs. Nothing else about the type changes.

**A stale id is reachable, so it is handled, not asserted.** `Scene::add` only
ever appends (SPEC-0002 §4: no removal, no reordering), so an id obtained from
*this* scene is always in range. But `Scene`'s fields are public, so a scene
assembled by struct literal, or a label moved between scenes, can carry an id
that is not. `objects.get(i)` returns `Option` and the label is culled. A
panic or an `expect` would be wrong twice over: constitution §6 forbids
unchecked failures in library code, and this is not an unreachable state.

**Two independent evaluations, deliberately.** The label re-evaluates the
object's track rather than reading a cached pose from phase 1. `Track::eval`
is a pure, allocation-free `O(log n)` function (SPEC-0001), so the cost is a
few hundred nanoseconds per label per frame; a `Vec<Motor3>` pose cache would
allocate on every `eval` including the overwhelmingly common label-free one.
If a scene ever carries thousands of pose-anchored labels this is a
throughput matter (R-0011), not a correctness one.

**A label survives its object's cull, and that is correct.** If object *O* has
one vertex behind the camera, *O* is dropped whole (SPEC-0002 §2.5) — but a
label anchored at a model point that is *in front* still projects, and is
emitted. Cull is per primitive, and a label is its own primitive. Documented
because it will otherwise be read as a bug.

#### 2.4.3 `Anchor::Screen` — AC4's 3 × 3 grid, and the `Scene::view` seam

A screen anchor is a position in the **view window**, and the view window is a
sink constructor argument (`SvgSink::with_view`, `PpmSink::with_view`) — which
`Scene::eval` cannot see. AC4 nevertheless requires a *fixed image-space
position*. So `Scene` gains the window:

```rust
/// The image-space window the frame is expected to show — `(width, height)`,
/// centred on the origin. Must match the sink's view window; both default to
/// `(3.2, 1.8)`. Contract: finite and > 0. Only `Anchor::Screen` reads it,
/// and `Scene::render` checks it against the sink's own (§2.4.3).
pub view: (f64, f64),
```

With `hw = view.0 / 2.0` and `hh = view.1 / 2.0`, the nine points are the
outer product of three x-values with three y-values — image space is y-up, so
`Top` is `+y`:

```
                  x = -hw        x = 0.0        x = +hw
   y = +hh        TopLeft        TopCentre      TopRight
   y =  0.0       MidLeft        Centre         MidRight
   y = -hh        BottomLeft     BottomCentre   BottomRight
```

The middle row and middle column are the **literal `0.0`**, not `hh - hh` or
`hw * 0.0`: a written constant is +0.0 for every `view`, so `Centre` is
`Pt2 { x: 0.0, y: 0.0 }` exactly and cannot inherit a sign from arithmetic.
Halving is exact (exponent decrement) — SPEC-0003 §2.5 already relies on it for
the viewBox, and the golden's `-1.6 -0.9` is the witness. Resolution touches no
camera and no `t`, so AC4 holds by construction: the same `Pt2`, bit-for-bit,
for every camera pose, both projections, every time — now over nine cases
instead of four.

**The mid row emits `y="-0"`, and that is fine.** §2.6 writes the SVG `y`
attribute as `{-at.y}`, the counter-flip's price. For `MidLeft`, `Centre` and
`MidRight` with no vertical offset, `at.y` is +0.0, so the emitted attribute is
the negation of positive zero: `y="-0"`. That is legal SVG (a signed zero is a
number), it is deterministic (negation is exact and value-independent), and it
is **already precedented in blessed bytes** — R-0003's golden carries
`points="… -0,0.25 …"` today. It is not cosmetic drift to be tidied away: a
special case that mapped `-0.0` to `0` would be a value-dependent branch in the
one place §2.6 promises there is none. `labels_00000.svg` therefore contains a
`y="-0"` label in its expected bytes (§3), and AC5 keeps a `-0` case (§6).

**The honest cost: the view window is stated twice — so the disagreement is
made an error.** A `Scene::view` that disagrees with the sink's silently places
screen-anchored labels off-frame or inset, and the first draft contained that
only with defaults, a field, a three-way default-agreement test, and a named
promotion path. That containment is real but thin: **the only configuration it
exercises is `(3.2, 1.8)` — the one configuration that cannot desynchronize.**
The moment an author calls `SvgSink::with_view` and forgets `scene.view`, every
guard above is silent.

`FrameSink` therefore gains one **defaulted** method, and `Scene::render` gains
one check:

```rust
pub trait FrameSink {
    fn frame(&mut self, index: usize, prims: &[Prim2]) -> io::Result<()>;

    /// The image-space view window this sink renders, if it has one.
    ///
    /// Read **only** by [`Scene::render`], to reject a scene whose
    /// `view` disagrees — screen anchors (§2.4.3) would otherwise land
    /// off-frame with no diagnostic. It is never an input to
    /// [`Scene::eval`]: a frame stays a pure function of `(scene, t)`.
    fn view(&self) -> Option<(f64, f64)> { None }
}
```

```
render(fps, sink):
    ...the existing fps and duration checks, unchanged...
    if let Some(v) = sink.view() {
        if v != self.view { return Err(InvalidInput, "scene.view must match the sink's view window") }
    }
    for index in 0..frames { ... }        # unchanged
```

**Why this survives the objection that killed the earlier hook.** The first
draft rejected a `FrameSink::view()` because it "makes `eval(t)` and `render`
disagree about corner placement, so a frame stops being a pure function of
`(scene, t)`". That objection was aimed at a *different* design — one where the
sink's window **fed** anchor resolution. This one is a **validator, not an
input**:

- `Scene::eval` is byte-for-byte unchanged and never calls `sink.view()`. It
  still reads `self.view` and nothing else, so `eval(t)` remains a pure
  function of `(scene, t)` — R-0002 AC6's property, and every golden's
  foundation, is untouched. **No golden moves, in either sink.**
- The only reachable behaviour change is that a scene which *would have
  rendered wrong* now returns an error instead of writing files.
- It is **additive**: the default body means every existing `impl FrameSink`
  compiles unmodified, including both test doubles in
  `tests/r0003_svg_sink.rs` (the recording sink and the failing sink), which
  return `None` and are therefore exempt. This is exactly SPEC-0004's amendment
  shape on `Prim2` — extend a frozen vocabulary in the one direction that
  cannot break an existing user — applied to a trait instead of an enum.
  SPEC-0003 §2.2 froze `FrameSink`'s *obligations*; a method no implementor
  must write adds none.
- It lands in the policy already in `sink.rs`: `Scene::render` validates `fps`
  and `duration` and returns `ErrorKind::InvalidInput` **before any sink call**.
  This is a third clause on the same list, in the same place, returning the same
  kind — not a new mechanism.
- The comparison is `PartialEq` on `(f64, f64)`, so a NaN `Scene::view` never
  compares equal and is rejected. That is the safe direction, and a NaN view is
  already a contract violation (finite, > 0).

`SvgSink` and `PpmSink` each override it with `Some(self.view)` — and each
needs **one new private field to override it with**, which is worth stating
rather than assuming. `SvgSink` is `{ dir, header, buf }` today: it formats the
window straight into the `viewBox` at construction and keeps no copy.
`PpmSink`'s `Canvas` keeps `scale` and `dims`, from which a letterboxed window
is not recoverable — `s = min(W/vw, H/vh)` discards the slack dimension. Both
therefore retain the `(f64, f64)` they are already handed by `with_view`, in a
private `view` field. It is never formatted, never written, and never read by
anything but `view()`, so no golden byte can move. A sink with no view window
(a counting or recording sink) keeps the default and opts out by
construction.

The **cheaper fallback**, rejected: `SvgSink::for_scene(dir, &scene)` /
`PpmSink::for_scene(...)` constructors that read the window off the scene, so
the two can never disagree. It needs no trait change at all — but **nothing
forces a constructor's use**. `with_view` stays public, it is the spelling
already documented, and the author who reaches for it is exactly the author who
will forget `scene.view`. A guard that only fires when you opt into it does not
guard. The validator fires on the path the mistake actually takes.

Still not fixed here, and still §4's debt: the duplication itself. A single
`View` value owned by the `Scene` and *read* by the sinks is the right end
state, and it is a breaking change to two landed sink constructors — its own
requirement, not a quiet edit. What changes is that the interim is now
**detected** rather than merely defaulted.

Alternatives weighed and rejected:

- **Normalized screen-anchor coordinates** carried through to the sink. `Prim2`
  would gain a position that is sometimes image space and sometimes not — the
  ambiguity `Prim2`'s enum shape exists to prevent.
- **Passing the view into `eval`.** Changes a landed public signature with
  acceptance tests against it, for one enum variant.
- **Making the sink's window authoritative for anchor resolution.** The
  original, correctly rejected: it is the version that breaks purity.

**Ergonomic trap, documented loudly — and `TopCentre` alone is not a centred
title.** The anchor sits on the **baseline** (§2.2), so a screen anchor with a
zero offset puts the whole cap zone *outside* the frame on the top row, and the
run *starts* at the anchor rather than straddling it. Placing a run at any of
the nine points is three decisions, not one:

| grid position | `align` | `offset.x` | `offset.y` |
|---|---|---|---|
| left column (`TopLeft`, `MidLeft`, `BottomLeft`) | `Align::Left` | **positive** — inset from the edge | — |
| centre column (`TopCentre`, `Centre`, `BottomCentre`) | **`Align::Center`** | `0.0` | — |
| right column (`TopRight`, `MidRight`, `BottomRight`) | `Align::Right` | **negative** — inset from the edge | — |
| top row | — | — | **negative** — at least the cap height (`0.75 · size` in the embedded face), plus the inset you want |
| mid row | — | — | **negative ≈ `0.375 · size`** to put the cap zone's midline on the anchor, since the anchor is the baseline and not the optical centre |
| bottom row | — | — | **positive** — at least the descender depth (`0.125 · size`), plus the inset |

So the centred title an explainer actually wants is all three:

```rust
Label::new("Angular momentum", Anchor::Screen(ScreenAnchor::TopCentre))
    .with_align(Align::Center)
    .with_offset(Pt2 { x: 0.0, y: -0.25 })
```

Drop `.with_align(Align::Center)` and the title hangs to the right of centre;
drop the offset and it is above the frame. The doc comment on `ScreenAnchor`
carries this worked example and the table, because the amendment that added
`TopCentre` was made *for* this recipe and shipping the variant without it
would hand the owner half a fix. The vertical figures are the embedded face's
metrics (§2.8); SVG's resolved face has its own cap height and descender, which
is exactly what §2.10 declines to guarantee — the anchor agrees, the optical
centring is approximate.

The alternative — a `ScreenAnchor` that resolves to a safe-area inset rather
than the literal edge — is still declined: it makes the value a function of a
constant nobody voted on, and the inset is already expressible as `offset`.

### 2.5 ASCII-only text, made total and visible (AC8)

The rule, applied once in `Scene::eval`, per `char`:

```
0x20 ..= 0x7E  → kept verbatim
everything else → exactly one '?' (0x3F)
```

"Everything else" is every `char` outside printable ASCII: accented Latin,
CJK, emoji, and also `\t`, `\n`, `\r`, `\0` and `\x7F`. One `char` in, one
byte out — so the run's length in characters, and therefore its alignment and
advance, are unchanged by substitution.

**Why substitute rather than drop or fail.** Dropping is the "silent
corruption" AC8 forbids. Panicking or returning an error contradicts three
specs' standing decision that the render loop is total and error-free
(SPEC-0001, SPEC-0002 §2.8, SPEC-0003 §2.3): `eval` has no error channel and
should not grow one for a typo. A visible wrong glyph puts the failure in
front of the author on the very first frame they look at.

**Why `'?'` specifically**, and the alternatives:

| candidate | rejected because |
|---|---|
| `U+FFFD` replacement char | Not ASCII: breaks the §2.3 invariant, is not in the face, and would put multi-byte UTF-8 in the SVG that the PPM sink cannot draw — the two sinks would disagree, which is exactly what §2.10 promises they will not |
| a reserved non-printable slot (`0x7F`) with a box glyph | Draws a box in PPM and **nothing visible** in SVG: the failure would be visible in one sink only, which is worse than either extreme |
| dropping the character | Silent corruption (AC8) |
| a multi-character marker (`[?]`) | Changes the run length, so alignment and advance shift and the author cannot tell a substitution from a layout bug |

`'?'` is printable ASCII (so the alphabet stays the single contiguous range
`0x20..=0x7E`, 95 glyphs, no reserved slots), is drawn by every font, and
renders identically in both sinks. Its one weakness — indistinguishable from
an author-typed `?` — is answered by `Label::is_ascii_renderable()`, a pure
predicate an author or a future authoring layer (R-0008) can check before
rendering. The substitute character is a one-line change and one blessed
golden if the owner prefers another (§5 Q5, adjudicated **keep**).

Consequences worth writing down:

- **A combining sequence is two chars.** `"e\u{301}"` becomes `"e?"`, not
  `"?"`. There is no grapheme segmentation, because there is no layout engine
  (R-0007 §4).
- **A newline is not a line break**, it is a non-renderable character and
  becomes `'?'`. One string, one line (R-0007 §4).
- The common case is already ASCII, and the normalizer still allocates one
  `String` per label per frame. That is the same order of allocation
  `Prim2::Polyline` already makes; borrowing (`Cow<'_, str>`) is a throughput
  question (R-0011), not a correctness one.

### 2.6 The SVG `<text>` template — amending SPEC-0003 §2.5 (AC5)

One pinned element per label, attribute order fixed, default float `Display`,
style attributes always emitted, no value-dependent choice of attributes:

```
Text → <text transform="scale(1 -1)" x="{x}" y="{-y}" font-family="monospace" font-size="{size}" text-anchor="{start|middle|end}" xml:space="preserve" fill="{#hex}" fill-opacity="{a}">{escaped}</text>
```

**The per-element counter-flip is load-bearing.** The document body sits inside
`<g transform="scale(1 -1)">` (SPEC-0003 §2.5), which would render glyphs
vertically mirrored. Composing a second `scale(1 -1)` on the element makes the
glyph transform identity, so text is upright; the element's own coordinates
are then in the parent's y-down frame, which is why **`y` is emitted as
`-p.y`**. Negation is exact, deterministic, and value-independent; `-0` is
already blessed in R-0003's golden bytes.

The rejected alternative — a second, unflipped `<g>` after the first — is what
makes this decision matter: emitting it always would change the byte skeleton
of *every existing frame* and move R-0003's golden; emitting it only when
labels exist would be exactly the value-dependent branching §2.5 forbids.
**The counter-flip is the design that keeps AC5's byte-unchanged clause true**
(§2.7).

Attribute order rationale, consistent with the four landed templates:
structure (`transform`) → geometry (`x`, `y`) → font (`family`, `size`) →
placement (`text-anchor`, `xml:space`) → paint (`fill`, `fill-opacity`). Text
is *filled*, not stroked, so it takes `<circle>`'s paint pair rather than the
stroke quintet — the same reasoning SPEC-0003 used to make a dot a filled
circle.

- **`font-family="monospace"`** is a pinned generic family, never a named
  face: a generic always resolves, and a fixed-advance family is the closest
  an SVG renderer gets to a fixed-advance bitmap face, which minimises (never
  eliminates — §2.10) the divergence. A configurable family is a byte-moving
  knob to bikeshed; §5 Q6 is adjudicated **pinned**, with a configurable
  family recorded there as a later, additive option.
- **`xml:space="preserve"`** so leading, trailing and repeated spaces render.
  Without it XML whitespace handling collapses them, while the PPM sink draws
  every space literally — a gratuitous cross-sink divergence for one constant
  attribute.
- `font-size` is in user units. Inside the group flip composed with the
  element counter-flip the net transform is identity, so **user units are
  image units** and `size` is written verbatim.

**The escaping rule (normative), and the invariant it amends.** SPEC-0003 §2.5
states: "*No escaping machinery:* every emitted value is a number or `#hex` —
the metacharacters `< & " '` cannot occur. No text content exists (RFC
non-goal)." The premise is now false. The replacement text:

> Every emitted **attribute value** remains a number, a `#hex` colour, or one
> of a closed set of literal keywords (`start`/`middle`/`end`,
> `monospace`, `preserve`, `scale(1 -1)`, `none`, `round`), so no attribute
> can contain a metacharacter and attribute escaping still does not exist.
> **Element content** exists in exactly one place — `<text>` — and is escaped
> by this total, character-wise map, applied to a string already guaranteed to
> be printable ASCII (§2.3):
>
> | `char` | emitted |
> |---|---|
> | `&` (U+0026) | `&amp;` |
> | `<` (U+003C) | `&lt;` |
> | `>` (U+003E) | `&gt;` |
> | any other `char` | verbatim |

**The map is total over `char`, and the table says so.** An earlier draft wrote
the last row as "any other `0x20..=0x7E`", which contradicted its own code:
`escape_text`'s fallback is `_ => out.push(c)`, with no arm outside that range
and none possible — a `match` on `char` must be exhaustive. Writing a narrower
domain in the normative table than the code implements would leave the one
interesting case, a violated ASCII contract, formally undefined. It is defined
below instead.

Four arms, no lookup table, no general XML escaper — because the ASCII
invariant means that in practice no UTF-8 continuation byte, no control
character and no numeric character reference arises. `"` and `'` are **not**
escaped: they are legal in element content and this string never enters an
attribute. `>` is escaped unconditionally even though XML requires it only in
`]]>`, because a fixed three-character rule is auditable at a glance and cannot
construct that sequence.

**If the ASCII contract is violated, the sinks disagree — about the string.**
`Prim2::Text.text` is printable ASCII by §2.3's invariant, upheld by
`Scene::eval`. A `Prim2` slice hand-built by a test or a future caller can
break it, and it is worth being exact about what then happens, because it is
the **one** case in this spec where the two sinks differ on something §2.10
lists as *guaranteed*:

| sink | behaviour on a non-ASCII `char` |
|---|---|
| `SvgSink` | emitted **verbatim as UTF-8**. The document stays well-formed — the file is declared `encoding="UTF-8"` and the character is legal element content — so the label renders correctly |
| `PpmSink` | `draw_text` iterates `text.bytes()`, so one multi-byte `char` becomes **one `'?'` per byte**, and `text.len()` is the byte length, so the run's advance and alignment shift with it. In a debug build `font::glyph`'s `debug_assert` tripwire fires first (§2.8) |

So §2.10's "the string, byte-for-byte" row is **conditional on the ASCII
contract holding** — which is the whole reason §2.5 normalizes once, upstream,
in `eval`, rather than in each writer. The guarantee is not weakened for any
label a `Scene` can produce; it is stated conditionally so the condition is
visible rather than assumed. §2.12 carries the row on the `SvgSink` side.

**How this sits with "no value-dependent branching".** That rule protects the
*byte shape*: which elements and attributes appear must not depend on the
data, so a frame's structure is diffable and predictable. Escaping does not
change which elements or attributes appear; it changes the encoding of one
text run through a total, deterministic, character-wise map. The amended
statement is therefore: **the element and attribute skeleton is
value-independent; exactly one text run per label is value-dependent, and its
transformation is a pure function of the bytes.** Recorded in the decision log
as a change to SPEC-0003's stated invariant, not as a reinterpretation of it.

**And it is no longer this spec's gloss.** The first draft stated that
refinement here and left R-0007 AC5 as written, which is a spec quietly
redefining a requirement clause (§1.2). The owner **amended AC5 on 2026-08-27**
to carry the refinement in the requirement's own words — skeleton
value-independent, one text run value-dependent through a total pure escaping
map, and "text content cannot be emitted any other way". The paragraph above
now *realizes* AC5 rather than reinterpreting it, and the constraint is
stronger than the draft's: the escaper is the only route to element content
that exists.

### 2.7 Why R-0003's golden fixture stays byte-unchanged (AC5)

The claim is mechanical, and the test that proves it is the one already
checked in: `tests/r0003_svg_sink.rs`'s
`ac3_golden_frame_matches_the_checked_in_fixture_byte_for_byte` must pass with
`tests/golden/frame_00000.svg` untouched and **no `MOTOREEL_BLESS`
regeneration** — the same "cheapest possible proof this amendment is additive"
argument SPEC-0004 §2.3 made for `Edges`.

1. **The golden scene has no labels.** It is R-0003's three-object scene built
   through `Scene::new(1.0)`, whose new `labels` field starts empty, so
   phase 2 of `eval` runs zero iterations and the `Vec<Prim2>` is bit-identical
   to today's.
2. **Phase 1 is untouched.** The object loop's expressions, order and
   arithmetic are unchanged; the new `Scene` fields are read only by phase 2
   (`labels`) and by `Anchor::Screen` (`view`). `Vec::with_capacity` changes
   from `objects.len()` to `objects.len() + labels.len()` — capacity only,
   never bytes (SPEC-0003 §2.5's own hygiene clause).
3. **The document skeleton is not edited.** The XML declaration, the `<svg>`
   header, `<g transform="scale(1 -1)">`, the four existing primitive
   templates, and the `</g>\n</svg>\n` tail are all byte-identical. The
   counter-flip design (§2.6) is precisely what avoids adding a wrapper
   element; had text been placed outside the flipped group, this clause would
   be false.
4. **The escaper is reachable only from the new `Text` arm** of `write_prim`.
   No existing byte path passes through it.
5. **No float formatting rule changes**, no attribute is added to or reordered
   within an existing element, and `hex()` is untouched.

The same argument covers SPEC-0006's PPM golden: `Canvas::draw` gains a route
reached only by `Prim2::Text`, and the stroke path's arithmetic, guards, tile
bounds and compositing are not edited (§2.13).

### 2.8 The embedded face — `font.rs` (AC6)

**Format.** A 5 × 7 glyph bitmap on a 6 × 8 cell, covering the 95 printable
ASCII characters `0x20..=0x7E`:

```rust
/// One row per byte, low 5 bits, `0b10000` = leftmost column.
const GLYPHS: [[u8; 7]; 95] = [ /* indexed by `byte - 0x20` */ ];
```

665 bytes of `const` data. Row-major rather than the more compact column-major
`[u8; 5]` (475 bytes) **because the table is then literally ASCII art in the
source** and can be reviewed by eye — the 190-byte saving is worth nothing and
readability is a constitution §2 requirement:

```rust
[0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b00000],  // 'A'
[0b11110, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110, 0b00000],  // 'B'
[0b01111, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110, 0b00000],  // 'C'
```

**Cell geometry (pinned).** Measured from the cell top, at scale `k = 1`:

| band | extent | contents |
|---|---|---|
| rows 0..=6 | 7 rows | the glyph bitmap |
| row 7 | 1 row | line gap, always blank |
| cols 0..=4 | 5 cols | the glyph bitmap |
| col 5 | 1 col | right side bearing, always blank |

**The baseline is 6 font-pixels below the cell top** — the edge between glyph
rows 5 and 6. So cap height is 6, descender depth is 1 (rows for `g j p q y`
and the tails of `, ;`), advance is 6, em is 8. Cap height / em = 0.75, which
is close to a real font's ~0.7, so `size` means about the same thing to both
sinks (§2.10).

**Scaling: integer nearest-neighbour.**

```
k = clamp(round(size · s / CELL_H), 1, 4096)   // s = px per image unit (SPEC-0006 §2.3)
```

The divisor is `font::CELL_H`, the declared cell height, not a literal `8`.
`font.rs` declares `CELL_W = 6`, `CELL_H = 8` and `BASELINE = 6` because those
three numbers *are* the face's contract with the rasterizer; `draw_text` uses
them everywhere the geometry appears (§2.9, §3). Declaring a constant and then
hard-coding its value beside it is the dead code constitution §2 forbids, and
worse than dead — it lets the two drift. The one number that stays a literal is
the row's 5-bit width (`0..5`, `0b10000 >> c`), which is the *storage layout*
the table's own doc comment defines rather than a metric anything else reads.

`f64::round` — the same function, with the same ties-away-from-zero rule,
that SPEC-0006 §2.5 already pins for the module, so there is one rounding rule
in `ppm.rs` and not two. The upper clamp keeps the arithmetic in `u32` without
a saturating-cast surprise; a `k` of 4096 is a 32 768-px cell, past any frame.

Integer replication is chosen over any filtered scaling for a reason stronger
than "it is simpler":

- Determinism is trivial — a glyph pixel is copied, not computed. §2.7's "no
  transcendental, no accumulation" claim extends with no new argument, exactly
  as SPEC-0006 §2.12 predicted.
- **Anti-aliasing would be a no-op anyway.** With integer `k` and an integer
  pen origin (§2.9), every glyph edge lands exactly on a pixel boundary, so a
  box-filtered coverage of the mask is exactly the mask. "Glyph edges are not
  anti-aliased" is therefore not a quality compromise here; it is the exact
  answer. (It *is* a visible difference from SVG, where the renderer
  antialiases outlines — §2.10.)
- The cost is **quantization**: rendered text size is the nearest multiple of
  `8/s` image units, not `size` exactly. At the default `s = 600` that is
  0.0133 image units of granularity — under 3 % at the default size. Stated in
  §2.10, not hidden.
- The lower clamp `k ≥ 1` means a `size` below `8/s` renders **larger** than
  asked rather than vanishing. That is the same principle as §2.5's visible
  substitute: a label that is not there is the failure R-0007 exists to
  prevent, and one the author cannot see; a too-large label is one they can.

```rust
/// The glyph for `byte`. `Prim2::Text`'s invariant makes the out-of-range
/// path unreachable; it returns `'?'` rather than panicking, with a
/// `debug_assert` tripwire — the pattern `svg.rs` and `ppm.rs` already use
/// for SPEC-0002's finite-coordinate contract.
pub(crate) fn glyph(byte: u8) -> &'static [u8; 7];
```

### 2.9 Raster placement, alignment and the baseline (AC6)

All of this happens in **pixel space**, after SPEC-0006 §2.3's mapping — which
is reused verbatim, not re-derived:

```
(px, py) = to_pixel(at, scale, dims)           // SPEC-0006 §2.3, unchanged
n        = text.len()                          // == char count; ASCII
w        = CELL_W · k · n                      // advance width, px (integer)
pen_x    = match align { Left => px, Center => px - (w/2) as f64,
                         Right => px - w as f64 }
x0       = pen_x.floor() as i64                // integer pen origin
y0       = py.floor() as i64 - BASELINE · k    // cell top row
```

- **Advance width, not ink width.** `CELL_W · k · n` includes the last glyph's
  side bearing, which is what SVG's `text-anchor` centres on too; and it is
  total for `n = 0`, where an ink-width formula (`k(CELL_W·n − 1)`) would go
  negative.
- **`w/2` is always an integer** because `CELL_W` is 6 — an even number — so
  centring introduces no half-pixel of its own. Only `px` itself can be
  fractional, and `floor` resolves it.
- **`dims`, not `size`.** SPEC-0006 calls the raster dimensions `size`
  throughout `ppm.rs` — `Canvas::size`, `to_pixel(_, _, size)`,
  `Tile::around(_, _, size)`. `Prim2::Text` brings a *third* meaning of the
  word into the same module, and `draw_text` holds two of them in scope at
  once: `size: f64` (the em height) and `self.size: (u32, u32)` (the frame's
  pixels). Two live bindings one field-access apart, spelled identically and
  meaning unrelated things, is the ambiguity §2 forbids — and the compiler
  would not catch a swap between `to_pixel`'s third argument and the em height
  in a call that took both. The raster dimensions are renamed **`dims`**
  (§2.13 edit 7); the em height keeps `size`, because that is what R-0007 AC1
  and the SVG attribute call it.
- **`floor`, matching SPEC-0006's tile arithmetic**, so `ppm.rs` has one
  pixel-quantization idiom. The resulting placement error is at most 1 px,
  which lands inside the ±1 px tolerance SPEC-0006 §2.8 already documents and
  which §2.10 explicitly extends to text.
- `at` is finite by §2.3's invariant and `k ≤ 4096`, so `pen_x` is finite and
  the `as i64` casts cannot produce a surprise; the destination rectangle is
  clipped to the canvas *before* any per-pixel loop, so work is bounded by
  `canvas ∩ glyph` however far off-frame the anchor sits.

**Glyph `i`, row `r`, column `c`** covers the half-open pixel block

```
x ∈ [x0 + 6k·i + k·c,  x0 + 6k·i + k·(c+1))
y ∈ [y0 + k·r,         y0 + k·(r+1))
```

so glyph row 5 (the last cap row) occupies `[baseline − k, baseline)` — sitting
directly on the baseline — and row 6 (the descender) occupies
`[baseline, baseline + k)`. That is the baseline convention, stated in pixels.

**Compositing needs no coverage tile.** Cells are `6k` apart and only `5k`
wide, and rows within a glyph are disjoint, so **no destination pixel is
touched twice by one `Prim2::Text`**. SPEC-0006 §2.5's invariant — *exactly one
`src_over` per pixel per primitive* — therefore holds without the
`max`-unioned tile the stroke path needs. Each lit pixel takes one
`src_over(dst, style.stroke, unit(style.alpha))` with coverage exactly `1.0`.
This is precisely SPEC-0006 §2.12's forecast: **a new coverage source, not a
new compositor.**

Guards, in the SPEC-0006 §2.5 style — each matching what SVG does rather than
being defensive:

```
!(size.is_finite() && size > 0.0)  → paint nothing   (SVG ignores a bad font-size)
unit(style.alpha) == 0.0           → return early    (observationally identical)
text.is_empty()                    → return early    (zero advance, zero ink)
```

`Style::width` is not consulted, so a text label with `width = 0` renders
normally — the one place text deliberately diverges from the stroke path's
`radius > 0.0` guard, and the reason `draw` must route `Text` **before** those
guards (§2.13).

### 2.10 What the two sinks guarantee, and what they do not (R-0007 §4)

R-0007 §4 requires this be stated rather than papered over. It is split the
way SPEC-0006 §2.8 split its own tolerance — exact, bounded, and not claimed —
because conflating the three is what makes such statements rot.

**Guaranteed, exactly.** Both sinks receive the *same* `Prim2::Text`, so:

| property | why |
|---|---|
| Presence / absence of a label | Decided once, in `Scene::eval` (§2.4). Both sinks see the same slice |
| The cull decision | Same — inherited from R-0002 §2.5, not re-implemented |
| The anchor position `at`, bit-for-bit | Same `Pt2`; SPEC-0006 §2.8's mapping is exact, not approximate |
| The string, byte-for-byte after ASCII normalization — **conditional on §2.3's ASCII invariant** | Normalized upstream (§2.5), so a substitution cannot differ between sinks. The invariant is upheld by `Scene::eval`, so it holds for every label a `Scene` can produce; a hand-built `Prim2` that breaks it is the one case where the sinks disagree about the *string*, itemized in §2.6 |
| The alignment mode, nominal `size`, colour, alpha | Carried verbatim |
| Draw order: every object, then every label, insertion order within each | Decided in `eval` (§2.1); neither sink sorts |
| Byte-identical re-renders *within* a sink | §2.11 |

**Guaranteed, bounded.** The anchor's *rendered* position in the PPM sink is
within **±1 px** of `to_pixel(at)`, extending SPEC-0006 §2.8's tolerance to
text; the extra source is the `floor` to an integer pen origin (§2.9).

**Not guaranteed — the honest list.**

| not guaranteed | why |
|---|---|
| Glyph shapes | SVG names `monospace` and the renderer resolves it — differently on every machine; PPM draws the embedded 5 × 7 face |
| Advance width, and therefore total run extent | PPM's advance is exactly `6k` px; a resolved monospace face has its own |
| **Where `Center`- and `Right`-aligned runs begin** | They are placed by subtracting the run width, which differs. `Align::Left` is the alignment with the tightest cross-sink agreement — the pen origin is the anchor itself, ±1 px. `Center` diverges by half the width difference, `Right` by all of it |
| Rendered size | PPM quantizes to integer `k`, i.e. multiples of `8/s` image units, and clamps at `k = 1`; SVG honours `font-size` exactly |
| Cap height, x-height, descender depth | Only the *baseline* is a shared convention; the metrics around it are the face's |
| Glyph edge anti-aliasing | SVG renderers antialias outlines, with hinting and subpixel policies of their own; PPM's glyph mask is hard-edged by construction (§2.8) |
| Behaviour when no monospace face exists | An SVG renderer substitutes something; the PPM sink cannot |

**AC7 says less than it looks like it says, and the reading is pinned here.**
"Same scene and `t` ⇒ byte-identical frames from both sinks" means *each sink
is byte-identical across renders* — not that an SVG file and a PPM file
contain the same bytes, which is impossible for two formats. The
cross-sink claim is the table above, not byte equality.

### 2.11 Determinism (AC7)

SPEC-0003 §2.6 and SPEC-0006 §2.7 already carry the argument; text adds three
links and breaks none.

1. **Anchor resolution is a pure function of `(scene, t)`.** Every anchor kind
   reads only scene data: `Point` and `Pose` go through the same
   compose/transform/project chain as geometry; `Screen` reads `Scene::view`
   and nothing else. No clock, environment, randomness, thread, or map
   iteration is involved, and `labels` is a `Vec` iterated in insertion order.
2. **The ASCII normalizer is a total character-wise map**, so `Prim2::Text`'s
   bytes are a pure function of the author's `String`.
3. **The raster text path adds no new numeric class.** `round`, `floor`,
   `clamp`, integer multiply and compare — all IEEE-correctly-rounded or exact;
   glyph coverage is a table lookup. SPEC-0006 §2.7's stronger claim —
   *the raster stage is bit-portable across platforms* — therefore survives
   intact, and both text fixtures can be trusted on any CI host.
4. **The SVG text path** is default `Display` on two f64s plus a
   character-wise escape of an ASCII string. `-p.y` is exact.
5. **The platform caveat is unchanged and still lives in the scene, not the
   sink**: both text goldens are trig-free by SPEC-0003 §2.6's construction
   rules — single-key tracks, `Motor3::identity`/`translator` poses,
   `Camera::default()` orthographic, every vertex in `z = 0`.

### 2.12 Error and edge-case table

| Site | Condition | Behaviour |
|---|---|---|
| `eval` | `Anchor::Point` behind / at the camera plane, or projecting non-finite | Label **absent** — R-0002 §2.5's whole-primitive cull, inherited |
| `eval` | `Anchor::Pose` anchor point behind the camera | Label absent |
| `eval` | `Anchor::Pose` object culled, anchor point in front | Label **present** — cull is per primitive (§2.4.2) |
| `eval` | `Anchor::Pose { object }` id out of range | Label absent. Reachable (`Scene`'s fields are public; ids are not scene-scoped), so `Option`, never `expect` |
| `eval` | `Anchor::Screen` | Never culled; depends on neither camera nor `t`. All nine variants |
| `eval` | `Scene::view` non-finite or ≤ 0 (contract violation) | The six edge-touching anchors resolve non-finite → culled by the finite guard. **`Centre` survives**: its coordinates are the literal `0.0`, not arithmetic on `view` (§2.4.3). Point- and pose-anchored labels unaffected. Normally unreachable — `Scene::render` rejects it earlier against a sink that reports a view |
| `eval` | `label.offset` non-finite | Resolved position non-finite → label absent, keeping §2.3's coordinate invariant unconditional |
| `eval` | Non-ASCII or control character in `text` | Each such `char` → `'?'`; character count preserved; no panic, no drop (§2.5) |
| `eval` | `text` empty | A `Prim2::Text` with an empty string **is** emitted — presence is a fact about the anchor, not the content. SVG writes `<text …></text>`; PPM paints nothing |
| `eval` | `label.size` non-finite or ≤ 0 | Carried verbatim, exactly as a bad `Style::width` is (SPEC-0002 §2.2) |
| `Scene::render` | `sink.view()` is `Some(v)` and `v != scene.view` | `ErrorKind::InvalidInput`, **before frame 0** — no file written. Joins the existing `fps` / `duration` checks in the same place (§2.4.3). NaN never compares equal, so a NaN `scene.view` is rejected too |
| `Scene::render` | `sink.view()` is `None` (a sink with no view window) | No check; renders. The defaulted method is how a recording or counting sink opts out |
| `SvgSink` | `&`, `<`, `>` in text | Escaped per §2.6. `"` / `'` need no escape (element content) |
| `SvgSink` | non-ASCII `char` in text (§2.3 invariant violated by a hand-built `Prim2`) | Emitted **verbatim as UTF-8**; the document stays well-formed. `PpmSink` instead draws one `'?'` per byte, so this is the single case where the sinks disagree about the string (§2.6, §2.10) |
| `SvgSink` | non-finite or ≤ 0 `size` | Written verbatim: `font-size="NaN"`. Deterministic bytes; the `stroke-width` precedent |
| `PpmSink` | non-finite or ≤ 0 `size` | Paints nothing (§2.9) |
| `PpmSink` | `alpha` NaN or 0 | Paints nothing, via SPEC-0006's `unit` |
| `PpmSink` | `size · s / 8 < 1` | `k` clamps to 1 — renders larger than asked rather than vanishing (§2.8) |
| `PpmSink` | run partly or wholly off-canvas | Clipped before any per-pixel loop; bounded work, no ink outside the frame |
| `PpmSink` | byte outside `0x20..=0x7E` reaching `font::glyph` | Unreachable by §2.3's invariant; returns `'?'` with a `debug_assert` tripwire, never a panic |
| `Scene::add_label` | — | Infallible; no id minted (§2.1) |
| `Label` | Text longer than the frame | No wrapping, no truncation, no ellipsis; ink runs off-frame and is clipped (R-0007 §4) |

**No new error type, and no new fallible path.** Every failure mode above is
either a cull (a label is absent) or a paint-nothing (SPEC-0006 §2.5's
established guard shape). This matches the standing decision recorded in
SPEC-0001, SPEC-0002 §2.8 and SPEC-0006 §2.9: the render loop is total, and
the only fallible surface in the pipeline remains `Track` construction.

### 2.13 SPEC-0006 touchpoints — exactly what is reused, and the seam

SPEC-0006 §2.12 predicted this requirement's shape; this section is the reply,
so the architect can check both sides of the seam at once.

**None of it exists yet, and that is a scheduling constraint, not a caveat.**
`crates/motoreel/src/ppm.rs` is not in the tree: `to_pixel`, `src_over`,
`unit`, `Tile` and `Canvas` are all SPEC-0006 designs awaiting implementation.
**R-0006 must be `Met` before implementation of this spec begins** — every
"reused unchanged" item below is reused from code that has to be written first,
and the seven edits are edits to a file that has to exist first. Reversing the
order would mean re-deriving SPEC-0006's mapping here, which is exactly what
this section exists to avoid.

**Reused unchanged — not re-derived, not copied:**

1. `to_pixel(p: Pt2, scale: f64, dims: (u32, u32)) -> Px` — the normative
   world→pixel mapping (§2.3), including `s = min(W/vw, H/vh)`, the `xMidYMid
   meet` derivation, and pixel centres at `+0.5`. Its arithmetic is untouched;
   only the third parameter's *name* changes (edit 7). Text anchors map through
   it with no additional concept, exactly as §2.12 forecast ("the screen-corner
   anchor is a `Pt2` like any other").
2. `src_over(dst: &mut [u8], src: Rgb, a: f64)` — one `f64::round`,
   ties away from zero, 8-bit sRGB, `u8` framebuffer. Glyph pixels composite
   at coverage `1.0`; the compositor is untouched.
3. `unit(x: f64) -> f64` — the total `[0, 1]` clamp with NaN → 0, applied to
   `style.alpha`. **Reused by reference, not restated:** SPEC-0006's own
   architect revision is correcting this function's spelling for a clippy
   defect, and this spec pins only its *semantics* (total, NaN → 0, `[0, 1]`).
   Whatever body that revision lands is the body the text path calls — there is
   no second copy here to drift out of step, and `cargo clippy --workspace
   --all-targets -- -D warnings` is a merge gate either way.
4. `Canvas`'s pixel buffer, background fill, and per-frame reset. Text writes
   through the same `pixels` slice in the same row-major order.
5. `PpmSink` itself — header, `File::create` + two `write_all`s, filename
   rule, directory policy, buffer sizing and `try_reserve`, `with_background`.
   **`PpmSink` has no text-specific code at all.**
6. The whole determinism argument (§2.7) and the AC2 ±1 px tolerance (§2.8),
   extended in §2.10–§2.11 rather than restated.
7. The golden/bless scheme, the `(x, y, channel)` byte-diff reporter, and the
   `.gitattributes` line `crates/motoreel/tests/golden/*.ppm binary`, which
   already covers both new PPM fixtures.
8. `f64::round` as the module's single rounding rule; `mul_add` stays
   forbidden; no transcendental enters the text path either.

**What SPEC-0006's code must change — the complete list, seven edits:**

| # | site | change |
|---|---|---|
| 1 | `Canvas::draw` | Route `Prim2::Text` to `draw_text` **before** the `radius > 0.0` / `alpha` guards, which are stroke guards and would wrongly reject a text label whose `Style::width` is 0. The match lists all five variants by name — still **no `_` arm** |
| 2 | `push_segments` | One explicit `Prim2::Text { .. } => {}` arm with the reason ("text has no centre-lines; `draw` routes it before here"). An empty arm, not a wildcard, so a sixth variant is still a compile error |
| 3 | `style_of` | One `Text` arm returning its `style` |
| 4 | `Tile` | One additive constructor, `Tile::clip(x0: i64, y0: i64, x1: i64, y1: i64, dims: (u32, u32)) -> Option<Tile>`, so the glyph run's integer destination rectangle is clipped by the same code that clips stroke tiles. `Tile::around` is not refactored to use it — that would be churn in landed logic for symmetry alone |
| 5 | new | `Canvas::draw_text(&mut self, at: Pt2, text: &str, size: f64, align: Align, style: Style)` and a private `fill_block` — the "second entry point on `Canvas` beside `draw`" §2.12 anticipated |
| 6 | `Canvas::draw` → new `Canvas::draw_stroke` | **An extract-method on landed code, missed by the first draft's "complete list".** Edit 1 turns `draw` into a two-arm router, so SPEC-0006's entire `draw` body — style lookup, the `radius`/`alpha` guards, `push_segments`, the tile, the coverage pass, the composite pass — moves verbatim into `fn draw_stroke(&mut self, prim: &Prim2)`, which `draw` calls for the four stroke variants (§3 shows the result). Pure extraction: not one expression is edited, reordered, or re-derived, so SPEC-0006's stroke golden cannot move. It is listed because "the complete list" has to be complete — a reviewer diffing `ppm.rs` will see the largest hunk in the file and it must be accounted for |
| 7 | `Canvas::size` → `Canvas::dims`; `to_pixel` and `Tile::*`'s `size` parameter → `dims` | The `size` collision §2.9 describes: `Prim2::Text` brings an em height into a module where `size` already means raster dimensions, and `draw_text` holds both. A private-field and private-parameter rename — no public API, no behaviour, no golden. If SPEC-0006's own architect revision adopts `dims` first, this edit disappears; it is recorded here so it happens in exactly one of the two places |

**Not needed, and worth saying so:** the coverage tile (`Canvas::coverage`),
`distance_to_segment`, `coverage`, and the `max` union are **not** used by the
text path — glyph cells are disjoint, so there is nothing to union (§2.9).
SPEC-0006's coverage machinery is untouched, which is why its stroke golden
cannot move.

**The module-split pressure valve stays unused.** §2.12 offered extracting
`raster.rs` if glyph code made `ppm.rs` unwieldy. With the face in `font.rs`,
`ppm.rs` grows by roughly 60 lines of blit and 5 lines of routing, so the
extraction is not taken. It remains available and additive.

### 2.14 SPEC-0002 / 0003 / 0004 touchpoints

1. **`Scene::eval(&self, t: f64) -> Vec<Prim2>`** — signature unchanged; the
   label phase is appended after the object loop (SPEC-0002 §2.6).
2. **The cull rule** (SPEC-0002 §2.5) is *inherited by calling the same
   `Projection::project`*, not restated. `Projection::project` being `pub`
   (SPEC-0002 §5 Q1, adjudicated) is what lets AC2/AC3 compute the expected
   position independently.
3. **The coordinate invariant** (SPEC-0002 §2.2) stays unconditional, which is
   why the offset add is followed by a finite guard rather than trusted.
4. **`ObjectId`** was reserved in SPEC-0002 §2.6 as "the seam R-0007's derived
   shapes will reference; inert until then". That forward reference means the
   *old* R-0007 (`JoinLine`/`MeetPoint`), renumbered to **R-0010** on
   2026-08-27 — and the id's first real consumer turns out to be labels, not
   derived shapes. **Two landed doc comments still carry the old number and are
   fixed in this spec's diff**, rather than left for someone to trip over:

   | file:line | today | becomes |
   |---|---|---|
   | `src/object.rs:11` | "derived incidence shapes (`JoinLine`, `MeetPoint`) arrive with M3 (R-0007)" | "…arrive with M3 (**R-0010**)" |
   | `src/scene.rs:10` | "the seam R-0007's derived shapes will reference; **inert until then**" | "the seam **R-0010**'s derived shapes will reference; first consumed by `Anchor::Pose` (R-0007)" |

   Neither is cosmetic. `object.rs:11` points a reader at a requirement that is
   now about text, and `scene.rs:10` will be *doubly* wrong the moment this spec
   lands — wrong number and wrong tense, on a type that is no longer inert.
   `scene.rs` is being edited anyway for the `ObjectId` move (§2.0.1), so the
   line is in the diff regardless; `object.rs` is being edited to receive it.
5. **`Scene` gains public fields.** No `Scene { … }` struct literal exists
   anywhere in the tree (checked: every construction goes through
   `Scene::new`), so the change is source-compatible today. For an external
   crate it would be breaking; motoreel is `0.0.0` and unpublished.
6. **SPEC-0003 §2.5** — the float rule, colour rule, element order, `\n`
   discipline, no-indentation rule and determinism hygiene all carry over
   unchanged; §2.6 amends exactly one sentence of it (the escaping clause) and
   nothing else.
7. **SPEC-0004 §2.3 is the precedent** for the whole shape of this amendment:
   one new variant, one pinned template, an explicit "the existing golden is
   byte-unchanged" clause, and one added arm in
   `tests/r0002_scene_camera.rs`'s `prim_parts` helper — which already carries
   R-0004's comment saying exactly that. (It needs the same arm in
   `tests/r0004_physics_playback.rs` too; §2.3 counts the sites.)
8. **`sink.rs` is amended, and SPEC-0003 §2.2's freeze survives it.** `FrameSink`
   gains a **defaulted** `view()` and `Scene::render` gains a third
   `InvalidInput` clause beside the `fps` and `duration` ones it already has
   (§2.4.3). The freeze SPEC-0003 declared is on what an implementor must
   *write*: `fn frame` is untouched, no existing `impl FrameSink` in the tree or
   in a test changes by one character, and a sink with no view window opts out
   by saying nothing. `Scene::eval` does not learn the method exists.

## 3. Code outline

```rust
// prim.rs — additions to the sink-facing vocabulary (std only)

/// Horizontal placement of a text run relative to its anchor point.
///
/// Maps 1:1 to SVG `text-anchor` (`start` / `middle` / `end`). motoreel text
/// is ASCII, single-line and left-to-right (R-0007 §4), so the directional
/// names are accurate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align { Left, Center, Right }

pub enum Prim2 {
    // … Point, Segment, Polyline, Edges unchanged …

    /// A single line of printable-ASCII text pinned to one image-space point.
    ///
    /// Invariants upheld by [`Scene::eval`]: `at` is finite, and every byte
    /// of `text` lies in `0x20..=0x7E`. `size` is author data under the same
    /// passthrough rule as [`Style`] — carried verbatim, guarded by the sinks.
    Text {
        /// Image position of the alignment point, on the text baseline.
        at: Pt2,
        /// The line to draw: printable ASCII, one line, no markup.
        text: String,
        /// Em height in image units. Contract: finite and > 0.
        size: f64,
        /// Horizontal placement of `at` relative to the run.
        align: Align,
        /// Fill colour and opacity; `Style::width` is unused for text.
        style: Style,
    },
}
```

```rust
// label.rs — the label vocabulary and anchor resolution

use garust::{pga, Motor3};
use crate::camera::Projection;
use crate::object::{Object, ObjectId};
use crate::prim::{Align, Pt2, Style};

/// A plain-text label anchored to the scene (R-0007).
#[derive(Clone, Debug, PartialEq)]
pub struct Label {
    pub text: String,
    pub anchor: Anchor,
    pub offset: Pt2,
    pub size: f64,
    pub align: Align,
    pub style: Style,
}

/// What a label is pinned to.
#[derive(Clone, Debug, PartialEq)]
pub enum Anchor {
    /// A world point, projected through the camera exactly as geometry is.
    Point(pga::Point),
    /// A point in `object`'s model space, carried by that object's track —
    /// the anchor that makes a label ride a moving body.
    Pose { object: ObjectId, at: pga::Point },
    /// A fixed image-space position derived from `Scene::view`; independent
    /// of camera and of time.
    Screen(ScreenAnchor),
}

/// One of nine fixed image-space positions — R-0007 AC4's 3 × 3 grid.
/// Image space is `y`-up, so `Top` is `+y`.
///
/// **The anchor is on the text baseline, and the run starts at it.** Placing a
/// run is three decisions, not one: the grid point, the [`Align`], and an
/// `offset`. In particular `TopCentre` alone is *not* a centred title — it
/// pins the baseline to the top edge and hangs the run to the right. The
/// title an explainer wants is all three (§2.4.3):
///
/// ```
/// # use motoreel::{Align, Anchor, Label, Pt2, ScreenAnchor};
/// Label::new("Angular momentum", Anchor::Screen(ScreenAnchor::TopCentre))
///     .with_align(Align::Center)
///     .with_offset(Pt2 { x: 0.0, y: -0.25 });
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScreenAnchor {
    TopLeft,    TopCentre,    TopRight,
    MidLeft,    Centre,       MidRight,
    BottomLeft, BottomCentre, BottomRight,
}

impl Label {
    /// A label with the default size (0.08 image units), `Align::Left` and
    /// `Style::default()`.
    pub fn new(text: impl Into<String>, anchor: Anchor) -> Self;
    #[must_use] pub fn with_offset(self, offset: Pt2) -> Self;
    #[must_use] pub fn with_size(self, size: f64) -> Self;
    #[must_use] pub fn with_align(self, align: Align) -> Self;
    #[must_use] pub fn with_style(self, style: Style) -> Self;

    /// Whether every character renders as authored — `false` when §2.5's
    /// `'?'` substitution would fire. Pure; `eval` stays total.
    pub fn is_ascii_renderable(&self) -> bool {
        self.text.chars().all(is_renderable)
    }
}

impl Anchor {
    /// Resolve to an image-space position at time `t`; `None` culls the
    /// whole label (SPEC-0002 §2.5's rule, applied to a label's one vertex).
    ///
    /// `view` is `camera.pose.inverse()`, composed once per `eval`; `window`
    /// is `Scene::view` and is read only by `Screen`.
    pub(crate) fn resolve(
        &self,
        objects: &[Object],
        view: &Motor3,
        projection: Projection,
        window: (f64, f64),
        t: f64,
    ) -> Option<Pt2> {
        match self {
            Anchor::Point(p) => projection.project(&p.transform(view)),
            Anchor::Pose { object, at } => {
                // A stale id is reachable: `Scene`'s fields are public and
                // ids are not scene-scoped. Cull, never panic.
                let obj = objects.get(object.index())?;
                // Character-for-character `project_shape`'s two-step, so a
                // pose-anchored label and a `Shape::Point` on the same track
                // agree bit-for-bit (AC3).
                let to_view = view.compose(&obj.track.eval(t));
                projection.project(&at.transform(&to_view))
            }
            Anchor::Screen(a) => Some(screen_point(*a, window)),
        }
    }
}

/// Image space is y-up, so `Top` is `+y`. Halving is exact, and the middle
/// row and column are the literal `0.0` — never arithmetic on `window`, so
/// `Centre` is `(+0.0, +0.0)` for every view (§2.4.3).
fn screen_point(anchor: ScreenAnchor, window: (f64, f64)) -> Pt2 {
    let (hw, hh) = (window.0 / 2.0, window.1 / 2.0);
    match anchor {
        ScreenAnchor::TopLeft      => Pt2 { x: -hw, y:  hh },
        ScreenAnchor::TopCentre    => Pt2 { x: 0.0, y:  hh },
        ScreenAnchor::TopRight     => Pt2 { x:  hw, y:  hh },
        ScreenAnchor::MidLeft      => Pt2 { x: -hw, y: 0.0 },
        ScreenAnchor::Centre       => Pt2 { x: 0.0, y: 0.0 },
        ScreenAnchor::MidRight     => Pt2 { x:  hw, y: 0.0 },
        ScreenAnchor::BottomLeft   => Pt2 { x: -hw, y: -hh },
        ScreenAnchor::BottomCentre => Pt2 { x: 0.0, y: -hh },
        ScreenAnchor::BottomRight  => Pt2 { x:  hw, y: -hh },
    }
}

/// Printable ASCII — the alphabet the face covers and the SVG escaper assumes.
fn is_renderable(c: char) -> bool {
    matches!(c, ' '..='~')
}

/// R-0007 AC8, total: every unrenderable `char` becomes exactly one `'?'`,
/// so the run's length — and therefore its alignment and advance — is
/// unchanged. Never drops, never panics (§2.5).
pub(crate) fn to_ascii(text: &str) -> String {
    text.chars().map(|c| if is_renderable(c) { c } else { '?' }).collect()
}
```

```rust
// scene.rs — the label phase (phase 1 is untouched)

impl Scene {
    /// Append a label. Labels paint after every object, in insertion order.
    pub fn add_label(&mut self, label: Label) {
        self.labels.push(label);
    }

    pub fn eval(&self, t: f64) -> Vec<Prim2> {
        let view = self.camera.pose.inverse();
        let mut prims = Vec::with_capacity(self.objects.len() + self.labels.len());

        for obj in &self.objects {
            // … SPEC-0002 §2.6, unchanged, byte for byte …
        }

        for label in &self.labels {
            let Some(anchor) = label.anchor.resolve(
                &self.objects, &view, self.camera.projection, self.view, t,
            ) else {
                continue; // culled whole — never a NaN position (AC2)
            };
            let at = Pt2 {
                x: anchor.x + label.offset.x,
                y: anchor.y + label.offset.y,
            };
            // `offset` is author data; the guard keeps SPEC-0002 §2.2's
            // finite-coordinate invariant unconditional.
            if !(at.x.is_finite() && at.y.is_finite()) {
                continue;
            }
            prims.push(Prim2::Text {
                at,
                text: label::to_ascii(&label.text),
                size: label.size,
                align: label.align,
                style: label.style,
            });
        }
        prims
    }
}
```

```rust
// sink.rs — one defaulted trait method and one validator (§2.4.3)

pub trait FrameSink {
    fn frame(&mut self, index: usize, prims: &[Prim2]) -> io::Result<()>;

    /// The image-space view window this sink renders, if it has one.
    ///
    /// Read **only** by [`Scene::render`], to reject a scene whose `view`
    /// disagrees — screen-anchored labels would otherwise land off-frame
    /// with no diagnostic. Never an input to [`Scene::eval`]: a frame stays
    /// a pure function of `(scene, t)`, so no golden moves. Defaulted, so a
    /// sink with no view window (a recorder, a counter) opts out by saying
    /// nothing.
    fn view(&self) -> Option<(f64, f64)> {
        None
    }
}

impl Scene {
    pub fn render<S: FrameSink + ?Sized>(&self, fps: f64, sink: &mut S) -> io::Result<()> {
        // … the existing `fps` and `duration` checks, unchanged …

        // Validate before side effects — SPEC-0003 §2.4's rule, third clause.
        // `!=` on (f64, f64): a NaN `self.view` never compares equal, so a
        // view violating its own contract is rejected here too.
        if let Some(v) = sink.view() {
            if v != self.view {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "scene.view must match the sink's view window",
                ));
            }
        }

        // … the frame walk, unchanged …
    }
}
```

```rust
// svg.rs — the pinned template and the escaper

Prim2::Text { at, text, size, align, style } => {
    debug_assert_finite(at);
    // `transform="scale(1 -1)"` undoes the document group's flip, so glyphs
    // are upright and the element's own coordinates are y-down: hence `-y`.
    // Emitting a second <g> instead would move R-0003's golden bytes (§2.7).
    let _ = write!(
        self.buf,
        "<text transform=\"scale(1 -1)\" x=\"{}\" y=\"{}\" \
         font-family=\"monospace\" font-size=\"{}\" text-anchor=\"{}\" \
         xml:space=\"preserve\" fill=\"{}\" fill-opacity=\"{}\">",
        at.x, -at.y, size, anchor_word(*align), hex(style), style.alpha,
    );
    escape_text(text, &mut self.buf);
    let _ = writeln!(self.buf, "</text>");
}

/// SVG's `text-anchor` keyword for an [`Align`] — a closed set of literals,
/// so no attribute value can ever contain a metacharacter (§2.6).
fn anchor_word(align: Align) -> &'static str {
    match align {
        Align::Left => "start",
        Align::Center => "middle",
        Align::Right => "end",
    }
}

/// XML element-content escaping — the only escaping motoreel performs.
///
/// `text` is printable ASCII by `Prim2::Text`'s invariant, so three
/// metacharacters exhaust the interesting cases: no control character and no
/// numeric character reference can arise. `"` and `'` are legal in element
/// content and this string never enters an attribute value.
///
/// The map is **total over `char`**, not over `0x20..=0x7E`: the fallback arm
/// passes anything else through, so a hand-built `Prim2` that violates the
/// invariant still yields well-formed UTF-8 rather than undefined behaviour —
/// the one case where the two sinks disagree about the string (§2.6, §2.10).
fn escape_text(text: &str, out: &mut String) {
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(c),
        }
    }
}
```

```rust
// ppm.rs — the new coverage source (SPEC-0006 §2.12's second entry point)

impl Canvas {
    fn draw(&mut self, prim: &Prim2) {
        match prim {
            // Text routes first: the stroke guards below key off
            // `Style::width`, which text does not use (§2.9).
            Prim2::Text { at, text, size, align, style } => {
                self.draw_text(*at, text, *size, *align, *style)
            }
            Prim2::Point { .. }
            | Prim2::Segment { .. }
            | Prim2::Polyline { .. }
            | Prim2::Edges { .. } => self.draw_stroke(prim),
        }
    }

    /// SPEC-0006 §2.5's rasterizer, verbatim: style lookup, the
    /// `radius`/`alpha` guards, `push_segments`, the tile, the coverage pass,
    /// the composite pass. This is a **pure extract-method** from `draw` —
    /// edit 6 of §2.13 — so that `draw` can become a two-arm router. Not one
    /// expression is edited or reordered, which is why SPEC-0006's stroke
    /// golden cannot move.
    fn draw_stroke(&mut self, prim: &Prim2) {
        // … SPEC-0006 §3's `draw` body, moved unchanged …
    }

    /// Blit one ASCII run from the embedded face (§2.8, §2.9). Integer
    /// nearest-neighbour scaling, integer pen origin, one `src_over` per lit
    /// pixel — glyph cells are disjoint, so no coverage tile is needed and
    /// SPEC-0006's one-composite-per-pixel-per-primitive rule still holds.
    fn draw_text(&mut self, at: Pt2, text: &str, size: f64, align: Align, style: Style) {
        let alpha = unit(style.alpha);
        if !(size.is_finite() && size > 0.0) || alpha == 0.0 || text.is_empty() {
            return;
        }
        // The face's own metrics, never their values inline (§2.8).
        let k = (size * self.scale / font::CELL_H as f64)
            .round()
            .clamp(1.0, 4096.0) as i64;
        let (px, py) = to_pixel(at, self.scale, self.dims); // SPEC-0006 §2.3

        let advance = font::CELL_W * k;
        let width = advance * text.len() as i64;
        let pen = match align {
            Align::Left => px,
            // `CELL_W · k · n` is even, so the halving is exact.
            Align::Center => px - (width / 2) as f64,
            Align::Right => px - width as f64,
        };
        let x0 = pen.floor() as i64;
        let y0 = py.floor() as i64 - font::BASELINE * k;

        for (i, byte) in text.bytes().enumerate() {
            let bits = font::glyph(byte);
            let gx = x0 + advance * i as i64;
            for (r, row) in bits.iter().enumerate() {
                // `0..5` and `0b10000` are the row's storage layout, defined
                // by the table's own doc comment — not a metric (§2.8).
                for c in 0..5 {
                    if row & (0b10000 >> c) == 0 {
                        continue;
                    }
                    self.fill_block(
                        gx + k * c as i64,
                        y0 + k * r as i64,
                        k,
                        style.stroke,
                        alpha,
                    );
                }
            }
        }
    }

    /// One `k × k` block, clipped to the canvas before any per-pixel work so
    /// an off-frame run costs nothing.
    fn fill_block(&mut self, x: i64, y: i64, k: i64, src: Rgb, alpha: f64) {
        let Some(tile) = Tile::clip(x, y, x + k, y + k, self.dims) else {
            return;
        };
        for py in tile.y0..tile.y1 {
            for px in tile.x0..tile.x1 {
                let i = (py as usize * self.dims.0 as usize + px as usize) * 3;
                src_over(&mut self.pixels[i..i + 3], src, alpha); // SPEC-0006
            }
        }
    }
}
```

```rust
// font.rs — the embedded face: one const table, one lookup. No types.

//! A 5 × 7 bitmap face on a 6 × 8 cell, covering printable ASCII
//! `0x20..=0x7E`. Row-major so the table reads as ASCII art (§2.8):
//! one `u8` per row, low 5 bits, `0b10000` is the leftmost column.
//! The baseline sits 6 font-pixels below the cell top — rows 0..=5 are the
//! cap zone, row 6 is the descender, row 7 (not stored) is the line gap.

pub(crate) const CELL_W: i64 = 6;
pub(crate) const CELL_H: i64 = 8;
pub(crate) const BASELINE: i64 = 6;

const FIRST: u8 = 0x20;
const GLYPHS: [[u8; 7]; 95] = [
    [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000], // ' '
    [0b00100, 0b00100, 0b00100, 0b00100, 0b00000, 0b00100, 0b00000], // '!'
    // … 92 more …
    [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b00000], // 'A'
    // …
    [0b00000, 0b00000, 0b01101, 0b10010, 0b10010, 0b01110, 0b00010], // 'g' — descender
    // …
    [0b01000, 0b10101, 0b00010, 0b00000, 0b00000, 0b00000, 0b00000], // '~'
];

/// The glyph for `byte`. `Prim2::Text`'s invariant makes the fallback
/// unreachable; it returns `'?'` rather than panicking, with the same
/// `debug_assert` tripwire pattern `svg.rs` and `ppm.rs` use for
/// SPEC-0002's finite-coordinate contract.
pub(crate) fn glyph(byte: u8) -> &'static [u8; 7] {
    debug_assert!((0x20..=0x7E).contains(&byte), "Prim2::Text's ASCII invariant was violated");
    GLYPHS
        .get(byte.wrapping_sub(FIRST) as usize)
        .unwrap_or(&GLYPHS[(b'?' - FIRST) as usize])
}
```

**The SVG text golden, illustrative** — exact bytes are frozen at
implementation from the pinned grammar and reviewed by eye, then blessed
(§6 AC5), as SPEC-0003 §3 does. A trig-free scene: `Scene::new(1.0)` (default
orthographic camera at `translator(0, 0, 5)`, default view `(3.2, 1.8)`, so
screen anchors sit at `±1.6` / `±0.9`), one white point object at world
`(0.5, −0.25, 0)` holding `translator(0.5, −0.25, 0)`, and four labels — one
per anchor kind, all three alignments, one escaping case, and the `-0` witness
§2.4.3 requires:

```
<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="1920" height="1080" viewBox="-1.6 -0.9 3.2 1.8">
<g transform="scale(1 -1)">
<circle cx="0.5" cy="-0.25" r="0.005" fill="#ffffff" fill-opacity="1"/>
<text transform="scale(1 -1)" x="-1" y="-0.5" font-family="monospace" font-size="0.08" text-anchor="start" xml:space="preserve" fill="#e0e0e0" fill-opacity="1">v = 2 m/s</text>
<text transform="scale(1 -1)" x="-1.475" y="-0.65" font-family="monospace" font-size="0.12" text-anchor="end" xml:space="preserve" fill="#ffffff" fill-opacity="1">L &lt; 90 &amp; rising</text>
<text transform="scale(1 -1)" x="0.5" y="0.25" font-family="monospace" font-size="0.08" text-anchor="middle" xml:space="preserve" fill="#ff4d00" fill-opacity="1">box</text>
<text transform="scale(1 -1)" x="0" y="-0" font-family="monospace" font-size="0.08" text-anchor="middle" xml:space="preserve" fill="#e0e0e0" fill-opacity="1">centre</text>
</g>
</svg>
```

Note the order: the object first, then the labels (§2.1). Three things in those
bytes are load-bearing rather than decorative:

- **The third label agrees with the circle, in the way §2.6's rule actually
  produces.** Its `x` equals the circle's `cx` **bit-for-bit**, and its `y` is
  the **exact negation** of the circle's `cy` — `0.25` against `cy="-0.25"` —
  because §2.6 emits `y` as `-p.y` for the counter-flip. Both come from the
  same `Pose` anchor / `Shape::Point` agreement AC3 asserts; the negation is
  the template's, not a discrepancy. (An earlier draft claimed both attributes
  were bit-identical to the circle's, which the block immediately below it
  contradicted.)
- **The fourth label is the `-0` witness.** It is
  `Anchor::Screen(ScreenAnchor::Centre)` with a zero offset, so `at` is
  `(+0.0, +0.0)`: `x` prints `0` and `y`, negated by the template, prints
  `-0`. Legal, deterministic, and already precedented — R-0003's golden carries
  `-0,0.25` in its polyline today (§2.4.3).
- **It is also the ergonomic trap, frozen in bytes.** That label has
  `Align::Center` but no vertical offset, so its baseline is on the origin and
  the run reads *above* centre. Anyone blessing this fixture sees the thing
  §2.4.3 warns about.

**The PPM text golden**: 96 × 48 px over view `(3.0, 1.5)`, so `s = 32`
exactly, on black. Length `13 + 96·48·3 = 13 837` bytes. The non-default view
is chosen deliberately: at `s = 32` the cell height `8/s = 0.25` and every
baseline row lands on a **dyadic** image coordinate, so `size = 0.5 → k = 2`
and `size = 0.25 → k = 1` are exact rather than rounding accidents, and every
expected pixel index is hand-derivable. (Agreement of the *default* views is
already covered by SPEC-0006 AC2(b); this fixture's job is to pin glyph
bytes.)

**Contents, exactly.** "One `k = 2` run, one `k = 1` corner run, one at alpha
0.5" is a sketch, not a fixture — nobody can bless bytes from it. The scene is
pinned here so the expected pixels are derivable before a line of code exists.
`Scene::new(1.0)` with `scene.view = (3.0, 1.5)`; **no objects**; three labels,
appended in this order. `to_pixel` at this scale is
`(px, py) = (48 + 32·x, 24 − 32·y)`, and screen anchors sit at `±1.5` / `±0.75`:

| # | anchor | `offset` | `align` | author text | emitted | `size` | `k` | `style` |
|---|---|---|---|---|---|---|---|---|
| 1 | `Screen(Centre)` | `(0.0, -0.25)` | `Center` | `"MOTO"` | `"MOTO"` | `0.5` | 2 | `#ffffff`, `width: 0.0`, `alpha: 1.0` |
| 2 | `Screen(BottomLeft)` | `(0.0625, 0.0625)` | `Left` | `"g\u{00E9}"` | `"g?"` | `0.25` | 1 | `#ff4d00`, `width: 0.0`, `alpha: 1.0` |
| 3 | `Screen(Centre)` | `(0.375, -0.25)` | `Center` | `"MOTO"` | `"MOTO"` | `0.5` | 2 | `#00b4d8`, `width: 0.0`, `alpha: 0.5` |

Derived placement, all integral, all inside the frame:

| # | `(px, py)` | `w` | `x0` | `y0` | ink |
|---|---|---|---|---|---|
| 1 | `(48, 32)` | 48 | 24 | 20 | cols 24..72, rows 20..34, baseline row 32 |
| 2 | `(2, 46)` | 12 | 2 | 40 | cols 2..14, rows 40..47, baseline row 46 |
| 3 | `(60, 32)` | 48 | 36 | 20 | cols 36..84, rows 20..34, baseline row 32 |

What each one is for:

- **1 — the `k = 2` interior run.** `M` and `O` have enclosed counters, so
  nearest-neighbour replication is checked on interior pixels and not only on
  edges. `width: 0.0` is deliberate: it proves §2.9's claim that text ignores
  the stroke path's `radius > 0.0` guard and renders anyway.
- **2 — the `k = 1` corner run, the baseline convention, and the
  substitution.** `g` puts ink on row 46 (the descender, *at* the baseline)
  and rows 40..45 above it, which is AC6(d)'s convention in the fixture rather
  than only in a unit test. The `?` is a **substituted** character: the author
  string is `"g\u{00E9}"`, so the golden also proves §2.5 fired upstream. The
  `0.0625` offsets are `2/32` — dyadic, and enough to clear the frame edge so
  nothing is clipped.
- **3 — compositing at alpha 0.5, over ink and over background.** It is
  displaced from run 1 by exactly `0.375` image units = 12 px = **one cell at
  `k = 2`**, so its four cells land cell-aligned on run 1's cells 1–3 and on
  black for the fourth. Both composites are hand-checkable from SPEC-0006's
  `src_over`, and all three channels land on an exact `.5` tie, so the fixture
  pins the *ties-away-from-zero* rounding rule as a side effect:
  over black → `(0, 90, 108)`; over run 1's white → `(128, 218, 236)`.

**No object, and no default-styled label — on purpose.** `Style::default()`
carries `width = 0.01`, which at `s = 32` is a stroke radius of
`0.5 · 32 · 0.01 = 0.16` px: sub-pixel, so a default-styled segment or dot in
this fixture would render as a faint partial-coverage smudge whose bytes depend
on the AA ramp rather than on anything this golden is testing. Every label
above therefore carries an explicit `Style`, and there is no geometry at all.
Text over strokes is not lost coverage: run 3 over run 1 exercises the same
`src_over` path, and SPEC-0006's own golden already owns the stroke bytes.

**The font specimen golden** (`tests/golden/font_specimen.ppm`) — §5 Q4,
accepted and **mandatory**. Same frame and view as above (96 × 48 over
`(3.0, 1.5)`, `s = 32`, black, 13 837 bytes), so `k = 1` cells are 6 × 8 px and
the frame is exactly **16 columns × 6 rows = 96 cells** for 95 glyphs. Six
labels, all `Anchor::Screen(ScreenAnchor::TopLeft)`, `Align::Left`,
`size = 0.25`, white, `width: 0.0`, `alpha: 1.0`; row `r` (0-based) carries the
16 characters `0x20 + 16r ..= 0x20 + 16r + 15` — the last row carries the
final 15, `0x70..=0x7E` — at `offset = Pt2 { x: 0.0, y: -(6 + 8r) / 32 }`,
which puts baselines on pixel rows 6, 14, 22, 30, 38 and 46. Every offset is
dyadic, every cell top is a multiple of 8, and each row's descender row sits
two rows clear of the next row's cell top, so no glyph can collide with its
neighbour and a collision in the blessed image is a real defect.

## 4. Non-goals

- **No layout engine** (R-0007 §4): no wrapping, no truncation, no ellipsis,
  no bidi, no kerning, no shaping, no grapheme clustering, no line breaking.
  One string, one line, one anchor. A `'\n'` is a non-renderable character
  (§2.5), not a break.
- **No LaTeX, no rich text, no markup, no per-run styling.** Their absence is
  documented here rather than hidden, as R-0007 §4 requires.
- **No font loading, no font files, no fallback chains, no font metrics
  tables.** One embedded face; the SVG sink names a generic family.
- **No collision avoidance, leader lines, or automatic placement** — placement
  is the author's job (R-0007 §4).
- **No rotated, curved, or path-following text**; no outline stroking of
  glyphs (`Style::width` is unused for text).
- **No non-ASCII**, by AC8. Not "unsupported": defined, total and visible
  (§2.5).
- **No pixel-identical text between the sinks.** Stated, itemized, and
  bounded in §2.10 rather than aspired to.
- **No interleaved object/label draw order.** All labels paint last (§2.1). If
  a future requirement needs interleaving, it adds an explicit ordering key —
  additively, since `objects` and `labels` remain separate `Vec`s.
- **No unification of the view window between `Scene` and the sinks.** The
  duplication is real and is now *detected* — `Scene::render` rejects a
  mismatch before frame 0 (§2.4.3) — but not removed. Removing it means a
  single `View` owned by the `Scene` and read by the sinks, which is a breaking
  change to two landed sink constructors and therefore its own requirement.
- **No `view()` consumer other than validation.** `FrameSink::view` is read in
  exactly one place, by `Scene::render`, to compare. It never reaches
  `Scene::eval`, never influences a coordinate, and never appears in an emitted
  byte. A sink that wanted to *drive* placement from its own window would be
  the design §2.4.3 rejects.
- **No changes to `camera.rs`, `track.rs`, `record.rs`,
  `examples/first_light/scene.rs`, or `examples/tumbling_box/`.** `sink.rs` is
  amended, additively and only as described in §2.4.3 and §2.14 item 8; every
  existing `impl FrameSink`, in the crate and in the tests, compiles unchanged.
  R-0003's and R-0006's golden fixtures are byte-unchanged (§2.7) — the
  cheapest proof this spec is additive.
- **No throughput work**: no glyph caching, no `Cow<'_, str>` in `Prim2`, no
  atlas. R-0011.

## 5. Open questions

**None.** All seven were adjudicated at the architect review of 2026-08-27 and
are recorded in §7. Kept here as a short ledger, so a reader of this section
does not have to reconstruct what was asked:

| # | question | outcome |
|---|---|---|
| Q1 | `font.rs` as its own module | **Yes.** It contradicts SPEC-0006 §2.0's "no module per single caller" only in letter: the face introduces no type, trait or indirection — it is data — and inlining ~100 lines of `const` bitmap would bury `ppm.rs`'s logic (§2.0) |
| Q2 | move `ObjectId` from `scene.rs` to `object.rs` | **Yes.** It removes a genuine `label → scene → label` module cycle and the public path `motoreel::ObjectId` is unchanged (§2.0.1). The stale R-0007 doc comments on both files are fixed in the same edit (§2.14 item 4) |
| Q3 | `Corner` as four corners only | **Moot.** The owner amended R-0007 AC4 to a 3 × 3 grid; the type is now `ScreenAnchor` with nine variants (§2.2, §2.4.3). The related sub-question — a safe-area inset instead of the literal edge — is still declined, for the reason it was raised with: the inset is already expressible as `offset` |
| Q4 | the font-specimen golden | **Yes, and mandatory.** The 665-byte hand-authored table carries all of AC6, and nobody proofreads 665 bytes in a diff. One blessed image, reviewed once by eye, is the only reviewable artefact the face will ever have, and it makes any later glyph edit visible. Pinned in §3 |
| Q5 | the substitute character `'?'` | **Keep.** Its one weakness — indistinguishable from an author-typed `?` — is answered by `is_ascii_renderable`; every alternative is worse in a way §2.5 tabulates. Changing it later costs one line and one blessed golden |
| Q6 | `font-family` pinned to `monospace` | **Pinned.** A `with_font_family` knob would move golden bytes and would not improve cross-sink agreement, which is limited by the embedded face, not by the SVG side. Additive later if a creator asks |
| Q7 | default `size` of 0.08 image units | **Accepted.** 48 px em, 36 px cap height at 1080p over the default view — a readable caption. Owner taste, one constant, no structural consequence |

One item is *not* an open question but is flagged for the owner rather than
settled here: `Label::is_ascii_renderable` is public API no AC requires (§2.2).
It stays, and the commitment belongs in **R-0007's** decision log at acceptance.

## 6. Acceptance criteria

Each maps to an R-0007 AC; the qa agent derives the binding tests from these
(tests first, red, then implementation) in
`crates/motoreel/tests/r0007_anchored_labels.rs`, with unit tests for the
private helpers (`to_ascii`, `screen_point`, `escape_text`, `font::glyph`, the
`k` derivation and the alignment arithmetic) in each module's own
`#[cfg(test)] mod tests` — the house pattern.

- [x] **AC1 — the `Label` shape and a scene that holds labels.** A `Label`
  built through `new` + the four builders round-trips every field into the
  emitted `Prim2::Text` (text, position, size, align, style bit-for-bit via
  `to_bits`, matching R-0002 AC4's strictness). `Scene::new` starts with
  `labels` empty and `view == (3.2, 1.8)`; `add_label` appends in order; a
  scene holding both objects and labels emits **every object, then every
  label**, each in insertion order (the §2.1 rule, asserted on the slice).
  Defaults: `size == 0.08`, `Align::Left`, `Style::default()`.
- [x] **AC2 — point anchors project and cull like geometry.**
  (a) A point-anchored label and an `Object::point` at the same world point
  under the same camera produce **bit-identical** positions (`to_bits` on `x`
  and `y`), under both projections.
  (b) A label anchored behind the camera, at the plane, at `d < NEAR`, and at
  an ideal point (via `pga::Point::from_multivector`, the generator
  `tests/r0002_scene_camera.rs` already has) is **absent** — no `Prim2::Text`
  in the slice — and no NaN appears anywhere in the frame.
  (c) A label whose `offset` is NaN or infinite is absent.
  (d) Property (`proptest`, the existing dev-dependency), mirroring SPEC-0002's
  never-non-finite property: hostile anchors to ±1e9, behind/at-plane points,
  ideal points, focal in `(1e-6, 1e3]`, both projections — every emitted
  `Text.at` is finite.
- [x] **AC3 — pose anchors ride the track.** The headline assertion, computed
  independently rather than by re-running the implementation: for
  `t ∈ {−1, 0, 0.5, 1, 4, 7}` on a multi-key track,
  `eval(t)`'s `Text.at` equals
  `camera.pose.inverse().compose(&track.eval(t))` applied to the model anchor
  point and then `Projection::project`ed — compared by `to_bits`
  (`Projection::project` is `pub` precisely for this).
  Plus: a pose-anchored label at model point `p` on object *O* is
  bit-identical in position to a `Shape::Point(p)` object carrying *O*'s
  track, for a **moving** track. Plus: inserting further objects after *O*
  does not move the label (id stability). Plus: a label whose object is culled
  but whose anchor point is in front is **present** (§2.4.2). Plus: a label
  carrying an out-of-range `ObjectId` (scene built by struct literal) is
  absent, with no panic.
- [x] **AC4 — screen anchors are fixed, over the full 3 × 3 grid.**
  (a) The matrix is **nine cases, not four**: for every `ScreenAnchor` variant,
  across `t ∈ {0, 1, 7}`, several camera poses (translated and rotated) and
  both projections, `Text.at` is the **same bits every time** and equals
  `screen_point(anchor, view) + offset` exactly, with the expected point
  written out literally per variant rather than computed by the helper under
  test. `Scene::view` set to a non-default pair moves all nine
  correspondingly.
  (b) The middle row and column are pinned as **`+0.0`**, by `to_bits`, not by
  `==` — `Centre` is `(+0.0, +0.0)` for every `view`, including a `view` whose
  halves are negative or non-finite, because those coordinates are a literal
  and not arithmetic (§2.4.3). The five corner/edge variants under a
  contract-violating `view` are absent (§2.12).
  (c) `Scene::new`, `SvgSink::new` and `PpmSink::new` report the same default
  view, extending SPEC-0006 §2.1's two-sink assertion to three.
  (d) **The view-agreement validator.** `Scene::render` with a sink whose
  `view()` disagrees returns `ErrorKind::InvalidInput` and **writes no file**
  (asserted on an empty output directory — the "before frame 0" clause is the
  claim, so the absence of `frame_00000` is the evidence). Agreement renders
  normally; a NaN `scene.view` against a finite sink view is rejected; a sink
  that keeps the defaulted `view()` — the recording double already in
  `tests/r0003_svg_sink.rs` — renders with no check and needs no edit, which
  is the additivity claim made executable.
- [x] **AC5 — the SVG `<text>` element, and the untouched golden.**
  (a) String assertions on the exact pinned template (§2.6) for each `Align`,
  and for `size`/`alpha` values that exercise the default float `Display`
  rule; style attributes always present, including `fill-opacity="1"`;
  attribute order asserted literally, no XML parser.
  (a′) **The `-0` case is kept and made specific.** A label on any of
  `MidLeft` / `Centre` / `MidRight` with no vertical offset has `at.y == +0.0`,
  so §2.6's `{-at.y}` emits `y="-0"` — asserted as that literal substring, for
  all three variants. This is a byte-level commitment, not an accident to be
  normalized away: the same shape is already blessed in R-0003's golden
  (`-0,0.25`), and `labels_00000.svg` carries it in its expected bytes (§3).
  (b) Escaping: `&`, `<`, `>` map to `&amp;`, `&lt;`, `&gt;`; `"` and `'` are
  emitted verbatim; a string of all 95 printable characters round-trips with
  exactly those three substitutions and no others.
  (c) **`tests/r0003_svg_sink.rs`'s golden test passes unmodified**, with
  `tests/golden/frame_00000.svg` byte-unchanged and no `MOTOREEL_BLESS`
  regeneration — asserted operationally by the test suite and enforced as an
  architect/PR gate on the fixture's diff (the R-0003 AC5 precedent for
  gating on a file's contents).
  (d) A new trig-free golden `tests/golden/labels_00000.svg` matches
  byte-for-byte (§3), blessed via
  `MOTOREEL_BLESS=1 cargo test -p motoreel --test r0007_anchored_labels`.
- [x] **AC6 — `PpmSink` rasterizes from the embedded face.**
  (a) `font::glyph` unit tests: 95 entries, every row `≤ 0b11111`, `'A'` and
  `'g'` pinned as ASCII art, `glyph(0)` returns the `'?'` glyph in release.
  (b) The `k` derivation pinned exactly: at `s = 32`, `size 0.25 → k = 1` and
  `size 0.5 → k = 2`; a sub-font-pixel `size` clamps to `k = 1`; a NaN or
  non-positive `size` paints nothing (pure-background frame).
  (c) Alignment arithmetic asserted exactly: a 3-character run at `k = 2` has
  `w = 36` px, so `Center` pens at `px − 18` and `Right` at `px − 36`, and
  `w/2` is integral for every `k` and `n`.
  (d) Baseline convention: for a capital `H` at `k = 1` with an integral `py`,
  the lowest lit pixel row is `py − 1` and no pixel is lit at `py` — the
  glyph sits **on** the baseline; a `'g'` at the same position lights row
  `py` (its descender).
  (e) Ink lands within **±1 px** of `to_pixel(at)` (SPEC-0006 §2.8's
  tolerance, extended); background is preserved outside the run's cell box.
  (f) `PpmSink`'s stroke path is unaffected: SPEC-0006's own golden
  (`tests/golden/frame_00000.ppm`) passes byte-unchanged.
  (g) `tests/golden/labels_00000.ppm` (96 × 48, `s = 32`) matches
  byte-for-byte, with SPEC-0006 §6 AC3's `(x, y, channel)` first-difference
  reporter. Its scene is the one pinned in §3 — three labels, no objects — and
  the derived table there (`x0`, `y0`, baseline row, and the two composite
  results `(0, 90, 108)` and `(128, 218, 236)`) is asserted as *named pixel
  probes* alongside the whole-file comparison, so a byte diff says which claim
  broke rather than only that one did.
  (h) **`font_specimen.ppm` is mandatory** (§5 Q4, accepted): 96 × 48, all 95
  glyphs at `k = 1` in six 16-cell rows, matching byte-for-byte. It is the
  only artefact in which the 665-byte face is reviewable; the test additionally
  asserts that no row's ink reaches the next row's cell top, so a mis-authored
  descender is caught as a failure and not merely as an ugly image.
- [x] **AC7 — determinism end to end.** Two renders of a label-bearing scene
  in one process into `CARGO_TARGET_TMPDIR/{a,b}` produce byte-identical
  files pairwise **for each sink independently** (§2.10 pins this reading);
  the file lists match. `Scene::eval` re-evaluated on the scene and on a
  `clone()` gives `Text` positions equal by `to_bits` and identical strings.
  Both text goldens are trig-free by SPEC-0003 §2.6's construction rules, so
  the bytes are portable. `Cargo.toml` is unchanged — the `section_keys`
  assertion from R-0003 AC5 / R-0006 AC7, reused verbatim.
- [x] **AC8 — ASCII-only, total and documented.** Table-driven over
  `["é", "→", "日本", "\t", "\n", "\r", "\u{0}", "\u{7F}", "e\u{301}", "🙂"]`:
  each yields the documented substitution, the **character count is
  preserved**, and nothing panics. `is_ascii_renderable` agrees with the
  substitution on every case. Both sinks show the same result: the SVG bytes
  contain `?` at the substituted positions and the PPM frame is byte-identical
  to the same scene authored with `?` literally — the cheapest proof that the
  substitution happens upstream and the sinks cannot disagree. Property
  (`proptest`) over arbitrary `String`: the emitted `Prim2::Text.text` is
  always entirely `0x20..=0x7E` and `chars().count()` is preserved.

## 7. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-08-27 | Labels live in a separate `Scene::labels: Vec<Label>`, not as a `Shape`/`Object` variant | A label is anchored, not posed: a screen-anchored label has no world position and would carry a lying `Track`; `Anchor::Pose` and `Object::track` would be two spellings of one idea; text is not a point set and does not scale with depth, so it cannot share `project_shape` (§2.1) |
| 2026-08-27 | Draw order is two-phase: every object, then every label, insertion order within each | A label exists to be read; occlusion by a later object would make legibility depend on insertion order. Needs no sort, so SPEC-0003 §2.5's "the sink never sorts" is untouched (§2.1) |
| 2026-08-27 | `add_label` mints no `LabelId` | Nothing in R-0007 references a label; `Scene::add` returns an id only because R-0002 recorded it as the RFC target API. `labels` is public, so an id is additive later (§2.1) |
| 2026-08-27 | `Label` carries a sixth field, `size` (em height in image units); `Style::width` is unused for text | Reusing `Style::width` would make one `Style` mean two things across variants and break SPEC-0002 AC4's bit-exact passthrough; adding `size` to `Style` would change a landed type carried by four other variants (§2.2) |
| 2026-08-27 | `Align` is horizontal only (`Left`/`Center`/`Right` → `start`/`middle`/`end`); the anchor sits on the **baseline** | SVG's `<text> y` *is* the baseline, so no `dominant-baseline` attribute is needed — an attribute with uneven renderer support and a pure source of cross-sink divergence. Vertical adjustment is `offset.y` (§2.2) |
| 2026-08-27 | `ObjectId` moves to `object.rs` and gains `pub(crate) fn index` | `label → scene → label` is a module cycle constitution §2 forbids; the id names an `Object`. The public path `motoreel::ObjectId` is unchanged (§2.0.1) |
| 2026-08-27 | `Anchor::Pose` resolves by re-running `project_shape`'s exact two-step, and re-evaluates the object's track rather than caching phase 1's poses | Identical expression order is what makes AC3's independent computation bit-exact; a pose cache would allocate on every `eval`, including the common label-free one (§2.4.2) |
| 2026-08-27 | An out-of-range `ObjectId` culls the label; no panic, no `expect` | Reachable, not unreachable: `Scene`'s fields are public and ids are not scene-scoped. Constitution §6 forbids unchecked failures in library code (§2.4.2) |
| 2026-08-27 | A label survives its anchor object's cull | Cull is per primitive and a label is its own primitive — the honest reading of R-0002 §2.5, documented because it will otherwise read as a bug (§2.4.2) |
| 2026-08-27 | `Scene` gains a public `view: (f64, f64)` field, defaulting to `(3.2, 1.8)`, so corners resolve in `eval` | AC4 needs a fixed image-space position and the view window is a sink argument `eval` cannot see. Rejected: a `FrameSink::view()` hook (would make `eval` and `render` disagree, so a frame stops being a pure function of `(scene, t)` — the property every golden rests on); normalized corner coordinates in `Prim2`; changing `eval`'s signature (§2.4.3) |
| 2026-08-27 | The view-window duplication is contained by matching defaults, a three-way default-agreement test, and a named promotion path — not fixed here | Unifying it is a breaking change to two landed sink constructors, so it is its own requirement rather than a quiet edit (§2.4.3, §4) |
| 2026-08-27 | Non-ASCII is normalized to `'?'` **in `Scene::eval`**, one `char` in / one byte out, and `Prim2::Text.text` carries the invariant `0x20..=0x7E` | Doing it once upstream is what makes "same string, same presence" a property of the pipeline rather than a coincidence between two writers; preserving the character count keeps alignment and advance predictable (§2.3, §2.5) |
| 2026-08-27 | The substitute is `'?'`, not `U+FFFD`, not a reserved slot, not a drop | `U+FFFD` breaks the ASCII invariant and would put UTF-8 in the SVG the raster sink cannot draw; a non-printable slot would be visible in one sink only; dropping is the silent corruption AC8 forbids. `Label::is_ascii_renderable` answers the residual ambiguity with an author-typed `?` (§2.5) |
| 2026-08-27 | A `size` below one font-pixel per cell clamps to `k = 1` (renders larger) rather than vanishing | Same principle as the visible substitute: an absent label is the failure R-0007 exists to prevent and the author cannot see it; a too-large one they can (§2.8) |
| 2026-08-27 | The `<text>` element carries its own `transform="scale(1 -1)"` and emits `y = -p.y`, rather than a second unflipped `<g>` | The counter-flip is what keeps the document skeleton — and therefore R-0003's golden bytes — unchanged; an always-emitted second group would move them, and a conditional one would be exactly the value-dependent branching SPEC-0003 §2.5 forbids (§2.6, §2.7) |
| 2026-08-27 | **SPEC-0003 §2.5's "no escaping machinery" invariant is amended**: attribute values stay escape-free (numbers, `#hex`, closed keyword set); element content exists in exactly one place and is escaped by a four-arm character map (`&`→`&amp;`, `<`→`&lt;`, `>`→`&gt;`) | The premise ("no text content exists") is now false and must be replaced, not reinterpreted. The byte *skeleton* stays value-independent, which is what that rule protects; only one text run per label is value-dependent, through a total pure map (§2.6) |
| 2026-08-27 | `font-family="monospace"` and `xml:space="preserve"` are pinned constants | A generic family always resolves and is the closest an SVG renderer gets to a fixed-advance face; without `preserve`, XML collapses spaces the raster sink draws literally — a gratuitous divergence for one constant attribute (§2.6) |
| 2026-08-27 | The face is a 5 × 7 bitmap on a 6 × 8 cell, 95 glyphs, stored **row-major** `[[u8; 7]; 95]` | Row-major reads as ASCII art in the source and can be reviewed by eye; the 190 bytes column-major would save are worth nothing against constitution §2's readability requirement (§2.8) |
| 2026-08-27 | Glyph scaling is integer nearest-neighbour, `k = clamp(round(size·s/8), 1, 4096)`, with `f64::round` — SPEC-0006's single rounding rule | Determinism is trivial and SPEC-0006 §2.7's "no transcendental, no accumulation" extends with no new argument; with integer `k` and an integer pen origin, glyph edges land on pixel boundaries, so anti-aliasing would be a *no-op* — "no AA" is the exact answer here, not a compromise. The cost, size quantization to multiples of `8/s`, is stated in §2.10 (§2.8) |
| 2026-08-27 | Text needs **no coverage tile**: glyph cells are `6k` apart and `5k` wide, so no pixel is touched twice per primitive | SPEC-0006 §2.5's one-composite-per-pixel-per-primitive invariant holds without the `max`-union machinery — the concrete form of §2.12's "a new coverage source, not a new compositor" (§2.9) |
| 2026-08-27 | `Canvas::draw` routes `Prim2::Text` **before** the stroke guards, with all five variants named and still no `_` arm | The `radius > 0.0` guard keys off `Style::width`, which text does not use; a wildcard arm would reintroduce exactly the silent-failure mode SPEC-0006 chose exhaustiveness to prevent (§2.13) |
| 2026-08-27 | The face lives in its own `font.rs` | It introduces no type, trait or indirection — it is data, not an abstraction — and inlining it would bury `ppm.rs`'s logic. Recorded against SPEC-0006 §2.0's opposite-direction rule as §5 Q1 (§2.0) |
| 2026-08-27 | The two sinks' text agreement is specified as an explicit guaranteed / bounded / not-guaranteed table, with `Align::Left` named as the tightest-agreeing alignment | R-0007 §4 requires the non-identity be stated. Naming *which* alignment diverges least, and by how much, is the difference between a disclaimer and a specification (§2.10) |
| 2026-08-27 | R-0007 AC7's "byte-identical frames from both sinks" is pinned as *each sink is byte-identical across renders*, not SVG bytes == PPM bytes | The literal reading is impossible for two formats; leaving the ambiguity would either weaken a real guarantee or invent an unmeetable one (§2.10) |
| 2026-08-27 | The PPM text golden uses a non-default 96 × 48 / `(3.0, 1.5)` view (`s = 32`) | Every cell height, baseline row and `k` is then dyadic and exact, so expected pixels are hand-derivable rather than rounding accidents. Default-view agreement is already SPEC-0006 AC2(b)'s job (§3) |
| 2026-08-27 | **`Corner` → `ScreenAnchor`; four variants → nine.** Replaces the first draft's "ship four corners, as AC4 words it" decision | The first draft shipped four and flagged the cost — *no way to centre a title* — as §5 Q3 for the owner. The owner took the call and **amended R-0007 AC4 to a 3 × 3 grid**, so the spec follows: a spec may not stay one draft behind the requirement it realizes (§1.2). The rename is forced by the content — "corner" is a false name for `Centre` and the three edge midpoints, and `Corner::Centre` would be a lying identifier §2 forbids. `screen_point` resolves the middle row and column to the literal `0.0`, and the four corners are four of the nine, so every corner argument in this spec survives verbatim (§2.2, §2.4.3) (architect review) |
| 2026-08-27 | The centred-title recipe is normative and shipped on the type's doc comment: `TopCentre` **plus** `Align::Center` **plus** a negative `offset.y` | The anchor is the baseline and the run starts at it, so `TopCentre` alone hangs a title to the right of centre and above the frame. The amendment that added the variant was made *for* the centred title; shipping the variant without the recipe would hand the owner half a fix (§2.4.3) (architect review) |
| 2026-08-27 | **`FrameSink` gains a defaulted `fn view(&self) -> Option<(f64, f64)>`, read only by `Scene::render`, which returns `InvalidInput` on disagreement before frame 0.** Supersedes the "rejected: a `FrameSink::view()` hook" clause of the `Scene::view` row above | The first draft's containment — matching defaults plus a three-way default-agreement test — exercises only `(3.2, 1.8)`, the one configuration that *cannot* desynchronize; the moment an author calls `with_view` it is silent. The earlier rejection was aimed at a hook that **fed** anchor resolution; this one is a **validator**, so `eval` is unchanged and stays pure over `(scene, t)` and no golden moves. Additive via the default body (SPEC-0004's amendment shape, applied to a trait), and it joins the `fps`/`duration` checks already in `Scene::render` under the validate-before-side-effects policy. Rejected as cheaper: `SvgSink::for_scene` / `PpmSink::for_scene` constructors — nothing forces a constructor's use, and the author who reaches for `with_view` is exactly the one who forgets `scene.view` (§2.4.3) (architect review) |
| 2026-08-27 | §2.3's exhaustive-match list is **four sites**, counted against the tree: `svg.rs::write_prim`, `ppm.rs` (not yet existing), and `prim_parts` in **both** `tests/r0002_scene_camera.rs` and `tests/r0004_physics_playback.rs` | The first draft said "three matches exist today" and was wrong twice: it missed R-0004's second copy of the helper, and it counted `ppm.rs`, which is not in the tree. R-0004 grew the duplicate deliberately rather than sharing one helper across integration binaries, so the arm is simply written twice (§2.3) (architect review) |
| 2026-08-27 | `every_f64` **does** cover `Prim2::Text`'s `size`, via one `if let` beside its `prim_parts` call; `prim_parts` and `prim_bits` are not widened | The property those helpers audit is *finite author data in ⇒ finite data out*, and they already push `style.width` on exactly the passthrough footing §2.3 gives `size`. A helper named `every_f64` that skipped one of a variant's f64s would misstate its own coverage. Not widening `prim_parts` leaves its five other call sites untouched; string determinism is AC7's clause, not `prim_bits`' job (§2.3) (architect review) |
| 2026-08-27 | The normative escaping table's last row is **"any other `char` → verbatim"**, matching the code's total `_ => out.push(c)`; the contract-violation case is written down | The table previously said `0x20..=0x7E`, a domain narrower than the `match` it specifies, which left the one interesting case undefined. Consequence stated honestly: a violated ASCII invariant is the single case where the sinks disagree about the **string** — SVG emits UTF-8 verbatim and stays well-formed, PPM draws one `'?'` per *byte* and shifts the advance — so §2.10's cross-sink string guarantee is explicitly **conditional** on the invariant that `Scene::eval` upholds (§2.6, §2.10, §2.12) (architect review) |
| 2026-08-27 | `draw_text` uses `font::CELL_W`, `CELL_H` and `BASELINE` instead of the literals `6`, `8.0`, `6` | Declaring three constants and hard-coding their values next to them is dead code (§2) and lets the two drift. The row's 5-bit width stays literal: it is the table's storage layout, not a metric anything else reads (§2.8, §2.9) (architect review) |
| 2026-08-27 | The raster dimensions are renamed `Canvas::dims` (and `to_pixel` / `Tile::*`'s parameter), leaving `size` to mean the text em height alone | `Prim2::Text` brings a third meaning of `size` into `ppm.rs`, and `draw_text` holds two of them in scope one field-access apart — a swap the compiler cannot catch. Private rename, no public API, no golden (§2.9, §2.13 edit 7) (architect review) |
| 2026-08-27 | §2.13's "complete list" is **seven** edits, not five: `Canvas::draw`'s body is extracted verbatim into `draw_stroke`, and the `dims` rename is edit 7 | Edit 1 makes `draw` a two-arm router, which necessarily moves SPEC-0006's whole rasterizer body — the largest hunk a reviewer will see in `ppm.rs` — so "complete" has to include it. Pure extraction, no expression edited or reordered, so SPEC-0006's stroke golden cannot move (§2.13) (architect review) |
| 2026-08-27 | The stale doc comments at `src/object.rs:11` and `src/scene.rs:10` are renumbered R-0007 → **R-0010** in this spec's diff | Both point a reader at a requirement that is now about text; `scene.rs:10` would additionally be wrong in tense, since `ObjectId` stops being inert here. `scene.rs` is in the diff anyway for the `ObjectId` move and `object.rs` to receive it, so the fix costs nothing (§2.14 item 4) (architect review) |
| 2026-08-27 | `Label::is_ascii_renderable` is kept, and flagged for promotion into **R-0007's** decision log at owner acceptance | §2.5's argument for `'?'` has no exit without it, so removing it would weaken a decision the owner already has. But a public predicate is a compatibility commitment, and a spec should not mint public surface on its own authority (§1.2, §2.2) (architect review) |
| 2026-08-27 | Both PPM goldens are pinned to exact scenes — anchors, offsets, alignments, strings, sizes, styles and derived pixel geometry — rather than described | "One `k = 2` run, one `k = 1` corner run, one at alpha 0.5" is a sketch nobody can bless bytes from. The fixture now also carries three exact `.5` rounding ties, so it pins SPEC-0006's ties-away-from-zero rule as a side effect (§3) (architect review) |
| 2026-08-27 | Neither PPM golden contains default-styled ink, and `labels_00000.ppm` contains no objects at all | `Style::default().width = 0.01` is a stroke radius of 0.16 px at this fixture's `s = 32` — sub-pixel, so a default-styled dot or segment would render as an AA smudge whose bytes test the ramp rather than the glyphs. Stroke bytes are SPEC-0006's golden's job (§3) (architect review) |
| 2026-08-27 | **R-0006 must be `Met` before implementation of this spec begins** — an ordering gate, not a paper dependency | `to_pixel`, `src_over`, `unit`, `Tile` and `Canvas` do not exist in the tree, and neither does `ppm.rs`: three of §2.13's seven edits have nothing to edit, and every "reused unchanged" item is reused from unwritten code. Reversing the order would mean re-deriving SPEC-0006's mapping here, which §2.13 exists to avoid (header, §2.13) (architect review) |
| 2026-08-27 | `unit` is reused **by reference to SPEC-0006's corrected spelling**, not restated here | SPEC-0006's own architect revision is fixing that function for a clippy defect; this spec pins only its semantics (total, NaN → 0, `[0, 1]`), so there is no second copy to drift (§2.13) (architect review) |
| 2026-08-27 | §3's golden narration corrected: the `Pose` label's `x` equals the circle's `cx` bit-for-bit and its `y` is the **exact negation** of `cy` | The draft claimed both were bit-identical, which the very block beneath it contradicted (`cy="-0.25"` against `y="0.25"`) — §2.6 emits `y` as `-p.y` for the counter-flip. The agreement AC3 asserts is real; the negation is the template's (§3) (architect review) |
| 2026-08-27 | §5's seven open questions adjudicated: Q1 `font.rs` **yes**; Q2 `ObjectId` move **yes**; Q3 **moot** (superseded by the 3 × 3 amendment); Q4 font specimen **yes and mandatory**; Q5 `'?'` **keep**; Q6 `monospace` **pinned**; Q7 default `size = 0.08` **accept** | Q4 is the one upgraded rather than merely accepted: the 665-byte face carries all of AC6 and would otherwise be hand-authored with no reviewable artefact, so the specimen is a requirement of the fixture set and not an option in it (§5) (architect review) |

## Changelog

- 2026-08-27 — created (Draft); submitted for architect review.
- 2026-08-27 — architect review returned **BLOCK** (the spec predated its own
  amended requirement) plus ten findings and two constitution notes. All
  applied: AC4's 3 × 3 grid and the `ScreenAnchor` rename (the blocker), the
  `FrameSink::view` validator, the corrected exhaustive-match census, the
  escaping table's domain, the font constants, the `dims` rename, the stale
  R-0007 doc comments, the seventh SPEC-0006 edit, exact PPM golden contents,
  the R-0006 ordering gate, and the `unit` reference. §5 closed; §7 appended
  to rather than rewritten.
- 2026-08-27 — implemented and QA-signed-off. 167 tests pass; clippy and
  rustfmt clean. The suite was **mutation-tested**, not merely re-run: nine
  deliberate defects (`to_ascii` dropping instead of substituting; `Centre`
  computed rather than a literal; a pose anchor ignoring `t`; a stale id
  panicking; the baseline offset dropped; `Center` alignment forgetting the
  halving; `escape_text` passing `&`; the `text-anchor` words swapped; one
  glyph row corrupted) were each caught, every one by the test written for
  it. The specimen fixture earns its keep: a single corrupted row of `'A'`
  fails it at pixel (10, 19), which is that glyph's crossbar.

  Two glyphs were caught during authoring by the face's own invariant test
  rather than by eye — `'%'` ran into the descender row, and `';'` was
  declared a descender but did not descend. `,` `:` `;` are now one family.

  Both goldens matched their spec-derived predictions on first bless: the
  SVG fixture reproduced §3's illustrative bytes exactly (including the
  `y="0.25"` counter-flip against `cy="-0.25"` and the `y="-0"` witness),
  and every derived placement in the PPM table — `(px, py)`, `w`, `x0`,
  `y0` — came out as written, with both composites landing on the
  ties-away rounding rule.
