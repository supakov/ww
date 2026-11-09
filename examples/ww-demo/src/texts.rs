//! The plugin of texts the program makes itself (`Plugins::add`): a text of any name, of UTF-8 or
//! UTF-16, as a page of W# (`ww::mdview`), Markdown when its type or its name says so. It names
//! no extension, only its MIME types, so it opens the files the host finds a text in by their
//! first bytes, the bytes another view offers as a text, and a part of a bundle that is one.

use std::path::Path;

use wwrt::blocks::BlockFile;
use wwrt::egui;
use wwrt::mime;
use wwrt::plugin::{Format, Host, Plugin, View};
use wwrt::serde_json::Value;
use wwrt::ww::Search;
use wwrt::ww::i18n::trf;
use wwrt::ww::mdview::{self, MdDoc};

/// The largest text the plugin reads whole.
const MAX_BYTES: u64 = 4 * 1024 * 1024;

static FORMATS: [Format; 1] = [Format {
    name: "Texts",
    extensions: &[],
    mimes: &["text/plain", "text/markdown"],
}];

pub struct TextsPlugin;

/// The text of some bytes in the charset their type names (UTF-16 without a mark), else UTF-8.
fn decode(bytes: &[u8], mime: &str) -> String {
    let set = mime::parameter(mime, "charset").map(|s| s.to_ascii_lowercase());
    let (pairs, _) = bytes.as_chunks::<2>();
    let units = |big: bool| -> Vec<u16> {
        pairs
            .iter()
            .map(|p| {
                if big {
                    u16::from_be_bytes(*p)
                } else {
                    u16::from_le_bytes(*p)
                }
            })
            .collect()
    };
    match set.as_deref() {
        Some("utf-16le") => String::from_utf16_lossy(&units(false)),
        Some("utf-16be") => String::from_utf16_lossy(&units(true)),
        _ => {
            let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
            String::from_utf8_lossy(bytes).into_owned()
        }
    }
}

/// The view of a text under a name.
fn view(name: &str, text: String, markdown: bool) -> Box<dyn View> {
    let doc = if markdown {
        MdDoc::new(name, None, text)
    } else {
        MdDoc::plain(name, None, text)
    };
    Box::new(TextView { doc })
}

impl Plugin for TextsPlugin {
    fn name(&self) -> &'static str {
        "Texts"
    }

    fn formats(&self) -> &'static [Format] {
        &FORMATS
    }

    fn open(&self, path: &Path, _settings: &Value) -> Result<Box<dyn View>, String> {
        let file = BlockFile::open(path).map_err(|e| e.to_string())?;
        let bytes = file.to_vec(MAX_BYTES)?;
        let mime = wwrt::mime::sniff(&bytes).unwrap_or("text/plain");
        let name = path
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        let markdown = name.to_ascii_lowercase().ends_with(".md");
        Ok(view(&name, decode(&bytes, mime), markdown))
    }

    fn opens_bytes(&self) -> bool {
        true
    }

    fn open_bytes(
        &self,
        name: &str,
        mime: &str,
        bytes: BlockFile,
        _settings: &Value,
    ) -> Result<Box<dyn View>, String> {
        let bytes = bytes.to_vec(MAX_BYTES)?;
        let markdown = mime::same(mime, "text/markdown") || name.ends_with(".md");
        Ok(view(name, decode(&bytes, mime), markdown))
    }
}

struct TextView {
    doc: MdDoc,
}

impl View for TextView {
    fn ui(&mut self, ui: &mut egui::Ui, host: &mut Host) {
        let (wrap, width) = (host.wrap_lines, host.page_width);
        egui::CentralPanel::default()
            .show(ui, |ui| mdview::md_view(ui, &mut self.doc, wrap, width));
    }

    fn search(&mut self) -> Option<&mut Search> {
        Some(&mut self.doc.search)
    }

    fn status(&self) -> String {
        trf("{n} lines", &[("n", &self.doc.text.lines().count())])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_text_of_any_encoding_is_read() {
        let wide: Vec<u8> = "hi".encode_utf16().flat_map(u16::to_le_bytes).collect();
        assert_eq!(decode(&wide, "text/plain; charset=utf-16le"), "hi");
        assert_eq!(decode(b"\xEF\xBB\xBFhi", "text/plain"), "hi");
        let plugin = TextsPlugin;
        let view = plugin
            .open_bytes(
                "a.md",
                "text/plain",
                BlockFile::from_bytes(b"# A\nb".to_vec()),
                &Value::Null,
            )
            .unwrap();
        assert_eq!(view.status(), "2 lines");
    }
}
