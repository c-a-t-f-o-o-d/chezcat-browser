//! Original geometric font fixtures. These bytes are authored here rather than
//! copied from an installed or downloaded font. Set CHEZCAT_FONT_FIXTURES to a
//! scratch folder to export the TTF/WOFF test faces for a local @font-face demo.
use super::*;

fn put16(bytes: &mut [u8], at: usize, value: u16) {
    bytes[at..at + 2].copy_from_slice(&value.to_be_bytes());
}
fn put32(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_be_bytes());
}
fn push16(bytes: &mut Vec<u8>, value: i16) {
    bytes.extend_from_slice(&value.to_be_bytes());
}

type Vertex = (i16, i16, bool);
fn simple(contours: &[Vec<Vertex>]) -> Vec<u8> {
    let points: Vec<_> = contours.iter().flatten().copied().collect();
    let mut bytes = Vec::new();
    push16(&mut bytes, contours.len() as i16);
    for value in [
        points.iter().map(|p| p.0).min().unwrap_or(0),
        points.iter().map(|p| p.1).min().unwrap_or(0),
        points.iter().map(|p| p.0).max().unwrap_or(0),
        points.iter().map(|p| p.1).max().unwrap_or(0),
    ] {
        push16(&mut bytes, value);
    }
    if contours.is_empty() {
        return bytes;
    }
    let mut end = 0usize;
    for contour in contours {
        end += contour.len();
        push16(&mut bytes, (end - 1) as i16);
    }
    push16(&mut bytes, 0); // No instructions.
    bytes.extend(points.iter().map(|p| u8::from(p.2)));
    for axis in 0..2 {
        let mut previous = 0;
        for &(x, y, _) in &points {
            let value = if axis == 0 { x } else { y };
            push16(&mut bytes, value - previous);
            previous = value;
        }
    }
    bytes
}
fn polygon(points: &[(i16, i16)]) -> Vec<Vertex> {
    points.iter().map(|&(x, y)| (x, y, true)).collect()
}

