//! Draws the window into a pixel buffer: every pixel works out how far it is
//! from each shape, which gives smooth edges without a graphics library.

use super::layout::{Button, Layout};
use super::model::Mode;
use crate::stick::Deflection;

/// Pixels as `0xAARRGGBB`, color already multiplied by alpha, top row first.
pub struct Canvas<'a> {
    pub width: usize,
    pub height: usize,
    pub pixels: &'a mut [u32],
}

type Rgb = [f32; 3];

const BAR: Rgb = [0.11, 0.11, 0.13];
const BAR_ALPHA: f32 = 0.94;
/// How much of the title bar remains visible in joystick mode.
const BAR_FADE: f32 = 0.35;
const ICON: Rgb = [0.93, 0.93, 0.95];
const ICON_ALPHA: f32 = 0.92;
/// Buttons that do nothing yet are drawn faint.
const INERT_ICON_ALPHA: f32 = 0.35;

const RING: Rgb = [0.26, 0.26, 0.29];
const RING_ACTIVE: Rgb = [0.16, 0.55, 1.0];
const GROOVE: Rgb = [0.04, 0.04, 0.05];
const WELL: Rgb = [0.085, 0.085, 0.1];
const KNOB_RIM: Rgb = [0.3, 0.3, 0.33];

/// Radii as fractions of the base radius.
const RING_INNER: f32 = 0.9;
const WELL_RADIUS: f32 = 0.86;
const KNOB_RADIUS: f32 = 0.4;
/// The knob's flat rim, as a fraction of the knob radius.
const KNOB_FACE: f32 = 0.86;

/// Draws every pixel from scratch. The reference for [`Painter`].
pub fn paint(canvas: &mut Canvas<'_>, layout: &Layout, mode: Mode, knob: Deflection) {
    let scene = Scene::new(layout, mode, knob);
    let bar_height = layout.title_bar_height() as usize;
    let rows = canvas
        .pixels
        .chunks_exact_mut(canvas.width.max(1))
        .take(canvas.height);
    for (y, row) in rows.enumerate() {
        for (x, pixel) in row.iter_mut().enumerate() {
            let at = (x as f32 + 0.5, y as f32 + 0.5);
            let color = if y < bar_height {
                scene.title_bar(at)
            } else {
                scene.joystick(at)
            };
            *pixel = color.pack();
        }
    }
}

/// Draws the same pixels as [`paint`], fast enough to follow the touchpad:
/// everything but the knob is drawn once per mode and kept, and each frame
/// only the square around the knob and its shadow is drawn again.
pub struct Painter {
    layout: Layout,
    background: Option<Background>,
}

impl Painter {
    pub fn new(layout: Layout) -> Self {
        Self {
            layout,
            background: None,
        }
    }

    pub fn paint(&mut self, canvas: &mut Canvas<'_>, mode: Mode, knob: Deflection) {
        let scene = Scene::new(&self.layout, mode, knob);
        let (width, height) = (canvas.width, canvas.height);
        let background = match self.background.take() {
            Some(kept) if kept.fits(mode, width, height) => kept,
            _ => Background::new(&scene, mode, width, height),
        };
        let shared = canvas.pixels.len().min(background.packed.len());
        canvas.pixels[..shared].copy_from_slice(&background.packed[..shared]);

        let (x_range, y_range) = scene.knob_square(width, height);
        for y in y_range {
            for x in x_range.clone() {
                let i = y * width + x;
                let Some(target) = canvas.pixels.get_mut(i) else {
                    continue;
                };
                let mut pixel = background.pixels[i];
                let at = (x as f32 + 0.5, y as f32 + 0.5);
                scene.knob_layers(&mut pixel, at, background.under[i]);
                *target = pixel.pack();
            }
        }
        self.background = Some(background);
    }
}

/// The window without the knob, for one mode and size.
struct Background {
    mode: Mode,
    width: usize,
    height: usize,
    pixels: Vec<Pixel>,
    under: Vec<Under>,
    packed: Vec<u32>,
}

impl Background {
    fn new(scene: &Scene, mode: Mode, width: usize, height: usize) -> Self {
        let bar_height = scene.bar_height as usize;
        let mut pixels = Vec::with_capacity(width * height);
        let mut under = Vec::with_capacity(width * height);
        for y in 0..height {
            for x in 0..width {
                let at = (x as f32 + 0.5, y as f32 + 0.5);
                let (pixel, below) = if y < bar_height {
                    (scene.title_bar(at), Under::default())
                } else {
                    scene.base(at)
                };
                pixels.push(pixel);
                under.push(below);
            }
        }
        let packed = pixels.iter().map(Pixel::pack).collect();
        Self {
            mode,
            width,
            height,
            pixels,
            under,
            packed,
        }
    }

