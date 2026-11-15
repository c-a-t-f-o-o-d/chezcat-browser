//! Original QOI and Netpbm sample readers. No codec library is called.
//! References: qoiformat.org/qoi-specification.pdf and the Netpbm format manuals.
use super::{be32, dimensions, pixel, reserved, Bitmap, ImageResult};

fn whitespace(byte: &u8) -> bool {
    // Netpbm uses all six ASCII/C whitespace bytes, including vertical tab.
    matches!(*byte, b' ' | b'\t'..=b'\r')
}

pub(super) fn qoi(data: &[u8], check: &dyn Fn() -> ImageResult<()>) -> ImageResult<Bitmap> {
    if data.len() < 22 || !data.starts_with(b"qoif") {
        return Err("Truncated QOI header or end marker.".into());
    }
    let width = be32(&data[4..]) as usize;
    let height = be32(&data[8..]) as usize;
    let count = dimensions(width, height)?;
    if !matches!(data[12], 3 | 4) || data[13] > 1 {
        return Err("Invalid QOI channel count or colorspace.".into());
    }
    let end = data.len() - 8;
    if data[end..] != [0, 0, 0, 0, 0, 0, 0, 1] {
        return Err("QOI end marker is missing.".into());
    }
    let mut at = 14;
    let mut last = [0u8, 0, 0, 255];
    let mut index = [[0u8; 4]; 64];
    let mut previous_index = None;
    let mut pixels = reserved(count)?;
    let mut next_check = 0;
    while pixels.len() < count {
        if pixels.len() >= next_check {
            check()?;
            next_check = pixels.len() + 4096;
        }
        let tag = *data
            .get(at)
            .filter(|_| at < end)
            .ok_or("Truncated QOI pixel stream.")?;
        at += 1;
        let mut run = 1;
        let current_index = if tag < 64 { Some(tag) } else { None };
        if current_index.is_some() && current_index == previous_index {
            return Err("Consecutive QOI index chunks must use a run.".into());
        }
        previous_index = current_index;
        match tag {
            254 | 255 => {
                let bytes = if tag == 254 { 3 } else { 4 };
                let stop = at
                    .checked_add(bytes)
                    .filter(|&n| n <= end)
                    .ok_or("Truncated QOI literal.")?;
                last[..bytes].copy_from_slice(&data[at..stop]);
                at = stop;
            }
            0..=63 => last = index[tag as usize],
            64..=127 => {
                last[0] = last[0].wrapping_add_signed(((tag >> 4) & 3) as i8 - 2);
                last[1] = last[1].wrapping_add_signed(((tag >> 2) & 3) as i8 - 2);
                last[2] = last[2].wrapping_add_signed((tag & 3) as i8 - 2);
            }
            128..=191 => {
                let extra = *data
                    .get(at)
                    .filter(|_| at < end)
                    .ok_or("Truncated QOI luma chunk.")?;
                at += 1;
                let green = (tag & 63) as i8 - 32;
                last[0] = last[0].wrapping_add_signed(green + (extra >> 4) as i8 - 8);
                last[1] = last[1].wrapping_add_signed(green);
                last[2] = last[2].wrapping_add_signed(green + (extra & 15) as i8 - 8);
            }
            _ => run = usize::from(tag & 63) + 1,
        }
        if run > count - pixels.len() {
            return Err("QOI run exceeds the image dimensions.".into());
        }
        let slot = (usize::from(last[0]) * 3
            + usize::from(last[1]) * 5
            + usize::from(last[2]) * 7
            + usize::from(last[3]) * 11)
            & 63;
        index[slot] = last;
        let color = pixel(last[0], last[1], last[2], last[3]);
        pixels.resize(pixels.len() + run, color);
    }
    if at != end {
        return Err("QOI pixel stream has trailing data.".into());
    }
    check()?;
    Ok(Bitmap {
        width,
        height,
        pixels,
    })
}

struct Tokens<'a> {
    data: &'a [u8],
    at: usize,
    until_check: usize,
    check: &'a dyn Fn() -> ImageResult<()>,
}

