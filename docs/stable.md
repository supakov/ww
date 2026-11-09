# The stable interface of the plugins: `ww.h`

A plugin of the stable interface is a DLL of any language with a C FFI (C, C++, Rust of any
toolchain, Zig, Go, Free Pascal and Delphi, C# with Native AOT) that exports `ww_plugin_get` (its
views), `ww_tools_get` (its tools, section 2a), or both, declared by `stable/ww.h`. Unlike a native plugin (docs/plugins.md) it shares no
crate with the host, so it is built once and loads into every program on W# that knows its
version of the interface, whatever compiler built either. It draws a picture of RGBA pixels and
names the controls of its bar; the host draws them, hands the view the keys and the mouse of its
area, and calls back when a control changes. It reads files and the bytes other views offer
through the cache of the blocks of the host.

The version is 1 (`WW_API_VERSION`) and stays 1 until the first release of W#: until then the
structures may still change, and a plugin is rebuilt with the header of the release it targets.

## 1. What a plugin gives

`ww_plugin_get(const ww_host *host)` gives the table of the plugin, `ww_plugin`, which lives for
the whole run; the host calls it once, after the DLL loaded, and gives its own table, which lives
for the run too.

| Field | What |
|---|---|
| `api_version`, `size` | `WW_API_VERSION` and `sizeof(ww_plugin)` of the header the plugin was built with |
| `name` | the name of the plugin, for the page of the plugins and the messages |
| `formats`, `format_count` | its formats: a name, the extensions (without the dot, lower case) and the MIME types of each (`ww_format`) |
| `files`, `file_count` | the files it embeds (`ww_file`: a path in the set, the bytes, the size), or NULL: its guide, samples, translations (section 5) |
| `open` | opens a file by its path: a view, or NULL with the reason written into a buffer |
| `open_bytes` | opens some bytes of a source under a name, as a MIME type of its formats; NULL for a plugin of files alone |
| `sniff` | the MIME type of a format of the plugin the first bytes of a file show (at most 512), or NULL; NULL for a plugin of names alone |
| `frame` | a frame of a view: the input since the last one in, the picture, the controls, the keys it takes, the cursor and a line of the status bar out |
| `control` | a control of the bar changed: a button pressed, a toggle, a slider or a choice set |
| `close` | the view closes; the plugin frees it |

`open`, `frame` and `close` are required; a table without them is refused with its reason.

## 2. What the host gives

`ww_host` holds the functions any thread of the plugin may call:

| Function | What |
|---|---|
| `open_file(path, error, size)` | a file read in blocks through the cache of the host: a `ww_source`, or NULL with the reason |
| `slice(source, offset, length)` | a part of a source as a source of its own, reading the same blocks |
| `close_source(source)` | frees a source of `open_file` or `slice`; the source given to `open_bytes` is the host's |
| `cache(out)` | the state of the cache (`ww_cache`): the size of a block, the budget, the bytes and the blocks in memory, the files open, the blocks read from the disk and found in memory |
| `call_tool(self, name, args, answer)` | calls a tool of the program by its name as the plugin (`self` is the field of the table, which names the plugin): `args` a JSON object, the answer (a JSON value, or the message of the error) written into `*answer`; gives `WW_TOOL_OK` or the kind of the error. The switches, the mode and the options of the server apply |
| `free_text(text)` | frees an answer of `call_tool` |

A `ww_source` is some bytes: its `length`, a `read(self, offset, out, count)` that copies at
most `count` bytes and gives how many, the file they come from (`path`, NULL for bytes in memory)
and where they start in it (`offset`). The source given to `open_bytes` is a part of a file
another view offers (a selection of a hex view, a part of a bundle) or a file a plugin embeds,
read through the cache, never copied: a view reads the blocks it shows and the rest stays on the
disk. The user sets the size of a block and the budget of the cache in the settings of the host.

## 2a. Tools

`ww_tools_get(const ww_host *host)` gives the table of the tools of the plugin, `ww_tools`, which
lives for the run: the program serves them to AI agents over MCP and calls them by their names
(docs/mcp.md). A DLL may export it alone (tools without a view), `ww_plugin_get` alone, or both.

