//! Original static SVG image renderer. XML, geometry, curve flattening,
//! scan conversion and compositing are implemented here without an SVG library.
//! SVG images cannot execute scripts, resolve entities, or fetch resources.
//! References: https://www.w3.org/TR/SVG2/ and SVG path implementation notes.
//! Static clipping/masking follows https://www.w3.org/TR/css-masking-1/.

use super::{dimensions, pixel, reserved, Bitmap, ImageResult};
use std::collections::HashMap;
use std::sync::Arc;

const MAX_SOURCE: usize = 1024 * 1024;
const MAX_NODES: usize = 4096;
const MAX_DEPTH: usize = 64;
const MAX_SEGMENTS: usize = 32_768;
const MAX_WORK: usize = 64 * 1024 * 1024;
const MAX_LAYERS: usize = 8 * 1024 * 1024; // Pixels shared by opacity, clipping and mask groups.
const SAMPLES: usize = 4;

type Point = (f64, f64);
type Attributes = Vec<(String, String)>;

#[derive(Clone)]
struct Node {
    name: String,
    attrs: Attributes,
    children: Vec<usize>,
    text: String,
}

impl Node {
    fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }
}

struct Work<'a> {
    left: usize,
    since_check: usize,
    segments: usize,
    clips: usize,
    layers: usize,
    check: &'a dyn Fn() -> ImageResult<()>,
}

impl Work<'_> {
    fn tick(&mut self, count: usize) -> ImageResult<()> {
        self.left = self
            .left
            .checked_sub(count)
            .ok_or("SVG rendering work limit reached.")?;
        self.since_check += count;
        if self.since_check >= 8192 {
            (self.check)()?;
            self.since_check = 0;
        }
        Ok(())
    }
    fn segment(&mut self) -> ImageResult<()> {
        self.segments += 1;
        if self.segments > MAX_SEGMENTS {
            return Err("SVG path segment limit reached (32768).".into());
        }
        self.tick(1)
    }
}

pub(super) fn looks_like(data: &[u8]) -> bool {
    // A byte limit must not cut a UTF-8 sequence in half. The outer image
    // loader already caps input; the SVG decoder applies its tighter limit.
    let Ok(text) = std::str::from_utf8(data) else {
        return false;
    };
    let mut rest = text.trim_start_matches('\u{feff}').trim_start();
    loop {
        if rest.starts_with("<svg") {
            return rest
                .as_bytes()
                .get(4)
                .is_some_and(|c| c.is_ascii_whitespace() || *c == b'>' || *c == b'/');
        }
        let ending = if rest.starts_with("<?xml") {
            "?>"
        } else if rest.starts_with("<!--") {
            "-->"
        } else {
            return rest.starts_with("<!DOCTYPE svg");
        };
        let Some(end) = rest.find(ending) else {
            return false;
        };
        rest = rest[end + ending.len()..].trim_start();
    }
}

fn xml_name(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b':' | b'.')
}

fn entity_text(text: &str) -> ImageResult<String> {
    let mut output = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        output.push_str(&rest[..at]);
        rest = &rest[at + 1..];
        let end = rest
            .find(';')
            .filter(|&n| n <= 16)
            .ok_or("Invalid SVG XML character reference.")?;
        let name = &rest[..end];
        let scalar = match name {
            "amp" => '&',
            "lt" => '<',
            "gt" => '>',
            "quot" => '"',
            "apos" => '\'',
            _ => {
                let number = if let Some(value) = name.strip_prefix("#x") {
                    u32::from_str_radix(value, 16).ok()
                } else if let Some(value) = name.strip_prefix('#') {
                    value.parse::<u32>().ok()
                } else {
                    None
                };
                number
                    .and_then(char::from_u32)
                    .filter(|&c| c != '\0' && (!c.is_control() || matches!(c, '\n' | '\r' | '\t')))
                    .ok_or("SVG only supports built-in XML and Unicode character references.")?
            }
        };
        output.push(scalar);
        rest = &rest[end + 1..];
    }
    output.push_str(rest);
    Ok(output)
}

fn parse_xml(text: &str, work: &mut Work<'_>) -> ImageResult<Vec<Node>> {
    let text = text.trim_start_matches('\u{feff}');
    let bytes = text.as_bytes();
    let mut nodes = Vec::<Node>::new();
    let mut stack = Vec::<usize>::new();
    let mut at = 0usize;
    let mut declared = false;
    while at < bytes.len() {
        work.tick(1)?;
        if bytes[at] != b'<' {
            let end = at + text[at..].find('<').unwrap_or(bytes.len() - at);
            if let Some(&parent) = stack.last() {
                nodes[parent].text.push_str(&entity_text(&text[at..end])?);
            } else if !text[at..end].trim().is_empty() {
                return Err("SVG text occurs outside the root element.".into());
            }
            at = end;
            continue;
        }
        if text[at..].starts_with("<!--") {
            let end = text[at + 4..]
                .find("-->")
                .ok_or("Unterminated SVG XML comment.")?;
            if text[at + 4..at + 4 + end].contains("--") {
                return Err("Invalid SVG XML comment.".into());
            }
            at += 4 + end + 3;
            continue;
        }
        if text[at..].starts_with("<![CDATA[") {
            let end = text[at + 9..]
                .find("]]>")
                .ok_or("Unterminated SVG CDATA.")?;
            let parent = *stack
                .last()
                .ok_or("SVG CDATA needs a containing element.")?;
            nodes[parent].text.push_str(&text[at + 9..at + 9 + end]);
            at += 9 + end + 3;
            continue;
        }
        if text[at..].starts_with("<?") {
            if !nodes.is_empty()
                || declared
                || !text[at..].starts_with("<?xml")
                || !bytes.get(at + 5).is_some_and(u8::is_ascii_whitespace)
            {
                return Err("SVG processing instructions are not supported.".into());
            }
            declared = true;
            let end = text[at + 2..]
                .find("?>")
                .ok_or("Unterminated SVG XML declaration.")?;
            let declaration = &text[at + 2..at + 2 + end];
            if declaration.contains("encoding")
                && !declaration.to_ascii_lowercase().contains("utf-8")
            {
                return Err("SVG XML must use UTF-8.".into());
            }
            at += 2 + end + 2;
            continue;
        }
        if text[at..].starts_with("<!") {
            return Err("SVG document types and entity declarations are not supported.".into());
        }
        at += 1;
        let closing = bytes.get(at) == Some(&b'/');
        if closing {
            at += 1;
        }
        let start = at;
        while bytes.get(at).is_some_and(|&b| xml_name(b)) {
            at += 1;
        }
        if start == at || !bytes[start].is_ascii_alphabetic() {
            return Err("Invalid SVG XML element name.".into());
        }
        let name = &text[start..at];
        let mut attrs = Vec::new();
        let mut self_closing = false;
        loop {
            let separated = bytes.get(at).is_some_and(u8::is_ascii_whitespace);
            while bytes.get(at).is_some_and(u8::is_ascii_whitespace) {
                at += 1;
            }
            match bytes.get(at) {
                Some(b'>') => {
                    at += 1;
                    break;
                }
                Some(b'/') if !closing && bytes.get(at + 1) == Some(&b'>') => {
                    at += 2;
                    self_closing = true;
                    break;
                }
                None => return Err("Truncated SVG XML tag.".into()),
                _ if closing => return Err("Invalid SVG closing tag.".into()),
                _ => {}
            }
            if !separated {
                return Err("SVG XML attributes require whitespace separation.".into());
            }
            if attrs.len() >= 64 {
                return Err("SVG attribute limit reached (64 per element).".into());
            }
            let start = at;
            while bytes.get(at).is_some_and(|&b| xml_name(b)) {
                at += 1;
            }
            if start == at {
                return Err("Invalid SVG XML attribute name.".into());
            }
            let key = &text[start..at];
            if attrs.iter().any(|(old, _)| old == key) {
                return Err("Duplicate SVG XML attribute.".into());
            }
            while bytes.get(at).is_some_and(u8::is_ascii_whitespace) {
                at += 1;
            }
            if bytes.get(at) != Some(&b'=') {
                return Err("SVG XML attributes require values.".into());
            }
            at += 1;
            while bytes.get(at).is_some_and(u8::is_ascii_whitespace) {
                at += 1;
            }
            let quote = *bytes
                .get(at)
                .filter(|&&b| matches!(b, b'\'' | b'"'))
                .ok_or("SVG attribute values must be quoted.")?;
            at += 1;
            let start = at;
            while bytes.get(at).is_some_and(|&b| b != quote) {
                if bytes[at] == b'<' {
                    return Err("Invalid '<' in SVG attribute.".into());
                }
                at += 1;
            }
            if at == bytes.len() {
                return Err("Truncated SVG attribute value.".into());
            }
            attrs.push((key.to_owned(), entity_text(&text[start..at])?));
            at += 1;
        }
        if closing {
            let index = stack.pop().ok_or("Unexpected SVG closing tag.")?;
            if nodes[index].name != name {
                return Err("Mismatched SVG XML closing tag.".into());
            }
        } else {
            if nodes.len() >= MAX_NODES || stack.len() >= MAX_DEPTH {
                return Err("SVG XML node or nesting limit reached.".into());
            }
            if stack.is_empty() && !nodes.is_empty() {
                return Err("SVG needs exactly one root element.".into());
            }
            let index = nodes.len();
            nodes.push(Node {
                name: name.into(),
                attrs,
                children: Vec::new(),
                text: String::new(),
            });
            if let Some(&parent) = stack.last() {
                nodes[parent].children.push(index);
            }
            if !self_closing {
                stack.push(index);
            }
        }
    }
    if !stack.is_empty() || nodes.first().is_none_or(|n| n.name != "svg") {
        return Err("SVG root element is missing or unclosed.".into());
    }
    if nodes[0]
        .attr("xmlns")
        .is_some_and(|value| value != "http://www.w3.org/2000/svg")
    {
        return Err("Unsupported SVG namespace.".into());
    }
    Ok(nodes)
}

#[derive(Clone, Copy)]
struct Matrix([f64; 6]);
impl Matrix {
    const ID: Self = Self([1., 0., 0., 1., 0., 0.]);
    fn point(self, p: Point) -> Point {
        let [a, b, c, d, e, f] = self.0;
        (a * p.0 + c * p.1 + e, b * p.0 + d * p.1 + f)
    }
    fn multiply(self, other: Self) -> ImageResult<Self> {
        let [a, b, c, d, e, f] = self.0;
        let [g, h, i, j, k, l] = other.0;
        let value = Self([
            a * g + c * h,
            b * g + d * h,
            a * i + c * j,
            b * i + d * j,
            a * k + c * l + e,
            b * k + d * l + f,
        ]);
        if value.0.iter().any(|v| !v.is_finite() || v.abs() > 1e12) {
            return Err("SVG transform exceeds its finite-coordinate limit.".into());
        }
        Ok(value)
    }
    fn inverse(self) -> Option<Self> {
        let [a, b, c, d, e, f] = self.0;
        let det = a * d - b * c;
        if !det.is_finite() || det.abs() < 1e-16 {
            return None;
        }
        Some(Self([
            d / det,
            -b / det,
            -c / det,
            a / det,
            (c * f - d * e) / det,
            (b * e - a * f) / det,
        ]))
    }
}

struct Numbers<'a> {
    text: &'a str,
    at: usize,
}
impl<'a> Numbers<'a> {
    fn new(text: &'a str) -> Self {
        Self { text, at: 0 }
    }
    fn spaces(&mut self) {
        while self
            .text
            .as_bytes()
            .get(self.at)
            .is_some_and(|b| b.is_ascii_whitespace() || *b == b',')
        {
            self.at += 1;
        }
    }
    fn more(&mut self) -> bool {
        self.spaces();
        self.at < self.text.len()
    }
    fn number(&mut self) -> ImageResult<f64> {
        self.spaces();
        let start = self.at;
        let b = self.text.as_bytes();
        if b.get(self.at).is_some_and(|v| matches!(*v, b'+' | b'-')) {
            self.at += 1;
        }
        let mut digits = 0;
        while b.get(self.at).is_some_and(u8::is_ascii_digit) {
            self.at += 1;
            digits += 1;
        }
        if b.get(self.at) == Some(&b'.') {
            self.at += 1;
            while b.get(self.at).is_some_and(u8::is_ascii_digit) {
                self.at += 1;
                digits += 1;
            }
        }
        if digits == 0 {
            return Err("SVG expected a finite number.".into());
        }
        if b.get(self.at).is_some_and(|v| matches!(*v, b'e' | b'E')) {
            self.at += 1;
            if b.get(self.at).is_some_and(|v| matches!(*v, b'+' | b'-')) {
                self.at += 1;
            }
            let exponent = self.at;
            while b.get(self.at).is_some_and(u8::is_ascii_digit) {
                self.at += 1;
            }
            if exponent == self.at {
                return Err("Invalid SVG numeric exponent.".into());
            }
        }
        let value = self.text[start..self.at]
            .parse::<f64>()
            .map_err(|_| "Invalid SVG number.")?;
        if !value.is_finite() || value.abs() > 1e7 {
            return Err("SVG coordinate exceeds its finite 10-million-unit limit.".into());
        }
        Ok(value)
    }
    fn flag(&mut self) -> ImageResult<bool> {
        self.spaces();
        let value = self.text.as_bytes().get(self.at).copied();
        self.at += 1;
        match value {
            Some(b'0') => Ok(false),
            Some(b'1') => Ok(true),
            _ => Err("SVG arc flags must be 0 or 1.".into()),
        }
    }
    fn pair(&mut self) -> ImageResult<Point> {
        Ok((self.number()?, self.number()?))
    }
}

fn numbers(text: &str) -> ImageResult<Vec<f64>> {
    let mut reader = Numbers::new(text);
    let mut out = Vec::new();
    while reader.more() {
        if out.len() >= 16 {
            return Err("Too many SVG numeric parameters.".into());
        }
        out.push(reader.number()?);
    }
    Ok(out)
}

