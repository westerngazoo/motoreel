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

## Punto de entrada recomendado

Empieza por el README del proyecto y el módulo/crate principal listado en la documentación de arquitectura.
