//! Page font selection and a bounded glyph cache. System font files are input
//! data; all parsing, measuring and rasterization belongs to our font module.
use crate::font::{Font, GlyphBitmap};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

const MAX_WEB_FACES: usize = 24;
const MAX_CACHE_BYTES: usize = 4 * 1024 * 1024;
const MAX_CACHE_GLYPHS: usize = 1024;

#[derive(Debug, Default)]
struct GlyphCache {
    images: HashMap<(u16, u16), Result<Arc<GlyphBitmap>, String>>,
    bytes: usize,
}

#[derive(Debug)]
pub struct Face {
    pub font: Arc<Font>,
    pub family: String,
    pub weight: u16,
    pub italic: bool,
    aliases: Vec<String>,
    ranges: Vec<(u32, u32)>,
    web: bool,
    cache: Mutex<GlyphCache>,
}

impl Face {
    pub fn supports(&self, character: char) -> bool {
        let code = character as u32;
        (self.ranges.is_empty() || self.ranges.iter().any(|&(a, b)| (a..=b).contains(&code)))
            && self.font.glyph_index(character) != 0
    }

    pub fn glyph(&self, glyph: u16, size: f32) -> Result<Arc<GlyphBitmap>, String> {
        if !size.is_finite() || !(0.25..=512.0).contains(&size) {
            return Err("Font raster size is outside the supported range.".into());
        }
        let size_key = (size * 64.0).round().max(1.0) as u16;
        let key = (glyph, size_key);
        {
            let cache = self
                .cache
                .lock()
                .map_err(|_| "Font glyph cache is unavailable.")?;
            if let Some(image) = cache.images.get(&key) {
                return image.clone();
            }
        }
        // Cache failures as well: repeated malformed outlines must not consume
        // the font decoder's complete work budget again on every repaint.
        let image = self
            .font
            .rasterize(glyph, f32::from(size_key) / 64.0)
            .map(Arc::new);
        let bytes = match &image {
            Ok(image) => image.coverage.len(),
            Err(error) => error.len(),
        };
        if bytes <= MAX_CACHE_BYTES {
            let mut cache = self
                .cache
                .lock()
                .map_err(|_| "Font glyph cache is unavailable.")?;
            if let Some(previous) = cache.images.get(&key) {
                return previous.clone();
            }
            if cache.images.len() >= MAX_CACHE_GLYPHS
                || cache.bytes.saturating_add(bytes) > MAX_CACHE_BYTES
            {
                cache.images.clear();
                cache.bytes = 0;
            }
            cache.bytes += bytes;
            cache.images.insert(key, image.clone());
        }
        image
    }

    fn named(&self, family: &str) -> bool {
        self.family.eq_ignore_ascii_case(family)
            || self
                .aliases
                .iter()
                .any(|alias| alias.eq_ignore_ascii_case(family))
    }
}

#[derive(Clone, Debug, Default)]
pub struct Book {
    faces: Vec<Arc<Face>>,
}

impl Book {
    /// Empty books keep deterministic bitmap fallback available to engine tests.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Parse the small named set once per process; never enumerate it into JS.
    pub fn system() -> Self {
        static SYSTEM: OnceLock<Book> = OnceLock::new();
        SYSTEM.get_or_init(load_system).clone()
    }

    pub fn extend(&mut self, other: &Book) {
        let remaining = 96usize.saturating_sub(self.faces.len());
        self.faces
            .extend(other.faces.iter().take(remaining).cloned());
    }

    pub fn len(&self) -> usize {
        self.faces.len()
    }

    #[cfg(test)]
    pub fn add(
        &mut self,
        family: String,
        weight: u16,
        italic: bool,
        font: Arc<Font>,
    ) -> Result<(), String> {
        self.add_with_ranges(family, weight, italic, font, Vec::new())
    }

    pub fn add_with_ranges(
        &mut self,
        family: String,
        weight: u16,
        italic: bool,
        font: Arc<Font>,
        ranges: Vec<(u32, u32)>,
    ) -> Result<(), String> {
        if self.faces.iter().filter(|face| face.web).count() >= MAX_WEB_FACES {
            return Err("Page font-face limit reached (24).".into());
        }
        if family.is_empty()
            || family.len() > 256
            || !(1..=1000).contains(&weight)
            || ranges.len() > 128
            || ranges.iter().any(|&(a, b)| a > b || b > 0x10ffff)
        {
            return Err("Invalid or oversized font-face descriptor.".into());
        }
        self.faces.push(Arc::new(Face {
            font,
            family,
            weight,
            italic,
            aliases: Vec::new(),
            ranges,
            web: true,
            cache: Mutex::new(GlyphCache::default()),
        }));
        Ok(())
    }