| Field | What |
|---|---|
| `api_version`, `size` | `WW_API_VERSION` and `sizeof(ww_tools)` |
| `name` | the name of the source of the tools in the tab of the server |
| `tools`, `tool_count` | `ww_tool`: a name (unique in the program), a description for the agent, the JSON schema of its arguments (an object), a group, the mode it needs (`WW_MODE_READ`, `WW_MODE_EDIT`, `WW_MODE_FILES`) and the options it needs (`"key=value,key=value"`, or NULL) |
| `options`, `option_count` | `ww_option`: a key, a title, the values (the weakest first) with their names, or none for a text, the initial value and a help line |
| `call(tool, args, options, answer)` | runs a tool after the host checked the switches, the mode and the options: the arguments and the values of the options of the plugin as JSON objects; the answer (a JSON value, or the message of the error) into `*answer`, freed by the host with `free_answer`; gives `WW_TOOL_OK` (0) or the kind of the error (`WW_TOOL_INVALID`, `WW_TOOL_NOT_FOUND`, `WW_TOOL_NOT_ALLOWED`, `WW_TOOL_READ_FAILED`, `WW_TOOL_WRITE_FAILED`, `WW_TOOL_FAILED`) |
| `free_answer(answer)` | frees an answer of `call` |

`call` comes from any thread of the host, several at once, and a tool may call another tool of
the program through `call_tool` of the host: the plugin keeps the state of its tools safe for
threads. The example in C has three tools (how fast a point leaves the set, the set drawn in
letters, its sample opened in a tab through `call_tool`) and the option `limit`, the most
iterations a call may ask.

## 3. The rules

- Every call of the table comes from the thread of the window; the functions of `ww_host` and the
  `read` of a source may be called from any thread. A plugin may run threads of its own.
- Every text is UTF-8 and ends with a zero.
- What the host gives for a call (an input, a name, a path) lives for the call; a source given to
  `open_bytes` lives until `close` of its view; a source of `open_file` or `slice` until
  `close_source`. What the plugin gives (its pictures, controls and texts) lives until its next
  call on the same view; its table, formats and files for the run.
- A structure with a `size` grows at its end only: each side reads the fields the size covers and
  takes the others as zero, and writes no field beyond the size the other side gave.
- The host refuses an `api_version` it does not know, a table smaller than its fixed part, and a
  table without `open`, `frame` or `close`; the page of the plugins names the reason.
- No exception, panic or `longjmp` crosses the interface: a plugin catches its own and says why in
  its status. A plugin runs with the rights of the program, in its process: a crash of it is a
  crash of the program, and the settings may admit only the plugins whose SHA-256 or signature
  they list.
- `sniff` gives one of the MIME types of the formats of the plugin, a text that lives for the run;
  another type is taken as none. The host asks every plugin in turn, before it takes a text as
  `text/plain`.
- The keys come by groups (`WW_KEYS_ARROWS`, `_PAGES`, `_LETTERS`, `_DIGITS`, `_SIGNS`, `_SPACE`,
  `_ESCAPE`, `_ENTER`) while the view has the focus, by their names of W3C (`ArrowLeft`,
  `PageUp`, `a`, `+`); the others stay the window's. Tab gives the focus back, and so does Escape
  unless the view takes it. The mouse: its place in the pixels of the view, the buttons held,
  clicked and clicked twice, the movement of a drag, the wheel in lines.
- A picture still at work sets `again`, and the host draws another frame at once; an idle view
  costs nothing.

The sizes on x86-64, for a binding of another language to check against: `ww_format` 40,
`ww_file` 24, `ww_source` 48, `ww_cache` 56, `ww_host` 40, `ww_key` 16, `ww_input` 88,
`ww_control` 64, `ww_output` 64, `ww_plugin` 96 bytes. The tests of `wwrt` and of `ww-stable`
check them.

## 4. Building a plugin

A plugin is an x86-64 DLL that exports `ww_plugin_get` and neither `ww_plugin` nor
`ww_plugin_abi` (the exports of a native plugin). Its C runtime is linked in or shipped next to
it: a clean Windows may lack the runtime of Visual C++. The DLL goes into the folder `plugins`
next to the executable of the host; `--plugins` of the demo lists it with the reason of a refusal.

