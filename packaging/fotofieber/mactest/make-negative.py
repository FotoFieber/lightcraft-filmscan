#!/usr/bin/env python3
"""Write synthetic film negative scans (PNG) for the macOS test: no private photos needed.

    make-negative.py <folder>

Each scan is one 35 mm frame in a holder, as in crates/engine/src/tests_negative.rs: black holder,
orange film base, sprocket holes along the long edges and a denser, textured picture on
120..780 x 80..520 of the straight 900 x 600 scan, put in askew by a few degrees.
"""
import math
import struct
import sys
import zlib
from pathlib import Path

W, H = 900, 600
BASE = (0.62, 0.26, 0.07)
SCANS = {"neg-straight": 0.0, "neg-askew-right": 1.5, "neg-askew-left": -1.0}


def srgb8(v):
    v = min(max(v, 0.0), 1.0)
    v = v * 12.92 if v <= 0.0031308 else 1.055 * v ** (1 / 2.4) - 0.055
    return int(v * 255 + 0.5)


def pixel(x, y):
    if not (60 <= x < 840 and 40 <= y < 560):
        return (0.004, 0.003, 0.003)
    for cy in (38.0, 542.0):
        if ((x - 40) % 56 - 28) ** 2 / 100 + (y - cy) ** 2 / 196 < 1:
            return (0.92, 0.93, 0.97)
    if not (120 <= x < 780 and 80 <= y < 520):
        return BASE
    t = 0.2 + 0.25 * abs(math.sin(x / 37) * math.cos(y / 23))
    if (int(x / 90) + int(y / 70)) % 2 == 0:
        t += 0.2
    return tuple(c * t for c in BASE)


def chunk(kind, data):
    return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))


def scan(angle):
    s, c = math.sin(math.radians(angle)), math.cos(math.radians(angle))
    rows = bytearray()
    for j in range(H):
        rows.append(0)  # PNG filter: none
        for i in range(W):
            dx, dy = i + 0.5 - W / 2, j + 0.5 - H / 2
            rows.extend(map(srgb8, pixel(W / 2 + c * dx - s * dy, H / 2 + s * dx + c * dy)))
    header = struct.pack(">IIBBBBB", W, H, 8, 2, 0, 0, 0)
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", header) + chunk(b"IDAT", zlib.compress(bytes(rows))) + chunk(b"IEND", b"")


out = Path(sys.argv[1])
out.mkdir(parents=True, exist_ok=True)
for name, angle in SCANS.items():
    (out / f"{name}.png").write_bytes(scan(angle))
    print(out / f"{name}.png")
