//! Finite solid Canvas 2D colors in the renderer's straight 0xAARRGGBB format.
//! Parsing shares CSS numeric and named-color handling. Unsupported CSS color
//! spaces, math, substitutions, escapes and system colors are not admitted.

pub(crate) use crate::css::{parse_solid_color as parse, ParsedColor};

/// HTML-compatible sRGB serialization: opaque RGB is lowercase six-digit hex;
/// nonopaque RGB uses legacy rgba() with an alpha that round-trips its byte.
pub(crate) fn serialize(argb: u32) -> String {
    let alpha = argb >> 24;
    let rgb = argb & 0x00ff_ffff;
    if alpha == 255 {
        return format!("#{rgb:06x}");
    }
    let alpha = legacy_alpha(alpha);
    format!(
        "rgba({}, {}, {}, {alpha})",
        (rgb >> 16) & 255,
        (rgb >> 8) & 255,
        rgb & 255,
    )
}

fn legacy_alpha(alpha: u32) -> String {
    if alpha == 0 {
        return "0".into();
    }
    if alpha == 255 {
        return "1".into();
    }
    // CSS Color 4's integer-alpha rule first preserves integer percentages.
    // Integer arithmetic avoids a floating-point tie changing e.g. 50% -> 128.
    let percent = (alpha * 100 + 127) / 255;
    if (percent * 255 + 50) / 100 == alpha {
        return format!("0.{percent:02}").trim_end_matches('0').to_owned();
    }
    let thousandths = (alpha * 1000 + 127) / 255;
    format!("0.{thousandths:03}")
        .trim_end_matches('0')
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canvas_color_names_hex_and_current_color() {
        for (text, argb) in [
            ("black", 0xff00_0000),
            ("TRANSPARENT", 0),
            ("\tReBeccAPurple\r\n", 0xff66_3399),
            ("#abc", 0xffaa_bbcc),
            ("#aBcD", 0xddaa_bbcc),
            ("#123456", 0xff12_3456),
            ("#12345678", 0x7812_3456),
            ("#0000", 0),
        ] {
            assert_eq!(parse(text), Some(ParsedColor::Solid(argb)), "{text}");
        }
        assert_eq!(parse(" currentColor "), Some(ParsedColor::CurrentColor));
    }

    #[test]
    fn canvas_color_rgb_aliases_units_clamping_and_finite_numbers() {
        for (text, argb) in [
            ("rgb(255, 0, 128)", 0xffff_0080),
            ("rgba(255, 0, 128)", 0xffff_0080),
            ("rgb(100%, 0%, 50%, 50%)", 0x80ff_0080),
            ("rgba(1e2, +.5, 2, .25)", 0x4064_0102),
            ("RGB(100% 0 50% / .5)", 0x80ff_0080),
            ("rgba(1\t2\n3 / 20%)", 0x3301_0203),
            ("rgb(-2, 270, 1.5, 4)", 0xff00_ff02),
            ("rgba(10%, -5%, 200%, -1)", 0x001a_00ff),
        ] {
            assert_eq!(parse(text), Some(ParsedColor::Solid(argb)), "{text}");
        }
    }

    #[test]
    fn canvas_color_invalid_and_unsupported_values_are_not_colors() {
        for text in [
            "",
            "inherit",
            "initial",
            "unset",
            "revert",
            "revert-layer",
            "none",
            "#",
            "#+12",
            "#12345",
            "#123456789",
            "red junk",
            "réd",
            "rgb(1,,2,3)",
            "rgb(1 2,3)",
            "rgb(1,2,3,)",
            "rgb(1,2,3 / .5)",
            "rgb(1% 2 3, .5)",
            "rgb(1%,2,3%)",
            "rgba(1,2,3,NaN)",
            "rgb(1. 2 3)",
            "rgb(1.e2 2 3)",
            "rgb(1e999 2 3)",
            "rgb(1 2 3 / / .5)",
            "rgb(1 2 3 / .5 .6)",
            "rgb(1 2 3))",
            "rgb(1\x0b2 3)",
            "\x0bred",
            "rgb(none 0 0)",
            "rgb(calc(1) 2 3)",
            "var(--color)",
            "hsl(0 100% 50%)",
            "color(display-p3 1 0 0)",
            "CanvasText",
            "forestgreen",
            "r\\65 d",
            "/*x*/red",
        ] {
            assert_eq!(parse(text), None, "{text}");
        }
        assert_eq!(parse(&" ".repeat(2049)), None);
    }

    #[test]
    fn canvas_color_html_serialization_and_legacy_alpha() {
        for (argb, expected) in [
            (0xff00_0000, "#000000"),
            (0xffab_cdef, "#abcdef"),
            (0, "rgba(0, 0, 0, 0)"),
            (0x8001_0203, "rgba(1, 2, 3, 0.5)"),
            (0xedff_00ff, "rgba(255, 0, 255, 0.93)"),
            (0xecff_00ff, "rgba(255, 0, 255, 0.925)"),
            (0x0100_0000, "rgba(0, 0, 0, 0.004)"),
        ] {
            assert_eq!(serialize(argb), expected);
        }
    }

    #[test]
    fn canvas_color_serialization_round_trips_all_alpha_bytes() {
        for alpha in 0..=255 {
            let argb = (alpha << 24) | 0x0012_abef;
            let serialized = serialize(argb);
            assert_eq!(parse(&serialized), Some(ParsedColor::Solid(argb)));
            assert!(serialized.len() <= 28);
        }
    }
}