    pub fn select(&self, families: &[String], weight: u16, italic: bool) -> Option<Arc<Face>> {
        self.select_for(families, weight, italic, None)
    }

    pub fn resolve(
        &self,
        families: &[String],
        weight: u16,
        italic: bool,
        character: char,
    ) -> Option<Arc<Face>> {
        self.select_for(families, weight, italic, Some(character))
    }

    pub fn local(&self, name: &str, weight: u16, italic: bool) -> Option<Arc<Face>> {
        self.best(name, weight, italic, None, true)
    }

    fn select_for(
        &self,
        families: &[String],
        weight: u16,
        italic: bool,
        character: Option<char>,
    ) -> Option<Arc<Face>> {
        for family in
            families
                .iter()
                .map(String::as_str)
                .chain(["serif", "sans-serif", "monospace"])
        {
            if let Some(face) = self.best(family, weight, italic, character, false) {
                return Some(face);
            }
        }
        None
    }

    fn best(
        &self,
        family: &str,
        weight: u16,
        italic: bool,
        character: Option<char>,
        local_only: bool,
    ) -> Option<Arc<Face>> {
        let has_web = !local_only && self.faces.iter().any(|face| face.web && face.named(family));
        self.faces
            .iter()
            .enumerate()
            .filter(|(_, face)| {
                face.named(family)
                    && (!local_only || !face.web)
                    && (!has_web || face.web)
                    && character.is_none_or(|ch| face.supports(ch))
            })
            .min_by_key(|(index, face)| {
                (
                    face.italic != italic,
                    weight_rank(weight, face.weight),
                    std::cmp::Reverse(*index),
                )
            })
            .map(|(_, face)| face.clone())
    }
}

// CSS searches 400..500 first upward to 500, then downward, then above 500.
fn weight_rank(requested: u16, found: u16) -> (u8, u16) {
    if (400..=500).contains(&requested) {
        if (requested..=500).contains(&found) {
            (0, found - requested)
        } else if found < requested {
            (1, requested - found)
        } else {
            (2, found - requested)
        }
    } else if requested < 400 {
        if found <= requested {
            (0, requested - found)
        } else {
            (1, found - requested)
        }
    } else if found >= requested {
        (0, found - requested)
    } else {
        (1, requested - found)
    }
}

fn system_face(
    book: &mut Book,
    directory: &Path,
    filename: &str,
    family: &str,
    aliases: &[&str],
    weight: u16,
    italic: bool,
) {
    let path = directory.join(filename);
    let Ok(bytes) = crate::navigation::read_limited(&path, 16 * 1024 * 1024) else {
        return;
    };
    let Ok(font) = Font::parse(&bytes) else {
        return;
    };
    book.faces.push(Arc::new(Face {
        font: Arc::new(font),
        family: family.into(),
        weight,
        italic,
        aliases: aliases.iter().map(|alias| (*alias).to_owned()).collect(),
        ranges: Vec::new(),
        web: false,
        cache: Mutex::new(GlyphCache::default()),
    }));
}

