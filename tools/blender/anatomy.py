"""The toy's anatomy in three layers, built headless in Blender (R-0021).

    /Applications/Blender.app/Contents/MacOS/Blender -b --factory-startup \
        --python tools/blender/anatomy.py -- assets/anatomy

Writes `skin.obj`, `muscle.obj`, `bone.obj` and `joints.tsv`. Original
work: every shape is a capsule, sphere, prism or box from the fixed
parameters below, no third-party model and no randomness.

The figure is the curl example's toy (`examples/curl_orbit/rig.rs`): the
same joints, the same capsule limbs (radii x CHUNK), the same head. It is
written in motoreel's frame (+y up, the toy facing +x, its right side +z)
and turned into Blender's (+z up) only when a vertex is stored, so the
loader's `Axes::ZUp` brings it back exactly as written here.

Rig contract (R-0021 AC4): every object is `segment.part`, the segment one
of `SEGMENTS`; a reel poses a segment with one motor about its joint, and
skin, muscle and bone of that segment follow it together. Every inner part
lies inside its own segment's skin, so motoreel's depth sort draws it
first (R-0021 AC1).
"""

import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import house  # noqa: E402  (after the path: Blender leaves the script's folder off it)

INK = 0.009  # the rig's ink hull offset, metres
CHUNK = 1.25  # the rig's limb radii over the 2D toy's
SIDES = 8
BANDS = 2

# --- vector helpers (motoreel's frame) ---------------------------------------


def add(a, b):
    return [a[0] + b[0], a[1] + b[1], a[2] + b[2]]


def sub(a, b):
    return [a[0] - b[0], a[1] - b[1], a[2] - b[2]]


def scale(a, s):
    return [a[0] * s, a[1] * s, a[2] * s]


def dot(a, b):
    return a[0] * b[0] + a[1] * b[1] + a[2] * b[2]


def cross(a, b):
    return [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]


def normalize(a):
    return scale(a, 1.0 / math.sqrt(dot(a, a)))


def lerp(a, b, t):
    return add(a, scale(sub(b, a), t))


# --- shapes: a port of the curl example's kit.rs ------------------------------


class Shape:
    """Vertices and faces in motoreel's frame, faces outward (convex)."""

    def __init__(self, verts, faces):
        self.verts = verts
        self.faces = faces

    def oriented(self):
        c = scale([sum(p[i] for p in self.verts) for i in range(3)], 1.0 / len(self.verts))
        for f in self.faces:
            p0, p1, p2 = (self.verts[f[k]] for k in range(3))
            if dot(cross(sub(p1, p0), sub(p2, p0)), sub(p0, c)) < 0.0:
                f.reverse()
        return self

    def scaled(self, s, about):
        self.verts = [add(about, [(p[i] - about[i]) * s[i] for i in range(3)]) for p in self.verts]
        return self

    def hull(self):
        for f in self.faces:
            f.reverse()
        return self


def frame(w):
    helper = [0.0, 1.0, 0.0] if abs(w[1]) < 0.9 else [1.0, 0.0, 0.0]
    u = normalize(cross(w, helper))
    return u, cross(w, u)


def lathe(a, b, pole_a, pole_b, rings, sides):
    w = normalize(sub(b, a))
    u, v = frame(w)
    verts = [pole_a]
    for c, r in rings:
        for k in range(sides):
            th = 2.0 * math.pi * k / sides
            verts.append(add(c, add(scale(u, r * math.cos(th)), scale(v, r * math.sin(th)))))
    verts.append(pole_b)
    top = len(verts) - 1

    def at(ring, k):
        return 1 + ring * sides + k % sides

    faces = []
    for k in range(sides):
        faces.append([0, at(0, k + 1), at(0, k)])
        faces.append([top, at(len(rings) - 1, k), at(len(rings) - 1, k + 1)])
    for ring in range(len(rings) - 1):
        for k in range(sides):
            faces.append([at(ring, k), at(ring, k + 1), at(ring + 1, k + 1), at(ring + 1, k)])
    return Shape(verts, faces).oriented()


