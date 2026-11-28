#!/usr/bin/env python3
"""Compare two folders of PNG exports, pixel by pixel.

    compare-png.py <reference> <other> [--label TEXT]

Prints a Markdown table (differences in 8-bit levels). Exits with 1 when a file is missing, a size
differs, the mean difference is above 1 level or more than 0.1 % of the values differ by more than 4.
"""
import struct
import sys
import zlib
from pathlib import Path

MEAN_LIMIT = 1.0
FAR_LEVELS, FAR_SHARE = 4.0, 0.001


def read_png(path):
    """(width, height, channels, values in 8-bit levels) of a non-interlaced 8 or 16 bit PNG."""
    data = path.read_bytes()
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise ValueError(f"{path}: not a PNG")
    pos, idat = 8, bytearray()
    while pos < len(data):
        size, kind = struct.unpack(">I4s", data[pos : pos + 8])
        body = data[pos + 8 : pos + 8 + size]
        if kind == b"IHDR":
            w, h, depth, colour, _, _, interlace = struct.unpack(">IIBBBBB", body)
        elif kind == b"IDAT":
            idat += body
        pos += 12 + size
    channels = {0: 1, 2: 3, 4: 2, 6: 4}[colour]
    if depth not in (8, 16) or interlace:
        raise ValueError(f"{path}: unsupported PNG (depth {depth}, interlace {interlace})")
    raw, bpp = zlib.decompress(bytes(idat)), channels * depth // 8
    stride, prev, pixels = w * bpp, bytearray(w * bpp), bytearray()
    for j in range(h):
        start = j * (stride + 1)
        kind, line = raw[start], bytearray(raw[start + 1 : start + 1 + stride])
        for i in range(stride):
            a = line[i - bpp] if i >= bpp else 0
            b, c = prev[i], prev[i - bpp] if i >= bpp else 0
            if kind == 1:
                p = a
            elif kind == 2:
                p = b
            elif kind == 3:
                p = (a + b) // 2
            elif kind == 4:
                q = a + b - c
                pa, pb, pc = abs(q - a), abs(q - b), abs(q - c)
                p = a if pa <= pb and pa <= pc else b if pb <= pc else c
            else:
                p = 0
            line[i] = (line[i] + p) & 255
        pixels += line
        prev = line
    if depth == 8:
        return w, h, channels, list(pixels)
    return w, h, channels, [v / 257 for v in struct.unpack(f">{len(pixels) // 2}H", bytes(pixels))]


def main():
    ref, other = Path(sys.argv[1]), Path(sys.argv[2])
    label = sys.argv[4] if len(sys.argv) > 4 and sys.argv[3] == "--label" else f"{ref.name} / {other.name}"
    files = sorted(ref.glob("*.png"))
    print(f"### {label}\n\n| Scan | Size | Mean | Max | > {FAR_LEVELS:g} levels | Result |\n|---|---|---|---|---|---|")
    failed = not files
    for f in files:
        g = other / f.name
        if not g.is_file():
            print(f"| {f.stem} | missing | | | | **fail** |")
            failed = True
            continue
        a, b = read_png(f), read_png(g)
        if a[:3] != b[:3]:
            print(f"| {f.stem} | {a[0]}x{a[1]} / {b[0]}x{b[1]} | | | | **fail** |")
            failed = True
            continue
        d = [abs(x - y) for x, y in zip(a[3], b[3])]
        mean, top = sum(d) / len(d), max(d)
        far = sum(v > FAR_LEVELS for v in d) / len(d)
        ok = mean <= MEAN_LIMIT and far <= FAR_SHARE
        failed |= not ok
        print(f"| {f.stem} | {a[0]}x{a[1]} | {mean:.3f} | {top:.0f} | {far:.3%} | {'ok' if ok else '**fail**'} |")
    print()
    sys.exit(1 if failed else 0)


main()