### C

The example, `examples/ww-mandelbrot/mandelbrot.c`, opens a `.mandel` file through the cache of
the host, opens the same text as some bytes, finds its type in the first bytes of a file of any
name, embeds a page and a sample, and draws the Mandelbrot set in steps. In the root of the
repository:

```bat
rem The compiler of Visual Studio (x64, a Developer Command Prompt); /MT links the C runtime in.
cl /nologo /LD /O2 /MT /W3 /std:c11 /I stable examples\ww-mandelbrot\mandelbrot.c /Fe:ww_mandelbrot.dll

rem clang of LLVM for Windows, in its mode of cl.
clang-cl /LD /O2 /MT /I stable examples\ww-mandelbrot\mandelbrot.c /Fe:ww_mandelbrot.dll
```

```bash
# MinGW-w64: the runtime of gcc linked in.
gcc -shared -O2 -std=c11 -static-libgcc -I stable examples/ww-mandelbrot/mandelbrot.c -o ww_mandelbrot.dll
```

The skeleton of a plugin:

```c
#include "ww.h"

static const ww_host *HOST;

static const char *const EXTENSIONS[] = {"grey"};
static const char *const MIMES[] = {"application/x-grey"};
static const ww_format FORMATS[] = {{"Grey files", EXTENSIONS, 1, MIMES, 1}};

static const ww_plugin PLUGIN = {
    WW_API_VERSION, sizeof(ww_plugin), "Grey", FORMATS, 1, NULL, 0,
    my_open, my_open_bytes, my_sniff, my_frame, my_control, my_close};

WW_EXPORT const ww_plugin *ww_plugin_get(const ww_host *host) {
    HOST = host;
    return &PLUGIN;
}
```

### C++

The header declares everything inside `extern "C"`, so a definition of `ww_plugin_get` after the
`#include` keeps the linkage of C. Fill the table by position, or by name in C++20:

```bat
cl /nologo /LD /O2 /MT /EHsc /std:c++20 /I stable plugin.cpp /Fe:plugin.dll
```

Catch every exception inside the functions of the table: one that reaches the host stops the
program.

### Rust of any toolchain: the crate `ww-stable`

`stable/rust` is the crate `ww-stable` (edition 2021, Rust 1.77 or later, no dependency): a plugin
of the stable interface in Rust without a line of C, built by any toolchain. The example,
`examples/ww-grey`, shows any bytes as a grey picture, one pixel a byte, and reads only the rows
it shows through the cache of the host. A plugin of one's own copies the folder `stable/rust` next
to its crate (or names it by a path or by git):

```toml
[lib]
crate-type = ["cdylib"]

[dependencies]
ww-stable = { path = "../ww-stable" }
```

A type implements `ww_stable::View` (`frame` from an `Input` to a `Frame`: a `Picture`, the
`Control`s of the bar, the keys it takes, a `Cursor`, `again`, a line of the status bar;
`control`), and `ww_stable::plugin!(View, Plugin { .. })` names the plugin: its name, its
`Format`s, its `File`s, `open` (a path to a view), `open_bytes` (a `Source`, a name and a MIME type
to a view, or `None`) and `sniff` (the first bytes to a type, or `None`). `ww_stable::open_file`
reads a file through the cache of the host and `ww_stable::cache` tells its state.
`ww_stable::tools!(Tools { .. })` names its tools (section 2a): the name of the source, the
`Tool`s, the `ToolOption`s and the function that runs a tool (its name, its arguments and the
options as JSON texts to the answer, or the kind of the error with its message);
`ww_stable::call_tool` calls a tool of the program by its name. `examples/ww-grey` has one,
`grey_cache`, the state of the cache of the host. The crate
keeps the texts and the controls of a frame until the next call on the view, reads and writes
the structures by the sizes the host gives, and turns a panic into the line *the plugin stopped*
of its view. The documentation of the crate holds a whole plugin.

```bash
cargo build --release
```

