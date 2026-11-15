//! Font selection, proportional advances and coverage painting for page text.
//! Browser-owned UI remains on the original deterministic bitmap font.
use crate::fonts::{Book, Face};
use crate::paint::{Canvas, GLYPH_ADVANCE, GLYPH_HEIGHT};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct FontSpec {
    book: Book,
    primary: Arc<Face>,
    families: Vec<String>,
    weight: u16,
    italic: bool,
    pub size: f32,
    pub scale: i32,
}

#[derive(Debug)]
struct Glyph {
    face: Option<Arc<Face>>,
    id: u16,
    character: char,
    x: f32,
    advance: f32,
    bold: bool,
    italic: bool,
}

#[derive(Debug)]
pub struct Shape {
    glyphs: Vec<Glyph>,
    positions: Vec<i32>,
    size: f32,
    scale: i32,
    pub ascent: i32,
    pub descent: i32,
}

#[derive(Default)]
pub struct Pen {
    x: f32,
    previous_x: f32,
    previous: Option<(Arc<Face>, u16)>,
}

impl FontSpec {
    pub fn new(
        book: &Book,
        families: &[String],
        weight: u16,
        italic: bool,
        size: f32,
    ) -> Option<Self> {
        let primary = book.select(families, weight, italic)?;
        let size = if size.is_finite() {
            size.clamp(1.0, 512.0)
        } else {
            16.0
        };
        Some(Self {
            book: book.clone(),
            primary,
            families: families.to_vec(),
            weight,
            italic,
            size,
            scale: (size / 8.0).round().clamp(1.0, 16.0) as i32,
        })
    }

    fn face(&self, character: char) -> Option<Arc<Face>> {
        if self.primary.supports(character) {
            return Some(self.primary.clone());
        }
        self.book
            .resolve(&self.families, self.weight, self.italic, character)
    }

    pub fn metrics(&self) -> (i32, i32) {
        if let Some(face) = self.book.select(&self.families, self.weight, self.italic) {
            let metrics = face.font.metrics(self.size);
            (
                metrics.ascent.ceil().max(1.0) as i32,
                metrics.descent.ceil().max(0.0) as i32,
            )
        } else {
            (GLYPH_HEIGHT * self.scale, 2 * self.scale)
        }
    }

    pub fn height(&self) -> i32 {
        let (ascent, descent) = self.metrics();
        ascent + descent
    }

    pub fn line_height(&self) -> i32 {
        self.height()
            + self
                .primary
                .font
                .metrics(self.size)
                .line_gap
                .ceil()
                .max(0.0) as i32
    }

    pub fn advance_pen(&self, pen: &mut Pen, character: char) -> i32 {
        if let Some(face) = self.face(character) {
            let id = face.font.glyph_index(character);
            if let Some((before, glyph)) = &pen.previous {
                if Arc::ptr_eq(before, &face) {
                    pen.x = (pen.x + face.font.kerning(*glyph, id, self.size))
                        .clamp(pen.previous_x, 1_000_000.0);
                }
            }
            pen.previous_x = pen.x;
            pen.x += face.font.advance(id, self.size).clamp(0.0, 8192.0);
            pen.previous = Some((face, id));
        } else {
            pen.x += (GLYPH_ADVANCE * self.scale) as f32;
            pen.previous = None;
        }
        pen.x = pen.x.min(1_000_000.0);
        pen.x.round() as i32
    }

    pub fn measure(&self, text: &str) -> i32 {
        let mut pen = Pen::default();
        for character in text.chars().take(1_000_000) {
            self.advance_pen(&mut pen, character);
        }
        pen.x.round() as i32
    }

    pub fn shape(&self, text: &str) -> Shape {
        let (mut ascent, mut descent) = self.metrics();
        let mut glyphs: Vec<Glyph> = Vec::new();
        let mut positions = vec![0];
        let mut x = 0.0f32;
        for character in text.chars().take(1_000_000) {
            let face = self.face(character);
            let id = face
                .as_ref()
                .map_or(0, |face| face.font.glyph_index(character));
            if let (Some(face), Some(previous)) = (&face, glyphs.last()) {
                if previous
                    .face
                    .as_ref()
                    .is_some_and(|before| Arc::ptr_eq(before, face))
                {
                    // Caret positions stay monotonic even for hostile kern tables.
                    x = (x + face.font.kerning(previous.id, id, self.size))
                        .clamp(previous.x, 1_000_000.0);
                    *positions.last_mut().unwrap() = x.round() as i32;
                }
            }
            let advance = if let Some(face) = &face {
                let metrics = face.font.metrics(self.size);
                ascent = ascent.max(metrics.ascent.ceil() as i32);
                descent = descent.max(metrics.descent.ceil() as i32);
                face.font.advance(id, self.size).clamp(0.0, 8192.0)
            } else {
                ascent = ascent.max(GLYPH_HEIGHT * self.scale);
                descent = descent.max(2 * self.scale);
                (GLYPH_ADVANCE * self.scale) as f32
            };
            let bold = self.weight >= 600 && face.as_ref().is_none_or(|face| face.weight < 600);
            let italic = self.italic && face.as_ref().is_none_or(|face| !face.italic);
            glyphs.push(Glyph {
                face,
                id,
                character,
                x,
                advance,
                bold,
                italic,
            });
            x = (x + advance).min(1_000_000.0);
            positions.push(x.round() as i32);
        }
        Shape {
            glyphs,
            positions,
            size: self.size,
            scale: self.scale,
            ascent,
            descent,
        }
    }

