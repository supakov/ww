//! ww-grey: any bytes as a grey picture, one pixel a byte, a row of `width` bytes; an example of
//! the stable interface of W# (`ww.h`) written in Rust with the crate `ww-stable`, built by any
//! toolchain.
//!
//! A view reads only the rows it shows, through the cache of the blocks of the host: a file of
//! any size (`open_file` of the host) or some bytes another view offers (a `Source`: a part of a
//! file, without a copy). The wheel, the arrows and the page keys scroll, + and - change the
//! width of a row, the bar has the width and the zoom; the status bar names the offset of the
//! first row shown and the state of the cache of the host. Its one tool (`tools!`), `grey_cache`,
//! gives an agent the state of that cache.

use ww_stable::{
    cache, keys, open_file, Control, Cursor, File, Format, Frame, Input, Picture, Plugin, Source,
    Tool, Tools, View, MODE_READ, TOOL_FAILED, TOOL_NOT_FOUND,
};

/// The widths of a row the bar offers.
const MIN_WIDTH: u32 = 8;
const MAX_WIDTH: u32 = 4096;
/// The ids of the controls.
const WIDTH: u32 = 1;
const ZOOM: u32 = 2;
const TOP: u32 = 3;

struct Grey {
    source: Source,
    /// The bytes a row.
    width: u32,
    /// The pixels of the screen a byte.
    zoom: u32,
    /// The first row shown.
    row: u64,
    pixels: Vec<u8>,
    /// What the last picture showed: the width and the height of the view, the row and the zoom.
    shown: (u32, u32, u64, u32, u32),
}

impl Grey {
    fn new(source: Source) -> Self {
        Grey {
            source,
            width: 256,
            zoom: 2,
            row: 0,
            pixels: Vec::new(),
            shown: (0, 0, u64::MAX, 0, 0),
        }
    }

    /// The rows of the bytes.
    fn rows(&self) -> u64 {
        self.source.len().div_ceil(u64::from(self.width))
    }

    fn scroll(&mut self, by: i64) {
        let last = self.rows().saturating_sub(1);
        self.row = (self.row as i64 + by).clamp(0, last as i64) as u64;
    }

    /// The picture of the rows the view shows, read through the source.
    fn draw(&mut self, w: u32, h: u32) {
        let zoom = self.zoom.max(1);
        let rows = h.div_ceil(zoom);
        let width = u64::from(self.width);
        let start = self.row * width;
        let count = (u64::from(rows) * width).min(self.source.len().saturating_sub(start));
        let bytes = self.source.read(start, count as usize);
        self.pixels = vec![0; (w * h * 4) as usize];
        for y in 0..h {
            let row = (y / zoom) as usize;
            for x in 0..w {
                let col = (x / zoom) as usize;
                let at = (y * w + x) as usize * 4;
                let px = &mut self.pixels[at..at + 4];
                if col >= self.width as usize {
                    px.copy_from_slice(&[40, 44, 52, 255]);
                    continue;
                }
                match bytes.get(row * self.width as usize + col) {
                    Some(v) => px.copy_from_slice(&[*v, *v, *v, 255]),
                    None => px.copy_from_slice(&[24, 26, 30, 255]),
                }
            }
        }
    }
}

impl View for Grey {
    fn frame(&mut self, input: &Input) -> Frame<'_> {
        let (w, h) = (input.width.max(1), input.height.max(1));
        let page = i64::from(h / self.zoom.max(1)).max(1);
        if input.hovered && input.wheel[1] != 0.0 {
            self.scroll((input.wheel[1] * 3.0) as i64);
        }
        for key in input.keys.iter().filter(|k| k.pressed) {
            match key.name.as_str() {
                "ArrowDown" => self.scroll(1),
                "ArrowUp" => self.scroll(-1),
                "PageDown" => self.scroll(page),
                "PageUp" => self.scroll(-page),
                "Home" => self.row = 0,
                "End" => self.scroll(i64::MAX / 2),
                "+" | "=" => self.width = (self.width * 2).min(MAX_WIDTH),
                "-" => self.width = (self.width / 2).max(MIN_WIDTH),
                _ => {}
            }
        }
        let now = (w, h, self.row, self.width, self.zoom);
        let changed = now != self.shown;
        if changed {
            self.draw(w, h);
            self.shown = now;
        }
        let state = cache().map_or_else(String::new, |c| {
            format!(
                "; cache: {} blocks of {} KB, {} read, {} found",
                c.blocks,
                c.block / 1024,
                c.reads,
                c.hits
            )
        });
        Frame {
            picture: Some(Picture::new(w, h, &self.pixels)),
            changed,
            controls: vec![
                Control::slider(
                    WIDTH,
                    "Width",
                    f64::from(self.width),
                    f64::from(MIN_WIDTH),
                    f64::from(MAX_WIDTH),
                    8.0,
                ),
                Control::choice(ZOOM, "Zoom", self.zoom as usize - 1, &["1", "2", "3", "4"]),
                Control::button(TOP, "Top"),
            ],
            keys: keys::ARROWS | keys::PAGES | keys::SIGNS,
            cursor: Cursor::Default,
            again: false,
            status: format!(
                "{} bytes, offset {} of row {} of {}{state}",
                self.source.len(),
                self.row * u64::from(self.width),
                self.row + 1,
                self.rows().max(1)
            ),
        }
    }

    fn control(&mut self, id: u32, value: f64) {
        match id {
            WIDTH => self.width = (value as u32).clamp(MIN_WIDTH, MAX_WIDTH),
            ZOOM => self.zoom = value as u32 + 1,
            TOP => self.row = 0,
            _ => {}
        }
        self.scroll(0);
    }
}

ww_stable::plugin!(
    Grey,
    Plugin {
        name: "Bytes as a picture (stable Rust)",
        formats: &[Format {
            name: "Any bytes",
            extensions: &["raw"],
            mimes: &["application/octet-stream"],
        }],
        files: &[File {
            path: "guide.md",
            bytes: include_bytes!("../guide.md"),
        }],
        open: |path: &str| open_file(path).map(Grey::new),
        open_bytes: Some(|source: Source, _name: &str, _mime: &str| Ok(Grey::new(source))),
        sniff: None,
    }
);

/// The tool of the plugin: the state of the cache of the blocks of the host, as JSON.
fn run_tool(tool: &str, _args: &str, _options: &str) -> Result<String, (i32, String)> {
    if tool != "grey_cache" {
        return Err((TOOL_NOT_FOUND, format!("no tool {tool}")));
    }
    let c = cache().ok_or((TOOL_FAILED, "the host tells no cache".to_owned()))?;
    Ok(format!(
        r#"{{"block": {}, "budget": {}, "used": {}, "blocks": {}, "files": {}, "reads": {}, "hits": {}}}"#,
        c.block, c.budget, c.used, c.blocks, c.files, c.reads, c.hits
    ))
}

ww_stable::tools!(Tools {
    name: "Bytes as a picture (stable Rust)",
    tools: &[Tool {
        name: "grey_cache",
        description: "The state of the cache of the blocks of the program: the size of a block, the budget, the bytes and the blocks in memory, the open files, the reads and the hits.",
        schema: r#"{"type": "object", "properties": {}}"#,
        group: "grey",
        mode: MODE_READ,
        needs: "",
    }],
    options: &[],
    call: run_tool,
});
