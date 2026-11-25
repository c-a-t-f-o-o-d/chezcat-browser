//! Native button/submit/reset painting, shared by App and pixel witnesses.
//! CSS images use the existing bounded background sampler and resource owner;
//! the native border, focus mark and caption remain foreground layers.
use super::{Canvas, ClipRect, GLYPH_ADVANCE, GLYPH_HEIGHT};
use crate::{forms::Control, image::Bitmap, layout::ControlBox};

pub(crate) fn draw_native_button(
    canvas: &mut Canvas,
    rect: &ControlBox,
    y: i32,
    control: &Control,
    focused: bool,
    bitmap: Option<&Bitmap>,
) {
    let (x, width, height, scale) = (rect.x, rect.width, rect.height, rect.scale);
    if width <= 0 || height <= 0 {
        return;
    }
    let color = if control.disabled { 0x78858e } else { rect.color };
    let background = if control.disabled {
        0xe8edf0
    } else {
        rect.background.unwrap_or(0xe3edf2)
    };
    canvas.fill_rect(x, y, width, height, background);
    if let (Some(background), Some(bitmap)) = (&rect.background_image, bitmap) {
        // The existing native frame is one physical pixel, including at zoom.
        // Its inner padding box is the positioning area; caption padding does
        // not shrink that area. The final frame covers any image under it.
        canvas.background_image(
            ClipRect::new(x, y, width, height),
            ClipRect::new(x + 1, y + 1, (width - 2).max(0), (height - 2).max(0)),
            bitmap,
            background.presentation,
        );
    }
    let border = if focused { 0x178b78 } else { 0x7c8b96 };
    canvas.fill_rect(x, y, width, 1, border);
    canvas.fill_rect(x, y, 1, height, border);
    canvas.fill_rect(x, y + height - 1, width, 1, border);
    canvas.fill_rect(x + width - 1, y, 1, height, border);
    if focused {
        canvas.fill_rect(x, y + height - 2.min(height), width, 2.min(height), border);
    }

    let padding = rect.padding;
    let (x, y, width, height) = (
        x.saturating_add(padding[3]),
        y.saturating_add(padding[0]),
        (width - padding[1] - padding[3]).max(0),
        (height - padding[0] - padding[2]).max(0),
    );
    if let Some(font) = &rect.text_font {
        let clip = canvas.clip();
        canvas.set_clip(Some(clip.intersect(ClipRect::new(
            x + 2,
            y + 2,
            (width - 4).max(0),
            (height - 4).max(0),
        ))));
        let caption = font.fit(&control.caption, (width - 12).max(0));
        let shape = font.shape(&caption);
        shape.draw(canvas, x + (width - shape.width()) / 2, y + (height - shape.height()) / 2, color);
        canvas.set_clip(Some(clip));
        return;
    }
    if height < GLYPH_HEIGHT * scale + 2 {
        return;
    }
    let clip = canvas.clip();
    if padding != [0; 4] {
        canvas.set_clip(Some(clip.intersect(ClipRect::new(
            x + 2,
            y + 2,
            (width - 4).max(0),
            (height - 4).max(0),
        ))));
    }
    let advance = GLYPH_ADVANCE * scale;
    let cells = ((width - 12) / advance).max(0) as usize;
    let caption = fit_text(&control.caption, cells);
    let text_x = x + (width - caption.chars().count() as i32 * advance) / 2;
    canvas.text(text_x, y + (height - GLYPH_HEIGHT * scale) / 2, &caption, scale, color);
    canvas.set_clip(Some(clip));
}

fn fit_text(text: &str, max_chars: usize) -> String {
    let mut chars = text.chars();
    let mut result: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() && max_chars >= 3 {
        result = result.chars().take(max_chars - 3).collect();
        result.push_str("...");
    }
    result
}
