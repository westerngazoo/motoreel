# Recorrido del código — motoreel

Guía orientada a desarrolladores para entender dónde vive cada responsabilidad.

### 1. Scene

Build `Scene` with objects and camera; motion lives in `Track<Motor3>`.

### 2. Keyframe path

Insert motor keyframes + `Ease` → interpolate via garust slerp.

### 3. Physics path

Run `garust-physics` → `record()` → same track render walk.

### 4. Render

Walk timeline → project through camera → write SVG/PPM via `FrameSink`.

### 5. Fills: flat and gradient

`Style::fill` is a `Fill { paint, alpha }`. `Paint::Flat(Rgb)` is the
R-0012 flat colour (`Fill::solid`); `Paint::Gradient` holds a linear or
radial `Gradient` (R-0016, `gradient.rs`), built by validating
constructors that return a `GradientError`.

- **Geometry rides the object.** A gradient is a three-point frame in the
  model plane `z = 0`; `Scene::eval` maps it with the path's control points
  (`scene.rs`, `pose_fill`).
- **Colour.** Stops interpolate in sRGB, unpremultiplied, with pad spread.
- **PPM.** `ppm.rs` picks an `Ink` per fill: flat fills composite exactly
  as before; gradients are evaluated per pixel centre and quantised with an
  8 × 8 Bayer ordered dither (a function of the pixel coordinates only), so
  slow ramps do not band and the bytes stay deterministic.
- **SVG.** `svg.rs` writes a native `<linearGradient>` / `<radialGradient>`
  (`gradientTransform` = the frame, ids `g0`, `g1`… per frame) before the
  path that uses it.

## Punto de entrada recomendado

Empieza por el README del proyecto y el módulo/crate principal listado en la documentación de arquitectura.
