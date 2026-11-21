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
    put16(&mut out, 4, u16::try_from(n).unwrap());
    let power = 1usize << n.ilog2();
    put16(&mut out, 6, u16::try_from(power * 16).unwrap());
    put16(&mut out, 8, u16::try_from(power.ilog2()).unwrap());
    put16(&mut out, 10, u16::try_from(n * 16 - power * 16).unwrap());
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

fn woff_from_sfnt(font: &[u8]) -> Vec<u8> {
    let n = usize::from(u16_at(font, 4).unwrap());
    let mut out = vec![0u8; 44 + n * 20];
    out[..4].copy_from_slice(b"wOFF");
    put32(&mut out, 4, 0x00010000);
    put16(&mut out, 12, n as u16);
    put32(&mut out, 16, font.len() as u32);
    put16(&mut out, 20, 1);
    for index in 0..n {
        let source = 12 + index * 16;
        let target = 44 + index * 20;
        let start = u32_at(font, source + 8).unwrap() as usize;
        let len = u32_at(font, source + 12).unwrap() as usize;
        let offset = out.len();
        out[target..target + 4].copy_from_slice(&font[source..source + 4]);
        put32(&mut out, target + 4, offset as u32);
        put32(&mut out, target + 8, len as u32);
        put32(&mut out, target + 12, len as u32);
        put32(&mut out, target + 16, u32_at(font, source + 4).unwrap());
        out.extend_from_slice(&font[start..start + len]);
        while !out.len().is_multiple_of(4) {
            out.push(0);
        }
    }
    let len = out.len();
    put32(&mut out, 8, len as u32);
    out
}

pub(crate) fn fixture_woff() -> Vec<u8> {
    woff_from_sfnt(&fixture_font())
}

fn tables_with_directory_count(count: usize) -> BTreeMap<[u8; 4], Vec<u8>> {
    let mut data = tables();
    assert!(count >= data.len());
    // Original custom tables use distinct printable, uppercase tags and a
    // four-byte payload. BTreeMap keeps the actual directory in tag order.
    for index in 0..count - data.len() {
        let tag: [u8; 4] = format!("X{index:03X}").as_bytes().try_into().unwrap();
        assert!(data.insert(tag, (index as u32).to_be_bytes().to_vec()).is_none());
    }
    data
}

#[test]
fn font_directory_capacity_restores_glyphs_and_search_fields_above_256() {
    let baseline = Font::parse(&fixture_font()).unwrap();
    let expected_raster = baseline.rasterize(2, 37.0).unwrap();
    for (count, search_fields) in [(257, [4096, 8, 16]), (4095, [32768, 11, 32752])] {
        let ttf = sfnt(tables_with_directory_count(count));
        let woff = woff_from_sfnt(&ttf);
        let restored = decode_woff(&woff).unwrap();
        assert_eq!(restored, ttf, "All tables survive WOFF reconstruction at {count}");
        assert_eq!(u16_at(&restored, 4).unwrap(), count as u16);
        for (index, expected) in search_fields.into_iter().enumerate() {
            assert_eq!(u16_at(&restored, 6 + index * 2).unwrap(), expected);
        }
        for encoded in [&ttf, &woff] {
            let font = Font::parse(encoded).unwrap();
            assert_eq!(font.glyph_index('A'), 2);
            assert_eq!(font.glyph_index('😀'), 7);
            assert!((font.advance(2, 20.0) - 13.6).abs() < 0.001);
            assert!((font.kerning(2, 3, 20.0) + 1.6).abs() < 0.001);
            assert_eq!(font.metrics(20.0).ascent, 18.0);
            let raster = font.rasterize(2, 37.0).unwrap();
            assert_eq!((raster.left, raster.top, raster.width, raster.height),
                (expected_raster.left, expected_raster.top, expected_raster.width, expected_raster.height));
            assert_eq!(raster.coverage, expected_raster.coverage);
        }
    }
}

#[test]
fn font_directory_capacity_rejects_unrepresentable_search_fields_before_expansion() {
    let ttf = sfnt(tables_with_directory_count(4095));
    let woff = woff_from_sfnt(&ttf);
    for count in [0, 4096, u16::MAX] {
        let expected = if count == 0 { "at least one table" } else { "sfnt search fields" };
        // These counts cannot describe a reconstructable sfnt search header;
        // reject that representation before reserving a declared directory.
        let mut invalid_ttf = ttf.clone();
        put16(&mut invalid_ttf, 4, count);
        assert!(Font::parse(&invalid_ttf).unwrap_err().contains(expected));
        let mut invalid_woff = woff.clone();
        put16(&mut invalid_woff, 12, count);
        assert!(Font::parse(&invalid_woff).unwrap_err().contains(expected));
    }
}

