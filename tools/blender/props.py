"""Low-poly gym props for motoreel, built headless in Blender (R-0020).

    /Applications/Blender.app/Contents/MacOS/Blender -b --factory-startup \
        --python tools/blender/props.py -- assets/props

Blender only models; motoreel renders (R-0020 §2). Every prop is built from
fixed parameters with bmesh, no randomness and no user preferences, and
exported as OBJ in Blender's native frame (+z up, -y the front, metres),
which motoreel reads with `Axes::ZUp`.

Conventions the loader and the house look rely on:

* Each part is its own OBJ object, so motoreel's depth sort orders parts
  as wholes (R-0018 AC4). Parts are convex and touch without crossing.
* Each outlined part is preceded by its ink hull: the part grown by `INK`
  with its faces reversed, material `ink` (or `skin_ink` on a skin). Back
  face culling then keeps the hull's far side, the house outline.
* Material names are the binding keys. A first word of `skin` or `inner`
  marks the layer the x-ray finish reads (`skin_shroud`, `inner_stack`).
* Objects are named `NNx_name` so file order is build order.
"""

import os
import sys

import bmesh
from mathutils import Matrix

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import house  # noqa: E402  (after the path: Blender leaves the script's folder off it)

INK = 0.01  # ink hull offset, metres (about 3 px at the example's framing)
SIDES = 8  # facets around a tube


class Prop:
    """One prop: a scene of parts, exported as one OBJ."""

    def __init__(self, name):
        self.name = name
        self.count = 0
        house.reset()

    def part(self, name, material, shape, ink="ink"):
        """`shape(grow)` adds the part to a bmesh with every size grown by
        `grow`; `ink` is the hull's material, or None for no outline."""
        if ink is not None:
            house.add_object(f"{self.count:02d}a_{name}_ink", [ink], shape(INK), reverse=True)
        house.add_object(f"{self.count:02d}b_{name}", [material], shape(0.0))
        self.count += 1

    def export(self, out_dir):
        house.export(out_dir, self.name)


def box(lo, hi):
    """An axis-aligned box from `lo` to `hi`; `grow` pads every side."""

    def shape(grow):
        def build(bm):
            size = [h - l + 2 * grow for l, h in zip(lo, hi)]
            centre = [(l + h) / 2 for l, h in zip(lo, hi)]
            m = Matrix.Translation(centre) @ Matrix.Diagonal((*size, 1.0))
            bmesh.ops.create_cube(bm, size=1.0, matrix=m)

        return build

    return shape


def tube(axis, a, b, at, r, sides=SIDES):
    """A capped prism of circumradius `r` along `axis` ('x', 'y' or 'z')
    from `a` to `b`, its axis through `at` (the other two coordinates).
    Axis-aligned, so each side quad stays planar after the OBJ's six
    decimals."""

    def shape(grow):
        def build(bm):
            depth = b - a + 2 * grow
            mid = (a + b) / 2
            turn = {
                "x": Matrix.Rotation(1.5707963267948966, 4, "Y"),
                "y": Matrix.Rotation(-1.5707963267948966, 4, "X"),
                "z": Matrix.Identity(4),
            }[axis]
            centre = {
                "x": (mid, at[0], at[1]),
                "y": (at[0], mid, at[1]),
                "z": (at[0], at[1], mid),
            }[axis]
            bmesh.ops.create_cone(
                bm,
                cap_ends=True,
                cap_tris=False,
                segments=sides,
                radius1=r + grow,
                radius2=r + grow,
                depth=depth,
                matrix=Matrix.Translation(centre) @ turn,
            )

        return build

    return shape


def power_rack():
    """Four posts, base and top rails between them, a pull-up bar over the
    front, two J-hooks on the front posts at 1.35 m. 1.23 x 1.17 x 2.29 m."""
    p = Prop("power_rack")
    t, xs, ys, top = 0.035, 0.58, 0.55, 2.25  # post half-thickness, post centres
    for sx in (-1, 1):
        for sy in (-1, 1):
            x, y = sx * xs, sy * ys
            p.part("post", "steel", box((x - t, y - t, 0.0), (x + t, y + t, top)))
    for sx in (-1, 1):
        x = sx * xs
        for z0 in (0.0, top - 2 * t):
            p.part("side_rail", "steel", box((x - t, -ys + t, z0), (x + t, ys - t, z0 + 2 * t)))
    for z0 in (0.0, top - 2 * t):
        p.part("back_rail", "steel", box((-xs + t, ys - t, z0), (xs - t, ys + t, z0 + 2 * t)))
    p.part("pullup_bar", "chrome", tube("x", -xs - 0.08, xs + 0.08, (-ys, top + 0.022), 0.02))
    for sx in (-1, 1):
        x = sx * xs
        y0 = -ys - t  # the post's front face
        p.part("jhook_plate", "steel", box((x - 0.045, y0 - 0.02, 1.28), (x + 0.045, y0, 1.42)))
        p.part("jhook_tray", "pad", box((x - 0.045, y0 - 0.09, 1.28), (x + 0.045, y0 - 0.02, 1.30)))
        p.part("jhook_lip", "pad", box((x - 0.045, y0 - 0.09, 1.30), (x + 0.045, y0 - 0.075, 1.36)))
    return p