def capsule(a, b, ra, rb, sides=SIDES, bands=BANDS, most=None):
    """`most` caps the cone rings: a thin bone would otherwise get one
    ring per radius along its length."""
    w = normalize(sub(b, a))
    rings = []
    for i in range(1, bands + 1):
        lat = -math.pi / 2 + math.pi / 2 * i / bands
        rings.append((add(a, scale(w, ra * math.sin(lat))), ra * math.cos(lat)))
    length = math.sqrt(dot(sub(b, a), sub(b, a)))
    mid = math.floor(length / (1.2 * max(ra, rb)))
    if most is not None:
        mid = min(mid, most)
    for j in range(1, mid + 1):
        f = j / (mid + 1)
        rings.append((lerp(a, b, f), ra + (rb - ra) * f))
    for i in range(bands):
        lat = math.pi / 2 * i / bands
        rings.append((add(b, scale(w, rb * math.sin(lat))), rb * math.cos(lat)))
    return lathe(a, b, sub(a, scale(w, ra)), add(b, scale(w, rb)), rings, sides)


def sphere(c, r, sides=SIDES, bands=4):
    up = [0.0, 1.0, 0.0]
    rings = []
    for i in range(1, bands):
        lat = -math.pi / 2 + math.pi * i / bands
        rings.append((add(c, scale(up, r * math.sin(lat))), r * math.cos(lat)))
    return lathe(c, add(c, up), sub(c, scale(up, r)), add(c, scale(up, r)), rings, sides)


def prism(a, b, r, sides):
    w = normalize(sub(b, a))
    u, v = frame(w)
    verts = []
    for end in (a, b):
        for k in range(sides):
            th = 2.0 * math.pi * k / sides
            verts.append(add(end, add(scale(u, r * math.cos(th)), scale(v, r * math.sin(th)))))
    faces = [list(range(sides)), list(range(sides, 2 * sides))]
    for k in range(sides):
        n = (k + 1) % sides
        faces.append([k, n, sides + n, sides + k])
    return Shape(verts, faces).oriented()


def band(a, b, r, sides):
    s = prism(a, b, r, sides)
    s.faces = s.faces[2:]
    return s


def cuboid(lo, hi):
    verts = [
        [hi[0] if i & 1 else lo[0], hi[1] if i & 2 else lo[1], hi[2] if i & 4 else lo[2]]
        for i in range(8)
    ]
    faces = [[0, 1, 3, 2], [4, 6, 7, 5], [0, 4, 5, 1], [2, 3, 7, 6], [0, 2, 6, 4], [1, 5, 7, 3]]
    return Shape(verts, faces).oriented()


def short_capsule(*args, **kwargs):
    """A capsule with at most one cone ring: muscles and bones are short
    next to their radius."""
    return capsule(*args, most=1, **kwargs)


def limb(a, b, ra, rb, grow=0.0):
    """A capsule with the 2D toy's radii made chunky (rig.rs `limb`)."""
    return capsule(a, b, ra * CHUNK + grow, rb * CHUNK + grow)


# --- the layers -----------------------------------------------------------------


class Layer:
    """One OBJ file: objects in build order."""

    def __init__(self, name):
        self.name = name
        house.reset()

    def part(self, segment, part, shape, material, face_material=None):
        """`segment.part` of `material`, or per face `face_material(i,
        centroid)` choosing among a list of names."""
        materials = [material] if face_material is None else material
        verts = shape.verts

        def build(bm):
            bv = [bm.verts.new((p[0], -p[2], p[1])) for p in verts]  # to +z up
            for i, f in enumerate(shape.faces):
                face = bm.faces.new([bv[k] for k in f])
                if face_material is not None:
                    c = scale([sum(verts[k][j] for k in f) for j in range(3)], 1.0 / len(f))
                    face.material_index = face_material(i, c)

        house.add_object(f"{segment}.{part}", materials, build)

    def outlined(self, segment, part, build, material, ink="skin_ink", face_material=None):
        """The part's ink hull (grown, reversed), then the part."""
        self.part(segment, f"{part}_ink", build(INK).hull(), ink)
        self.part(segment, part, build(0.0), material, face_material)

    def export(self, out_dir):
        house.export(out_dir, self.name)


SIDE = {"r": 1.0, "l": -1.0}


def joints_of(s):
    """The rig's joints on side `s` (+1 right, toward +z)."""
    z = lambda v: v * s  # noqa: E731
    return {
        "ankle": [0.0, 0.09, z(0.115)],
        "knee": [0.01, 0.50, z(0.115)],
        "hip": [0.0, 0.92, z(0.105)],
        "shoulder": [0.0, 1.37, z(0.245)],
        "elbow": [-0.01, 1.08, z(0.29)],
        "wrist": [-0.01, 0.78, z(0.31)],
        "hand": [-0.01, 0.75, z(0.315)],
    }


