//! Finite original paint values shared by document cascade and SVG renderer.
use super::*;
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PaintValues {
    pub(super) color: Option<u32>,
    pub(super) fill: Paint,
    pub(super) stroke: Paint,
}
impl PaintValues {
    pub(crate) fn new(color: u32) -> Self {
        Self {
            color: Some(color),
            fill: Paint::Solid(0xff000000),
            stroke: Paint::None,
        }
    }
    pub(crate) fn unavailable() -> Self {
        Self {
            color: None,
            fill: Paint::Unavailable,
            stroke: Paint::Unavailable,
        }
    }
    pub(crate) fn property(name: &str) -> bool {
        matches!(name, "color" | "fill" | "stroke")
    }
    pub(crate) fn apply(&mut self, parent: &Self, key: &str, text: &str, initial_color: u32) {
        let text = text.trim();
        let lower = text.to_ascii_lowercase();
        if matches!(lower.as_str(), "inherit" | "unset") {
            match key {
                "color" => self.color = parent.color,
                "fill" => self.fill = parent.fill.clone(),
                "stroke" => self.stroke = parent.stroke.clone(),
                _ => {}
            };
            return;
        }
        if lower == "initial" {
            let initial = Self::new(initial_color);
            match key {
                "color" => self.color = initial.color,
                "fill" => self.fill = initial.fill,
                "stroke" => self.stroke = initial.stroke,
                _ => {}
            };
            return;
        }
        if key == "color" {
            self.color = if lower == "currentcolor" {
                parent.color
            } else {
                color(text).ok()
            };
            return;
        }
        let value = if lower == "none" {
            Paint::None
        } else if lower == "currentcolor" {
            Paint::Current
        } else if lower.starts_with("url(") {
            paint(&format!("url{}", &text[3..])).unwrap_or(Paint::Unavailable)
        } else {
            paint(text).unwrap_or(Paint::Unavailable)
        };
        match key {
            "fill" => self.fill = value,
            "stroke" => self.stroke = value,
            _ => {}
        }
    }
}
