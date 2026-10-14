# R-0015 — Inline formulas written as they read: scripts, the true minus, the radical

- **Status:** Discussing — requirement, spec and red tests written; code outline awaiting the owner (CLAUDE.md §4 step 4)
- **Milestone:** MC (the creator pipeline)
- **Owner:** Gustavo Delgadillo (westerngazoo)
- **Created:** 2026-10-14
- **Depends on:** R-0014 (glyph outlines)
- **Realized by:** [SPEC-0015](../specs/0015-inline-formulas.md)
- **QA:** `qa` agent run scoped to this requirement

## 1. Statement

`motoreel-typeset` must set a short inline formula, written the way it reads
in Unicode («(e₁e₂)² = −1», «z i³ = −z i», «√−1»), as contours: subscript and
superscript digits as real scripts (smaller and shifted, not the font's
floating Unicode forms), the minus sign as wide as the plus, a radical with a
bar over its radicand, and each letter upright or italic by the caller's
notation rule.

## 2. Rationale

Explainer videos are full of one-line formulas, and a creator who is not a
TeX user writes them as text. Set as plain text, they look wrong in the ways
every physics or maths viewer notices: ₁ floats above where an index sits, ²
sticks to the letter or drifts away, − is narrower than +, and √ alone in a
mono face reads as a V. The sibling `rotorf-sico` studio solved each of these
in `mathematicus/motor/src/formula.rs` (2026-10-14, after design reviews of
three reels), and both its scenes and its subtitles use it. The rules are
typography, not a brand; the owner decided they belong here (rotorf-sico
`docs/decisions.md`, 2026-10-14). Which letters are italic is notation, and
stays the caller's (R-0015 AC1).

## 3. Acceptance criteria

- **AC1. Reading.** `parse(text, italic)` reads a formula into pieces: the
  digits ₀–₉ as subscripts, ⁰–⁹ as superscripts, a space as a thin space,
  `√` followed by a run without spaces as a radical over that run (read
  recursively), every other character as one glyph, upright or italic as
  `italic(char, next)` decides.
- **AC2. Scripts.** A subscript is set at 0.72 of the size and lowered
  0.22 em; a superscript at 0.62 (never below the style's minimum size), raised
  0.40 em and pulled 0.04 em toward its base. The factors are fields of the
  style, with these defaults.
- **AC3. The minus.** U+2212 keeps its advance and is stretched horizontally,
  about its centre, to the ink width of the face's `+`.
- **AC4. The radical.** The `√` glyph, then the radicand at 0.82 of the size
  after a 0.04 em gap, then a 0.08 em tail; a bar 0.094 em thick runs from the
  √'s top-right tip (overlapping it by 0.6 bar thickness) to the end of the
  tail, with its top at the tip, wound like the √'s own contours so the nonzero
  fill adds rather than cancels.
- **AC5. Width and contours.** `set(pieces, style, size, fonts)` returns
  the contours and the advance (R-0014's `Outline`); `width` is the same
  advance without building contours, and both are deterministic.
- **AC6. A drop-in for the studio it came from.** With the studio's faces,
  sizes and italic rule, the contours and advances equal those of rotorf-sico's
  `formula.rs` for every formula its reels set (verified out of tree, md5 of
  the rendered reels, as R-0009 of guion-video-creator did for the studio).

## 4. Constraints & non-goals

- On R-0014's outlines; no new dependency.
- Not here: fractions, stacked limits, matrices, an equation editor, or a
  TeX parser. One line, as written.

## 5. Open questions

- OQ-1. Should the radical read its radicand up to a space (today) or take a
  parenthesised group (`√(a+b)`)? Recommend: also the group, as an addition
  that does not change the space rule.
- OQ-2. Should scripts beyond digits (ₐ, ⁿ, ⁻) be read? Recommend yes, in
  the same tables, once a reel needs one.

## 6. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| | | |

## Changelog

- 2026-10-14 — created (Discussing) with SPEC-0015 and the red tests
  (`crates/motoreel-typeset/tests/r0015_formulas.rs`, behind the empty
  feature `r0015`).
