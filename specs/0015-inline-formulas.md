# SPEC-0015 — Inline formulas

- **Status:** Draft
- **Realizes:** R-0015
- **Author:** Claude, with the owner
- **Created:** 2026-10-14
- **Depends on:** SPEC-0014 (outlines)
- **Module(s):** `motoreel-typeset` (new module `math`)

## 1. Motivation

Realizes R-0015. Lifted from rotorf-sico's `formula.rs` (`leer`, `Mono`),
with its notation rule (which letters are italic) turned into a parameter.

## 2. Design

Coordinates as `place` and R-0014: x right, y up, the pen at x0 on baseline y.

`parse` walks the characters once. `√` consumes the run up to the next space
and parses it recursively into `Root`. `₀–₉` → `Sub(d)`, `⁰–⁹` → `Sup(d)`,
`' '` → `Space`; anything else → `Glyph { text, italic: italic(c, next) }`.

`set` keeps a pen `x`:

| Piece | Face, size | Offset | Pen |
|---|---|---|---|
| `Glyph` | upright or italic face, `size` | — | + advance (the − rule below) |
| `Sub` | upright, `sub_scale · size` | y − `sub_drop · size` | + advance |
| `Sup` | upright, `max(sup_scale · size, sup_min)` | x − `sup_kern · size`, y + `sup_rise · size` | starts `sup_kern · size` back, + advance |
| `Space` | — | — | + `space · size` |
| `Root` | `√` at `size`; radicand at `root_scale · size` | radicand at x + adv(√) + `root_gap · size` | radicand end + `root_tail · size` |

**The minus (AC3).** Set `−`; take the x-extent of its control points
`[a0, a1]` and of `+`'s `[b0, b1]`; map x ↦ `cb + (x − ca) · (b1 − b0)/(a1 − a0)`
with `ca`, `cb` the centres. The advance stays the minus's own.

**The bar (AC4).** From the √'s contours take the maximum x and maximum y
(y up) over on-curve points: the tip. With `g = bar · size`, the bar is the
rectangle `[tip.x − 0.6 g, end] × [tip.y − g, tip.y]`, its winding reversed if
its signed area's sign differs from the √'s first contour's.

## 3. Code outline

```rust
// crates/motoreel-typeset/src/math.rs — to be reviewed before any code lands
#[derive(Clone, Debug, PartialEq)]
pub enum Piece { Glyph { text: String, italic: bool }, Sub(String), Sup(String), Space, Root(Vec<Piece>) }

pub fn parse(text: &str, italic: impl Fn(char, Option<char>) -> bool + Copy) -> Vec<Piece>;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FormulaStyle {
    pub upright: FaceId, pub italic: FaceId,
    pub sub_scale: f64, pub sub_drop: f64,              // 0.72, 0.22
    pub sup_scale: f64, pub sup_rise: f64, pub sup_kern: f64, pub sup_min: f64, // 0.62, 0.40, 0.04, 0
    pub space: f64,                                      // 0.3
    pub root_scale: f64, pub root_gap: f64, pub root_tail: f64, pub bar: f64, // 0.82, 0.04, 0.08, 0.094
}

impl FormulaStyle { pub fn new(upright: FaceId, italic: FaceId) -> FormulaStyle; }

pub fn set(pieces: &[Piece], style: &FormulaStyle, size: f64, fonts: &Fonts) -> Result<Outline, Error>;
pub fn width(pieces: &[Piece], style: &FormulaStyle, size: f64, fonts: &Fonts) -> Result<f64, Error>;
```

rotorf-sico's wrapper would then be: its italic rule (a, b, v, z, and e
before an index: its `brand/notation.md`), `FormulaStyle::new(MONO_BOLD,
MONO_BOLD_ITALIC)` with `sup_min` from its designer review, and a y flip.

## 4. Non-goals

Fractions, limits, matrices, TeX input (R-0015 §4).

## 5. Open questions

R-0015 OQ-1 and OQ-2.

## 6. Acceptance criteria

- [ ] AC1 — `tests/r0015_formulas.rs::ac1_reads_scripts_spaces_and_radicals`
- [ ] AC2 — `ac2_scripts_are_smaller_and_shifted`
- [ ] AC3 — `ac3_the_minus_is_as_wide_as_the_plus`
- [ ] AC4 — `ac4_the_radical_has_its_bar`
- [ ] AC5 — `ac5_width_is_the_set_advance`
- [ ] AC6 — out of tree, in rotorf-sico (md5 of its narrated reels before and after)

## 7. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| | | |

## Changelog

- 2026-10-14 — created.