HEAD_C = [0.03, 1.68, 0.0]
HEAD_R = 0.145
TORSO_C = [0.0, 1.17, 0.0]


def torso_shape(g):
    return capsule([0.0, 0.99, 0.0], [0.0, 1.33, 0.0], 0.13 + g, 0.16 + g, 10).scaled(
        [0.95, 1.0, 1.4], TORSO_C
    )


def segments():
    """(segment, parent, joint) rows, the root first."""
    rows = [
        ("pelvis", "-", [0.0, 0.97, 0.0]),
        ("torso", "pelvis", [0.0, 0.99, 0.0]),
        ("neck", "torso", [0.0, 1.40, 0.0]),
        ("head", "neck", [0.01, 1.55, 0.0]),
    ]
    for side, s in SIDE.items():
        j = joints_of(s)
        rows += [
            (f"upper_arm_{side}", "torso", j["shoulder"]),
            (f"forearm_{side}", f"upper_arm_{side}", j["elbow"]),
            (f"hand_{side}", f"forearm_{side}", j["wrist"]),
            (f"thigh_{side}", "pelvis", j["hip"]),
            (f"shin_{side}", f"thigh_{side}", j["knee"]),
            (f"foot_{side}", f"shin_{side}", j["ankle"]),
        ]
    return rows


def skin():
    lay = Layer("skin")
    for side, s in SIDE.items():
        j = joints_of(s)
        lay.outlined(f"shin_{side}", "shin", lambda g: limb(j["ankle"], j["knee"], 0.055, 0.068, g), "skin_blue")
        mid_y = (j["knee"][1] + j["hip"][1]) / 2

        def shorts_or_blue(i, c, mid_y=mid_y):
            return 1 if c[1] > mid_y else 0

        lay.outlined(
            f"thigh_{side}",
            "thigh",
            lambda g: limb(j["knee"], j["hip"], 0.072, 0.1, g),
            ["skin_blue", "skin_shorts"],
            face_material=shorts_or_blue,
        )
        heel, toe = [-0.08, 0.055, 0.115 * s], [0.16, 0.045, 0.115 * s]
        lay.outlined(f"foot_{side}", "shoe", lambda g: capsule(heel, toe, 0.06 + g, 0.05 + g), "skin_shoe")
        sole_c = [0.04, 0.014, 0.115 * s]
        sole = capsule([-0.09, 0.014, 0.115 * s], [0.18, 0.014, 0.115 * s], 0.05, 0.045, SIDES, 1)
        lay.part(f"foot_{side}", "sole", sole.scaled([1.0, 0.3, 1.15], sole_c), "skin_band")
        lay.outlined(
            f"upper_arm_{side}", "upper_arm", lambda g: limb(j["shoulder"], j["elbow"], 0.06, 0.045, g), "skin_blue"
        )
        lay.outlined(f"forearm_{side}", "forearm", lambda g: limb(j["elbow"], j["wrist"], 0.043, 0.04, g), "skin_blue")
        lay.outlined(f"hand_{side}", "hand", lambda g: sphere(j["hand"], 0.06 + g), "skin_blue")
    lay.outlined(
        "pelvis", "hips", lambda g: capsule([0.0, 0.97, -0.12], [0.0, 0.97, 0.12], 0.14 + g, 0.14 + g, 10), "skin_shorts"
    )
    lay.outlined("torso", "torso", torso_shape, "skin_blue")
    lay.outlined("neck", "neck", lambda g: capsule([0.0, 1.40, 0.0], [0.01, 1.55, 0.0], 0.06 + g, 0.06 + g, SIDES, 1), "skin_blue")
    lay.outlined("head", "head", lambda g: sphere(HEAD_C, HEAD_R + g, 10, 5), "skin_blue")
    lay.outlined(
        "head",
        "headband",
        lambda g: band(add(HEAD_C, [0.0, 0.02, 0.0]), add(HEAD_C, [0.0, 0.065, 0.0]), 0.155 + g, 10),
        "skin_band",
    )
    for s in (1.0, -1.0):
        d = normalize([0.93, -0.03, 0.36 * s])
        eye = add(HEAD_C, scale(d, 0.9 * HEAD_R))
        lay.part("head", "eye", sphere(eye, 0.036).scaled([0.45, 1.1, 1.0], eye), "skin_eye")
        pupil = add(HEAD_C, scale(d, 0.99 * HEAD_R))
        lay.part("head", "pupil", sphere(pupil, 0.018, 6, 3).scaled([0.5, 1.0, 1.0], eye), "skin_pupil")
    return lay