`target/release/<name>.dll` goes into `plugins`. Build it outside a workspace whose
`.cargo/config.toml` links std dynamically (the native family of this repository does): the
plugin takes std in. For the toolchain msvc the line
`rustflags = ["-C", "target-feature=+crt-static"]` under `[target.x86_64-pc-windows-msvc]` of
the `.cargo/config.toml` of the plugin links the C runtime in, as `/MT` does for C; in this
repository `build-all.cmd` builds `examples/ww-grey` with `RUSTFLAGS` set so.

### Zig

`@cImport` reads the header, and an `export fn` gives the table:

```zig
const ww = @cImport(@cInclude("ww.h"));

var host: ?*const ww.ww_host = null;
const plugin = ww.ww_plugin{ .api_version = ww.WW_API_VERSION, .size = @sizeOf(ww.ww_plugin), ... };

export fn ww_plugin_get(h: ?*const ww.ww_host) ?*const ww.ww_plugin {
    host = h;
    return &plugin;
}
```

```bash
zig build-lib -dynamic -O ReleaseSafe -target x86_64-windows -I stable -lc plugin.zig
```

### Go

A DLL of Go (`go build -buildmode=c-shared`) carries the runtime of Go, which starts when the DLL
loads; the host never unloads a plugin, so it stays. The table points to functions of C: small
functions in the preamble of cgo call the exported functions of Go, and `ww_plugin_get` gives a
table made in C. Pictures, controls and texts the host reads are memory of C (`C.malloc`), kept
until the next call, never memory of Go.

```bash
CGO_ENABLED=1 GOOS=windows GOARCH=amd64 go build -buildmode=c-shared -o plugin.dll
```

### Free Pascal and Delphi

The records follow the layout of C on x86-64 (`{$PACKRECORDS C}` in Free Pascal, the default
alignment `{$A8}` in Delphi), the texts are `PAnsiChar` of UTF-8, the functions `cdecl` (on x86-64
there is one convention), and the library exports `ww_plugin_get`:

```pascal
function ww_plugin_get(host: PWwHost): PWwPlugin; cdecl;
begin
  WwHost := host;
  Result := @Plugin;
end;

exports ww_plugin_get;
```

### C# with Native AOT

.NET 8 or later publishes a native DLL: an `[UnmanagedCallersOnly(EntryPoint = "ww_plugin_get")]`
static method gives the table, a struct of `[StructLayout(LayoutKind.Sequential)]` with function
pointers (`delegate* unmanaged<...>`), kept in unmanaged memory (`NativeMemory.Alloc`):

```bash
dotnet publish -r win-x64 -c Release -p:PublishAot=true -p:NativeLib=Shared
```

The examples in C and in Rust are built and loaded by the tests and by CI; the recipes of the
other languages follow the FFI of each of them, and the sizes of section 3 check a binding.

## 5. The files of a plugin

`files` lists the files a plugin embeds, each with its path in the set (folders apart by `/`) and
its bytes, static data of the DLL:

```c
static const char GUIDE[] = "# Grey\n\nAny bytes as a grey picture.\n";
static const ww_file FILES[] = {
    {"guide.md", (const uint8_t *)GUIDE, sizeof GUIDE - 1},
    {"lang/uk/ui.json", (const uint8_t *)UK, sizeof UK - 1},
};
```

The page of the plugins links its pages (the `.md` files); a page shows in the language of the
interface when the set has `lang/<code>/<path>`, and its links lead to the other files of the set,
relative to the page, a file that is no page by the plugin of its type (`open_bytes` of a source
of the embedded bytes). `lang/<code>/ui.json`, a JSON object of the English texts and their
translations, translates the name of the plugin, of its formats and of its controls. docs/embedded.md
tells the same of the native plugins and the programs.

## 6. In the host

The loader of wwrt takes a DLL that exports `ww_plugin_get` as such a plugin, else a native one,
and wraps the table into an ordinary `Plugin` and `View` (`wwrt::stable`): the picture becomes a
texture, the controls widgets of egui in a strip over the view, the input of egui `ww_input` in
pixels, a source a `BlockFile` of the cache. `wwrt::stable::host()` is the table every plugin
gets.