fn transform(text: Option<&str>) -> ImageResult<Matrix> {
    let Some(text) = text else {
        return Ok(Matrix::ID);
    };
    let mut reader = Numbers::new(text);
    let mut matrix = Matrix::ID;
    let mut count = 0;
    while reader.more() {
        count += 1;
        if count > 64 {
            return Err("SVG transform-list limit reached.".into());
        }
        let start = reader.at;
        while reader
            .text
            .as_bytes()
            .get(reader.at)
            .is_some_and(u8::is_ascii_alphabetic)
        {
            reader.at += 1;
        }
        let name = &text[start..reader.at];
        reader.spaces();
        if text.as_bytes().get(reader.at) != Some(&b'(') {
            return Err("Invalid SVG transform.".into());
        }
        reader.at += 1;
        let end = reader.at
            + text[reader.at..]
                .find(')')
                .ok_or("Unclosed SVG transform.")?;
        let values = numbers(&text[reader.at..end])?;
        reader.at = end + 1;
        let next = match (name, values.as_slice()) {
            ("matrix", [a, b, c, d, e, f]) => Matrix([*a, *b, *c, *d, *e, *f]),
            ("translate", [x]) => Matrix([1., 0., 0., 1., *x, 0.]),
            ("translate", [x, y]) => Matrix([1., 0., 0., 1., *x, *y]),
            ("scale", [x]) => Matrix([*x, 0., 0., *x, 0., 0.]),
            ("scale", [x, y]) => Matrix([*x, 0., 0., *y, 0., 0.]),
            ("rotate", [angle]) | ("rotate", [angle, _, _]) => {
                let angle = angle.to_radians();
                let (s, c) = angle.sin_cos();
                let (cx, cy) = if values.len() == 3 {
                    (values[1], values[2])
                } else {
                    (0., 0.)
                };
                Matrix([c, s, -s, c, cx - c * cx + s * cy, cy - s * cx - c * cy])
            }
            ("skewX", [angle]) => Matrix([1., 0., angle.to_radians().tan(), 1., 0., 0.]),
            ("skewY", [angle]) => Matrix([1., angle.to_radians().tan(), 0., 1., 0., 0.]),
            _ => return Err("Unsupported SVG transform or parameter count.".into()),
        };
        matrix = matrix.multiply(next)?;
    }
    Ok(matrix)
}

fn length(text: &str, reference: f64) -> ImageResult<f64> {
    let mut reader = Numbers::new(text);
    let value = reader.number()?;
    let unit = text[reader.at..].trim();
    let factor = match unit {
        "" | "px" => 1.,
        "%" => reference / 100.,
        "pt" => 96. / 72.,
        "pc" => 16.,
        "in" => 96.,
        "cm" => 96. / 2.54,
        "mm" => 96. / 25.4,
        "Q" | "q" => 96. / 101.6,
        _ => return Err("Unsupported SVG length unit.".into()),
    };
    let result = value * factor;
    if !result.is_finite() || result.abs() > 1e7 {
        return Err("SVG length exceeds its coordinate limit.".into());
    }
    Ok(result)
}

fn number_attr(node: &Node, key: &str, default: f64, reference: f64) -> ImageResult<f64> {
    node.attr(key).map_or(Ok(default), |v| length(v, reference))
}

#[derive(Clone)]
enum Paint {
    None,
    Solid(u32),
    Current,
    Server(Arc<str>),
}
#[derive(Clone, Copy)]
enum Cap {
    Butt,
    Round,
    Square,
}
#[derive(Clone, Copy)]
enum Join {
    Miter,
    Round,
    Bevel,
}

#[derive(Clone)]
struct Style {
    color: u32,
    fill: Paint,
    stroke: Paint,
    fill_alpha: f64,
    stroke_alpha: f64,
    opacity: f64,
    width: f64,
    even_odd: bool,
    clip_even_odd: bool,
    clip: Option<Arc<str>>,
    mask: Option<Arc<str>>,
    mask_alpha: bool,
    mask_mode: Option<bool>,
    cap: Cap,
    join: Join,
    miter: f64,
    visible: bool,
    display: bool,
    stop_color: u32,
    stop_current: bool,
    stop_alpha: f64,
}
impl Default for Style {
    fn default() -> Self {
        Self {
            color: 0xff000000,
            fill: Paint::Solid(0xff000000),
            stroke: Paint::None,
            fill_alpha: 1.,
            stroke_alpha: 1.,
            opacity: 1.,
            width: 1.,
            even_odd: false,
            clip_even_odd: false,
            clip: None,
            mask: None,
            mask_alpha: false,
            mask_mode: None,
            cap: Cap::Butt,
            join: Join::Miter,
            miter: 4.,
            visible: true,
            display: true,
            stop_color: 0xff000000,
            stop_current: false,
            stop_alpha: 1.,
        }
    }
}

fn color(text: &str) -> ImageResult<u32> {
    let value = text.trim().to_ascii_lowercase();
    if let Some(hex) = value.strip_prefix('#') {
        if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("Invalid SVG hexadecimal color.".into());
        }
        let digit =
            |s: &str| u8::from_str_radix(s, 16).map_err(|_| "Invalid SVG hexadecimal color.");
        return match hex.len() {
            3 | 4 => {
                let mut rgba = [0, 0, 0, 255];
                for (i, value) in rgba.iter_mut().enumerate().take(hex.len()) {
                    *value = digit(&hex[i..i + 1])? * 17;
                }
                Ok(pixel(rgba[0], rgba[1], rgba[2], rgba[3]))
            }
            6 | 8 => {
                let mut rgba = [0, 0, 0, 255];
                for (i, value) in rgba.iter_mut().enumerate().take(hex.len() / 2) {
                    *value = digit(&hex[i * 2..i * 2 + 2])?;
                }
                Ok(pixel(rgba[0], rgba[1], rgba[2], rgba[3]))
            }
            _ => Err("Invalid SVG hexadecimal color length.".into()),
        };
    }
    if value.starts_with("rgb(") || value.starts_with("rgba(") {
        let start = value.find('(').unwrap() + 1;
        let body = value[start..]
            .strip_suffix(')')
            .ok_or("Invalid SVG RGB color.")?;
        let parts: Vec<&str> = body
            .split(|c: char| c == ',' || c == '/' || c.is_ascii_whitespace())
            .filter(|v| !v.is_empty())
            .collect();
        if !(3..=4).contains(&parts.len()) {
            return Err("Invalid SVG RGB component count.".into());
        }
        let mut rgba = [0, 0, 0, 255];
        for (i, part) in parts.iter().enumerate() {
            let n = if i == 3 {
                length(part, 1.)? * 255.
            } else {
                length(part, 255.)?
            };
            rgba[i] = n.clamp(0., 255.).round() as u8;
        }
        return Ok(pixel(rgba[0], rgba[1], rgba[2], rgba[3]));
    }
    Ok(match value.as_str() {
        "transparent" => 0,
        "black" => 0xff000000,
        "white" => 0xffffffff,
        "red" => 0xffff0000,
        "green" => 0xff008000,
        "blue" => 0xff0000ff,
        "yellow" => 0xffffff00,
        "cyan" | "aqua" => 0xff00ffff,
        "magenta" | "fuchsia" => 0xffff00ff,
        "gray" | "grey" => 0xff808080,
        "silver" => 0xffc0c0c0,
        "maroon" => 0xff800000,
        "olive" => 0xff808000,
        "lime" => 0xff00ff00,
        "teal" => 0xff008080,
        "navy" => 0xff000080,
        "purple" => 0xff800080,
        "orange" => 0xffffa500,
        "rebeccapurple" => 0xff663399,
        _ => {
            return Err(format!(
                "Unsupported SVG color {text:?}. Use hexadecimal or RGB colors."
            ))
        }
    })
}

fn paint(text: &str) -> ImageResult<Paint> {
    let value = text.trim();
    match value {
        "none" => Ok(Paint::None),
        "currentColor" | "currentcolor" => Ok(Paint::Current),
        _ if value.starts_with("url(") => {
            let inner = value
                .strip_prefix("url(")
                .and_then(|v| v.strip_suffix(')'))
                .ok_or("Invalid SVG paint URL.")?
                .trim()
                .trim_matches(['\'', '"']);
            let id = crate::svg_reference::fragment(inner)
                .filter(|v| !v.is_empty() && !v.contains(char::is_whitespace))
                .ok_or("SVG paint servers must be local fragment references.")?;
            Ok(Paint::Server(id.into()))
        }
        _ => color(value).map(Paint::Solid),
    }
}

fn local_effect(text: &str) -> ImageResult<Option<Arc<str>>> {
    crate::svg_reference::effect(text).map(|id|id.map(Into::into)).map_err(Into::into)
}

fn declarations(text: &str) -> ImageResult<Vec<(String, String)>> {
    let mut out = Vec::new();
    for declaration in text.split(';') {
        let declaration = declaration.trim();
        if declaration.is_empty() {
            continue;
        }
        let (key, value) = declaration
            .split_once(':')
            .ok_or("Invalid SVG style declaration.")?;
        if value.contains('!') {
            return Err("SVG !important declarations are not implemented.".into());
        }
        if out.len() >= 64 {
            return Err("SVG style declaration limit reached.".into());
        }
        let key = key.trim().to_ascii_lowercase();
        if matches!(
            key.as_str(),
            "transform"
                | "transform-origin"
                | "transform-box"
                | "x"
                | "y"
                | "cx"
                | "cy"
                | "r"
                | "rx"
                | "ry"
                | "width"
                | "height"
                | "d"
        ) {
            return Err(format!(
                "SVG CSS geometry property {key:?} is not implemented; use attributes."
            ));
        }
        out.push((key, value.trim().into()));
    }
    Ok(out)
}

fn style_property(
    style: &mut Style,
    parent: &Style,
    key: &str,
    value: &str,
    viewport: Point,
) -> ImageResult<()> {
    if value == "inherit" {
        // Resolve explicit inheritance against the parent, not an earlier
        // declaration on this same element. Opacity/display do not otherwise inherit.
        match key {
            "color" => style.color = parent.color,
            "fill" => style.fill = parent.fill.clone(),
            "stroke" => style.stroke = parent.stroke.clone(),
            "opacity" => style.opacity = parent.opacity,
            "fill-opacity" => style.fill_alpha = parent.fill_alpha,
            "stroke-opacity" => style.stroke_alpha = parent.stroke_alpha,
            "stroke-width" => style.width = parent.width,
            "fill-rule" => style.even_odd = parent.even_odd,
            "clip-rule" => style.clip_even_odd = parent.clip_even_odd,
            "clip-path" => style.clip = parent.clip.clone(),
            "mask" => {
                style.mask = parent.mask.clone();
                style.mask_mode = parent.mask_mode;
            }
            "mask-image" => style.mask = parent.mask.clone(),
            "mask-type" => style.mask_alpha = parent.mask_alpha,
            "mask-mode" => style.mask_mode = parent.mask_mode,
            "stroke-linecap" => style.cap = parent.cap,
            "stroke-linejoin" => style.join = parent.join,
            "stroke-miterlimit" => style.miter = parent.miter,
            "display" => style.display = parent.display,
            "visibility" => style.visible = parent.visible,
            "stop-color" => {
                style.stop_color = parent.stop_color;
                style.stop_current = parent.stop_current;
            }
            "stop-opacity" => style.stop_alpha = parent.stop_alpha,
            _ => {}
        }
        return Ok(());
    }
    match key {
        "color" => {
            style.color = if value.eq_ignore_ascii_case("currentcolor") {
                parent.color
            } else {
                color(value)?
            };
        }
        "fill" => style.fill = paint(value)?,
        "stroke" => style.stroke = paint(value)?,
        "opacity" => style.opacity = length(value, 1.)?.clamp(0., 1.),
        "fill-opacity" => style.fill_alpha = length(value, 1.)?.clamp(0., 1.),
        "stroke-opacity" => style.stroke_alpha = length(value, 1.)?.clamp(0., 1.),
        "stroke-width" => {
            style.width = length(value, viewport.0.hypot(viewport.1) / 2f64.sqrt())?;
            if style.width < 0. {
                return Err("SVG stroke width cannot be negative.".into());
            }
        }
        "fill-rule" => {
            style.even_odd = match value {
                "nonzero" => false,
                "evenodd" => true,
                _ => return Err("Unsupported SVG fill rule.".into()),
            }
        }
        "clip-rule" => {
            style.clip_even_odd = match value {
                "nonzero" => false,
                "evenodd" => true,
                _ => return Err("Unsupported SVG clip rule.".into()),
            }
        }
        "clip-path" => style.clip = local_effect(value)?,
        "mask" | "mask-image" => {
            style.mask = local_effect(value)?;
            if key == "mask" {
                style.mask_mode = None;
            }
        }
        "mask-type" => {
            style.mask_alpha = match value {
                "alpha" => true,
                "luminance" => false,
                _ => return Err("Unsupported SVG mask type.".into()),
            }
        }
        "mask-mode" => {
            style.mask_mode = match value {
                "match-source" => None,
                "alpha" => Some(true),
                "luminance" => Some(false),
                _ => return Err("Unsupported SVG mask mode.".into()),
            }
        }
        "stroke-linecap" => {
            style.cap = match value {
                "butt" => Cap::Butt,
                "round" => Cap::Round,
                "square" => Cap::Square,
                _ => return Err("Unsupported SVG line cap.".into()),
            }
        }
        "stroke-linejoin" => {
            style.join = match value {
                "miter" => Join::Miter,
                "round" => Join::Round,
                "bevel" => Join::Bevel,
                _ => return Err("Unsupported SVG line join.".into()),
            }
        }
        "stroke-miterlimit" => {
            style.miter = length(value, 1.)?;
            if style.miter < 1. {
                return Err("SVG miter limit must be at least one.".into());
            }
        }
        "display" => style.display = value != "none",
        "visibility" => {
            style.visible = match value {
                "visible" => true,
                "hidden" | "collapse" => false,
                _ => return Err("Unsupported SVG visibility.".into()),
            }
        }
        "stop-color" => {
            style.stop_current = value.eq_ignore_ascii_case("currentcolor");
            if !style.stop_current {
                style.stop_color = color(value)?;
            }
        }
        "stop-opacity" => style.stop_alpha = length(value, 1.)?.clamp(0., 1.),
        "stroke-dasharray" if value == "none" => {}
        "filter" if value == "none" => {}
        "vector-effect" if value == "none" => {}
        "color-interpolation" if matches!(value, "sRGB" | "srgb") => {}
        "stroke-dasharray"
        | "stroke-dashoffset"
        | "filter"
        | "mask-composite"
        | "mask-position"
        | "mask-size"
        | "mask-repeat"
        | "mask-origin"
        | "mask-clip"
        | "vector-effect"
        | "paint-order"
        | "marker-start"
        | "marker-mid"
        | "marker-end"
        | "mix-blend-mode"
        | "color-interpolation" => return Err(format!("SVG property {key:?} is not implemented.")),
        _ => {}
    }
    Ok(())
}

