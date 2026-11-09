//! MIME types: the spellings of one type (`canonical`), the types a textual one falls back to when
//! no plugin names it (`fallbacks`), and the least the host knows of the type of some bytes by
//! their first ones (`sniff`: a text, in UTF-8 or UTF-16). A plugin names the types of its formats
//! (`plugin::Format::mimes`) and finds its own in the first bytes (`plugin::Plugin::sniff`); the
//! host asks every plugin before it falls back to a text (`plugin::Plugins::sniff`), and finds the
//! plugin of some bytes by their type (`plugin::Host::open_inside`).

/// How many first bytes `sniff` looks at, at most.
pub const SNIFFED: usize = 512;

/// Any bytes: the type of the plugin a file no other plugin opens falls back to.
pub const OCTET: &str = "application/octet-stream";

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
    (
        "application/x-msdownload",
        "application/vnd.microsoft.portable-executable",
    ),
    (
        "application/x-dosexec",
        "application/vnd.microsoft.portable-executable",
    ),
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

/// The type of some bytes the host knows by itself: a text of UTF-8 or UTF-16 (with or without
/// a mark) is `text/plain`, with the `charset` of UTF-16 without its mark; any other bytes have
/// none here, and the plugins name their types (`plugin::Plugins::sniff`).
pub fn sniff(bytes: &[u8]) -> Option<&'static str> {
    text(bytes)?;
    Some(match charset(bytes) {
        Some("utf-16le") => "text/plain; charset=utf-16le",
        Some(_) => "text/plain; charset=utf-16be",
        None => "text/plain",
    })
}

/// The type with the `charset` of the bytes when they are a text of UTF-16 without its mark: a
/// textual type a plugin found in them (`application/json` of UTF-16 is `application/json;
/// charset=utf-16le`); any other type as it is.
pub fn with_charset(mime: &str, bytes: &[u8]) -> String {
    let textual = canonical(mime) == "text/plain" || fallbacks(mime).contains(&"text/plain");
    match charset(bytes) {
        Some(set) if textual && parameter(mime, "charset").is_none() => {
            format!("{mime}; charset={set}")
        }
        _ => mime.to_owned(),
    }
}

/// The charset of the first bytes when they are a text of UTF-16 without its mark: a zero byte
/// in every pair, after the character in little endian.
pub fn charset(bytes: &[u8]) -> Option<&'static str> {
    let bytes = &bytes[..bytes.len().min(SNIFFED)];
    let marked = bytes.starts_with(b"\xFF\xFE") || bytes.starts_with(b"\xFE\xFF");
    if marked || utf8(bytes).is_some() {
        return None;
    }
    let (pairs, _) = bytes.as_chunks::<2>();
    if pairs.len() < 2 {
        return None;
    }
    if pairs.iter().all(|[_, high]| *high == 0) && utf16(bytes, false).is_some() {
        return Some("utf-16le");
    }
    if pairs.iter().all(|[high, _]| *high == 0) && utf16(bytes, true).is_some() {
        return Some("utf-16be");
    }
    None
}

/// The text the first bytes hold (`SNIFFED` of them at most), in UTF-8 or UTF-16 with or
/// without a mark, when every character is readable: the start a plugin looks at for the type
/// of a text (an object of JSON, a declaration of XML).
pub fn text(bytes: &[u8]) -> Option<String> {
    let bytes = &bytes[..bytes.len().min(SNIFFED)];
    if let Some(rest) = bytes.strip_prefix(b"\xEF\xBB\xBF") {
        return utf8(rest);
    }
    if let Some(rest) = bytes.strip_prefix(b"\xFF\xFE") {
        return utf16(rest, false);
    }
    if let Some(rest) = bytes.strip_prefix(b"\xFE\xFF") {
        return utf16(rest, true);
    }
    if let Some(text) = utf8(bytes) {
        return Some(text);
    }
    match charset(bytes) {
        Some("utf-16le") => utf16(bytes, false),
        Some(_) => utf16(bytes, true),
        None => None,
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_host_knows_a_text_in_any_encoding() {
        assert_eq!(sniff(b"x = 1\nname = a\n"), Some("text/plain"));
        assert_eq!(sniff("\u{0444}\u{0444}".as_bytes()), Some("text/plain"));
        // A character of UTF-8 cut at the end of the bytes looked at.
        assert_eq!(
            sniff(&"\u{0444}\u{0444}".as_bytes()[..3]),
            Some("text/plain")
        );
        let wide: Vec<u8> = "<html>".encode_utf16().flat_map(u16::to_le_bytes).collect();
        let mut marked = vec![0xFF, 0xFE];
        marked.extend(&wide);
        assert_eq!(
            sniff(&marked),
            Some("text/plain"),
            "a mark needs no charset"
        );
        assert_eq!(text(&marked).as_deref(), Some("<html>"));
        assert_eq!(sniff(&wide), Some("text/plain; charset=utf-16le"));
        assert_eq!(text(&wide).as_deref(), Some("<html>"));
        let plain: Vec<u8> = "name".encode_utf16().flat_map(u16::to_be_bytes).collect();
        assert_eq!(sniff(&plain), Some("text/plain; charset=utf-16be"));
        assert_eq!(sniff(b"one\0two"), None, "a zero byte is no text of UTF-8");
        assert_eq!(
            sniff(b"\x89PNG\r\n\x1a\n"),
            None,
            "the plugins know the rest"
        );
        assert_eq!(sniff(&[]), None);
        assert_eq!(
            text(b"\xEF\xBB\xBF{\"a\": 1}").as_deref(),
            Some("{\"a\": 1}")
        );
        assert_eq!(
            with_charset("application/json", &wide),
            "application/json; charset=utf-16le"
        );
        assert_eq!(with_charset("application/json", b"{}"), "application/json");
        assert_eq!(with_charset("image/png", &wide), "image/png");
        assert_eq!(
            with_charset("text/plain; charset=utf-16le", &wide),
            "text/plain; charset=utf-16le"
        );
    }

    #[test]
    fn the_spellings_of_a_type_and_its_fallbacks() {
        assert!(same("image/ICO", "image/vnd.microsoft.icon"));
        assert!(same("text/xml; charset=utf-8", "application/xml"));
        assert!(same(
            "application/x-msdownload",
            "application/vnd.microsoft.portable-executable"
        ));
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
