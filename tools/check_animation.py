#!/usr/bin/env python3
"""Independent generated GIF/APNG animation checks for ChezCat.

Development only. Pillow, ImageMagick and FFmpeg supply independent decoding;
original generated streams additionally check format semantics and malformed
framing. No image library is linked into or invoked by the browser.
"""
from __future__ import annotations

import argparse
import hashlib
import io
import json
from pathlib import Path
import random
import shutil
import struct
import subprocess
import tempfile
import zlib

from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
PROBE = r'''
#![allow(dead_code)]
#[path = IMAGE_SOURCE] mod image;
#[path = COMPRESSION_SOURCE] mod compression;
#[path = JPEG_SOURCE] mod jpeg;
use std::io::{self, Write};
use std::cell::Cell;
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&args[1]).unwrap();
    let count = Cell::new(0usize);
    let stop = args.get(2).and_then(|s| s.parse::<usize>().ok()).unwrap_or(usize::MAX);
    let check = || {
        count.set(count.get() + 1);
        if count.get() >= stop { Err("cancelled by animation probe".to_string()) } else { Ok(()) }
    };
    let result = image::decode_asset_checked(&data, &check);
    let mut out = io::BufWriter::new(io::stdout().lock());
    match result {
        Err(error) => writeln!(out, "ERROR {}", error.replace('\n', " ")).unwrap(),
        Ok(image) => {
            let (plays, frames) = match &image.animation {
                Some(a) => (a.plays.map_or(-1, i64::from), a.frames.as_slice()),
                None => (0, &[][..]),
            };
            let b = &image.bitmap;
            writeln!(out, "OK {} {} {} {} {} {}", b.width, b.height, plays, frames.len(), usize::from(image.notice.is_some()), image.retained_bytes().unwrap()).unwrap();
            for p in &b.pixels { out.write_all(&p.to_le_bytes()).unwrap(); }
            for frame in frames {
                out.write_all(&frame.delay_ms.to_le_bytes()).unwrap();
                for p in &frame.bitmap.pixels { out.write_all(&p.to_le_bytes()).unwrap(); }
            }
        }
    }
}
'''

PALETTE = [(0, 0, 0), (227, 47, 71), (32, 186, 156), (245, 197, 91)]
ADAM7 = [(0, 0, 8, 8), (4, 0, 8, 8), (0, 4, 4, 8), (2, 0, 4, 4),
         (0, 2, 2, 4), (1, 0, 2, 2), (0, 1, 1, 2)]


