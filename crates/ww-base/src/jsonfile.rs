//! Strict reading of JSON files: a size limit, no byte order mark, a nesting limit.
//!
//! Every file the program reads (profiles, diffs, catalog files, templates, settings) goes through
//! this module, so a defect becomes a short message and never a crash (principle P5).

use std::path::Path;

use serde::Serialize;
use serde::de::DeserializeOwned;

/// The default size limit of a document file.
pub const MAX_BYTES: u64 = 16 * 1024 * 1024;
/// The deepest nesting of objects and arrays a file may have.
pub const MAX_DEPTH: usize = 32;

const BOM: [u8; 3] = [0xEF, 0xBB, 0xBF];

/// What went wrong while reading a JSON file. `name` is the file name only, never the full path.
#[derive(Debug, thiserror::Error)]
pub enum JsonError {
    #[error("{name}: {source}")]
    Io {
        name: String,
        #[source]
        source: std::io::Error,
    },
    #[error("{name}: larger than {limit} bytes")]
    TooLarge { name: String, limit: u64 },
    #[error("{name}: a byte order mark is not allowed")]
    Bom { name: String },
    #[error("{name}: not UTF-8")]
    Utf8 { name: String },
    #[error("{name}: nesting deeper than {limit} levels")]
    TooDeep { name: String, limit: usize },
    #[error("{name}: {source}")]
    Syntax {
        name: String,
        #[source]
        source: serde_json::Error,
    },
}

/// The file name of a path for messages.
pub fn display_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

/// Reads a text file of at most `limit` bytes, UTF-8 without a byte order mark.
pub fn read_text(path: &Path, limit: u64) -> Result<String, JsonError> {
    let name = display_name(path);
    let meta = std::fs::metadata(path).map_err(|source| JsonError::Io {
        name: name.clone(),
        source,
    })?;
    if meta.len() > limit {
        return Err(JsonError::TooLarge { name, limit });
    }
    let bytes = std::fs::read(path).map_err(|source| JsonError::Io {
        name: name.clone(),
        source,
    })?;
    if bytes.len() as u64 > limit {
        return Err(JsonError::TooLarge { name, limit });
    }
    if bytes.starts_with(&BOM) {
        return Err(JsonError::Bom { name });
    }
    String::from_utf8(bytes).map_err(|_| JsonError::Utf8 { name })
}

/// The text of bytes another program wrote: UTF-8 with or without its byte order mark, or UTF-16
/// with its mark, little or big endian (Windows PowerShell and the ADMX of most vendors write it).
pub fn decode_bom(bytes: &[u8]) -> Option<String> {
    if let Some(rest) = bytes.strip_prefix(&BOM[..]) {
        return String::from_utf8(rest.to_vec()).ok();
    }
    let utf16 = |rest: &[u8], little: bool| {
        if !rest.len().is_multiple_of(2) {
            return None;
        }
        let units: Vec<u16> = rest
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| {
                if little {
                    u16::from_le_bytes(*c)
                } else {
                    u16::from_be_bytes(*c)
                }
            })
            .collect();
        String::from_utf16(&units).ok()
    };
    if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE][..]) {
        return utf16(rest, true);
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFE, 0xFF][..]) {
        return utf16(rest, false);
    }
    String::from_utf8(bytes.to_vec()).ok()
}

/// Reads a text file of at most `limit` bytes written by another program, in the encodings of
/// [`decode_bom`].
pub fn read_text_bom(path: &Path, limit: u64) -> Result<String, JsonError> {
    let name = display_name(path);
    let meta = std::fs::metadata(path).map_err(|source| JsonError::Io {
        name: name.clone(),
        source,
    })?;
    if meta.len() > limit {
        return Err(JsonError::TooLarge { name, limit });
    }
    let bytes = std::fs::read(path).map_err(|source| JsonError::Io {
        name: name.clone(),
        source,
    })?;
    decode_bom(&bytes).ok_or(JsonError::Utf8 { name })
}

/// Parses JSON text into `T` after the checks of this module.
pub fn parse<T: DeserializeOwned>(text: &str, name: &str) -> Result<T, JsonError> {
    if text.as_bytes().starts_with(&BOM) {
        return Err(JsonError::Bom {
            name: name.to_owned(),
        });
    }
    let depth = nesting_depth(text);
    if depth > MAX_DEPTH {
        return Err(JsonError::TooDeep {
            name: name.to_owned(),
            limit: MAX_DEPTH,
        });
    }
    serde_json::from_str(text).map_err(|source| JsonError::Syntax {
        name: name.to_owned(),
        source,
    })
}

/// Reads and parses a JSON file of at most `limit` bytes.
pub fn read<T: DeserializeOwned>(path: &Path, limit: u64) -> Result<T, JsonError> {
    let text = read_text(path, limit)?;
    parse(&text, &display_name(path))
}