struct Rule {
    selector: String,
    specificity: (usize, usize, usize),
    properties: Vec<(String, String)>,
}
fn selector_match(selector: &str, node: &Node) -> bool {
    let mut at = 0;
    let bytes = selector.as_bytes();
    if bytes.first().is_some_and(|b| *b != b'.' && *b != b'#') {
        while at < bytes.len() && !matches!(bytes[at], b'.' | b'#') {
            at += 1;
        }
        if &selector[..at] != "*" && selector[..at] != node.name {
            return false;
        }
    }
    while at < bytes.len() {
        let kind = bytes[at];
        at += 1;
        let start = at;
        while at < bytes.len() && !matches!(bytes[at], b'.' | b'#') {
            at += 1;
        }
        let name = &selector[start..at];
        let matches = if kind == b'#' {
            node.attr("id") == Some(name)
        } else {
            node.attr("class")
                .is_some_and(|v| v.split_ascii_whitespace().any(|v| v == name))
        };
        if !matches {
            return false;
        }
    }
    true
}

fn rules(nodes: &[Node]) -> ImageResult<Vec<Rule>> {
    let mut out = Vec::new();
    let mut bytes = 0usize;
    for node in nodes.iter().filter(|n| n.name == "style") {
        let mut text = node.text.as_str();
        while !text.trim().is_empty() {
            text = text.trim_start();
            if text.starts_with("/*") {
                let end = text[2..].find("*/").ok_or("Unclosed SVG CSS comment.")?;
                text = &text[end + 4..];
                continue;
            }
            let (open, body) = text.split_once('{').ok_or("Invalid SVG stylesheet.")?;
            let (close, rest) = body
                .split_once('}')
                .ok_or("Unclosed SVG stylesheet rule.")?;
            let properties = declarations(close)?;
            for selector in open.split(',') {
                let selector = selector.trim();
                if selector.is_empty()
                    || selector.bytes().any(|c| {
                        !(c.is_ascii_alphanumeric()
                            || matches!(c, b'-' | b'_' | b'.' | b'#' | b'*'))
                    })
                {
                    return Err(
                        "SVG stylesheets support simple element, class and id selectors only."
                            .into(),
                    );
                }
                if out.len() >= 512 {
                    return Err("SVG stylesheet rule limit reached.".into());
                }
                let specificity = (
                    selector.matches('#').count(),
                    selector.matches('.').count(),
                    usize::from(!selector.starts_with(['.', '#', '*'])),
                );
                bytes += selector.len()
                    + properties
                        .iter()
                        .map(|(a, b)| a.len() + b.len())
                        .sum::<usize>();
                if bytes > 4 * MAX_SOURCE {
                    return Err("SVG expanded stylesheet limit reached (4 MiB).".into());
                }
                out.push(Rule {
                    selector: selector.into(),
                    specificity,
                    properties: properties.clone(),
                });
            }
            text = rest;
        }
    }
    out.sort_by_key(|r| r.specificity);
    Ok(out)
}

fn computed(node: &Node, parent: &Style, rules: &[Rule], viewport: Point) -> ImageResult<Style> {
    let mut style = parent.clone();
    style.opacity = 1.;
    style.display = true;
    style.clip = None;
    style.mask = None;
    style.mask_alpha = false;
    style.mask_mode = None;
    style.stop_color = 0xff000000;
    style.stop_current = false;
    style.stop_alpha = 1.;
    for (key, value) in &node.attrs {
        if key.starts_with("on") {
            return Err("SVG event handlers are not executed in image documents.".into());
        }
        style_property(&mut style, parent, key, value, viewport)?;
    }
    for rule in rules {
        if selector_match(&rule.selector, node) {
            for (key, value) in &rule.properties {
                style_property(&mut style, parent, key, value, viewport)?;
            }
        }
    }
    if let Some(inline) = node.attr("style") {
        for (key, value) in declarations(inline)? {
            style_property(&mut style, parent, &key, &value, viewport)?;
        }
    }
    Ok(style)
}

#[derive(Clone)]
struct Contour {
    points: Vec<Point>,
    closed: bool,
}
type Path = Vec<Contour>;

fn add(contour: &mut Contour, point: Point, work: &mut Work<'_>) -> ImageResult<()> {
    if !point.0.is_finite() || !point.1.is_finite() || point.0.abs() > 1e9 || point.1.abs() > 1e9 {
        return Err("SVG path exceeds its finite-coordinate limit.".into());
    }
    work.segment()?;
    contour.points.push(point);
    Ok(())
}
fn mid(a: Point, b: Point) -> Point {
    ((a.0 + b.0) * 0.5, (a.1 + b.1) * 0.5)
}
fn distance_to_line(p: Point, a: Point, b: Point) -> f64 {
    let dx = b.0 - a.0;
    let dy = b.1 - a.1;
    let length = dx.hypot(dy);
    if length < 1e-12 {
        (p.0 - a.0).hypot(p.1 - a.1)
    } else {
        ((p.0 - a.0) * dy - (p.1 - a.1) * dx).abs() / length
    }
}

fn cubic(
    contour: &mut Contour,
    points: [Point; 4],
    matrix: Matrix,
    work: &mut Work<'_>,
) -> ImageResult<()> {
    let mut stack = vec![(points, 0usize)];
    while let Some((p, depth)) = stack.pop() {
        work.tick(1)?;
        let q = p.map(|v| matrix.point(v));
        let chord = (q[3].0 - q[0].0).hypot(q[3].1 - q[0].1);
        let polygon = q
            .windows(2)
            .map(|p| (p[1].0 - p[0].0).hypot(p[1].1 - p[0].1))
            .sum::<f64>();
        if distance_to_line(q[1], q[0], q[3]).max(distance_to_line(q[2], q[0], q[3])) <= 0.15
            && polygon <= chord + 0.3
        {
            add(contour, p[3], work)?;
            continue;
        }
        if depth >= 16 {
            return Err("SVG curve subdivision limit reached.".into());
        }
        let a = mid(p[0], p[1]);
        let b = mid(p[1], p[2]);
        let c = mid(p[2], p[3]);
        let d = mid(a, b);
        let e = mid(b, c);
        let f = mid(d, e);
        stack.push(([f, e, c, p[3]], depth + 1));
        stack.push(([p[0], a, d, f], depth + 1));
    }
    Ok(())
}

fn arc_steps(radius: f64, angle: f64, matrix: Matrix) -> ImageResult<usize> {
    let scale =
        (matrix.0[0].powi(2) + matrix.0[1].powi(2) + matrix.0[2].powi(2) + matrix.0[3].powi(2))
            .sqrt();
    let radius = radius * scale;
    let step = if radius <= 0.15 {
        std::f64::consts::PI / 2.
    } else {
        2. * (1. - 0.15 / radius).clamp(-1., 1.).acos()
    };
    let count = (angle.abs() / step).ceil().max(1.);
    if !count.is_finite() || count > 8192. {
        return Err("SVG arc subdivision limit reached.".into());
    }
    Ok(count as usize)
}

// Parameters follow the SVG endpoint-arc representation.
#[allow(clippy::too_many_arguments)]
fn arc(
    contour: &mut Contour,
    from: Point,
    to: Point,
    radii: Point,
    rotation: f64,
    large: bool,
    sweep: bool,
    matrix: Matrix,
    work: &mut Work<'_>,
) -> ImageResult<()> {
    let (mut rx, mut ry) = (radii.0.abs(), radii.1.abs());
    if from == to {
        return Ok(());
    }
    if rx == 0. || ry == 0. {
        return add(contour, to, work);
    }
    let phi = rotation.to_radians();
    let (s, c) = phi.sin_cos();
    let dx = (from.0 - to.0) * 0.5;
    let dy = (from.1 - to.1) * 0.5;
    let xp = c * dx + s * dy;
    let yp = -s * dx + c * dy;
    let lambda = (xp / rx).powi(2) + (yp / ry).powi(2);
    if lambda > 1. {
        let factor = lambda.sqrt();
        rx *= factor;
        ry *= factor;
    }
    let rx2 = rx * rx;
    let ry2 = ry * ry;
    let denominator = rx2 * yp * yp + ry2 * xp * xp;
    let factor = if denominator == 0. {
        0.
    } else {
        ((rx2 * ry2 - denominator) / denominator).max(0.).sqrt()
            * if large == sweep { -1. } else { 1. }
    };
    let cxp = factor * rx * yp / ry;
    let cyp = -factor * ry * xp / rx;
    let center = (
        c * cxp - s * cyp + (from.0 + to.0) * 0.5,
        s * cxp + c * cyp + (from.1 + to.1) * 0.5,
    );
    let u = ((xp - cxp) / rx, (yp - cyp) / ry);
    let v = ((-xp - cxp) / rx, (-yp - cyp) / ry);
    let begin = u.1.atan2(u.0);
    let mut delta = (u.0 * v.1 - u.1 * v.0).atan2(u.0 * v.0 + u.1 * v.1);
    if sweep && delta < 0. {
        delta += std::f64::consts::TAU;
    } else if !sweep && delta > 0. {
        delta -= std::f64::consts::TAU;
    }
    let count = arc_steps(rx.max(ry), delta, matrix)?;
    for i in 1..=count {
        let angle = begin + delta * i as f64 / count as f64;
        let (sa, ca) = angle.sin_cos();
        let point = if i == count {
            to
        } else {
            (
                center.0 + c * rx * ca - s * ry * sa,
                center.1 + s * rx * ca + c * ry * sa,
            )
        };
        add(contour, point, work)?;
    }
    Ok(())
}

fn parse_path(text: &str, matrix: Matrix, work: &mut Work<'_>) -> ImageResult<Path> {
    let mut reader = Numbers::new(text);
    let mut path = Path::new();
    let mut current = (0., 0.);
    let mut command = b' ';
    let mut previous = b' ';
    let (mut cubic_control, mut quadratic_control) = ((0., 0.), (0., 0.));
    while reader.more() {
        work.tick(1)?;
        if text.as_bytes()[reader.at].is_ascii_alphabetic() {
            command = text.as_bytes()[reader.at];
            reader.at += 1;
        }
        let upper = command.to_ascii_uppercase();
        let relative = command.is_ascii_lowercase();
        let offset = if relative { current } else { (0., 0.) };
        let point = |p: Point| (p.0 + offset.0, p.1 + offset.1);
        if upper == b'M' {
            let p = point(reader.pair()?);
            let mut contour = Contour {
                points: Vec::new(),
                closed: false,
            };
            add(&mut contour, p, work)?;
            path.push(contour);
            current = p;
            previous = b'M';
            command = if relative { b'l' } else { b'L' };
            continue;
        }
        if previous == b'Z' && upper != b'Z' {
            let mut contour = Contour {
                points: Vec::new(),
                closed: false,
            };
            add(&mut contour, current, work)?;
            path.push(contour);
        }
        let contour = path.last_mut().ok_or("SVG path must begin with moveto.")?;
        match upper {
            b'Z' => {
                contour.closed = true;
                current = contour.points[0];
                command = b' ';
            }
            b'L' => {
                current = point(reader.pair()?);
                add(contour, current, work)?;
            }
            b'H' => {
                current.0 = reader.number()? + offset.0;
                add(contour, current, work)?;
            }
            b'V' => {
                current.1 = reader.number()? + offset.1;
                add(contour, current, work)?;
            }
            b'C' => {
                let a = point(reader.pair()?);
                let b = point(reader.pair()?);
                let end = point(reader.pair()?);
                cubic(contour, [current, a, b, end], matrix, work)?;
                cubic_control = b;
                current = end;
            }
            b'S' => {
                let a = if matches!(previous, b'C' | b'S') {
                    (
                        2. * current.0 - cubic_control.0,
                        2. * current.1 - cubic_control.1,
                    )
                } else {
                    current
                };
                let b = point(reader.pair()?);
                let end = point(reader.pair()?);
                cubic(contour, [current, a, b, end], matrix, work)?;
                cubic_control = b;
                current = end;
            }
            b'Q' | b'T' => {
                let q = if upper == b'Q' {
                    point(reader.pair()?)
                } else if matches!(previous, b'Q' | b'T') {
                    (
                        2. * current.0 - quadratic_control.0,
                        2. * current.1 - quadratic_control.1,
                    )
                } else {
                    current
                };
                let end = point(reader.pair()?);
                let a = (
                    current.0 + (q.0 - current.0) * 2. / 3.,
                    current.1 + (q.1 - current.1) * 2. / 3.,
                );
                let b = (
                    end.0 + (q.0 - end.0) * 2. / 3.,
                    end.1 + (q.1 - end.1) * 2. / 3.,
                );
                cubic(contour, [current, a, b, end], matrix, work)?;
                quadratic_control = q;
                current = end;
            }
            b'A' => {
                let radii = reader.pair()?;
                let angle = reader.number()?;
                let large = reader.flag()?;
                let sweep = reader.flag()?;
                let end = point(reader.pair()?);
                arc(
                    contour, current, end, radii, angle, large, sweep, matrix, work,
                )?;
                current = end;
            }
            _ => return Err("Unsupported SVG path command or missing parameters.".into()),
        }
        previous = upper;
    }
    Ok(path)
}

fn ellipse(
    center: Point,
    radii: Point,
    matrix: Matrix,
    work: &mut Work<'_>,
) -> ImageResult<Contour> {
    let count = arc_steps(radii.0.max(radii.1), std::f64::consts::TAU, matrix)?.max(8);
    let mut contour = Contour {
        points: Vec::new(),
        closed: true,
    };
    for i in 0..count {
        let a = std::f64::consts::TAU * i as f64 / count as f64;
        add(
            &mut contour,
            (center.0 + radii.0 * a.cos(), center.1 + radii.1 * a.sin()),
            work,
        )?;
    }
    Ok(contour)
}

