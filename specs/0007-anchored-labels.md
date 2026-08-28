# SPEC-0007 — Anchored text labels: `Label`, `Prim2::Text`, and an embedded face

- **Status:** Draft — awaiting architect review
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
  `object.rs`, `scene.rs`, `svg.rs`, `ppm.rs`, `lib.rs`

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
  label.rs                     — `Label`, `Anchor`, `Corner`, anchor resolution,
                                 the ASCII rule
  font.rs                      — the embedded 5×7 face: one `const` table and
                                 one lookup function. No types, no traits.
  prim.rs                      — + `Align`, + `Prim2::Text`
  object.rs                    — + `ObjectId` (moved from `scene.rs`, §2.0.1)
  scene.rs                     — + `Scene::labels`, `Scene::view`, `add_label`,
                                 the label phase of `eval`
  svg.rs                       — + the `<text>` template and the XML escaper
  ppm.rs                       — + `Canvas::draw_text` (SPEC-0006 §2.12's
                                 "second entry point beside `draw`")
crates/motoreel/tests/
  r0007_anchored_labels.rs     — every AC (one file per requirement)
  golden/labels_00000.svg      — the SVG text fixture
  golden/labels_00000.ppm      — the PPM text fixture (96×48)
  golden/font_specimen.ppm     — all 95 glyphs at k = 1 (96×48) — §5 Q4
```

`lib.rs` gains `mod font; mod label;` and
`pub use label::{Anchor, Corner, Label};`, `pub use prim::Align`. The
`ObjectId` move keeps the public path `motoreel::ObjectId` unchanged; only
the `pub use` line it appears on moves from `scene::` to `object::`.

**`font.rs` is a data module, not an abstraction.** SPEC-0006 §2.0 declined a
`raster.rs` on the rule "a second module for a single caller is the premature
abstraction §2 forbids", and that rule is right about *abstractions*. The face
introduces no type, no trait, no indirection and no second call path: it is
~100 lines of hand-authored `const` bitmap that would otherwise dominate
`ppm.rs` by line count and bury the ~60 lines of blit logic that actually need
reading. §2 asks for readable, well-structured modules; extracting the table
serves that and costs nothing. Recorded as §5 Q1 for the architect, since it
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
label  ← prim, object, camera, garust  Label, Anchor, Corner
scene  ← label, object, camera, prim   Scene, eval
sink   ← prim, scene                   FrameSink, Scene::render
svg    ← prim, sink                    SvgSink
ppm    ← prim, sink, font              PpmSink, Canvas
```

The move is invisible outside the crate (`motoreel::ObjectId` is unchanged,
and `tests/r0002_scene_camera.rs` imports it from the crate root). The
alternative — defining `Label`/`Anchor` inside `scene.rs` — avoids the move
but merges "the label vocabulary" into "the evaluation loop", roughly
doubling `scene.rs` for no gain. §5 Q2.

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
  anchor kinds (`Corner`) has no world position at all. A corner-anchored
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
    Corner(Corner),                             // fixed image position
}