fn tables() -> BTreeMap<[u8; 4], Vec<u8>> {
    let missing = simple(&[
        polygon(&[(40, 0), (40, 700), (640, 700), (640, 0)]),
        polygon(&[(150, 110), (530, 110), (530, 590), (150, 590)]),
    ]);
    let a = simple(&[
        polygon(&[
            (30, 0),
            (280, 700),
            (400, 700),
            (650, 0),
            (520, 0),
            (455, 200),
            (225, 200),
            (160, 0),
        ]),
        polygon(&[(265, 320), (415, 320), (340, 560)]),
    ]);
    let v = simple(&[polygon(&[
        (20, 700),
        (150, 700),
        (335, 150),
        (520, 700),
        (650, 700),
        (405, 0),
        (265, 0),
    ])]);
    let o = simple(&[
        vec![
            (50, 350, true),
            (50, 650, false),
            (300, 650, true),
            (550, 650, false),
            (550, 350, true),
            (550, 50, false),
            (300, 50, true),
            (50, 50, false),
        ],
        vec![
            (180, 350, true),
            (180, 180, false),
            (300, 180, true),
            (420, 180, false),
            (420, 350, true),
            (420, 520, false),
            (300, 520, true),
            (180, 520, false),
        ],
    ]);
    let accent = simple(&[polygon(&[(240, 740), (330, 900), (460, 900), (330, 740)])]);
    let mut composite = Vec::new();
    for value in [-1, 50, 50, 550, 900, 0x23, 4, 0, 0, 3, 5, 0, 0] {
        push16(&mut composite, value);
    }
    let symbol = simple(&[
        polygon(&[(50, 350), (350, 700), (650, 350), (350, 0)]),
        polygon(&[(230, 350), (350, 180), (470, 350), (350, 520)]),
    ]);
    let glyphs = [missing, Vec::new(), a, v, o, accent, composite, symbol];
    let mut glyf = Vec::new();
    let mut loca = Vec::new();
    for glyph in &glyphs {
        loca.extend_from_slice(&(glyf.len() as u32).to_be_bytes());
        glyf.extend_from_slice(glyph);
        if glyf.len() % 2 != 0 {
            glyf.push(0);
        }
    }
    loca.extend_from_slice(&(glyf.len() as u32).to_be_bytes());
    let mut head = vec![0u8; 54];
    put32(&mut head, 0, 0x00010000);
    put32(&mut head, 4, 0x00010000);
    put32(&mut head, 12, 0x5f0f3cf5);
    put16(&mut head, 18, 1000);
    put16(&mut head, 40, 650);
    put16(&mut head, 42, 900);
    put16(&mut head, 46, 8);
    put16(&mut head, 48, 2);
    put16(&mut head, 50, 1);
    let mut maxp = vec![0u8; 32];
    put32(&mut maxp, 0, 0x00010000);
    put16(&mut maxp, 4, glyphs.len() as u16);
    for (at, value) in [
        (6, 48),
        (8, 8),
        (10, 64),
        (12, 8),
        (14, 1),
        (28, 2),
        (30, 1),
    ] {
        put16(&mut maxp, at, value);
    }
    let mut hhea = vec![0u8; 36];
    put32(&mut hhea, 0, 0x00010000);
    for (at, value) in [
        (4, 900i16),
        (6, -200),
        (8, 100),
        (10, 700),
        (16, 700),
        (18, 1),
        (34, 8),
    ] {
        put16(&mut hhea, at, value as u16);
    }
    let mut hmtx = Vec::new();
    for advance in [700i16, 300, 680, 680, 600, 0, 600, 700] {
        push16(&mut hmtx, advance);
        push16(&mut hmtx, 0);
    }
    let mappings = [
        (32u32, 1u32),
        (65, 2),
        (86, 3),
        (111, 4),
        (233, 6),
        (0x1f600, 7),
    ];
    let mut cmap = vec![0u8; 12 + 16 + mappings.len() * 12];
    put16(&mut cmap, 2, 1);
    put16(&mut cmap, 4, 3);
    put16(&mut cmap, 6, 10);
    put32(&mut cmap, 8, 12);
    put16(&mut cmap, 12, 12);
    put32(&mut cmap, 16, (16 + mappings.len() * 12) as u32);
    put32(&mut cmap, 24, mappings.len() as u32);
    for (index, &(code, glyph)) in mappings.iter().enumerate() {
        let at = 28 + index * 12;
        put32(&mut cmap, at, code);
        put32(&mut cmap, at + 4, code);
        put32(&mut cmap, at + 8, glyph);
    }
    let mut kern = vec![0u8; 24];
    put16(&mut kern, 2, 1);
    put16(&mut kern, 6, 20);
    put16(&mut kern, 8, 1);
    put16(&mut kern, 10, 1);
    put16(&mut kern, 12, 6);
    put16(&mut kern, 18, 2);
    put16(&mut kern, 20, 3);
    put16(&mut kern, 22, (-80i16) as u16);
    let names = [
        (1u16, "ChezCat Geometry"),
        (2, "Regular"),
        (4, "ChezCat Geometry"),
        (6, "ChezCatGeometry"),
    ];
    let mut name = vec![0u8; 6 + names.len() * 12];
    put16(&mut name, 2, names.len() as u16);
    put16(&mut name, 4, (6 + names.len() * 12) as u16);
    let mut strings = Vec::new();
    for (index, &(id, text)) in names.iter().enumerate() {
        let at = 6 + index * 12;
        let value: Vec<_> = text.encode_utf16().flat_map(u16::to_be_bytes).collect();
        for (pos, item) in [
            (0, 3),
            (2, 1),
            (4, 0x409),
            (6, id),
            (8, value.len() as u16),
            (10, strings.len() as u16),
        ] {
            put16(&mut name, at + pos, item);
        }
        strings.extend(value);
    }
    name.extend(strings);
    let mut post = vec![0u8; 32];
    put32(&mut post, 0, 0x00030000);
    let mut os2 = vec![0u8; 96];
    for (at, value) in [
        (0, 4u16),
        (2, 600),
        (4, 400),
        (6, 5),
        (62, 0xc0),
        (64, 32),
        (66, 0xffff),
        (68, 900),
        (70, (-200i16) as u16),
        (72, 100),
        (74, 900),
        (76, 200),
        (86, 500),
        (88, 700),
        (92, 32),
        (94, 2),
    ] {
        put16(&mut os2, at, value);
    }
    put32(&mut os2, 42, 1);
    BTreeMap::from([
        (*b"head", head),
        (*b"maxp", maxp),
        (*b"hhea", hhea),
        (*b"hmtx", hmtx),
        (*b"loca", loca),
        (*b"glyf", glyf),
        (*b"cmap", cmap),
        (*b"kern", kern),
        (*b"name", name),
        (*b"post", post),
        (*b"OS/2", os2),
    ])
}