fn shape(node: &Node, matrix: Matrix, viewport: Point, work: &mut Work<'_>) -> ImageResult<Path> {
    let x = |key, default| number_attr(node, key, default, viewport.0);
    let y = |key, default| number_attr(node, key, default, viewport.1);
    let mut contour = Contour {
        points: Vec::new(),
        closed: false,
    };
    match node.name.as_str() {
        "path" => return parse_path(node.attr("d").unwrap_or(""), matrix, work),
        "rect" => {
            let (x, y, w, h) = (x("x", 0.)?, y("y", 0.)?, x("width", 0.)?, y("height", 0.)?);
            if w < 0. || h < 0. {
                return Err("SVG rectangle dimensions cannot be negative.".into());
            }
            if w == 0. || h == 0. {
                return Ok(Vec::new());
            }
            let rx = node.attr("rx").map(|v| length(v, viewport.0)).transpose()?;
            let ry = node.attr("ry").map(|v| length(v, viewport.1)).transpose()?;
            let (rx, ry) = (
                rx.or(ry).unwrap_or(0.).min(w / 2.),
                ry.or(rx).unwrap_or(0.).min(h / 2.),
            );
            if rx < 0. || ry < 0. {
                return Err("SVG rectangle radii cannot be negative.".into());
            }
            if rx == 0. || ry == 0. {
                for p in [(x, y), (x + w, y), (x + w, y + h), (x, y + h)] {
                    add(&mut contour, p, work)?;
                }
            } else {
                add(&mut contour, (x + rx, y), work)?;
                for (a, b) in [
                    ((x + w - rx, y), (x + w, y + ry)),
                    ((x + w, y + h - ry), (x + w - rx, y + h)),
                    ((x + rx, y + h), (x, y + h - ry)),
                    ((x, y + ry), (x + rx, y)),
                ] {
                    add(&mut contour, a, work)?;
                    arc(&mut contour, a, b, (rx, ry), 0., false, true, matrix, work)?;
                }
            }
            contour.closed = true;
        }
        "circle" | "ellipse" => {
            let center = (x("cx", 0.)?, y("cy", 0.)?);
            let radii = if node.name == "circle" {
                let r = number_attr(node, "r", 0., viewport.0.hypot(viewport.1) / 2f64.sqrt())?;
                (r, r)
            } else {
                (x("rx", 0.)?, y("ry", 0.)?)
            };
            if radii.0 < 0. || radii.1 < 0. {
                return Err("SVG radii cannot be negative.".into());
            }
            if radii.0 == 0. || radii.1 == 0. {
                return Ok(Vec::new());
            }
            contour = ellipse(center, radii, matrix, work)?;
        }
        "line" => {
            add(&mut contour, (x("x1", 0.)?, y("y1", 0.)?), work)?;
            add(&mut contour, (x("x2", 0.)?, y("y2", 0.)?), work)?;
        }
        "polyline" | "polygon" => {
            let mut values = Numbers::new(node.attr("points").unwrap_or(""));
            while values.more() {
                add(&mut contour, values.pair()?, work)?;
            }
            contour.closed = node.name == "polygon";
        }
        _ => return Err(format!("SVG element {:?} is not implemented.", node.name)),
    }
    Ok(vec![contour])
}

fn polygon(points: Vec<Point>, path: &mut Path, work: &mut Work<'_>) -> ImageResult<()> {
    let mut points = points;
    let mut area = 0.;
    for i in 0..points.len() {
        let a = points[i];
        let b = points[(i + 1) % points.len()];
        area += a.0 * b.1 - a.1 * b.0;
    }
    if area < 0. {
        points.reverse();
    }
    let mut contour = Contour {
        points: Vec::new(),
        closed: true,
    };
    for p in points {
        add(&mut contour, p, work)?;
    }
    path.push(contour);
    Ok(())
}

fn stroke(path: &Path, style: &Style, matrix: Matrix, work: &mut Work<'_>) -> ImageResult<Path> {
    let mut output = Vec::new();
    let radius = style.width / 2.;
    if radius == 0. {
        return Ok(output);
    }
    for contour in path {
        if contour.points.len() == 1 && !contour.closed {
            continue;
        }
        let mut points = Vec::new();
        for &point in &contour.points {
            if points.last() != Some(&point) {
                points.push(point);
            }
        }
        if contour.closed && points.len() > 1 && points.first() == points.last() {
            points.pop();
        }
        if points.len() == 1 {
            if matches!(style.cap, Cap::Round) {
                output.push(ellipse(points[0], (radius, radius), matrix, work)?);
            } else if matches!(style.cap, Cap::Square) {
                let (x, y) = points[0];
                polygon(
                    vec![
                        (x - radius, y - radius),
                        (x + radius, y - radius),
                        (x + radius, y + radius),
                        (x - radius, y + radius),
                    ],
                    &mut output,
                    work,
                )?;
            }
            continue;
        }
        if points.len() < 2 {
            continue;
        }
        let count = if contour.closed {
            points.len()
        } else {
            points.len() - 1
        };
        let mut normals = Vec::new();
        let mut tangents = Vec::new();
        for i in 0..count {
            let a = points[i];
            let b = points[(i + 1) % points.len()];
            let (dx, dy) = (b.0 - a.0, b.1 - a.1);
            let length = dx.hypot(dy);
            let t = (dx / length, dy / length);
            let n = (-t.1 * radius, t.0 * radius);
            normals.push(n);
            tangents.push(t);
            let extension_a = if i == 0 && !contour.closed && matches!(style.cap, Cap::Square) {
                (-t.0 * radius, -t.1 * radius)
            } else {
                (0., 0.)
            };
            let extension_b =
                if i + 1 == count && !contour.closed && matches!(style.cap, Cap::Square) {
                    (t.0 * radius, t.1 * radius)
                } else {
                    (0., 0.)
                };
            polygon(
                vec![
                    (a.0 + n.0 + extension_a.0, a.1 + n.1 + extension_a.1),
                    (a.0 - n.0 + extension_a.0, a.1 - n.1 + extension_a.1),
                    (b.0 - n.0 + extension_b.0, b.1 - n.1 + extension_b.1),
                    (b.0 + n.0 + extension_b.0, b.1 + n.1 + extension_b.1),
                ],
                &mut output,
                work,
            )?;
        }
        let joins = if contour.closed {
            points.len()
        } else {
            points.len() - 2
        };
        for j in 0..joins {
            let i = if contour.closed { j } else { j + 1 };
            let before = (i + count - 1) % count;
            let after = i % count;
            let p = points[i];
            let a = tangents[before];
            let b = tangents[after];
            let cross = a.0 * b.1 - a.1 * b.0;
            if matches!(style.join, Join::Round) {
                output.push(ellipse(p, (radius, radius), matrix, work)?);
                continue;
            }
            if cross.abs() < 1e-12 {
                continue;
            }
            let sign = if cross > 0. { -1. } else { 1. };
            let n = (normals[before].0 * sign, normals[before].1 * sign);
            let m = (normals[after].0 * sign, normals[after].1 * sign);
            let first = (p.0 + n.0, p.1 + n.1);
            let last = (p.0 + m.0, p.1 + m.1);
            let mut join = vec![p, first];
            if matches!(style.join, Join::Miter) {
                let difference = (m.0 - n.0, m.1 - n.1);
                let t = (difference.0 * b.1 - difference.1 * b.0) / cross;
                let tip = (first.0 + a.0 * t, first.1 + a.1 * t);
                if (tip.0 - p.0).hypot(tip.1 - p.1) <= radius * style.miter {
                    join.push(tip);
                }
            }
            join.push(last);
            polygon(join, &mut output, work)?;
        }
        if !contour.closed && matches!(style.cap, Cap::Round) {
            output.push(ellipse(points[0], (radius, radius), matrix, work)?);
            output.push(ellipse(
                *points.last().unwrap(),
                (radius, radius),
                matrix,
                work,
            )?);
        }
    }
    Ok(output)
}

fn blend(destination: &mut u32, color: u32, coverage: f64) {
    let alpha = (((color >> 24) as f64) * coverage).round().clamp(0., 255.) as u32;
    if alpha == 0 {
        return;
    }
    if alpha == 255 {
        *destination = color;
        return;
    }
    let old_alpha = *destination >> 24;
    let remain = (old_alpha * (255 - alpha) + 127) / 255;
    let total = alpha + remain;
    let channel = |shift: u32| {
        let src = (color >> shift) & 255;
        let dst = (*destination >> shift) & 255;
        ((src * alpha + dst * remain + total / 2) / total) << shift
    };
    *destination = (total << 24) | channel(16) | channel(8) | channel(0);
}

#[derive(Clone, Copy)]
struct Bounds {
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
}
fn bounds(path: &Path) -> Option<Bounds> {
    let mut out: Option<Bounds> = None;
    for point in path.iter().flat_map(|c| &c.points) {
        if let Some(b) = &mut out {
            b.left = b.left.min(point.0);
            b.right = b.right.max(point.0);
            b.top = b.top.min(point.1);
            b.bottom = b.bottom.max(point.1);
        } else {
            out = Some(Bounds {
                left: point.0,
                right: point.0,
                top: point.1,
                bottom: point.1,
            });
        }
    }
    out
}

impl Bounds {
    fn include(&mut self, other: Self) {
        self.left = self.left.min(other.left);
        self.top = self.top.min(other.top);
        self.right = self.right.max(other.right);
        self.bottom = self.bottom.max(other.bottom);
    }
    fn units(self) -> Option<Matrix> {
        let (w, h) = (self.right - self.left, self.bottom - self.top);
        (w > 0. && h > 0.).then_some(Matrix([w, 0., 0., h, self.left, self.top]))
    }
}

// A clip is a union of child silhouettes, intersected with any referenced clip.
// Combining scanline intervals BEFORE antialiasing is important: painting white
// shapes into an alpha image would give incorrect coverage along shared edges.
enum Clip {
    Empty,
    Shape {
        edges: Vec<(Point, Point)>,
        top: f64,
        bottom: f64,
        even_odd: bool,
    },
    Union(Vec<Clip>),
    Intersection(Box<Clip>, Box<Clip>),
}

impl Clip {
    fn geometry(
        path: &Path,
        matrix: Matrix,
        even_odd: bool,
        work: &mut Work<'_>,
    ) -> ImageResult<Self> {
        let mut edges = Vec::new();
        let mut top = f64::INFINITY;
        let mut bottom = f64::NEG_INFINITY;
        for contour in path {
            if contour.points.len() < 3 {
                continue;
            }
            work.tick(contour.points.len())?;
            for i in 0..contour.points.len() {
                let a = matrix.point(contour.points[i]);
                let b = matrix.point(contour.points[(i + 1) % contour.points.len()]);
                if !a.0.is_finite() || !a.1.is_finite() || !b.0.is_finite() || !b.1.is_finite() {
                    return Err("Nonfinite transformed SVG clip path.".into());
                }
                if a.1 != b.1 {
                    top = top.min(a.1.min(b.1));
                    bottom = bottom.max(a.1.max(b.1));
                    edges.push((a, b));
                }
            }
        }
        Ok(Self::Shape {
            edges,
            top,
            bottom,
            even_odd,
        })
    }

    fn intersect(self, other: Self) -> Self {
        Self::Intersection(Box::new(self), Box::new(other))
    }

    fn intervals(&self, y: f64, work: &mut Work<'_>) -> ImageResult<Vec<(f64, f64)>> {
        work.tick(1)?;
        match self {
            Self::Empty => Ok(Vec::new()),
            Self::Shape {
                edges,
                top,
                bottom,
                even_odd,
            } => {
                if y < *top || y >= *bottom {
                    return Ok(Vec::new());
                }
                work.tick(edges.len())?;
                let mut crosses = Vec::new();
                for &(a, b) in edges {
                    if y >= a.1.min(b.1) && y < a.1.max(b.1) {
                        crosses.push((
                            a.0 + (y - a.1) * (b.0 - a.0) / (b.1 - a.1),
                            if b.1 > a.1 { 1 } else { -1 },
                        ));
                    }
                }
                work.tick(crosses.len() * (crosses.len().max(1).ilog2() as usize + 1))?;
                crosses.sort_by(|a, b| a.0.total_cmp(&b.0));
                let mut out = Vec::new();
                let (mut winding, mut previous) = (0i32, 0.);
                for (x, delta) in crosses {
                    if (if *even_odd {
                        winding & 1 != 0
                    } else {
                        winding != 0
                    }) && x > previous
                    {
                        out.push((previous, x));
                    }
                    winding += if *even_odd { 1 } else { delta };
                    previous = x;
                }
                Ok(out)
            }
            Self::Union(children) => {
                let mut intervals = Vec::new();
                for child in children {
                    intervals.extend(child.intervals(y, work)?);
                }
                work.tick(intervals.len() * (intervals.len().max(1).ilog2() as usize + 1))?;
                intervals.sort_by(|a, b| a.0.total_cmp(&b.0));
                let mut out = Vec::<(f64, f64)>::new();
                for (left, right) in intervals {
                    if let Some(last) = out.last_mut().filter(|last| last.1 >= left) {
                        last.1 = last.1.max(right);
                    } else {
                        out.push((left, right));
                    }
                }
                Ok(out)
            }
            Self::Intersection(left, right) => {
                let a = left.intervals(y, work)?;
                let b = right.intervals(y, work)?;
                work.tick(a.len() + b.len())?;
                let (mut i, mut j) = (0, 0);
                let mut out = Vec::new();
                while i < a.len() && j < b.len() {
                    let start = a[i].0.max(b[j].0);
                    let end = a[i].1.min(b[j].1);
                    if end > start {
                        out.push((start, end));
                    }
                    if a[i].1 < b[j].1 {
                        i += 1;
                    } else {
                        j += 1;
                    }
                }
                Ok(out)
            }
        }
    }
}

fn multiply_alpha(color: &mut u32, factor: f64) {
    let alpha = ((*color >> 24) as f64 * factor).round().clamp(0., 255.) as u32;
    *color = if alpha == 0 {
        0
    } else {
        (*color & 0x00ff_ffff) | (alpha << 24)
    };
}

fn clip_pixels(
    clip: &Clip,
    width: usize,
    height: usize,
    pixels: &mut [u32],
    work: &mut Work<'_>,
) -> ImageResult<()> {
    let mut coverage = vec![0f64; width];
    for y in 0..height {
        coverage.fill(0.);
        work.tick(width)?;
        for sample in 0..SAMPLES {
            let line = y as f64 + (sample as f64 + 0.5) / SAMPLES as f64;
            for (left, right) in clip.intervals(line, work)? {
                let start = left.max(0.);
                let end = right.min(width as f64);
                if start < end {
                    let first = start.floor() as usize;
                    let last = end.ceil() as usize;
                    work.tick(last - first)?;
                    for (x, amount) in coverage.iter_mut().enumerate().take(last).skip(first) {
                        *amount +=
                            (end.min(x as f64 + 1.) - start.max(x as f64)).max(0.) / SAMPLES as f64;
                    }
                }
            }
        }
        work.tick(width)?;
        for (color, amount) in pixels[y * width..(y + 1) * width].iter_mut().zip(&coverage) {
            multiply_alpha(color, amount.clamp(0., 1.));
        }
    }
    Ok(())
}

