//! The pixels of the window: a bitmap the app draws into and Windows shows.

use std::mem::size_of;

use padpilot_core::overlay::paint::Canvas;
use windows::Win32::Foundation::{COLORREF, HWND, POINT, SIZE};
use windows::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION,
    CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, HBITMAP, HDC,
    HGDIOBJ, SelectObject,
};
use windows::Win32::UI::WindowsAndMessaging::{ULW_ALPHA, UpdateLayeredWindow};

use crate::error::Error;

pub struct Surface {
    dc: HDC,
    bitmap: HBITMAP,
    previous: HGDIOBJ,
    pixels: *mut u32,
    width: i32,
    height: i32,
}

impl Surface {
    pub fn new(width: i32, height: i32) -> Result<Self, Error> {
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                // Negative: the first row is the top one.
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut pixels = std::ptr::null_mut();
        // SAFETY: `info` describes a 32-bit bitmap; Windows allocates the
        // pixels and returns them in `pixels`.
        let bitmap = unsafe { CreateDIBSection(None, &info, DIB_RGB_COLORS, &mut pixels, None, 0) }
            .map_err(|err| Error::caused("cannot create the window bitmap", err))?;
        // SAFETY: selecting a bitmap this surface owns into a DC it owns.
        let (dc, previous) = unsafe {
            let dc = CreateCompatibleDC(None);
            (dc, SelectObject(dc, bitmap.into()))
        };
        Ok(Self {
            dc,
            bitmap,
            previous,
            pixels: pixels.cast(),
            width,
            height,
        })
    }

    pub fn canvas(&mut self) -> Canvas<'_> {
        let (width, height) = (self.width as usize, self.height as usize);
        // SAFETY: the bitmap holds width x height 32-bit pixels and lives as
        // long as `self`, which the returned canvas borrows.
        let pixels = unsafe { std::slice::from_raw_parts_mut(self.pixels, width * height) };
        Canvas {
            width,
            height,
            pixels,
        }
    }

    /// Shows the pixels in `window`, resizing it to fit and moving its
    /// top-left corner to `at` when given. `opacity` scales the whole window;
    /// fully transparent pixels let clicks through.
    pub fn present(&self, window: HWND, opacity: u8, at: Option<POINT>) -> Result<(), Error> {
        let size = SIZE {
            cx: self.width,
            cy: self.height,
        };
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: opacity,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        // SAFETY: every pointer refers to a local that outlives the call.
        unsafe {
            UpdateLayeredWindow(
                window,
                None,
                at.as_ref().map(std::ptr::from_ref),
                Some(&size),
                Some(self.dc),
                Some(&POINT::default()),
                COLORREF(0),
                Some(&blend),
                ULW_ALPHA,
            )
        }
        .map_err(|err| Error::caused("cannot draw the window", err))
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        // SAFETY: releases what `new` created, in reverse order.
        unsafe {
            SelectObject(self.dc, self.previous);
            let _ = DeleteObject(self.bitmap.into());
            let _ = DeleteDC(self.dc);
        }
    }
}
