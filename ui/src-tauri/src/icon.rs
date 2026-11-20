//! An exe's own icon as a PNG data URL, for the app cards.

use base64::Engine;
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDIBits,
};
use windows::Win32::UI::Shell::SHDefExtractIconW;
use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, HICON, ICONINFO};
use windows_core::HSTRING;

const SIZE: u32 = 64;

pub fn png_data_url(exe: &str) -> Option<String> {
    let rgba = unsafe {
        let mut icon = HICON::default();
        // niconsize: the low word is the large icon size; Windows scales if the exe lacks it
        SHDefExtractIconW(&HSTRING::from(exe), 0, 0, Some(&mut icon), None, SIZE).ok().ok()?;
        if icon.is_invalid() {
            return None;
        }
        let rgba = icon_rgba(icon);
        let _ = DestroyIcon(icon);
        rgba?
    };
    let mut png_bytes = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut png_bytes, SIZE, SIZE);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header().ok()?.write_image_data(&rgba).ok()?;
    }
    Some(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png_bytes)))
}

unsafe fn icon_rgba(icon: HICON) -> Option<Vec<u8>> {
    unsafe {
        let mut info = ICONINFO::default();
        GetIconInfo(icon, &mut info).ok()?;
        let dc = CreateCompatibleDC(None);
        let mut bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: SIZE as i32,
                biHeight: -(SIZE as i32), // top-down rows
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut buf = vec![0u8; (SIZE * SIZE * 4) as usize];
        let lines = GetDIBits(dc, info.hbmColor, 0, SIZE, Some(buf.as_mut_ptr().cast()), &mut bmi, DIB_RGB_COLORS);
        let _ = DeleteDC(dc);
        let _ = DeleteObject(info.hbmColor.into());
        let _ = DeleteObject(info.hbmMask.into());
        if lines == 0 {
            return None;
        }
        // BGRA -> RGBA; very old icons carry no alpha at all, so make those opaque
        let has_alpha = buf.chunks_exact(4).any(|p| p[3] != 0);
        for p in buf.chunks_exact_mut(4) {
            p.swap(0, 2);
            if !has_alpha {
                p[3] = 255;
            }
        }
        Some(buf)
    }
}
