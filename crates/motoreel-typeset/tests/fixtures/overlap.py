"""Builds overlap.ttf, the R-0013 fixture: a face with two glyphs.

- 'X': two squares that overlap in the middle, both wound the same way.
  Real faces ship such contours at stroke junctions (JetBrains Mono 700,
  'g' and 'r'); a renderer that sums coverage per contour sees 2 there.
- 'U': one rectangle, the union of the two. What 'X' must look like.

Run with fontTools (`pip install fonttools`):  python3 overlap.py
The output is committed; this script is how to remake it.
"""
from pathlib import Path

from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen


def rect(pen, x0, y0, x1, y1):
    # TrueType outer contours run clockwise
    pen.moveTo((x0, y0))
    pen.lineTo((x0, y1))
    pen.lineTo((x1, y1))
    pen.lineTo((x1, y0))
    pen.closePath()


fb = FontBuilder(1000, isTTF=True)
fb.setupGlyphOrder([".notdef", "X", "U"])
fb.setupCharacterMap({ord("X"): "X", ord("U"): "U"})
glyphs = {}
pen = TTGlyphPen(None)
glyphs[".notdef"] = pen.glyph()
pen = TTGlyphPen(None)
rect(pen, 100, 0, 600, 700)
rect(pen, 300, 0, 800, 700)
glyphs["X"] = pen.glyph()
pen = TTGlyphPen(None)
rect(pen, 100, 0, 800, 700)
glyphs["U"] = pen.glyph()
fb.setupGlyf(glyphs)
fb.setupHorizontalMetrics({".notdef": (500, 0), "X": (900, 100), "U": (900, 100)})
fb.setupHorizontalHeader(ascent=800, descent=-200)
fb.setupNameTable({"familyName": "Overlap", "styleName": "Regular"})
fb.setupOS2(sTypoAscender=800, sTypoDescender=-200, usWinAscent=800, usWinDescent=200)
fb.setupPost()
fb.save(str(Path(__file__).with_name("overlap.ttf")))
