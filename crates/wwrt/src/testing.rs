//! Helpers of the tests of a program or a plugin on wwrt: the texts of its sources the window
//! shows, and the ones a translation lacks. A test of a plugin is three lines:
//!
//! ```ignore
//! let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
//! let missing = wwrt::testing::missing(&TRANSLATIONS, &wwrt::testing::plugin_texts(&src));
//! assert!(missing.is_empty(), "{missing:#?}");
//! ```

use std::path::{Path, PathBuf};

/// The calls of `tr` and `trf`, spelt so that a scan of the runtime finds none in this file.
const CALLS: [&str; 2] = ["tr\u{28}", "trf\u{28}"];

/// The literals of the calls of `tr` and `trf` in the Rust sources under a folder.
pub fn texts(dir: &Path) -> Vec<String> {
    found(dir, &CALLS)
}

/// `texts` and the literals a plugin gives the host to translate: `const NAME: &str =` (the name
/// of the plugin) and the fields `name:` (of a format) and `label:` (of a tool).
pub fn plugin_texts(dir: &Path) -> Vec<String> {
    found(
        dir,
        &[CALLS[0], CALLS[1], "const NAME: &str =", "name:", "label:"],
    )
}

/// The texts of `wanted` a translation of `list` (the code and the file of each) lacks, as
/// `code: text`, and the files that do not read.
pub fn missing(list: &[(&str, &str)], wanted: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for (code, text) in list {
        match ww::i18n::parse(text, code) {
            Ok((_, table)) => {
                for w in wanted {
                    if !table.contains_key(w) {
                        out.push(format!("{code}: {w}"));
                    }
                }
            }
            Err(e) => out.push(format!("{code}: {e}")),
        }
    }
    out.sort();
    out.dedup();
    out
}

fn found(dir: &Path, patterns: &[&str]) -> Vec<String> {
    let mut files = Vec::new();
    sources(dir, &mut files);
    let mut out = Vec::new();
    for file in files {
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        for pattern in patterns {
            literals(&text, pattern, &mut out);
        }
    }
    out.retain(|t| !t.is_empty());
    out.sort();
    out.dedup();
    out
}

/// The string literals right after a pattern (and white space), where the pattern starts a word.
fn literals(text: &str, pattern: &str, out: &mut Vec<String>) {
    let mut rest = text;
    while let Some(i) = rest.find(pattern) {
        let before = rest[..i].chars().last();
        let after = &rest[i + pattern.len()..];
        rest = after;
        if before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
            continue;
        }
        let trimmed = after.trim_start();
        let Some(body) = trimmed.strip_prefix('"') else {
            continue;
        };
        let mut literal = String::new();
        let mut escaped = false;
        for c in body.chars() {
            if escaped {
                literal.push(match c {
                    'n' => '\n',
                    't' => '\t',
                    other => other,
                });
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                break;
            } else {
                literal.push(c);
            }
        }
        out.push(literal);
    }
}

/// Every Rust source under a folder, in its subfolders too.
fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_literals_of_a_source_are_found() {
        let mut out = Vec::new();
        let source =
            "x(tr(\"Open\")); trf(\n    \"{n} of {count}\", &[]); str(\"no\"); label: \"Fit\",";
        literals(source, CALLS[0], &mut out);
        literals(source, CALLS[1], &mut out);
        literals(source, "label:", &mut out);
        assert_eq!(out, ["Open", "{n} of {count}", "Fit"]);
        let list = [("uk", "{\"Open\": \"Відкрити\"}")];
        let wanted = vec!["Open".to_owned(), "Fit".to_owned()];
        assert_eq!(missing(&list, &wanted), ["uk: Fit"]);
    }
}
