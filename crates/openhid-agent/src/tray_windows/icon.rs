//! Owned Windows icon resources and DPI-sized battery glyphs.

use crate::tray_status::{IconContent, glyph};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateIconFromResourceEx, DestroyIcon, HICON, LR_DEFAULTCOLOR,
};

/// Exactly one ownership token per non-shared HICON.
pub(super) struct OwnedIcon(HICON);

impl OwnedIcon {
    pub(super) const fn handle(&self) -> HICON {
        self.0
    }
}

impl Drop for OwnedIcon {
    fn drop(&mut self) {
        // SAFETY: constructed only by CreateIcon* returning a non-shared icon.
        unsafe { DestroyIcon(self.0) };
    }
}

/// Keep brand assets at source resolution; render battery glyphs at the requested size.
/// Each returned icon owns its handle.
pub(super) fn create(content: &IconContent, light: bool, size: i32) -> Option<OwnedIcon> {
    let battery_png;
    let png: &[u8] = match content {
        IconContent::Brand if light => include_bytes!("../../assets/tray-icon@2x.png"),
        IconContent::Brand => include_bytes!("../../assets/tray-icon-white@2x.png"),
        IconContent::Battery(battery) => {
            battery_png = glyph::png(battery, light, u16::try_from(size).ok()?)?;
            &battery_png
        }
    };
    // SAFETY: PNG bytes live throughout this copying call. Win32 owns the
    // decoded icon; OwnedIcon releases it exactly once after shell replacement.
    let handle = unsafe {
        CreateIconFromResourceEx(
            png.as_ptr(),
            u32::try_from(png.len()).ok()?,
            1,
            0x0003_0000,
            // Preserve the PNG's pixels for Explorer's final scaling. Reducing
            // the 36px brand to a DPI-virtualized 16px here loses detail before
            // a high-DPI taskbar enlarges it. Battery PNGs already have `size`.
            0,
            0,
            LR_DEFAULTCOLOR,
        )
    };
    (!handle.is_null()).then_some(OwnedIcon(handle))
}

#[cfg(test)]
mod tests {
    use super::*;
    use openhid_core::device::{BatteryInfo, BatteryLevel, BatteryStatus};
    use windows_sys::Win32::Graphics::Gdi::{
        BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC,
        DeleteObject, GetDIBits, GetObjectW, HGDIOBJ,
    };