    fn fits(&self, mode: Mode, width: usize, height: usize) -> bool {
        (self.mode, self.width, self.height) == (mode, width, height)
    }
}

/// What the knob's layers need to know about the base beneath a pixel.
#[derive(Debug, Clone, Copy, Default)]
struct Under {
    base: f32,
    in_groove: f32,
}

/// A pixel being built up, color multiplied by alpha.
#[derive(Debug, Clone, Copy, Default)]
struct Pixel {
    rgb: Rgb,
    alpha: f32,
}

impl Pixel {
    fn cover(&mut self, color: Rgb, alpha: f32) {
        let alpha = alpha.clamp(0.0, 1.0);
        for (mine, theirs) in self.rgb.iter_mut().zip(color) {
            *mine = theirs.clamp(0.0, 1.0) * alpha + *mine * (1.0 - alpha);
        }
        self.alpha = alpha + self.alpha * (1.0 - alpha);
    }

    fn pack(&self) -> u32 {
        let byte = |value: f32| (value.clamp(0.0, 1.0) * 255.0).round() as u32;
        let alpha = byte(self.alpha);
        let [r, g, b] = self.rgb.map(|channel| byte(channel).min(alpha));
        alpha << 24 | r << 16 | g << 8 | b
    }
}

/// How much of a pixel a shape covers, from the pixel's distance to the
/// shape's edge (negative inside).
fn coverage(distance: f32) -> f32 {
    (0.5 - distance).clamp(0.0, 1.0)
}

fn mix(from: Rgb, to: Rgb, amount: f32) -> Rgb {
    std::array::from_fn(|i| from[i] + (to[i] - from[i]) * amount)
}

fn shade(color: Rgb, factor: f32) -> Rgb {
    color.map(|channel| channel * factor)
}

struct Scene {
    scale: f32,
    width: f32,
    bar_height: f32,
    bar_fade: f32,
    buttons: [(Button, f32, f32); 4],
    button_size: f32,
    center: (f32, f32),
    radius: f32,
    ring: Rgb,
    knob: (f32, f32),
}

impl Scene {
    fn new(layout: &Layout, mode: Mode, knob: Deflection) -> Self {
        let center = layout.joystick_center();
        let radius = layout.base_radius();
        let travel = (WELL_RADIUS - KNOB_RADIUS) * radius;
        let buttons = Button::ALL.map(|button| {
            let rect = layout.button_rect(button);
            (
                button,
                (rect.left + rect.right) as f32 / 2.0,
                (rect.top + rect.bottom) as f32 / 2.0,
            )
        });
        let close = layout.button_rect(Button::Close);
        Self {
            scale: layout.scale(),
            width: layout.width() as f32,
            bar_height: layout.title_bar_height() as f32,
            bar_fade: if mode == Mode::Joystick {
                BAR_FADE
            } else {
                1.0
            },
            buttons,
            button_size: (close.right - close.left) as f32,
            center,
            radius,
            ring: if mode == Mode::Joystick {
                RING_ACTIVE
            } else {
                RING
            },
            // Up on the stick is up on the screen, where y gets smaller.
            knob: (center.0 + knob.x() * travel, center.1 - knob.y() * travel),
        }
    }

    fn title_bar(&self, (x, y): (f32, f32)) -> Pixel {
        let mut pixel = Pixel::default();
        let corner = 8.0 * self.scale;
        let half = (self.width / 2.0, self.bar_height / 2.0);
        let bar = coverage(rounded_box(x - half.0, y - half.1, half.0, half.1, corner));
        pixel.cover(BAR, BAR_ALPHA * bar * self.bar_fade);

        let unit = self.button_size / 32.0;
        for &(button, center_x, center_y) in &self.buttons {
            if (x - center_x).abs() > self.button_size / 2.0 {
                continue;
            }
            let icon = icon_distance(button, (x - center_x) / unit, (y - center_y) / unit);
            let alpha = match button {
                Button::Minimize | Button::Close => ICON_ALPHA,
                Button::Opacity | Button::Settings => INERT_ICON_ALPHA,
            };
            pixel.cover(ICON, alpha * coverage(icon * unit) * bar * self.bar_fade);
        }
        pixel
    }

    fn joystick(&self, at: (f32, f32)) -> Pixel {
        let (mut pixel, under) = self.base(at);
        self.knob_layers(&mut pixel, at, under);
        pixel
    }

    /// The pixels the knob and its shadow can reach, clipped to the canvas.
    fn knob_square(
        &self,
        width: usize,
        height: usize,
    ) -> (std::ops::Range<usize>, std::ops::Range<usize>) {
        // The shadow reaches 1.3 knob radii from a point 0.12 radii below
        // the knob; one more pixel covers the edge pixels.
        let reach = 1.42 * KNOB_RADIUS * self.radius + 1.0;
        let span = |center: f32, low: usize, high: usize| {
            let from = ((center - reach).floor().max(0.0) as usize).max(low);
            let to = ((center + reach).ceil().max(0.0) as usize).min(high);
            from..to.max(from)
        };
        (
            span(self.knob.0, 0, width),
            span(self.knob.1, self.bar_height as usize, height),
        )
    }