def cable_machine():
    """A lat pulldown: everything on one 0.36 m wide spine at the back (a
    shroud over the weight stack, the arm over the top) and the seat in
    front, so a lifter beside it (|x| > 0.6 m) is never behind it. The
    shroud is skin and the stack inner, so the x-ray shows the plates."""
    p = Prop("cable_machine")
    p.part("base", "steel", box((-0.3, -0.7, 0.0), (0.3, 0.45, 0.05)))
    p.part("shroud", "skin_shroud", box((-0.18, 0.15, 0.05), (0.18, 0.45, 1.95)), ink="skin_ink")
    for k in range(8):
        z0 = 0.1 + 0.06 * k
        p.part("plate", "inner_stack", box((-0.12, 0.22, z0), (0.12, 0.38, z0 + 0.05)), ink=None)
    p.part("arm", "steel", box((-0.05, -0.55, 1.95), (0.05, 0.45, 2.05)))
    p.part("pulley", "chrome", tube("x", -0.02, 0.02, (-0.5, 1.88), 0.07))
    p.part("cable", "ink", box((-0.006, -0.506, 1.556), (0.006, -0.494, 1.81)), ink=None)
    p.part("lat_bar", "chrome", tube("x", -0.55, 0.55, (-0.5, 1.54), 0.016))
    p.part("seat_post", "steel", box((-0.035, -0.385, 0.05), (0.035, -0.315, 0.42)))
    p.part("seat", "pad", box((-0.2, -0.5, 0.42), (0.2, -0.2, 0.5)))
    p.part("pad_post", "steel", box((-0.025, -0.25, 0.5), (0.025, -0.2, 0.63)))
    p.part("thigh_pad", "pad", tube("x", -0.2, 0.2, (-0.225, 0.68), 0.05))
    return p


def flat_bench():
    """A flat bench along x: pad at 0.44 m, a spine, two legs, two feet."""
    p = Prop("flat_bench")
    p.part("pad", "pad", box((-0.6, -0.15, 0.36), (0.6, 0.15, 0.44)))
    p.part("spine", "steel", box((-0.5, -0.04, 0.30), (0.5, 0.04, 0.36)))
    for sx in (-1, 1):
        x = sx * 0.45
        p.part("leg", "steel", box((x - 0.03, -0.04, 0.05), (x + 0.03, 0.04, 0.30)))
        p.part("foot", "steel", box((x - 0.04, -0.22, 0.0), (x + 0.04, 0.22, 0.05)))
    return p


def dumbbell():
    """A hex dumbbell along x, its origin at the handle's centre."""
    p = Prop("dumbbell")
    p.part("handle", "chrome", tube("x", -0.062, 0.062, (0.0, 0.0), 0.018))
    for sx in (-1, 1):
        a, b = sorted((sx * 0.062, sx * 0.115))
        p.part("head", "steel", tube("x", a, b, (0.0, 0.0), 0.07, sides=6))
    return p


def barbell():
    """A 2.2 m bar along x with two 0.45 m plates a side, its origin at the
    bar's centre. Parts butt end to end, so nothing crosses."""
    p = Prop("barbell")
    p.part("bar", "chrome", tube("x", -0.62, 0.62, (0.0, 0.0), 0.014))
    for sx in (-1, 1):
        def seg(a, b):
            return sorted((sx * a, sx * b))

        p.part("collar", "chrome", tube("x", *seg(0.62, 0.66), (0.0, 0.0), 0.035))
        p.part("plate", "rubber", tube("x", *seg(0.66, 0.71), (0.0, 0.0), 0.225, sides=12))
        p.part("plate", "rubber", tube("x", *seg(0.71, 0.76), (0.0, 0.0), 0.225, sides=12))
        p.part("clip", "chrome", tube("x", *seg(0.76, 0.79), (0.0, 0.0), 0.03))
        p.part("sleeve", "chrome", tube("x", *seg(0.79, 1.10), (0.0, 0.0), 0.025))
    return p


def main():
    out_dir = sys.argv[sys.argv.index("--") + 1] if "--" in sys.argv else "assets/props"
    for build in (power_rack, cable_machine, flat_bench, dumbbell, barbell):
        build().export(out_dir)


main()
