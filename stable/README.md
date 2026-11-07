# The stable interface of the plugins of wview

A plugin in any language with a C FFI (C, C++, Zig, Go, Delphi, Rust of any toolchain) is a DLL
that exports one function, `wv_plugin_get`, declared by `wview.h`. Unlike a native plugin, it
outlives the releases of wview: built once, it loads into every wview that knows its interface
version (1 now). It draws a picture of RGBA pixels and names the controls of its bar; wview draws
them over the view, gives the plugin the keys and the mouse of its view and calls it back when a
control changes (docs/plugins.md, sections 6.7 to 6.10).

## The interface (`wview.h`)

- `wv_plugin_get` gives a `wv_plugin`: the version of the interface, the name, the formats (a name
  and the extensions), and four functions: `open` (a file to a view, or the reason it did not
  open), `frame` (the input since the last frame in, the picture, the controls, the keys the view
  takes, the cursor and a line of the status bar out), `control` (a control of the bar changed)
  and `close`.
- Every call comes from the thread of the window; every text is UTF-8. What wview gives lives for
  the call; what the plugin gives lives until its next call on that view.
- The keys come by groups (`WV_KEYS_ARROWS`, `_PAGES`, `_LETTERS`, `_DIGITS`, `_SIGNS`, `_SPACE`,
  `_ESCAPE`, `_ENTER`) while the view has the focus, by their names of W3C; the others stay the
  window's. The mouse: its place in the pixels of the view, the buttons held, clicked and clicked
  twice, the movement of a drag, the wheel in lines.
- A picture that is still being drawn sets `again`, and wview draws another frame at once.

## The example: the Mandelbrot set (`wv-mandelbrot`)

`examples/wv-mandelbrot/mandelbrot.c` (in the root of the repository) opens a `.mandel` file
(`seahorse.mandel`: the centre, the zoom, the iterations, the palette and the smooth colours) and
saves the view back. Its bar: Iterations, Palette, Smooth colours, Zoom in, Zoom out, Reset, Save.
Its mouse: the wheel zooms at the pointer, a drag moves the picture, Shift and a drag draws a
rectangle that the release zooms into, a double click centres and zooms in, the right button zooms
out. Its keys: the arrows move, + and - zoom, Page Up and Page Down double or halve the iterations,
P takes the next palette, Home resets, Escape drops a rectangle. It draws in steps, blocks of 8
pixels, then 4, 2 and 1, a few milliseconds a frame.

Built with the compiler of Visual Studio (x64, in a Developer Command Prompt), in the root of the
repository:

```bat
cl /nologo /LD /O2 /MT /W3 /std:c11 /I stable examples\wv-mandelbrot\mandelbrot.c /Fe:wv_mandelbrot.dll
```

`/MT` links the C runtime into the DLL, so it needs no DLL of the Visual C++ runtime. With MinGW:

```bash
gcc -shared -O2 -std=c11 -I stable examples/wv-mandelbrot/mandelbrot.c -o wv_mandelbrot.dll
```

Copy the DLL into the folder `plugins` next to `wview.exe`; `wview --plugins` lists it, and
`wview --probe seahorse.mandel` draws it without a window.

## Rust of any toolchain: the crate `wview-stable`

`rust/` is the crate `wview-stable` (edition 2021, Rust 1.77 or later, no dependency): a plugin
of the stable interface in Rust without a line of C. Copy the folder next to the crate of the
plugin, outside this repository and the one of wview (whose `.cargo/config.toml` links std
dynamically for the native plugins), and:

```toml
[lib]
crate-type = ["cdylib"]

[dependencies]
wview-stable = { path = "../wview-stable" }
```

A type of the plugin implements `wview_stable::View`: `frame` takes an `Input` (the size of the
view, the focus, the mouse, the keys by their names) and gives a `Frame` (a `Picture` of RGBA
pixels, the `Control`s of the bar, the groups of keys it takes, a `Cursor`, `again`, a line of the
status bar); `control` takes a change of a control. `wview_stable::plugin!` names the plugin, its
formats, the type of its view and the function that opens a file, and exports `wv_plugin_get`; the
documentation of the crate holds a whole plugin. The crate keeps the texts and the controls of a
frame until the next call on the view, reads the input and writes the output by the sizes the host
gives, and turns a panic of the plugin into the line *the plugin stopped* of its view.

```bash
cargo build --release
```

`target/release/<name>.dll` goes into `plugins`. For the toolchain msvc the line
`rustflags = ["-C", "target-feature=+crt-static"]` under `[target.x86_64-pc-windows-msvc]` of the
`.cargo/config.toml` of the plugin links the C runtime in, as `/MT` does for C.
