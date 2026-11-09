# The stable interface of the plugins of W#

A plugin in any language with a C FFI is a DLL that exports one function, `ww_plugin_get`,
declared by `ww.h` (version 1). It loads into every program on W# that knows that version,
whatever compiler built either, and is built once: it draws a picture and names its controls,
the host draws them and gives it the keys and the mouse of its view, and it reads files and the
bytes other views offer through the cache of the blocks of the host.

| File | What |
|---|---|
| `ww.h` | the header: the structures, the constants and the rules |
| `rust/` | the crate `ww-stable`: a plugin of the interface in Rust of any toolchain |

The interface, its rules and how to build a plugin in C, C++, Rust, Zig, Go, Free Pascal and
Delphi or C# are in `docs/stable.md`; the examples are `examples/ww-mandelbrot` (C) and
`examples/ww-grey` (Rust with `ww-stable`).

```bat
cl /nologo /LD /O2 /MT /W3 /std:c11 /I stable examples\ww-mandelbrot\mandelbrot.c /Fe:ww_mandelbrot.dll
```

The DLL goes into the folder `plugins` next to the executable of the host; `ww-demo --plugins`
lists it, and `ww-demo --probe examples\ww-mandelbrot\seahorse.mandel` draws it without a window.