// Keep the raster inputs explicit; no hidden global canvas or paint state.
#[allow(clippy::too_many_arguments)]
fn fill(
    path: &Path,
    matrix: Matrix,
    even_odd: bool,
    paint: &Brush,
    alpha: f64,
    width: usize,
    height: usize,
    pixels: &mut [u32],
    work: &mut Work<'_>,
) -> ImageResult<()> {
    let mut edges = Vec::new();
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for contour in path {
        if contour.points.len() < 3 {
            continue;
        }
        for i in 0..contour.points.len() {
            let a = matrix.point(contour.points[i]);
            let b = matrix.point(contour.points[(i + 1) % contour.points.len()]);
            if !a.0.is_finite() || !a.1.is_finite() || !b.0.is_finite() || !b.1.is_finite() {
                return Err("Nonfinite transformed SVG path.".into());
            }
            if a.1 == b.1 {
                continue;
            }
            min_y = min_y.min(a.1.min(b.1));
            max_y = max_y.max(a.1.max(b.1));
            edges.push((a, b));
        }
    }
    if edges.is_empty() || alpha == 0. {
        return Ok(());
    }
    let first = min_y.floor().clamp(0., height as f64) as usize;
    let last = max_y.ceil().clamp(0., height as f64) as usize;
    let mut intersections = Vec::<(f64, i32)>::with_capacity(edges.len());
    let mut coverage = vec![0f64; width];
    for y in first..last {
        coverage.fill(0.);
        work.tick(width)?;
        for sample in 0..SAMPLES {
            let line = y as f64 + (sample as f64 + 0.5) / SAMPLES as f64;
            intersections.clear();
            work.tick(edges.len())?;
            for &(a, b) in &edges {
                if line >= a.1.min(b.1) && line < a.1.max(b.1) {
                    intersections.push((
                        a.0 + (line - a.1) * (b.0 - a.0) / (b.1 - a.1),
                        if b.1 > a.1 { 1 } else { -1 },
                    ));
                }
            }
            work.tick(intersections.len() * (intersections.len().max(1).ilog2() as usize + 1))?;
            intersections.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut winding = 0i32;
            let mut prior = 0.;
            for &(x, delta) in &intersections {
                if if even_odd {
                    winding & 1 != 0
                } else {
                    winding != 0
                } {
                    let start: f64 = f64::max(prior, 0.);
                    let end: f64 = f64::min(x, width as f64);
                    if start < end {
                        let left = start.floor() as usize;
                        let right = end.ceil() as usize;
                        work.tick(right - left)?;
                        for (column, value) in
                            coverage.iter_mut().enumerate().take(right).skip(left)
                        {
                            *value += (end.min(column as f64 + 1.) - start.max(column as f64))
                                .max(0.)
                                / SAMPLES as f64;
                        }
                    }
                }
                winding += if even_odd { 1 } else { delta };
                prior = x;
            }
        }
        for (x, &amount) in coverage.iter().enumerate() {
            if amount > 0. {
                blend(
                    &mut pixels[y * width + x],
                    paint.color((x as f64 + 0.5, y as f64 + 0.5)),
                    amount.min(1.) * alpha,
                );
            }
        }
    }
    Ok(())
}

enum GradientKind {
    Linear(Point, Point),
    Radial {
        center: Point,
        focus: Point,
        radius: f64,
    },
}
enum Spread {
    Pad,
    Repeat,
    Reflect,
}
struct Gradient {
    inverse: Matrix,
    kind: GradientKind,
    spread: Spread,
    stops: Vec<(f64, u32)>,
}
enum Brush {
    Solid(u32),
    Gradient(Gradient),
}

impl Brush {
    fn color(&self, point: Point) -> u32 {
        let Self::Gradient(gradient) = self else {
            let Self::Solid(value) = self else {
                unreachable!()
            };
            return *value;
        };
        let point = gradient.inverse.point(point);
        let mut position = match gradient.kind {
            GradientKind::Linear(a, b) => {
                let dx = b.0 - a.0;
                let dy = b.1 - a.1;
                let length = dx * dx + dy * dy;
                if length == 0. {
                    1.
                } else {
                    ((point.0 - a.0) * dx + (point.1 - a.1) * dy) / length
                }
            }
            GradientKind::Radial {
                center,
                focus,
                radius,
            } => {
                if radius == 0. {
                    1.
                } else {
                    let d = (point.0 - focus.0, point.1 - focus.1);
                    let c = (focus.0 - center.0, focus.1 - center.1);
                    let a = d.0 * d.0 + d.1 * d.1;
                    if a < 1e-20 {
                        0.
                    } else {
                        let b = 2. * (d.0 * c.0 + d.1 * c.1);
                        let c = c.0 * c.0 + c.1 * c.1 - radius * radius;
                        let lambda = (-b + (b * b - 4. * a * c).max(0.).sqrt()) / (2. * a);
                        if lambda > 0. {
                            1. / lambda
                        } else {
                            1.
                        }
                    }
                }
            }
        };
        position = match gradient.spread {
            Spread::Pad => position.clamp(0., 1.),
            Spread::Repeat => position.rem_euclid(1.),
            Spread::Reflect => {
                let n = position.rem_euclid(2.);
                if n > 1. {
                    2. - n
                } else {
                    n
                }
            }
        };
        let Some(&(first_at, first)) = gradient.stops.first() else {
            return 0;
        };
        if position < first_at {
            return first;
        }
        // Duplicate offsets form hard stops. Binary search bounds per-pixel
        // gradient cost, even for the maximum 256 stops.
        let index = gradient.stops.partition_point(|&(at, _)| at <= position);
        if index == gradient.stops.len() {
            return gradient.stops[index - 1].1;
        }
        let (left, a) = gradient.stops[index - 1];
        let (right, b) = gradient.stops[index];
        interpolate(a, b, (position - left) / (right - left))
    }
}

fn interpolate(a: u32, b: u32, t: f64) -> u32 {
    // SVG gradients interpolate straight RGB and alpha independently (unlike
    // CSS gradients). A transparent-black stop therefore darkens a fade.
    let channel = |shift: u32| {
        let left = ((a >> shift) & 255) as f64;
        let right = ((b >> shift) & 255) as f64;
        ((left + (right - left) * t).round().clamp(0., 255.) as u32) << shift
    };
    channel(24) | channel(16) | channel(8) | channel(0)
}

struct Renderer<'a> {
    nodes: Vec<Node>,
    rules: Vec<Rule>,
    ids: HashMap<String, usize>,
    styles: Vec<Style>,
    rule_cost: usize,
    width: usize,
    height: usize,
    viewport: Point,
    active: Vec<usize>,
    work: Work<'a>,
}

