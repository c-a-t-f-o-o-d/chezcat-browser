//! Bounded native textarea editing and painting. The shared form value remains
//! authoritative; visual soft wraps never add characters to submitted values.
use super::*;
use std::ops::Range;

fn line_height(rect: &layout::ControlBox) -> i32 {
    rect.text_font
        .as_ref()
        .map_or((GLYPH_HEIGHT + 2) * rect.scale, |font| font.height() + 2)
        .max(1)
}

fn lines(rect: &layout::ControlBox, value: &Editor) -> Vec<Range<usize>> {
    let available = (rect.width - 12).max(1);
    let mut rows = Vec::new();
    let (mut start, mut width) = (0, 0);
    let mut pen = crate::text::Pen::default();
    for (index, &ch) in value.chars.iter().enumerate() {
        if ch == '\n' {
            rows.push(start..index);
            start = index + 1;
            width = 0;
            pen = crate::text::Pen::default();
            continue;
        }
        let next = rect.text_font.as_ref().map_or_else(
            || width + GLYPH_ADVANCE * rect.scale,
            |font| font.advance_pen(&mut pen, ch),
        );
        if next > available && index > start {
            rows.push(start..index);
            start = index;
            pen = crate::text::Pen::default();
            width = rect
                .text_font
                .as_ref()
                .map_or(GLYPH_ADVANCE * rect.scale, |font| {
                    font.advance_pen(&mut pen, ch)
                });
        } else {
            width = next;
        }
    }
    rows.push(start..value.chars.len());
    rows
}

fn row_for(rows: &[Range<usize>], position: usize) -> usize {
    rows.iter()
        .enumerate()
        .find_map(|(index, row)| {
            (position < row.end
                || position == row.end
                    && rows.get(index + 1).is_none_or(|next| next.start > row.end))
            .then_some(index)
        })
        .unwrap_or(rows.len() - 1)
}

fn row_text(value: &Editor, row: &Range<usize>) -> String {
    value.chars[row.clone()].iter().collect()
}

fn x_at(rect: &layout::ControlBox, value: &Editor, row: &Range<usize>, index: usize) -> i32 {
    let offset = index.saturating_sub(row.start).min(row.len());
    rect.text_font
        .as_ref()
        .map_or(offset as i32 * GLYPH_ADVANCE * rect.scale, |font| {
            font.shape(&row_text(value, row)).x(offset)
        })
}

fn hit(rect: &layout::ControlBox, value: &Editor, row: &Range<usize>, x: i32) -> usize {
    row.start
        + rect
            .text_font
            .as_ref()
            .map_or_else(
                || (x.max(0) / (GLYPH_ADVANCE * rect.scale).max(1)) as usize,
                |font| font.shape(&row_text(value, row)).hit(x),
            )
            .min(row.len())
}

pub(super) fn sync(rect: &layout::ControlBox, value: &mut Editor) {
    let rows = lines(rect, value);
    let current = row_for(&rows, value.cursor);
    let visible = ((rect.height - 8) / line_height(rect)).max(1) as usize;
    value.offset = value.offset.min(rows.len().saturating_sub(visible));
    if current < value.offset {
        value.offset = current;
    } else if current >= value.offset + visible {
        value.offset = current + 1 - visible;
    }
}

pub(super) fn character_at(
    rect: &layout::ControlBox,
    value: &Editor,
    x: i32,
    document_y: i32,
) -> usize {
    let rows = lines(rect, value);
    let row = (value.offset + ((document_y - rect.y - 4).max(0) / line_height(rect)) as usize)
        .min(rows.len() - 1);
    hit(rect, value, &rows[row], x - rect.x - 6)
}

impl App {
    pub(super) fn textarea_key(&mut self, node: usize, key: u32, shift: bool) -> bool {
        if key == 0x0d {
            self.paste("\n");
            return true;
        }
        if !matches!(key, 0x26 | 0x28 | 0x24 | 0x23) {
            return false;
        }
        let Some(rect) = self.page.controls.iter().find(|rect| rect.node == node) else {
            return true;
        };
        let value = &self.forms.controls[&node].value;
        let rows = lines(rect, value);
        let index = row_for(&rows, value.cursor);
        let position = match key {
            0x24 => rows[index].start,
            0x23 => rows[index].end,
            _ => {
                let next = if key == 0x26 {
                    index.saturating_sub(1)
                } else {
                    (index + 1).min(rows.len() - 1)
                };
                let x = x_at(rect, value, &rows[index], value.cursor);
                hit(rect, value, &rows[next], x)
            }
        };
        self.forms
            .controls
            .get_mut(&node)
            .unwrap()
            .value
            .move_to(position, shift);
        self.sync_editors();
        true
    }
}