pub enum Corner { TopLeft, TopRight, BottomLeft, BottomRight }
```

**`size` is the one field R-0007 AC1 does not name, and the design provably
needs it.** AC1 requires the five listed fields and all five are present;
`size` is additional. It is not folded into `Style` because:

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

`Corner` is **exactly four corners**, matching R-0007 AC4's wording. A
top-centre title — the single most likely creator request — is not expressible
today. That is flagged rather than decided: adding `Top`/`Bottom`/`Left`/
`Right`/`Center` is one new variant and one more match arm, purely additive
(§5 Q3). Constitution §1.2 puts the call with the owner, not here.

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
arm of every exhaustive match. Three of those matches exist today and each
becomes a compile error until amended — which is the designed behaviour
(SPEC-0006 §2.4: "a label that silently fails to render is the failure mode
this is chosen to prevent"):

| site | amendment |
|---|---|
| `svg.rs` `write_prim` | the new `<text>` template (§2.6) |
| `ppm.rs` `push_segments`, `style_of`, `Canvas::draw` | §2.13 |
| `tests/r0002_scene_camera.rs` `prim_parts` | one arm, exactly as R-0004 added for `Edges` — the file already carries that precedent comment verbatim |

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

#### 2.4.3 `Anchor::Corner` — AC4, and the `Scene::view` seam

A corner is a position in the **view window**, and the view window is a sink
constructor argument (`SvgSink::with_view`, `PpmSink::with_view`) — which
`Scene::eval` cannot see. AC4 nevertheless requires a *fixed image-space
position*. So `Scene` gains the window:

```rust
/// The image-space window the frame is expected to show — `(width, height)`,
/// centred on the origin. Must match the sink's view window; both default to
/// `(3.2, 1.8)`. Contract: finite and > 0. Only `Anchor::Corner` reads it.
pub view: (f64, f64),
```

```
TopLeft     → Pt2 { x: -view.0 / 2.0, y:  view.1 / 2.0 }     # image space is y-up
TopRight    → Pt2 { x:  view.0 / 2.0, y:  view.1 / 2.0 }
BottomLeft  → Pt2 { x: -view.0 / 2.0, y: -view.1 / 2.0 }
BottomRight → Pt2 { x:  view.0 / 2.0, y: -view.1 / 2.0 }
```

Halving is exact (exponent decrement) — SPEC-0003 §2.5 already relies on it
for the viewBox, and the golden's `-1.6 -0.9` is the witness. Corner
resolution touches no camera and no `t`, so AC4 holds by construction: the
same `Pt2`, bit-for-bit, for every camera pose, both projections, every time.

**The honest cost: the view window is now stated twice.** A `Scene::view` that
disagrees with the sink's places corner labels off-frame or inset. Four things
contain it:

1. Both sinks and `Scene` default to `(3.2, 1.8)`, so the common case is
   consistent with zero author effort.
2. `view` is a plain public field set like `scene.camera` is today — no
   builder, no second spelling.
3. A test asserts all three defaults agree, extending the two-sink assertion
   SPEC-0006 §2.1 already specifies.
4. The unification is named as debt (§4): a single `View` value owned by the
   `Scene` and read by the sinks is the right end state, and it is a
   **breaking change to two landed sink constructors**, so it is its own
   requirement rather than a quiet edit here.

Alternatives weighed and rejected:

- **A `FrameSink::view()` trait method** consumed by `Scene::render`. It
  removes the duplication, and it is even additive (a defaulted method).
  Rejected because it makes `eval(t)` and `render` disagree about corner
  placement, so a frame stops being a pure function of `(scene, t)` — the
  property R-0002 AC6 and *every* golden test in this repo rest on. It also
  reopens `FrameSink`, which SPEC-0003 §2.2 deliberately froze at the RFC
  signature.
- **Normalized corner coordinates** carried through to the sink. `Prim2` would
  gain a position that is sometimes image space and sometimes not — the
  ambiguity `Prim2`'s enum shape exists to prevent.
- **Passing the view into `eval`.** Changes a landed public signature with
  acceptance tests against it, for one enum variant.

**Ergonomic trap, documented loudly:** because the anchor is on the baseline,
`Corner::TopLeft` with a zero offset puts the whole run *above* the frame. The
doc comment carries the worked title recipe —
`Label::new("Angular momentum", Anchor::Corner(Corner::TopLeft))
.with_offset(Pt2 { x: 0.125, y: -0.25 })` — and §5 Q3 records "a safe-area
inset" as the alternative if the owner would rather the corner mean something
less literal.

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
golden if the owner prefers another (§5 Q5).

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
  knob to bikeshed; §5 Q6 records it as a later, additive option.
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
> | byte | emitted |
> |---|---|
> | `&` (0x26) | `&amp;` |
> | `<` (0x3C) | `&lt;` |
> | `>` (0x3E) | `&gt;` |
> | any other `0x20..=0x7E` | verbatim |

Four arms, no lookup table, no general XML escaper — because the ASCII
invariant means no UTF-8 continuation byte, no control character and no
numeric character reference can ever arise. `"` and `'` are **not** escaped:
they are legal in element content and this string never enters an attribute.
`>` is escaped unconditionally even though XML requires it only in `]]>`,
because a fixed three-character rule is auditable at a glance and cannot
construct that sequence.