pub(crate) fn positive_boundary_fixture() -> Vec<u8> {
    let mut data = tables();
    let cmap = data.get_mut(b"cmap").unwrap();
    put32(cmap, 28, 46); // Reuse the space outline/advance for a test period.
    put32(cmap, 32, 46);
    let kern = data.get_mut(b"kern").unwrap();
    put16(kern, 20, 1); // A followed by a period has positive kerning.
    put16(kern, 22, 100);
    sfnt(data)
}

fn sfnt(tables: BTreeMap<[u8; 4], Vec<u8>>) -> Vec<u8> {
    let n = tables.len();
    let mut out = vec![0u8; 12 + n * 16];
    put32(&mut out, 0, 0x00010000);
    put16(&mut out, 4, n as u16);
    let power = 1usize << n.ilog2();
    put16(&mut out, 6, (power * 16) as u16);
    put16(&mut out, 8, power.ilog2() as u16);
    put16(&mut out, 10, (n * 16 - power * 16) as u16);
    let mut head = None;
    for (index, (tag, data)) in tables.into_iter().enumerate() {
        let at = 12 + index * 16;
        let start = out.len();
        out[at..at + 4].copy_from_slice(&tag);
        put32(&mut out, at + 4, checksum(&data, &tag == b"head"));
        put32(&mut out, at + 8, start as u32);
        put32(&mut out, at + 12, data.len() as u32);
        if &tag == b"head" {
            head = Some(start);
        }
        out.extend(data);
        while !out.len().is_multiple_of(4) {
            out.push(0);
        }
    }
    if let Some(head) = head {
        let adjustment = 0xb1b0afbau32.wrapping_sub(checksum(&out, false));
        put32(&mut out, head + 8, adjustment);
    }
    out
}

pub(crate) fn fixture_font() -> Vec<u8> {
    sfnt(tables())
}

pub(crate) fn fixture_woff() -> Vec<u8> {
    let font = fixture_font();
    let n = usize::from(u16_at(&font, 4).unwrap());
    let mut out = vec![0u8; 44 + n * 20];
    out[..4].copy_from_slice(b"wOFF");
    put32(&mut out, 4, 0x00010000);
    put16(&mut out, 12, n as u16);
    put32(&mut out, 16, font.len() as u32);
    put16(&mut out, 20, 1);
    for index in 0..n {
        let source = 12 + index * 16;
        let target = 44 + index * 20;
        let start = u32_at(&font, source + 8).unwrap() as usize;
        let len = u32_at(&font, source + 12).unwrap() as usize;
        let offset = out.len();
        out[target..target + 4].copy_from_slice(&font[source..source + 4]);
        put32(&mut out, target + 4, offset as u32);
        put32(&mut out, target + 8, len as u32);
        put32(&mut out, target + 12, len as u32);
        put32(&mut out, target + 16, u32_at(&font, source + 4).unwrap());
        out.extend_from_slice(&font[start..start + len]);
        while !out.len().is_multiple_of(4) {
            out.push(0);
        }
    }
    let len = out.len();
    put32(&mut out, 8, len as u32);
    out
}