pub(super) fn draw(
    canvas: &mut Canvas,
    rect: &layout::ControlBox,
    y: i32,
    control: &crate::forms::Control,
    placeholder: &str,
    focused: bool,
    color: u32,
) {
    let clip = canvas.clip();
    canvas.set_clip(Some(clip.intersect(crate::paint::ClipRect::new(
        rect.x + 2,
        y + 2,
        (rect.width - 4).max(0),
        (rect.height - 4).max(0),
    ))));
    let value = &control.value;
    let rows = lines(rect, value);
    let height = line_height(rect);
    let selected = value.range();
    let caret_row = row_for(&rows, value.cursor);
    let available_rows = ((rect.height - 4) / height).max(1) as usize + 1;
    if value.chars.is_empty() && !focused {
        if let Some(font) = &rect.text_font {
            font.shape(&font.fit(placeholder, (rect.width - 12).max(0)))
                .draw(canvas, rect.x + 6, y + 4, 0x75818b);
        } else {
            let cells = ((rect.width - 12) / (GLYPH_ADVANCE * rect.scale).max(1)).max(0) as usize;
            canvas.text(
                rect.x + 6,
                y + 4,
                &fit_text(placeholder, cells),
                rect.scale,
                0x75818b,
            );
        }
    }
    for (index, row) in rows
        .iter()
        .enumerate()
        .skip(value.offset)
        .take(available_rows)
    {
        let top = y + 4 + (index - value.offset) as i32 * height;
        let text = row_text(value, row);
        let first = selected.start.max(row.start).min(row.end);
        let last = selected.end.max(row.start).min(row.end);
        if focused && first < last {
            let left = x_at(rect, value, row, first);
            let right = x_at(rect, value, row, last);
            canvas.fill_rect(
                rect.x + 6 + left,
                top,
                (right - left).max(1),
                height,
                0xb4d8fa,
            );
        }
        if let Some(font) = &rect.text_font {
            let shape = font.shape(&text);
            shape.draw(canvas, rect.x + 6, top, color);
            if focused && first < last {
                shape.draw_range(
                    canvas,
                    rect.x + 6,
                    top,
                    0x203847,
                    first - row.start,
                    last - row.start,
                );
            }
        } else {
            canvas.text(rect.x + 6, top, &text, rect.scale, color);
        }
        if focused && caret_row == index {
            canvas.fill_rect(
                rect.x + 6 + x_at(rect, value, row, value.cursor),
                top,
                1,
                height,
                color,
            );
        }
    }
    canvas.set_clip(Some(clip));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn textarea_wraps_without_changing_values_and_hit_testing_tracks_visible_rows() {
        let document = html::parse("<textarea id=q cols=3 rows=2></textarea>");
        let mut app = App::new();
        app.document = document;
        app.forms = Forms::new(&app.document);
        app.reflow();
        let node = *app.forms.controls.keys().next().unwrap();
        let rect = app
            .page
            .controls
            .iter()
            .find(|rect| rect.node == node)
            .unwrap()
            .clone();
        app.forms.script_set_value(node, "abc\ndef\nghi").unwrap();
        let value = &mut app.forms.controls.get_mut(&node).unwrap().value;
        let before = value.text();
        sync(&rect, value);
        assert!(value.offset > 0, "caret on last line must scroll into view");
        assert_eq!(
            value.text(),
            before,
            "visual wrapping must not edit form data"
        );
        let rows = lines(&rect, value);
        let at = character_at(&rect, value, rect.x + 6, rect.y + 4);
        assert_eq!(at, rows[value.offset].start);
        value.move_to(0, false);
        sync(&rect, value);
        assert_eq!(value.offset, 0);
        assert_eq!(
            character_at(&rect, value, rect.x + 6, rect.y + 4 + line_height(&rect)),
            rows[1].start
        );
    }
}
