"""Losslessly encode ChezCat's P6 renderer output as PNG for inspection.

The original PPM stays unchanged. Only the file encoding changes, never pixels.
"""
import argparse
from pathlib import Path
import struct
import zlib


def convert(path):
    with path.open("rb") as stream:
        if stream.readline() != b"P6\n":
            raise ValueError("Expected ChezCat P6 renderer output")
        width, height = map(int, stream.readline().split())
        if stream.readline() != b"255\n" or not 1 <= width <= 8192 or not 1 <= height <= 8192:
            raise ValueError("Unsupported PPM dimensions/range")
        pixels = stream.read(width * height * 3 + 1)
    if len(pixels) != width * height * 3:
        raise ValueError("PPM pixel length mismatch")

    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))

    rows = b"".join(b"\0" + pixels[y * width * 3:(y + 1) * width * 3] for y in range(height))
    result = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0))
    result += chunk(b"IDAT", zlib.compress(rows, 6)) + chunk(b"IEND", b"")
    output = path.with_suffix(".png")
    output.write_bytes(result)
    print(output)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("paths", nargs="+", type=Path)
    for selected in parser.parse_args().paths:
        convert(selected)
