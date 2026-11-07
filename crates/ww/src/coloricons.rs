//! Icons in colour. egui draws a glyph in one colour, so the icons of the command bar and of the
//! tabs are drawn by DirectWrite in the colours of their font (Segoe UI Emoji), once a size, and
//! shown as pictures. Without Windows, or when the drawing fails, an icon stays a glyph.

use std::collections::HashMap;

use eframe::egui::{self, Color32, TextureHandle};

/// An icon, its height in pixels and the colour of its glyphs that have none of their own.
type Key = (String, u32, [u8; 4]);

/// The pictures made, and the icons that could not be drawn (`None`), so each is tried once.
#[derive(Clone, Default)]
struct Cache(HashMap<Key, Option<TextureHandle>>);

fn cache_id() -> egui::Id {
    egui::Id::new("ww-color-icons")
}

/// The picture of an icon `px` pixels high and wide; the glyphs without colours of their own
/// are drawn in `ink`. `None` when this system cannot draw it.
pub fn texture(ctx: &egui::Context, icon: &str, px: u32, ink: Color32) -> Option<TextureHandle> {
    let key = (icon.to_owned(), px, ink.to_array());
    let known = ctx.data_mut(|d| {
        d.get_temp_mut_or_default::<Cache>(cache_id())
            .0
            .get(&key)
            .cloned()
    });
    if let Some(known) = known {
        return known;
    }
    let made = render(icon, px, ink).map(|image| {
        ctx.load_texture(
            format!("ww-icon-{icon}-{px}"),
            image,
            egui::TextureOptions::LINEAR,
        )
    });
    ctx.data_mut(|d| {
        d.get_temp_mut_or_default::<Cache>(cache_id())
            .0
            .insert(key, made.clone());
    });
    made
}

/// Draws the icon with Direct2D and DirectWrite into a bitmap of WIC, with the colours of the
/// font: premultiplied BGRA turned into the RGBA of egui.
#[cfg(windows)]
pub fn render(icon: &str, px: u32, ink: Color32) -> Option<egui::ColorImage> {
    use windows::Win32::Graphics::Direct2D::Common::{
        D2D_RECT_F, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_PIXEL_FORMAT,
    };
    use windows::Win32::Graphics::Direct2D::{
        D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT, D2D1_FACTORY_TYPE_SINGLE_THREADED,
        D2D1_FEATURE_LEVEL_DEFAULT, D2D1_RENDER_TARGET_PROPERTIES, D2D1_RENDER_TARGET_TYPE_DEFAULT,
        D2D1_RENDER_TARGET_USAGE_NONE, D2D1CreateFactory, ID2D1Factory,
    };
    use windows::Win32::Graphics::DirectWrite::{
        DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
        DWRITE_FONT_WEIGHT_NORMAL, DWRITE_MEASURING_MODE_NATURAL,
        DWRITE_PARAGRAPH_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_CENTER, DWriteCreateFactory,
        IDWriteFactory,
    };
    use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
    use windows::Win32::Graphics::Imaging::{
        CLSID_WICImagingFactory, GUID_WICPixelFormat32bppPBGRA, IWICImagingFactory,
        WICBitmapCacheOnLoad,
    };
    use windows::Win32::System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
    };
    use windows::core::w;

    if icon.is_empty() || !(4..=512).contains(&px) {
        return None;
    }
    // SAFETY: the calls follow the documented use of COM, WIC, Direct2D and DirectWrite; every
    // object lives in this function, and the buffer of the copy has the size the bitmap needs.
    unsafe {
        // The thread may have COM already (the window does): the result does not matter.
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let wic: IWICImagingFactory =
            CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER).ok()?;
        let bitmap = wic
            .CreateBitmap(px, px, &GUID_WICPixelFormat32bppPBGRA, WICBitmapCacheOnLoad)
            .ok()?;
        let d2d: ID2D1Factory = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None).ok()?;
        let props = D2D1_RENDER_TARGET_PROPERTIES {
            r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            dpiX: 96.0,
            dpiY: 96.0,
            usage: D2D1_RENDER_TARGET_USAGE_NONE,
            minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
        };
        let target = d2d.CreateWicBitmapRenderTarget(&bitmap, &props).ok()?;
        let dwrite: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED).ok()?;
        let format = dwrite
            .CreateTextFormat(
                w!("Segoe UI Emoji"),
                None,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                px as f32 * 0.78,
                w!("en-us"),
            )
            .ok()?;
        format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER).ok()?;
        format
            .SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)
            .ok()?;
        let [r, g, b, a] = ink.to_srgba_unmultiplied();
        let color = D2D1_COLOR_F {
            r: f32::from(r) / 255.0,
            g: f32::from(g) / 255.0,
            b: f32::from(b) / 255.0,
            a: f32::from(a) / 255.0,
        };
        let brush = target.CreateSolidColorBrush(&color, None).ok()?;
        let text: Vec<u16> = icon.encode_utf16().collect();
        let side = px as f32;
        let rect = D2D_RECT_F {
            left: -side,
            top: 0.0,
            right: 2.0 * side,
            bottom: side,
        };
        target.BeginDraw();
        target.Clear(Some(&D2D1_COLOR_F::default()));
        target.DrawText(
            &text,
            &format,
            &rect,
            &brush,
            D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT,
            DWRITE_MEASURING_MODE_NATURAL,
        );
        target.EndDraw(None, None).ok()?;
        let mut pixels = vec![0u8; (px * px * 4) as usize];
        bitmap
            .CopyPixels(std::ptr::null(), px * 4, &mut pixels)
            .ok()?;
        for c in pixels.as_chunks_mut::<4>().0 {
            c.swap(0, 2);
        }
        if pixels.as_chunks::<4>().0.iter().all(|c| c[3] == 0) {
            return None;
        }
        Some(egui::ColorImage::from_rgba_premultiplied(
            [px as usize, px as usize],
            &pixels,
        ))
    }
}

/// Without Windows the icons stay glyphs.
#[cfg(not(windows))]
pub fn render(_icon: &str, _px: u32, _ink: Color32) -> Option<egui::ColorImage> {
    None
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn an_emoji_is_drawn_in_its_colours() {
        let image = render(crate::icons::SAVE, 32, Color32::BLACK).expect("the icon is drawn");
        assert_eq!(image.size, [32, 32]);
        let coloured = image
            .pixels
            .iter()
            .filter(|c| c.a() > 0)
            .filter(|c| c.r().abs_diff(c.g()) > 20 || c.g().abs_diff(c.b()) > 20)
            .count();
        assert!(coloured > 20, "{coloured} pixels in colour");
    }

    #[test]
    fn a_symbol_without_colours_takes_the_ink() {
        let image = render(crate::icons::UP, 32, Color32::from_rgb(200, 0, 0));
        if let Some(image) = image {
            assert!(
                image.pixels.iter().any(|c| c.a() > 0 && c.r() > c.g()),
                "the glyph is drawn in the ink"
            );
        }
    }
}
