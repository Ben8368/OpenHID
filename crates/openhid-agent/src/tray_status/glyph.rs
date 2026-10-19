//! Native battery glyphs, independent of GPUI and hardware access.
//!
//! Matches the desktop's horizontal rounded battery, terminal, fill, and status
//! marks. Supersampling keeps the same silhouette legible at small tray sizes.

use openhid_core::battery::{current_charge_reading, glyph_fill, is_charging};
use openhid_core::device::{BatteryInfo, BatteryStatus};

/// A straight-alpha PNG: Win32 copies it into an owned HICON, while AppKit uses
/// the same silhouette as a template image that follows the menu-bar theme.
pub(crate) fn png(battery: &BatteryInfo, light: bool, side: u16) -> Option<Vec<u8>> {
    if !(16..=256).contains(&side) {
        return None;
    }
    let pixels = raster(battery, light, side);
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, u32::from(side), u32::from(side));
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().ok()?;
        writer.write_image_data(&pixels).ok()?;
        writer.finish().ok()?;
    }
    Some(bytes)
}

fn raster(battery: &BatteryInfo, light: bool, side: u16) -> Vec<u8> {
    let fill = glyph_fill(current_charge_reading(battery));
    let scale = f64::from(side - 2) / 27.;
    let top = (f64::from(side) - 14. * scale) / 2.;
    let stroke = 1. / scale;
    let inset = (1.6 / scale).min(3.);
    let color = if light { 0 } else { 255 };
    let mut pixels = Vec::with_capacity(usize::from(side).pow(2) * 4);
    for y in 0..side {
        for x in 0..side {
            let mut coverage = 0_u16;
            for sy in 0..4_u16 {
                for sx in 0..4_u16 {
                    let gx = (f64::from(x) + (f64::from(sx) + 0.5) / 4. - 1.) / scale;
                    let gy = (f64::from(y) + (f64::from(sy) + 0.5) / 4. - top) / scale;
                    let outline = rounded(gx, gy, [0., 1., 25., 12.], 3.)
                        && !rounded(
                            gx,
                            gy,
                            [stroke, 1. + stroke, 25. - 2. * stroke, 12. - 2. * stroke],
                            (3. - stroke).max(0.),
                        );
                    let terminal = rounded(gx, gy, [24.5, 4.72, 2., 4.56], 1.);
                    let filled = fill.is_some_and(|value| {
                        value > 0
                            && rounded(
                                gx,
                                gy,
                                [
                                    inset,
                                    1. + inset,
                                    ((25. - 2. * inset) * f64::from(value.min(100)) / 100.)
                                        .max(stroke),
                                    12. - 2. * inset,
                                ],
                                0.5,
                            )
                    });
                    let mark = if is_charging(battery) {
                        bolt(gx, gy)
                    } else if battery.status == BatteryStatus::Error {
                        rounded(gx, gy, [11.75, 3.5, 1.5, 4.5], 0.4)
                            || rounded(gx, gy, [11.75, 9., 1.5, 1.5], 0.4)
                    } else {
                        fill.is_none() && rounded(gx, gy, [10., 6.4, 5., 1.2], 0.4)
                    };
                    let ink = if mark {
                        fill.is_none_or(|value| value < 55)
                    } else {
                        outline || terminal || filled
                    };
                    coverage += u16::from(ink);
                }
            }
            let alpha = u8::try_from(coverage * 255 / 16).unwrap_or(255);
            pixels.extend_from_slice(&[color, color, color, alpha]);
        }
    }
    pixels
}

fn rounded(x: f64, y: f64, rect: [f64; 4], radius: f64) -> bool {
    let [left, top, width, height] = rect;
    if x < left || x > left + width || y < top || y > top + height {
        return false;
    }
    let radius = radius.min(width / 2.).min(height / 2.);
    let dx = x - x.clamp(left + radius, left + width - radius);
    let dy = y - y.clamp(top + radius, top + height - radius);
    dx * dx + dy * dy <= radius * radius
}

fn bolt(x: f64, y: f64) -> bool {
    // The desktop glyph's six-point lightning mark, centered in the body.
    let points = [
        (13.1, 2.44),
        (10.04, 7.55),
        (12.3, 7.55),
        (11.9, 11.56),
        (14.96, 6.45),
        (12.7, 6.45),
    ];
    let mut inside = false;
    let mut previous = points[points.len() - 1];
    for current in points {
        if (current.1 > y) != (previous.1 > y)
            && x < (previous.0 - current.0) * (y - current.1) / (previous.1 - current.1) + current.0
        {
            inside = !inside;
        }
        previous = current;
    }
    inside
}

#[cfg(test)]
mod tests {
    use super::*;
    use openhid_core::device::BatteryLevel;

    fn battery(percentage: Option<u8>, status: BatteryStatus) -> BatteryInfo {
        BatteryInfo {
            freshness: openhid_core::device::BatteryFreshness::Current,
            percentage,
            level: BatteryLevel::Unknown,
            status,
        }
    }

    #[test]
    fn charge_fill_grows_and_status_marks_are_distinct_at_every_scale() {
        for side in [16, 20, 24, 32, 36, 48, 64] {
            let mut previous_ink = 0;
            for value in [0, 10, 25, 50, 75, 100] {
                let pixels = raster(
                    &battery(Some(value), BatteryStatus::Discharging),
                    true,
                    side,
                );
                let ink: u64 = pixels
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|p| u64::from(p[3]))
                    .sum();
                assert!(ink > previous_ink, "fill {value} at {side}px");
                previous_ink = ink;
            }
            let empty = raster(&battery(Some(0), BatteryStatus::Discharging), true, side);
            let unknown = raster(&battery(None, BatteryStatus::Unknown), true, side);
            let charging = raster(&battery(Some(0), BatteryStatus::Charging), true, side);
            let error = raster(&battery(None, BatteryStatus::Error), true, side);
            assert_ne!(empty, unknown);
            assert_ne!(unknown, charging);
            assert_ne!(charging, error);
            assert_ne!(unknown, error);
        }
    }

    #[test]
    fn png_round_trip_preserves_theme_alpha_and_bounds() {
        for light in [false, true] {
            let reading = battery(Some(85), BatteryStatus::Discharging);
            let encoded = png(&reading, light, 36).unwrap();
            let mut decoder = png::Decoder::new(std::io::Cursor::new(encoded));
            decoder.set_transformations(png::Transformations::IDENTITY);
            let mut reader = decoder.read_info().unwrap();
            assert_eq!((reader.info().width, reader.info().height), (36, 36));
            let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
            reader.next_frame(&mut pixels).unwrap();
            assert_eq!(pixels, raster(&reading, light, 36));
            assert!(
                pixels
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .any(|p| p[3] > 0 && p[3] < 255)
            );
        }
        assert!(png(&battery(None, BatteryStatus::Unknown), true, 0).is_none());
        assert!(png(&battery(None, BatteryStatus::Unknown), true, 257).is_none());
    }
}