impl Renderer<'_> {
    fn layer(&mut self) -> ImageResult<Vec<u32>> {
        let count = self.width * self.height;
        self.work.layers = self
            .work
            .layers
            .checked_add(count)
            .filter(|&n| n <= MAX_LAYERS)
            .ok_or("SVG temporary-layer pixel budget reached (opacity, clips and masks).")?;
        self.work.tick(count)?;
        let mut layer = reserved(count)?;
        layer.resize(count, 0);
        Ok(layer)
    }

    fn local_use(&self, index: usize) -> ImageResult<(usize, Matrix)> {
        let node = &self.nodes[index];
        let href = node
            .attr("href")
            .or_else(|| node.attr("xlink:href"))
            .ok_or("SVG use needs a local fragment reference.")?;
        let id = crate::svg_reference::fragment(href)
            .ok_or("SVG use cannot fetch external resources.")?;
        let target = *self.ids.get(id).ok_or("SVG use target was not found.")?;
        if matches!(self.nodes[target].name.as_str(), "svg" | "symbol") {
            return Err("SVG use with symbol/nested viewports is not implemented.".into());
        }
        let translation = Matrix([
            1.,
            0.,
            0.,
            1.,
            number_attr(node, "x", 0., self.viewport.0)?,
            number_attr(node, "y", 0., self.viewport.1)?,
        ]);
        Ok((target, translation))
    }

    // Object bounding boxes exclude strokes and effects, but include geometry
    // hidden by visibility, transparent paint or opacity. Non-rendered
    // definitions and display:none descendants never enlarge an object's box.
    fn object_bounds(
        &mut self,
        index: usize,
        style: &Style,
        device: Matrix,
    ) -> ImageResult<Option<Bounds>> {
        self.bounds_body(index, style, Matrix::ID, device, &mut vec![index])
    }

    fn bounds_node(
        &mut self,
        index: usize,
        parent: &Style,
        matrix: Matrix,
        device: Matrix,
        visited: &mut Vec<usize>,
    ) -> ImageResult<Option<Bounds>> {
        self.work.tick(1)?;
        if visited.len() >= MAX_DEPTH || visited.contains(&index) {
            return Err("SVG geometry/reference nesting limit or cycle reached.".into());
        }
        if matches!(
            self.nodes[index].name.as_str(),
            "defs"
                | "style"
                | "title"
                | "desc"
                | "metadata"
                | "linearGradient"
                | "radialGradient"
                | "stop"
                | "clipPath"
                | "mask"
                | "filter"
                | "pattern"
                | "marker"
                | "symbol"
        ) {
            return Ok(None);
        }
        let style = self.style(index, parent)?;
        if !style.display {
            return Ok(None);
        }
        let matrix = matrix.multiply(transform(self.nodes[index].attr("transform"))?)?;
        visited.push(index);
        let result = self.bounds_body(index, &style, matrix, device, visited)?;
        visited.pop();
        Ok(result)
    }

    fn bounds_body(
        &mut self,
        index: usize,
        style: &Style,
        matrix: Matrix,
        device: Matrix,
        visited: &mut Vec<usize>,
    ) -> ImageResult<Option<Bounds>> {
        match self.nodes[index].name.as_str() {
            "svg" | "g" | "a" | "clipPath" | "mask" => {
                let children = self.nodes[index].children.clone();
                let mut out: Option<Bounds> = None;
                for child in children {
                    if let Some(next) = self.bounds_node(child, style, matrix, device, visited)? {
                        if let Some(previous) = &mut out {
                            previous.include(next);
                        } else {
                            out = Some(next);
                        }
                    }
                }
                Ok(out)
            }
            "use" => {
                let (target, translation) = self.local_use(index)?;
                self.bounds_node(
                    target,
                    style,
                    matrix.multiply(translation)?,
                    device,
                    visited,
                )
            }
            "rect" | "circle" | "ellipse" | "line" | "polygon" | "polyline" | "path" => {
                // Bound in local coordinates, but flatten at device precision:
                // otherwise a tiny cubic enlarged by scale() gets a coarse box.
                let mut path = shape(
                    &self.nodes[index],
                    device.multiply(matrix)?,
                    self.viewport,
                    &mut self.work,
                )?;
                for contour in &mut path {
                    self.work.tick(contour.points.len())?;
                    for point in &mut contour.points {
                        *point = matrix.point(*point);
                    }
                }
                Ok(bounds(&path))
            }
            _ => Ok(None),
        }
    }

    fn enter_clip(&mut self, index: usize) -> ImageResult<()> {
        self.work.tick(1)?;
        self.work.clips += 1;
        if self.work.clips > MAX_NODES {
            return Err("SVG expanded clip-node limit reached (4096).".into());
        }
        if self.active.len() >= MAX_DEPTH || self.active.contains(&index) {
            return Err("SVG clipping/reference nesting limit or cycle reached.".into());
        }
        self.active.push(index);
        Ok(())
    }

    fn clip_reference(
        &mut self,
        id: &str,
        target_bounds: Option<Bounds>,
        matrix: Matrix,
    ) -> ImageResult<Option<Clip>> {
        // CSS Masking 1: an unresolved/wrong-kind clip reference means no clip.
        let Some(index) = self
            .ids
            .get(id)
            .copied()
            .filter(|&i| self.nodes[i].name == "clipPath")
        else {
            return Ok(None);
        };
        let object = match self.nodes[index]
            .attr("clipPathUnits")
            .unwrap_or("userSpaceOnUse")
        {
            "userSpaceOnUse" => false,
            "objectBoundingBox" => true,
            _ => return Err("Unsupported SVG clipPathUnits.".into()),
        };
        let units = if object {
            let Some(units) = target_bounds.and_then(Bounds::units) else {
                return Ok(Some(Clip::Empty));
            };
            units
        } else {
            Matrix::ID
        };
        let matrix = matrix
            .multiply(units)?
            .multiply(transform(self.nodes[index].attr("transform"))?)?;
        self.enter_clip(index)?;
        let old_viewport = self.viewport;
        if object {
            self.viewport = (1., 1.);
        }
        let style = self.styles[index].clone();
        let children = self.nodes[index].children.clone();
        let mut silhouettes = Vec::new();
        for child in children {
            silhouettes.push(self.clip_child(child, &style, matrix)?);
        }
        let mut clip = Clip::Union(silhouettes);
        if let Some(id) = &style.clip {
            let bounds = self.object_bounds(index, &style, matrix)?;
            if let Some(other) = self.clip_reference(id, bounds, matrix)? {
                clip = clip.intersect(other);
            }
        }
        self.viewport = old_viewport;
        self.active.pop();
        Ok(Some(clip))
    }

    fn clip_child(&mut self, index: usize, parent: &Style, matrix: Matrix) -> ImageResult<Clip> {
        let name = self.nodes[index].name.clone();
        if matches!(name.as_str(), "title" | "desc" | "metadata") {
            return Ok(Clip::Empty);
        }
        self.enter_clip(index)?;
        let style = self.style(index, parent)?;
        if !style.display {
            self.active.pop();
            return Ok(Clip::Empty);
        }
        let matrix = matrix.multiply(transform(self.nodes[index].attr("transform"))?)?;
        let mut clip = match name.as_str() {
            "use" => {
                let (target, translation) = self.local_use(index)?;
                if !matches!(self.nodes[target].name.as_str(), "rect" | "circle" | "ellipse"
                    | "line" | "polygon" | "polyline" | "path") {
                    return Err("SVG clipPath use must directly reference a supported basic shape or path.".into());
                }
                self.clip_child(target, &style, matrix.multiply(translation)?)?
            }
            "rect" | "circle" | "ellipse" | "line" | "polygon" | "polyline" | "path" => {
                if style.visible {
                    let path = shape(&self.nodes[index], matrix, self.viewport, &mut self.work)?;
                    Clip::geometry(&path, matrix, style.clip_even_odd, &mut self.work)?
                } else { Clip::Empty }
            }
            _ => return Err(format!("SVG clipPath child {name:?} is not implemented; use shapes, paths or direct shape references.")),
        };
        // Fill/stroke/opacity do not affect clipping silhouettes. Only the
        // inherited clip-rule and explicit descendant clipping apply.
        if let Some(id) = &style.clip {
            let bounds = self.object_bounds(index, &style, matrix)?;
            if let Some(other) = self.clip_reference(id, bounds, matrix)? {
                clip = clip.intersect(other);
            }
        }
        self.active.pop();
        Ok(clip)
    }

    fn effects(
        &mut self,
        index: usize,
        style: &Style,
        matrix: Matrix,
        pixels: &mut [u32],
    ) -> ImageResult<()> {
        if style.clip.is_none() && style.mask.is_none() {
            return Ok(());
        }
        let bounds = self.object_bounds(index, style, matrix)?;
        if let Some(id) = &style.clip {
            if let Some(clip) = self.clip_reference(id, bounds, matrix)? {
                clip_pixels(&clip, self.width, self.height, pixels, &mut self.work)?;
            }
        }
        if let Some(id) = &style.mask {
            self.apply_mask(id, style.mask_mode, bounds, matrix, pixels)?;
        }
        Ok(())
    }

    fn apply_mask(
        &mut self,
        id: &str,
        mode: Option<bool>,
        target_bounds: Option<Bounds>,
        matrix: Matrix,
        pixels: &mut [u32],
    ) -> ImageResult<()> {
        let Some(index) = self
            .ids
            .get(id)
            .copied()
            .filter(|&i| self.nodes[i].name == "mask")
        else {
            self.work.tick(pixels.len())?;
            pixels.fill(0); // Invalid mask images are transparent black.
            return Ok(());
        };
        if self.active.len() >= MAX_DEPTH || self.active.contains(&index) {
            return Err("SVG masking/reference nesting limit or cycle reached.".into());
        }
        let node = self.nodes[index].clone();
        let object_region = match node.attr("maskUnits").unwrap_or("objectBoundingBox") {
            "userSpaceOnUse" => false,
            "objectBoundingBox" => true,
            _ => return Err("Unsupported SVG maskUnits.".into()),
        };
        let object_content = match node.attr("maskContentUnits").unwrap_or("userSpaceOnUse") {
            "userSpaceOnUse" => false,
            "objectBoundingBox" => true,
            _ => return Err("Unsupported SVG maskContentUnits.".into()),
        };
        let has_region = ["x", "y", "width", "height"]
            .iter()
            .any(|name| node.attr(name).is_some());
        let units = target_bounds.and_then(Bounds::units);
        if units.is_none() && (object_content || (has_region && object_region)) {
            self.work.tick(pixels.len())?;
            pixels.fill(0);
            return Ok(());
        }
        let region = if has_region {
            let reference = if object_region {
                (1., 1.)
            } else {
                self.viewport
            };
            let x = length(node.attr("x").unwrap_or("-10%"), reference.0)?;
            let y = length(node.attr("y").unwrap_or("-10%"), reference.1)?;
            let w = length(node.attr("width").unwrap_or("120%"), reference.0)?;
            let h = length(node.attr("height").unwrap_or("120%"), reference.1)?;
            if w <= 0. || h <= 0. {
                self.work.tick(pixels.len())?;
                pixels.fill(0);
                return Ok(());
            }
            for _ in 0..4 {
                self.work.segment()?;
            }
            let path = vec![Contour {
                points: vec![(x, y), (x + w, y), (x + w, y + h), (x, y + h)],
                closed: true,
            }];
            let transform = matrix.multiply(if object_region {
                units.unwrap()
            } else {
                Matrix::ID
            })?;
            Some(Clip::geometry(&path, transform, false, &mut self.work)?)
        } else {
            None
        };
        let mask_matrix = matrix
            .multiply(if object_content {
                units.unwrap()
            } else {
                Matrix::ID
            })?
            .multiply(transform(node.attr("transform"))?)?;
        self.active.push(index);
        let old_viewport = self.viewport;
        if object_content {
            self.viewport = (1., 1.);
        }
        let style = self.styles[index].clone();
        let mut layer = self.layer()?;
        // A mask uses styles inherited from its original tree; the mask root's
        // display and opacity do not suppress or attenuate the mask contents.
        for child in node.children {
            self.element(child, &style, mask_matrix, &mut layer)?;
        }
        self.effects(index, &style, mask_matrix, &mut layer)?;
        self.viewport = old_viewport;
        self.active.pop();
        if let Some(region) = region {
            clip_pixels(&region, self.width, self.height, &mut layer, &mut self.work)?;
        }
        let alpha = mode.unwrap_or(style.mask_alpha);
        for (destinations, masks) in pixels.chunks_mut(4096).zip(layer.chunks(4096)) {
            self.work.tick(destinations.len())?;
            for (destination, mask) in destinations.iter_mut().zip(masks) {
                let luminance = if alpha {
                    1.
                } else {
                    // CSS Masking 1 / Filter Effects luminanceToAlpha, sRGB. Linear
                    // RGB remains an explicit unsupported color-interpolation mode.
                    (0.2126 * ((mask >> 16) & 255) as f64
                        + 0.7152 * ((mask >> 8) & 255) as f64
                        + 0.0722 * (mask & 255) as f64)
                        / 255.
                };
                multiply_alpha(destination, (mask >> 24) as f64 / 255. * luminance);
            }
        }
        self.work.layers -= layer.len();
        Ok(())
    }

    fn style(&mut self, index: usize, parent: &Style) -> ImageResult<Style> {
        let cost = self.nodes[index]
            .attrs
            .iter()
            .map(|(a, b)| a.len() + b.len())
            .sum::<usize>()
            + self.rule_cost
            + 1;
        self.work.tick(cost)?;
        computed(&self.nodes[index], parent, &self.rules, self.viewport)
    }

    fn brush(
        &mut self,
        paint: &Paint,
        current: u32,
        matrix: Matrix,
        bounds: Bounds,
    ) -> ImageResult<Option<Brush>> {
        let id = match paint {
            Paint::None => return Ok(None),
            Paint::Solid(color) => return Ok(Some(Brush::Solid(*color))),
            Paint::Current => return Ok(Some(Brush::Solid(current))),
            Paint::Server(id) => id,
        };
        let mut index = *self
            .ids
            .get(id.as_ref())
            .ok_or("SVG paint server was not found.")?;
        let host = index;
        let kind = self.nodes[index].name.clone();
        if !matches!(kind.as_str(), "linearGradient" | "radialGradient") {
            return Err("SVG only implements linear and radial gradient paint servers.".into());
        }
        let mut attrs = Attributes::new();
        let mut stops = None;
        let mut visited = Vec::new();
        loop {
            if visited.len() >= 16 || visited.contains(&index) {
                return Err("SVG gradient reference cycle or depth limit reached.".into());
            }
            visited.push(index);
            let node = &self.nodes[index];
            if !matches!(node.name.as_str(), "linearGradient" | "radialGradient") {
                return Err("SVG gradient references must target gradients.".into());
            }
            for (key, value) in &node.attrs {
                if !attrs.iter().any(|(old, _)| old == key) {
                    attrs.push((key.clone(), value.clone()));
                }
            }
            if stops.is_none() {
                let children: Vec<usize> = node
                    .children
                    .iter()
                    .copied()
                    .filter(|&i| self.nodes[i].name == "stop")
                    .collect();
                if !children.is_empty() {
                    stops = Some(children);
                }
            }
            if let Some(href) = node.attr("href").or_else(|| node.attr("xlink:href")) {
                let id = crate::svg_reference::fragment(href)
                    .ok_or("SVG gradients cannot fetch external references.")?;
                index = *self
                    .ids
                    .get(id)
                    .ok_or("SVG gradient reference was not found.")?;
            } else {
                break;
            }
        }
        let attr = |name: &str| {
            attrs
                .iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.as_str())
        };
        let object_units = match attr("gradientUnits").unwrap_or("objectBoundingBox") {
            "objectBoundingBox" => true,
            "userSpaceOnUse" => false,
            _ => return Err("Unsupported SVG gradient units.".into()),
        };
        let unit_matrix = if object_units {
            let w = bounds.right - bounds.left;
            let h = bounds.bottom - bounds.top;
            if w <= 0. || h <= 0. {
                return Ok(None);
            }
            Matrix([w, 0., 0., h, bounds.left, bounds.top])
        } else {
            Matrix::ID
        };
        let gradient_matrix = matrix
            .multiply(unit_matrix)?
            .multiply(transform(attr("gradientTransform"))?)?;
        let Some(inverse) = gradient_matrix.inverse() else {
            return Ok(None);
        };
        let viewport = if object_units {
            (1., 1.)
        } else {
            self.viewport
        };
        let x = |key, default| length(attr(key).unwrap_or(default), viewport.0);
        let y = |key, default| length(attr(key).unwrap_or(default), viewport.1);
        let kind = if kind == "linearGradient" {
            GradientKind::Linear(
                (x("x1", "0%")?, y("y1", "0%")?),
                (x("x2", "100%")?, y("y2", "0%")?),
            )
        } else {
            if attr("fr").is_some_and(|v| v != "0" && v != "0%" && v != "0px") {
                return Err("SVG radial gradient focal radius is not implemented.".into());
            }
            let center = (x("cx", "50%")?, y("cy", "50%")?);
            let radius = length(
                attr("r").unwrap_or("50%"),
                viewport.0.hypot(viewport.1) / 2f64.sqrt(),
            )?;
            if radius < 0. {
                return Err("SVG gradient radius cannot be negative.".into());
            }
            let mut focus = (
                attr("fx").map_or(Ok(center.0), |v| length(v, viewport.0))?,
                attr("fy").map_or(Ok(center.1), |v| length(v, viewport.1))?,
            );
            let d = (focus.0 - center.0).hypot(focus.1 - center.1);
            if radius > 0. && d >= radius {
                let scale = radius * (1. - 1e-9) / d;
                focus = (
                    center.0 + (focus.0 - center.0) * scale,
                    center.1 + (focus.1 - center.1) * scale,
                );
            }
            GradientKind::Radial {
                center,
                focus,
                radius,
            }
        };
        let spread = match attr("spreadMethod").unwrap_or("pad") {
            "pad" => Spread::Pad,
            "repeat" => Spread::Repeat,
            "reflect" => Spread::Reflect,
            _ => return Err("Unsupported SVG gradient spread method.".into()),
        };
        let mut colors = Vec::new();
        let mut last = 0f64;
        for stop in stops.unwrap_or_default() {
            if colors.len() >= 256 {
                return Err("SVG gradient stop limit reached (256).".into());
            }
            self.work.tick(1)?;
            let node = &self.nodes[stop];
            let offset = length(node.attr("offset").unwrap_or("0"), 1.)?
                .clamp(0., 1.)
                .max(last);
            last = offset;
            let parent_style = self.styles[host].clone();
            let style = self.style(stop, &parent_style)?;
            let color = if style.stop_current {
                style.color
            } else {
                style.stop_color
            };
            let alpha = ((color >> 24) as f64 * style.stop_alpha).round() as u32;
            colors.push((offset, (color & 0x00ff_ffff) | (alpha << 24)));
        }
        Ok(Some(Brush::Gradient(Gradient {
            inverse,
            kind,
            spread,
            stops: colors,
        })))
    }

    fn body(
        &mut self,
        index: usize,
        style: &Style,
        matrix: Matrix,
        pixels: &mut [u32],
    ) -> ImageResult<()> {
        let name = self.nodes[index].name.clone();
        match name.as_str() {
            "svg" if index != 0 => {
                return Err("Nested SVG viewports are not implemented in image documents.".into())
            }
            "svg" | "g" | "a" => {
                let children = self.nodes[index].children.clone();
                for child in children {
                    self.element(child, style, matrix, pixels)?;
                }
            }
            "defs" | "style" | "title" | "desc" | "metadata" | "linearGradient"
            | "radialGradient" | "stop" | "clipPath" | "mask" | "filter" | "pattern" | "marker"
            | "symbol" => {}
            "use" => {
                let (target, translation) = self.local_use(index)?;
                self.element(target, style, matrix.multiply(translation)?, pixels)?;
            }
            "rect" | "circle" | "ellipse" | "line" | "polygon" | "polyline" | "path" => {
                if !style.visible {
                    return Ok(());
                }
                let path = shape(&self.nodes[index], matrix, self.viewport, &mut self.work)?;
                let Some(bounds) = bounds(&path) else {
                    return Ok(());
                };
                if let Some(brush) = self.brush(&style.fill, style.color, matrix, bounds)? {
                    fill(
                        &path,
                        matrix,
                        style.even_odd,
                        &brush,
                        style.fill_alpha,
                        self.width,
                        self.height,
                        pixels,
                        &mut self.work,
                    )?;
                }
                if style.width > 0. {
                    if let Some(brush) = self.brush(&style.stroke, style.color, matrix, bounds)? {
                        let outline = stroke(&path, style, matrix, &mut self.work)?;
                        fill(
                            &outline,
                            matrix,
                            false,
                            &brush,
                            style.stroke_alpha,
                            self.width,
                            self.height,
                            pixels,
                            &mut self.work,
                        )?;
                    }
                }
            }
            "script" => return Err("SVG scripts are not executed in image documents.".into()),
            "image" => {
                return Err(
                    "Nested images and external resource fetches in SVG are not implemented."
                        .into(),
                )
            }
            "text" | "tspan" | "textPath" => {
                return Err(
                    "SVG text rendering is not implemented; convert text to paths for this subset."
                        .into(),
                )
            }
            _ => return Err(format!("SVG element {name:?} is not implemented.")),
        }
        Ok(())
    }

    fn element(
        &mut self,
        index: usize,
        parent: &Style,
        matrix: Matrix,
        pixels: &mut [u32],
    ) -> ImageResult<()> {
        if self.active.len() >= MAX_DEPTH || self.active.contains(&index) {
            return Err("SVG rendering/reference nesting limit or cycle reached.".into());
        }
        self.work.tick(1)?;
        let style = self.style(index, parent)?;
        if !style.display || style.opacity == 0. {
            return Ok(());
        }
        let matrix = if index == 0 {
            matrix
        } else {
            matrix.multiply(transform(self.nodes[index].attr("transform"))?)?
        };
        self.active.push(index);
        if style.opacity < 1. || style.clip.is_some() || style.mask.is_some() {
            let mut layer = self.layer()?;
            self.body(index, &style, matrix, &mut layer)?;
            self.effects(index, &style, matrix, &mut layer)?;
            let count = layer.len();
            for (destinations, colors) in pixels.chunks_mut(4096).zip(layer.chunks(4096)) {
                self.work.tick(destinations.len())?;
                for (destination, color) in destinations.iter_mut().zip(colors) {
                    blend(destination, *color, style.opacity);
                }
            }
            self.work.layers -= count;
        } else {
            self.body(index, &style, matrix, pixels)?;
        }
        self.active.pop();
        Ok(())
    }
}

