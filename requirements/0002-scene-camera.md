# R-0002 — Scene, objects, motor-posed camera, projection

- **Status:** Met
- **Milestone:** M1
- **Owner:** Gustavo Delgadillo (westerngazoo)
- **Created:** 2026-08-20
- **Depends on:** R-0001
- **Realized by:** SPEC-0002
- **QA:** `qa` agent run scoped to this requirement

## 1. Statement

motoreel must describe a renderable **scene**: a set of objects — geometry
(`Point`, `Segment`, `Polyline` of PGA points) with a stroke style and a
motor track — plus a **camera whose pose is itself a motor** and whose
projection is pinhole or orthographic. Evaluating the scene at time `t`
produces a flat list of 2D primitives (`Prim2`) in image space: every object's
track is evaluated (R-0001), its geometry transformed by the resulting motor,
then viewed through the inverse camera motor and projected.

## 2. Rationale

The bridge from motor mathematics to drawable frames. The camera being a
motor keeps the no-matrices promise (matrices appear only inside projection)
and means camera moves are authored/simulated exactly like object motion.
(RFC-012 §3.2, §3.4, A2.)

## 3. Acceptance criteria

- **AC1.** Golden evaluation: one frame of a known scene at a known `t`
  produces a checked-in `Prim2` list, exactly.
- **AC2.** Camera/world duality: transforming the camera by motor `M` yields
  the same image as transforming the world by `M⁻¹`. (Property-tested.)
- **AC3.** Pinhole projection divides by view-depth with the configured
  focal length; orthographic omits the divide. Both models available on one
  camera type; a point at known coordinates projects to hand-computed values
  under each.
- **AC4.** Object style (stroke color, width, alpha) is carried through to
  the emitted primitives unchanged.
- **AC5.** Objects evaluate their tracks at scene time `t`; a static object
  (single-key track) renders at its fixed pose.
- **AC6.** Deterministic: same scene, same `t` → identical primitive list.

## 4. Constraints & non-goals

- No derived/incidence shapes (`JoinLine`, `MeetPoint`) — that is M3
  (R-0007). No text, fills, gradients, clipping, or depth sorting.
- Style vocabulary is minimal by decision: `stroke`, `width`, `alpha`.
- Behind-camera / at-plane points: a defined, documented policy (cull the
  primitive) — never NaN output.

## 5. Open questions

None open — RFC draft answers adopted (see decision log); owner may veto.

## 6. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-08-20 | Camera offers pinhole **and** orthographic | RFC-012 §6 Q2 draft answer: one enum, two divide rules; ortho is the classic math-diagram look |
| 2026-08-20 | Style vocabulary minimal: stroke, width, alpha | RFC-012 §6 Q3 draft answer; defer gradients/fills until a scene needs them |

## Changelog

- 2026-08-20 — created; accepted adopting RFC-012 draft answers.
- 2026-08-20 — implemented (SPEC-0002) and QA-signed-off: PASS, 52/52
  workspace tests, gates verified independently, test suite byte-unmoved
  through red→green. Merge formalities pending repo setup (owner).
