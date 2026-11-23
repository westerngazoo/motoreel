"""What the house's Blender scripts share: an empty scene, one object from
a bmesh builder, and the OBJ export motoreel reads (R-0020).

Imported by `props.py` and `anatomy.py`; Blender runs a script with its
own folder off `sys.path`, so each adds it first.
"""

import os

import bmesh
import bpy


def reset():
    """An empty scene with factory settings: nothing from the user's
    preferences reaches the output."""
    bpy.ops.wm.read_factory_settings(use_empty=True)


def add_object(name, materials, build, reverse=False):
    """Links a new object `name` whose mesh `build(bm)` fills. `materials`
    is the list of slot names; `build` sets each face's `material_index`.
    `reverse` turns every face inside out (an ink hull)."""
    mesh = bpy.data.meshes.new(name)
    bm = bmesh.new()
    build(bm)
    if reverse:
        bmesh.ops.reverse_faces(bm, faces=bm.faces)
    bm.to_mesh(mesh)
    bm.free()
    for material in materials:
        mat = bpy.data.materials.get(material) or bpy.data.materials.new(material)
        mesh.materials.append(mat)
    obj = bpy.data.objects.new(name, mesh)
    bpy.context.scene.collection.objects.link(obj)


def export(out_dir, name):
    """The scene as `out_dir/name.obj` in Blender's frame (+z up, -y the
    front, metres), geometry and `usemtl` only: no normals, no UVs, no
    `.mtl` (colours are the reel's, bound by name)."""
    os.makedirs(out_dir, exist_ok=True)
    path = os.path.join(out_dir, f"{name}.obj")
    bpy.ops.wm.obj_export(
        filepath=path,
        export_materials=True,  # writes `usemtl`; the .mtl is removed below
        export_normals=False,
        export_uv=False,
        export_triangulated_mesh=False,
        forward_axis="Y",
        up_axis="Z",
    )
    os.remove(os.path.join(out_dir, f"{name}.mtl"))
    with open(path) as f:
        lines = [l for l in f if not l.startswith("mtllib ")]
    with open(path, "w") as f:
        f.writelines(lines)