def muscle():
    """The groups a reel names, each its own material, sized to fill the
    skin of their segment without leaving it, and `muscle_other` for the
    rest of the figure (face, neck, abs, traps, forearm extensors,
    tibialis, palms, soles), so the muscle layer reads as a body."""
    lay = Layer("muscle")
    lay.part("head", "face", sphere(HEAD_C, 0.12, 10, 5), "muscle_other")
    lay.part("neck", "neck", short_capsule([0.005, 1.42, 0.0], [0.01, 1.53, 0.0], 0.045, 0.045, SIDES, 1), "muscle_other")
    lay.part("torso", "abs", cuboid([0.07, 1.03, -0.05], [0.11, 1.2, 0.05]), "muscle_other")
    traps = [-0.04, 1.36, 0.0]
    lay.part("torso", "traps", sphere(traps, 0.07).scaled([0.6, 0.6, 1.6], traps), "muscle_other")
    for side, s in SIDE.items():
        j = joints_of(s)
        sh, el, wr, hd = j["shoulder"], j["elbow"], j["wrist"], j["hand"]
        up = f"upper_arm_{side}"
        lay.part(up, "deltoid", sphere(add(sh, [0.0, -0.012, 0.01 * s]), 0.046), "muscle_deltoid")
        lay.part(up, "biceps", short_capsule(add(sh, [0.032, -0.1, 0.0]), add(el, [0.027, 0.05, 0.0]), 0.025, 0.016), "muscle_biceps")
        lay.part(up, "triceps", short_capsule(add(sh, [-0.036, -0.08, 0.0]), add(el, [-0.03, 0.06, 0.0]), 0.026, 0.017), "muscle_triceps")
        fore = f"forearm_{side}"
        lay.part(fore, "flexors", short_capsule(add(el, [0.02, -0.04, 0.0]), add(wr, [0.02, 0.05, 0.0]), 0.024, 0.016), "muscle_forearm")
        lay.part(fore, "extensors", short_capsule(add(el, [-0.02, -0.04, 0.0]), add(wr, [-0.02, 0.05, 0.0]), 0.022, 0.015), "muscle_other")
        lay.part(f"hand_{side}", "palm", sphere(hd, 0.045), "muscle_other")
        hip, knee, ankle = j["hip"], j["knee"], j["ankle"]
        th = f"thigh_{side}"
        lay.part(th, "quads", short_capsule(add(knee, [0.04, 0.1, 0.0]), add(hip, [0.045, -0.12, 0.0]), 0.04, 0.055), "muscle_quads")
        lay.part(
            th, "hamstrings", short_capsule(add(knee, [-0.04, 0.12, 0.0]), add(hip, [-0.045, -0.14, 0.0]), 0.036, 0.05), "muscle_hamstrings"
        )
        shin = f"shin_{side}"
        lay.part(shin, "calves", short_capsule(add(knee, [-0.035, -0.08, 0.0]), add(ankle, [-0.025, 0.16, 0.0]), 0.038, 0.02), "muscle_calves")
        lay.part(shin, "tibialis", short_capsule(add(knee, [0.03, -0.06, 0.0]), add(ankle, [0.02, 0.1, 0.0]), 0.022, 0.014), "muscle_other")
        lay.part(f"foot_{side}", "sole", cuboid([-0.075, 0.025, 0.115 * s - 0.035], [0.14, 0.07, 0.115 * s + 0.035]), "muscle_other")
        lay.part("pelvis", "glute", sphere([-0.055, 0.94, 0.06 * s], 0.065), "muscle_glutes")
        pec = [0.07, 1.26, 0.07 * s]
        lay.part("torso", "pec", sphere(pec, 0.075).scaled([0.5, 0.8, 1.0], pec), "muscle_pecs")
        lat = [-0.055, 1.19, 0.1 * s]
        lay.part("torso", "lat", sphere(lat, 0.08).scaled([0.5, 1.3, 0.7], lat), "muscle_lats")
    return lay


