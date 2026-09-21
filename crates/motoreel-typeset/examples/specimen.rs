//! Set the exact lines the engine used to mangle, to a P6 PPM.
//!
//! ```text
//! cargo run -p motoreel-typeset --example specimen -- Face.ttf out.ppm
//! ```
//!
//! The reference is a frame of the reel-09 template rendered on
//! 2026-09-20, which read «?POR QU? TANTO?» and «b?ceps». If this
//! specimen reads the same, nothing has been fixed.

use motoreel_typeset::{measure, place, rasterize, Align, Face, Fonts};

const W: usize = 1080;
const H: usize = 560;
const PAPER: [u8; 3] = [242, 230, 208];
const INK: [u8; 3] = [20, 16, 12];

/// What the engine drew, and what it should have drawn.
const LINES: &[(&str, f64)] = &[
    ("¿POR QUÉ TANTO?", 92.0),
    ("bíceps · 161 N", 64.0),
    ("DE TERCER GÉNERO", 56.0),
    ("τ = F × L × sen(φ)   ángulo 30°", 44.0),
];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let (Some(font), Some(out)) = (args.next(), args.next()) else {
        eprintln!("uso: specimen <fuente.ttf> <salida.ppm>");
        std::process::exit(2);
    };

    let mut fonts = Fonts::new();
    let id = fonts.add(Face::load(std::fs::read(&font)?, font.clone())?);

    // Report before drawing: a face that cannot set the specimen should
    // say so here, not leave a hole in the image.
    let face = fonts.get(id)?;
    for (text, _) in LINES {
        let gaps = face.missing(text);
        if !gaps.is_empty() {
            eprintln!("{font}: no puede escribir {gaps:?} de {text:?}");
        }
    }

    let mut px = vec![0u8; W * H * 3];
    for chunk in px.as_chunks_mut::<3>().0 {
        *chunk = PAPER;
    }

    let mut y = H as f64 - 130.0;
    for (text, size) in LINES {
        let run = match measure(text, id, *size, &fonts) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("  saltada: {e}");
                y -= size * 1.7;
                continue;
            }
        };
        let layout = place(&run, (W as f64 / 2.0, y), Align::Center);
        if let Some((l, b, r, t)) = layout.ink_bounds() {
            println!("{text:?}\n  caja {l:.1}..{r:.1} x {b:.1}..{t:.1}  (sin rasterizar)");
        }
        rasterize(&layout, &fonts, |x, y, a| {
            if x < 0 || y < 0 || x >= W as i64 || y >= H as i64 {
                return;
            }
            // y-up to the buffer's y-down rows.
            let i = ((H - 1 - y as usize) * W + x as usize) * 3;
            for c in 0..3 {
                let over = f64::from(INK[c]);
                let under = f64::from(px[i + c]);
                px[i + c] = a.mul_add(over - under, under).round() as u8;
            }
        })?;
        y -= size * 1.7;
    }

    let mut bytes = format!("P6\n{W} {H}\n255\n").into_bytes();
    bytes.extend_from_slice(&px);
    std::fs::write(&out, bytes)?;
    println!("-> {out}");
    Ok(())
}
