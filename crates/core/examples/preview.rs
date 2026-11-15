//! Renders the overlay to a PPM image, to judge the drawing without Windows:
//! `cargo run -p padpilot-core --example preview -- out.ppm [scale]`
//! Normal mode on the left, joystick mode (stick pushed up-right) on the right.

use std::io::Write;

use padpilot_core::overlay::layout::{DEFAULT_JOYSTICK_SIDE, Layout};
use padpilot_core::overlay::model::Mode;
use padpilot_core::overlay::paint::{Canvas, paint};
use padpilot_core::stick::Deflection;

fn main() -> std::io::Result<()> {
    let mut args = std::env::args().skip(1);
    let path = args.next().unwrap_or_else(|| "preview.ppm".to_owned());
    let scale = args
        .next()
        .and_then(|scale| scale.parse().ok())
        .unwrap_or(1.0);

    let layout = Layout::new(DEFAULT_JOYSTICK_SIDE, scale);
    let (width, height) = (layout.width() as usize, layout.height() as usize);
    let views = [
        (Mode::Normal, Deflection::NEUTRAL),
        (Mode::Joystick, Deflection::new(0.6, 0.7)),
    ];
    let images = views.map(|(mode, knob)| {
        let mut pixels = vec![0; width * height];
        paint(
            &mut Canvas {
                width,
                height,
                pixels: &mut pixels,
            },
            &layout,
            mode,
            knob,
        );
        pixels
    });

    let mut out = std::io::BufWriter::new(std::fs::File::create(path)?);
    write!(out, "P6 {} {} 255 ", width * 2, height)?;
    for y in 0..height {
        for image in &images {
            for x in 0..width {
                // A game-like background shows through the transparent parts.
                let background = if (x / 16 + y / 16) % 2 == 0 {
                    [70, 120, 70]
                } else {
                    [90, 140, 90]
                };
                let pixel = image[y * width + x];
                let alpha = pixel >> 24;
                let channel = |shift: u32, under: u32| {
                    (((pixel >> shift) & 0xFF) + under * (255 - alpha) / 255) as u8
                };
                out.write_all(&[
                    channel(16, background[0]),
                    channel(8, background[1]),
                    channel(0, background[2]),
                ])?;
            }
        }
    }
    Ok(())
}
