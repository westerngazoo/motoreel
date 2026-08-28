//! The embedded face: one const table, one lookup. No types.
//!
//! A 5 × 7 bitmap face on a 6 × 8 cell, covering printable ASCII
//! `0x20..=0x7E`. Row-major so the table reads as ASCII art: one `u8` per
//! row, low 5 bits, `0b10000` is the leftmost column.
//!
//! The baseline sits 6 font-pixels below the cell top — rows 0..=5 are the
//! cap zone, row 6 is the descender, row 7 (not stored) is the line gap.
//! Capitals and digits therefore span rows 0..=5; lowercase x-height spans
//! rows 2..=5; only `g j p q y` (and `,` `;`) reach row 6.
//!
//! The face is reviewable in exactly one place: `tests/golden/font_specimen.ppm`,
//! which renders all 95 glyphs. A mis-authored glyph is a fixture diff.

/// Cell width in font-pixels: 5 columns of ink plus one of advance.
pub(crate) const CELL_W: i64 = 6;
/// Cell height in font-pixels: 7 stored rows plus one of line gap.
pub(crate) const CELL_H: i64 = 8;
/// Rows from the cell top down to the baseline.
pub(crate) const BASELINE: i64 = 6;

const FIRST: u8 = 0x20;

#[rustfmt::skip]
const GLYPHS: [[u8; 7]; 95] = [
    [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000], // ' '
    [0b00100, 0b00100, 0b00100, 0b00100, 0b00000, 0b00100, 0b00000], // '!'
    [0b01010, 0b01010, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000], // '"'
    [0b01010, 0b11111, 0b01010, 0b11111, 0b01010, 0b00000, 0b00000], // '#'
    [0b00100, 0b01111, 0b10100, 0b01110, 0b00101, 0b11110, 0b00100], // '$'
    [0b11001, 0b11010, 0b00010, 0b00100, 0b01011, 0b10011, 0b00000], // '%'
    [0b01100, 0b10010, 0b01100, 0b10010, 0b10001, 0b01110, 0b00000], // '&'
    [0b00100, 0b00100, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000], // "'"
    [0b00010, 0b00100, 0b01000, 0b01000, 0b00100, 0b00010, 0b00000], // '('
    [0b01000, 0b00100, 0b00010, 0b00010, 0b00100, 0b01000, 0b00000], // ')'
    [0b00000, 0b10101, 0b01110, 0b11111, 0b01110, 0b10101, 0b00000], // '*'
    [0b00000, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0b00000], // '+'
    [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00110, 0b01000], // ','
    [0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000], // '-'
    [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00110, 0b00000], // '.'
    [0b00001, 0b00010, 0b00100, 0b00100, 0b01000, 0b10000, 0b00000], // '/'
    [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b01110, 0b00000], // '0'
    [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b01110, 0b00000], // '1'
    [0b01110, 0b10001, 0b00001, 0b00110, 0b01000, 0b11111, 0b00000], // '2'
    [0b11111, 0b00010, 0b00110, 0b00001, 0b10001, 0b01110, 0b00000], // '3'
    [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00000], // '4'
    [0b11111, 0b10000, 0b11110, 0b00001, 0b10001, 0b01110, 0b00000], // '5'
    [0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b01110, 0b00000], // '6'
    [0b11111, 0b00001, 0b00010, 0b00100, 0b00100, 0b00100, 0b00000], // '7'
    [0b01110, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110, 0b00000], // '8'
    [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b01100, 0b00000], // '9'
    [0b00000, 0b00000, 0b00110, 0b00000, 0b00110, 0b00000, 0b00000], // ':'
    [0b00000, 0b00000, 0b00110, 0b00000, 0b00110, 0b00100, 0b01000], // ';'
    [0b00010, 0b00100, 0b01000, 0b01000, 0b00100, 0b00010, 0b00000], // '<'
    [0b00000, 0b00000, 0b11111, 0b00000, 0b11111, 0b00000, 0b00000], // '='
    [0b01000, 0b00100, 0b00010, 0b00010, 0b00100, 0b01000, 0b00000], // '>'
    [0b01110, 0b10001, 0b00010, 0b00100, 0b00000, 0b00100, 0b00000], // '?'
    [0b01110, 0b10001, 0b10111, 0b10111, 0b10000, 0b01110, 0b00000], // '@'
    [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b00000], // 'A'
    [0b11110, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110, 0b00000], // 'B'
    [0b01110, 0b10001, 0b10000, 0b10000, 0b10001, 0b01110, 0b00000], // 'C'
    [0b11100, 0b10010, 0b10001, 0b10001, 0b10010, 0b11100, 0b00000], // 'D'
    [0b11111, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111, 0b00000], // 'E'
    [0b11111, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000, 0b00000], // 'F'
    [0b01110, 0b10001, 0b10000, 0b10011, 0b10001, 0b01110, 0b00000], // 'G'
    [0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001, 0b00000], // 'H'
    [0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110, 0b00000], // 'I'
    [0b00111, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100, 0b00000], // 'J'
    [0b10001, 0b10010, 0b11100, 0b10010, 0b10010, 0b10001, 0b00000], // 'K'
    [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111, 0b00000], // 'L'
    [0b10001, 0b11011, 0b10101, 0b10001, 0b10001, 0b10001, 0b00000], // 'M'
    [0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b00000], // 'N'
    [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110, 0b00000], // 'O'
    [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b00000], // 'P'
    [0b01110, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101, 0b00000], // 'Q'
    [0b11110, 0b10001, 0b10001, 0b11110, 0b10010, 0b10001, 0b00000], // 'R'
    [0b01111, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110, 0b00000], // 'S'
    [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00000], // 'T'
    [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110, 0b00000], // 'U'
    [0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100, 0b00000], // 'V'
    [0b10001, 0b10001, 0b10001, 0b10101, 0b11011, 0b10001, 0b00000], // 'W'
    [0b10001, 0b01010, 0b00100, 0b00100, 0b01010, 0b10001, 0b00000], // 'X'
    [0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100, 0b00000], // 'Y'
    [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111, 0b00000], // 'Z'
    [0b00111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00111, 0b00000], // '['
    [0b10000, 0b01000, 0b00100, 0b00100, 0b00010, 0b00001, 0b00000], // '\\'
    [0b11100, 0b00100, 0b00100, 0b00100, 0b00100, 0b11100, 0b00000], // ']'
    [0b00100, 0b01010, 0b10001, 0b00000, 0b00000, 0b00000, 0b00000], // '^'
    [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b11111, 0b00000], // '_'
    [0b01000, 0b00100, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000], // '`'
    [0b00000, 0b00000, 0b01110, 0b00001, 0b01111, 0b01111, 0b00000], // 'a'
    [0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b11110, 0b00000], // 'b'
    [0b00000, 0b00000, 0b01110, 0b10000, 0b10000, 0b01110, 0b00000], // 'c'
    [0b00001, 0b00001, 0b01111, 0b10001, 0b10001, 0b01111, 0b00000], // 'd'
    [0b00000, 0b00000, 0b01110, 0b11111, 0b10000, 0b01110, 0b00000], // 'e'
    [0b00110, 0b01000, 0b11110, 0b01000, 0b01000, 0b01000, 0b00000], // 'f'
    [0b00000, 0b00000, 0b01101, 0b10010, 0b10010, 0b01110, 0b00010], // 'g'
    [0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b10001, 0b00000], // 'h'
    [0b00100, 0b00000, 0b01100, 0b00100, 0b00100, 0b01110, 0b00000], // 'i'
    [0b00010, 0b00000, 0b00110, 0b00010, 0b00010, 0b10010, 0b01100], // 'j'
    [0b10000, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b00000], // 'k'
    [0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110, 0b00000], // 'l'
    [0b00000, 0b00000, 0b11010, 0b10101, 0b10101, 0b10001, 0b00000], // 'm'
    [0b00000, 0b00000, 0b11110, 0b10001, 0b10001, 0b10001, 0b00000], // 'n'
    [0b00000, 0b00000, 0b01110, 0b10001, 0b10001, 0b01110, 0b00000], // 'o'
    [0b00000, 0b00000, 0b11110, 0b10001, 0b11110, 0b10000, 0b10000], // 'p'
    [0b00000, 0b00000, 0b01111, 0b10001, 0b01111, 0b00001, 0b00001], // 'q'
    [0b00000, 0b00000, 0b10110, 0b11000, 0b10000, 0b10000, 0b00000], // 'r'
    [0b00000, 0b00000, 0b01111, 0b01100, 0b00011, 0b11110, 0b00000], // 's'
    [0b01000, 0b01000, 0b11110, 0b01000, 0b01001, 0b00110, 0b00000], // 't'
    [0b00000, 0b00000, 0b10001, 0b10001, 0b10001, 0b01111, 0b00000], // 'u'
    [0b00000, 0b00000, 0b10001, 0b10001, 0b01010, 0b00100, 0b00000], // 'v'
    [0b00000, 0b00000, 0b10001, 0b10101, 0b10101, 0b01010, 0b00000], // 'w'
    [0b00000, 0b00000, 0b10001, 0b01010, 0b01010, 0b10001, 0b00000], // 'x'
    [0b00000, 0b00000, 0b10001, 0b10001, 0b01111, 0b00001, 0b01110], // 'y'
    [0b00000, 0b00000, 0b11111, 0b00010, 0b01000, 0b11111, 0b00000], // 'z'
    [0b00011, 0b00100, 0b01100, 0b00100, 0b00100, 0b00011, 0b00000], // '{'
    [0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00000], // '|'
    [0b11000, 0b00100, 0b00110, 0b00100, 0b00100, 0b11000, 0b00000], // '}'
    [0b00000, 0b00000, 0b01001, 0b10110, 0b00000, 0b00000, 0b00000], // '~'
];