**How this sits with "no value-dependent branching".** That rule protects the
*byte shape*: which elements and attributes appear must not depend on the
data, so a frame's structure is diffable and predictable. Escaping does not
change which elements or attributes appear; it changes the encoding of one
text run through a total, deterministic, character-wise map. The amended
statement is therefore: **the element and attribute skeleton is
value-independent; exactly one text run per label is value-dependent, and its
transformation is a pure function of the bytes.** Recorded in the decision log
as a change to SPEC-0003's stated invariant, not as a reinterpretation of it.

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
   (`labels`) and by `Anchor::Corner` (`view`). `Vec::with_capacity` changes
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
k = clamp(round(size · s / 8), 1, 4096)     // s = px per image unit (SPEC-0006 §2.3)
```

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
(px, py) = to_pixel(at, scale, size)          // SPEC-0006 §2.3, unchanged
n        = text.len()                          // == char count; ASCII
w        = 6 · k · n                           // advance width, px (integer)
pen_x    = match align { Left => px, Center => px - (w/2) as f64,
                         Right => px - w as f64 }
x0       = pen_x.floor() as i64                // integer pen origin
y0       = py.floor() as i64 - 6·k             // cell top row
```

- **Advance width, not ink width.** `6·k·n` includes the last glyph's side
  bearing, which is what SVG's `text-anchor` centres on too; and it is total
  for `n = 0`, where an ink-width formula (`k(6n − 1)`) would go negative.
- **`w/2` is always an integer** because the advance is 6 — an even number —
  so centring introduces no half-pixel of its own. Only `px` itself can be
  fractional, and `floor` resolves it.
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
| The string, byte-for-byte after ASCII normalization | Normalized upstream (§2.5), so a substitution cannot differ between sinks |
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
   compose/transform/project chain as geometry; `Corner` reads `Scene::view`
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
| `eval` | `Anchor::Corner` | Never culled; depends on neither camera nor `t` |
| `eval` | `Scene::view` non-finite or ≤ 0 (contract violation) | Corner position non-finite → those labels culled by the finite guard. Point- and pose-anchored labels unaffected |
| `eval` | `label.offset` non-finite | Resolved position non-finite → label absent, keeping §2.3's coordinate invariant unconditional |
| `eval` | Non-ASCII or control character in `text` | Each such `char` → `'?'`; character count preserved; no panic, no drop (§2.5) |
| `eval` | `text` empty | A `Prim2::Text` with an empty string **is** emitted — presence is a fact about the anchor, not the content. SVG writes `<text …></text>`; PPM paints nothing |
| `eval` | `label.size` non-finite or ≤ 0 | Carried verbatim, exactly as a bad `Style::width` is (SPEC-0002 §2.2) |
| `SvgSink` | `&`, `<`, `>` in text | Escaped per §2.6. `"` / `'` need no escape (element content) |
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

**Reused unchanged — not re-derived, not copied:**

