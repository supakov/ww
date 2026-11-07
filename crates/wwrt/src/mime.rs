//! MIME types: the type of some bytes by their first ones (`sniff`), the spellings of one type
//! (`canonical`), and the types a textual one falls back to when no plugin names it
//! (`fallbacks`). A plugin names the types of its formats (`plugin::Format::mimes`); the host finds
//! the plugin of some bytes inside another file by them (`plugin::Host::open_inside`).

/// How many first bytes `sniff` looks at, at most.
pub const SNIFFED: usize = 512;

/// The type of a portable executable (an exe, a DLL).
pub const PE: &str = "application/vnd.microsoft.portable-executable";

/// The spellings of a type that differ, each with the one used here.
const ALIASES: [(&str, &str); 11] = [
    ("image/ico", "image/x-icon"),
    ("image/vnd.microsoft.icon", "image/x-icon"),
    ("image/jpg", "image/jpeg"),
    ("image/pjpeg", "image/jpeg"),
    ("image/tif", "image/tiff"),
    ("image/x-bmp", "image/bmp"),
    ("image/x-ms-bmp", "image/bmp"),
    ("image/vnd-ms.dds", "image/vnd.ms-dds"),
    ("text/xml", "application/xml"),
    ("application/x-msdownload", PE),
    ("application/x-dosexec", PE),
];

/// The type without its parameters, in lower case: `text/plain; charset=utf-16le` is
/// `text/plain`.
pub fn base(mime: &str) -> String {
    mime.split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase()
}

/// A parameter of a type, its name in any case and its value as it is written: `charset` of
/// `text/plain; charset=utf-16le` is `utf-16le`, `members` of `image/x-group-icon; members=ICON`
/// is `ICON`.
pub fn parameter(mime: &str, name: &str) -> Option<String> {
    mime.split(';').skip(1).find_map(|p| {
        let (key, value) = p.split_once('=')?;
        key.trim()
            .eq_ignore_ascii_case(name)
            .then(|| value.trim().trim_matches('"').to_owned())
    })
}

/// The base of a type with its spelling made the one used here: two spellings of one type give
/// one text.
pub fn canonical(mime: &str) -> String {
    let base = base(mime);
    ALIASES
        .iter()
        .find(|(alias, _)| *alias == base)
        .map_or(base, |(_, one)| (*one).to_owned())
}

/// Whether two types are one, whatever their spellings and parameters.
pub fn same(a: &str, b: &str) -> bool {
    canonical(a) == canonical(b)
}

/// The types a textual type falls back to, in turn, when no plugin names it: a type of XML (and
/// one that ends in `+xml`) to `application/xml`, then every text, JSON and script to
/// `text/plain`. Other types fall back to none.
pub fn fallbacks(mime: &str) -> Vec<&'static str> {
    let m = canonical(mime);
    let mut out = Vec::new();
    if m.ends_with("+xml") {
        out.push("application/xml");
    }
    let textual = m.starts_with("text/")
        || m.ends_with("+xml")
        || m.ends_with("+json")
        || matches!(
            m.as_str(),
            "application/xml" | "application/json" | "application/javascript"
        );
    if textual && m != "text/plain" {
        out.push("text/plain");
    }
    out
}

