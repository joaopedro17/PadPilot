//! Writes the program icon, drawn by the joystick's own painter:
//! `cargo run -p padpilot-core --example icon` makes `assets/padpilot.ico`
//! (every size Windows asks for) and `assets/padpilot.png` (for the README).

use padpilot_core::overlay::paint::{Canvas, paint_icon};

const ICON_SIZES: [u32; 8] = [16, 20, 24, 32, 40, 48, 64, 256];
const PNG_SIZE: u32 = 128;

fn main() -> std::io::Result<()> {
    std::fs::create_dir_all("assets")?;
    let images: Vec<(u32, Vec<u8>)> = ICON_SIZES
        .iter()
        .map(|&size| (size, bmp_entry(size, &straight_bgra(size))))
        .collect();
    std::fs::write("assets/padpilot.ico", ico(&images))?;
    let png_pixels = straight_bgra(PNG_SIZE);
    std::fs::write("assets/padpilot.png", png(PNG_SIZE, &png_pixels))?;
    println!("wrote assets/padpilot.ico and assets/padpilot.png");
    Ok(())
}

/// The icon at `size`, as rows of straight (not premultiplied) BGRA, top first.
fn straight_bgra(size: u32) -> Vec<[u8; 4]> {
    let side = size as usize;
    let mut pixels = vec![0u32; side * side];
    paint_icon(&mut Canvas {
        width: side,
        height: side,
        pixels: &mut pixels,
    });
    pixels
        .iter()
        .map(|&p| {
            let alpha = (p >> 24) as u8;
            let channel = |shift: u32| {
                let premultiplied = (p >> shift) & 0xFF;
                if alpha == 0 {
                    0
                } else {
                    ((premultiplied * 255 + u32::from(alpha) / 2) / u32::from(alpha)).min(255) as u8
                }
            };
            [channel(0), channel(8), channel(16), alpha]
        })
        .collect()
}

/// One icon image: a 32-bit bitmap, bottom row first, with an empty mask.
fn bmp_entry(size: u32, pixels: &[[u8; 4]]) -> Vec<u8> {
    let mut out = Vec::new();
    let mask_row = size.div_ceil(32) * 4;
    for value in [40u32, size, size * 2] {
        out.extend(value.to_le_bytes());
    }
    out.extend(1u16.to_le_bytes()); // planes
    out.extend(32u16.to_le_bytes()); // bits per pixel
    for value in [0u32, size * size * 4 + mask_row * size, 0, 0, 0, 0] {
        out.extend(value.to_le_bytes());
    }
    for row in pixels.chunks(size as usize).rev() {
        for pixel in row {
            out.extend(pixel);
        }
    }
    out.extend(std::iter::repeat_n(0u8, (mask_row * size) as usize));
    out
}

fn ico(images: &[(u32, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    for value in [0u16, 1, images.len() as u16] {
        out.extend(value.to_le_bytes());
    }
    let mut offset = 6 + 16 * images.len() as u32;
    for (size, data) in images {
        let side = if *size >= 256 { 0 } else { *size as u8 };
        out.extend([side, side, 0, 0]);
        out.extend(1u16.to_le_bytes());
        out.extend(32u16.to_le_bytes());
        out.extend((data.len() as u32).to_le_bytes());
        out.extend(offset.to_le_bytes());
        offset += data.len() as u32;
    }
    for (_, data) in images {
        out.extend(data);
    }
    out
}

/// A PNG with uncompressed (stored) deflate blocks: no compressor needed.
fn png(size: u32, pixels: &[[u8; 4]]) -> Vec<u8> {
    let mut raw = Vec::new();
    for row in pixels.chunks(size as usize) {
        raw.push(0); // no filter
        for [b, g, r, a] in row {
            raw.extend([*r, *g, *b, *a]);
        }
    }
    let mut zlib = vec![0x78, 0x01];
    let blocks: Vec<&[u8]> = raw.chunks(65535).collect();
    for (i, block) in blocks.iter().enumerate() {
        zlib.push(u8::from(i + 1 == blocks.len()));
        let length = block.len() as u16;
        zlib.extend(length.to_le_bytes());
        zlib.extend((!length).to_le_bytes());
        zlib.extend(*block);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &byte in &raw {
        a = (a + u32::from(byte)) % 65521;
        b = (b + a) % 65521;
    }
    zlib.extend(((b << 16) | a).to_be_bytes());

    let mut header = Vec::new();
    header.extend(size.to_be_bytes());
    header.extend(size.to_be_bytes());
    header.extend([8, 6, 0, 0, 0]); // 8-bit RGBA
    let mut out = vec![0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1A, b'\n'];
    for (kind, data) in [(b"IHDR", header), (b"IDAT", zlib), (b"IEND", Vec::new())] {
        out.extend((data.len() as u32).to_be_bytes());
        let mut chunk = kind.to_vec();
        chunk.extend(&data);
        out.extend(&chunk);
        out.extend(crc32(&chunk).to_be_bytes());
    }
    out
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}