#[test]
fn original_ttf_and_woff_share_unicode_advances_and_rasters() {
    let ttf = fixture_font();
    let woff = fixture_woff();
    let a = Font::parse(&ttf).unwrap();
    let b = Font::parse(&woff).unwrap();
    assert_eq!(a.glyph_index('A'), 2);
    assert_eq!(a.glyph_index('😀'), 7);
    assert_eq!(a.glyph_index('z'), 0);
    assert!((a.advance(a.glyph_index('A'), 20.0) - 13.6).abs() < 0.001);
    assert!((a.kerning(2, 3, 20.0) + 1.6).abs() < 0.001);
    assert_eq!(a.metrics(20.0).ascent, 18.0);
    assert_eq!(a.metrics(20.0).descent, 4.0);
    assert_eq!(a.metrics(20.0).line_gap, 2.0);
    let space = a.rasterize(a.glyph_index(' '), 20.0).unwrap();
    assert!(space.coverage.is_empty());
    assert_eq!(space.advance, 6.0);
    for character in ['A', 'V', 'o', 'é', '😀'] {
        let one = a.rasterize(a.glyph_index(character), 37.0).unwrap();
        let two = b.rasterize(b.glyph_index(character), 37.0).unwrap();
        assert_eq!(
            (one.left, one.top, one.width, one.height),
            (two.left, two.top, two.width, two.height)
        );
        assert_eq!(one.coverage, two.coverage);
        assert!(one.coverage.iter().any(|&value| value > 0 && value < 255));
    }
    if let Some(folder) = std::env::var_os("CHEZCAT_FONT_FIXTURES") {
        let folder = std::path::PathBuf::from(folder);
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("chezcat-geometry.ttf"), ttf).unwrap();
        std::fs::write(folder.join("chezcat-geometry.woff"), woff).unwrap();
    }
}

#[test]
fn quadratic_holes_and_composite_accents_preserve_outline_geometry() {
    let font = Font::parse(&fixture_font()).unwrap();
    let o = font.rasterize(font.glyph_index('o'), 100.0).unwrap();
    let x = (30 - o.left) as usize;
    let y = (-35 - o.top) as usize;
    assert_eq!(
        o.coverage[y * o.width + x],
        0,
        "the opposite-winding inner contour stays empty"
    );
    assert!(o.coverage.contains(&255));
    let accented = font.rasterize(font.glyph_index('é'), 100.0).unwrap();
    assert!(
        accented.top < o.top - 15,
        "accent component appears above the base curve"
    );
    for row in 0..o.height {
        for col in 0..o.width {
            let target_y = (o.top + row as i32 - accented.top) as usize;
            let target_x = (o.left + col as i32 - accented.left) as usize;
            assert_eq!(
                o.coverage[row * o.width + col],
                accented.coverage[target_y * accented.width + target_x]
            );
        }
    }
}

#[test]
fn cmap_four_supports_delta_and_glyph_array_mappings() {
    let mut data = vec![0u8; 12 + 16 + 3 * 8 + 2];
    put16(&mut data, 2, 1);
    put16(&mut data, 4, 3);
    put16(&mut data, 6, 1);
    put32(&mut data, 8, 12);
    let sub = &mut data[12..];
    put16(sub, 0, 4);
    let len = sub.len();
    put16(sub, 2, len as u16);
    put16(sub, 6, 6);
    for (i, end) in [65u16, 86, 65535].into_iter().enumerate() {
        put16(sub, 14 + i * 2, end);
        put16(sub, 22 + i * 2, end);
    }
    put16(sub, 28, (2u16).wrapping_sub(65));
    put16(sub, 30, 1);
    put16(sub, 32, 1);
    put16(sub, 36, 4); // Field at36 plus4 reaches first glyph word.
    put16(sub, 40, 2); // Apply delta1 -> glyph3.
    let mut font_tables = tables();
    font_tables.insert(*b"cmap", data);
    let font = Font::parse(&sfnt(font_tables)).unwrap();
    assert_eq!(font.glyph_index('A'), 2);
    assert_eq!(font.glyph_index('V'), 3);
    assert_eq!(font.glyph_index('😀'), 0);
}