/// The type of common files by their first bytes (`SNIFFED` of them, or all a shorter part
/// has): PNG, JPEG, GIF, BMP, ICO, TIFF, WebP, QOI, farbfeld, Radiance HDR, Netpbm, a portable
/// executable, ZIP, PDF, and texts of
/// UTF-8 or UTF-16 (with or without a mark) as XML, HTML or plain text. A text of UTF-16 without
/// its mark says so in its `charset`.
pub fn sniff(bytes: &[u8]) -> Option<&'static str> {
    let bytes = &bytes[..bytes.len().min(SNIFFED)];
    let starts = |magic: &[u8]| bytes.starts_with(magic);
    let u16_at = |at: usize| {
        bytes
            .get(at..at + 2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
    };
    let u32_at = |at: usize| {
        bytes
            .get(at..at + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    if starts(b"\x89PNG\r\n\x1a\n") {
        return Some("image/png");
    }
    if starts(b"\xFF\xD8\xFF") {
        return Some("image/jpeg");
    }
    if starts(b"GIF87a") || starts(b"GIF89a") {
        return Some("image/gif");
    }
    if starts(b"BM") && u32_at(6) == Some(0) && u32_at(14).is_some_and(|s| s >= 12) {
        return Some("image/bmp");
    }
    if starts(&[0, 0, 1, 0]) && u16_at(4).is_some_and(|n| n > 0) && bytes.get(9) == Some(&0) {
        return Some("image/x-icon");
    }
    if starts(b"II*\0") || starts(b"MM\0*") {
        return Some("image/tiff");
    }
    if starts(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        return Some("image/webp");
    }
    if starts(b"MZ")
        && let Some(header) = u32_at(0x3C)
    {
        let header = header as usize;
        match bytes.get(header..header + 4) {
            Some(b"PE\0\0") => return Some(PE),
            // The header of PE beyond the bytes looked at: the stub of MS-DOS is enough.
            None if header >= bytes.len() && header < 1 << 20 => return Some(PE),
            _ => {}
        }
    }
    if starts(b"qoif") {
        return Some("image/qoi");
    }
    if starts(b"farbfeld") {
        return Some("image/x-farbfeld");
    }
    if starts(b"#?RADIANCE") || starts(b"#?RGBE") {
        return Some("image/vnd.radiance");
    }
    // Netpbm: P1 to P7 and white space.
    if let [b'P', kind @ b'1'..=b'7', next, ..] = bytes
        && next.is_ascii_whitespace()
    {
        let _ = kind;
        return Some("image/x-portable-anymap");
    }
    if starts(b"PK\x03\x04") || starts(b"PK\x05\x06") {
        return Some("application/zip");
    }
    if starts(b"%PDF-") {
        return Some("application/pdf");
    }
    text(bytes)
}

/// The type of a text: XML, HTML or plain, by its start.
fn text(bytes: &[u8]) -> Option<&'static str> {
    if let Some(rest) = bytes.strip_prefix(b"\xEF\xBB\xBF") {
        return Some(kind_of(&utf8(rest)?));
    }
    if let Some(rest) = bytes.strip_prefix(b"\xFF\xFE") {
        return Some(kind_of(&utf16(rest, false)?));
    }
    if let Some(rest) = bytes.strip_prefix(b"\xFE\xFF") {
        return Some(kind_of(&utf16(rest, true)?));
    }
    if let Some(text) = utf8(bytes) {
        return Some(kind_of(&text));
    }
    // UTF-16 without its mark: a zero byte in every pair, after the character in little endian.
    let (pairs, _) = bytes.as_chunks::<2>();
    if pairs.len() >= 2 {
        let little = pairs.iter().all(|[_, high]| *high == 0);
        let big = pairs.iter().all(|[high, _]| *high == 0);
        if little && let Some(text) = utf16(bytes, false) {
            return Some(match kind_of(&text) {
                "application/xml" => "application/xml; charset=utf-16le",
                "text/html" => "text/html; charset=utf-16le",
                _ => "text/plain; charset=utf-16le",
            });
        }
        if big && let Some(text) = utf16(bytes, true) {
            return Some(match kind_of(&text) {
                "application/xml" => "application/xml; charset=utf-16be",
                "text/html" => "text/html; charset=utf-16be",
                _ => "text/plain; charset=utf-16be",
            });
        }
    }
    None
}

/// Whether a character may be in a text: no control character but the white space ones.
fn readable(c: char) -> bool {
    !c.is_control() || matches!(c, '\t' | '\n' | '\r' | '\x0C' | '\x1B')
}

/// The bytes as UTF-8, a character cut at their end allowed, when every character is readable.
fn utf8(bytes: &[u8]) -> Option<String> {
    if bytes.is_empty() {
        return None;
    }
    let text = match std::str::from_utf8(bytes) {
        Ok(t) => t,
        Err(e) if e.error_len().is_none() && e.valid_up_to() > 0 => {
            std::str::from_utf8(&bytes[..e.valid_up_to()]).ok()?
        }
        Err(_) => return None,
    };
    text.chars().all(readable).then(|| text.to_owned())
}

/// The bytes as UTF-16, a unit cut at their end allowed, when every character is readable.
fn utf16(bytes: &[u8], big: bool) -> Option<String> {
    let (pairs, _) = bytes.as_chunks::<2>();
    if pairs.is_empty() {
        return None;
    }
    let units = pairs.iter().map(|p| {
        if big {
            u16::from_be_bytes(*p)
        } else {
            u16::from_le_bytes(*p)
        }
    });
    let mut text = String::new();
    for c in char::decode_utf16(units) {
        match c {
            Ok(c) if readable(c) => text.push(c),
            Ok(_) => return None,
            // A pair of surrogates cut at the end of the bytes looked at.
            Err(_) => break,
        }
    }
    (!text.is_empty()).then_some(text)
}

/// XML, HTML or plain text, by the start of a text.
fn kind_of(text: &str) -> &'static str {
    let start = text.trim_start().to_ascii_lowercase();
    if start.starts_with("<?xml") {
        "application/xml"
    } else if start.starts_with("<!doctype html") || start.starts_with("<html") {
        "text/html"
    } else {
        "text/plain"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_bytes_name_the_type() {
        assert_eq!(sniff(b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR"), Some("image/png"));
        assert_eq!(sniff(b"\xFF\xD8\xFF\xE0\0\x10JFIF"), Some("image/jpeg"));
        assert_eq!(sniff(b"GIF89a\x01\0\x01\0"), Some("image/gif"));
        let mut bmp = b"BM".to_vec();
        bmp.extend(70u32.to_le_bytes());
        bmp.extend([0; 4]);
        bmp.extend(54u32.to_le_bytes());
        bmp.extend(40u32.to_le_bytes());
        assert_eq!(sniff(&bmp), Some("image/bmp"));
        assert_eq!(
            sniff(&[0, 0, 1, 0, 1, 0, 16, 16, 0, 0]),
            Some("image/x-icon")
        );
        assert_eq!(sniff(b"II*\0\x08\0\0\0"), Some("image/tiff"));
        assert_eq!(sniff(b"RIFF\0\0\0\0WEBPVP8 "), Some("image/webp"));
        assert_eq!(sniff(b"PK\x03\x04\x14\0"), Some("application/zip"));
        assert_eq!(sniff(b"qoif\0\0\0\x05"), Some("image/qoi"));
        assert_eq!(sniff(b"farbfeld\0\0"), Some("image/x-farbfeld"));
        assert_eq!(sniff(b"#?RADIANCE\nFORMAT"), Some("image/vnd.radiance"));
        assert_eq!(sniff(b"P6\n5 3\n255\n"), Some("image/x-portable-anymap"));
        assert_eq!(
            sniff(b"Pa text"),
            Some("text/plain"),
            "P and a letter is a text"
        );
        assert_eq!(sniff(b"%PDF-1.7\n"), Some("application/pdf"));
        let mut exe = vec![0u8; 0x48];
        exe[..2].copy_from_slice(b"MZ");
        exe[0x3C..0x40].copy_from_slice(&0x40u32.to_le_bytes());
        exe[0x40..0x44].copy_from_slice(b"PE\0\0");
        assert_eq!(sniff(&exe), Some(PE));
        exe[0x40] = b'X';
        assert_eq!(sniff(&exe), None, "MZ with another header is no program");
        assert_eq!(sniff(&[]), None);
        assert_eq!(sniff(&[0, 1, 2, 3, 0xFE, 0x80]), None);
    }

    #[test]
    fn a_text_is_xml_html_or_plain_in_any_encoding() {
        assert_eq!(
            sniff(b"<?xml version=\"1.0\"?>\r\n<assembly/>"),
            Some("application/xml")
        );
        assert_eq!(
            sniff(b"\xEF\xBB\xBF<?xml version='1.0'?>"),
            Some("application/xml")
        );
        assert_eq!(sniff(b"  <!DOCTYPE html>\n<html>"), Some("text/html"));
        assert_eq!(
            sniff("x = 1\nname = \u{0444}\n".as_bytes()),
            Some("text/plain")
        );
        // A character of UTF-8 cut at the end of the bytes looked at.
        assert_eq!(
            sniff(&"\u{0444}\u{0444}".as_bytes()[..3]),
            Some("text/plain")
        );
        let wide: Vec<u8> = "<html>".encode_utf16().flat_map(u16::to_le_bytes).collect();
        let mut marked = vec![0xFF, 0xFE];
        marked.extend(&wide);
        assert_eq!(sniff(&marked), Some("text/html"));
        assert_eq!(sniff(&wide), Some("text/html; charset=utf-16le"));
        let plain: Vec<u8> = "name".encode_utf16().flat_map(u16::to_be_bytes).collect();
        assert_eq!(sniff(&plain), Some("text/plain; charset=utf-16be"));
        assert_eq!(sniff(b"one\0two"), None, "a zero byte is no text of UTF-8");
    }

    #[test]
    fn the_spellings_of_a_type_and_its_fallbacks() {
        assert!(same("image/ICO", "image/vnd.microsoft.icon"));
        assert!(same("text/xml; charset=utf-8", "application/xml"));
        assert!(!same("image/png", "image/jpeg"));
        assert_eq!(base("Text/Plain ; charset=UTF-16LE"), "text/plain");
        assert_eq!(
            parameter("text/plain; Charset=\"UTF-16LE\"", "charset").as_deref(),
            Some("UTF-16LE")
        );
        assert_eq!(parameter("text/plain", "charset"), None);
        assert_eq!(
            fallbacks("image/svg+xml"),
            ["application/xml", "text/plain"]
        );
        assert_eq!(fallbacks("text/xml"), ["text/plain"]);
        assert_eq!(fallbacks("text/html"), ["text/plain"]);
        assert_eq!(fallbacks("application/json"), ["text/plain"]);
        assert!(fallbacks("text/plain").is_empty());
        assert!(fallbacks("image/png").is_empty());
    }
}