def gif_frame(width, height, indices, *, left=0, top=0, disposal=1,
              transparent=None, delay=70, local=None, interlaced=False):
    assert len(indices) == width * height
    flags = disposal << 2 | (transparent is not None)
    out = bytearray(b"\x21\xf9\x04" + bytes([flags]) + struct.pack("<H", delay // 10)
                    + bytes([transparent or 0, 0]))
    out += b"\x2c" + struct.pack("<4H", left, top, width, height)
    out += bytes([(0x81 if local else 0) | (0x40 if interlaced else 0)])
    if local:
        out += bytes(channel for color in local for channel in color)
    if interlaced:
        indices = [indices[row * width + col]
                   for start, step in [(0, 8), (4, 8), (2, 4), (1, 2)]
                   for row in range(start, height, step) for col in range(width)]
    # Clear before each literal: an intentionally simple, original GIF encoder.
    codes = [item for index in indices for item in (4, index)] + [5]
    packed = bytearray(); buffer = bits = 0
    for code in codes:
        buffer |= code << bits; bits += 3
        while bits >= 8:
            packed.append(buffer & 255); buffer >>= 8; bits -= 8
    if bits:
        packed.append(buffer)
    out.append(2)
    for start in range(0, len(packed), 255):
        block = packed[start:start + 255]
        out += bytes([len(block)]) + block
    out.append(0)
    return out


def gif(width, height, frames, *, loops=0, palette=PALETTE, background=0):
    out = bytearray(b"GIF89a" + struct.pack("<HHBBB", width, height, 0x81, background, 0))
    out += bytes(channel for color in palette for channel in color)
    if loops is not None:
        out += b"\x21\xff\x0bNETSCAPE2.0\x03\x01" + struct.pack("<H", loops) + b"\x00"
    for frame in frames:
        out += frame
    return bytes(out + b"\x3b")


def chunk(kind, body):
    return struct.pack(">I", len(body)) + kind + body + struct.pack(">I", zlib.crc32(kind + body))


def rgba_data(width, height, pixels):
    return zlib.compress(b"".join(b"\0" + bytes(channel for pixel in pixels[y * width:(y + 1) * width] for channel in pixel)
                                 for y in range(height)))


def sample_data(width, height, samples, depth, interlaced):
    raw = bytearray()
    for x, y, dx, dy in ADAM7 if interlaced else [(0, 0, 1, 1)]:
        if x >= width or y >= height:
            continue
        for row in range(y, height, dy):
            values = [v for col in range(x, width, dx) for v in samples[row * width + col]]
            raw.append(0)
            if depth == 16:
                raw += struct.pack(">" + "H" * len(values), *values)
            elif depth == 8:
                raw += bytes(values)
            else:
                packed = bytearray((len(values) * depth + 7) // 8)
                for i, value in enumerate(values):
                    packed[i * depth // 8] |= value << (8 - depth - i * depth % 8)
                raw += packed
    return zlib.compress(raw)


def apng(width, height, frames, *, poster=None, plays=0, split=0, ancillary=False):
    # Frames: (w, h, x, y, RGBA pixels, numerator, denominator, disposal, blend).
    out = bytearray(b"\x89PNG\r\n\x1a\n")
    out += chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
    out += chunk(b"acTL", struct.pack(">II", len(frames), plays))
    sequence = 0
    if poster is not None:
        out += chunk(b"IDAT", rgba_data(width, height, poster))
    for i, (w, h, x, y, pixels, num, den, disposal, blend) in enumerate(frames):
        out += chunk(b"fcTL", struct.pack(">IIIIIHHBB", sequence, w, h, x, y, num, den, disposal, blend))
        sequence += 1
        compressed = rgba_data(w, h, pixels)
        pieces = [compressed]
        if split:
            pieces = [b""] + [compressed[n:n + split] for n in range(0, len(compressed), split)] + [b""]
        for part in pieces:
            if i == 0 and poster is None:
                out += chunk(b"IDAT", part)
            else:
                out += chunk(b"fdAT", struct.pack(">I", sequence) + part)
                sequence += 1
                if ancillary:
                    out += chunk(b"tEXt", b"Note\0Original animation fixture")
    return bytes(out + chunk(b"IEND", b""))


def rgba(raw):
    return [(r, g, b, a) for b, g, r, a in struct.iter_unpack("4B", raw)]


def canonical(pixels):
    return [(r, g, b, a) if a else (0, 0, 0, 0) for r, g, b, a in pixels]


def pillow_frames(data):
    with Image.open(io.BytesIO(data)) as image:
        frames = []
        for i in range(image.n_frames):
            image.seek(i)
            converted = image.convert("RGBA")
            frames.append(list(converted.get_flattened_data() if hasattr(converted, "get_flattened_data") else converted.getdata()))
        return frames, bool(image.info.get("default_image")), image.info


def chunks(data):
    position = 8
    result = []
    while position < len(data):
        length, = struct.unpack_from(">I", data, position)
        result.append((data[position + 4:position + 8], data[position + 8:position + 8 + length]))
        position += length + 12
    return result


def rebuild(parts):
    return b"\x89PNG\r\n\x1a\n" + b"".join(chunk(kind, data) for kind, data in parts)


def demo_assets():
    """Original geometric motion, with no external artwork or site content."""
    width, height = 160, 96
    palette = [(20, 39, 53), (143, 226, 210), (232, 181, 97), (240, 247, 247)]
    background = [0] * (width * height)
    for y in range(14, 82):
        for x in range(14, 146):
            if x in (14, 145) or y in (14, 81) or (y == 68 and x % 8 < 4):
                background[y * width + x] = 1
    frames = [gif_frame(width, height, background, disposal=1, delay=100)]
    for x in [20, 32, 44, 56, 68, 80, 92, 104, 116, 104, 92, 80, 68, 56, 44, 32]:
        patch = [0] * (24 * 24)
        for y in range(24):
            for col in range(24):
                if abs(y - 11) + abs(col - 11) <= 10:
                    patch[y * 24 + col] = 2 if col < 12 else 3
        frames.append(gif_frame(24, 24, patch, left=x, top=32, transparent=0,
                                disposal=3, delay=100))
    gif_data = gif(width, height, frames, palette=palette, background=0)
    png_background = [(*palette[index], 255) for index in background]
    png_frames = [(width, height, 0, 0, png_background, 1, 10, 0, 0)]
    for x in [20, 32, 44, 56, 68, 80, 92, 104, 116, 104, 92, 80, 68, 56, 44, 32]:
        patch = []
        for y in range(24):
            for col in range(24):
                distance = (col - 11.5) ** 2 + (y - 11.5) ** 2
                if distance <= 121:
                    alpha = 224 if distance < 70 else 104
                    patch.append((*palette[2 if col < 12 else 1], alpha))
                else:
                    patch.append((0, 0, 0, 0))
        png_frames.append((24, 24, x, 32, patch, 1, 10, 2, 1))
    return {"format-animated.gif": gif_data,
            "format-animated.apng": apng(width, height, png_frames)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rustc", default="rustc")
    parser.add_argument("--output", type=Path, default=Path("animation-report.json"))
    parser.add_argument("--artifacts", type=Path)
    parser.add_argument("--magick", default=shutil.which("magick"))
    parser.add_argument("--ffmpeg", default=shutil.which("ffmpeg"))
    parser.add_argument("--write-demo-assets", type=Path, help="Also regenerate the two original moving geometric gallery assets")
    args = parser.parse_args()
    temporary = tempfile.TemporaryDirectory(prefix="chezcat-animation-")
    folder = args.artifacts or Path(temporary.name)
    folder.mkdir(parents=True, exist_ok=True)
    source = PROBE.replace("IMAGE_SOURCE", json.dumps(str(ROOT / "src/image.rs")))
    source = source.replace("COMPRESSION_SOURCE", json.dumps(str(ROOT / "src/compression.rs")))
    source = source.replace("JPEG_SOURCE", json.dumps(str(ROOT / "src/jpeg.rs")))
    (folder / "probe.rs").write_text(source)
    compiled = subprocess.run([args.rustc, "--edition=2021", "-O", str(folder / "probe.rs"), "-o", str(folder / "probe")], capture_output=True, text=True)
    if compiled.returncode:
        raise RuntimeError(compiled.stderr)
    checks = []; rejections = []; mutations = 0

    def run(name, data, cancel=None):
        path = folder / name
        path.write_bytes(data)
        command = [str(folder / "probe"), str(path)]
        if cancel is not None:
            command.append(str(cancel))
        process = subprocess.run(command, capture_output=True, timeout=20)
        assert process.returncode == 0, (name, process.returncode, process.stderr.decode(errors="replace"))
        line, _, body = process.stdout.partition(b"\n")
        if line.startswith(b"ERROR "):
            return line.decode(errors="replace")
        fields = line.decode().split()
        assert fields[0] == "OK", (name, line)
        width, height, plays, count, notice, retained = map(int, fields[1:])
        size = width * height * 4
        poster = rgba(body[:size]); at = size; frames = []
        for _ in range(count):
            delay, = struct.unpack_from("<I", body, at); at += 4
            frames.append((delay, rgba(body[at:at + size]))); at += size
        assert at == len(body), (name, at, len(body))
        return {"width": width, "height": height, "plays": plays, "frames": frames, "poster": poster, "notice": notice, "retained": retained}

    def compare(name, data, expected_plays, *, tolerance=0, reference="Pillow", reference_data=None):
        actual = run(name, data)
        assert isinstance(actual, dict), (name, actual)
        expected, separate, _ = pillow_frames(reference_data if reference_data is not None else data)
        assert canonical(actual["poster"]) == canonical(expected[0]), (name, "poster")
        if separate:
            expected = expected[1:]
        if reference == "ImageMagick":
            if not args.magick:
                raise RuntimeError("This animation check requires ImageMagick (magick) as a second independent oracle.")
            # Pillow retains the first frame for GIF disposal=previous, while
            # GIF89a requires restoring what preceded it. ImageMagick agrees
            # with the spec for these generated first-frame disposal cases.
            output = subprocess.check_output([args.magick, str(folder / name), "-coalesce", "-depth", "8", "rgba:-"], timeout=20)
            size = actual["width"] * actual["height"] * 4
            assert len(output) == size * len(expected)
            expected = [list(struct.iter_unpack("4B", output[at:at + size])) for at in range(0, len(output), size)]
        elif reference == "FFmpeg":
            if not args.ffmpeg:
                raise RuntimeError("This animation check requires FFmpeg as an independent APNG oracle.")
            # Cross-check APNG with a second decoder: Pillow's OVER+PREVIOUS
            # handling leaves pixels from the disposed frame in these cases.
            output = subprocess.check_output([args.ffmpeg, "-hide_banner", "-loglevel", "error", "-ignore_loop", "1", "-i", str(folder / name),
                                              "-fps_mode", "passthrough", "-f", "rawvideo", "-pix_fmt", "rgba", "-"], timeout=20)
            size = actual["width"] * actual["height"] * 4
            assert len(output) == size * len(expected), (name, len(output), size * len(expected))
            expected = [list(struct.iter_unpack("4B", output[at:at + size])) for at in range(0, len(output), size)]
        assert len(actual["frames"]) == len(expected), (name, len(actual["frames"]), len(expected))
        worst = 0
        for (_, frame), reference in zip(actual["frames"], expected):
            a, b = canonical(frame), canonical(reference)
            error = max(abs(c - d) for p, q in zip(a, b) for c, d in zip(p, q))
            worst = max(worst, error)
            assert error <= tolerance, (name, "frame", error, next((i, p, q) for i, (p, q) in enumerate(zip(a, b)) if max(abs(c - d) for c, d in zip(p, q)) > tolerance))
        assert actual["plays"] == expected_plays and not actual["notice"], (name, actual["plays"], expected_plays)
        checks.append({"name": name, "frames": len(expected), "reference": reference, "maximum_channel_difference": worst})
        return actual

    # GIF streams vary disposal, offsets, transparency, local tables, interlace,
    # and finite/indefinite/no-loop metadata. Palette entries are our own colors.
    for disposal in range(4):
        for transparent in [None, 0]:
            for loops in [None, 0, 2]:
                frames = [gif_frame(4, 3, [1, 2, 3, 1] * 3, transparent=transparent, disposal=1, delay=70),
                          gif_frame(2, 2, [0, 2, 3, 0], left=1, top=1, transparent=transparent, disposal=disposal, delay=110),
                          gif_frame(1, 3, [2, 3, 1], left=0, transparent=transparent, disposal=1, delay=30)]
                compare(f"gif-d{disposal}-t{transparent}-l{loops}.gif", gif(4, 3, frames, loops=loops), -1 if loops == 0 else (loops + 1 if loops else 1))
    for height in [1, 2, 3, 5, 9, 17]:
        local = [(17, 41, 71), (222, 199, 51), (111, 31, 219), (81, 169, 206)]
        frames = [gif_frame(3, height, [i % 4 for i in range(3 * height)], interlaced=True, disposal=3),
                  gif_frame(3, height, [(i + 1) % 4 for i in range(3 * height)], interlaced=True, local=local)]
        compare(f"gif-interlace-{height}.gif", gif(3, height, frames, background=2), -1)
    # Independent encoder creates variable-width dictionary/code streams.
    for seed in range(12):
        rng = random.Random(seed)
        frames = []
        for i in range(4):
            image = Image.new("P", (31, 23)); image.putpalette([v for c in PALETTE for v in c] + [0] * (768 - 12))
            image.putdata([rng.randrange(4) for _ in range(31 * 23)])
            frames.append(image)
        out = io.BytesIO()
        frames[0].save(out, "GIF", save_all=True, append_images=frames[1:], duration=[40, 70, 100, 130], loop=seed % 3, disposal=(seed % 3) + 1, optimize=bool(seed & 1))
        compare(f"gif-pillow-{seed}.gif", out.getvalue(), -1 if seed % 3 == 0 else seed % 3 + 1,
                reference="ImageMagick" if seed % 3 == 2 else "Pillow")

    red = (227, 47, 71, 255); teal = (32, 186, 156, 255); gold = (245, 197, 91, 255); clear = (0, 0, 0, 0)
    for disposal in range(3):
        for blend in range(2):
            for separate in [False, True]:
                frames = [(4, 3, 0, 0, [red, teal, gold, clear] * 3, 7, 100, 0, 0),
                          (2, 2, 1, 1, [teal, clear, gold, red], 11, 100, disposal, blend),
                          (1, 3, 0, 0, [gold, red, teal], 3, 100, 0, 0)]
                data = apng(4, 3, frames, poster=[gold] * 12 if separate else None, plays=2, split=7, ancillary=True)
                # Identical rasters in a conventional chunk arrangement: Pillow
                # rejects legal empty fdAT pieces in the stress arrangement.
                reference = apng(4, 3, frames, poster=[gold] * 12 if separate else None, plays=2)
                compare(f"apng-d{disposal}-b{blend}-p{separate}.png", data, 2, reference_data=reference,
                        reference="FFmpeg" if disposal == 2 and blend == 1 else "Pillow")
    # Correct alpha-over compositing is compared with an independent encoder and
    # decoder on opaque destination pixels; transparent-destination arithmetic
    # has dedicated analytical tests in the Rust module.
    for seed in range(12):
        rng = random.Random(seed + 123)
        first = Image.new("RGBA", (13, 9), red)
        second = Image.new("RGBA", (13, 9)); second.putdata([(rng.randrange(256), rng.randrange(256), rng.randrange(256), 255) for _ in range(117)])
        third = Image.new("RGBA", (13, 9), teal)
        out = io.BytesIO(); first.save(out, "PNG", save_all=True, append_images=[second, third], duration=[20, 80, 120], loop=seed % 3, disposal=[0, seed % 3, 0], blend=[0, seed % 2, 0])
        compare(f"apng-pillow-{seed}.png", out.getvalue(), -1 if seed % 3 == 0 else seed % 3)

    # Animation frame rasters inherit the PNG color/depth/palette/Adam7 header.
    # Compare them to original sample values, including narrow interlaced
    # patches with empty passes, packed indices and full-depth transparency.
    sample_cases = 0
    for color, depths in [(0, [1, 2, 4, 8, 16]), (2, [8, 16]), (3, [1, 2, 4, 8]), (4, [8, 16]), (6, [8, 16])]:
        for depth in depths:
            for interlaced in [False, True]:
                maximum = (1 << depth) - 1
                channels = {0: 1, 2: 3, 3: 1, 4: 2, 6: 4}[color]
                palette = [(i * 53 % 256, i * 137 % 256, i * 29 % 256, 90 if i % 3 == 0 else 255)
                           for i in range(1 << depth)] if color == 3 else None
                samples = [tuple((i * 37 + c * 101) % (maximum + 1) for c in range(channels)) for i in range(13 * 9)]
                patch = [tuple((i * 17 + c * 79 + 11) % (maximum + 1) for c in range(channels)) for i in range(3)]
                transparent = samples[3] if color in [0, 2] else None
                def converted(values):
                    if color == 3:
                        return palette[values[0]]
                    scaled = [(n * 255 + maximum // 2) // maximum for n in values]
                    if color in [0, 4]:
                        return (scaled[0],) * 3 + (scaled[1] if color == 4 else 0 if values == transparent else 255,)
                    return tuple(scaled[:3]) + (scaled[3] if color == 6 else 0 if values == transparent else 255,)
                out = bytearray(b"\x89PNG\r\n\x1a\n")
                out += chunk(b"IHDR", struct.pack(">IIBBBBB", 13, 9, depth, color, 0, 0, int(interlaced)))
                if palette:
                    out += chunk(b"PLTE", bytes(v for p in palette for v in p[:3]))
                    out += chunk(b"tRNS", bytes(p[3] for p in palette))
                elif transparent is not None:
                    out += chunk(b"tRNS", struct.pack(">" + "H" * channels, *transparent))
                out += chunk(b"acTL", struct.pack(">II", 2, 1))
                out += chunk(b"fcTL", struct.pack(">IIIIIHHBB", 0, 13, 9, 0, 0, 7, 100, 0, 0))
                out += chunk(b"IDAT", sample_data(13, 9, samples, depth, interlaced))
                out += chunk(b"fcTL", struct.pack(">IIIIIHHBB", 1, 1, 3, 6, 2, 11, 100, 0, 0))
                out += chunk(b"fdAT", struct.pack(">I", 2) + sample_data(1, 3, patch, depth, interlaced))
                out += chunk(b"IEND", b"")
                name = f"apng-samples-c{color}-d{depth}-i{int(interlaced)}.png"
                actual = run(name, out)
                assert isinstance(actual, dict), (name, actual)
                expected = list(map(converted, samples))
                assert canonical(actual["poster"]) == canonical(expected), (name, "poster")
                assert canonical(actual["frames"][0][1]) == canonical(expected), (name, "frame zero")
                for y, pixel in enumerate(map(converted, patch)):
                    expected[(2 + y) * 13 + 6] = pixel
                assert canonical(actual["frames"][1][1]) == canonical(expected), (name, "frame patch")
                assert actual["plays"] == 1 and [f[0] for f in actual["frames"]] == [70, 110]
                sample_cases += 1

    base_frames = [(4, 3, 0, 0, [red] * 12, 0, 0, 2, 1), (1, 1, 2, 1, [teal], 1, 3, 0, 1)]
    base = apng(4, 3, base_frames)
    actual = compare("apng-fraction-first-previous.png", base, -1)
    assert [delay for delay, _ in actual["frames"]] == [0, 333]
    parts = chunks(base)
    bad = []
    for index, (kind, body) in enumerate(parts):
        if kind in [b"fcTL", b"fdAT"]:
            changed = list(parts); changed[index] = (kind, struct.pack(">I", 99) + body[4:]); bad.append((f"sequence-{index}", rebuild(changed)))
        if kind == b"fcTL":
            for offset, replacement in [(4, struct.pack(">I", 0)), (12, struct.pack(">I", 0xffffffff)), (24, b"\x03"), (25, b"\x02")]:
                changed = list(parts); changed[index] = (kind, body[:offset] + replacement + body[offset + len(replacement):]); bad.append((f"control-{index}-{offset}", rebuild(changed)))
    bad += [("missing-actl", rebuild([p for p in parts if p[0] != b"acTL"])),
            ("duplicate-actl", rebuild(parts[:2] + [parts[1]] + parts[2:])),
            ("wrong-count", rebuild([(k, struct.pack(">II", 3, 0) if k == b"acTL" else b) for k, b in parts])),
            ("zero-count", rebuild([(k, struct.pack(">II", 0, 0) if k == b"acTL" else b) for k, b in parts])),
            ("too-many", rebuild([(k, struct.pack(">II", 257, 0) if k == b"acTL" else b) for k, b in parts])),
            ("missing-frame-data", rebuild([p for p in parts if p[0] != b"fdAT"]))]
    for name, data in bad:
        result = run("bad-" + name + ".png", data)
        assert isinstance(result, str), (name, "accepted invalid animation")
        rejections.append({"name": name, "error": result})

    for label, data in [("gif", gif(4, 3, [gif_frame(4, 3, [1] * 12), gif_frame(4, 3, [2] * 12)])), ("apng", base)]:
        cancellation_data = (gif(40, 40, [gif_frame(40, 40, [1] * 1600), gif_frame(40, 40, [2] * 1600)])
                             if label == "gif" else data)
        for count in [1, 2, 4, 7]:
            result = run(f"cancel-{label}-{count}", cancellation_data, count)
            assert isinstance(result, str) and "cancelled" in result, (label, count, result)
            rejections.append({"name": f"cancel-{label}-{count}", "error": result})
        rng = random.Random(8128)
        for i in range(160):
            mutated = bytearray(data)
            if i < 60:
                mutated = mutated[:rng.randrange(len(mutated))]
            else:
                for _ in range(1 + i % 3):
                    at = rng.randrange(len(mutated)); mutated[at] ^= 1 << rng.randrange(8)
            run(f"mutation-{label}.bin", mutated)
            mutations += 1

    # Sparse patches on a large canvas cross the retained-frame limit while
    # keeping source and decode work small. Later damaged rasters must still fail.
    large_gif = gif(1024, 1024, [gif_frame(1, 1, [i % 3 + 1], left=i, top=i) for i in range(17)])
    fallback = run("gif-poster-fallback.gif", large_gif)
    assert isinstance(fallback, dict) and fallback["notice"] and not fallback["frames"]
    assert fallback["retained"] == 1024 * 1024 * 4
    broken_gif_frame = gif_frame(1, 1, [1], left=16, top=16)
    broken_gif_frame[-3] = 0xff
    invalid_gif = gif(1024, 1024, [gif_frame(1, 1, [i % 3 + 1], left=i, top=i) for i in range(16)] + [broken_gif_frame])
    assert isinstance(run("gif-fallback-invalid-last.gif", invalid_gif), str)
    default = [clear] * (1024 * 1024)
    large_png = apng(1024, 1024, [(1, 1, i, i, [red], 7, 100, 0, 0) for i in range(16)], poster=default)
    fallback = run("apng-poster-fallback.png", large_png)
    assert isinstance(fallback, dict) and fallback["notice"] and not fallback["frames"]
    assert fallback["retained"] == 1024 * 1024 * 4
    # Recompute the final frame CRC after destroying its compressed bytes.
    large_parts = chunks(large_png)
    last_data = max(i for i, part in enumerate(large_parts) if part[0] == b"fdAT")
    old = large_parts[last_data][1]
    large_parts[last_data] = (b"fdAT", old[:4] + b"broken")
    assert isinstance(run("apng-fallback-invalid-last.png", rebuild(large_parts)), str)

    demos = demo_assets()
    for name, data in demos.items():
        actual = compare(name, data, -1, reference="Pillow" if name.endswith(".gif") else "FFmpeg",
                         tolerance=0 if name.endswith(".gif") else 1)
        assert isinstance(actual, dict) and len(actual["frames"]) == 17 and actual["plays"] == -1
        if args.write_demo_assets:
            args.write_demo_assets.mkdir(parents=True, exist_ok=True)
            (args.write_demo_assets / name).write_bytes(data)

    report = {"pillow": Image.__version__, "comparison_cases": len(checks),
              "frames_compared": sum(item["frames"] for item in checks),
              "maximum_channel_difference": max(item["maximum_channel_difference"] for item in checks),
              "rejection_checks": len(rejections), "malformed_mutations": mutations,
              "inherited_sample_cases": sample_cases,
              "poster_fallback_cases": 2,
              "malformed_after_fallback_checks": 2,
              "demo_sha256": {name: hashlib.sha256(data).hexdigest() for name, data in demos.items()},
              "gif_source_sha256": hashlib.sha256((ROOT / "src/gif.rs").read_bytes()).hexdigest(),
              "png_source_sha256": hashlib.sha256((ROOT / "src/image_png.rs").read_bytes()).hexdigest(),
              "comparisons": checks, "rejections": rejections}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({k: v for k, v in report.items() if k not in ("comparisons", "rejections")}, indent=2))


if __name__ == "__main__":
    main()
