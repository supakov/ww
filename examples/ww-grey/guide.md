# Bytes as a picture

A plugin of the stable interface of W# written in Rust with the crate `ww-stable` and built by
any toolchain. It shows any bytes as a grey picture: one pixel a byte, a row of the width of the
bar, so the parts of a file (a header, a table, text, compressed data) look apart.

It reads only the rows it shows, through the cache of the blocks of the host: a file of any size,
or the bytes another view offers to Open with (a part of a file, never copied).

- The wheel, the arrows and Page Up and Page Down scroll; Home and End go to the start and the
  end; `+` and `-` double or halve the width of a row.
- The bar has the width of a row, the zoom and Top.
- The status bar names the offset of the first row shown and the state of the cache.