#[test]
fn font_directory_capacity_keeps_late_sfnt_record_validation() {
    let count = 4095;
    let original = sfnt(tables_with_directory_count(count));
    let directory_end = 12 + count * 16;
    assert!(Font::parse(&original[..directory_end - 1]).unwrap_err().contains("Truncated font data"));
    let last = 12 + (count - 1) * 16;
    for (at, value, expected) in [
        (last, u32_at(&original, 12).unwrap(), "Invalid or duplicate TrueType table"),
        (last + 8, u32::MAX, "Truncated font data"),
        (last + 8, u32_at(&original, last + 8).unwrap() - 1, "Invalid or duplicate TrueType table"),
        (last + 8, u32_at(&original, 20).unwrap(), "Overlapping TrueType tables"),
        (last + 12, u32::MAX, "Truncated font data"),
    ] {
        let mut invalid = original.clone();
        put32(&mut invalid, at, value);
        assert!(Font::parse(&invalid).unwrap_err().contains(expected), "offset={at}: {expected}");
    }
}

#[test]
fn font_directory_capacity_keeps_late_woff_extent_order_and_checksum_validation() {
    let count = 4095;
    let original = woff_from_sfnt(&sfnt(tables_with_directory_count(count)));
    let last = 44 + (count - 1) * 20;
    for (at, value, expected) in [
        (last, u32_at(&original, 44).unwrap(), "Invalid WOFF table directory"),
        (last, u32::from_be_bytes(*b"AAAA"), "Invalid WOFF table directory"),
        (last + 4, u32::MAX & !3, "Truncated font data"),
        (last + 4, u32_at(&original, last + 4).unwrap() - 1, "Invalid WOFF table directory"),
        (last + 4, u32_at(&original, 48).unwrap(), "Overlapping WOFF blocks"),
        (last + 8, u32_at(&original, last + 12).unwrap() + 1, "Invalid WOFF table directory"),
        (last + 16, u32_at(&original, last + 16).unwrap() ^ 1, "WOFF table checksum"),
    ] {
        let mut invalid = original.clone();
        put32(&mut invalid, at, value);
        assert!(Font::parse(&invalid).unwrap_err().contains(expected), "offset={at}: {expected}");
    }
    let directory_end = 44 + count * 20;
    let mut truncated = original[..directory_end - 1].to_vec();
    let len = truncated.len();
    put32(&mut truncated, 8, len as u32);
    assert!(Font::parse(&truncated).unwrap_err().contains("Truncated font data"));
}

#[test]
fn font_directory_capacity_keeps_font_byte_admission_and_decoded_size_validation() {
    assert!(Font::parse(&vec![0; MAX_FONT_BYTES + 1]).unwrap_err().contains("16 MiB limit"));
    let original = woff_from_sfnt(&sfnt(tables_with_directory_count(257)));
    for (size, expected) in [
        ((MAX_FONT_BYTES + 4) as u32, "16 MiB limit"),
        (u32_at(&original, 16).unwrap() + 4, "totalSfntSize"),
    ] {
        let mut invalid = original.clone();
        put32(&mut invalid, 16, size);
        assert!(Font::parse(&invalid).unwrap_err().contains(expected));
    }
}

