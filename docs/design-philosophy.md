# Filosofía de diseño — motoreel

## Principios

- Motors are the only motion representation — no Euler angles in paths.
- Easing remaps time, not geometry.
- Deterministic offline rendering for reproducible output.
- Requirement-driven SDLC (R-NNNN → spec → test → impl).

## Contexto

Offline deterministic animation engine: keyframed or physics-recorded motor tracks → numbered frames → video.

## Trade-offs explícitos

Este proyecto prioriza coherencia con los principios anteriores sobre conveniencia ad-hoc.
Cuando una decisión contradice un principio, debe documentarse como ADR o RFC.

## Relación con el ecosistema

- **garust**
- **guion**
- **fisicobuenfisico**
