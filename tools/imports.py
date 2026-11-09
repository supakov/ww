"""The DLLs a Windows executable or DLL imports, read from its import table (and its delayed one).

    python tools/imports.py FILE...        one line a file: its name and the DLLs it imports
    python tools/imports.py --std FILE     the name of the std-<hash>.dll FILE imports

A program with plugins links std dynamically (docs/plugins.md): its archive takes along the
std-<hash>.dll its executable names (build-all.cmd, CI), and the list of what every file of the
archive imports shows which DLLs of the system it needs (vcruntime140.dll of msvc, say).
"""
import struct
import sys
from pathlib import Path


def imports(path: Path) -> list[str]:
    data = path.read_bytes()
    (pe,) = struct.unpack_from("<I", data, 0x3C)
    if data[pe:pe + 4] != b"PE\0\0":
        raise ValueError(f"{path} is not a PE file")
    sections, optional_size = struct.unpack_from("<H12xH", data, pe + 6)
    optional = pe + 24
    (magic,) = struct.unpack_from("<H", data, optional)
    directories = optional + (112 if magic == 0x20B else 96)
    table = optional + optional_size
    spans = []
    for i in range(sections):
        base = table + 40 * i
        size, address, raw_size, raw = struct.unpack_from("<IIII", data, base + 8)
        spans.append((address, max(size, raw_size), raw))

    def offset(rva: int) -> int:
        for address, size, raw in spans:
            if address <= rva < address + size:
                return rva - address + raw
        raise ValueError(f"{path}: the address {rva:#x} is in no section")

    def name(rva: int) -> str:
        start = offset(rva)
        return data[start:data.index(b"\0", start)].decode("ascii", "replace")

    out = []
    for index, entry in [(1, 20), (13, 32)]:
        rva, size = struct.unpack_from("<II", data, directories + 8 * index)
        if not rva or not size:
            continue
        at = offset(rva)
        while True:
            fields = struct.unpack_from("<" + "I" * (entry // 4), data, at)
            name_rva = fields[3] if index == 1 else fields[1]
            if not any(fields):
                break
            out.append(name(name_rva))
            at += entry
    return out


def main() -> int:
    args = sys.argv[1:]
    if args[:1] == ["--std"] and len(args) == 2:
        found = [d for d in imports(Path(args[1])) if d.lower().startswith("std-")]
        if not found:
            print(f"{args[1]} imports no std-<hash>.dll", file=sys.stderr)
            return 1
        print(found[0])
        return 0
    if not args:
        print(__doc__)
        return 2
    for file in args:
        print(f"{Path(file).name}: {', '.join(imports(Path(file)))}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