    struct GdiObject(HGDIOBJ);
    impl Drop for GdiObject {
        fn drop(&mut self) {
            // SAFETY: GetIconInfo returned these owned, unselected bitmaps.
            unsafe { DeleteObject(self.0) };
        }
    }
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetIconInfo, ICONINFO};

    const TASKBAR_SIZES: [i32; 7] = [16, 20, 24, 27, 32, 48, 64];

    fn pixels(icon: &OwnedIcon) -> (i32, Vec<u8>) {
        // SAFETY: GetIconInfo creates two owned bitmap copies, both released
        // after the unselected color bitmap has been copied into a sized buffer.
        unsafe {
            let mut info: ICONINFO = std::mem::zeroed();
            assert_ne!(GetIconInfo(icon.handle(), &raw mut info), 0);
            let _mask = GdiObject(info.hbmMask);
            let color = GdiObject(info.hbmColor);
            let mut bitmap: BITMAP = std::mem::zeroed();
            assert_ne!(
                GetObjectW(
                    color.0,
                    i32::try_from(std::mem::size_of::<BITMAP>()).unwrap(),
                    (&raw mut bitmap).cast(),
                ),
                0
            );
            assert_eq!(bitmap.bmWidth, bitmap.bmHeight);
            let size = bitmap.bmWidth;
            let dc = CreateCompatibleDC(std::ptr::null_mut());
            assert!(!dc.is_null());
            let mut format: BITMAPINFO = std::mem::zeroed();
            format.bmiHeader = BITMAPINFOHEADER {
                biSize: u32::try_from(std::mem::size_of::<BITMAPINFOHEADER>()).unwrap(),
                biWidth: size,
                biHeight: -size,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                ..std::mem::zeroed()
            };
            let mut pixels = vec![0; usize::try_from(size * size * 4).unwrap()];
            let rows = GetDIBits(
                dc,
                color.0,
                0,
                size.cast_unsigned(),
                pixels.as_mut_ptr().cast(),
                &raw mut format,
                DIB_RGB_COLORS,
            );
            DeleteDC(dc);
            assert_eq!(rows, size);
            (size, pixels)
        }
    }

    #[test]
    fn brand_icons_preserve_source_pixels_at_every_taskbar_scale() {
        for (light, png) in [
            (
                true,
                include_bytes!("../../assets/tray-icon@2x.png").as_slice(),
            ),
            (
                false,
                include_bytes!("../../assets/tray-icon-white@2x.png").as_slice(),
            ),
        ] {
            let mut reader = png::Decoder::new(std::io::Cursor::new(png))
                .read_info()
                .unwrap();
            let mut source = vec![0; reader.output_buffer_size().unwrap()];
            let frame = reader.next_frame(&mut source).unwrap();
            assert_eq!(frame.color_type, png::ColorType::Rgba);
            assert_eq!(frame.width, frame.height);
            source.truncate(frame.buffer_size());
            for pixel in source.as_chunks_mut::<4>().0 {
                pixel.swap(0, 2); // RGBA source to native BGRA.
            }
            for size in TASKBAR_SIZES {
                let icon = create(&IconContent::Brand, light, size).expect("native brand icon");
                let (side, image) = pixels(&icon);
                assert_eq!(
                    side.cast_unsigned(),
                    frame.width,
                    "brand source was resampled before reaching the taskbar at {size}px"
                );
                assert_eq!(image, source, "brand source pixels changed at {size}px");
            }
        }
    }

    #[test]
    fn native_tray_icons_render_at_supported_scales_and_themes() {
        let sizes = TASKBAR_SIZES;
        let readings = [
            (Some(0), BatteryStatus::Discharging),
            (Some(20), BatteryStatus::Discharging),
            (Some(50), BatteryStatus::Discharging),
            (Some(85), BatteryStatus::Discharging),
            (Some(100), BatteryStatus::Discharging),
            (Some(20), BatteryStatus::Charging),
            (None, BatteryStatus::Unknown),
            (None, BatteryStatus::Charging),
            (None, BatteryStatus::Error),
        ];
        let width = (readings.len() + 1) * 80;
        let height = sizes.len() * 2 * 80;
        let mut preview = vec![0_u8; width * height * 3];
        for (size_index, size) in sizes.into_iter().enumerate() {
            for (theme_index, light) in [false, true].into_iter().enumerate() {
                for (index, content) in readings
                    .iter()
                    .map(|&(percentage, status)| {
                        IconContent::Battery(BatteryInfo {
                            freshness: openhid_core::device::BatteryFreshness::Current,
                            percentage,
                            status,
                            level: BatteryLevel::Unknown,
                        })
                    })
                    .chain([IconContent::Brand])
                    .enumerate()
                {
                    let icon = create(&content, light, size).expect("native icon");
                    let (native_size, image) = pixels(&icon);
                    if matches!(content, IconContent::Battery(_)) {
                        assert_eq!(native_size, size);
                    }
                    let side = usize::try_from(native_size).unwrap();
                    let occupied: Vec<_> = image
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .enumerate()
                        .filter(|(_, pixel)| pixel[3] > 0)
                        .map(|(i, _)| (i % side, i / side))
                        .collect();
                    assert!(!occupied.is_empty(), "empty {content:?} at {size}px");
                    if matches!(content, IconContent::Battery(_)) {
                        assert!(
                            occupied.iter().all(|&(x, _)| x > 0 && x < side - 1),
                            "clipped {content:?} at {size}px"
                        );
                    }
                    let bg = if light { 245_u16 } else { 30_u16 };
                    for y in 0..80 {
                        for x in 0..80 {
                            let at = ((size_index * 2 + theme_index) * 80 + y) * width * 3
                                + (index * 80 + x) * 3;
                            let offset = (80 - side) / 2;
                            let rgb = if x >= offset
                                && x < offset + side
                                && y >= offset
                                && y < offset + side
                            {
                                let source = ((y - offset) * side + x - offset) * 4;
                                let alpha = u16::from(image[source + 3]);
                                u8::try_from(
                                    (u16::from(image[source]) * alpha / 255)
                                        + bg * (255 - alpha) / 255,
                                )
                                .unwrap()
                            } else {
                                u8::try_from(bg).unwrap()
                            };
                            preview[at..at + 3].fill(rgb);
                        }
                    }
                }
            }
        }
        let path = std::env::temp_dir().join("openhid-tray-icons.ppm");
        let mut file = format!("P6\n{width} {height}\n255\n").into_bytes();
        file.extend_from_slice(&preview);
        std::fs::write(&path, file).unwrap();
        println!("Native tray preview: {}", path.display());
    }
}
