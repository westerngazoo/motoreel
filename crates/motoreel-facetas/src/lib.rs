//! SPIKE — faceted low-poly 3D on motoreel, with interchangeable finishes.
//!
//! Not a requirement yet: a working sample for the owner to look at while
//! the requirement («3D de facetas», «Acabado intercambiable») and its ADR
//! are written. Nothing in `motoreel` changes; this crate only emits the
//! `Prim2` paths motoreel already rasterizes, so antialiasing, gradients
//! and byte-determinism are motoreel's.
//!
//! The layers, bottom to top:
//!
//! 1. [`mesh`]: convex low-poly solids as triangle lists;
//! 2. [`pipeline::Part`]: a mesh, a [`Material`] and a garust motor pose;
//! 3. [`pipeline::faces`]: world transform, back-face cull, Lambert from
//!    one key [`Light`], projection through motoreel's pinhole
//!    [`motoreel::Camera`] (posed by an [`Orbit`]), and a painter's sort;
//! 4. [`Acabado`]: the seam — one lit [`Face`] in, motoreel primitives
//!    out. [`Facetas`], [`Pintura`], [`RayosX`], and [`Barrido`] to split
//!    the frame between two of them.

pub mod acabado;
pub mod barrido;
pub mod camera;
pub mod face;
pub mod facetas;
pub mod light;
pub mod material;
pub mod mesh;
pub mod pintura;
pub mod pipeline;
pub mod rayos_x;
pub mod vec3;

pub use acabado::Acabado;
pub use barrido::Barrido;
pub use camera::Orbit;
pub use face::Face;
pub use facetas::Facetas;
pub use light::Light;
pub use material::{Layer, Material, Tones};
pub use mesh::Mesh;
pub use pintura::{Brush, Pintura};
pub use pipeline::{faces, paint, Part};
pub use rayos_x::RayosX;