#[test]
fn legacy_large_kern_wrap_is_checked_against_the_actual_table() {
    let count = 10_921usize;
    let len = 14 + count * 6;
    let mut data = vec![0; 4 + len];
    put16(&mut data, 2, 1);
    put16(&mut data, 6, len as u16);
    put16(&mut data, 8, 1);
    put16(&mut data, 10, count as u16);
    for index in 0..count {
        put32(&mut data, 18 + index * 6, index as u32);
        put16(&mut data, 22 + index * 6, (-7i16) as u16);
    }
    let pairs = read_kern(&data).unwrap();
    assert_eq!(pairs.len(), count);
    assert_eq!(pairs.last(), Some(&((count - 1) as u32, -7)));
    assert!(read_kern(&data[..data.len() - 1]).is_err());
    put16(&mut data, 6, (len - 2) as u16);
    assert!(read_kern(&data).is_err());
}

#[test]
fn malformed_tables_and_composite_cycles_fail_without_panics() {
    let original = fixture_font();
    for len in (0..original.len()).step_by(17) {
        assert!(Font::parse(&original[..len]).is_err());
    }
    let mut duplicate = original.clone();
    duplicate[28..32].copy_from_slice(&original[12..16]);
    assert!(Font::parse(&duplicate).is_err());
    let mut bad = tables();
    put32(bad.get_mut(b"loca").unwrap(), 4, u32::MAX);
    assert!(Font::parse(&sfnt(bad)).is_err());
    let mut cyclic = tables();
    let offset = u32_at(&cyclic[b"loca"], 6 * 4).unwrap() as usize;
    put16(cyclic.get_mut(b"glyf").unwrap(), offset + 12, 6);
    let font = Font::parse(&sfnt(cyclic)).unwrap();
    assert!(font.rasterize(6, 20.0).unwrap_err().contains("recursion"));
    let mut bad = tables();
    let offset = u32_at(&bad[b"loca"], 2 * 4).unwrap() as usize;
    let glyph = &mut bad.get_mut(b"glyf").unwrap()[offset..];
    glyph[16] = 9;
    glyph[17] = 255; // Repeat the first flag beyond this glyph's point count.
    assert!(Font::parse(&sfnt(bad)).unwrap().rasterize(2, 20.0).is_err());
}

#[test]
fn unsupported_formats_and_raster_limits_are_explicit() {
    assert!(Font::parse(b"wOF2").unwrap_err().contains("WOFF2"));
    assert!(Font::parse(b"OTTO").unwrap_err().contains("CFF"));
    assert!(Font::parse(b"ttcf").unwrap_err().contains("collections"));
    let mut variable = tables();
    variable.insert(*b"fvar", vec![0; 16]);
    assert!(Font::parse(&sfnt(variable))
        .unwrap_err()
        .contains("Variable"));
    let font = Font::parse(&fixture_font()).unwrap();
    for size in [f32::NAN, f32::INFINITY, -1.0, 0.0, 513.0] {
        assert!(font.rasterize(2, size).is_err());
    }
    assert!(font.rasterize(u16::MAX, 20.0).is_err());
    let mut woff = fixture_woff();
    let last = woff.len() - 4;
    woff[last] ^= 1;
    assert!(Font::parse(&woff).is_err());
}