impl Tokens<'_> {
    fn advance(&mut self) -> ImageResult<()> {
        self.at += 1;
        self.until_check -= 1;
        if self.until_check == 0 {
            (self.check)()?;
            self.until_check = 4096;
        }
        Ok(())
    }
    fn skip(&mut self) -> ImageResult<()> {
        loop {
            while self.data.get(self.at).is_some_and(whitespace) {
                self.advance()?;
            }
            if self.data.get(self.at) != Some(&b'#') {
                return Ok(());
            }
            while self
                .data
                .get(self.at)
                .is_some_and(|b| !matches!(*b, b'\r' | b'\n'))
            {
                self.advance()?;
            }
        }
    }
    fn number(&mut self) -> ImageResult<usize> {
        self.skip()?;
        let start = self.at;
        let mut number = 0usize;
        while let Some(value) = self.data.get(self.at).filter(|b| b.is_ascii_digit()) {
            number = number
                .checked_mul(10)
                .and_then(|n| n.checked_add(usize::from(*value - b'0')))
                .ok_or("Netpbm integer is too large.")?;
            self.advance()?;
        }
        if start == self.at
            || self
                .data
                .get(self.at)
                .is_some_and(|b| !whitespace(b) && *b != b'#')
        {
            return Err("Invalid Netpbm header or sample.".into());
        }
        Ok(number)
    }
    fn bitmap_bit(&mut self) -> ImageResult<usize> {
        self.skip()?;
        let bit = match self.data.get(self.at) {
            Some(b'0') => 0,
            Some(b'1') => 1,
            _ => return Err("Plain PBM pixels must be 0 or 1.".into()),
        };
        self.advance()?;
        Ok(bit)
    }
    fn raster_separator(&mut self) -> ImageResult<()> {
        // A comment immediately after the last integer precedes the separator.
        // Once one whitespace byte is consumed, all following bytes are pixels.
        if self.data.get(self.at) == Some(&b'#') {
            while self
                .data
                .get(self.at)
                .is_some_and(|b| !matches!(*b, b'\r' | b'\n'))
            {
                self.advance()?;
            }
        }
        if !self.data.get(self.at).is_some_and(whitespace) {
            return Err("Netpbm raster separator is missing.".into());
        }
        self.advance()
    }
}

fn scaled(value: usize, maximum: usize) -> ImageResult<u8> {
    if value > maximum {
        return Err("Netpbm sample exceeds its declared range.".into());
    }
    Ok(((value * 255 + maximum / 2) / maximum) as u8)
}

pub(super) fn netpbm(data: &[u8], check: &dyn Fn() -> ImageResult<()>) -> ImageResult<Bitmap> {
    if data.len() < 3 || data[0] != b'P' || !whitespace(&data[2]) {
        return Err("Invalid Netpbm signature.".into());
    }
    if data[1] == b'7' {
        return pam(data, check);
    }
    if !(b'1'..=b'6').contains(&data[1]) {
        return Err("Unsupported Netpbm image type.".into());
    }
    let mut tokens = Tokens {
        data,
        at: 2,
        until_check: 4096,
        check,
    };
    let width = tokens.number()?;
    let height = tokens.number()?;
    let count = dimensions(width, height)?;
    let kind = data[1] - b'0';
    let maximum = if matches!(kind, 1 | 4) {
        1
    } else {
        tokens.number()?
    };
    if maximum == 0 || maximum > 65535 {
        return Err("Netpbm maximum sample must be 1 through 65535.".into());
    }
    let channels = if matches!(kind, 3 | 6) { 3 } else { 1 };
    let mut pixels = reserved(count)?;
    if kind <= 3 {
        for n in 0..count {
            if n & 1023 == 0 {
                check()?;
            }
            let first = if kind == 1 {
                1 - tokens.bitmap_bit()?
            } else {
                tokens.number()?
            };
            let r = scaled(first, maximum)?;
            let (g, b) = if channels == 3 {
                (
                    scaled(tokens.number()?, maximum)?,
                    scaled(tokens.number()?, maximum)?,
                )
            } else {
                (r, r)
            };
            pixels.push(pixel(r, g, b, 255));
        }
    } else {
        tokens.raster_separator()?;
        let bytes_per_sample = if maximum < 256 { 1 } else { 2 };
        let row_bytes = if kind == 4 {
            width.div_ceil(8)
        } else {
            width * channels * bytes_per_sample
        };
        let end = tokens
            .at
            .checked_add(row_bytes * height)
            .filter(|&n| n <= data.len())
            .ok_or("Truncated Netpbm raster.")?;
        let raster = &data[tokens.at..end];
        for y in 0..height {
            check()?;
            let row = &raster[y * row_bytes..(y + 1) * row_bytes];
            for x in 0..width {
                if x & 1023 == 0 {
                    check()?;
                }
                if kind == 4 {
                    let value = if (row[x / 8] >> (7 - x % 8)) & 1 == 0 {
                        255
                    } else {
                        0
                    };
                    pixels.push(pixel(value, value, value, 255));
                } else {
                    let sample = |channel: usize| {
                        let at = (x * channels + channel) * bytes_per_sample;
                        scaled(
                            if bytes_per_sample == 1 {
                                usize::from(row[at])
                            } else {
                                usize::from(u16::from_be_bytes([row[at], row[at + 1]]))
                            },
                            maximum,
                        )
                    };
                    let r = sample(0)?;
                    let (g, b) = if channels == 3 {
                        (sample(1)?, sample(2)?)
                    } else {
                        (r, r)
                    };
                    pixels.push(pixel(r, g, b, 255));
                }
            }
        }
    }
    // Netpbm streams can contain more than one image. We display the first.
    check()?;
    Ok(Bitmap {
        width,
        height,
        pixels,
    })
}