def bone():
    lay = Layer("bone")
    lay.part("head", "skull", sphere(HEAD_C, 0.1, 10, 5), "inner_bone")
    lay.part("head", "jaw", cuboid(add(HEAD_C, [0.0, -0.095, -0.045]), add(HEAD_C, [0.06, -0.06, 0.045])), "inner_bone")
    # The spine: seven blocks, the lowest two in the pelvis, one in the neck.
    spine = [("pelvis", 0.90), ("pelvis", 0.95), ("torso", 1.03), ("torso", 1.12), ("torso", 1.21), ("torso", 1.30), ("neck", 1.45)]
    for k, (segment, y) in enumerate(spine, start=1):
        lay.part(segment, f"vertebra_{k}", cuboid([-0.04, y, -0.022], [-0.005, y + 0.04, 0.022]), "inner_bone")
    for k, y in enumerate((1.20, 1.25, 1.30), start=1):
        rib = band([0.0, y, 0.0], [0.0, y + 0.018, 0.0], 0.1, 10).scaled([0.95, 1.0, 1.4], [0.0, y, 0.0])
        lay.part("torso", f"rib_{k}", rib, "inner_bone")
    lay.part("torso", "sternum", cuboid([0.085, 1.18, -0.015], [0.1, 1.33, 0.015]), "inner_bone")
    lay.part("pelvis", "pelvis", prism([0.0, 0.93, -0.1], [0.0, 0.93, 0.1], 0.065, 6), "inner_bone")
    for side, s in SIDE.items():
        j = joints_of(s)
        sh, el, wr, hd = j["shoulder"], j["elbow"], j["wrist"], j["hand"]
        lay.part("torso", "scapula", cuboid([-0.125, 1.24, 0.05 * s - 0.04], [-0.105, 1.34, 0.05 * s + 0.04]), "inner_bone")
        lay.part("torso", "clavicle", short_capsule([0.02, 1.38, 0.03 * s], [0.0, 1.38, 0.17 * s], 0.012, 0.012, 6, 1), "inner_bone")
        lay.part(f"upper_arm_{side}", "humerus", short_capsule(sh, el, 0.016, 0.013, 6, 1), "inner_bone")
        lay.part(f"forearm_{side}", "radius", short_capsule(add(el, [0.004, -0.02, 0.012 * s]), add(wr, [0.004, 0.0, 0.012 * s]), 0.009, 0.009, 6, 1), "inner_forearm")
        lay.part(f"forearm_{side}", "ulna", short_capsule(add(el, [-0.006, -0.02, -0.01 * s]), add(wr, [-0.006, 0.0, -0.01 * s]), 0.009, 0.009, 6, 1), "inner_forearm")
        lay.part(f"hand_{side}", "hand", cuboid(add(hd, [-0.025, -0.035, -0.025]), add(hd, [0.025, 0.02, 0.025])), "inner_bone")
        hip, knee, ankle = j["hip"], j["knee"], j["ankle"]
        lay.part(f"thigh_{side}", "femur", short_capsule(add(hip, [0.0, -0.01, 0.0]), add(knee, [0.0, 0.02, 0.0]), 0.022, 0.018, 6, 1), "inner_bone")
        lay.part(f"shin_{side}", "tibia", short_capsule(add(knee, [0.008, -0.02, 0.0]), add(ankle, [0.008, 0.03, 0.0]), 0.015, 0.012, 6, 1), "inner_bone")
        lay.part(f"shin_{side}", "fibula", short_capsule(add(knee, [-0.006, -0.04, 0.026 * s]), add(ankle, [-0.006, 0.04, 0.026 * s]), 0.007, 0.007, 6, 1), "inner_bone")
        lay.part(f"foot_{side}", "foot", cuboid([-0.07, 0.03, 0.115 * s - 0.03], [0.13, 0.065, 0.115 * s + 0.03]), "inner_bone")
    return lay


def main():
    out_dir = sys.argv[sys.argv.index("--") + 1] if "--" in sys.argv else "assets/anatomy"
    for build in (skin, muscle, bone):
        build().export(out_dir)
    with open(os.path.join(out_dir, "joints.tsv"), "w") as f:
        f.write("segment\tparent\tx\ty\tz\n")
        for segment, parent, (x, y, z) in segments():
            f.write(f"{segment}\t{parent}\t{x!r}\t{y!r}\t{z!r}\n")


main()