fn viewbox(node: &Node, width: f64, height: f64) -> ImageResult<(Matrix, Point)> {
    let Some(value) = node.attr("viewBox") else {
        return Ok((Matrix::ID, (width, height)));
    };
    let values = numbers(value)?;
    let [x, y, w, h] = values.as_slice() else {
        return Err("SVG viewBox needs four numbers.".into());
    };
    if *w <= 0. || *h <= 0. {
        return Err("SVG viewBox dimensions must be positive.".into());
    }
    let policy = node.attr("preserveAspectRatio").unwrap_or("xMidYMid meet");
    let mut parts = policy.split_ascii_whitespace();
    let mut align = parts.next().unwrap_or("xMidYMid");
    if align == "defer" {
        align = parts.next().unwrap_or("xMidYMid");
    }
    let mode = parts.next().unwrap_or("meet");
    if parts.next().is_some() {
        return Err("Invalid SVG preserveAspectRatio.".into());
    }
    if align == "none" {
        return Ok((
            Matrix([
                width / w,
                0.,
                0.,
                height / h,
                -x * width / w,
                -y * height / h,
            ]),
            (*w, *h),
        ));
    }
    let scale = match mode {
        "meet" => (width / w).min(height / h),
        "slice" => (width / w).max(height / h),
        _ => return Err("Unsupported SVG aspect-ratio fitting mode.".into()),
    };
    let (horizontal, vertical) = match align {
        "xMinYMin" => (0., 0.),
        "xMidYMin" => (0.5, 0.),
        "xMaxYMin" => (1., 0.),
        "xMinYMid" => (0., 0.5),
        "xMidYMid" => (0.5, 0.5),
        "xMaxYMid" => (1., 0.5),
        "xMinYMax" => (0., 1.),
        "xMidYMax" => (0.5, 1.),
        "xMaxYMax" => (1., 1.),
        _ => return Err("Unsupported SVG aspect-ratio alignment.".into()),
    };
    Ok((
        Matrix([
            scale,
            0.,
            0.,
            scale,
            (width - w * scale) * horizontal - x * scale,
            (height - h * scale) * vertical - y * scale,
        ]),
        (*w, *h),
    ))
}