1. `to_pixel(p: Pt2, scale: f64, size: (u32, u32)) -> Px` — the normative
   world→pixel mapping (§2.3), including `s = min(W/vw, H/vh)`, the `xMidYMid
   meet` derivation, and pixel centres at `+0.5`. Text anchors map through it
   with no additional concept, exactly as §2.12 forecast ("the screen-corner
   anchor is a `Pt2` like any other").
2. `src_over(dst: &mut [u8], src: Rgb, a: f64)` — one `f64::round`,
   ties away from zero, 8-bit sRGB, `u8` framebuffer. Glyph pixels composite
   at coverage `1.0`; the compositor is untouched.
3. `unit(x: f64) -> f64` — the total `[0, 1]` clamp with NaN → 0, applied to
   `style.alpha`.
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

**What SPEC-0006's code must change — the complete list, five edits:**

| # | site | change |
|---|---|---|
| 1 | `Canvas::draw` | Route `Prim2::Text` to `draw_text` **before** the `radius > 0.0` / `alpha` guards, which are stroke guards and would wrongly reject a text label whose `Style::width` is 0. The match lists all five variants by name — still **no `_` arm** |
| 2 | `push_segments` | One explicit `Prim2::Text { .. } => {}` arm with the reason ("text has no centre-lines; `draw` routes it before here"). An empty arm, not a wildcard, so a sixth variant is still a compile error |
| 3 | `style_of` | One `Text` arm returning its `style` |
| 4 | `Tile` | One additive constructor, `Tile::clip(x0: i64, y0: i64, x1: i64, y1: i64, size: (u32, u32)) -> Option<Tile>`, so the glyph run's integer destination rectangle is clipped by the same code that clips stroke tiles. `Tile::around` is not refactored to use it — that would be churn in landed logic for symmetry alone |
| 5 | new | `Canvas::draw_text(&mut self, at: Pt2, text: &str, size: f64, align: Align, style: Style)` and a private `blit_glyph` — the "second entry point on `Canvas` beside `draw`" §2.12 anticipated |

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
   shapes will reference; inert until then". Two notes for the orchestrator,
   not edited here: that forward reference means the *old* R-0007
   (`JoinLine`/`MeetPoint`), renumbered to **R-0010** on 2026-08-27; and the
   id's first real consumer turns out to be labels, not derived shapes.
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
   R-0004's comment saying exactly that.

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
    Corner(Corner),
}

/// Image-space corners, `y` up (`Top` is `+y`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Corner { TopLeft, TopRight, BottomLeft, BottomRight }

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
    /// is `Scene::view` and is read only by `Corner`.
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
            Anchor::Corner(c) => Some(corner_point(*c, window)),
        }
    }
}