fn load_system() -> Book {
    let mut book = Book::empty();
    #[cfg(windows)]
    {
        let directory = std::env::var_os("WINDIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\Windows"))
            .join("Fonts");
        for (family, aliases, files) in [
            (
                "Arial",
                vec!["sans-serif", "ui-sans-serif", "Helvetica"],
                ["arial.ttf", "arialbd.ttf", "ariali.ttf", "arialbi.ttf"],
            ),
            (
                "Times New Roman",
                vec!["serif", "ui-serif", "Times"],
                ["times.ttf", "timesbd.ttf", "timesi.ttf", "timesbi.ttf"],
            ),
            (
                "Courier New",
                vec!["monospace", "ui-monospace", "Courier"],
                ["cour.ttf", "courbd.ttf", "couri.ttf", "courbi.ttf"],
            ),
            (
                "Segoe UI",
                vec!["system-ui", "-apple-system", "BlinkMacSystemFont"],
                [
                    "segoeui.ttf",
                    "segoeuib.ttf",
                    "segoeuii.ttf",
                    "segoeuiz.ttf",
                ],
            ),
            (
                "Verdana",
                vec![],
                [
                    "verdana.ttf",
                    "verdanab.ttf",
                    "verdanai.ttf",
                    "verdanaz.ttf",
                ],
            ),
            (
                "Georgia",
                vec![],
                [
                    "georgia.ttf",
                    "georgiab.ttf",
                    "georgiai.ttf",
                    "georgiaz.ttf",
                ],
            ),
            ("Tahoma", vec![], ["tahoma.ttf", "tahomabd.ttf", "", ""]),
            (
                "Consolas",
                vec![],
                [
                    "consola.ttf",
                    "consolab.ttf",
                    "consolai.ttf",
                    "consolaz.ttf",
                ],
            ),
            (
                "Trebuchet MS",
                vec![],
                ["trebuc.ttf", "trebucbd.ttf", "trebucit.ttf", "trebucbi.ttf"],
            ),
            (
                "Comic Sans MS",
                vec!["cursive"],
                ["comic.ttf", "comicbd.ttf", "comici.ttf", "comicz.ttf"],
            ),
        ] {
            for (i, file) in files
                .iter()
                .enumerate()
                .filter(|(_, file)| !file.is_empty())
            {
                system_face(
                    &mut book,
                    &directory,
                    file,
                    family,
                    &aliases,
                    if i % 2 == 1 { 700 } else { 400 },
                    i >= 2,
                );
            }
        }
    }
    #[cfg(not(windows))]
    {
        // Named, conventional data locations for headless QA. No directory scan.
        let directory = PathBuf::from("/usr/share/fonts/truetype/dejavu");
        for (family, aliases, files) in [
            (
                "DejaVu Sans",
                vec!["sans-serif", "ui-sans-serif", "system-ui"],
                [
                    "DejaVuSans.ttf",
                    "DejaVuSans-Bold.ttf",
                    "DejaVuSans-Oblique.ttf",
                    "DejaVuSans-BoldOblique.ttf",
                ],
            ),
            (
                "DejaVu Serif",
                vec!["serif", "ui-serif"],
                [
                    "DejaVuSerif.ttf",
                    "DejaVuSerif-Bold.ttf",
                    "DejaVuSerif-Italic.ttf",
                    "DejaVuSerif-BoldItalic.ttf",
                ],
            ),
            (
                "DejaVu Sans Mono",
                vec!["monospace", "ui-monospace"],
                [
                    "DejaVuSansMono.ttf",
                    "DejaVuSansMono-Bold.ttf",
                    "DejaVuSansMono-Oblique.ttf",
                    "DejaVuSansMono-BoldOblique.ttf",
                ],
            ),
        ] {
            for (i, file) in files.iter().enumerate() {
                system_face(
                    &mut book,
                    &directory,
                    file,
                    family,
                    &aliases,
                    if i % 2 == 1 { 700 } else { 400 },
                    i >= 2,
                );
            }
        }
    }
    book
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn font_selection_obeys_family_order_weight_and_ranges() {
        let font = Arc::new(Font::parse(&crate::font::tests::fixture_font()).unwrap());
        let mut book = Book::empty();
        book.add("One".into(), 400, false, font.clone()).unwrap();
        book.add("One".into(), 700, false, font.clone()).unwrap();
        book.add("Two".into(), 400, true, font.clone()).unwrap();
        assert_eq!(
            book.select(&["missing".into(), "one".into()], 600, false)
                .unwrap()
                .weight,
            700
        );
        assert_eq!(
            book.select(&["Two".into(), "One".into()], 400, false)
                .unwrap()
                .family,
            "Two"
        );
        book.add_with_ranges("One".into(), 400, false, font, vec![(86, 86)])
            .unwrap();
        let only_v = book.select(&["One".into()], 400, false).unwrap();
        assert!(only_v.supports('V'));
        assert!(!only_v.supports('A'));
        assert!(book
            .resolve(&["One".into()], 400, false, 'A')
            .unwrap()
            .supports('A'));
        assert_eq!(weight_rank(400, 500).0, 0);
        assert_eq!(weight_rank(400, 300).0, 1);
        assert_eq!(weight_rank(400, 600).0, 2);
    }

    #[test]
    fn glyph_cache_reuses_pixels_and_rejects_invalid_sizes() {
        let mut book = Book::empty();
        book.add(
            "Original".into(),
            400,
            false,
            Arc::new(Font::parse(&crate::font::tests::fixture_font()).unwrap()),
        )
        .unwrap();
        let face = book.select(&["Original".into()], 400, false).unwrap();
        let glyph = face.font.glyph_index('A');
        let first = face.glyph(glyph, 20.0).unwrap();
        assert!(Arc::ptr_eq(&first, &face.glyph(glyph, 20.0).unwrap()));
        assert!(!Arc::ptr_eq(&first, &face.glyph(glyph, 21.0).unwrap()));
        assert!(face.glyph(glyph, f32::INFINITY).is_err());
        let failed = face.glyph(u16::MAX, 20.0).unwrap_err();
        assert_eq!(face.glyph(u16::MAX, 20.0).unwrap_err(), failed);
        assert!(face
            .cache
            .lock()
            .unwrap()
            .images
            .get(&(u16::MAX, 1280))
            .unwrap()
            .is_err());
    }
}