fn pam(data: &[u8], check: &dyn Fn() -> ImageResult<()>) -> ImageResult<Bitmap> {
    let mut position = 0;
    let mut fields = [None; 4];
    let mut tuple = String::new();
    loop {
        check()?;
        let stop = data[position..data.len().min(65536)]
            .iter()
            .position(|b| *b == b'\n')
            .and_then(|n| position.checked_add(n))
            .ok_or("Truncated PAM header.")?;
        if stop >= 65536 {
            return Err("PAM header exceeds 64 KiB.".into());
        }
        let line = std::str::from_utf8(&data[position..stop])
            .map_err(|_| "PAM header must be ASCII text.")?;
        if !line.is_ascii() {
            return Err("PAM header must be ASCII text.".into());
        }
        position = stop + 1;
        let line = line.trim();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        let mut words = line
            .split(|c: char| c.is_ascii() && whitespace(&(c as u8)))
            .filter(|s| !s.is_empty());
        let key = words.next().unwrap();
        if key == "P7" && fields.iter().all(Option::is_none) && tuple.is_empty() && stop <= 3 {
            if words.next().is_some() {
                return Err("Invalid PAM signature line.".into());
            }
            continue;
        }
        if key == "ENDHDR" {
            if words.next().is_some() {
                return Err("Invalid PAM ENDHDR line.".into());
            }
            break;
        }
        if key == "TUPLTYPE" {
            let value = words.collect::<Vec<_>>().join(" ");
            if value.is_empty() {
                return Err("PAM tuple type is empty.".into());
            }
            if !tuple.is_empty() {
                tuple.push(' ');
            }
            tuple.push_str(&value);
            if tuple.len() > 128 {
                return Err("PAM tuple type exceeds its limit.".into());
            }
            continue;
        }
        let slot = match key {
            "WIDTH" => 0,
            "HEIGHT" => 1,
            "DEPTH" => 2,
            "MAXVAL" => 3,
            _ => return Err(format!("Unsupported PAM header field {key}.")),
        };
        let value = words
            .next()
            .filter(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
            .ok_or("Invalid PAM header value.")?
            .parse::<usize>()
            .map_err(|_| "PAM value is too large.")?;
        if words.next().is_some() || fields[slot].replace(value).is_some() {
            return Err("Duplicate or invalid PAM header field.".into());
        }
    }
    let width = fields[0].ok_or("PAM width is missing.")?;
    let height = fields[1].ok_or("PAM height is missing.")?;
    let depth = fields[2].ok_or("PAM depth is missing.")?;
    let maximum = fields[3].ok_or("PAM maximum sample is missing.")?;
    let count = dimensions(width, height)?;
    if maximum == 0 || maximum > 65535 || !(1..=4).contains(&depth) {
        return Err("PAM supports one through four unsigned channels up to 16 bits.".into());
    }
    let valid = match tuple.as_str() {
        "" => true,
        "BLACKANDWHITE" => depth == 1 && maximum == 1,
        "BLACKANDWHITE_ALPHA" => depth == 2 && maximum == 1,
        "GRAYSCALE" => depth == 1,
        "GRAYSCALE_ALPHA" => depth == 2,
        "RGB" => depth == 3,
        "RGB_ALPHA" => depth == 4,
        _ => false,
    };
    if !valid {
        return Err("Unsupported or inconsistent PAM tuple type.".into());
    }
    let sample_bytes = if maximum < 256 { 1 } else { 2 };
    let end = position
        .checked_add(count * depth * sample_bytes)
        .filter(|&n| n <= data.len())
        .ok_or("Truncated PAM raster.")?;
    let raster = &data[position..end];
    let mut pixels = reserved(count)?;
    for n in 0..count {
        if n & 1023 == 0 {
            check()?;
        }
        let sample = |c: usize| {
            let at = (n * depth + c) * sample_bytes;
            scaled(
                if sample_bytes == 1 {
                    usize::from(raster[at])
                } else {
                    usize::from(u16::from_be_bytes([raster[at], raster[at + 1]]))
                },
                maximum,
            )
        };
        let r = sample(0)?;
        let (g, b) = if depth >= 3 {
            (sample(1)?, sample(2)?)
        } else {
            (r, r)
        };
        let a = if depth == 2 || depth == 4 {
            sample(depth - 1)?
        } else {
            255
        };
        pixels.push(pixel(r, g, b, a));
    }
    check()?;
    Ok(Bitmap {
        width,
        height,
        pixels,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn qoi_file(width: u32, payload: &[u8]) -> Vec<u8> {
        let mut data = b"qoif".to_vec();
        data.extend_from_slice(&width.to_be_bytes());
        data.extend_from_slice(&1u32.to_be_bytes());
        data.extend_from_slice(&[4, 0]);
        data.extend_from_slice(payload);
        data.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 1]);
        data
    }
    #[test]
    fn qoi_literals_deltas_runs_index_and_marker_are_exact() {
        let bytes = qoi_file(
            6,
            &[
                255, 10, 20, 30, 40, 254, 20, 30, 40, 0x7f, 0xc0, 0xa1, 0x88, 12,
            ],
        );
        let decoded = qoi(&bytes, &|| Ok(())).unwrap();
        assert_eq!(
            decoded.pixels,
            [0x280a141e, 0x28141e28, 0x28151f29, 0x28151f29, 0x2816202a, 0x280a141e]
        );
        for end in 0..bytes.len() {
            assert!(qoi(&bytes[..end], &|| Ok(())).is_err());
        }
        assert!(qoi(&qoi_file(1, &[0xc1]), &|| Ok(())).is_err());
        assert!(qoi(&qoi_file(2, &[0, 0]), &|| Ok(())).is_err());
    }
    #[test]
    fn netpbm_packed_plain_high_depth_and_alpha_samples() {
        assert_eq!(
            netpbm(b"P1\n3 1\n010", &|| Ok(())).unwrap().pixels,
            [0xffffffff, 0xff000000, 0xffffffff]
        );
        assert_eq!(
            netpbm(b"P4\n3 1\n\x40", &|| Ok(())).unwrap().pixels,
            [0xffffffff, 0xff000000, 0xffffffff]
        );
        assert_eq!(
            netpbm(b"P5\n2 1\n65535\n\x80\0\xff\xff", &|| Ok(()))
                .unwrap()
                .pixels,
            [0xff808080, 0xffffffff]
        );
        assert_eq!(
            netpbm(b"P6\n1 1\n255\r\x0a\x14\x1e", &|| Ok(()))
                .unwrap()
                .pixels,
            [0xff0a141e]
        );
        assert_eq!(netpbm(b"P7\nWIDTH 1\nHEIGHT 1\nDEPTH 2\nMAXVAL 15\nTUPLTYPE GRAYSCALE_ALPHA\nENDHDR\n\x0f\x08",&||Ok(())).unwrap().pixels,[0x88ffffff]);
        assert!(netpbm(b"P5\n1 1\n15\n\x10", &|| Ok(())).is_err());
        assert!(netpbm(b"P7\nWIDTH 1\nWIDTH 2\nENDHDR\n", &|| Ok(())).is_err());
    }
}