    /// The joystick without the knob: well, groove and ring.
    fn base(&self, (x, y): (f32, f32)) -> (Pixel, Under) {
        let mut pixel = Pixel::default();
        let (dx, dy) = (x - self.center.0, y - self.center.1);
        let distance = dx.hypot(dy);
        let base = coverage(distance - self.radius);
        if base == 0.0 {
            return (pixel, Under::default());
        }

        // Base, from the center outwards: well, groove, ring. Light comes
        // from the top, so the ring is brighter where it faces up.
        let well = shade(WELL, 1.0 - 0.35 * (distance / self.radius).powi(2));
        let lit_ring = shade(self.ring, 1.0 - 0.25 * dy / distance.max(1.0));
        let in_groove = 1.0 - coverage(distance - WELL_RADIUS * self.radius);
        let in_ring = 1.0 - coverage(distance - RING_INNER * self.radius);
        pixel.cover(mix(mix(well, GROOVE, in_groove), lit_ring, in_ring), base);
        (pixel, Under { base, in_groove })
    }

    /// The knob and the shadow it casts, over the base.
    fn knob_layers(&self, pixel: &mut Pixel, (x, y): (f32, f32), under: Under) {
        let Under { base, in_groove } = under;
        if base == 0.0 {
            return;
        }
        let knob_radius = KNOB_RADIUS * self.radius;
        let (kx, ky) = (x - self.knob.0, y - self.knob.1);
        let from_knob = kx.hypot(ky);

        // Soft shadow the knob casts on the well, a little below it.
        let shadow_reach = 0.3 * knob_radius;
        let from_shadow = kx.hypot(ky - 0.12 * knob_radius) - knob_radius;
        let shadow = (1.0 - from_shadow / shadow_reach).clamp(0.0, 1.0);
        pixel.cover([0.0; 3], 0.55 * shadow * shadow * (1.0 - in_groove) * base);

        // Knob: a raised rim around a concave, finely ridged face. Being
        // concave, the face is brighter towards its lower edge.
        let along = from_knob / knob_radius;
        let ridges = 0.012 * (from_knob / self.scale * 1.6).sin();
        let face_light = (0.15 + 0.1 * along * along + ridges) * (1.0 + 0.18 * ky / knob_radius);
        let face = [face_light, face_light, face_light * 1.07];
        let rim = shade(KNOB_RIM, 1.0 - 0.35 * ky / from_knob.max(1.0));
        let on_rim = 1.0 - coverage(from_knob - KNOB_FACE * knob_radius);
        pixel.cover(
            mix(face, rim, on_rim),
            coverage(from_knob - knob_radius) * base,
        );
    }
}

/// Distance to a box with rounded corners, centered on the origin.
fn rounded_box(x: f32, y: f32, half_width: f32, half_height: f32, corner: f32) -> f32 {
    let qx = x.abs() - (half_width - corner);
    let qy = y.abs() - (half_height - corner);
    qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - corner
}

/// Distance to a line segment with round ends.
fn stroke((x, y): (f32, f32), from: (f32, f32), to: (f32, f32), half_width: f32) -> f32 {
    let (px, py) = (x - from.0, y - from.1);
    let (sx, sy) = (to.0 - from.0, to.1 - from.1);
    let along = ((px * sx + py * sy) / (sx * sx + sy * sy)).clamp(0.0, 1.0);
    (px - sx * along).hypot(py - sy * along) - half_width
}

/// Distance to a button's icon, in a 32-unit square centered on the origin.
fn icon_distance(button: Button, x: f32, y: f32) -> f32 {
    let at = (x, y);
    match button {
        Button::Close => stroke(at, (-4.5, -4.5), (4.5, 4.5), 0.75).min(stroke(
            at,
            (-4.5, 4.5),
            (4.5, -4.5),
            0.75,
        )),
        Button::Minimize => stroke(at, (-5.0, 0.0), (5.0, 0.0), 0.75),
        // A circle, half filled.
        Button::Opacity => {
            let from_edge = x.hypot(y) - 5.5;
            (from_edge.abs() - 0.75).min(from_edge.max(-x))
        }
        // Two sliders.
        Button::Settings => stroke(at, (-5.5, -3.0), (5.5, -3.0), 0.6)
            .min(stroke(at, (-5.5, 3.0), (5.5, 3.0), 0.6))
            .min((x + 2.0).hypot(y + 3.0) - 2.2)
            .min((x - 2.0).hypot(y - 3.0) - 2.2),
    }
}