    pub fn fit(&self, text: &str, pixels: i32) -> String {
        let shape = self.shape(text);
        if shape.width() <= pixels {
            return text.to_owned();
        }
        let suffix = "...";
        let allowance = (pixels - self.measure(suffix)).max(0);
        let mut count = shape.fitting_count(0, allowance);
        if pixels < self.measure(suffix) {
            return text
                .chars()
                .take(shape.fitting_count(0, pixels.max(0)))
                .collect();
        }
        // Joining a prefix and an ellipsis introduces a new kerning pair.
        // Reuse the measured prefix pen rather than repeatedly reshaping it.
        while count > 0 {
            let glyph = &shape.glyphs[count - 1];
            let mut pen = Pen {
                x: (glyph.x + glyph.advance).min(1_000_000.0),
                previous_x: glyph.x,
                previous: glyph.face.as_ref().map(|face| (face.clone(), glyph.id)),
            };
            for character in suffix.chars() {
                self.advance_pen(&mut pen, character);
            }
            if pen.x.round() as i32 <= pixels {
                break;
            }
            count -= 1;
        }
        text.chars().take(count).chain(suffix.chars()).collect()
    }
}

impl Shape {
    pub fn width(&self) -> i32 {
        self.positions.last().copied().unwrap_or(0)
    }
    pub fn height(&self) -> i32 {
        self.ascent + self.descent
    }
    pub fn len(&self) -> usize {
        self.glyphs.len()
    }
    pub fn x(&self, index: usize) -> i32 {
        self.positions
            .get(index)
            .copied()
            .unwrap_or_else(|| self.width())
    }
    pub fn fitting_count(&self, start: usize, pixels: i32) -> usize {
        let start = start.min(self.len());
        let origin = self.glyphs.get(start).map_or(0.0, |glyph| glyph.x);
        // A line break removes kerning with the following character. Caret
        // positions include that adjustment and cannot measure a standalone
        // substring, e.g. the A in AV when V moves to the next line.
        self.glyphs[start..]
            .iter()
            .take_while(|glyph| (glyph.x + glyph.advance - origin).round() as i32 <= pixels.max(0))
            .count()
    }
    pub fn hit(&self, x: i32) -> usize {
        self.positions
            .windows(2)
            .take_while(|pair| x >= pair[0] + (pair[1] - pair[0]) / 2)
            .count()
    }
    pub fn draw(&self, canvas: &mut Canvas, x: i32, y: i32, color: u32) {
        self.draw_range(canvas, x, y, color, 0, self.len());
    }
    pub fn draw_range(
        &self,
        canvas: &mut Canvas,
        x: i32,
        y: i32,
        color: u32,
        start: usize,
        end: usize,
    ) {
        let clip = canvas.clip();
        for glyph in self.glyphs.iter().take(end).skip(start) {
            let origin = x.saturating_add(glyph.x.round() as i32);
            if let Some(face) = &glyph.face {
                if let Ok(bitmap) = face.glyph(glyph.id, self.size) {
                    let top = y.saturating_add(self.ascent).saturating_add(bitmap.top);
                    let first = clip.top.saturating_sub(top).max(0) as usize;
                    let last = (clip.bottom.saturating_sub(top).max(0) as usize).min(bitmap.height);
                    for row in first..last {
                        let baseline_y = bitmap.top + row as i32;
                        let slant = if glyph.italic {
                            (-baseline_y as f32 * 0.2).round() as i32
                        } else {
                            0
                        };
                        let left = origin.saturating_add(bitmap.left).saturating_add(slant);
                        let first_col = clip
                            .left
                            .saturating_sub(left)
                            .saturating_sub(i32::from(glyph.bold))
                            .max(0) as usize;
                        let last_col =
                            (clip.right.saturating_sub(left).max(0) as usize).min(bitmap.width);
                        for col in first_col..last_col {
                            let alpha = bitmap.coverage[row * bitmap.width + col];
                            if alpha == 0 {
                                continue;
                            }
                            let px = left + col as i32;
                            let py = top + row as i32;
                            canvas.blend_pixel(px, py, color, alpha);
                            if glyph.bold {
                                canvas.blend_pixel(px + 1, py, color, alpha);
                            }
                        }
                    }
                    continue;
                }
            }
            // A missing or rejected outline stays visible through the original
            // bitmap fallback; it cannot remove surrounding page text.
            let mut buffer = [0; 4];
            let character = if glyph.character == '\u{a0}' {
                ' '
            } else {
                glyph.character
            };
            let text = character.encode_utf8(&mut buffer);
            let top = y + self.ascent - GLYPH_HEIGHT * self.scale;
            canvas.styled_text(origin, top, text, self.scale, color, glyph.italic);
            if glyph.bold {
                canvas.styled_text(origin + 1, top, text, self.scale, color, glyph.italic);
            }
        }
    }
}

#[cfg(test)]
#[path = "text_tests.rs"]
mod tests;