pub(super) fn decode(data: &[u8], check: &dyn Fn() -> ImageResult<()>) -> ImageResult<Bitmap> {
    if data.len() > MAX_SOURCE {
        return Err("SVG image source exceeds 1 MiB.".into());
    }
    let text = std::str::from_utf8(data).map_err(|_| "SVG images must use UTF-8.")?;
    let mut work = Work {
        left: MAX_WORK,
        since_check: 0,
        segments: 0,
        clips: 0,
        layers: 0,
        check,
    };
    check()?;
    let nodes = parse_xml(text, &mut work)?;
    for node in &nodes {
        if matches!(
            node.name.as_str(),
            "script"
                | "foreignObject"
                | "animate"
                | "animateMotion"
                | "animateTransform"
                | "set"
                | "discard"
        ) {
            return Err(format!(
                "SVG element {:?} is not implemented in static image documents.",
                node.name
            ));
        }
    }
    let root = &nodes[0];
    let fallback = if let Some(value) = root.attr("viewBox") {
        let values = numbers(value)?;
        if values.len() != 4 || values[2] <= 0. || values[3] <= 0. {
            return Err("Invalid SVG viewBox dimensions.".into());
        }
        (values[2], values[3])
    } else {
        (300., 150.)
    };
    let mut width = number_attr(root, "width", fallback.0, fallback.0)?;
    let mut height = number_attr(root, "height", fallback.1, fallback.1)?;
    if root.attr("viewBox").is_some() {
        if root.attr("width").is_some() && root.attr("height").is_none() {
            height = width * fallback.1 / fallback.0;
        } else if root.attr("height").is_some() && root.attr("width").is_none() {
            width = height * fallback.0 / fallback.1;
        }
    }
    if width <= 0. || height <= 0. {
        return Err("SVG image dimensions must be positive.".into());
    }
    let (width, height) = (width.ceil() as usize, height.ceil() as usize);
    let count = dimensions(width, height)?;
    let (view, viewport) = viewbox(root, width as f64, height as f64)?;
    let matrix = transform(root.attr("transform"))?.multiply(view)?;
    let rules = rules(&nodes)?;
    let rule_cost = rules
        .iter()
        .map(|rule| {
            rule.selector.len()
                + rule
                    .properties
                    .iter()
                    .map(|(a, b)| a.len() + b.len())
                    .sum::<usize>()
        })
        .sum();
    let mut ids = HashMap::new();
    for (index, node) in nodes.iter().enumerate() {
        if let Some(id) = node.attr("id") {
            ids.entry(id.to_owned()).or_insert(index);
        }
    }
    let mut renderer = Renderer {
        styles: vec![Style::default(); nodes.len()],
        nodes,
        rules,
        ids,
        rule_cost,
        width,
        height,
        viewport,
        active: Vec::new(),
        work,
    };
    let mut stack = vec![(0usize, Style::default())];
    while let Some((index, parent)) = stack.pop() {
        let style = renderer.style(index, &parent)?;
        renderer.styles[index] = style.clone();
        for &child in renderer.nodes[index].children.iter().rev() {
            stack.push((child, style.clone()));
        }
    }
    let mut pixels = reserved(count)?;
    pixels.resize(count, 0);
    renderer.element(0, &Style::default(), matrix, &mut pixels)?;
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
    use std::cell::Cell;

    fn svg(body: &str) -> Bitmap {
        let source = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32">{body}</svg>"#
        );
        super::super::decode(source.as_bytes()).unwrap()
    }
    fn at(bitmap: &Bitmap, x: usize, y: usize) -> u32 {
        bitmap.pixels[y * bitmap.width + x]
    }
    fn reject(body: &str, message: &str) {
        let source = format!(r#"<svg width="32" height="32">{body}</svg>"#);
        let error = super::super::decode(source.as_bytes()).unwrap_err();
        assert!(
            error.contains(message),
            "{error:?} did not contain {message:?}"
        );
    }

    #[test]
    fn svg_shapes_transforms_viewbox_and_units() {
        let source = br##"<svg width="48" viewBox="10 20 24 16"><g transform="translate(10 20)"><rect width="24" height="16" fill="#123456"/><rect x="4" y="4" width="8" height="8" fill="blue"/></g></svg>"##;
        let bitmap = super::super::decode(source).unwrap();
        assert_eq!((bitmap.width, bitmap.height), (48, 32));
        assert_eq!(at(&bitmap, 0, 0), 0xff123456);
        assert_eq!(at(&bitmap, 10, 10), 0xff0000ff);
        assert_eq!(at(&bitmap, 30, 25), 0xff123456);
        let tiny = super::super::decode(br#"<svg width=".25in" height="12pt"><rect width="100%" height="100%" fill="red"/></svg>"#).unwrap();
        assert_eq!((tiny.width, tiny.height), (24, 16));
        assert!(tiny.pixels.iter().all(|p| *p == 0xffff0000));
        let circle = svg(
            r##"<circle cx="16" cy="16" r="12" fill="#14a"/><ellipse cx="16" cy="16" rx="5" ry="8" fill="#f90"/>"##,
        );
        assert_eq!(at(&circle, 0, 0), 0);
        assert_eq!(at(&circle, 16, 16), 0xffff9900);
        assert_eq!(at(&circle, 8, 16), 0xff1144aa);
    }

    #[test]
    fn svg_path_curves_arcs_and_fill_rules() {
        let square =
            svg(r#"<path d="m2 2h28v28h-28zm8 8h12v12h-12z" fill="red" fill-rule="evenodd"/>"#);
        assert_eq!(at(&square, 4, 4), 0xffff0000);
        assert_eq!(at(&square, 16, 16), 0);
        let nonzero = svg(r#"<path d="M2 2h28v28h-28zM10 10v12h12v-12z" fill="blue"/>"#);
        assert_eq!(at(&nonzero, 16, 16), 0);
        let disc = svg(r#"<path d="M4 16A12 12 0 0 1 28 16a12 12 0 0 1-24 0Z" fill="green"/>"#);
        assert_eq!(at(&disc, 16, 16), 0xff008000);
        assert_eq!(at(&disc, 0, 0), 0);
        let curves = svg(
            r#"<path d="M2 16C2 2 16 2 16 16S30 30 30 16L30 31Q16 16 2 31T2 16Z" fill="purple"/>"#,
        );
        assert!(curves.pixels.iter().filter(|p| **p >> 24 > 0).count() > 180);
        reject(r#"<path d="M0 0 A5 5 0 2 0 20 20"/>"#, "flags");
        reject(r#"<path d="L10 10"/>"#, "moveto");
        reject(r#"<path d="M0 0C10 10 20"/>"#, "number");
    }

    #[test]
    fn svg_group_opacity_and_stroke_are_composited_once() {
        let bitmap = svg(
            r#"<g opacity=".5" fill="red"><rect x="2" y="2" width="20" height="20"/><rect x="10" y="10" width="20" height="20"/></g>"#,
        );
        assert_eq!(at(&bitmap, 4, 4), 0x80ff0000);
        assert_eq!(at(&bitmap, 15, 15), 0x80ff0000); // Group overlap is not darkened.
        let filled = svg(
            r#"<rect x="5" y="5" width="20" height="20" fill="blue" stroke="red" stroke-width="6" opacity=".5"/>"#,
        );
        assert_eq!(at(&filled, 16, 16), 0x800000ff);
        assert_eq!(at(&filled, 6, 16), 0x80ff0000);
        let inherited =
            svg(r#"<g opacity=".5"><rect width="32" height="32" opacity="inherit"/></g>"#);
        assert_eq!(at(&inherited, 16, 16) >> 24, 64);
    }

    #[test]
    fn svg_stroke_caps_zero_length_and_move_only_paths() {
        let bitmap = svg(
            r#"<path d="M6 6" stroke="red" stroke-width="8" stroke-linecap="round"/><path d="M16 16L16 16" stroke="blue" stroke-width="8" stroke-linecap="square"/><path d="M8 26H24" stroke="green" stroke-width="4" stroke-linecap="round"/>"#,
        );
        assert_eq!(at(&bitmap, 6, 6), 0); // A lone moveto is not a drawable zero-length segment.
        assert_eq!(at(&bitmap, 12, 12), 0xff0000ff); // SVG 2 requires a full square.
        assert_eq!(at(&bitmap, 19, 19), 0xff0000ff);
        assert_eq!(at(&bitmap, 20, 20), 0);
        assert_eq!(at(&bitmap, 7, 26), 0xff008000);
        let transformed = svg(
            r#"<g transform="scale(2 1)"><line x1="3" y1="16" x2="13" y2="16" stroke="red" stroke-width="4" stroke-linecap="square"/></g>"#,
        );
        assert_eq!(at(&transformed, 2, 16), 0xffff0000);
        assert_eq!(at(&transformed, 29, 16), 0xffff0000);
        assert_eq!(at(&transformed, 30, 16), 0);
    }

    #[test]
    fn svg_css_cascade_local_references_and_current_color() {
        let bitmap = svg(
            r##"<style>rect{fill:red}.tile{fill:blue}#a{fill:#123456}</style><defs><rect id="template" width="8" height="8" style="fill:inherit"/></defs><rect id="a" class="tile" width="16" height="16"/><rect class="tile" x="16" width="16" height="16" style="fill:green"/><g fill="currentColor" color="orange"><use href="#template" x="2" y="20"/></g>"##,
        );
        assert_eq!(at(&bitmap, 3, 3), 0xff123456);
        assert_eq!(at(&bitmap, 20, 3), 0xff008000);
        assert_eq!(at(&bitmap, 3, 23), 0xffffa500);
        reject(
            r##"<defs><g id="a"><use href="#b"/></g><g id="b"><use href="#a"/></g></defs><use href="#a"/>"##,
            "cycle",
        );
        reject(
            r#"<use href="https://example.invalid/logo.svg#x"/>"#,
            "external",
        );
        reject(
            r#"<style>g rect {fill:red}</style><rect width="30" height="30"/>"#,
            "selectors",
        );
        reject(
            r#"<rect width="30" height="30" style="transform:scale(2)"/>"#,
            "geometry",
        );
    }

    #[test]
    fn svg_gradients_interpolate_straight_color_and_resolve_templates() {
        let bitmap = svg(
            r##"<defs><linearGradient id="g"><stop stop-color="red"/><stop offset="1" stop-color="transparent"/></linearGradient></defs><rect width="32" height="32" fill="url(#g)"/>"##,
        );
        let middle = at(&bitmap, 15, 15);
        assert_eq!((middle >> 16) & 255, 131);
        assert_eq!(middle >> 24, 131);
        assert_eq!(middle & 65535, 0);
        let template = svg(
            r##"<defs><linearGradient id="a"><stop stop-color="currentColor"/><stop offset="1" stop-color="currentColor"/></linearGradient><linearGradient id="b" href="#a" color="blue"/></defs><rect width="32" height="32" fill="url(#b)"/>"##,
        );
        assert!(template.pixels.iter().all(|p| *p == 0xff0000ff));
        let late_color = svg(
            r##"<defs><linearGradient id="g"><stop style="stop-color:currentColor;color:red"/></linearGradient></defs><rect width="32" height="32" fill="url(#g)"/>"##,
        );
        assert!(late_color.pixels.iter().all(|p| *p == 0xffff0000));
        let hard = svg(
            r##"<defs><linearGradient id="g"><stop stop-color="red"/><stop stop-color="blue"/><stop offset="1" stop-color="blue"/></linearGradient></defs><rect width="32" height="32" fill="url(#g)"/>"##,
        );
        assert!(hard.pixels.iter().all(|p| *p == 0xff0000ff));
    }

    #[test]
    fn svg_radial_gradients_and_spread_modes() {
        let bitmap = svg(
            r##"<defs><radialGradient id="g"><stop stop-color="white"/><stop offset="1" stop-color="black"/></radialGradient></defs><rect width="32" height="32" fill="url(#g)"/>"##,
        );
        assert!((at(&bitmap, 16, 16) & 255) > 235);
        assert_eq!(at(&bitmap, 0, 0), 0xff000000);
        let spread = svg(
            r##"<defs><linearGradient id="g" gradientUnits="userSpaceOnUse" x1="0" x2="8" spreadMethod="repeat"><stop stop-color="red"/><stop offset="1" stop-color="blue"/></linearGradient></defs><rect width="32" height="32" fill="url(#g)"/>"##,
        );
        assert_eq!(at(&spread, 2, 10), at(&spread, 10, 10));
        reject(
            r##"<defs><linearGradient id="a" href="#a"/></defs><rect width="10" height="10" fill="url(#a)"/>"##,
            "cycle",
        );
    }

    #[test]
    fn svg_xml_utf8_and_unsupported_features_are_explicit() {
        let source=format!("\u{feff}<?xml version='1.0' encoding='UTF-8'?><!-- {} --><svg width='2' height='2'><title>Cat &amp; &#x1f431;</title><rect width='2' height='2'/></svg>", "a".repeat(4080)+"猫");
        assert!(looks_like(source.as_bytes()));
        assert_eq!(
            super::super::decode(source.as_bytes()).unwrap().pixels,
            vec![0xff000000; 4]
        );
        for body in [
            "<script>throw 1</script>",
            "<defs><script/></defs>",
            "<rect><animate attributeName='x'/></rect>",
            "<foreignObject/>",
        ] {
            reject(body, "static");
        }
        reject("<rect onclick='x()'/>", "handlers");
        reject("<image href='https://example.invalid/x'/>", "external");
        reject("<text>word</text>", "text rendering");
        reject("<rect filter='url(#x)'/>", "not implemented");
        reject("<rect width='3'height='4'/>", "whitespace");
        reject("<rect width='3' width='4'/>", "Duplicate");
        reject("<title>&unknown;</title>", "character references");
        reject("<rect fill='#ééé'/>", "hexadecimal");
        assert!(
            super::super::decode(b"<!DOCTYPE svg [<!ENTITY e 'x'>]><svg/>")
                .unwrap_err()
                .contains("document types")
        );
        assert!(
            super::super::decode(b"<?xml-stylesheet href='https://example.invalid/x'?><svg/>")
                .unwrap_err()
                .contains("processing")
        );
    }

    #[test]
    fn svg_clip_silhouettes_union_before_antialiasing_and_inherit_rules() {
        let union = svg(
            r##"<defs><clipPath id="c"><path d="M2 2H22V26H2Z"/><path d="M10 10V30H30V10Z"/></clipPath></defs><rect width="32" height="32" fill="red" clip-path="url(#c)"/>"##,
        );
        assert_eq!(at(&union, 16, 16), 0xffff0000); // Opposite winding does not cancel separate children.
        assert_eq!(at(&union, 1, 1), 0);
        assert_eq!(at(&union, 28, 28), 0xffff0000);
        let joined = svg(
            r##"<defs><clipPath id="c"><path d="M2 2H30V30Z"/><path d="M2 2V30H30Z"/></clipPath></defs><rect width="32" height="32" fill="blue" clip-path="url(#c)"/>"##,
        );
        for position in 3..29 {
            assert_eq!(at(&joined, position, position), 0xff0000ff);
        }
        let hole = svg(
            r##"<defs clip-rule="evenodd"><clipPath id="c" display="none" opacity="0"><path fill="none" stroke="red" stroke-width="9" d="M2 2H30V30H2Z M10 10H22V22H10Z" opacity="0"/></clipPath></defs><rect width="32" height="32" clip-rule="nonzero" fill="green" clip-path="url('#c')"/>"##,
        );
        assert_eq!(at(&hole, 4, 4), 0xff008000);
        assert_eq!(at(&hole, 16, 16), 0);
        let empty =
            svg(r##"<clipPath id="c"/><rect width="32" height="32" clip-path="url(#c)"/>"##);
        assert!(empty.pixels.iter().all(|&p| p == 0));
        let missing =
            svg(r##"<rect width="32" height="32" fill="red" clip-path="url(#missing)"/>"##);
        assert!(missing.pixels.iter().all(|&p| p == 0xffff0000));
    }

    #[test]
    fn svg_clip_object_boxes_use_geometry_and_intersections() {
        let boxed = svg(
            r##"<defs><clipPath id="c" clipPathUnits="objectBoundingBox"><rect x=".5" width=".5" height="1"/></clipPath></defs><rect x="8" y="8" width="16" height="16" fill="blue" stroke="red" stroke-width="8" clip-path="url(#c)"/>"##,
        );
        assert_eq!(at(&boxed, 12, 16), 0);
        assert_eq!(at(&boxed, 17, 16), 0xff0000ff);
        assert_eq!(at(&boxed, 22, 16), 0xffff0000);
        assert_eq!(at(&boxed, 26, 16), 0); // Stroke cannot enlarge the clip's object box.
        let nested = svg(
            r##"<defs><clipPath id="a"><rect x="4" y="4" width="16" height="24"/></clipPath><clipPath id="b" clip-path="url(#a)"><rect x="12" y="2" width="18" height="28"/></clipPath><clipPath id="c"><rect x="2" y="12" width="28" height="8" clip-path="url(#b)"/><rect width="4" height="4"/></clipPath></defs><g clip-path="url(#c)"><rect width="32" height="32" fill="blue"/></g>"##,
        );
        assert_eq!(at(&nested, 16, 16), 0xff0000ff);
        assert_eq!(at(&nested, 2, 2), 0xff0000ff);
        for (x, y) in [(8, 16), (24, 16), (16, 8), (16, 24)] {
            assert_eq!(at(&nested, x, y), 0);
        }
        let line = svg(
            r##"<clipPath id="c" clipPathUnits="objectBoundingBox"><rect width="1" height="1"/></clipPath><line x1="2" y1="16" x2="30" y2="16" stroke="red" stroke-width="8" clip-path="url(#c)"/>"##,
        );
        assert!(line.pixels.iter().all(|&p| p == 0));
        let scaled = svg(
            r##"<clipPath id="c" clipPathUnits="objectBoundingBox"><rect y=".5" width="1" height=".5"/></clipPath><path d="M0 0C0 1 1 0 1 0Z" transform="scale(100)" fill="blue" clip-path="url(#c)"/>"##,
        );
        // The cubic has maximum y=4/9; the lower-half clip starts at 22.222...
        // device pixels, even though the path's local coordinates are tiny.
        assert_eq!(at(&scaled, 20, 21), 0);
        assert_eq!(at(&scaled, 20, 23), 0xff0000ff);
    }

    #[test]
    fn svg_masks_luminance_alpha_and_group_opacity() {
        let luminance = svg(
            r##"<mask id="m"><rect width="8" height="32" fill="red"/><rect x="8" width="8" height="32" fill="lime"/><rect x="16" width="8" height="32" fill="blue"/><rect x="24" width="8" height="32" fill="white" opacity=".5"/></mask><rect width="32" height="32" fill="#123456" mask="url(#m)"/>"##,
        );
        for (x, alpha) in [(4, 54), (12, 182), (20, 18), (28, 128)] {
            assert_eq!(at(&luminance, x, 16), (alpha << 24) | 0x123456);
        }
        let alpha = svg(
            r##"<mask id="m" mask-type="alpha"><g opacity=".5"><rect x="2" y="2" width="20" height="20" fill="black"/><rect x="10" y="10" width="20" height="20" fill="red"/></g></mask><g mask="url(#m)" opacity=".5"><rect width="24" height="32" fill="blue"/><rect x="8" width="24" height="32" fill="green"/></g>"##,
        );
        assert_eq!(at(&alpha, 4, 4), 0x400000ff);
        assert_eq!(at(&alpha, 16, 16), 0x40008000);
        assert_eq!(at(&alpha, 28, 28), 0x40008000);
        assert_eq!(at(&alpha, 0, 0), 0);
        let override_mode = svg(
            r##"<mask id="m"><rect width="32" height="32" fill="black"/></mask><rect width="32" height="32" fill="red" style="mask-image:url(#m);mask-mode:alpha"/>"##,
        );
        assert!(override_mode.pixels.iter().all(|&p| p == 0xffff0000));
        let inherited = svg(
            r##"<mask id="m"><rect width="32" height="32" fill="black"/></mask><g style="mask-image:url(#m);mask-mode:alpha"><rect width="32" height="32" fill="blue" style="mask:inherit"/></g>"##,
        );
        assert!(inherited.pixels.iter().all(|&p| p == 0xff0000ff));
    }

    #[test]
    fn svg_mask_regions_content_units_and_original_styles() {
        let mask = svg(
            r##"<defs fill="white"><mask id="m" maskContentUnits="objectBoundingBox" x="0" y="0" width="1" height="1" display="none" opacity="0"><rect x=".5" width=".5" height="1"/></mask></defs><rect x="4" y="8" width="24" height="16" fill="blue" mask="url(#m)"/>"##,
        );
        assert_eq!(at(&mask, 8, 16), 0);
        assert_eq!(at(&mask, 20, 16), 0xff0000ff);
        let source = r##"<mask id="m" REGION><rect width="32" height="32" fill="white"/></mask><rect x="12" y="12" width="8" height="8" fill="blue" stroke="blue" stroke-width="12" mask="url(#m)"/>"##;
        // CSS Masking 1 §9.1 only installs a region when at least one region
        // attribute is present. A missing width/height then defaults to 120%.
        assert_eq!(at(&svg(&source.replace("REGION", "")), 7, 16), 0xff0000ff);
        let limited = svg(&source.replace("REGION", "x='-10%'"));
        assert_eq!(at(&limited, 7, 16), 0);
        assert_eq!(at(&limited, 16, 16), 0xff0000ff);
        for region in ["width='0'", "width='-1'", "height='0'"] {
            assert!(svg(&source.replace("REGION", region))
                .pixels
                .iter()
                .all(|&p| p == 0));
        }
        for reference in ["missing", "wrong"] {
            let bitmap = svg(&format!(
                r##"<g id="wrong"/><rect width="32" height="32" fill="red" mask="url(#{reference})"/>"##
            ));
            assert!(bitmap.pixels.iter().all(|&p| p == 0));
        }
    }

    #[test]
    fn svg_effect_cycles_unsupported_inputs_and_shared_layer_limit() {
        reject(
            r##"<clipPath id="c" clip-path="url(#c)"><rect width="32" height="32"/></clipPath><rect width="32" height="32" clip-path="url(#c)"/>"##,
            "cycle",
        );
        reject(
            r##"<mask id="m"><rect width="32" height="32" fill="white" mask="url(#m)"/></mask><rect width="32" height="32" mask="url(#m)"/>"##,
            "cycle",
        );
        reject(
            r##"<clipPath id="c"><g><rect width="32" height="32"/></g></clipPath><rect width="32" height="32" clip-path="url(#c)"/>"##,
            "clipPath child",
        );
        reject(
            r##"<path id="p" d="M0 0H20V20H0Z"/><use id="u" href="#p"/><clipPath id="c"><use href="#u"/></clipPath><rect width="32" height="32" clip-path="url(#c)"/>"##,
            "directly reference",
        );
        reject(
            r#"<rect width="32" height="32" clip-path="url(https://example.invalid/clip.svg#c)"/>"#,
            "external",
        );
        reject(
            r#"<rect width="32" height="32" mask="url(data:image/svg+xml,x)"/>"#,
            "external",
        );
        reject(
            r##"<rect width="32" height="32" clip-path="url('#c)"/>"##,
            "quoted",
        );
        reject(
            r##"<rect width="32" height="32" mask="url(#a),url(#b)"/>"##,
            "external",
        );
        reject(
            r#"<mask color-interpolation="linearRGB"/>"#,
            "not implemented",
        );
        reject(
            r##"<clipPath id="c" clipPathUnits="bad"/><rect width="32" height="32" clip-path="url(#c)"/>"##,
            "clipPathUnits",
        );
        let definitions = (0..10)
            .map(|i| format!(r##"<mask id="m{i}" mask="url(#m{})"/>"##, i + 1))
            .collect::<String>();
        let source = format!(
            r##"<svg width="1024" height="1024">{definitions}<rect width="1024" height="1024" mask="url(#m0)"/></svg>"##
        );
        assert!(decode(source.as_bytes(), &|| Ok(()))
            .unwrap_err()
            .contains("temporary-layer"));
    }

    #[test]
    fn svg_effect_expansion_and_compositing_remain_cancelable() {
        // A tiny source can otherwise expand exponentially through child clips.
        let mut definitions = String::from(r#"<clipPath id="c0"/>"#);
        for i in 1..14 {
            definitions.push_str(&format!(r##"<clipPath id="c{i}"><rect width="32" height="32" clip-path="url(#c{})"/><rect width="32" height="32" clip-path="url(#c{})"/></clipPath>"##,i-1,i-1));
        }
        let source = format!(
            r##"<svg width="32" height="32">{definitions}<rect width="32" height="32" clip-path="url(#c13)"/></svg>"##
        );
        let error = decode(source.as_bytes(), &|| Ok(())).unwrap_err();
        assert!(error.contains("expanded clip-node limit"), "{error}");
        let source = br##"<svg width="256" height="256"><mask id="m"><rect width="256" height="256" fill="white"/></mask><rect width="256" height="256" fill="red" mask="url(#m)"/></svg>"##;
        let calls = Cell::new(0);
        let expected = decode(source, &|| {
            calls.set(calls.get() + 1);
            Ok(())
        })
        .unwrap();
        assert!(expected.pixels.iter().all(|&p| p == 0xffff0000));
        // Cancel on every observed rendering checkpoint, including mask
        // generation/compositing and the final decode callback.
        let total = calls.get();
        assert!(total > 20);
        for stop in 2..=total {
            calls.set(0);
            assert_eq!(
                decode(source, &|| {
                    calls.set(calls.get() + 1);
                    if calls.get() == stop {
                        Err("cancelled SVG effects".into())
                    } else {
                        Ok(())
                    }
                })
                .unwrap_err(),
                "cancelled SVG effects"
            );
        }
    }

    #[test]
    fn svg_input_geometry_work_and_cancel_limits_are_enforced() {
        let overlong = format!("<svg>{}</svg>", " ".repeat(MAX_SOURCE));
        assert!(decode(overlong.as_bytes(), &|| Ok(()))
            .unwrap_err()
            .contains("1 MiB"));
        reject(&"<g/>".repeat(MAX_NODES), "node");
        reject(
            &format!("<path d='M0 0{}'/>", "l1 0".repeat(MAX_SEGMENTS)),
            "segment",
        );
        reject("<circle r='-1'/>", "negative");
        reject(
            "<rect transform='scale(10000000) scale(10000000)'/>",
            "transform",
        );
        let source = br#"<svg width="512" height="512"><rect width="512" height="512"/></svg>"#;
        let calls = Cell::new(0);
        let error = decode(source, &|| {
            calls.set(calls.get() + 1);
            if calls.get() >= 3 {
                Err("cancelled in SVG raster".into())
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(error, "cancelled in SVG raster");
        let source=br#"<svg width="32" height="32"><g opacity=".5"><g opacity=".5"><rect width="32" height="32"/></g></g></svg>"#;
        // The decode path above checks the shared opacity pixel budget; here a tiny
        // work budget exercises exhaustion without a slow allocation stress test.
        let mut work = Work {
            left: 5,
            since_check: 0,
            segments: 0,
            clips: 0,
            layers: 0,
            check: &|| Ok(()),
        };
        assert!(parse_xml(std::str::from_utf8(source).unwrap(), &mut work)
            .err()
            .unwrap()
            .contains("work limit"));
    }
}