/// The glyph for `byte`. `Prim2::Text`'s invariant makes the fallback
/// unreachable; it returns `'?'` rather than panicking, with the same
/// `debug_assert` tripwire pattern `svg.rs` and `ppm.rs` use for
/// SPEC-0002's finite-coordinate contract.
pub(crate) fn glyph(byte: u8) -> &'static [u8; 7] {
    debug_assert!(
        (0x20..=0x7E).contains(&byte),
        "Prim2::Text's ASCII invariant was violated"
    );
    GLYPHS
        .get(byte.wrapping_sub(FIRST) as usize)
        .unwrap_or(&GLYPHS[(b'?' - FIRST) as usize])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Render one glyph as ASCII art, the form the table is authored in.
    fn art(c: char) -> Vec<String> {
        glyph(c as u8)
            .iter()
            .map(|row| {
                (0..5)
                    .map(|i| if row & (0b10000 >> i) != 0 { '#' } else { '.' })
                    .collect()
            })
            .collect()
    }

    // AC6(a) — 95 entries, every row within the 5-bit cell.
    #[test]
    fn the_face_covers_printable_ascii_in_five_columns() {
        assert_eq!(GLYPHS.len(), 95, "0x20..=0x7E is 95 code points");
        for (i, g) in GLYPHS.iter().enumerate() {
            for (r, row) in g.iter().enumerate() {
                assert!(
                    *row <= 0b11111,
                    "glyph {:?} row {r} overflows the 5-column cell",
                    (FIRST + i as u8) as char
                );
            }
        }
    }

    // AC6(a) — 'A' and 'g' pinned as ASCII art. 'A' is a cap: rows 0..=5,
    // nothing on the descender row. 'g' is the descender witness.
    #[test]
    fn capital_a_is_pinned() {
        assert_eq!(
            art('A'),
            vec![".###.", "#...#", "#...#", "#####", "#...#", "#...#", "....."]
        );
    }

    #[test]
    fn lowercase_g_is_pinned_with_its_descender() {
        assert_eq!(
            art('g'),
            vec![".....", ".....", ".##.#", "#..#.", "#..#.", ".###.", "...#."]
        );
        assert_ne!(glyph(b'g')[6], 0, "'g' must reach the descender row");
        assert_eq!(glyph(b'A')[6], 0, "'A' must not");
    }

    // AC6(a) — out of range returns '?', not a panic, in release.
    #[test]
    fn out_of_range_falls_back_to_question_mark() {
        assert_eq!(
            glyph(0x7E),
            &GLYPHS[(0x7E - FIRST) as usize],
            "'~' is in range"
        );
        // `glyph(0)` trips the debug_assert in a debug build, so the
        // fallback is asserted through the table directly — the release
        // behaviour, stated where a reader will find it.
        assert_eq!(
            GLYPHS
                .get(0u8.wrapping_sub(FIRST) as usize)
                .unwrap_or(&GLYPHS[(b'?' - FIRST) as usize]),
            &GLYPHS[(b'?' - FIRST) as usize]
        );
    }

    // The metrics the blit derives everything from.
    #[test]
    fn metrics_are_the_documented_cell() {
        assert_eq!((CELL_W, CELL_H, BASELINE), (6, 8, 6));
    }

    // Every glyph fits above the baseline unless it is a known descender:
    // the property the specimen fixture makes visible.
    #[test]
    fn only_known_descenders_reach_row_six() {
        let expected = "gjpqy,;$";
        for c in 0x20u8..=0x7E {
            let has = glyph(c)[6] != 0;
            assert_eq!(
                has,
                expected.contains(c as char),
                "{:?}: descender row disagrees with the documented set",
                c as char
            );
        }
    }
}