#[test]
fn font_directory_capacity_preserves_existing_required_table_and_raw_header_behavior() {
    let mut stored_fields = sfnt(tables_with_directory_count(257));
    stored_fields[6..12].fill(0xff);
    // The raw reader derives admissibility from numTables and continues to
    // ignore the stored optimization fields rather than trusting or using them.
    assert_eq!(Font::parse(&stored_fields).unwrap().glyph_index('A'), 2);
    let mut missing = tables_with_directory_count(4095);
    missing.remove(b"head");
    assert!(Font::parse(&sfnt(missing)).unwrap_err().contains("Missing TrueType head table"));
    let mut variable = tables_with_directory_count(4095);
    variable.remove(b"X000");
    variable.insert(*b"fvar", Vec::new());
    assert!(Font::parse(&sfnt(variable)).unwrap_err().contains("Variable TrueType fonts"));
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

fn cmap_record(data: &mut [u8], index: usize, platform: u16, encoding: u16, offset: usize) {
    let at = 4 + index * 8;
    put16(data, at, platform);
    put16(data, at + 2, encoding);
    put32(data, at + 4, u32::try_from(offset).unwrap());
}

// Synthetic parser-capacity fixtures: unsupported encoding IDs fill the
// directory. These are not claims that every ID is a registered cmap encoding.
fn cmap_records(count: usize, subtable: &[u8]) -> Vec<u8> {
    assert!((1..=usize::from(u16::MAX)).contains(&count));
    let directory_end = 4 + count * 8;
    let mut data = vec![0; directory_end];
    put16(&mut data, 2, count as u16);
    for index in 0..count {
        cmap_record(&mut data, index, 1, index as u16, directory_end);
    }
    cmap_record(&mut data, count - 1, 3, 10, directory_end);
    data.extend_from_slice(subtable);
    data
}

fn cmap_a_mapping(format: u16, glyph: u16) -> Vec<u8> {
    if format == 12 {
        let mut data = vec![0; 28];
        put16(&mut data, 0, 12);
        put32(&mut data, 4, 28);
        put32(&mut data, 12, 1);
        put32(&mut data, 16, 65);
        put32(&mut data, 20, 65);
        put32(&mut data, 24, u32::from(glyph));
        data
    } else {
        assert_eq!(format, 4);
        let mut data = vec![0; 32];
        put16(&mut data, 0, 4);
        put16(&mut data, 2, 32);
        put16(&mut data, 6, 4);
        put16(&mut data, 8, 4);
        put16(&mut data, 10, 1);
        for (at, value) in [(14, 65), (16, u16::MAX), (20, 65), (22, u16::MAX),
                            (24, glyph.wrapping_sub(65)), (26, 1)] {
            put16(&mut data, at, value);
        }
        data
    }
}

#[test]
fn cmap_metadata_record_capacity_reaches_the_tail_and_full_u16_count() {
    for count in [257, usize::from(u16::MAX)] {
        let mut font_tables = tables();
        let subtable = font_tables[b"cmap"][12..].to_vec();
        font_tables.insert(*b"cmap", cmap_records(count, &subtable));
        let font = Font::parse(&sfnt(font_tables)).unwrap();
        assert_eq!(font.glyph_index('A'), 2);
        assert_eq!(font.glyph_index('😀'), 7);
        assert!((font.advance(font.glyph_index('A'), 20.0) - 13.6).abs() < 0.001);
    }
}

#[test]
fn cmap_metadata_record_capacity_preserves_format_platform_and_offset_preference() {
    // Deliberately vary record order to cover the existing parser's selection
    // behavior; this does not add directory-order conformance requirements.
    for (later_format, first_platform, last_platform, higher_offset_first, expected) in [
        (4, 0, 3, false, 2),  // Format 12 precedes platform preference.
        (12, 3, 0, false, 2), // Platform 3 precedes the later table offset.
        (12, 0, 0, true, 3),  // Equal format/platform prefer the higher offset.
    ] {
        let first_offset = 4 + 257 * 8;
        let mut data = cmap_records(257, &cmap_a_mapping(12, 2));
        let later_offset = data.len();
        data.extend_from_slice(&cmap_a_mapping(later_format, 3));
        let (first, last) = if higher_offset_first {
            (later_offset, first_offset)
        } else {
            (first_offset, later_offset)
        };
        cmap_record(&mut data, 0, first_platform, if first_platform == 0 { 6 } else { 10 }, first);
        cmap_record(&mut data, 256, last_platform, if last_platform == 0 { 4 } else { 10 }, last);
        let mut font_tables = tables();
        font_tables.insert(*b"cmap", data);
        let font = Font::parse(&sfnt(font_tables)).unwrap();
        assert_eq!(font.glyph_index('A'), expected);
    }
}

#[test]
fn cmap_metadata_record_capacity_keeps_directory_and_selected_tail_validation() {
    let directory_end = 4 + 257 * 8;
    let mut data = cmap_records(257, &cmap_a_mapping(12, 2));
    cmap_record(&mut data, 0, 0, 4, directory_end);
    assert!(read_cmap(&data[..directory_end - 1]).unwrap_err().contains("Truncated font data"));

    let mut invalid_offset = data.clone();
    cmap_record(&mut invalid_offset, 256, 3, 10, data.len());
    assert!(read_cmap(&invalid_offset).unwrap_err().contains("Truncated font data"));

    let invalid_tail = data.len();
    let mut selected = cmap_a_mapping(12, 3);
    put16(&mut selected, 2, 1); // Reserved field in the selected format-12 table.
    data.extend_from_slice(&selected);
    cmap_record(&mut data, 256, 3, 10, invalid_tail);
    assert!(read_cmap(&data).unwrap_err().contains("Invalid cmap format 12 groups"));

    put16(&mut data, 2, 0);
    assert!(read_cmap(&data).is_err());
}

fn kern_subtable(pairs: &[(u32, i16)], override_existing: bool) -> Vec<u8> {
    assert!(pairs.len() <= usize::from(u16::MAX));
    let mut data = vec![0; 14 + pairs.len() * 6];
    let len = data.len();
    put16(&mut data, 2, len as u16);
    put16(&mut data, 4, if override_existing { 9 } else { 1 });
    put16(&mut data, 6, pairs.len() as u16);
    if !pairs.is_empty() {
        let power = 1usize << pairs.len().ilog2();
        put16(&mut data, 8, (power * 6) as u16);
        put16(&mut data, 10, power.ilog2() as u16);
        put16(&mut data, 12, ((pairs.len() - power) * 6) as u16);
    }
    for (index, &(key, value)) in pairs.iter().enumerate() {
        put32(&mut data, 14 + index * 6, key);
        put16(&mut data, 18 + index * 6, value as u16);
    }
    data
}

fn repeated_kern_subtables(subtable: &[u8], count: usize) -> Vec<u8> {
    assert!(count <= usize::from(u16::MAX));
    let mut data = vec![0; 4];
    put16(&mut data, 2, count as u16);
    for _ in 0..count {
        data.extend_from_slice(subtable);
    }
    data
}

#[test]
fn kern_metadata_record_capacity_keeps_late_additive_and_override_metrics() {
    let key = 0x0002_0003;
    let subtable = kern_subtable(&[(key, -1)], false);
    let mut data = repeated_kern_subtables(&subtable, 65);
    for expected in [-65.0, -7.0, -4.0] {
        let mut font_tables = tables();
        font_tables.insert(*b"kern", data.clone());
        let font = Font::parse(&sfnt(font_tables)).unwrap();
        assert_eq!(font.kerning(2, 3, 250.0), expected / 4.0);
        if expected == -65.0 {
            // The 65th subtable replaces all earlier contributions.
            let last = 4 + 64 * subtable.len();
            data[last..].copy_from_slice(&kern_subtable(&[(key, -7)], true));
        } else if expected == -7.0 {
            put16(&mut data, 2, 66);
            data.extend_from_slice(&kern_subtable(&[(key, 3)], false));
        }
    }
}

#[test]
fn kern_metadata_record_capacity_full_u16_count_keeps_i32_extremes() {
    let key = 0x0002_0003;
    for (value, expected) in [(i16::MIN, -2_147_450_880), (i16::MAX, 2_147_385_345)] {
        let data = repeated_kern_subtables(&kern_subtable(&[(key, value)], false), usize::from(u16::MAX));
        assert_eq!(read_kern(&data).unwrap(), vec![(key, expected)]);
    }
}

#[test]
fn kern_metadata_record_capacity_full_count_skips_unknown_formats_with_input_progress() {
    let mut subtable = vec![0; 6];
    put16(&mut subtable, 2, 6);
    put16(&mut subtable, 4, 0x0100);
    let data = repeated_kern_subtables(&subtable, usize::from(u16::MAX));
    assert!(read_kern(&data).unwrap().is_empty());
    assert!(read_kern(&data[..data.len() - 1]).unwrap_err().contains("Truncated font data"));
}

#[test]
fn kern_metadata_record_capacity_keeps_late_length_truncation_and_pair_order_checks() {
    let subtable = kern_subtable(&[(0x0002_0003, -1)], false);
    let data = repeated_kern_subtables(&subtable, 65);
    assert!(read_kern(&data[..data.len() - 1]).unwrap_err().contains("Truncated font data"));
    let mut invalid_length = data.clone();
    put16(&mut invalid_length, 4 + 64 * subtable.len() + 2, 0);
    assert!(read_kern(&invalid_length).unwrap_err().contains("Invalid kerning subtable length"));

    let mut unsorted = data[..4 + 64 * subtable.len()].to_vec();
    unsorted.extend_from_slice(&kern_subtable(&[(0x0002_0003, -1), (0x0002_0002, -1)], false));
    assert!(read_kern(&unsorted).unwrap_err().contains("Unsorted kerning pairs"));
}

#[test]
fn kern_metadata_record_capacity_retains_the_aggregate_pair_budget() {
    let pairs: Vec<_> = (0..1024).map(|key| (key, -1)).collect();
    let mut data = repeated_kern_subtables(&kern_subtable(&pairs, false), 256);
    let found = read_kern(&data).unwrap();
    assert_eq!(found.len(), 1024);
    assert_eq!(found.first(), Some(&(0, -256)));
    assert_eq!(found.last(), Some(&(1023, -256)));
    put16(&mut data, 2, 257);
    data.extend_from_slice(&kern_subtable(&[(0, -1)], false));
    assert!(read_kern(&data).unwrap_err().contains("Kerning pairs exceed their limit"));
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