/// Image space is y-up, so `Top` is `+y`. Halving is exact.
fn corner_point(corner: Corner, window: (f64, f64)) -> Pt2 {
    let (hw, hh) = (window.0 / 2.0, window.1 / 2.0);
    match corner {
        Corner::TopLeft     => Pt2 { x: -hw, y:  hh },
        Corner::TopRight    => Pt2 { x:  hw, y:  hh },
        Corner::BottomLeft  => Pt2 { x: -hw, y: -hh },
        Corner::BottomRight => Pt2 { x:  hw, y: -hh },
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
/// metacharacters exhaust the cases: no UTF-8 continuation byte, no control
/// character, no numeric character reference can arise. `"` and `'` are legal
/// in element content and this string never enters an attribute value.
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
            | Prim2::Edges { .. } => self.draw_stroke(prim), // SPEC-0006 §2.5
        }
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
        let k = (size * self.scale / 8.0).round().clamp(1.0, 4096.0) as i64;
        let (px, py) = to_pixel(at, self.scale, self.size); // SPEC-0006 §2.3

        let advance = 6 * k;
        let width = advance * text.len() as i64;
        let pen = match align {
            Align::Left => px,
            Align::Center => px - (width / 2) as f64, // `6k·n` is even: exact
            Align::Right => px - width as f64,
        };
        let x0 = pen.floor() as i64;
        let y0 = py.floor() as i64 - 6 * k; // baseline is 6 font-pixels down

        for (i, byte) in text.bytes().enumerate() {
            let bits = font::glyph(byte);
            let gx = x0 + advance * i as i64;
            for (r, row) in bits.iter().enumerate() {
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
        let Some(tile) = Tile::clip(x, y, x + k, y + k, self.size) else {
            return;
        };
        for py in tile.y0..tile.y1 {
            for px in tile.x0..tile.x1 {
                let i = (py as usize * self.size.0 as usize + px as usize) * 3;
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
orthographic camera at `translator(0, 0, 5)`, default view), one white point
object at world `(0.5, −0.25, 0)` holding `translator(0.5, −0.25, 0)`, and
three labels — one per anchor kind, all three alignments, one escaping case:

```
<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="1920" height="1080" viewBox="-1.6 -0.9 3.2 1.8">
<g transform="scale(1 -1)">
<circle cx="0.5" cy="-0.25" r="0.005" fill="#ffffff" fill-opacity="1"/>
<text transform="scale(1 -1)" x="-1" y="-0.5" font-family="monospace" font-size="0.08" text-anchor="start" xml:space="preserve" fill="#e0e0e0" fill-opacity="1">v = 2 m/s</text>
<text transform="scale(1 -1)" x="-1.475" y="-0.65" font-family="monospace" font-size="0.12" text-anchor="end" xml:space="preserve" fill="#ffffff" fill-opacity="1">L &lt; 90 &amp; rising</text>
<text transform="scale(1 -1)" x="0.5" y="0.25" font-family="monospace" font-size="0.08" text-anchor="middle" xml:space="preserve" fill="#ff4d00" fill-opacity="1">box</text>
</g>
</svg>
```

Note the order: the object first, then the labels (§2.1), and the third
label's `x`/`y` are bit-identical to the circle's `cx`/`cy` — the same
`Pose` anchor / `Shape::Point` agreement AC3 asserts, visible in the bytes.

**The PPM text golden**: 96 × 48 px over view `(3.0, 1.5)`, so `s = 32`
exactly, on black. Length `13 + 96·48·3 = 13 837` bytes. The non-default view
is chosen deliberately: at `s = 32` the cell height `8/s = 0.25` and every
baseline row lands on a **dyadic** image coordinate, so `size = 0.5 → k = 2`
and `size = 0.25 → k = 1` are exact rather than rounding accidents, and every
expected pixel index is hand-derivable. (Agreement of the *default* views is
already covered by SPEC-0006 AC2(b); this fixture's job is to pin glyph
bytes.) Contents: one `k = 2` run mid-frame exercising interiors, one `k = 1`
corner run, one run with an alpha of 0.5 over the first, and one substituted
character.

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
  duplication is real, contained by defaults and a test, and named as debt
  (§2.4.3): the fix is a breaking change to two landed sink constructors and
  therefore its own requirement.
- **No changes to `sink.rs`, `camera.rs`, `track.rs`, `record.rs`,
  `examples/first_light/scene.rs`, or `examples/tumbling_box/`.** R-0003's and
  R-0006's golden fixtures are byte-unchanged (§2.7) — the cheapest proof this
  spec is additive.
- **No throughput work**: no glyph caching, no `Cow<'_, str>` in `Prim2`, no
  atlas. R-0011.

## 5. Open questions

Recorded for architect adjudication; each carries a recommendation, in the
house pattern (architect-recommended → owner acceptance).

- **Q1 — `font.rs` as its own module.** Recommended: yes. It contradicts
  SPEC-0006 §2.0's "no module per single caller" only in letter: the face
  introduces no type, trait or indirection, and inlining ~100 lines of `const`
  bitmap into `ppm.rs` would bury the logic that needs reading (§2.0).
- **Q2 — moving `ObjectId` from `scene.rs` to `object.rs`.** Recommended: yes,
  it removes a genuine module cycle and the public path is unchanged (§2.0.1).
  The alternative is to define `Label`/`Anchor` inside `scene.rs`.
- **Q3 — `Corner` is four corners only.** Recommended: ship four, as R-0007 AC4
  words it. The owner should know the cost: **there is no way to centre a
  title**, which is the most likely first creator request. Adding
  `Top`/`Bottom`/`Left`/`Right`/`Center` is one variant plus one match arm,
  purely additive. Related: whether `Corner` should carry a default safe-area
  inset rather than resolving to the literal corner (§2.4.3) — recommended
  **no**, keep it literal and predictable, with the inset expressed as
  `offset`.
- **Q4 — the font-specimen golden** (`tests/golden/font_specimen.ppm`, 96 × 48,
  all 95 glyphs at `k = 1`, every coordinate dyadic). Recommended: **yes**. A
  hand-authored face is 665 bytes nobody will proofread in a diff; one blessed
  image, reviewed once by eye, is the only practical way to know the table is
  right, and it makes any later glyph edit visible.
- **Q5 — the substitute character `'?'`.** Recommended: keep. Its one weakness
  (indistinguishable from an author-typed `?`) is answered by
  `is_ascii_renderable`. Changing it later costs one line and one blessed
  golden.
- **Q6 — `font-family` pinned to `monospace`.** Recommended: pinned. A
  `SvgSink::with_font_family` knob would move golden bytes and would not
  improve cross-sink agreement, which is limited by the embedded face, not by
  the SVG side. Additive later if a creator asks.
- **Q7 — default `size` of 0.08 image units** (48 px em, 36 px cap at 1080p).
  Recommended: accept; owner taste, one constant, no structural consequence.

## 6. Acceptance criteria

Each maps to an R-0007 AC; the qa agent derives the binding tests from these
(tests first, red, then implementation) in
`crates/motoreel/tests/r0007_anchored_labels.rs`, with unit tests for the
private helpers (`to_ascii`, `corner_point`, `escape_text`, `font::glyph`, the
`k` derivation and the alignment arithmetic) in each module's own
`#[cfg(test)] mod tests` — the house pattern.

- [ ] **AC1 — the `Label` shape and a scene that holds labels.** A `Label`
  built through `new` + the four builders round-trips every field into the
  emitted `Prim2::Text` (text, position, size, align, style bit-for-bit via
  `to_bits`, matching R-0002 AC4's strictness). `Scene::new` starts with
  `labels` empty and `view == (3.2, 1.8)`; `add_label` appends in order; a
  scene holding both objects and labels emits **every object, then every
  label**, each in insertion order (the §2.1 rule, asserted on the slice).
  Defaults: `size == 0.08`, `Align::Left`, `Style::default()`.
- [ ] **AC2 — point anchors project and cull like geometry.**
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
- [ ] **AC3 — pose anchors ride the track.** The headline assertion, computed
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
- [ ] **AC4 — corner anchors are fixed.** For all four corners, across
  `t ∈ {0, 1, 7}`, several camera poses (translated and rotated) and both
  projections, `Text.at` is the **same bits every time** and equals
  `(∓view.0/2, ±view.1/2) + offset` exactly. `Scene::view` set to a
  non-default pair moves the corners correspondingly. A test asserts
  `Scene::new`, `SvgSink::new` and `PpmSink::new` report the same default
  view, extending SPEC-0006 §2.1's two-sink assertion to three.
- [ ] **AC5 — the SVG `<text>` element, and the untouched golden.**
  (a) String assertions on the exact pinned template (§2.6) for each `Align`,
  for a `-0` y case, and for `size`/`alpha` values that exercise the default
  float `Display` rule; style attributes always present, including
  `fill-opacity="1"`; attribute order asserted literally, no XML parser.
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
- [ ] **AC6 — `PpmSink` rasterizes from the embedded face.**
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
  reporter. (h) §5 Q4's `font_specimen.ppm`, if accepted.
- [ ] **AC7 — determinism end to end.** Two renders of a label-bearing scene
  in one process into `CARGO_TARGET_TMPDIR/{a,b}` produce byte-identical
  files pairwise **for each sink independently** (§2.10 pins this reading);
  the file lists match. `Scene::eval` re-evaluated on the scene and on a
  `clone()` gives `Text` positions equal by `to_bits` and identical strings.
  Both text goldens are trig-free by SPEC-0003 §2.6's construction rules, so
  the bytes are portable. `Cargo.toml` is unchanged — the `section_keys`
  assertion from R-0003 AC5 / R-0006 AC7, reused verbatim.
- [ ] **AC8 — ASCII-only, total and documented.** Table-driven over
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
| 2026-08-27 | Labels live in a separate `Scene::labels: Vec<Label>`, not as a `Shape`/`Object` variant | A label is anchored, not posed: a corner label has no world position and would carry a lying `Track`; `Anchor::Pose` and `Object::track` would be two spellings of one idea; text is not a point set and does not scale with depth, so it cannot share `project_shape` (§2.1) |
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
| 2026-08-27 | `Corner` ships as four corners only, resolving to the literal corner with no safe-area inset | R-0007 AC4's wording, and §2 forbids premature abstraction — but the owner should decide knowingly, since it means no centred title. Additive to extend (§5 Q3) |

## Changelog

- 2026-08-27 — created (Draft); submitted for architect review.