/// What went wrong while reading or writing a document of the program (a profile, a diff, a
/// bookmarks file, a users file). `name` is the file name only, never the full path.
#[derive(Debug, thiserror::Error)]
pub enum DocError {
    #[error(transparent)]
    Json(#[from] JsonError),
    #[error("{name}: {source}")]
    Io {
        name: String,
        #[source]
        source: std::io::Error,
    },
    #[error("{name}: not a WinWeaver {kind} (format {found})")]
    Format {
        name: String,
        kind: &'static str,
        found: String,
    },
    #[error("{name}: {kind} version {found} is newer than this program knows ({known})")]
    Version {
        name: String,
        kind: &'static str,
        found: u32,
        known: u32,
    },
}

/// A JSON document of the program: a format name and a version in its first fields, an
/// extension, a kind the messages name. The methods with bodies read and write every document
/// the same way: strictly, and through a temporary file that replaces the target.
pub trait Document: Serialize + DeserializeOwned {
    const FORMAT: &'static str;
    const VERSION: u32;
    const EXTENSION: &'static str;
    /// What a message calls the document: "profile", "diff", "bookmark file", "users file".
    const KIND: &'static str;
    /// The size limit of its file.
    const MAX_BYTES: u64 = MAX_BYTES;

    fn format(&self) -> &str;
    fn version(&self) -> u32;

    /// Parses the text and checks its format and version.
    fn parse_checked(text: &str, name: &str) -> Result<Self, DocError> {
        let doc: Self = parse(text, name)?;
        if doc.format() != Self::FORMAT {
            return Err(DocError::Format {
                name: name.to_owned(),
                kind: Self::KIND,
                found: doc.format().to_owned(),
            });
        }
        if doc.version() > Self::VERSION {
            return Err(DocError::Version {
                name: name.to_owned(),
                kind: Self::KIND,
                found: doc.version(),
                known: Self::VERSION,
            });
        }
        Ok(doc)
    }

    /// Reads the file strictly.
    fn load_checked(path: &Path) -> Result<Self, DocError> {
        let text = read_text(path, Self::MAX_BYTES)?;
        Self::parse_checked(&text, &display_name(path))
    }

    /// The canonical text of the document: two-space indentation, a trailing line break.
    fn json_text(&self) -> String {
        let mut text = serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".into());
        text.push('\n');
        text
    }
}

/// Writes a text through a temporary file that replaces the target, so a failed write leaves the
/// old file whole.
pub fn write_atomic(path: &Path, text: &str) -> Result<(), DocError> {
    let name = display_name(path);
    let io = |source: std::io::Error| DocError::Io {
        name: name.clone(),
        source,
    };
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, text).map_err(io)?;
    std::fs::rename(&tmp, path).map_err(io)
}

/// The deepest nesting of objects and arrays in JSON text, counted outside strings.
pub fn nesting_depth(text: &str) -> usize {
    let mut depth = 0usize;
    let mut deepest = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for c in text.chars() {
        if in_string {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }
        match c {
            '"' => in_string = true,
            '{' | '[' => {
                depth += 1;
                deepest = deepest.max(depth);
            }
            '}' | ']' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    deepest
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_is_read_in_utf8_and_in_utf16_with_its_mark() {
        let text = "Ab\u{439}";
        let le: Vec<u8> = [0xFF, 0xFE]
            .into_iter()
            .chain(text.encode_utf16().flat_map(u16::to_le_bytes))
            .collect();
        let be: Vec<u8> = [0xFE, 0xFF]
            .into_iter()
            .chain(text.encode_utf16().flat_map(u16::to_be_bytes))
            .collect();
        assert_eq!(decode_bom(&le).as_deref(), Some(text));
        assert_eq!(decode_bom(&be).as_deref(), Some(text));
        assert_eq!(decode_bom(b"\xEF\xBB\xBFx").as_deref(), Some("x"));
        assert_eq!(decode_bom(b"x").as_deref(), Some("x"));
        assert_eq!(decode_bom(&[0xFF, 0xFE, 0x41]), None, "an odd length");
        assert_eq!(decode_bom(&[0xC3]), None);
    }

    #[test]
    fn depth_ignores_brackets_in_strings() {
        assert_eq!(nesting_depth(r#"{"a": "[[[", "b": [1, [2]]}"#), 3);
        assert_eq!(nesting_depth(r#"{"a": "\"[", "b": 1}"#), 1);
        assert_eq!(nesting_depth("12"), 0);
    }

    #[test]
    fn too_deep_is_refused() {
        let text = format!(
            "{}1{}",
            "[".repeat(MAX_DEPTH + 1),
            "]".repeat(MAX_DEPTH + 1)
        );
        let err = parse::<serde_json::Value>(&text, "deep.json").unwrap_err();
        assert!(matches!(err, JsonError::TooDeep { .. }), "{err}");
    }

    #[test]
    fn bom_is_refused() {
        let err = parse::<serde_json::Value>("\u{feff}{}", "bom.json").unwrap_err();
        assert!(matches!(err, JsonError::Bom { .. }), "{err}");
    }
}
