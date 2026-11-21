//! Bounded block/inline layout and a small, original flex formatting context.
//! Flex text is laid out once at its assigned width. Later height changes resize
//! stored boxes and translate their geometry, including nested flex children.
use std::collections::{BTreeMap, HashMap};

use crate::css::{
    self, Align, Display, FlexAlign, FlexDirection, FlexPack, Length, LineHeight, ListStyle,
    Position, Style, WhiteSpace,
};
use crate::fonts::Book;
use crate::forms::{self, Kind};
use crate::html::{Document, NodeKind};
use crate::paint::{ClipRect, GLYPH_ADVANCE, GLYPH_HEIGHT};
use crate::text::{FontSpec, Shape};

const MAX_RUNS: usize = 100_000;
const MAX_ITEMS: usize = 250_000;
const MAX_HEIGHT: i32 = 1_000_000;
const MAX_WIDTH: i32 = 32_768;
const MAX_DEPTH: usize = 128;
const MAX_FLEX_ITEMS: usize = 2_048;
const MAX_FLEX_WORK: usize = 4_000_000;
const MAX_POSITIONED: usize = 2_048;
#[path="layout_scroll.rs"]
mod scrolling;
#[path = "layout_offset_parent.rs"]
mod offset_parent;
pub(crate) use offset_parent::{offset_parent_storage, offset_parent_work, OffsetParent};
#[path = "layout_translate.rs"]
mod translate;
pub(crate) use translate::VisualBorderRect;
#[path = "layout_opacity.rs"]
mod opacity;
#[path = "layout_grid.rs"]
mod grid;
use grid::GridGroup;

#[cfg(test)]
#[path = "layout_background_tests.rs"]
mod background_tests;

#[cfg(test)]
#[path = "layout_native_padding_tests.rs"]
mod native_padding_tests;

#[cfg(test)]
#[path = "layout_offset_parent_tests.rs"]
mod offset_parent_tests;

#[cfg(test)]
#[path = "layout_intrinsic_text_tests.rs"]
mod intrinsic_text_tests;

#[cfg(test)]
#[path = "layout_implicit_grid_tests.rs"]
mod implicit_grid_tests;

#[cfg(test)]
#[path = "layout_grid_alignment_tests.rs"]
mod grid_alignment_tests;

fn native_button_padding(kind: Option<Kind>) -> bool {
    matches!(kind, Some(Kind::Button | Kind::Submit | Kind::Reset))
}

/// Use page-space limits for an open axis. Actual paint buffers and hit
/// rectangles intersect this clip with their finite bounds before drawing.
fn overflow_clip(mut padding: ClipRect, overflow: css::Overflow) -> Option<ClipRect> {
    match (overflow.clips_x(), overflow.clips_y()) {
        (false, false) => return None,
        (true, true) => {}
        (true, false) => {
            padding.top = i32::MIN;
            padding.bottom = i32::MAX;
        }
        (false, true) => {
            padding.left = i32::MIN;
            padding.right = i32::MAX;
        }
    }
    Some(padding)
}

fn clipped_bottom(bounds: ClipRect, clip: Option<ClipRect>) -> Option<i32> {
    // Preserve zero-width layout extents when they are not clipped.
    let Some(clip) = clip else {
        return Some(bounds.bottom);
    };
    let area = bounds.intersect(clip);
    (area.left < area.right && area.top < area.bottom).then_some(area.bottom)
}

#[derive(Clone, Copy, Default)]
struct Intrinsic {
    preferred: i32,
    minimum: i32,
}

#[derive(Clone, Copy)]
struct Mark {
    runs: usize,
    items: usize,
    atoms: usize,
    controls: usize,
    anchors: usize,
    boxes: usize,
    pending: usize,
    decorations: usize,
}

#[derive(Clone, Copy)]
struct Span {
    start: Mark,
    end: Mark,
}

struct FlexItem {
    nodes: Vec<usize>,
    style: Style,
    margin: [i32; 4],
    edges: [i32; 2], // Horizontal and vertical padding + borders.
    intrinsic: Intrinsic,
    base: f64,
    minimum: f64,
    maximum: f64,
    main: f64,
    width: i32,
    height: i32,
    natural_height: i32,
    minimum_height: i32,
    span: Option<Span>,
    x: i32,
    y: i32,
    replaced: bool,
    anonymous: bool,
}

struct FlexGroup {
    style: Style,
    items: Vec<FlexItem>,
    lines: Vec<std::ops::Range<usize>>,
}

enum FormattingContext {
    Flex(FlexGroup),
    Grid(GridGroup),
}

struct TableRow {
    node: usize,
    cells: Vec<usize>,
}

struct BoxRecord {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    natural_height: i32,
    definite_height: bool,
    padding: [i32; 4],
    border: [i32; 4],
    background: Option<usize>,
    background_image: Option<usize>,
    borders: [Option<usize>; 4],
    hit_box: Option<usize>,
    formatting: Option<FormattingContext>,
}

impl BoxRecord {
    fn grid_complete(&self) -> bool {
        !matches!(&self.formatting, Some(FormattingContext::Grid(group)) if !group.complete)
    }
}

struct Pending {
    node: usize,
    link: Option<usize>,
    label: Option<usize>,
    depth: usize,
    x: i32,
    y: i32,
    fixed: bool,
    placed_bounds: Option<ClipRect>,
}

#[derive(Clone, Copy)]
struct ContainingBlock {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    definite_height: bool,
}

#[derive(Debug)]
pub struct TextRun {
    pub x: i32,
    pub y: i32,
    pub text: String,
    pub scale: i32,
    pub color: u32,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    /// Text-stream separation; wrapping and font changes do not create spaces.
    pub separator_before: &'static str,
    #[cfg_attr(not(test), allow(dead_code))]
    pub link: Option<usize>,
    pixel_width: i32,
    pub shaped: Option<Shape>,
}

impl TextRun {
    pub fn width(&self) -> i32 {
        self.pixel_width
    }
    pub fn ascent(&self) -> i32 {
        self.shaped
            .as_ref()
            .map_or(GLYPH_HEIGHT * self.scale, |shape| shape.ascent)
    }
    pub fn height(&self) -> i32 {
        self.shaped
            .as_ref()
            .map_or((GLYPH_HEIGHT + 2) * self.scale, Shape::height)
    }
    pub fn character_x(&self, index: usize) -> i32 {
        self.shaped
            .as_ref()
            .map_or(index as i32 * GLYPH_ADVANCE * self.scale, |shape| {
                shape.x(index)
            })
    }
    pub fn character_at(&self, x: i32) -> usize {
        self.shaped.as_ref().map_or_else(
            || {
                let advance = GLYPH_ADVANCE * self.scale;
                (((x + advance / 2).max(0) / advance) as usize).min(self.text.chars().count())
            },
            |shape| shape.hit(x),
        )
    }
}

#[derive(Clone, Debug)]
pub enum PaintItem {
    Rect {
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        color: u32,
        alpha: u8,
    },
    /// One circular radius on an opaque block color background, optionally
    /// including one uniform solid border in the same boundary composite.
    /// The unnormalized radius is retained across final sizing/flex stretch.
    RoundedRect {
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        color: u32,
        alpha: u8,
        radius: f32,
        border: Option<crate::paint::UniformBorder>,
        /// Used physical color-clip insets; outer bounds remain border bounds.
        background_insets: [i32; 4],
    },
    Text(usize),
    Control(usize),
    /// A block decoration, independent of replaced-image atoms and intrinsic size.
    Background {
        node: usize,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        image: css::BackgroundImage,
        presentation: crate::paint::BackgroundPresentation,
        border: [i32; 4],
    },
    Image {
        node: usize,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        presentation: crate::paint::ImagePresentation,
    },
    /// Interactive box geometry participates in hit order but paints no pixels.
    HitBox {
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        link: Option<usize>,
        control: Option<usize>,
    },
}

#[derive(Clone)]
struct HitRegion {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    link: Option<usize>,
    control: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct ImageControl {
    /// Content coordinates relative to the control's border-box origin.
    /// All values are zoomed layout pixels; side arrays are top/right/bottom/left.
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub border: [i32; 4],
    pub border_color: [u32; 4],
    pub border_alpha: [u8; 4],
    pub presentation: crate::paint::ImagePresentation,
}

#[derive(Clone, Debug)]
pub struct ControlBox {
    pub node: usize,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub scale: i32,
    pub text_font: Option<FontSpec>,
    pub color: u32,
    pub background: Option<u32>,
    /// Authored native button-family padding, in zoomed top/right/bottom/left pixels.
    /// Other native control consumers retain their existing geometry paths.
    pub padding: [i32; 4],
    pub image: Option<ImageControl>,
}

#[derive(Default)]
struct FixedGeometry {
    runs: Vec<usize>,
    items: Vec<usize>,
    controls: Vec<usize>,
    anchors: Vec<usize>,
    hits: Vec<usize>,
}

/// The padding box and the leading borders actually used by the renderer.
/// Ordinary inline text has no client box; replaced elements do.
#[derive(Clone, Copy, Debug, Default)]
pub struct ClientBox {
    pub width: i32,
    pub height: i32,
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
    pub padding: [i32; 4],
}

/// Final unclipped border box in build coordinates. Fixed y is viewport-relative;
/// ordinary y is document-relative. Painting may translate fixed items on scroll,
/// while this canonical measurement remains unchanged for CSSOM snapshots.
#[derive(Clone, Copy, Debug)]
pub struct BorderRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub fixed: bool,
}

pub struct Layout {
    pub(crate) computed_properties: css::ComputedProperties,
    pub(crate) stylesheet_cache: Option<css::StylesheetCache>,
    pub runs: Vec<TextRun>,
    pub items: Vec<PaintItem>,
    pub(crate) opacity_groups: Vec<crate::paint::OpacityGroup>,
    pub(crate) opacity_unavailable: bool,
    pub(crate) opacity_unsupported: bool,
    pub(crate) opacity_draw_refused: std::cell::Cell<bool>,
    pub(crate) opacity_root_background_unsupported: bool,
    pub anchors: Vec<(String, i32)>,
    pub controls: Vec<ControlBox>,
    targets: Vec<String>,
    target_nodes: Vec<usize>,
    hits: Vec<HitRegion>,
    pub height: i32,
    pub client_boxes: Vec<Option<ClientBox>>,
    pub scroll_heights: Vec<Option<i32>>,
    pub viewport_scroll_height:i32,
    pub viewport_scroll_unavailable:bool,
    pub(crate) incomplete_grid_nodes: Vec<bool>,
    pub border_rects: Vec<Option<BorderRect>>,
    /// Empty vectors mean the raw rectangles are also the visual geometry.
    pub(crate) visual_border_rects: Vec<Option<VisualBorderRect>>,
    pub(crate) visual_unavailable: Vec<bool>,
    pub(crate) transform_translations: Vec<Option<[f64; 2]>>,
    /// No padding-box record is insufficient to prove no inline CSS box.
    pub offset_unavailable: Vec<bool>,
    pub(crate) offset_parents: Vec<OffsetParent>,
    fixed_runs: Vec<bool>,
    fixed_items: Vec<bool>,
    fixed_controls: Vec<bool>,
    fixed_anchors: Vec<bool>,
    fixed_hits: Vec<bool>,
    /// Zero is ordinary flow; positioned nodes use their document index + 1.
    positioned_items: Vec<usize>,
    item_clips: Vec<Option<ClipRect>>,
    run_clips: Vec<Option<ClipRect>>,
    fixed: FixedGeometry,
    scroll: i32,
}

impl Layout {
    pub fn link_targets(&self) -> impl Iterator<Item = &str> {
        self.targets.iter().map(String::as_str)
    }
    /// Positive clipped hit regions intersecting this viewport. Regions may
    /// overlap or be occluded; only link_at at an actual point proves a hit.
    pub(crate) fn viewport_link_regions(&self, width: i32, scroll: i32, height: i32) -> usize {
        if width <= 0 || height <= 0 { return 0; }
        self.hits.iter().filter(|hit| hit.link.is_some_and(|index| index < self.targets.len())
            && hit.width > 0 && hit.height > 0 && hit.x < width && hit.x.saturating_add(hit.width) > 0
            && hit.y < scroll.saturating_add(height) && hit.y.saturating_add(hit.height) > scroll).count()
    }
    pub fn link_node_at(&self, x: i32, y: i32) -> Option<usize> {
        for hit in self.hits.iter().rev() {
            if x >= hit.x
                && x < hit.x.saturating_add(hit.width)
                && y >= hit.y
                && y < hit.y.saturating_add(hit.height)
            {
                return hit
                    .link
                    .and_then(|index| self.target_nodes.get(index))
                    .copied();
            }
        }
        None
    }
    /// Fixed content uses the same document-coordinate interface as ordinary
    /// content. Translate only its geometry when the viewport scroll changes;
    /// no layout or paint pass is repeated and a down/up round trip is exact.
    pub fn set_scroll(&mut self, scroll: i32) {
        let scroll = scroll.clamp(0, MAX_HEIGHT);
        let delta = scroll - self.scroll;
        if delta == 0 {
            return;
        }
        self.scroll = scroll;
        for &index in &self.fixed.runs {
            self.runs[index].y += delta;
        }
        for &index in &self.fixed.items {
            if let Some(clip) = &mut self.item_clips[index] {
                *clip = clip.translate_y(delta);
            }
            match &mut self.items[index] {
                PaintItem::Rect { y, .. }
                | PaintItem::RoundedRect { y, .. }
                | PaintItem::Background { y, .. }
                | PaintItem::Image { y, .. }
                | PaintItem::HitBox { y, .. } => *y += delta,
                _ => {}
            }
        }
        for &index in &self.fixed.runs {
            if let Some(clip) = &mut self.run_clips[index] {
                *clip = clip.translate_y(delta);
            }
        }
        for &index in &self.fixed.controls {
            self.controls[index].y += delta;
        }
        for &index in &self.fixed.anchors {
            self.anchors[index].1 += delta;
        }
        for &index in &self.fixed.hits {
            self.hits[index].y += delta;
        }
    }

    pub fn run_is_fixed(&self, run_index: usize) -> bool {
        self.fixed_runs.get(run_index).copied().unwrap_or(false)
    }

    pub fn control_is_fixed(&self, node_id: usize) -> bool {
        self.controls
            .iter()
            .zip(&self.fixed_controls)
            .any(|(control, fixed)| control.node == node_id && *fixed)
    }

    pub fn anchor_is_fixed(&self, id: &str) -> bool {
        self.anchors
            .iter()
            .zip(&self.fixed_anchors)
            .find(|((name, _), _)| name == id)
            .is_some_and(|(_, fixed)| *fixed)
    }

    pub fn paint_is_positioned(&self, item_index: usize) -> bool {
        self.positioned_items.get(item_index).copied().unwrap_or(0) != 0
    }

    pub fn paint_clip(&self, item_index: usize) -> Option<ClipRect> {
        self.item_clips.get(item_index).copied().flatten()
    }

    /// Native video controls occupy their image item, including its paint order.
    /// A later painted/hit box at this point prevents activation through it.
    pub(crate) fn image_topmost_at(&self,node:usize,x:i32,y:i32)->bool {
        for (index,item) in self.items.iter().enumerate().rev(){
            if self.paint_clip(index).is_some_and(|clip|!clip.contains(x,y)){continue;}
            let rect=match item {
                PaintItem::Image {node:owner,x:left,y:top,width,height,..}=>{if ClipRect::new(*left,*top,*width,*height).contains(x,y){return *owner==node;}continue;},
                PaintItem::Rect {x,y,width,height,..} | PaintItem::RoundedRect {x,y,width,height,..} | PaintItem::Background {x,y,width,height,..}=>ClipRect::new(*x,*y,*width,*height),
                PaintItem::HitBox {x,y,width,height,link,control}=>{if link.is_none()&&control.is_none(){continue;}ClipRect::new(*x,*y,*width,*height)},
                PaintItem::Text(i)=>{let r=&self.runs[*i];ClipRect::new(r.x,r.y,r.width(),r.height())},
                PaintItem::Control(i)=>{let r=&self.controls[*i];ClipRect::new(r.x,r.y,r.width,r.height)},
            };
            if rect.contains(x,y){return false;}
        }
        false
    }

    pub fn run_clip(&self, run_index: usize) -> Option<ClipRect> {
        self.run_clips.get(run_index).copied().flatten()
    }

    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    pub fn control_at(&self, x: i32, y: i32) -> Option<usize> {
        for hit in self.hits.iter().rev() {
            if x >= hit.x
                && x < hit.x.saturating_add(hit.width)
                && y >= hit.y
                && y < hit.y.saturating_add(hit.height)
            {
                return hit.control;
            }
        }
        None
    }
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    pub fn link_at(&self, x: i32, y: i32) -> Option<&str> {
        for hit in self.hits.iter().rev() {
            if x >= hit.x
                && x < hit.x.saturating_add(hit.width)
                && y >= hit.y
                && y < hit.y.saturating_add(hit.height)
            {
                return hit
                    .link
                    .and_then(|index| self.targets.get(index))
                    .map(String::as_str);
            }
        }
        None
    }
}

#[derive(Clone, Copy)]
enum AtomKind {
    Text(usize),
    Image { item: usize, node: usize },
    Control(usize),
    InlineBlock(Span),
}

struct Atom {
    kind: AtomKind,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    ascent: i32,
    line: usize,
    link: Option<usize>,
    control: Option<usize>,
    /// Internal lines in an atomic inline box do not become fragments of an
    /// enclosing inline decoration. Its single outer atom belongs to its parent.
    context: Option<usize>,
}

struct Flow {
    left: i32,
    right: i32,
    x: i32,
    y: i32,
    line_height: i32,
    ascent: i32,
    descent: i32,
    atoms: Vec<usize>,
    space_pending: bool,
    align: Align,
    white_space: WhiteSpace,
}

impl Flow {
    fn new(left: i32, right: i32, y: i32, align: Align, white_space: WhiteSpace) -> Self {
        Self {
            left,
            right: right.max(left + 1),
            x: left,
            y,
            line_height: 0,
            ascent: 0,
            descent: 0,
            atoms: Vec::new(),
            space_pending: false,
            align,
            white_space,
        }
    }
}

struct Decoration {
    node: usize,
    before: usize,
    depth: usize,
    start: usize,
    end: usize,
    background: Option<u32>,
    border: [i32; 4],
    padding: [i32; 4],
    colors: [u32; 4],
    alpha: [u8; 4],
    items: Vec<PaintItem>,
    context: Option<usize>,
    link: Option<usize>,
    control: Option<usize>,
}

struct DecorationGroup {
    depth: usize,
    fixed: bool,
    layer: usize,
    clip: Option<ClipRect>,
    opacity: usize,
    items: Vec<PaintItem>,
}

struct TextInlineBox {
    node: usize,
    edges: [i32; 4],
}

struct Engine<'a> {
    fonts: &'a Book,
    doc: &'a Document,
    styles: Vec<Style>,
    image_sizes: &'a HashMap<usize, (usize, usize)>,
    zoom: f32,
    layout: Layout,
    flow: Flow,
    atoms: Vec<Atom>,
    decorations: Vec<Decoration>,
    lists: Vec<i32>,
    line_number: usize,
    visits: usize,
    pending_separator: &'static str,
    labels: HashMap<usize, usize>,
    active_label: Option<usize>,
    intrinsics: Vec<Option<Intrinsic>>,
    canvas_sizes: HashMap<usize, (crate::html_canvas::NaturalDimension, crate::html_canvas::NaturalDimension)>,
    measure_visits: usize,
    boxes: Vec<Option<BoxRecord>>,
    // Inline formatting establishes a box even when fragment dimensions are unavailable.
    inline_boxes: Vec<bool>,
    text_inline_boxes: Vec<TextInlineBox>,
    used_margins:HashMap<usize,[i32;4]>,
    placed_boxes: Vec<usize>,
    flex_work: usize,
    grid_budget: grid::GridBudget,
    viewport: ContainingBlock,
    parents: Vec<Option<usize>>,
    spans: Vec<Option<Span>>,
    pending: Vec<Pending>,
    positioning: Option<usize>,
    relatives: Vec<usize>,
    relative_cursor: usize,
    current_node: usize,
    run_nodes: Vec<usize>,
    anchor_nodes: Vec<usize>,
    decoration_cursor: usize,
    decoration_items: usize,
    inline_context: Option<usize>,
    translated_ranks: Vec<usize>,
    translated_owners: Option<(Vec<usize>, Vec<usize>)>,
    opacity_nodes: Vec<usize>,
    opacity_items: Vec<usize>,
    opacity_contexts: Vec<opacity::Context>,
}

/// Scripts and renderer use the document's full media viewport. Standalone
/// layouts know the dimensions, but have no primary-input device policy.
pub(crate) fn viewport_style_environment(
    doc: &Document, width: i32, height: i32, zoom: f32,
) -> css::MediaEnvironment {
    doc.media_environment.unwrap_or_else(|| {
        let mut environment = css::MediaEnvironment::desktop_viewport(width, height, zoom);
        environment.hover = css::Hover::Unknown;
        environment
    })
}

#[cfg(test)]
pub fn layout(doc: &Document, width: i32) -> Layout {
    build(doc, width, 1.0, &HashMap::new(), &HashMap::new())
}

#[cfg(test)]
pub fn build(
    doc: &Document,
    width: i32,
    zoom: f32,
    external_styles: &HashMap<usize, String>,
    image_sizes: &HashMap<usize, (usize, usize)>,
) -> Layout {
    build_viewport(doc, width, 760, zoom, external_styles, image_sizes)
}

pub fn build_viewport(
    doc: &Document,
    width: i32,
    height: i32,
    zoom: f32,
    external_styles: &HashMap<usize, String>,
    image_sizes: &HashMap<usize, (usize, usize)>,
) -> Layout {
    build_viewport_with_state(doc, width, height, zoom, external_styles, image_sizes, None)
}

/// Form values are live state; they must not be confused with initial HTML
/// checked/selected attributes when matching dynamic CSS selectors.
pub fn build_viewport_with_state(
    doc: &Document,
    width: i32,
    height: i32,
    zoom: f32,
    external_styles: &HashMap<usize, String>,
    image_sizes: &HashMap<usize, (usize, usize)>,
    checked: Option<&dyn Fn(usize) -> Option<bool>>,
) -> Layout {
    build_viewport_with_fonts(
        doc,
        width,
        height,
        zoom,
        external_styles,
        image_sizes,
        checked,
        &Book::empty(),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn build_viewport_with_fonts(
    doc: &Document,
    width: i32,
    height: i32,
    zoom: f32,
    external_styles: &HashMap<usize, String>,
    image_sizes: &HashMap<usize, (usize, usize)>,
    checked: Option<&dyn Fn(usize) -> Option<bool>>,
    fonts: &Book,
) -> Layout {
    build_viewport_with_fonts_inner(doc, width, height, zoom, external_styles, image_sizes, checked, fonts, None, None)
        .expect("Default rendering has no caller-budget refusal.")
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_viewport_with_fonts_budgeted(
    doc: &Document, width: i32, height: i32, zoom: f32,
    external_styles: &HashMap<usize, String>, image_sizes: &HashMap<usize, (usize, usize)>,
    checked: Option<&dyn Fn(usize) -> Option<bool>>, fonts: &Book, budget: &mut css::CssBudget<'_>,
) -> Result<Layout, String> {
    build_viewport_with_fonts_inner(doc, width, height, zoom, external_styles, image_sizes, checked, fonts, Some(budget), None)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_viewport_with_fonts_cached(
    doc:&Document,width:i32,height:i32,zoom:f32,external_styles:&HashMap<usize,String>,
    image_sizes:&HashMap<usize,(usize,usize)>,checked:Option<&dyn Fn(usize)->Option<bool>>,
    fonts:&Book,cache:Option<&css::StylesheetCache>,
) -> Layout {
    build_viewport_with_fonts_inner(doc,width,height,zoom,external_styles,image_sizes,checked,fonts,None,cache)
        .expect("Default rendering has no caller-budget refusal.")
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_viewport_with_fonts_budgeted_cached(
    doc:&Document,width:i32,height:i32,zoom:f32,external_styles:&HashMap<usize,String>,
    image_sizes:&HashMap<usize,(usize,usize)>,checked:Option<&dyn Fn(usize)->Option<bool>>,
    fonts:&Book,budget:&mut css::CssBudget<'_>,cache:Option<&css::StylesheetCache>,
) -> Result<Layout,String> {
    build_viewport_with_fonts_inner(doc,width,height,zoom,external_styles,image_sizes,checked,fonts,Some(budget),cache)
}

#[allow(clippy::too_many_arguments)]
fn build_viewport_with_fonts_inner(
    doc: &Document, width: i32, height: i32, zoom: f32,
    external_styles: &HashMap<usize, String>, image_sizes: &HashMap<usize, (usize, usize)>,
    checked: Option<&dyn Fn(usize) -> Option<bool>>, fonts: &Book, mut budget: Option<&mut css::CssBudget<'_>>,
    stylesheet_cache: Option<&css::StylesheetCache>,
) -> Result<Layout, String> {
    let width = width.clamp(1, 16_384);
    let zoom = if zoom.is_finite() {
        zoom.clamp(0.25, 5.0)
    } else {
        1.0
    };
    let environment = viewport_style_environment(doc, width, height, zoom);
    let cascade = match budget.as_deref_mut() {
        Some(meter) if stylesheet_cache.is_some() => css::compute_with_environment_and_properties_transform_budgeted_cached(
            doc,external_styles,checked,Some(environment),meter,stylesheet_cache,
        )?,
        Some(meter) => css::compute_with_environment_and_properties_transform_budgeted(
            doc, external_styles, checked, Some(environment), meter,
        )?,
        None => css::compute_with_environment_and_properties_cached(
            doc, external_styles, checked, Some(environment), stylesheet_cache,
        ),
    };
    let css::CascadeOutput { styles, computed_properties, stylesheet_cache } = cascade;
    if let Some(meter) = budget.as_deref_mut() {
        if computed_properties.has_transform_geometry() {
            // Additional containing-block/fixed-family walks and the fixed
            // ancestor context retained only for transform-bearing cascades.
            meter.work(doc.nodes.len().saturating_mul(4 * MAX_DEPTH))?;
            meter.copies(doc.nodes.len().saturating_mul(std::mem::size_of::<OffsetParent>()))?;
        }
    }
    let mut page = Layout {
        computed_properties,
        stylesheet_cache,
        runs: Vec::new(),
        items: Vec::new(),
        opacity_groups: Vec::new(),
        opacity_unavailable: false,
        opacity_unsupported: false,
        opacity_draw_refused: std::cell::Cell::new(false),
        opacity_root_background_unsupported: false,
        anchors: Vec::new(),
        controls: Vec::new(),
        targets: Vec::new(),
        target_nodes: Vec::new(),
        hits: Vec::new(),
        height: 0,
        client_boxes: vec![None; doc.nodes.len()],
        scroll_heights:vec![None;doc.nodes.len()],
        viewport_scroll_height:0,
        viewport_scroll_unavailable:false,
        incomplete_grid_nodes: Vec::new(),
        border_rects: vec![None; doc.nodes.len()],
        visual_border_rects: Vec::new(),
        visual_unavailable: Vec::new(),
        transform_translations: Vec::new(),
        offset_unavailable: vec![true; doc.nodes.len()],
        offset_parents: vec![OffsetParent::Unavailable; doc.nodes.len()],
        fixed_runs: Vec::new(),
        fixed_items: Vec::new(),
        fixed_controls: Vec::new(),
        fixed_anchors: Vec::new(),
        fixed_hits: Vec::new(),
        positioned_items: Vec::new(),
        item_clips: Vec::new(),
        run_clips: Vec::new(),
        fixed: FixedGeometry::default(),
        scroll: 0,
    };
    // The root background extends over the page viewport, including short pages.
    let root_background = ["html", "body"].iter().find_map(|wanted| {
        doc.nodes.iter().enumerate().find_map(|(index, node)| {
            if matches!(&node.kind, NodeKind::Element { tag, .. } if tag == wanted) {
                styles
                    .get(index)
                    .filter(|style| style.display != Display::None)
                    .and_then(|style| style.background)
            } else {
                None
            }
        })
    });
    if let Some(color) = root_background {
        page.items.push(PaintItem::Rect {
            x: 0,
            y: 0,
            width,
            height: MAX_HEIGHT,
            color,
            alpha: 255,
        });
    }
    let mut parents = vec![None; doc.nodes.len()];
    let mut canvas_sizes = HashMap::new();
    for (parent, node) in doc.nodes.iter().enumerate() {
        if doc.scripting_enabled && crate::html_canvas::element(doc, parent) {
            let NodeKind::Element { attrs, .. } = &node.kind else { unreachable!() };
            if let Some(meter) = budget.as_deref_mut() {
                meter.work(attrs.len().saturating_mul(2))?;
            }
            let raw_width = doc.attr(parent, "width");
            let raw_height = doc.attr(parent, "height");
            if let Some(meter) = budget.as_deref_mut() {
                meter.work(raw_width.map_or(1, str::len).saturating_add(raw_height.map_or(1, str::len)))?;
                meter.copies(3 * std::mem::size_of::<(usize, (crate::html_canvas::NaturalDimension, crate::html_canvas::NaturalDimension))>())?;
                canvas_sizes.try_reserve(1).map_err(|_| "Canvas natural-dimension storage reservation failed.")?;
            }
            canvas_sizes.insert(parent, (
                crate::html_canvas::natural_dimension(raw_width, "width"),
                crate::html_canvas::natural_dimension(raw_height, "height"),
            ));
        }
        for &child in &node.children {
            if child < parents.len() && child != 0 && parents[child].is_none() {
                parents[child] = Some(parent);
            }
        }
    }
    let mut engine = Engine {
        fonts,
        doc,
        styles,
        image_sizes,
        zoom,
        layout: page,
        flow: Flow::new(0, width, 0, Align::Left, WhiteSpace::Normal),
        atoms: Vec::new(),
        decorations: Vec::new(),
        lists: Vec::new(),
        line_number: 0,
        visits: 0,
        pending_separator: "",
        labels: forms::label_targets(doc),
        active_label: None,
        intrinsics: vec![None; doc.nodes.len()],
        canvas_sizes,
        measure_visits: 0,
        boxes: (0..doc.nodes.len()).map(|_| None).collect(),
        inline_boxes: vec![false; doc.nodes.len()],
        text_inline_boxes: Vec::new(),
        used_margins:HashMap::new(),
        placed_boxes: Vec::new(),
        flex_work: MAX_FLEX_WORK,
        grid_budget: grid::GridBudget::new(budget.as_deref()),
        viewport: ContainingBlock {
            x: 0,
            y: 0,
            width,
            height: height.clamp(1, 16_384),
            definite_height: true,
        },
        parents,
        spans: vec![None; doc.nodes.len()],
        pending: Vec::new(),
        positioning: None,
        relatives: Vec::new(),
        relative_cursor: 0,
        current_node: 0,
        run_nodes: Vec::new(),
        anchor_nodes: Vec::new(),
        decoration_cursor: 0,
        decoration_items: 0,
        inline_context: None,
        translated_ranks: Vec::new(),
        translated_owners: None,
        opacity_nodes: Vec::new(),
        opacity_items: Vec::new(),
        opacity_contexts: Vec::new(),
    };
    if !doc.nodes.is_empty() {
        engine.walk(0, None, 0);
    }
    engine.finish_line(false, 0);
    engine.cache_decorations();
    engine.apply_relative();
    engine.place_positioned();
    if let Some(meter) = budget.as_deref_mut() {
        engine.grid_budget.settle(meter)?;
    }
    let fixed_boxes = engine.mark_fixed();
    engine.finish_client_boxes();
    engine.finish_offset_parents();
    engine.finish_border_rects(&fixed_boxes);
    engine.finish_scroll_heights(&fixed_boxes);
    if engine.grid_budget.incomplete {
        engine.withhold_incomplete_grid_geometry(budget.as_deref_mut())?;
    }
    let inline_rects = engine.measure_text_inline_rects(budget.as_deref_mut())?;
    engine.apply_translations(budget.as_deref_mut(), &fixed_boxes, &inline_rects)?;
    // Inline client geometry is a readback. Flow and overflow still use the
    // existing layout boxes, paint fragments and hit regions.
    for (node, rect) in inline_rects {
        engine.layout.border_rects[node].get_or_insert(rect);
    }
    engine.prepare_opacity(budget.as_deref_mut())?;
    let atom_clips = engine.clip_overflow();
    // Flex children may move up as well as down after their single paint pass.
    // Recompute the final extent instead of retaining intermediate coordinates.
    let painted_bottom = engine
        .atoms
        .iter()
        .enumerate()
        .filter(|(index, _)| !engine.layout.fixed_hits[*index])
        .filter_map(|(index, atom)| {
            clipped_bottom(ClipRect::new(atom.x, atom.y, atom.width, atom.height), atom_clips[index])
        })
        .chain(
            engine
                .boxes
                .iter()
                .enumerate()
                .filter(|(node, _)| !fixed_boxes[*node])
                .filter_map(|(node, b)| b.as_ref().map(|b| (node, b)))
                .filter_map(|(node, b)| {
                    let bounds = ClipRect::new(
                        b.x,
                        b.y,
                        b.width + b.padding[1] + b.padding[3] + b.border[1] + b.border[3],
                        b.height + b.padding[0] + b.padding[2] + b.border[0] + b.border[2],
                    );
                    clipped_bottom(bounds, engine.ancestor_clip(node))
                }),
        )
        .chain(
            engine
                .pending
                .iter()
                .filter(|pending| !pending.fixed)
                .filter_map(|pending| {
                    clipped_bottom(pending.placed_bounds?, engine.ancestor_clip(pending.node))
                }),
        )
        .chain(
            engine
                .decorations
                .iter()
                .filter(|decoration| !engine.decoration_layer(decoration.node).1)
                .flat_map(|decoration| {
                    decoration.items.iter().filter_map(|item| match item {
                        PaintItem::Rect {
                            x,
                            y,
                            width,
                            height,
                            ..
                        } => clipped_bottom(
                            ClipRect::new(*x, *y, *width, *height),
                            engine.ancestor_clip(decoration.node),
                        ),
                        _ => None,
                    })
                }),
        )
        .max()
        .unwrap_or(0);
    engine.layout.height = engine
        .flow
        .y
        .max(painted_bottom)
        .saturating_add(16)
        .clamp(16, MAX_HEIGHT);
    engine.finish_decorations();
    engine.order_positioned_paint();
    engine.finish_opacity();
    engine.build_hits();
    engine.restore_source_order();
    let indices = |flags: &[bool]| {
        flags
            .iter()
            .enumerate()
            .filter_map(|(index, flag)| flag.then_some(index))
            .collect()
    };
    engine.layout.fixed = FixedGeometry {
        runs: indices(&engine.layout.fixed_runs),
        items: indices(&engine.layout.fixed_items),
        controls: indices(&engine.layout.fixed_controls),
        anchors: indices(&engine.layout.fixed_anchors),
        hits: indices(&engine.layout.fixed_hits),
    };
    Ok(engine.layout)
}

impl Engine<'_> {
    fn finish_border_rects(&mut self, fixed_boxes: &[bool]) {
        for (node, record) in self.boxes.iter().enumerate() {
            if let Some(record) = record {
                if !record.grid_complete() { continue; }
                self.layout.border_rects[node] = Some(BorderRect {
                    x: record.x, y: record.y,
                    width: record.width + record.padding[1] + record.padding[3] + record.border[1] + record.border[3],
                    height: record.height + record.padding[0] + record.padding[2] + record.border[0] + record.border[2],
                    fixed: fixed_boxes.get(node).copied().unwrap_or(false),
                });
            }
        }
        for (index, item) in self.layout.items.iter().enumerate() {
            if let PaintItem::Image { node, x, y, .. } = item {
                if let Some(client) = self.layout.client_boxes[*node] {
                    self.layout.border_rects[*node] = Some(BorderRect {
                        x: *x - client.padding[3] - client.left,
                        y: *y - client.padding[0] - client.top,
                        width: client.width + client.left + client.right,
                        height: client.height + client.top + client.bottom,
                        fixed: self.layout.fixed_items.get(index).copied().unwrap_or(false),
                    });
                }
            }
        }
        for (index, control) in self.layout.controls.iter().enumerate() {
            self.layout.border_rects[control.node] = Some(BorderRect {
                x: control.x, y: control.y, width: control.width, height: control.height,
                fixed: self.layout.fixed_controls.get(index).copied().unwrap_or(false),
            });
        }
    }

    fn measure_text_inline_rects(
        &self, mut budget: Option<&mut css::CssBudget<'_>>,
    ) -> Result<HashMap<usize, BorderRect>, String> {
        let count = self.text_inline_boxes.len();
        if let Some(meter) = budget.as_deref_mut() {
            meter.work(count)?;
            meter.copies(count.saturating_mul(
                std::mem::size_of::<TextInlineBox>() + std::mem::size_of::<(usize, BorderRect)>() * 2,
            ))?;
        }
        let mut rects = HashMap::new();
        rects.try_reserve(count).map_err(|_| "Could not reserve inline client rectangles.")?;
        for &TextInlineBox { node, edges: [top, right, bottom, left] } in &self.text_inline_boxes {
            if self.layout.incomplete_grid_nodes.get(node) == Some(&true) { continue; }
            let Some(span) = self.spans[node] else { continue; };
            let atoms = &self.atoms[span.start.atoms..span.end.atoms];
            let Some(first) = atoms.first() else { continue; };
            let last = atoms.last().unwrap();
            if let Some(meter) = budget.as_deref_mut() { meter.work(atoms.len())?; }
            // Inline decoration and hit testing use these same final line
            // positions. Horizontal edges belong to the first and last line.
            let mut bounds = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
            for atom in atoms {
                bounds.0 = bounds.0.min(atom.x - if atom.line == first.line { left } else { 0 });
                bounds.1 = bounds.1.min(atom.y - top);
                bounds.2 = bounds.2.max(atom.x + atom.width + if atom.line == last.line { right } else { 0 });
                bounds.3 = bounds.3.max(atom.y + atom.height + bottom);
            }
            rects.insert(node, BorderRect {
                x: bounds.0, y: bounds.1, width: bounds.2 - bounds.0, height: bounds.3 - bounds.1,
                fixed: self.layout.fixed_hits[span.start.atoms],
            });
        }
        Ok(rects)
    }

    fn finish_client_boxes(&mut self) {
        // Read the final sizes, after flex stretching and positioned layout.
        for (node, record) in self.boxes.iter().enumerate() {
            if let Some(record) = record {
                if !record.grid_complete() { continue; }
                self.layout.client_boxes[node] = Some(ClientBox {
                    width: record.width + record.padding[3] + record.padding[1],
                    height: record.height + record.padding[0] + record.padding[2],
                    left: record.border[3],
                    top: record.border[0],
                    right: record.border[1],
                    bottom: record.border[2],
                    padding: record.padding,
                });
            }
        }
        for item in &self.layout.items {
            if let PaintItem::Image {
                node,
                width,
                height,
                ..
            } = item
            {
                if let Some(client) = &mut self.layout.client_boxes[*node] {
                    // image() stored the resolved padding before line/flex sizing.
                    client.width += *width;
                    client.height += *height;
                }
            }
        }
        for control in &self.layout.controls {
            let border = control.image.as_ref().map_or([1; 4], |image| image.border);
            self.layout.client_boxes[control.node] = Some(ClientBox {
                width: (control.width - border[1] - border[3]).max(0),
                height: (control.height - border[0] - border[2]).max(0),
                left: border[3],
                top: border[0],
                right: border[1],
                bottom: border[2],
                padding: control.padding,
            });
        }
        // Confirm only genuine no-box cases. Unmeasured inline fragments or
        // truncated/unsupported layout retain an explicit unavailable result.
        for node in 0..self.doc.nodes.len() {
            if self.layout.client_boxes[node].is_some() { continue; }
            let mut current=Some(node);
            for _ in 0..MAX_DEPTH {
                let Some(parent)=current else {break;};
                if self.styles[parent].display==Display::None || forms::kind(self.doc,parent)==Some(Kind::Hidden) {
                    self.layout.offset_unavailable[node]=false;
                    break;
                }
                current=self.parents[parent];
            }
        }
    }

    fn loaded_image_control(&self, node: usize) -> bool {
        forms::kind(self.doc, node) == Some(Kind::Image) && self.image_sizes.contains_key(&node)
    }

    fn replaced_image(&self, node: usize) -> bool {
        matches!(&self.doc.nodes[node].kind, NodeKind::Element { tag, .. } if tag == "img")
            || self.video(node)
            || self.canvas(node)
            || self.inline_svg(node)
            || self.loaded_image_control(node)
    }
    fn inline_svg(&self,node:usize)->bool {self.doc.nodes[node].namespace==crate::html::Namespace::Svg&&matches!(&self.doc.nodes[node].kind,NodeKind::Element{tag,..} if tag=="svg")}
    fn video(&self,node:usize)->bool {self.doc.nodes[node].namespace==crate::html::Namespace::Html&&matches!(&self.doc.nodes[node].kind,NodeKind::Element {tag,..} if tag=="video")}
    fn canvas(&self, node: usize) -> bool {
        self.doc.scripting_enabled && crate::html_canvas::element(self.doc, node)
    }

    fn containing_block(&self, node: usize, positioned: bool) -> ContainingBlock {
        let fixed = positioned && self.styles[node].position == Position::Fixed;
        if fixed && !self.layout.computed_properties.has_transform_geometry() { return self.viewport; }
        let mut parent = self.parents[node];
        for _ in 0..MAX_DEPTH {
            let Some(index) = parent else {
                break;
            };
            let style = &self.styles[index];
            if !positioned || self.transform_establishes_cb(index) || !fixed && style.position != Position::Static {
                if let Some(record) = &self.boxes[index] {
                    return if positioned {
                        ContainingBlock {
                            x: record.x + record.border[3],
                            y: record.y + record.border[0],
                            width: record.width + record.padding[1] + record.padding[3],
                            height: record.height + record.padding[0] + record.padding[2],
                            definite_height: true,
                        }
                    } else {
                        ContainingBlock {
                            x: record.x + record.border[3] + record.padding[3],
                            y: record.y + record.border[0] + record.padding[0],
                            width: record.width,
                            height: record.height,
                            definite_height: record.definite_height,
                        }
                    };
                }
                if positioned {
                    if let Some(span) = self.spans[index] {
                        let mut bounds: Option<(i32, i32, i32, i32)> = None;
                        for atom in &self.atoms[span.start.atoms..span.end.atoms] {
                            let (x, y, right, bottom) =
                                bounds.unwrap_or((atom.x, atom.y, atom.x, atom.y));
                            bounds = Some((
                                x.min(atom.x),
                                y.min(atom.y),
                                right.max(atom.x + atom.width),
                                bottom.max(atom.y + atom.height),
                            ));
                        }
                        if let Some((x, y, right, bottom)) = bounds {
                            let padding = self.edges(style.padding, right - x, style, false);
                            return ContainingBlock {
                                x: x - padding[3],
                                y: y - padding[0],
                                width: right - x + padding[1] + padding[3],
                                height: bottom - y + padding[0] + padding[2],
                                definite_height: true,
                            };
                        }
                    }
                }
            }
            parent = self.parents[index];
        }
        self.viewport
    }

    fn apply_relative(&mut self) {
        while self.relative_cursor < self.relatives.len() {
            let node = self.relatives[self.relative_cursor];
            self.relative_cursor += 1;
            let Some(span) = self.spans[node] else {
                continue;
            };
            let style = &self.styles[node];
            let container = self.containing_block(node, false);
            let vertical = |offset:Length| {
                if offset.has_percentage() && !container.definite_height {
                    None
                } else {
                    self.length(offset, container.height, style)
                }
            };
            // This engine has horizontal left-to-right writing: left wins an
            // over-constrained horizontal pair, and top wins vertically.
            let dx = self
                .length(style.inset[3], container.width, style)
                .or_else(|| {
                    self.length(style.inset[1], container.width, style)
                        .map(|n| -n)
                })
                .unwrap_or(0);
            let dy = vertical(style.inset[0])
                .or_else(|| vertical(style.inset[2]).map(|n| -n))
                .unwrap_or(0);
            self.move_span(span, dx, dy);
        }
    }

    fn positioned_axis(
        inset: [Option<i32>; 2],
        size: i32,
        extent: i32,
        margin: [i32; 2],
        auto_margin: [bool; 2],
        fallback: i32,
        vertical: bool,
    ) -> (i32,[i32;2]) {
        let [start, end] = inset;
        let [mut before, mut after] = margin;
        let [auto_before, auto_after] = auto_margin;
        if let (Some(start), Some(end)) = (start, end) {
            let spare = size - start - end - extent - before - after;
            match (auto_before, auto_after) {
                (true, true) => {
                    before = if vertical {
                        spare / 2
                    } else {
                        spare.max(0) / 2
                    };
                    after = spare - before;
                }
                (true, false) => before = spare,
                (false, true) => after = spare,
                _ => {}
            }
        }
        let position=start
            .map(|start| start + before)
            .or_else(|| end.map(|end| size - end - extent - after))
            .unwrap_or(fallback + before);
        (position,[before,after])
    }

    fn place_positioned(&mut self) {
        let mut cursor = 0;
        while cursor < self.pending.len() {
            let node = self.pending[cursor].node;
            let link = self.pending[cursor].link;
            let label = self.pending[cursor].label;
            let depth = self.pending[cursor].depth;
            let fallback = (self.pending[cursor].x, self.pending[cursor].y);
            cursor += 1;
            if self.finished() {
                break;
            }
            let original = self.styles[node].clone();
            let cb = self.containing_block(node, true);
            let margin = self.edges(original.margin, cb.width, &original, true);
            let control = forms::kind(self.doc, node).is_some();
            let image = self.replaced_image(node);
            let replaced = control || image;
            let padding = if control && !image && !native_button_padding(forms::kind(self.doc,node)) {
                [0; 4]
            } else {
                self.edges(original.padding, cb.width, &original, false)
            };
            let border = if control && !image {
                [0; 4]
            } else {
                self.border(&original, cb.width)
            };
            let edges = [
                padding[1] + padding[3] + border[1] + border[3],
                padding[0] + padding[2] + border[0] + border[2],
            ];
            let inset: [Option<i32>; 4] = std::array::from_fn(|side| {
                self.length(
                    original.inset[side],
                    if side % 2 == 0 { cb.height } else { cb.width },
                    &original,
                )
            });
            let hint = |name| {
                if image && !self.canvas(node) {
                    self.doc
                        .attr(node, name)
                        .and_then(|value| value.trim().parse::<f32>().ok())
                        .filter(|value| value.is_finite() && *value >= 0.0)
                        .map(|value| {
                            let value =
                                (value * self.zoom).round().clamp(0.0, MAX_WIDTH as f32) as i32;
                            if control && original.border_box {
                                (value - edges[usize::from(name == "height")]).max(0)
                            } else {
                                value
                            }
                        })
                } else {
                    None
                }
            };
            let canvas_size = self.canvas(node).then(|| self.image_dimensions(node, &original, cb.width));
            let mut specified_width = self
                .dimension(original.width, Some(cb.width), &original, edges[0])
                .or_else(|| hint("width"))
                .or_else(|| canvas_size.map(|size| size.0));
            let specified_height = self
                .dimension(original.height, Some(cb.height), &original, edges[1])
                .or_else(|| hint("height"))
                .or_else(|| canvas_size.map(|size| size.1));
            if image && specified_width.is_none() {
                if let (Some(height), Some(&(width, natural_height))) =
                    (specified_height, self.image_sizes.get(&node))
                {
                    if natural_height > 0 {
                        specified_width = Some(
                            (i64::from(height) * width as i64 / natural_height as i64)
                                .clamp(0, i64::from(MAX_WIDTH)) as i32,
                        );
                    }
                }
            }
            let available = (cb.width
                - inset[1].unwrap_or(0)
                - inset[3].unwrap_or(0)
                - margin[1]
                - margin[3]
                - edges[0])
                .max(0);
            let intrinsic = self.intrinsic(node, depth);
            let width = specified_width.unwrap_or_else(|| {
                if !replaced && inset[1].is_some() && inset[3].is_some() {
                    available
                } else {
                    available
                        .max((intrinsic.minimum - edges[0]).max(0))
                        .min((intrinsic.preferred - edges[0]).max(0))
                }
            });
            let width = self
                .constrained(
                    width,
                    original.min_width,
                    original.max_width,
                    Some(cb.width),
                    &original,
                    edges[0],
                )
                .min(MAX_WIDTH);
            let height = specified_height
                .or_else(|| {
                    if replaced {
                        None
                    } else {
                        inset[0].zip(inset[2]).map(|(top, bottom)| {
                            (cb.height - top - bottom - margin[0] - margin[2] - edges[1]).max(0)
                        })
                    }
                })
                .map(|height| {
                    self.constrained(
                        height,
                        original.min_height,
                        original.max_height,
                        Some(cb.height),
                        &original,
                        edges[1],
                    )
                });
            let mut used = original.clone();
            if matches!(used.display, Display::Inline | Display::InlineBlock) {
                used.display = Display::Block;
            }
            used.margin = [Length::Px(0.0); 4];
            used.width = Length::Px(width as f32 / self.zoom);
            used.min_width = Length::Px(0.0);
            used.max_width = None;
            used.height = height
                .map(|value| Length::Px(value as f32 / self.zoom))
                .unwrap_or(Length::Auto);
            used.min_height = self
                .dimension(original.min_height, Some(cb.height), &original, edges[1])
                .map(|value| Length::Px(value as f32 / self.zoom))
                .unwrap_or(Length::Auto);
            used.max_height = original
                .max_height
                .and_then(|value| self.dimension(value, Some(cb.height), &original, edges[1]))
                .map(|value| Length::Px(value as f32 / self.zoom));
            used.padding = padding.map(|value| Length::Px(value as f32 / self.zoom));
            used.border_width = border.map(|value| Length::Px(value as f32 / self.zoom));
            used.border_box = false;
            let flow = std::mem::replace(
                &mut self.flow,
                Flow::new(
                    0,
                    (width + edges[0]).max(1),
                    0,
                    used.align,
                    used.white_space,
                ),
            );
            let previous_positioning = self.positioning.replace(node);
            let previous_label = self.active_label;
            let separator = self.pending_separator;
            self.active_label = label;
            self.styles[node] = used;
            self.separate("\n");
            self.walk(node, link, depth);
            self.finish_line(false, 0);
            self.cache_decorations();
            let outer_height = self.flow.y.max(0);
            let (offset_x,horizontal_margin)=Self::positioned_axis(
                    [inset[3], inset[1]],
                    cb.width,
                    width + edges[0],
                    [margin[3], margin[1]],
                    [
                        original.margin[3] == Length::Auto,
                        original.margin[1] == Length::Auto,
                    ],
                    fallback.0 - cb.x,
                    false,
                );
            let (offset_y,vertical_margin)=Self::positioned_axis(
                    [inset[0], inset[2]],
                    cb.height,
                    outer_height,
                    [margin[0], margin[2]],
                    [
                        original.margin[0] == Length::Auto,
                        original.margin[2] == Length::Auto,
                    ],
                    fallback.1 - cb.y,
                    true,
                );
            let x=cb.x.saturating_add(offset_x);let y=cb.y.saturating_add(offset_y);
            self.used_margins.insert(node,[vertical_margin[0],horizontal_margin[1],vertical_margin[1],horizontal_margin[0]]);
            if let Some(span) = self.spans[node] {
                self.move_span(span, x, y);
            }
            self.pending[cursor - 1].placed_bounds =
                Some(ClipRect::new(x, y, width.saturating_add(edges[0]), outer_height));
            self.styles[node] = original;
            self.positioning = previous_positioning;
            self.active_label = previous_label;
            self.pending_separator = separator;
            self.flow = flow;
            self.apply_relative();
        }
    }

    fn mark_fixed(&mut self) -> Vec<bool> {
        self.layout.fixed_runs = vec![false; self.layout.runs.len()];
        self.layout.fixed_items = vec![false; self.layout.items.len()];
        self.layout.fixed_controls = vec![false; self.layout.controls.len()];
        self.layout.fixed_anchors = vec![false; self.layout.anchors.len()];
        self.layout.fixed_hits = vec![false; self.atoms.len()];
        self.layout.positioned_items = vec![0; self.layout.items.len()];
        for (node, span) in self.spans.iter().enumerate() {
            if (self.styles[node].position == Position::Static && !self.transform_establishes_cb(node))
                || !matches!(self.doc.nodes[node].kind, NodeKind::Element { .. })
            {
                continue;
            }
            if let Some(span) = span {
                self.layout.positioned_items[span.start.items..span.end.items].fill(node + 1);
            }
        }
        let mut boxes = vec![false; self.boxes.len()];
        for pending in &self.pending {
            if !pending.fixed {
                continue;
            }
            let Some(span) = self.spans[pending.node] else {
                continue;
            };
            self.layout.fixed_runs[span.start.runs..span.end.runs].fill(true);
            self.layout.fixed_items[span.start.items..span.end.items].fill(true);
            self.layout.fixed_controls[span.start.controls..span.end.controls].fill(true);
            self.layout.fixed_anchors[span.start.anchors..span.end.anchors].fill(true);
            self.layout.fixed_hits[span.start.atoms..span.end.atoms].fill(true);
            for &node in &self.placed_boxes[span.start.boxes..span.end.boxes] {
                boxes[node] = true;
            }
        }
        boxes
    }

    fn clip_parent(&self, node: usize) -> Option<usize> {
        match self.styles[node].position {
            Position::Fixed => self.transformed_parent(node),
            Position::Absolute => {
                // An absolute box escapes overflow on ancestors between it
                // and its containing block (CSS 2.2 section 11.1).
                let mut parent = self.parents[node];
                for _ in 0..MAX_DEPTH {
                    let index = parent?;
                    if self.styles[index].position != Position::Static || self.transform_establishes_cb(index) {
                        return Some(index);
                    }
                    parent = self.parents[index];
                }
                None
            }
            _ => self.parents[node],
        }
    }

    fn ancestor_clip(&self, node: usize) -> Option<ClipRect> {
        let mut node = node;
        let mut clip: Option<ClipRect> = None;
        for _ in 0..MAX_DEPTH {
            let Some(parent) = self.clip_parent(node) else {
                break;
            };
            if let Some(b) = &self.boxes[parent] {
                let padding = ClipRect::new(
                    b.x.saturating_add(b.border[3]),
                    b.y.saturating_add(b.border[0]),
                    b.width.saturating_add(b.padding[1]).saturating_add(b.padding[3]),
                    b.height.saturating_add(b.padding[0]).saturating_add(b.padding[2]),
                );
                if let Some(bounds) = overflow_clip(padding, self.styles[parent].overflow) {
                    clip = Some(clip.map_or(bounds, |old| old.intersect(bounds)));
                }
            }
            node = parent;
        }
        clip
    }

    fn clip_overflow(&mut self) -> Vec<Option<ClipRect>> {
        if let Some(clips) = self.translated_clips() { return clips; }
        self.layout.item_clips = vec![None; self.layout.items.len()];
        let mut atoms = vec![None; self.atoms.len()];
        // Parent spans include normal-flow descendants. Later child spans
        // replace them with their more specific clip; deferred positioned
        // spans receive their own containing-block-aware clip.
        for (node, span) in self.spans.iter().enumerate() {
            if let Some(span) = span {
                let clip = self.ancestor_clip(node);
                self.layout.item_clips[span.start.items..span.end.items].fill(clip);
                atoms[span.start.atoms..span.end.atoms].fill(clip);
            }
        }
        self.layout.run_clips = vec![None; self.layout.runs.len()];
        for (index, item) in self.layout.items.iter().enumerate() {
            if let PaintItem::Text(run) = item {
                self.layout.run_clips[*run] = self.layout.item_clips[index];
            }
        }
        atoms
    }

    fn restore_source_order(&mut self) {
        if self.pending.is_empty() {
            return;
        }
        // Deferred layout changes paint order but must not change reading,
        // find, clipboard, keyboard or duplicate-fragment lookup order.
        let mut runs: Vec<_> = std::mem::take(&mut self.layout.runs)
            .into_iter()
            .enumerate()
            .collect();
        runs.sort_by_key(|(index, _)| (self.run_nodes[*index], *index));
        let mut run_map = vec![0; runs.len()];
        let old_fixed = std::mem::take(&mut self.layout.fixed_runs);
        let old_clips = std::mem::take(&mut self.layout.run_clips);
        for (new, (old, run)) in runs.into_iter().enumerate() {
            run_map[old] = new;
            self.layout.fixed_runs.push(old_fixed[old]);
            self.layout.run_clips.push(old_clips[old]);
            self.layout.runs.push(run);
        }
        if let Some(first) = self.layout.runs.first_mut() {
            first.separator_before = "";
        }
        let mut controls: Vec<_> = std::mem::take(&mut self.layout.controls)
            .into_iter()
            .enumerate()
            .collect();
        controls.sort_by_key(|(index, control)| (control.node, *index));
        let mut control_map = vec![0; controls.len()];
        let old_fixed = std::mem::take(&mut self.layout.fixed_controls);
        for (new, (old, control)) in controls.into_iter().enumerate() {
            control_map[old] = new;
            self.layout.fixed_controls.push(old_fixed[old]);
            self.layout.controls.push(control);
        }
        let mut anchors: Vec<_> = std::mem::take(&mut self.layout.anchors)
            .into_iter()
            .enumerate()
            .collect();
        anchors.sort_by_key(|(index, _)| (self.anchor_nodes[*index], *index));
        let old_fixed = std::mem::take(&mut self.layout.fixed_anchors);
        for (old, anchor) in anchors {
            self.layout.fixed_anchors.push(old_fixed[old]);
            self.layout.anchors.push(anchor);
        }
        for item in &mut self.layout.items {
            match item {
                PaintItem::Text(index) => *index = run_map[*index],
                PaintItem::Control(index) => *index = control_map[*index],
                _ => {}
            }
        }
    }

    fn order_positioned_paint(&mut self) {
        if self.pending.is_empty() && self.relatives.is_empty() && self.translated_ranks.is_empty() {
            return;
        }
        // Supported positioned boxes use the default auto/zero layer. Normal
        // flow paints first, then positioned groups in DOM order; child groups
        // keep their own later position. Explicit z-index/stacking contexts
        // remain outside this subset.
        let items = std::mem::take(&mut self.layout.items);
        let fixed = std::mem::take(&mut self.layout.fixed_items);
        let layers = std::mem::take(&mut self.layout.positioned_items);
        let clips = std::mem::take(&mut self.layout.item_clips);
        let opacity = std::mem::take(&mut self.opacity_items);
        if opacity.is_empty() {
            let mut ordered: Vec<_> = items.into_iter().zip(fixed).zip(layers).zip(clips).collect();
            ordered.sort_by_key(|((_, layer), _)| *layer);
            for (((item, fixed), layer), clip) in ordered {
                self.layout.items.push(item);
                self.layout.fixed_items.push(fixed);
                self.layout.positioned_items.push(layer);
                self.layout.item_clips.push(clip);
            }
            return;
        }
        let mut ordered: Vec<_> = items
            .into_iter()
            .zip(fixed)
            .zip(layers)
            .zip(clips)
            .enumerate()
            .map(|(index, item)| (item, opacity[index]))
            .collect();
        ordered.sort_by_key(|(((_, layer), _), _)| *layer);
        for ((((item, fixed), layer), clip), opacity) in ordered {
            self.layout.items.push(item);
            self.layout.fixed_items.push(fixed);
            self.layout.positioned_items.push(layer);
            self.layout.item_clips.push(clip);
            self.opacity_items.push(opacity);
        }
    }

    fn build_hits(&mut self) {
        let mut text_atoms = vec![None; self.layout.runs.len()];
        let mut control_atoms = vec![None; self.layout.controls.len()];
        let mut image_atoms = HashMap::new();
        // Image nodes are rendered once; decoration insertion can change paint
        // indices, so use their stable node IDs for the final ordering pass.
        for (index, atom) in self.atoms.iter().enumerate() {
            match atom.kind {
                AtomKind::Text(run) => text_atoms[run] = Some(index),
                AtomKind::Control(control) => control_atoms[control] = Some(index),
                AtomKind::Image { node, .. } => {
                    image_atoms.insert(node, index);
                }
                AtomKind::InlineBlock(_) => {}
            }
        }
        let atom_fixed = std::mem::take(&mut self.layout.fixed_hits);
        self.layout.hits.clear();
        for (index, item) in self.layout.items.iter().enumerate() {
            let (mut hit, fixed) = match item {
                PaintItem::Rect {
                    x,
                    y,
                    width,
                    height,
                    ..
                }
                | PaintItem::RoundedRect { x, y, width, height, .. }
                | PaintItem::Background { x, y, width, height, .. } => (
                    HitRegion {
                        x: *x,
                        y: *y,
                        width: *width,
                        height: *height,
                        link: None,
                        control: None,
                    },
                    self.layout.fixed_items[index],
                ),
                PaintItem::Text(run) | PaintItem::Control(run) => {
                    let atom = if matches!(item, PaintItem::Text(_)) {
                        text_atoms[*run]
                    } else {
                        control_atoms[*run]
                    };
                    let Some(atom) = atom else {
                        continue;
                    };
                    let a = &self.atoms[atom];
                    (
                        HitRegion {
                            x: a.x,
                            y: a.y,
                            width: a.width,
                            height: a.height,
                            link: a.link,
                            control: a.control,
                        },
                        atom_fixed[atom],
                    )
                }
                PaintItem::Image { node, .. } => {
                    let Some(atom) = image_atoms.get(node) else {
                        continue;
                    };
                    let a = &self.atoms[*atom];
                    (
                        HitRegion {
                            x: a.x,
                            y: a.y,
                            width: a.width,
                            height: a.height,
                            link: a.link,
                            control: a.control,
                        },
                        atom_fixed[*atom],
                    )
                }
                PaintItem::HitBox {
                    x,
                    y,
                    width,
                    height,
                    link,
                    control,
                } => (
                    HitRegion {
                        x: *x,
                        y: *y,
                        width: *width,
                        height: *height,
                        link: *link,
                        control: *control,
                    },
                    self.layout.fixed_items[index],
                ),
            };
            if let Some(clip) = self.layout.item_clips[index] {
                let area = ClipRect::new(hit.x, hit.y, hit.width, hit.height).intersect(clip);
                hit.x = area.left;
                hit.y = area.top;
                hit.width = area.right - area.left;
                hit.height = area.bottom - area.top;
            }
            self.layout.hits.push(hit);
            self.layout.fixed_hits.push(fixed);
        }
    }

    fn separate(&mut self, separator: &'static str) {
        if separator == "\n" || self.pending_separator != "\n" {
            self.pending_separator = separator;
        }
    }

    fn scale(&self, style: &Style) -> i32 {
        (style.font_size * self.zoom / 8.0).round().clamp(1.0, 16.0) as i32
    }

    fn font_spec(&self, style: &Style) -> Option<FontSpec> {
        FontSpec::new(
            self.fonts,
            &style.font_families,
            style.font_weight,
            style.italic,
            style.font_size * self.zoom,
        )
    }
    fn text_width(&self, text: &str, style: &Style) -> i32 {
        self.font_spec(style).map_or_else(
            || {
                (text.chars().count().min(MAX_HEIGHT as usize) as i32)
                    .saturating_mul(GLYPH_ADVANCE * self.scale(style))
            },
            |font| font.measure(text),
        )
    }
    fn space_width(&self, style: &Style) -> i32 {
        self.text_width(" ", style).max(1)
    }

    fn length(&self, length: Length, reference: i32, style: &Style) -> Option<i32> {
        length
            .resolve(
                reference as f32 / self.zoom,
                style.font_size,
                style.root_font_size,
            )
            .map(|value| (value * self.zoom).round().clamp(-100_000.0, 100_000.0) as i32)
    }

    fn dimension(
        &self,
        length: Length,
        reference: Option<i32>,
        style: &Style,
        edges: i32,
    ) -> Option<i32> {
        if length.has_percentage() && reference.is_none() {
            return None;
        }
        self.length(length, reference.unwrap_or(0), style)
            .map(|n| (n - if style.border_box { edges } else { 0 }).max(0))
    }

    fn constrained(
        &self,
        value: i32,
        minimum: Length,
        maximum: Option<Length>,
        reference: Option<i32>,
        style: &Style,
        edges: i32,
    ) -> i32 {
        let low = self
            .dimension(minimum, reference, style, edges)
            .unwrap_or(0)
            .clamp(0, MAX_HEIGHT);
        let high = maximum
            .and_then(|n| self.dimension(n, reference, style, edges))
            .unwrap_or(MAX_HEIGHT)
            .clamp(low, MAX_HEIGHT);
        value.clamp(low, high)
    }

    fn edges(
        &self,
        values: [Length; 4],
        reference: i32,
        style: &Style,
        negative: bool,
    ) -> [i32; 4] {
        values.map(|value| {
            self.length(value, reference, style)
                .unwrap_or(0)
                .clamp(if negative { -MAX_WIDTH } else { 0 }, MAX_WIDTH)
        })
    }

    fn border(&self, style: &Style, reference: i32) -> [i32; 4] {
        let widths = self.edges(style.border_width, reference, style, false);
        std::array::from_fn(|index| {
            if style.border_solid[index] {
                widths[index].min(512)
            } else {
                0
            }
        })
    }

    fn line_height(&self, style: &Style) -> i32 {
        let height = match style.line_height {
            LineHeight::Factor(factor) => style.font_size * factor,
            LineHeight::Length(length) => length
                .resolve(style.font_size, style.font_size, style.root_font_size)
                .unwrap_or(style.font_size * 1.5),
        };
        ((height * self.zoom).round() as i32).clamp(1, 20_000).max(
            self.font_spec(style)
                .map_or((GLYPH_HEIGHT + 3) * self.scale(style), |font| {
                    font.line_height()
                }),
        )
    }

    fn finished(&self) -> bool {
        self.layout.runs.len() >= MAX_RUNS
            || self.layout.items.len() >= MAX_ITEMS
            || self.flow.y >= MAX_HEIGHT
            || self.visits >= 40_000
    }

    fn walk(&mut self, index: usize, link: Option<usize>, depth: usize) {
        if depth > MAX_DEPTH || self.finished() || index >= self.styles.len() {
            return;
        }
        if self.doc.scripting_enabled
            && matches!(&self.doc.nodes[index].kind, NodeKind::Element { tag, .. } if tag == "noscript")
        {
            return;
        }
        if matches!(self.doc.nodes[index].kind, NodeKind::Element { .. }) {
            let style = &self.styles[index];
            if style.display == Display::None || forms::kind(self.doc, index) == Some(Kind::Hidden)
            {
                return;
            }
            if matches!(style.position, Position::Absolute | Position::Fixed)
                && self.positioning != Some(index)
            {
                if self.pending.len() < MAX_POSITIONED {
                    let fixed = self.viewport_fixed(index);
                    self.pending.push(Pending {
                        node: index,
                        link,
                        label: self.active_label,
                        depth,
                        x: self.flow.x,
                        y: self.flow.y,
                        fixed,
                        placed_bounds: None,
                    });
                }
                self.visits += 1;
                return;
            }
        }
        let start = self.mark();
        let previous = self.current_node;
        self.current_node = index;
        self.walk_node(index, link, depth);
        self.current_node = previous;
        self.spans[index] = Some(Span {
            start,
            end: self.mark(),
        });
        if self.styles[index].position == Position::Relative
            && matches!(self.doc.nodes[index].kind, NodeKind::Element { .. })
        {
            self.relatives.push(index);
        }
    }

    fn anchor(&mut self, node: usize, id: &str, y: i32) {
        self.layout.anchors.push((id.to_owned(), y));
        self.anchor_nodes.push(node);
    }

    fn walk_node(&mut self, index: usize, mut link: Option<usize>, depth: usize) {
        if depth > MAX_DEPTH || self.finished() {
            return;
        }
        self.visits += 1;
        let Some(node) = self.doc.nodes.get(index) else {
            return;
        };
        let Some(style) = self.styles.get(index).cloned() else {
            return;
        };
        match &node.kind {
            NodeKind::Comment(_) | NodeKind::ProcessingInstruction { .. } | NodeKind::DocumentFragment => {},
            NodeKind::Text(text) => self.text(text, &style, link),
            NodeKind::Element { tag, id, .. } => {
                if style.display == Display::None
                    || matches!(
                        tag.as_str(),
                        "head"
                            | "title"
                            | "script"
                            | "style"
                            | "link"
                            | "meta"
                            | "base"
                            | "template"
                            | "iframe"
                            | "object"
                            | "datalist"
                    )
                {
                    return;
                }
                if tag == "a" {
                    if let Some(href) = self.doc.attr(index, "href") {
                        link = Some(self.layout.targets.len());
                        self.layout.targets.push(href.to_owned());
                        self.layout.target_nodes.push(index);
                    }
                }
                if tag == "label" {
                    let previous = self.active_label;
                    self.active_label = self.labels.get(&index).copied();
                    if style.display.block_level() {
                        self.block(index, &style, link, depth);
                    } else if style.display == Display::InlineBlock {
                        self.inline_block(index, &style, link, depth);
                    } else {
                        self.inline(index, &style, link, depth);
                    }
                    self.active_label = previous;
                    return;
                }
                if tag == "br" {
                    self.inline_boxes[index] = true;
                    if let Some(id) = id {
                        self.anchor(index, id, self.flow.y);
                    }
                    self.finish_line(true, self.line_height(&style));
                    self.separate("\n");
                    return;
                }
                if tag == "img" || self.video(index) || self.canvas(index) || self.inline_svg(index) {
                    if style.display.block_level() {
                        self.finish_line(false, 0);
                        self.separate("\n");
                    }
                    if let Some(id) = id {
                        self.anchor(index, id, self.flow.y);
                    }
                    self.image(index, &style, link, depth);
                    if style.display.block_level() {
                        self.finish_line(false, 0);
                        self.separate("\n");
                    }
                    return;
                }
                if let Some(kind) = forms::kind(self.doc, index) {
                    if kind == Kind::Hidden {
                        return;
                    }
                    if style.display.block_level() {
                        self.finish_line(false, 0);
                        self.separate("\n");
                    }
                    if let Some(id) = id {
                        self.anchor(index, id, self.flow.y);
                    }
                    if self.loaded_image_control(index) {
                        self.image_control(index, &style);
                    } else {
                        self.control(index, &style, kind);
                    }
                    if style.display.block_level() {
                        self.finish_line(false, 0);
                        self.separate("\n");
                    }
                    return;
                }
                if style.display.block_level() {
                    self.block(index, &style, link, depth);
                } else if style.display == Display::InlineBlock {
                    self.inline_block(index, &style, link, depth);
                } else {
                    if let Some(id) = id {
                        self.anchor(index, id, self.flow.y);
                    }
                    self.inline(index, &style, link, depth);
                }
            }
        }
    }

    fn block(&mut self, index: usize, style: &Style, link: Option<usize>, depth: usize) {
        self.finish_line(false, 0);
        self.separate("\n");
        let available = (self.flow.right - self.flow.left).max(1);
        let mut margin = self.edges(style.margin, available, style, true);
        let padding = self.edges(style.padding, available, style, false);
        let border = self.border(style, available);
        let horizontal = padding[1] + padding[3] + border[1] + border[3];
        let natural_width = if style.display == Display::Table {
            let intrinsic = self.intrinsic(index, depth);
            available
                .max((intrinsic.minimum - horizontal).max(0))
                .min((intrinsic.preferred - horizontal).max(0))
        } else {
            available.saturating_sub(margin[1] + margin[3] + horizontal)
        };
        let width = self
            .dimension(style.width, Some(available), style, horizontal)
            .unwrap_or(natural_width)
            .clamp(0, MAX_WIDTH);
        let width = self
            .constrained(
                width,
                style.min_width,
                style.max_width,
                Some(available),
                style,
                horizontal,
            )
            .min(MAX_WIDTH);
        let spare = (available - width - horizontal - margin[1] - margin[3]).max(0);
        match (
            style.margin[3] == Length::Auto,
            style.margin[1] == Length::Auto,
        ) {
            (true, true) => {
                margin[3] = spare / 2;
                margin[1] = spare - margin[3];
            }
            (true, false) => margin[3] = spare,
            (false, true) => margin[1] = spare,
            _ if self.doc.attr(index, "align").is_none() => {
                // HTML's legacy center/align behavior also aligns fixed-width
                // descendants. CSS text-align alone never centers block boxes.
                match style.legacy_align {
                    Some(Align::Center) => margin[3] += spare / 2,
                    Some(Align::Right) => margin[3] += spare,
                    _ => {}
                }
            }
            _ => {}
        }
        let x = (self.flow.left + margin[3]).clamp(-MAX_HEIGHT, MAX_HEIGHT);
        let y = self
            .flow
            .y
            .saturating_add(margin[0])
            .clamp(-MAX_HEIGHT, MAX_HEIGHT);
        let outer_width = (width + horizontal).max(0);
        let top = y + border[0] + padding[0];
        let left = x + border[3] + padding[3];
        let colors = style.border_color.map(|color| color.unwrap_or(style.color));
        let background = style
            .background
            .and_then(|color| self.rect(x, y, outer_width, 0, color));
        // Keep the outer item bounds for geometry/hits. Color clipping changes
        // only coverage, using the actual used border and padding dimensions.
        let uniform = (border[0] > 0 && border == [border[0]; 4]
            && style.border_solid == [true; 4] && colors == [colors[0]; 4]
            && style.border_alpha == [style.border_alpha[0]; 4])
            .then_some(crate::paint::UniformBorder {
                width: border[0], color: colors[0], alpha: style.border_alpha[0],
            });
        let mut rounded_border = false;
        if style.background_image.is_none()
            && matches!(style.display, Display::Block | Display::Flex | Display::Grid | Display::InlineBlock)
        {
            let circular = if border == [0; 4] || uniform.is_some() { style.circular_radius() } else { None };
            let radius = circular.unwrap_or(0.0) * self.zoom;
            let insets = match style.background_clip {
                Some(css::BackgroundClip::PaddingBox) => border,
                Some(css::BackgroundClip::ContentBox) => std::array::from_fn(|side| border[side].saturating_add(padding[side])),
                _ => [0; 4],
            };
            // Unequal rounded insets produce elliptical inner corners, outside
            // this circular-color slice. Preserve their existing decoration.
            let insets = if radius > 0.0 && insets != [insets[0]; 4] { [0; 4] } else { insets };
            if let Some(item) = background.filter(|_| circular.is_some() || insets != [0; 4]) {
                if let PaintItem::Rect { color, alpha, .. } = self.layout.items[item] {
                    self.layout.items[item] = PaintItem::RoundedRect {
                        x, y, width: outer_width, height: 0, color, alpha, radius,
                        border: if circular.is_some() { uniform } else { None },
                        background_insets: insets,
                    };
                    rounded_border = circular.is_some() && uniform.is_some();
                }
            }
        }
        let background_image = self.background_image(index, style, x, y, outer_width, 0, border);
        let borders: [Option<usize>; 4] = std::array::from_fn(|side| {
            if border[side] > 0 && !rounded_border {
                let item=self.rect(x, y, 0, 0, colors[side]);
                if let Some(item)=item {if let PaintItem::Rect {alpha,..}=&mut self.layout.items[item] {*alpha=style.border_alpha[side];}}
                item
            } else {
                None
            }
        });
        let hit_box = if (link.is_some() || self.active_label.is_some())
            && self.layout.items.len() < MAX_ITEMS
        {
            let item = self.layout.items.len();
            self.layout.items.push(PaintItem::HitBox {
                x,
                y,
                width: outer_width,
                height: 0,
                link,
                control: self.active_label.filter(|_| link.is_none()),
            });
            Some(item)
        } else {
            None
        };
        let tag = match &self.doc.nodes[index].kind {
            NodeKind::Element { tag, id, .. } => {
                if let Some(id) = id {
                    self.anchor(index, id, y);
                }
                tag.clone()
            }
            _ => String::new(),
        };
        let previous = std::mem::replace(
            &mut self.flow,
            Flow::new(
                left,
                left + width.max(1),
                top,
                style.align,
                style.white_space,
            ),
        );
        let list = matches!(tag.as_str(), "ul" | "ol");
        if list {
            let start = self
                .doc
                .attr(index, "start")
                .and_then(|value| value.parse::<i32>().ok())
                .unwrap_or(1)
                .clamp(-1_000_000, 1_000_000);
            self.lists.push(start);
        }
        let marker = if tag == "li" {
            let count = if let Some(counter) = self.lists.last_mut() {
                if let Some(value) = self
                    .doc
                    .attr(index, "value")
                    .and_then(|value| value.parse::<i32>().ok())
                {
                    *counter = value.clamp(-1_000_000, 1_000_000);
                }
                let value = *counter;
                *counter = counter.saturating_add(1);
                value
            } else {
                1
            };
            if style.list_style == ListStyle::None {
                None
            } else {
                let text = if style.list_style == ListStyle::Decimal {
                    format!("{count}.")
                } else {
                    "-".to_owned()
                };
                let marker_width = self.text_width(&text, style);
                self.add_run(
                    text,
                    left - marker_width - (8.0 * self.zoom) as i32,
                    top,
                    style,
                    None,
                )
            }
        } else {
            None
        };
        if marker.is_some() {
            self.separate(" ");
        }
        let atom_start = self.atoms.len();
        let vertical = border[0] + padding[0] + padding[2] + border[2];
        let specified_height = self.dimension(style.height, None, style, vertical);
        let formatting = if style.display == Display::Flex {
            Some(FormattingContext::Flex(self.flex(index, style, link, depth, width, specified_height)))
        } else if style.display == Display::Grid {
            let group = self.grid(index, style, link, depth, width, specified_height);
            self.grid_budget.incomplete |= !group.complete;
            Some(FormattingContext::Grid(group))
        } else if style.display == Display::Table {
            self.table(index, style, link, depth, width);
            None
        } else {
            for &child in &self.doc.nodes[index].children {
                self.walk(child, link, depth + 1);
                if self.finished() {
                    break;
                }
            }
            None
        };
        self.finish_line(false, 0);
        if let Some(run) = marker {
            if let Some(first) = self.atoms.get(atom_start) {
                self.layout.runs[run].y = first.y;
            }
        }
        if list {
            self.lists.pop();
        }
        let natural_height = (self.flow.y - top).max(0);
        let content_height = self.constrained(
            specified_height.unwrap_or(natural_height),
            style.min_height,
            style.max_height,
            None,
            style,
            vertical,
        );
        let outer_height =
            (border[0] + padding[0] + content_height + padding[2] + border[2]).clamp(0, MAX_HEIGHT);
        if let Some(item) = background {
            self.set_rect(item, x, y, outer_width, outer_height);
        }
        if let Some(item) = background_image {
            self.set_background_rect(item, x, y, outer_width, outer_height);
        }
        if let Some(item) = hit_box {
            if let PaintItem::HitBox { height, .. } = &mut self.layout.items[item] {
                *height = outer_height;
            }
        }
        let boxes = crate::paint::border_segments(x,y,outer_width,outer_height,border,style.border_alpha);
        for side in 0..4 {
            if let Some(item) = borders[side] {
                let (x, y, w, h) = boxes[side];
                self.set_rect(item, x, y, w, h);
            }
        }
        self.boxes[index] = Some(BoxRecord {
            x,
            y,
            width,
            height: content_height,
            natural_height,
            definite_height: specified_height.is_some(),
            padding,
            border,
            background,
            background_image,
            borders,
            hit_box,
            formatting,
        });
        self.used_margins.insert(index,margin);
        self.placed_boxes.push(index);
        // A min/max-height constraint can establish a different final flex size.
        if matches!(style.display, Display::Flex | Display::Grid) && content_height != natural_height {
            self.resize_box(index, content_height, depth);
        }
        self.flow = previous;
        self.flow.y = y
            .saturating_add(outer_height)
            .saturating_add(margin[2])
            .clamp(-MAX_HEIGHT, MAX_HEIGHT);
        self.flow.x = self.flow.left;
        self.flow.space_pending = false;
        self.separate("\n");
    }

    fn table_rows(&self, node: usize) -> Vec<TableRow> {
        let mut pending: Vec<_> = self.doc.nodes[node]
            .children
            .iter()
            .rev()
            .copied()
            .collect();
        let mut rows = Vec::new();
        let (mut visits, mut cells) = (0usize, 0usize);
        while let Some(node) = pending.pop() {
            visits += 1;
            if visits > 8_192 || rows.len() == 2_048 || cells == 2_048 {
                break;
            }
            match self.styles[node].display {
                Display::TableRow => {
                    let row: Vec<_> = self.doc.nodes[node]
                        .children
                        .iter()
                        .copied()
                        .filter(|&cell| self.styles[cell].display == Display::TableCell)
                        .take(128.min(2_048 - cells))
                        .collect();
                    cells += row.len();
                    rows.push(TableRow { node, cells: row });
                }
                Display::TableRowGroup => {
                    pending.extend(self.doc.nodes[node].children.iter().rev().copied())
                }
                _ => {}
            }
        }
        rows
    }

    fn table_columns(&mut self, rows: &[TableRow], depth: usize) -> Vec<(i32, i32, f32)> {
        let mut columns =
            vec![(0, 0, 0.0f32); rows.iter().map(|row| row.cells.len()).max().unwrap_or(0)];
        for row in rows {
            for (column, &node) in row.cells.iter().enumerate() {
                let intrinsic = self.intrinsic(node, depth + 1);
                columns[column].0 = columns[column].0.max(intrinsic.minimum);
                columns[column].1 = columns[column].1.max(intrinsic.preferred);
                if let Length::Percent(percent) = self.styles[node].width {
                    columns[column].2 = columns[column].2.max((percent / 100.0).clamp(0.0, 1.0));
                }
            }
        }
        columns
    }

    /// A separate-border table subset: shared non-spanning columns, intrinsic
    /// widths, percentage hints and one paint pass per cell. Span resolution,
    /// collapsed borders and anonymous table-object repair remain unsupported.
    fn table(
        &mut self,
        index: usize,
        style: &Style,
        link: Option<usize>,
        depth: usize,
        width: i32,
    ) {
        let rows = self.table_rows(index);
        if rows.is_empty() {
            for &child in &self.doc.nodes[index].children {
                self.walk(child, link, depth + 1);
            }
            return;
        }
        let columns = self.table_columns(&rows, depth);
        let gaps = style
            .border_spacing
            .map(|v| self.length(v, 0, style).unwrap_or(0).max(0));
        let edges = gaps[0] * (columns.len() as i32 + 1);
        let room = (width - edges).max(0);
        let minimum = columns.iter().map(|col| col.0).sum::<i32>();
        let mut widths: Vec<_> = columns.iter().map(|col| col.0).collect();
        let mut spare = (room - minimum).max(0);
        let desired: Vec<_> = columns
            .iter()
            .map(|&(minimum, preferred, percent)| {
                preferred
                    .max((room as f32 * percent).round() as i32)
                    .saturating_sub(minimum)
                    .max(0)
            })
            .collect();
        let total = desired.iter().map(|&n| i64::from(n)).sum::<i64>();
        let mut assigned = 0i32;
        if total > 0 {
            let allot = spare.min(total.min(i64::from(MAX_HEIGHT)) as i32);
            let mut cumulative = 0i64;
            for (&want, used) in desired.iter().zip(&mut widths) {
                cumulative += i64::from(want);
                let next = (i64::from(allot) * cumulative / total) as i32;
                if want > 0 {
                    *used += next - assigned;
                }
                assigned = next;
            }
            spare -= assigned;
        }
        if spare > 0 && !widths.is_empty() {
            let count = widths.len() as i32;
            for (column, used) in widths.iter_mut().enumerate() {
                *used += spare / count + i32::from((column as i32) < spare % count);
            }
        }
        let left = self.flow.left;
        self.flow.y = self.flow.y.saturating_add(gaps[1]).min(MAX_HEIGHT);
        let row_width = (width - 2 * gaps[0]).max(0);
        let mut group_extents: HashMap<usize, (i32, i32)> = HashMap::new();
        for row in rows {
            if self.finished() {
                break;
            }
            self.visits += 1;
            let top = self.flow.y;
            let start = self.mark();
            let row_style = self.styles[row.node].clone();
            let row_background = row_style.background.and_then(|color| {
                self.rect(left + gaps[0], top, (width - 2 * gaps[0]).max(0), 0, color)
            });
            if let NodeKind::Element { id: Some(id), .. } = &self.doc.nodes[row.node].kind {
                self.anchor(row.node, id, top);
            }
            let mut height = self
                .length(row_style.height, 0, &row_style)
                .unwrap_or(0)
                .max(0);
            let mut placed = Vec::new();
            let mut x = left + gaps[0];
            for (column, node) in row.cells.into_iter().enumerate() {
                let original = self.styles[node].clone();
                let padding = self.edges(original.padding, width, &original, false);
                let border = self.border(&original, width);
                let horizontal = padding[1] + padding[3] + border[1] + border[3];
                let mut used = original.clone();
                used.display = Display::Block;
                used.margin = [Length::Px(0.0); 4];
                used.width = Length::Px((widths[column] - horizontal).max(0) as f32 / self.zoom);
                used.min_width = Length::Auto;
                used.max_width = None;
                used.border_box = false;
                used.legacy_align = None;
                used.padding = padding.map(|n| Length::Px(n as f32 / self.zoom));
                used.border_width = border.map(|n| Length::Px(n as f32 / self.zoom));
                let vertical = padding[0] + padding[2] + border[0] + border[2];
                used.height = self
                    .dimension(original.height, None, &original, vertical)
                    .map(|n| Length::Px(n as f32 / self.zoom))
                    .unwrap_or(Length::Auto);
                used.min_height = self
                    .dimension(original.min_height, None, &original, vertical)
                    .map(|n| Length::Px(n as f32 / self.zoom))
                    .unwrap_or(Length::Auto);
                used.max_height = original
                    .max_height
                    .and_then(|n| self.dimension(n, None, &original, vertical))
                    .map(|n| Length::Px(n as f32 / self.zoom));
                let prior = std::mem::replace(
                    &mut self.flow,
                    Flow::new(
                        x,
                        x + widths[column].max(1),
                        top,
                        used.align,
                        used.white_space,
                    ),
                );
                self.styles[node] = used;
                self.walk(node, link, depth + 2);
                self.finish_line(false, 0);
                let cell_height = (self.flow.y - top).max(0);
                self.styles[node] = original;
                self.flow = prior;
                height = height.max(cell_height);
                placed.push((node, cell_height, vertical));
                x = x
                    .saturating_add(widths[column] + gaps[0])
                    .clamp(-MAX_HEIGHT, MAX_HEIGHT);
            }
            for (node, cell_height, vertical) in placed {
                let alignment = self
                    .doc
                    .attr(node, "valign")
                    .or_else(|| self.doc.attr(row.node, "valign"))
                    .unwrap_or("middle");
                let extra = (height - cell_height).max(0);
                let offset = if alignment.eq_ignore_ascii_case("bottom") {
                    extra
                } else if alignment.eq_ignore_ascii_case("middle") {
                    extra / 2
                } else {
                    0
                };
                // Stretch only the cell's border/background; its contents keep
                // their measured height and move together for vertical alignment.
                if let Some(mut content) = self.spans[node] {
                    if let Some(record) = &self.boxes[node] {
                        content.start.items += usize::from(record.background.is_some())
                            + usize::from(record.background_image.is_some())
                            + record.borders.iter().filter(|n| n.is_some()).count()
                            + usize::from(record.hit_box.is_some());
                        content.end.boxes =
                            content.end.boxes.saturating_sub(1).max(content.start.boxes);
                        if self.doc.attr(node, "id").is_some() {
                            content.start.anchors += 1;
                        }
                        self.move_span(content, 0, offset);
                    }
                    self.resize_box(node, (height - vertical).max(0), depth + 1);
                }
            }
            if let Some(background) = row_background {
                self.set_rect(
                    background,
                    left + gaps[0],
                    top,
                    (width - 2 * gaps[0]).max(0),
                    height,
                );
            }
            self.spans[row.node] = Some(Span {
                start,
                end: self.mark(),
            });
            if row_style.position == Position::Relative {
                self.relatives.push(row.node);
            }
            self.layout.client_boxes[row.node] = Some(ClientBox {
                width: row_width,
                height,
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
                padding: [0; 4],
            });
            // Row groups have no separate paint pass. Their rendered extent
            // spans their rows, including spacing between those rows but not
            // the gaps to adjacent groups. Stop at this table so nested tables
            // cannot contribute their own rows to an outer group's bounds.
            let mut ancestor = self.parents[row.node];
            for _ in 0..MAX_DEPTH {
                let Some(parent) = ancestor else { break };
                if parent == index {
                    break;
                }
                if self.styles[parent].display == Display::TableRowGroup {
                    let extent = group_extents.entry(parent).or_insert((top, top + height));
                    extent.0 = extent.0.min(top);
                    extent.1 = extent.1.max(top + height);
                }
                ancestor = self.parents[parent];
            }
            self.flow.y = top
                .saturating_add(height)
                .saturating_add(gaps[1])
                .min(MAX_HEIGHT);
            self.flow.x = left;
        }
        for (node, (top, bottom)) in group_extents {
            self.layout.client_boxes[node] = Some(ClientBox {
                width: row_width,
                height: (bottom - top).max(0),
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
                padding: [0; 4],
            });
        }
    }

    fn mark(&self) -> Mark {
        Mark {
            runs: self.layout.runs.len(),
            items: self.layout.items.len(),
            atoms: self.atoms.len(),
            controls: self.layout.controls.len(),
            anchors: self.layout.anchors.len(),
            boxes: self.placed_boxes.len(),
            pending: self.pending.len(),
            decorations: self.decorations.len(),
        }
    }

    fn move_span(&mut self, span: Span, dx: i32, dy: i32) {
        let shift = |n: &mut i32, delta: i32| {
            *n = n.saturating_add(delta).clamp(-MAX_HEIGHT, MAX_HEIGHT);
        };
        for run in &mut self.layout.runs[span.start.runs..span.end.runs] {
            shift(&mut run.x, dx);
            shift(&mut run.y, dy);
        }
        for item in &mut self.layout.items[span.start.items..span.end.items] {
            match item {
                PaintItem::Rect { x, y, .. }
                | PaintItem::RoundedRect { x, y, .. }
                | PaintItem::Background { x, y, .. }
                | PaintItem::Image { x, y, .. }
                | PaintItem::HitBox { x, y, .. } => {
                    shift(x, dx);
                    shift(y, dy);
                }
                _ => {}
            }
        }
        for atom in &mut self.atoms[span.start.atoms..span.end.atoms] {
            shift(&mut atom.x, dx);
            shift(&mut atom.y, dy);
        }
        for control in &mut self.layout.controls[span.start.controls..span.end.controls] {
            shift(&mut control.x, dx);
            shift(&mut control.y, dy);
        }
        for (_, y) in &mut self.layout.anchors[span.start.anchors..span.end.anchors] {
            shift(y, dy);
        }
        for &node in &self.placed_boxes[span.start.boxes..span.end.boxes] {
            if let Some(record) = &mut self.boxes[node] {
                shift(&mut record.x, dx);
                shift(&mut record.y, dy);
            }
        }
        for pending in &mut self.pending[span.start.pending..span.end.pending] {
            shift(&mut pending.x, dx);
            shift(&mut pending.y, dy);
        }
        for decoration in &mut self.decorations[span.start.decorations..span.end.decorations] {
            for item in &mut decoration.items {
                if let PaintItem::Rect { x, y, .. } | PaintItem::HitBox { x, y, .. } = item {
                    shift(x, dx);
                    shift(y, dy);
                }
            }
        }
    }

    fn collapsed_text_intrinsic(&self, text: &str, style: &Style) -> Intrinsic {
        // Keep the font's one-million-character shaping ceiling across the
        // complete text node, rather than restarting it for every word.
        let text = if text.len() > crate::text::MAX_SHAPED_CHARACTERS {
            let end = text.char_indices()
                .nth(crate::text::MAX_SHAPED_CHARACTERS)
                .map_or(text.len(), |(end, _)| end);
            &text[..end]
        } else {
            text
        };
        let advance = self.space_width(style);
        let font = self.font_spec(style);
        let (mut preferred, mut minimum, mut seen_word) = (0i32, 0i32, false);
        // text()/word() shape each collapsed word independently and round its
        // advance before adding a separately rounded space. Differences of
        // rounded positions in a whole-string shape can be one pixel shorter,
        // making a shrink-to-fit box wrap its own max-content estimate.
        for word in text.split_ascii_whitespace() {
            let width = font.as_ref().map_or_else(
                || (word.chars().count().min(MAX_HEIGHT as usize) as i32).saturating_mul(advance),
                |font| font.measure(word),
            );
            if seen_word {
                preferred = preferred.saturating_add(advance);
            }
            preferred = preferred.saturating_add(width);
            minimum = minimum.max(width);
            seen_word = true;
        }
        let preferred = preferred.min(MAX_WIDTH);
        Intrinsic {
            preferred,
            minimum: if style.white_space == WhiteSpace::NoWrap {
                preferred
            } else {
                minimum.min(MAX_WIDTH)
            },
        }
    }

    /// Intrinsic widths are cached once per node. This is a bounded approximation
    /// to CSS min/max-content using the renderer's fonts and supported boxes.
    fn intrinsic(&mut self, node: usize, depth: usize) -> Intrinsic {
        if depth > MAX_DEPTH || self.measure_visits >= 40_000 {
            if self.grid_budget.measuring { self.grid_budget.measurement_complete = false; }
            return Intrinsic::default();
        }
        if matches!(self.doc.nodes[node].kind, NodeKind::Comment(_) | NodeKind::ProcessingInstruction { .. } | NodeKind::DocumentFragment) { return Intrinsic::default(); }
        if let Some(value) = self.intrinsics[node] {
            return value;
        }
        if self.grid_budget.measuring && (!self.grid_budget.measurement_complete || !self.grid_intrinsic_cost(node)) {
            self.grid_budget.measurement_complete = false;
            return Intrinsic::default();
        }
        self.measure_visits += 1;
        let style = self.styles[node].clone();
        if style.display == Display::None || forms::kind(self.doc, node) == Some(Kind::Hidden) {
            return Intrinsic::default();
        }
        let mut value = match &self.doc.nodes[node].kind {
            NodeKind::Comment(_) | NodeKind::ProcessingInstruction { .. } | NodeKind::DocumentFragment => Intrinsic::default(),
            NodeKind::Text(text) if matches!(style.white_space, WhiteSpace::Normal | WhiteSpace::NoWrap) => {
                self.collapsed_text_intrinsic(text, &style)
            }
            NodeKind::Text(text) => {
                let advance = self.space_width(&style);
                let shaped = self.font_spec(&style).map(|font| font.shape(text));
                let (mut line, mut word, mut longest, mut preferred) =
                    (0i32, 0i32, 0i32, 0i32);
                for (i, character) in text.chars().enumerate() {
                    if matches!(character, '\r' | '\n') {
                        longest = longest.max(word);
                        preferred = preferred.max(line);
                        line = 0;
                        word = 0;
                    } else if matches!(character, ' ' | '\t' | '\u{000c}') {
                        longest = longest.max(word);
                        word = 0;
                        line = line.saturating_add(if character == '\t' {
                            4 * advance
                        } else {
                            advance
                        });
                    } else {
                        let width = shaped
                            .as_ref()
                            .map_or(advance, |shape| shape.x(i + 1) - shape.x(i));
                        line = line.saturating_add(width);
                        word = word.saturating_add(width);
                    }
                }
                preferred = preferred.max(line).min(MAX_WIDTH);
                Intrinsic {
                    preferred,
                    minimum: if style.white_space == WhiteSpace::Pre {
                        preferred
                    } else {
                        longest.max(word).min(MAX_WIDTH)
                    },
                }
            }
            NodeKind::Element { tag, .. } if tag == "img" || self.video(node) || self.canvas(node) || self.inline_svg(node) || self.loaded_image_control(node) => {
                let width = if self.canvas(node) {
                    let mut intrinsic_style = style.clone();
                    if intrinsic_style.width.has_percentage() {
                        intrinsic_style.width = Length::Auto;
                    }
                    self.image_dimensions(node, &intrinsic_style, 0).0
                } else if tag != "img" && !self.video(node) && !self.inline_svg(node) {
                    let mut intrinsic_style = style.clone();
                    if intrinsic_style.width.has_percentage() {
                        intrinsic_style.width = Length::Auto;
                    }
                    if intrinsic_style.max_width.is_some_and(Length::has_percentage) {
                        intrinsic_style.max_width = None;
                    }
                    self.image_control_dimensions(node, &intrinsic_style, 0).0
                } else {
                    self.doc
                        .attr(node, "width")
                        .and_then(|s| s.parse::<f32>().ok())
                        .filter(|n| n.is_finite() && *n >= 0.0)
                        .map(|n| (n * self.zoom).round() as i32)
                        .or_else(|| {
                            self.image_sizes
                                .get(&node)
                                .map(|(w, _)| (*w as f32 * self.zoom).round() as i32)
                        })
                        .unwrap_or_else(|| {
                            ((self
                                .doc
                                .attr(node, "alt")
                                .unwrap_or("Image")
                                .chars()
                                .count()
                                .min(40) as i32)
                                * GLYPH_ADVANCE
                                * self.scale(&style)
                                + 20)
                                .clamp(64, 360)
                        })
                };
                Intrinsic {
                    preferred: width.clamp(0, MAX_WIDTH),
                    minimum: width.clamp(0, MAX_WIDTH),
                }
            }
            NodeKind::Element { .. } => {
                if let Some(kind) = forms::kind(self.doc, node) {
                    let scale = self.scale(&style);
                    let width = if matches!(kind, Kind::Checkbox | Kind::Radio) {
                        (GLYPH_HEIGHT + 4) * scale
                    } else if matches!(kind, Kind::Text | Kind::TextArea) {
                        self.doc
                            .attr(
                                node,
                                if kind == Kind::TextArea {
                                    "cols"
                                } else {
                                    "size"
                                },
                            )
                            .and_then(|s| s.parse::<i32>().ok())
                            .filter(|n| *n > 0)
                            .unwrap_or(20)
                            .clamp(1, 200)
                            * self.text_width("0", &style).max(1)
                            + 12
                    } else {
                        self.text_width(
                            &forms::caption(self.doc, node, kind)
                                .chars()
                                .take(80)
                                .collect::<String>(),
                            &style,
                        )
                        .max(1)
                            + if kind == Kind::Select { 36 } else { 20 }
                    };
                    Intrinsic {
                        preferred: width,
                        minimum: width,
                    }
                } else {
                    let row = style.display == Display::Flex
                        && matches!(
                            style.flex_direction,
                            FlexDirection::Row | FlexDirection::RowReverse
                        );
                    let children = self.doc.nodes[node].children.clone();
                    let (mut line, mut preferred, mut minimum, mut count) =
                        (0i32, 0i32, 0i32, 0usize);
                    for child in children {
                        if self.styles[child].display == Display::None
                            || forms::kind(self.doc, child) == Some(Kind::Hidden)
                            || (matches!(
                                self.styles[child].position,
                                Position::Absolute | Position::Fixed
                            ) && matches!(self.doc.nodes[child].kind, NodeKind::Element { .. }))
                        {
                            continue;
                        }
                        if matches!(style.display, Display::Flex | Display::Grid)
                            && matches!(&self.doc.nodes[child].kind, NodeKind::Text(text) if text.chars().all(|c| matches!(c, ' ' | '\t' | '\r' | '\n' | '\u{000c}')))
                        {
                            continue;
                        }
                        if matches!(&self.doc.nodes[child].kind, NodeKind::Element { tag, .. } if tag == "br")
                        {
                            preferred = preferred.max(line);
                            line = 0;
                            continue;
                        }
                        let dimensions = self.intrinsic(child, depth + 1);
                        let child_style = &self.styles[child];
                        let margin =
                            if matches!(self.doc.nodes[child].kind, NodeKind::Element { .. }) {
                                self.edges(child_style.margin, 0, child_style, true)
                            } else {
                                [0; 4]
                            };
                        let outer = dimensions.preferred.saturating_add(margin[1] + margin[3]);
                        let child_min = dimensions.minimum.saturating_add(margin[1] + margin[3]);
                        if row {
                            preferred = preferred.saturating_add(outer);
                            minimum = if style.flex_wrap {
                                minimum.max(child_min)
                            } else {
                                minimum.saturating_add(child_min)
                            };
                            count += 1;
                        } else if style.display == Display::Flex
                            || ((style.display == Display::Grid || child_style.display.block_level())
                                && matches!(self.doc.nodes[child].kind, NodeKind::Element { .. }))
                        {
                            preferred = preferred.max(line).max(outer);
                            line = 0;
                            minimum = minimum.max(child_min);
                        } else {
                            line = line.saturating_add(outer);
                            minimum = minimum.max(child_min);
                        }
                    }
                    if row {
                        let gap = self.length(style.gap[1], 0, &style).unwrap_or(0).max(0);
                        let gaps =
                            gap.saturating_mul(count.saturating_sub(1).min(MAX_FLEX_ITEMS) as i32);
                        preferred = preferred.saturating_add(gaps);
                        if !style.flex_wrap {
                            minimum = minimum.saturating_add(gaps);
                        }
                    }
                    Intrinsic {
                        preferred: preferred.max(line).clamp(0, MAX_WIDTH),
                        minimum: minimum.clamp(0, MAX_WIDTH),
                    }
                }
            }
        };
        if style.display == Display::Table {
            let rows = self.table_rows(node);
            let columns = self.table_columns(&rows, depth);
            let spacing = self
                .length(style.border_spacing[0], 0, &style)
                .unwrap_or(0)
                .max(0);
            let edges = spacing.saturating_mul(columns.len().saturating_add(1).min(129) as i32);
            let minimum = columns.iter().map(|col| col.0).sum::<i32>();
            let preferred = columns.iter().map(|col| col.1).sum::<i32>();
            let percent = columns.iter().map(|col| col.2).sum::<f32>();
            let fixed = columns
                .iter()
                .filter(|col| col.2 == 0.0)
                .map(|col| col.1)
                .sum::<i32>();
            let mut desired = preferred as f32;
            if percent > 0.0 && percent < 1.0 {
                desired = desired.max(fixed as f32 / (1.0 - percent));
            }
            for &(.., width, percent) in &columns {
                if percent > 0.0 {
                    desired = desired.max(width as f32 / percent);
                }
            }
            value.minimum = minimum.saturating_add(edges).min(MAX_WIDTH);
            value.preferred = (desired.ceil() as i32).saturating_add(edges).min(MAX_WIDTH);
        }
        if style.white_space == WhiteSpace::NoWrap {
            value.minimum = value.preferred;
        }
        if matches!(self.doc.nodes[node].kind, NodeKind::Element { .. }) {
            let padding = self.edges(style.padding, 0, &style, false);
            let border = self.border(&style, 0);
            let kind = forms::kind(self.doc,node);
            let edges = if kind.is_some() && !self.loaded_image_control(node) {
                if native_button_padding(kind) {padding[1] + padding[3]} else {0}
            } else {
                padding[1] + padding[3] + border[1] + border[3]
            };
            if let Some(width) = self.dimension(style.width, None, &style, edges) {
                value.preferred = width;
                value.minimum = value.minimum.min(width);
            }
            value.preferred = self.constrained(
                value.preferred,
                style.min_width,
                style.max_width,
                None,
                &style,
                edges,
            );
            value.minimum = self.constrained(
                value.minimum,
                style.min_width,
                style.max_width,
                None,
                &style,
                edges,
            );
            value.preferred = value.preferred.saturating_add(edges).min(MAX_WIDTH);
            value.minimum = value.minimum.saturating_add(edges).min(MAX_WIDTH);
        }
        if !self.grid_budget.measuring || self.grid_budget.measurement_complete {
            self.intrinsics[node] = Some(value);
        }
        value
    }

    // Flex line admission and length distribution use fractional main sizes.
    // Round only when the line's cumulative sizes become physical rectangles;
    // rounding each 50% item early makes two halves overflow an odd-sized row.
    fn flex_dimension(&self, length: Length, reference: Option<i32>, style: &Style, edges: i32) -> Option<f64> {
        let physical=if let Length::Calc(value)=length{
            let reference=if length.has_percentage(){reference? as f32/self.zoom}else{0.0};
            Some(value.resolve(reference,style.font_size,style.root_font_size)*f64::from(self.zoom))
        }else if let Length::Percent(percent)=length {
            // Avoid a lossy CSS/physical round trip: two exact halves must not
            // exceed their reference because of division by fractional zoom.
            let css_limit=100_000.0*f64::from(self.zoom);
            Some((f64::from(reference?)*f64::from(percent)/100.0).clamp(-css_limit,css_limit))
        } else {
            length.resolve(0.0,style.font_size,style.root_font_size)
                .map(|value| f64::from(value)*f64::from(self.zoom))
        };
        physical.map(|value| {
            (value.clamp(-100_000.0,100_000.0)-if style.border_box {f64::from(edges)}else{0.0}).max(0.0)
        })
    }

    fn flex_parameters(&self, item: &mut FlexItem, row: bool, reference: Option<i32>) {
        let edges = item.edges[usize::from(!row)];
        let style = &item.style;
        let specified = self.flex_dimension(
            if row { style.width } else { style.height },
            reference,
            style,
            edges,
        );
        let natural = if row {
            (item.intrinsic.preferred - edges).max(0)
        } else {
            (item.natural_height - edges).max(0)
        };
        item.base =
            self.flex_dimension(style.flex_basis, reference, style, edges)
                .or(specified)
                .unwrap_or(f64::from(natural));
        let automatic_min = if row {
            (item.intrinsic.minimum - edges).max(0)
        } else {
            (item.minimum_height - edges).max(0)
        };
        let automatic_min = specified.map_or(f64::from(automatic_min), |n| f64::from(automatic_min).min(n));
        let maximum = (if row {
            style.max_width
        } else {
            style.max_height
        })
        .and_then(|v| self.flex_dimension(v, reference, style, edges));
        let automatic_min = maximum.map_or(automatic_min, |n| automatic_min.min(n));
        item.minimum =
            self.flex_dimension(
                if row {
                    style.min_width
                } else {
                    style.min_height
                },
                reference,
                style,
                edges,
            )
            .unwrap_or(automatic_min);
        item.maximum = maximum.unwrap_or(f64::from(if row { MAX_WIDTH } else { MAX_HEIGHT }))
            .max(item.minimum);
        item.main = item.base.clamp(item.minimum, item.maximum);
    }

    /// Resolve flex lengths with the specification's freezing/violation loop.
    /// Each iteration freezes an item; a shared work budget also bounds hostile
    /// collections of nested containers and min/max constraints.
    fn flex_sizes(&mut self, items: &mut [FlexItem], available: i32, gap: i32, row: bool) {
        if items.is_empty() {
            return;
        }
        let edges = |item: &FlexItem| {
            f64::from(if row {
                item.edges[0] + item.margin[1] + item.margin[3]
            } else {
                item.edges[1] + item.margin[0] + item.margin[2]
            })
        };
        let room = f64::from(available)
            - f64::from(gap) * (items.len() - 1) as f64
            - items.iter().map(edges).sum::<f64>();
        let grow = items.iter().map(|item| item.main).sum::<f64>() < room;
        let mut frozen: Vec<bool> = items
            .iter()
            .map(|item| {
                let factor = if grow {
                    item.style.flex_grow
                } else {
                    item.style.flex_shrink
                };
                factor == 0.0 || (grow && item.base > item.main) || (!grow && item.base < item.main)
            })
            .collect();
        let initial = room
            - items
                .iter()
                .zip(&frozen)
                .map(|(item, frozen)| if *frozen { item.main } else { item.base })
                .sum::<f64>();
        for _ in 0..=items.len() {
            if frozen.iter().all(|value| *value) || self.flex_work < items.len() {
                break;
            }
            self.flex_work -= items.len();
            let mut remaining = room
                - items
                    .iter()
                    .zip(&frozen)
                    .map(|(item, frozen)| if *frozen { item.main } else { item.base })
                    .sum::<f64>();
            let factors = items
                .iter()
                .zip(&frozen)
                .filter(|(_, frozen)| !**frozen)
                .map(|(item, _)| {
                    f64::from(if grow {
                        item.style.flex_grow
                    } else {
                        item.style.flex_shrink
                    })
                })
                .sum::<f64>();
            if factors < 1.0 && (initial * factors).abs() < remaining.abs() {
                remaining = initial * factors;
            }
            let weight = items
                .iter()
                .zip(&frozen)
                .filter(|(_, frozen)| !**frozen)
                .map(|(item, _)| {
                    if grow {
                        f64::from(item.style.flex_grow)
                    } else {
                        f64::from(item.style.flex_shrink) * item.base
                    }
                })
                .sum::<f64>();
            let mut violations = vec![0.0; items.len()];
            let mut total = 0.0;
            for (index, item) in items.iter_mut().enumerate() {
                if frozen[index] {
                    continue;
                }
                let factor = if grow {
                    f64::from(item.style.flex_grow)
                } else {
                    f64::from(item.style.flex_shrink) * item.base
                };
                let target = item.base
                    + if weight > 0.0 {
                        remaining * factor / weight
                    } else {
                        0.0
                    };
                item.main = target.clamp(item.minimum, item.maximum);
                violations[index] = item.main - target;
                total += violations[index];
            }
            for index in 0..items.len() {
                if total.abs() < 0.000_01
                    || (total > 0.0 && violations[index] > 0.0)
                    || (total < 0.0 && violations[index] < 0.0)
                {
                    frozen[index] = true;
                }
            }
        }
    }

    fn flex(
        &mut self,
        node: usize,
        style: &Style,
        link: Option<usize>,
        depth: usize,
        width: i32,
        height: Option<i32>,
    ) -> FlexGroup {
        let row = matches!(
            style.flex_direction,
            FlexDirection::Row | FlexDirection::RowReverse
        );
        let mut group = FlexGroup {
            style: style.clone(),
            items: Vec::new(),
            lines: Vec::new(),
        };
        for &child in &self.doc.nodes[node].children {
            if self.doc.nodes[child].kind.is_nonrendered_character_data() { continue; }
            if group.items.len() == MAX_FLEX_ITEMS {
                break;
            }
            if self.styles[child].display == Display::None
                || forms::kind(self.doc, child) == Some(Kind::Hidden)
            {
                continue;
            }
            if matches!(
                self.styles[child].position,
                Position::Absolute | Position::Fixed
            ) && matches!(self.doc.nodes[child].kind, NodeKind::Element { .. })
            {
                self.walk(child, link, depth + 1);
                continue;
            }
            let anonymous = matches!(self.doc.nodes[child].kind, NodeKind::Text(_));
            if let NodeKind::Text(text) = &self.doc.nodes[child].kind {
                if text
                    .chars()
                    .all(|c| matches!(c, ' ' | '\t' | '\r' | '\n' | '\u{000c}'))
                {
                    continue;
                }
            }
            let mut child_style = if anonymous {
                Style {
                    font_size: style.font_size,
                    root_font_size: style.root_font_size,
                    font_families: style.font_families.clone(),
                    font_weight: style.font_weight,
                    bold: style.bold,
                    italic: style.italic,
                    white_space: style.white_space,
                    ..Style::default()
                }
            } else {
                self.styles[child].clone()
            };
            let image = self.replaced_image(child);
            if image {
                // The existing image renderer honors HTML dimensions as size
                // hints. They also constrain flex sizing, including stretching.
                let hint = |name| {
                    if self.canvas(child) { return None; }
                    self.doc
                        .attr(child, name)
                        .and_then(|v| v.trim().parse::<f32>().ok())
                        .filter(|n| n.is_finite() && *n >= 0.0)
                        .map(|n| Length::Px(n.min(MAX_WIDTH as f32 / self.zoom)))
                };
                if child_style.width == Length::Auto {
                    if let Some(value) = hint("width") {
                        child_style.width = value;
                    }
                }
                if child_style.height == Length::Auto
                    || (height.is_none() && child_style.height.has_percentage())
                {
                    if let Some(value) = hint("height") {
                        child_style.height = value;
                    }
                }
            }
            let margin = self.edges(child_style.margin, width, &child_style, true);
            let padding = self.edges(child_style.padding, width, &child_style, false);
            let border = self.border(&child_style, width);
            let control = forms::kind(self.doc, child).is_some();
            let replaced = control || image;
            let edges = if control && !image {
                if native_button_padding(forms::kind(self.doc,child)) {
                    [padding[1] + padding[3],padding[0] + padding[2]]
                } else {[0, 0]}
            } else {
                [
                    padding[1] + padding[3] + border[1] + border[3],
                    padding[0] + padding[2] + border[0] + border[2],
                ]
            };
            let intrinsic = self.intrinsic(child, depth + 1);
            group.items.push(FlexItem {
                nodes: vec![child],
                style: child_style,
                margin,
                edges,
                intrinsic,
                base: 0.0,
                minimum: 0.0,
                maximum: 0.0,
                main: 0.0,
                width: 0,
                height: 0,
                natural_height: 0,
                minimum_height: 0,
                span: None,
                x: 0,
                y: 0,
                replaced,
                anonymous,
            });
        }
        let column_gap = self
            .length(style.gap[1], width, style)
            .unwrap_or(0)
            .clamp(0, MAX_WIDTH);
        if row {
            let (mut start, mut used) = (0, 0.0);
            for index in 0..group.items.len() {
                let item = &mut group.items[index];
                self.flex_parameters(item, true, Some(width));
                let outer = item.main + f64::from(item.edges[0] + item.margin[1] + item.margin[3]);
                if style.flex_wrap
                    && index > start
                    && used + f64::from(column_gap) + outer > f64::from(width)
                {
                    group.lines.push(start..index);
                    start = index;
                    used = 0.0;
                }
                if index > start {
                    used += f64::from(column_gap);
                }
                used += outer;
            }
            if start < group.items.len() {
                group.lines.push(start..group.items.len());
            }
            for line in &group.lines {
                self.flex_sizes(&mut group.items[line.clone()], width, column_gap, true);
                let (mut exact, mut assigned) = (0.0, 0i32);
                for item in &mut group.items[line.clone()] {
                    exact += item.main;
                    item.width = (exact.round() as i32 - assigned).clamp(0, MAX_WIDTH);
                    assigned += item.width;
                }
            }
        } else {
            if !group.items.is_empty() {
                group.lines.push(0..group.items.len());
            }
            for item in &mut group.items {
                let explicit =
                    self.dimension(item.style.width, Some(width), &item.style, item.edges[0]);
                let align = item.style.align_self.unwrap_or(style.align_items);
                let available = (width - item.edges[0] - item.margin[1] - item.margin[3]).max(0);
                let auto_margin =
                    item.style.margin[1] == Length::Auto || item.style.margin[3] == Length::Auto;
                let desired = explicit.unwrap_or_else(|| {
                    if align == FlexAlign::Stretch && !auto_margin {
                        available
                    } else {
                        (item.intrinsic.preferred - item.edges[0])
                            .min(available)
                            .max(0)
                    }
                });
                item.width = self
                    .constrained(
                        desired,
                        item.style.min_width,
                        item.style.max_width,
                        Some(width),
                        &item.style,
                        item.edges[0],
                    )
                    .min(MAX_WIDTH);
            }
        }
        let (left, top) = (self.flow.left, self.flow.y);
        for item in &mut group.items {
            if self.finished() {
                break;
            }
            let start = self.mark();
            let prior = std::mem::replace(
                &mut self.flow,
                Flow::new(
                    left,
                    left + (item.width + item.edges[0]).max(1),
                    top,
                    item.style.align,
                    item.style.white_space,
                ),
            );
            self.separate("\n");
            for &child in &item.nodes {
                let old = self.styles[child].clone();
                if !item.anonymous {
                    let mut used = old.clone();
                    used.margin = [Length::Px(0.0); 4];
                    used.width = Length::Px(item.width as f32 / self.zoom);
                    used.min_width = Length::Px(0.0);
                    used.max_width = None;
                    used.border_box = false;
                    used.padding = self
                        .edges(old.padding, width, &old, false)
                        .map(|n| Length::Px(n as f32 / self.zoom));
                    used.border_width = self
                        .border(&old, width)
                        .map(|n| Length::Px(n as f32 / self.zoom));
                    // The assigned content width must not change the original height's box model.
                    if let Some(h) =
                        self.dimension(item.style.height, height, &item.style, item.edges[1])
                    {
                        used.height = Length::Px(h as f32 / self.zoom);
                    }
                    used.min_height = self
                        .dimension(old.min_height, height, &old, item.edges[1])
                        .map(|n| Length::Px(n as f32 / self.zoom))
                        .unwrap_or(Length::Auto);
                    used.max_height = old
                        .max_height
                        .and_then(|n| self.dimension(n, height, &old, item.edges[1]))
                        .map(|n| Length::Px(n as f32 / self.zoom));
                    if matches!(used.display, Display::Inline | Display::InlineBlock) {
                        used.display = Display::Block;
                    }
                    self.styles[child] = used;
                }
                self.walk(child, link, depth + 1);
                self.styles[child] = old;
            }
            self.finish_line(false, 0);
            item.height = (self.flow.y - top).max(0);
            item.natural_height = item.height;
            item.minimum_height = self.boxes[item.nodes[0]]
                .as_ref()
                .map_or(item.height, |record| record.natural_height + item.edges[1]);
            item.span = Some(Span {
                start,
                end: self.mark(),
            });
            self.flow = prior;
        }
        let used_height = self.position_flex(&mut group, left, top, width, height, depth);
        self.flow.y = top.saturating_add(used_height).min(MAX_HEIGHT);
        self.flow.x = left;
        self.flow.space_pending = false;
        group
    }

    fn resize_box(&mut self, node: usize, height: i32, depth: usize) {
        let Some(mut record) = self.boxes[node].take() else {
            return;
        };
        record.height = height.clamp(0, MAX_HEIGHT);
        let outer_width = record.width
            + record.padding[1]
            + record.padding[3]
            + record.border[1]
            + record.border[3];
        let outer_height = record.height
            + record.padding[0]
            + record.padding[2]
            + record.border[0]
            + record.border[2];
        if let Some(item) = record.background {
            self.set_rect(item, record.x, record.y, outer_width, outer_height);
        }
        if let Some(item) = record.background_image {
            self.set_background_rect(item, record.x, record.y, outer_width, outer_height);
        }
        if let Some(item) = record.hit_box {
            if let PaintItem::HitBox { width, height, .. } = &mut self.layout.items[item] {
                *width = outer_width;
                *height = outer_height;
            }
        }
        let alpha=record.borders.map(|item|item.and_then(|item|match self.layout.items.get(item) {
            Some(PaintItem::Rect {alpha,..})=>Some(*alpha),_=>None,
        }).unwrap_or(255));
        let sides=crate::paint::border_segments(record.x,record.y,outer_width,outer_height,record.border,alpha);
        for (side, (x, y, w, h)) in sides.into_iter().enumerate() {
            if let Some(item) = record.borders[side] {
                self.set_rect(item, x, y, w, h);
            }
        }
        if depth < MAX_DEPTH {
            if let Some(formatting) = &mut record.formatting {
                match formatting {
                    FormattingContext::Flex(group) => {
                        self.position_flex(
                            group,
                            record.x + record.border[3] + record.padding[3],
                            record.y + record.border[0] + record.padding[0],
                            record.width,
                            Some(record.height),
                            depth + 1,
                        );
                    }
                    FormattingContext::Grid(group) => {
                        self.position_grid(group, node, Some(record.height), depth + 1);
                        self.grid_budget.incomplete |= !group.complete;
                    }
                }
            }
        }
        self.boxes[node] = Some(record);
    }

    fn resize_flex_item(&mut self, item: &mut FlexItem, outer_height: i32, depth: usize) {
        let outer_height = outer_height.max(item.edges[1]);
        if outer_height == item.height {
            return;
        }
        let content_height = outer_height - item.edges[1];
        if !item.anonymous && self.boxes[item.nodes[0]].is_some() {
            self.resize_box(item.nodes[0], content_height, depth + 1);
        } else if item.replaced {
            if let Some(span) = item.span {
                for atom in &mut self.atoms[span.start.atoms..span.end.atoms] {
                    atom.height = content_height;
                    atom.ascent = content_height;
                    match atom.kind {
                        AtomKind::Image { item: index, .. } => {
                            if let PaintItem::Image { height, .. } = &mut self.layout.items[index] {
                                *height = content_height;
                            }
                        }
                        AtomKind::Control(index) => {
                            let control = &mut self.layout.controls[index];
                            if let Some(image) = &mut control.image {
                                image.height = content_height;
                                control.height = outer_height;
                                atom.height = outer_height;
                                atom.ascent = outer_height;
                            } else {
                                control.height = content_height;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        item.height = outer_height;
    }

    fn pack(pack: FlexPack, free: f64, count: usize) -> (f64, f64) {
        match pack {
            FlexPack::End | FlexPack::LogicalEnd => (free, 0.0),
            FlexPack::Center => (free / 2.0, 0.0),
            FlexPack::Between if free > 0.0 && count > 1 => (0.0, free / (count - 1) as f64),
            FlexPack::Around if free > 0.0 && count > 0 => {
                (free / (2 * count) as f64, free / count as f64)
            }
            FlexPack::Evenly if free > 0.0 => {
                (free / (count + 1) as f64, free / (count + 1) as f64)
            }
            FlexPack::Around | FlexPack::Evenly => (free / 2.0, 0.0),
            _ => (0.0, 0.0),
        }
    }

    fn position_flex(
        &mut self,
        group: &mut FlexGroup,
        _left: i32,
        _top: i32,
        width: i32,
        height: Option<i32>,
        depth: usize,
    ) -> i32 {
        let row = matches!(
            group.style.flex_direction,
            FlexDirection::Row | FlexDirection::RowReverse
        );
        let reverse = matches!(
            group.style.flex_direction,
            FlexDirection::RowReverse | FlexDirection::ColumnReverse
        );
        let row_gap = if group.style.gap[0].has_percentage() && height.is_none() {
            0
        } else {
            self.length(group.style.gap[0], height.unwrap_or(0), &group.style)
                .unwrap_or(0)
                .clamp(0, MAX_WIDTH)
        };
        let column_gap = self
            .length(group.style.gap[1], width, &group.style)
            .unwrap_or(0)
            .clamp(0, MAX_WIDTH);
        if !row {
            for item in &mut group.items {
                self.flex_parameters(item, false, height);
            }
            if let Some(height) = height {
                self.flex_sizes(&mut group.items, height, row_gap, false);
            }
            let (mut exact, mut assigned) = (0.0, 0i32);
            for item in &mut group.items {
                exact += item.main;
                let content = (exact.round() as i32 - assigned).clamp(0, MAX_HEIGHT);
                assigned += content;
                self.resize_flex_item(item, content + item.edges[1], depth);
            }
        }
        let mut cross_sizes = Vec::new();
        for line in &group.lines {
            let mut cross = 0;
            for item in &group.items[line.clone()] {
                let value = if row {
                    let specified =
                        self.dimension(item.style.height, height, &item.style, item.edges[1]);
                    self.constrained(
                        specified.unwrap_or((item.natural_height - item.edges[1]).max(0)),
                        item.style.min_height,
                        item.style.max_height,
                        height,
                        &item.style,
                        item.edges[1],
                    ) + item.edges[1]
                        + item.margin[0]
                        + item.margin[2]
                } else {
                    width
                };
                cross = cross.max(value);
            }
            cross_sizes.push(cross.max(0));
        }
        let natural_height = if row {
            cross_sizes.iter().map(|&n| i64::from(n)).sum::<i64>()
                + i64::from(row_gap) * cross_sizes.len().saturating_sub(1) as i64
        } else {
            group
                .items
                .iter()
                .map(|i| i64::from(i.height) + i64::from(i.margin[0]) + i64::from(i.margin[2]))
                .sum::<i64>()
                + i64::from(row_gap) * group.items.len().saturating_sub(1) as i64
        };
        let natural_height = natural_height.clamp(0, i64::from(MAX_HEIGHT)) as i32;
        let height_reference = height;
        let height = height.unwrap_or(natural_height).clamp(0, MAX_HEIGHT);
        let mut cross_offset = 0.0;
        let mut cross_extra = 0.0;
        if row && !group.style.flex_wrap {
            if let Some(first) = cross_sizes.first_mut() {
                *first = height;
            }
        } else if row {
            let spare = height - natural_height;
            if group.style.align_content == FlexPack::Stretch
                && spare > 0
                && !cross_sizes.is_empty()
            {
                let count = cross_sizes.len() as i32;
                for (i, value) in cross_sizes.iter_mut().enumerate() {
                    *value += spare / count + i32::from((i as i32) < spare % count);
                }
            } else {
                (cross_offset, cross_extra) = Self::pack(
                    group.style.align_content,
                    f64::from(spare),
                    cross_sizes.len(),
                );
            }
        }
        for (line_index, line) in group.lines.iter().enumerate() {
            let main_size = if row { width } else { height };
            let main_gap = if row { column_gap } else { row_gap };
            let count = line.len();
            let used = group.items[line.clone()]
                .iter()
                .map(|item| {
                    i64::from(if row {
                        item.width + item.edges[0] + item.margin[1] + item.margin[3]
                    } else {
                        item.height + item.margin[0] + item.margin[2]
                    })
                })
                .sum::<i64>()
                + i64::from(main_gap) * count.saturating_sub(1) as i64;
            let used = used.clamp(-i64::from(MAX_HEIGHT), i64::from(MAX_HEIGHT)) as i32;
            let free = main_size - used;
            let (start_side, end_side) = if row {
                if reverse {
                    (1, 3)
                } else {
                    (3, 1)
                }
            } else if reverse {
                (2, 0)
            } else {
                (0, 2)
            };
            let auto_count = group.items[line.clone()]
                .iter()
                .map(|item| {
                    usize::from(item.style.margin[start_side] == Length::Auto)
                        + usize::from(item.style.margin[end_side] == Length::Auto)
                })
                .sum::<usize>();
            let auto_margin = if free > 0 && auto_count > 0 {
                f64::from(free) / auto_count as f64
            } else {
                0.0
            };
            let pack = match group.style.justify_content {
                FlexPack::LogicalStart if reverse => FlexPack::End,
                FlexPack::LogicalEnd if reverse => FlexPack::Start,
                other => other,
            };
            let (mut main_offset, main_extra) = Self::pack(
                pack,
                if auto_margin > 0.0 {
                    0.0
                } else {
                    f64::from(free)
                },
                count,
            );
            for item in &mut group.items[line.clone()] {
                let align = item.style.align_self.unwrap_or(group.style.align_items);
                let (cross_start, cross_end) = if row { (0, 2) } else { (3, 1) };
                let cross_size = cross_sizes[line_index];
                if row {
                    let specified = self.dimension(
                        item.style.height,
                        height_reference,
                        &item.style,
                        item.edges[1],
                    );
                    let auto_cross_margins = item.style.margin[0] == Length::Auto
                        || item.style.margin[2] == Length::Auto;
                    let desired = if align == FlexAlign::Stretch
                        && specified.is_none()
                        && !auto_cross_margins
                    {
                        cross_size - item.margin[0] - item.margin[2] - item.edges[1]
                    } else {
                        specified.unwrap_or((item.natural_height - item.edges[1]).max(0))
                    };
                    let desired = self.constrained(
                        desired.max(0),
                        item.style.min_height,
                        item.style.max_height,
                        height_reference,
                        &item.style,
                        item.edges[1],
                    );
                    self.resize_flex_item(item, desired + item.edges[1], depth);
                }
                let outer_cross = if row {
                    item.height
                } else {
                    item.width + item.edges[0]
                };
                let spare =
                    cross_size - outer_cross - item.margin[cross_start] - item.margin[cross_end];
                let auto_start = item.style.margin[cross_start] == Length::Auto;
                let auto_end = item.style.margin[cross_end] == Length::Auto;
                let align_offset = if auto_start || auto_end {
                    if auto_start && spare > 0 {
                        if auto_end {
                            spare / 2
                        } else {
                            spare
                        }
                    } else {
                        0
                    }
                } else {
                    match align {
                        FlexAlign::End => spare,
                        FlexAlign::Center => spare / 2,
                        _ => 0,
                    }
                };
                main_offset += f64::from(item.margin[start_side])
                    + if item.style.margin[start_side] == Length::Auto {
                        auto_margin
                    } else {
                        0.0
                    };
                let outer_main = if row {
                    item.width + item.edges[0]
                } else {
                    item.height
                };
                let main = if reverse {
                    f64::from(main_size - outer_main) - main_offset
                } else {
                    main_offset
                };
                let cross = cross_offset.round() as i32 + item.margin[cross_start] + align_offset;
                let (x, y) = if row {
                    (main.round() as i32, cross)
                } else {
                    (cross, main.round() as i32)
                };
                if let Some(span) = item.span {
                    self.move_span(span, x.saturating_sub(item.x), y.saturating_sub(item.y));
                }
                item.x = x;
                item.y = y;
                if let Some(&node)=item.nodes.first() {
                    let mut used=item.margin;
                    if item.style.margin[start_side]==Length::Auto {used[start_side]=(main_offset.round()-(main_offset-auto_margin).round()) as i32;}
                    if item.style.margin[end_side]==Length::Auto {let boundary=main_offset+f64::from(outer_main);used[end_side]=((boundary+auto_margin).round()-boundary.round()) as i32;}
                    if spare>0 {
                        if auto_start {used[cross_start]=used[cross_start].saturating_add(if auto_end{spare/2}else{spare});}
                        if auto_end {used[cross_end]=used[cross_end].saturating_add(if auto_start{spare-spare/2}else{spare});}
                    }
                    self.used_margins.insert(node,used);
                }
                main_offset += f64::from(outer_main + item.margin[end_side] + main_gap)
                    + main_extra
                    + if item.style.margin[end_side] == Length::Auto {
                        auto_margin
                    } else {
                        0.0
                    };
            }
            cross_offset += f64::from(cross_sizes[line_index] + row_gap) + cross_extra;
        }
        height
    }

    fn inline_block(&mut self, index: usize, style: &Style, link: Option<usize>, depth: usize) {
        let available = (self.flow.right - self.flow.left).max(1);
        let margin = self.edges(style.margin, available, style, true);
        let padding = self.edges(style.padding, available, style, false);
        let border = self.border(style, available);
        let horizontal = padding[1] + padding[3] + border[1] + border[3];
        let intrinsic = self.intrinsic(index, depth);
        let room = (available - margin[1] - margin[3] - horizontal).max(0);
        let width = self
            .dimension(style.width, Some(available), style, horizontal)
            .unwrap_or_else(|| {
                room.max((intrinsic.minimum - horizontal).max(0))
                    .min((intrinsic.preferred - horizontal).max(0))
            });
        let width = self
            .constrained(
                width,
                style.min_width,
                style.max_width,
                Some(available),
                style,
                horizontal,
            )
            .min(MAX_WIDTH);
        let outer_width = width + horizontal;
        let gap = if self.flow.space_pending && !self.flow.atoms.is_empty() {
            self.space_width(style)
        } else {
            0
        };
        if !matches!(self.flow.white_space, WhiteSpace::NoWrap | WhiteSpace::Pre)
            && !self.flow.atoms.is_empty()
            && self.flow.x + gap + margin[3] + outer_width + margin[1] > self.flow.right
        {
            self.finish_line(false, 0);
        } else {
            self.flow.x += gap;
        }
        let x = self.flow.x + margin[3];
        let y = self.flow.y + margin[0];
        let previous = std::mem::replace(
            &mut self.flow,
            Flow::new(0, outer_width.max(1), 0, style.align, style.white_space),
        );
        let previous_context = self.inline_context.replace(index);
        let separator = self.pending_separator;
        let start = self.mark();
        let mut used = style.clone();
        used.display = Display::Block;
        used.width = Length::Px(width as f32 / self.zoom);
        used.min_width = Length::Auto;
        used.max_width = None;
        used.border_box = false;
        used.margin = [Length::Px(0.0); 4];
        // Percentage edges refer to the containing block, not the shrunken box.
        used.padding = padding.map(|n| Length::Px(n as f32 / self.zoom));
        used.border_width = border.map(|n| Length::Px(n as f32 / self.zoom));
        let vertical = padding[0] + padding[2] + border[0] + border[2];
        used.height = self
            .dimension(style.height, None, style, vertical)
            .map(|n| Length::Px(n as f32 / self.zoom))
            .unwrap_or(Length::Auto);
        used.min_height = self
            .dimension(style.min_height, None, style, vertical)
            .map(|n| Length::Px(n as f32 / self.zoom))
            .unwrap_or(Length::Auto);
        used.max_height = style
            .max_height
            .and_then(|n| self.dimension(n, None, style, vertical))
            .map(|n| Length::Px(n as f32 / self.zoom));
        self.block(index, &used, link, depth);
        let height = self.flow.y.max(0);
        let span = Span {
            start,
            end: self.mark(),
        };
        let baseline = self.atoms[start.atoms..span.end.atoms]
            .iter()
            .filter(|atom| atom.context == Some(index))
            .max_by_key(|atom| atom.line)
            .map_or(height, |atom| atom.y + atom.ascent);
        if let Some(first) = self.layout.runs.get_mut(start.runs) {
            first.separator_before = if start.runs == 0 { "" } else { separator };
            self.pending_separator = "";
        } else {
            self.pending_separator = separator;
        }
        self.flow = previous;
        self.inline_context = previous_context;
        self.move_span(span, x, y);
        self.used_margins.insert(index,margin);
        self.atoms.push(Atom {
            kind: AtomKind::InlineBlock(span),
            x,
            y,
            width: outer_width,
            height,
            ascent: baseline,
            line: self.line_number,
            link,
            control: self.active_label.filter(|_| link.is_none()),
            context: self.inline_context,
        });
        self.flow.atoms.push(self.atoms.len() - 1);
        self.flow.x = (x + outer_width + margin[1]).clamp(-MAX_HEIGHT, MAX_HEIGHT);
        self.flow.ascent = self.flow.ascent.max(baseline + margin[0]);
        self.flow.descent = self.flow.descent.max(height - baseline + margin[2]);
        self.flow.line_height = self.flow.line_height.max(height + margin[0] + margin[2]);
        self.flow.space_pending = false;
    }

    fn inline(&mut self, index: usize, style: &Style, link: Option<usize>, depth: usize) {
        self.inline_boxes[index] = true;
        let parent_white_space = self.flow.white_space;
        self.flow.white_space = style.white_space;
        let available = (self.flow.right - self.flow.left).max(1);
        let padding = self.edges(style.padding, available, style, false);
        let margin = self.edges(style.margin, available, style, true);
        let border = self.border(style, available);
        let before = self.layout.items.len();
        let start = self.atoms.len();
        self.flow.x = self
            .flow
            .x
            .saturating_add(padding[3] + border[3] + margin[3])
            .clamp(-MAX_HEIGHT, MAX_HEIGHT);
        for &child in &self.doc.nodes[index].children {
            self.walk(child, link, depth + 1);
            if self.finished() {
                break;
            }
        }
        self.flow.x = self
            .flow
            .x
            .saturating_add(padding[1] + border[1] + margin[1])
            .clamp(-MAX_HEIGHT, MAX_HEIGHT);
        // A direct text inline uses its own font and line atoms throughout.
        // Nested inline boxes require their own fragment and baseline metrics.
        if self.doc.scripting_enabled && start < self.atoms.len() && !self.finished()
            && self.doc.nodes[index].namespace == crate::html::Namespace::Html
            && self.doc.nodes[index].children.iter().all(|&child| {
                self.doc.nodes.get(child).is_some_and(|child| {
                    matches!(child.kind, NodeKind::Text(_) | NodeKind::Comment(_) | NodeKind::ProcessingInstruction { .. })
                })
            })
        {
            self.text_inline_boxes.push(TextInlineBox {
                node: index,
                edges: std::array::from_fn(|i| padding[i] + border[i]),
            });
        }
        if self.decorations.len() < 8_192
            && (style.background.is_some()
                || border.iter().any(|width| *width > 0)
                || ((link.is_some() || self.active_label.is_some())
                    && padding.iter().any(|n| *n > 0)))
        {
            self.decorations.push(Decoration {
                node: index,
                before,
                depth,
                start,
                end: self.atoms.len(),
                background: style.background,
                border,
                padding,
                colors: style.border_color.map(|color| color.unwrap_or(style.color)),
                alpha: style.border_alpha,
                items: Vec::new(),
                context: self.inline_context,
                link,
                control: self.active_label.filter(|_| link.is_none()),
            });
        }
        self.flow.white_space = parent_white_space;
    }

    fn text(&mut self, text: &str, style: &Style, link: Option<usize>) {
        if matches!(style.white_space, WhiteSpace::Pre | WhiteSpace::PreWrap) {
            self.pre_text(text, style, link);
            return;
        }
        let mut word = String::new();
        for character in text.chars() {
            if matches!(character, ' ' | '\t' | '\r' | '\n' | '\u{000c}') {
                if !word.is_empty() {
                    self.word(&word, style, link);
                    word.clear();
                }
                self.flow.space_pending = true;
                self.separate(" ");
                if self.finished() {
                    return;
                }
            } else {
                word.push(character);
            }
        }
        if !word.is_empty() {
            self.word(&word, style, link);
        }
    }

    fn word(&mut self, word: &str, style: &Style, link: Option<usize>) {
        let advance = self.space_width(style);
        let shaped = self.font_spec(style).map(|font| font.shape(word));
        let characters: Vec<char> = word.chars().collect();
        let gap = if self.flow.space_pending && !self.flow.atoms.is_empty() {
            advance
        } else {
            0
        };
        let word_width = shaped.as_ref().map_or_else(
            || (characters.len().min(MAX_HEIGHT as usize) as i32).saturating_mul(advance),
            Shape::width,
        );
        if style.white_space != WhiteSpace::NoWrap
            && !self.flow.atoms.is_empty()
            && self.flow.x.saturating_add(gap).saturating_add(word_width) > self.flow.right
        {
            self.finish_line(false, 0);
        } else {
            self.flow.x = self.flow.x.saturating_add(gap);
        }
        self.flow.space_pending = false;
        if style.white_space == WhiteSpace::NoWrap {
            self.place_text(word.to_owned(), style, link);
            return;
        }
        let mut offset = 0;
        while offset < characters.len() && !self.finished() {
            let available = self.flow.right - self.flow.x;
            let count = shaped
                .as_ref()
                .map_or_else(
                    || (available / advance).max(1) as usize,
                    |shape| shape.fitting_count(offset, available).max(1),
                )
                .min(characters.len() - offset);
            let text: String = characters[offset..offset + count].iter().collect();
            self.place_text(text, style, link);
            offset += count;
            if offset < characters.len() {
                self.finish_line(false, 0);
            }
        }
    }

    fn pre_text(&mut self, text: &str, style: &Style, link: Option<usize>) {
        let advance = self.space_width(style);
        let font = self.font_spec(style);
        let mut chunk = String::new();
        let mut chunk_width = 0i32;
        let mut pen = crate::text::Pen::default();
        let mut column = ((self.flow.x - self.flow.left).max(0) / advance) as usize;
        let wrap = style.white_space == WhiteSpace::PreWrap;
        let flush = |engine: &mut Self, chunk: &mut String| {
            if !chunk.is_empty() {
                engine.place_text(std::mem::take(chunk), style, link);
            }
        };
        let mut characters = text.chars().peekable();
        while let Some(character) = characters.next() {
            if self.finished() {
                break;
            }
            if character == '\n' || character == '\r' {
                if character == '\r' && characters.peek() == Some(&'\n') {
                    characters.next();
                }
                flush(self, &mut chunk);
                chunk_width = 0;
                pen = crate::text::Pen::default();
                self.finish_line(true, self.line_height(style));
                self.separate("\n");
                column = 0;
                continue;
            }
            let count = if character == '\t' { 4 - column % 4 } else { 1 };
            for _ in 0..count {
                let shown = if character == '\t' { ' ' } else { character };
                let mut next_width = font.as_ref().map_or(chunk_width + advance, |font| {
                    font.advance_pen(&mut pen, shown)
                });
                if wrap && self.flow.x.saturating_add(next_width) > self.flow.right {
                    flush(self, &mut chunk);
                    pen = crate::text::Pen::default();
                    next_width = font
                        .as_ref()
                        .map_or(advance, |font| font.advance_pen(&mut pen, shown));
                    self.finish_line(false, 0);
                }
                chunk.push(shown);
                chunk_width = next_width;
                column += 1;
                // Bound a run and prevent quadratic recounting on long preserved lines.
                if chunk.len() >= 4_096 {
                    flush(self, &mut chunk);
                    chunk_width = 0;
                    pen = crate::text::Pen::default();
                }
            }
        }
        flush(self, &mut chunk);
        self.flow.space_pending = false;
    }

    fn add_run(
        &mut self,
        text: String,
        x: i32,
        y: i32,
        style: &Style,
        link: Option<usize>,
    ) -> Option<usize> {
        if self.layout.runs.len() == MAX_RUNS || self.layout.items.len() == MAX_ITEMS {
            return None;
        }
        let index = self.layout.runs.len();
        let scale = self.scale(style);
        let shaped = self.font_spec(style).map(|font| font.shape(&text));
        let pixel_width = shaped.as_ref().map_or_else(
            || {
                (text.chars().count().min(MAX_HEIGHT as usize) as i32)
                    .saturating_mul(GLYPH_ADVANCE * scale)
            },
            Shape::width,
        );
        self.layout.runs.push(TextRun {
            x,
            y,
            text,
            scale,
            color: style.color,
            bold: style.bold,
            italic: style.italic,
            underline: style.underline,
            separator_before: if index == 0 {
                ""
            } else {
                self.pending_separator
            },
            link,
            pixel_width,
            shaped,
        });
        self.run_nodes.push(self.current_node);
        self.pending_separator = "";
        self.layout.items.push(PaintItem::Text(index));
        Some(index)
    }

    fn place_text(&mut self, text: String, style: &Style, link: Option<usize>) {
        let Some(run) = self.add_run(text, self.flow.x, self.flow.y, style, link) else {
            return;
        };
        let width = self.layout.runs[run].width();
        let ascent = self.layout.runs[run].ascent();
        let text_height = self.layout.runs[run].height();
        self.atoms.push(Atom {
            kind: AtomKind::Text(run),
            x: self.flow.x,
            y: self.flow.y,
            width,
            height: text_height,
            ascent,
            line: self.line_number,
            link,
            control: self.active_label.filter(|_| link.is_none()),
            context: self.inline_context,
        });
        self.flow.atoms.push(self.atoms.len() - 1);
        self.flow.x = self
            .flow
            .x
            .saturating_add(width)
            .clamp(-MAX_HEIGHT, MAX_HEIGHT);
        self.flow.ascent = self.flow.ascent.max(ascent);
        self.flow.descent = self.flow.descent.max(text_height - ascent);
        self.flow.line_height = self.flow.line_height.max(self.line_height(style));
    }

    fn control(&mut self, node: usize, style: &Style, kind: Kind) {
        if self.layout.controls.len() >= forms::MAX_CONTROLS {
            return;
        }
        let available = (self.flow.right - self.flow.left).max(1);
        let scale = self.scale(style);
        let margin = self.edges(style.margin, available, style, true);
        let button_padding = native_button_padding(Some(kind));
        let padding = if button_padding {self.edges(style.padding,available,style,false)} else {[0;4]};
        let horizontal = padding[1] + padding[3];
        let vertical = padding[0] + padding[2];
        let toggle = matches!(kind, Kind::Checkbox | Kind::Radio);
        let default_width = if toggle {
            (GLYPH_HEIGHT + 4) * scale
        } else if matches!(kind, Kind::Text | Kind::TextArea) {
            let cells = self
                .doc
                .attr(
                    node,
                    if kind == Kind::TextArea {
                        "cols"
                    } else {
                        "size"
                    },
                )
                .and_then(|s| s.parse::<i32>().ok())
                .filter(|&v| v > 0)
                .unwrap_or(20)
                .clamp(1, 200);
            cells * self.text_width("0", style).max(1) + 12
        } else {
            self.text_width(
                &forms::caption(self.doc, node, kind)
                    .chars()
                    .take(80)
                    .collect::<String>(),
                style,
            )
            .max(1)
                + if kind == Kind::Select { 36 } else { 20 }
        };
        let mut width = self
            .dimension(style.width, Some(available), style, horizontal)
            .unwrap_or(default_width)
            .clamp(0, MAX_WIDTH);
        if let Some(maximum) = style
            .max_width
            .and_then(|v| self.dimension(v, Some(available), style, horizontal))
        {
            width = width.min(maximum.max(0));
        }
        // Intrinsic controls fit the column. An explicit CSS width may overflow it.
        if style.width == Length::Auto {
            width = width.min((available-horizontal).max(0));
        }
        let width = width.saturating_add(horizontal).clamp(0,MAX_WIDTH);
        let default_height = if kind == Kind::TextArea {
            let rows = self
                .doc
                .attr(node, "rows")
                .and_then(|value| value.parse::<i32>().ok())
                .filter(|&value| value > 0)
                .unwrap_or(2)
                .clamp(1, 100);
            let line = self
                .font_spec(style)
                .map_or((GLYPH_HEIGHT + 2) * scale, |font| font.height() + 2);
            rows * line + 8
        } else {
            self.font_spec(style)
                .map_or((GLYPH_HEIGHT + 6) * scale, |font| font.height() + 10)
        };
        let content_height = if button_padding && vertical != 0 {
            self.dimension(style.height,None,style,vertical)
        } else {self.length(style.height,0,style)};
        let height = content_height
            .unwrap_or(default_height)
            .clamp(0, MAX_WIDTH).saturating_add(vertical).clamp(0,MAX_WIDTH);
        if width == 0 || height == 0 {
            return;
        }
        let gap = if self.flow.space_pending && !self.flow.atoms.is_empty() {
            self.space_width(style)
        } else {
            0
        };
        if !matches!(self.flow.white_space, WhiteSpace::NoWrap | WhiteSpace::Pre)
            && !self.flow.atoms.is_empty()
            && self.flow.x + gap + margin[3] + width + margin[1] > self.flow.right
        {
            self.finish_line(false, 0);
        } else {
            self.flow.x += gap;
        }
        let x = self.flow.x + margin[3];
        let y = self.flow.y + margin[0];
        let index = self.layout.controls.len();
        self.layout.controls.push(ControlBox {
            node,
            x,
            y,
            width,
            height,
            scale,
            text_font: self.font_spec(style),
            color: style.color,
            background: style.background,
            padding,
            image: None,
        });
        self.used_margins.insert(node,margin);
        self.layout.items.push(PaintItem::Control(index));
        self.atoms.push(Atom {
            kind: AtomKind::Control(index),
            x,
            y,
            width,
            height,
            ascent: height,
            line: self.line_number,
            link: None,
            control: Some(node),
            context: self.inline_context,
        });
        self.flow.atoms.push(self.atoms.len() - 1);
        self.flow.x = (x + width + margin[1]).clamp(-MAX_HEIGHT, MAX_HEIGHT);
        self.flow.ascent = self.flow.ascent.max(height + margin[0]);
        self.flow.descent = self.flow.descent.max(margin[2]);
        self.flow.line_height = self.flow.line_height.max(height + margin[0] + margin[2]);
        self.flow.space_pending = false;
        self.separate(" ");
    }

    fn image_control_dimensions(&self, node: usize, style: &Style, available: i32) -> (i32, i32) {
        if !style.border_box {
            return self.image_dimensions(node, style, available);
        }
        let padding = self.edges(style.padding, available, style, false);
        let border = self.border(style, available);
        let horizontal = padding[1] + padding[3] + border[1] + border[3];
        let vertical = padding[0] + padding[2] + border[0] + border[2];
        let pixels = |value: i32| Length::Px(value as f32 / self.zoom);
        let mut used = style.clone();
        // HTML image-button dimensions are width/height presentational hints,
        // so an explicit border-box applies to these hints as well as CSS sizes.
        let hint = |name| {
            self.doc
                .attr(node, name)
                .and_then(|value| value.trim().parse::<f32>().ok())
                .filter(|value| value.is_finite() && *value >= 0.0)
                .map(|value| Length::Px(value.min(MAX_WIDTH as f32 / self.zoom)))
        };
        let width = if style.width == Length::Auto {
            hint("width").unwrap_or(style.width)
        } else {
            style.width
        };
        let height = if (matches!(style.height,Length::Auto)||style.height.has_percentage()) {
            hint("height").unwrap_or(style.height)
        } else {
            style.height
        };
        used.width = self
            .dimension(width, Some(available), style, horizontal)
            .map(pixels)
            .unwrap_or(Length::Auto);
        used.height = self
            .dimension(height, None, style, vertical)
            .map(pixels)
            .unwrap_or(style.height);
        used.max_width = style.max_width.and_then(|value| {
            self.dimension(value, Some(available), style, horizontal)
                .map(pixels)
        });
        used.border_box = false;
        self.image_dimensions(node, &used, available)
    }

    fn image_control(&mut self, node: usize, style: &Style) {
        if self.layout.controls.len() >= forms::MAX_CONTROLS || self.layout.items.len() >= MAX_ITEMS
        {
            return;
        }
        let available = (self.flow.right - self.flow.left).max(1);
        let padding = self.edges(style.padding, available, style, false);
        let border = self.border(style, available);
        let mut margin = self.edges(style.margin, available, style, true);
        let (content_width, content_height) = self.image_control_dimensions(node, style, available);
        if content_width == 0 || content_height == 0 {
            return;
        }
        let width = content_width + padding[1] + padding[3] + border[1] + border[3];
        let height = content_height + padding[0] + padding[2] + border[0] + border[2];
        if style.display.block_level() {
            let spare = (available - width - margin[1] - margin[3]).max(0);
            match (
                style.margin[3] == Length::Auto,
                style.margin[1] == Length::Auto,
            ) {
                (true, true) => {
                    margin[3] = spare / 2;
                    margin[1] = spare - margin[3];
                }
                (true, false) => margin[3] = spare,
                (false, true) => margin[1] = spare,
                _ => {}
            }
        }
        let gap = if self.flow.space_pending && !self.flow.atoms.is_empty() {
            self.space_width(style)
        } else {
            0
        };
        if !matches!(self.flow.white_space, WhiteSpace::NoWrap | WhiteSpace::Pre)
            && !self.flow.atoms.is_empty()
            && self.flow.x + gap + margin[3] + width + margin[1] > self.flow.right
        {
            self.finish_line(false, 0);
        } else {
            self.flow.x += gap;
        }
        let x = self.flow.x + margin[3];
        let y = self.flow.y + margin[0];
        let index = self.layout.controls.len();
        self.layout.controls.push(ControlBox {
            node,
            x,
            y,
            width,
            height,
            scale: self.scale(style),
            text_font: self.font_spec(style),
            color: style.color,
            background: style.background,
            padding: [0;4],
            image: Some(ImageControl {
                x: border[3] + padding[3],
                y: border[0] + padding[0],
                width: content_width,
                height: content_height,
                border,
                border_color: style.border_color.map(|color| color.unwrap_or(style.color)),
                border_alpha: style.border_alpha,
                presentation: self.image_presentation(node, style),
            }),
        });
        self.used_margins.insert(node,margin);
        self.layout.items.push(PaintItem::Control(index));
        self.atoms.push(Atom {
            kind: AtomKind::Control(index),
            x,
            y,
            width,
            height,
            ascent: height,
            line: self.line_number,
            link: None,
            control: Some(node),
            context: self.inline_context,
        });
        self.flow.atoms.push(self.atoms.len() - 1);
        self.flow.x = (x + width + margin[1]).clamp(-MAX_HEIGHT, MAX_HEIGHT);
        self.flow.ascent = self.flow.ascent.max(height + margin[0]);
        self.flow.descent = self.flow.descent.max(margin[2]);
        self.flow.line_height = self.flow.line_height.max(height + margin[0] + margin[2]);
        self.flow.space_pending = false;
        self.separate(" ");
    }

    fn background_presentation(&self, style: &Style) -> crate::paint::BackgroundPresentation {
        crate::paint::BackgroundPresentation {
            size: style.background_size,
            repeat: style.background_repeat,
            position: style.background_position.map(|position| {
                (
                    position.fraction,
                    self.length(position.offset, 0, style).unwrap_or(0) as f32,
                )
            }),
            scale: self.zoom,
            rendering: style.image_rendering,
        }
    }

    fn image_presentation(&self, node: usize, style: &Style) -> crate::paint::ImagePresentation {
        let natural = if let Some(&(width, height)) = self.canvas_sizes.get(&node) {
            (width.bounded_value(32_768.0) as f32, height.bounded_value(32_768.0) as f32)
        } else {
            let (width, height) = self.image_sizes.get(&node).copied().unwrap_or_default();
            (width.min(32_768) as f32, height.min(32_768) as f32)
        };
        crate::paint::ImagePresentation {
            fit: style.object_fit,
            rendering: style.image_rendering,
            position: style.object_position.map(|position| {
                (
                    position.fraction,
                    self.length(position.offset, 0, style).unwrap_or(0) as f32,
                )
            }),
            natural: (natural.0 * self.zoom, natural.1 * self.zoom),
        }
    }

    fn image_dimensions(&self, node: usize, style: &Style, available: i32) -> (i32, i32) {
        if self.canvas(node) { return self.canvas_dimensions(node, style, available); }
        let natural = self.image_sizes.get(&node).copied();
        let attribute = |name| {
            self.doc
                .attr(node, name)
                .and_then(|value| value.trim().parse::<f32>().ok())
                .filter(|value| value.is_finite() && *value >= 0.0)
                .map(|value| (value * self.zoom).round().clamp(0.0, MAX_WIDTH as f32) as i32)
        };
        let explicit_width = self
            .length(style.width, available, style)
            .or_else(|| attribute("width"));
        let explicit_height = (if style.height.has_percentage() {
            None
        } else {
            self.length(style.height, 0, style)
        })
        .or_else(|| attribute("height"));
        let (natural_width, natural_height) = natural
            .map(|(w, h)| {
                (
                    (w.min(32_768) as f32 * self.zoom).round() as i32,
                    (h.min(32_768) as f32 * self.zoom).round() as i32,
                )
            })
            .unwrap_or_else(|| {
                if self.video(node){return ((300.0*self.zoom).round() as i32,(150.0*self.zoom).round() as i32);}
                let alt = self
                    .doc
                    .attr(node, "alt")
                    .unwrap_or("Image")
                    .chars()
                    .count()
                    .min(40) as i32;
                (
                    (alt * GLYPH_ADVANCE * self.scale(style) + 20).clamp(64, 360),
                    self.line_height(style) + 12,
                )
            });
        let mut width = explicit_width.unwrap_or(natural_width).clamp(0, MAX_WIDTH);
        let mut height = explicit_height
            .unwrap_or_else(|| {
                if explicit_width.is_some() && natural_width > 0 {
                    (i64::from(natural_height) * i64::from(width) / i64::from(natural_width))
                        .clamp(0, i64::from(MAX_WIDTH)) as i32
                } else {
                    natural_height
                }
            })
            .clamp(0, MAX_WIDTH);
        if explicit_width.is_none() && explicit_height.is_some() && natural_height > 0 {
            width = (i64::from(natural_width) * i64::from(height) / i64::from(natural_height))
                .clamp(0, i64::from(MAX_WIDTH)) as i32;
        }
        if let Some(maximum) = style
            .max_width
            .and_then(|value| self.length(value, available, style))
        {
            if width > maximum.max(0) {
                if explicit_height.is_none() && width > 0 {
                    height =
                        (i64::from(height) * i64::from(maximum.max(0)) / i64::from(width)) as i32;
                }
                width = maximum.max(0);
            }
        }
        (width, height)
    }

    fn canvas_dimensions(&self, node: usize, style: &Style, available: i32) -> (i32, i32) {
        let &(natural_width, natural_height) = self.canvas_sizes.get(&node)
            .expect("Every genuine context-none canvas has preflighted natural dimensions.");
        let padding = self.edges(style.padding, available, style, false);
        let border = self.border(style, available);
        let horizontal = padding[1] + padding[3] + border[1] + border[3];
        let vertical = padding[0] + padding[2] + border[0] + border[2];
        let explicit_width = self.dimension(style.width, Some(available), style, horizontal);
        let explicit_height = self.dimension(style.height, None, style, vertical);
        let zoom = f64::from(self.zoom);
        let natural_width_px = natural_width.bounded_value(f64::from(MAX_HEIGHT) / zoom) * zoom;
        let natural_height_px = natural_height.bounded_value(f64::from(MAX_HEIGHT) / zoom) * zoom;
        let unconstrained_width = explicit_width.map(f64::from).unwrap_or_else(|| {
            explicit_height.and_then(|height| natural_width.scaled_ratio(natural_height, f64::from(height)))
                .unwrap_or(natural_width_px)
        }).round().clamp(0.0, f64::from(MAX_HEIGHT)) as i32;
        let width = self.constrained(unconstrained_width, style.min_width, style.max_width, Some(available), style, horizontal)
            .clamp(0, MAX_HEIGHT);
        let height = explicit_height.map(f64::from).unwrap_or_else(|| {
            if explicit_width.is_some() || width != unconstrained_width {
                natural_height.scaled_ratio(natural_width, f64::from(width)).unwrap_or(natural_height_px)
            } else { natural_height_px }
        }).round().clamp(0.0, f64::from(MAX_HEIGHT)) as i32;
        let height = self.constrained(height, style.min_height, style.max_height, None, style, vertical)
            .clamp(0, MAX_WIDTH);
        (width.min(MAX_WIDTH), height)
    }

    fn image(&mut self, node: usize, style: &Style, link: Option<usize>, depth: usize) {
        if self.layout.items.len() >= MAX_ITEMS {
            return;
        }
        let available = (self.flow.right - self.flow.left).max(1);
        let mut margin = self.edges(style.margin, available, style, true);
        let padding = self.edges(style.padding, available, style, false);
        let border = self.border(style, available);
        let (width, height) = self.image_dimensions(node, style, available);
        if !self.canvas(node) && (width == 0 || height == 0) {
            return;
        }
        let horizontal = padding[3] + border[3] + padding[1] + border[1];
        if style.display.block_level() {
            let spare = (available - width - horizontal - margin[3] - margin[1]).max(0);
            match (
                style.margin[3] == Length::Auto,
                style.margin[1] == Length::Auto,
            ) {
                (true, true) => {
                    margin[3] = spare / 2;
                    margin[1] = spare - margin[3];
                }
                (true, false) => margin[3] = spare,
                (false, true) => margin[1] = spare,
                _ => {}
            }
        }
        let gap = if self.flow.space_pending && !self.flow.atoms.is_empty() {
            self.space_width(style)
        } else {
            0
        };
        if !matches!(self.flow.white_space, WhiteSpace::NoWrap | WhiteSpace::Pre)
            && !self.flow.atoms.is_empty()
            && self.flow.x + gap + margin[3] + width + horizontal + margin[1] > self.flow.right
        {
            self.finish_line(false, 0);
        } else {
            self.flow.x += gap;
        }
        self.flow.x += margin[3] + border[3] + padding[3];
        let x = self.flow.x;
        let y = self.flow.y + margin[0];
        let item = self.layout.items.len();
        self.layout.items.push(PaintItem::Image {
            node,
            x,
            y,
            width,
            height,
            presentation: self.image_presentation(node, style),
        });
        // Keep resolved edges here; finish_client_boxes adds the final content
        // dimensions, which flex layout may still change.
        self.layout.client_boxes[node] = Some(ClientBox {
            width: padding[3] + padding[1],
            height: padding[0] + padding[2],
            left: border[3],
            top: border[0],
            right: border[1],
            bottom: border[2],
            padding,
        });
        self.used_margins.insert(node,margin);
        self.atoms.push(Atom {
            kind: AtomKind::Image { item, node },
            x,
            y,
            width,
            height,
            ascent: height,
            line: self.line_number,
            link,
            control: self.active_label.filter(|_| link.is_none()),
            context: self.inline_context,
        });
        self.flow.atoms.push(self.atoms.len() - 1);
        self.flow.x =
            (x + width + padding[1] + border[1] + margin[1]).clamp(-MAX_HEIGHT, MAX_HEIGHT);
        self.flow.ascent = self
            .flow
            .ascent
            .max(height + padding[0] + border[0] + margin[0]);
        self.flow.descent = self.flow.descent.max(padding[2] + border[2] + margin[2]);
        self.flow.line_height = self
            .flow
            .line_height
            .max(height + padding[0] + border[0] + margin[0] + padding[2] + border[2] + margin[2]);
        self.flow.space_pending = false;
        if self.decorations.len() < 8_192
            && (style.background.is_some() || border.iter().any(|width| *width > 0))
        {
            self.decorations.push(Decoration {
                node,
                before: item,
                depth,
                start: self.atoms.len() - 1,
                end: self.atoms.len(),
                background: style.background,
                border,
                padding,
                colors: style.border_color.map(|color| color.unwrap_or(style.color)),
                alpha: style.border_alpha,
                items: Vec::new(),
                context: self.inline_context,
                link: None,
                control: None,
            });
        }
    }

    fn finish_line(&mut self, force: bool, minimum: i32) {
        if self.flow.atoms.is_empty() {
            if force {
                self.flow.y = self.flow.y.saturating_add(minimum.max(1)).min(MAX_HEIGHT);
            }
        } else {
            let spare = (self.flow.right - self.flow.x).max(0);
            let shift = match self.flow.align {
                Align::Left => 0,
                Align::Center => spare / 2,
                Align::Right => spare,
            };
            let line_atoms = std::mem::take(&mut self.flow.atoms);
            for index in line_atoms {
                let atom = &mut self.atoms[index];
                let old_x = atom.x;
                let old_y = atom.y;
                atom.x = atom.x.saturating_add(shift);
                atom.y = self.flow.y.saturating_add(self.flow.ascent - atom.ascent);
                atom.line = self.line_number;
                match atom.kind {
                    AtomKind::Text(run) => {
                        self.layout.runs[run].x = atom.x;
                        self.layout.runs[run].y = atom.y;
                    }
                    AtomKind::Image { item, .. } => {
                        if let PaintItem::Image { x, y, .. } = &mut self.layout.items[item] {
                            *x = atom.x;
                            *y = atom.y;
                        }
                    }
                    AtomKind::Control(index) => {
                        self.layout.controls[index].x = atom.x;
                        self.layout.controls[index].y = atom.y;
                    }
                    AtomKind::InlineBlock(span) => {
                        let dx = atom.x - old_x;
                        let dy = atom.y - old_y;
                        self.move_span(span, dx, dy);
                    }
                }
            }
            let height = self
                .flow
                .line_height
                .max(self.flow.ascent + self.flow.descent)
                .max(minimum);
            self.flow.y = self.flow.y.saturating_add(height).min(MAX_HEIGHT);
            self.line_number += 1;
        }
        self.flow.x = self.flow.left;
        self.flow.line_height = 0;
        self.flow.ascent = 0;
        self.flow.descent = 0;
        self.flow.space_pending = false;
    }

    fn rect(&mut self, x: i32, y: i32, width: i32, height: i32, color: u32) -> Option<usize> {
        if self.layout.items.len() >= MAX_ITEMS {
            return None;
        }
        let index = self.layout.items.len();
        self.layout.items.push(PaintItem::Rect {
            x,
            y,
            width,
            height,
            color,
            alpha: 255,
        });
        Some(index)
    }

    #[allow(clippy::too_many_arguments)]
    fn background_image(
        &mut self,
        node: usize,
        style: &Style,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        border: [i32; 4],
    ) -> Option<usize> {
        if self.layout.items.len() >= MAX_ITEMS {
            return None;
        }
        let image = style.background_image.as_ref()?.clone();
        let index = self.layout.items.len();
        self.layout.items.push(PaintItem::Background {
            node, x, y, width, height, image,
            presentation: self.background_presentation(style),
            border,
        });
        Some(index)
    }

    fn set_background_rect(&mut self, item: usize, x: i32, y: i32, width: i32, height: i32) {
        if let PaintItem::Background { x: rx, y: ry, width: rw, height: rh, .. } = &mut self.layout.items[item] {
            *rx = x;
            *ry = y;
            *rw = width.max(0);
            *rh = height.max(0);
        }
    }

    fn set_rect(&mut self, item: usize, x: i32, y: i32, width: i32, height: i32) {
        if let PaintItem::Rect {
            x: rx,
            y: ry,
            width: rw,
            height: rh,
            ..
        } | PaintItem::RoundedRect {
            x: rx, y: ry, width: rw, height: rh, ..
        } = &mut self.layout.items[item]
        {
            *rx = x;
            *ry = y;
            *rw = width.max(0);
            *rh = height.max(0);
        }
    }

    fn cache_decorations(&mut self) {
        // Freeze each inline owner's original-flow fragments before relative
        // children move. Moving a subtree later translates only its own cached
        // decorations, never an ancestor's background or border.
        let mut remaining =
            MAX_ITEMS.saturating_sub(self.layout.items.len() + self.decoration_items);
        for decoration in &mut self.decorations[self.decoration_cursor..] {
            if remaining == 0 {
                break;
            }
            let mut lines: BTreeMap<usize, (i32, i32, i32, i32)> = BTreeMap::new();
            for atom in &self.atoms[decoration.start..decoration.end] {
                if atom.context != decoration.context {
                    continue;
                }
                let bounds = lines.entry(atom.line).or_insert((
                    atom.x,
                    atom.y,
                    atom.x + atom.width,
                    atom.y + atom.height,
                ));
                bounds.0 = bounds.0.min(atom.x);
                bounds.1 = bounds.1.min(atom.y);
                bounds.2 = bounds.2.max(atom.x + atom.width);
                bounds.3 = bounds.3.max(atom.y + atom.height);
            }
            let first = lines.keys().next().copied();
            let last = lines.keys().next_back().copied();
            let mut items = Vec::new();
            for (line, (left, top, right, bottom)) in lines {
                let border = [
                    decoration.border[0],
                    if Some(line) == last {
                        decoration.border[1]
                    } else {
                        0
                    },
                    decoration.border[2],
                    if Some(line) == first {
                        decoration.border[3]
                    } else {
                        0
                    },
                ];
                let x = left
                    - if Some(line) == first {
                        decoration.padding[3] + border[3]
                    } else {
                        0
                    };
                let y = top - decoration.padding[0] - border[0];
                let width = right - x
                    + if Some(line) == last {
                        decoration.padding[1] + border[1]
                    } else {
                        0
                    };
                let height = bottom - y + decoration.padding[2] + border[2];
                if let Some(color) = decoration.background {
                    items.push(PaintItem::Rect {
                        x,
                        y,
                        width,
                        height,
                        color,
                        alpha: 255,
                    });
                }
                let boxes = crate::paint::border_segments(x,y,width,height,border,decoration.alpha);
                for (side, (x, y, width, height)) in boxes.into_iter().enumerate() {
                    if border[side] > 0 {
                        items.push(PaintItem::Rect {
                            x,
                            y,
                            width,
                            height,
                            color: decoration.colors[side],
                            alpha: decoration.alpha[side],
                        });
                    }
                }
                if decoration.link.is_some() || decoration.control.is_some() {
                    items.push(PaintItem::HitBox {
                        x,
                        y,
                        width,
                        height,
                        link: decoration.link,
                        control: decoration.control,
                    });
                }
                if items.len() >= remaining {
                    break;
                }
            }
            items.truncate(remaining);
            remaining -= items.len();
            self.decoration_items += items.len();
            decoration.items = items;
        }
        self.decoration_cursor = self.decorations.len();
    }

    fn decoration_layer(&self, node: usize) -> (usize, bool) {
        if let Some(&rank) = self.translated_ranks.get(node) { return (rank, self.viewport_fixed(node)); }
        let mut parent = Some(node);
        let (mut layer, mut fixed) = (0, false);
        for _ in 0..MAX_DEPTH {
            let Some(index) = parent else {
                break;
            };
            let position = self.styles[index].position;
            if layer == 0 && position != Position::Static {
                layer = index + 1;
            }
            if position == Position::Fixed {
                fixed = true;
                break;
            }
            parent = self.parents[index];
        }
        (layer, fixed)
    }

    fn finish_decorations(&mut self) {
        let mut inserted: BTreeMap<usize, Vec<DecorationGroup>> = BTreeMap::new();
        let mut remaining = MAX_ITEMS.saturating_sub(self.layout.items.len());
        for index in 0..self.decorations.len() {
            if remaining == 0 {
                break;
            }
            let (layer, fixed) = self.decoration_layer(self.decorations[index].node);
            let clip = self.ancestor_clip(self.decorations[index].node);
            let opacity = self.opacity_nodes.get(self.decorations[index].node).copied().unwrap_or(0);
            let decoration = &mut self.decorations[index];
            let mut items = std::mem::take(&mut decoration.items);
            items.truncate(remaining);
            remaining -= items.len();
            inserted
                .entry(decoration.before)
                .or_default()
                .push(DecorationGroup {
                    depth: decoration.depth,
                    fixed,
                    layer,
                    clip,
                    opacity,
                    items,
                });
        }
        for groups in inserted.values_mut() {
            groups.sort_by_key(|group| group.depth);
        }
        let original = std::mem::take(&mut self.layout.items);
        let original_fixed = std::mem::take(&mut self.layout.fixed_items);
        let original_positioned = std::mem::take(&mut self.layout.positioned_items);
        let original_clips = std::mem::take(&mut self.layout.item_clips);
        let original_opacity = std::mem::take(&mut self.opacity_items);
        let has_opacity = !original_opacity.is_empty();
        let mut ordered =
            Vec::with_capacity(original.len() + MAX_ITEMS.saturating_sub(remaining).min(8_192));
        for (index, item) in original.into_iter().enumerate() {
            if let Some(groups) = inserted.remove(&index) {
                for group in groups {
                    self.layout
                        .fixed_items
                        .extend(std::iter::repeat_n(group.fixed, group.items.len()));
                    self.layout
                        .positioned_items
                        .extend(std::iter::repeat_n(group.layer, group.items.len()));
                    self.layout
                        .item_clips
                        .extend(std::iter::repeat_n(group.clip, group.items.len()));
                    if has_opacity { self.opacity_items.extend(std::iter::repeat_n(group.opacity, group.items.len())); }
                    ordered.extend(group.items);
                }
            }
            self.layout.fixed_items.push(original_fixed[index]);
            self.layout
                .positioned_items
                .push(original_positioned[index]);
            self.layout.item_clips.push(original_clips[index]);
            if has_opacity { self.opacity_items.push(original_opacity[index]); }
            ordered.push(item);
        }
        self.layout.items = ordered;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::html::parse;

    fn image_control_page(
        source: &str,
        width: i32,
        height: i32,
        zoom: f32,
        natural: Option<(usize, usize)>,
    ) -> Layout {
        let doc = parse(source);
        let images = doc
            .nodes
            .iter()
            .enumerate()
            .filter(|(node, _)| forms::kind(&doc, *node) == Some(Kind::Image))
            .filter_map(|(node, _)| natural.map(|size| (node, size)))
            .collect();
        build_viewport(&doc, width, height, zoom, &HashMap::new(), &images)
    }

    #[test]
    fn image_control_natural_geometry_keeps_padding_border_and_one_hit_target() {
        let page = image_control_page("<body style='margin:0'><input type=image src=go.gif alt=Run style='padding:3px 5px 7px 11px;border-top:1px solid red;border-right:2px solid green;border-bottom:3px solid blue;border-left:4px solid black;margin:13px 17px 19px 23px'></body>", 300, 200, 1.0, Some((80, 40)));
        assert_eq!(page.controls.len(), 1);
        let control = &page.controls[0];
        let image = control.image.as_ref().unwrap();
        assert_eq!(
            (control.x, control.y, control.width, control.height),
            (23, 13, 102, 54)
        );
        assert_eq!(
            (image.x, image.y, image.width, image.height),
            (15, 4, 80, 40)
        );
        assert_eq!(image.border, [1, 2, 3, 4]);
        assert_eq!(image.border_color, [0xff0000, 0x008000, 0x0000ff, 0x000000]);
        assert_eq!(page.control_at(23, 13), Some(control.node));
        assert_eq!(page.control_at(124, 66), Some(control.node));
        assert_eq!(page.control_at(22, 13), None);
        assert_eq!(page.control_at(125, 13), None);
        assert_eq!(page.control_at(23, 67), None);
        assert_eq!(
            page.items
                .iter()
                .filter(|item| matches!(item, PaintItem::Control(_)))
                .count(),
            1
        );
        assert!(!page
            .items
            .iter()
            .any(|item| matches!(item, PaintItem::Image { .. })));
    }

    #[test]
    fn image_control_dimensions_preserve_ratio_hints_css_box_model_and_zoom() {
        for (attrs, css, content, outer) in [
            ("", "", (120, 60), (120, 60)),
            ("width=60", "", (60, 30), (60, 30)),
            ("height=15", "", (30, 15), (30, 15)),
            ("width=60 height=25", "", (60, 25), (60, 25)),
            (
                "width=60 height=25",
                "width:90px;height:45px",
                (90, 45),
                (90, 45),
            ),
            ("width=60", "max-width:30px", (30, 15), (30, 15)),
            ("", "width:50%", (100, 50), (100, 50)),
            ("", "height:30px", (60, 30), (60, 30)),
            (
                "",
                "box-sizing:border-box;width:80px;height:40px;padding:4px;border:2px solid red",
                (68, 28),
                (80, 40),
            ),
            (
                "",
                "box-sizing:border-box;width:80px;max-width:60px;padding:4px;border:2px solid red",
                (48, 24),
                (60, 36),
            ),
            (
                "width=80 height=40",
                "box-sizing:border-box;padding:4px;border:2px solid red",
                (68, 28),
                (80, 40),
            ),
        ] {
            let source = format!("<body style='margin:0'><input type=image src=go.gif alt=Run {attrs} style='{css}'></body>");
            let page = image_control_page(&source, 200, 200, 1.0, Some((120, 60)));
            let control = &page.controls[0];
            let image = control.image.as_ref().unwrap();
            assert_eq!((image.width, image.height), content, "{attrs} / {css}");
            assert_eq!((control.width, control.height), outer, "{attrs} / {css}");
        }
        for zoom in [0.5, 1.0, 1.5, 2.0] {
            let page = image_control_page("<body style='margin:0'><input type=image src=go.gif width=60 style='padding:4px;border:2px solid red'></body>", 400, 200, zoom, Some((120, 60)));
            let control = &page.controls[0];
            let image = control.image.as_ref().unwrap();
            assert_eq!(
                (image.x, image.y, image.width, image.height),
                (
                    (6.0 * zoom) as i32,
                    (6.0 * zoom) as i32,
                    (60.0 * zoom) as i32,
                    (30.0 * zoom) as i32
                )
            );
            assert_eq!(
                (control.width, control.height),
                ((72.0 * zoom) as i32, (42.0 * zoom) as i32)
            );
            assert_eq!(
                page.control_at(control.x + 1, control.y + 1),
                Some(control.node)
            );
        }
    }

    #[test]
    fn missing_image_control_uses_the_normal_alt_caption_button() {
        let source =
            "<body style='margin:0'><input type=image src=missing.gif alt='Run check'></body>";
        let fallback = image_control_page(source, 300, 200, 1.0, None);
        let button = layout(
            &parse("<body style='margin:0'><input type=submit value='Run check'></body>"),
            300,
        );
        let control = &fallback.controls[0];
        let ordinary = &button.controls[0];
        assert!(control.image.is_none());
        assert_eq!(
            (control.width, control.height),
            (ordinary.width, ordinary.height)
        );
        assert_eq!(fallback.control_at(1, 1), Some(control.node));
        let loaded = image_control_page(source, 300, 200, 1.0, Some((30, 15)));
        assert_eq!(
            (loaded.controls[0].width, loaded.controls[0].height),
            (30, 15)
        );
        assert!(loaded.controls[0].image.is_some());
    }

    #[test]
    fn image_control_positioned_border_boxes_and_fixed_scroll_preserve_content_origin() {
        for position in ["absolute", "fixed"] {
            let source = format!("<body style='margin:0'><input type=image src=go.gif width=80 height=40 style='box-sizing:border-box;position:{position};right:10px;bottom:20px;padding:4px;border:2px solid red'></body>");
            let mut page = image_control_page(&source, 300, 200, 1.0, Some((120, 60)));
            let control = &page.controls[0];
            let node = control.node;
            let image = control.image.as_ref().unwrap();
            assert_eq!(
                (control.x, control.y, control.width, control.height),
                (210, 140, 80, 40),
                "{position}"
            );
            assert_eq!(
                (image.x, image.y, image.width, image.height),
                (6, 6, 68, 28)
            );
            assert_eq!(page.control_at(211, 141), Some(node));
            page.set_scroll(100);
            let expected_y = if position == "fixed" { 240 } else { 140 };
            assert_eq!(page.controls[0].y, expected_y);
            assert_eq!(page.controls[0].image.as_ref().unwrap().y, 6);
            assert_eq!(page.control_at(211, expected_y + 1), Some(node));
            page.set_scroll(0);
            assert_eq!(page.controls[0].y, 140);
        }
    }

    #[test]
    fn image_control_flex_shrink_stretch_and_intrinsics_keep_content_inside_control() {
        let shrinking = image_control_page("<body style='margin:0'><div style='display:flex;width:100px;align-items:flex-start'><input type=image src=go.gif style='min-width:0;padding:4px;border:1px solid red'><input type=image src=go.gif style='min-width:0;padding:4px;border:1px solid red'></div></body>", 300, 200, 1.0, Some((120, 60)));
        assert_eq!(shrinking.controls.len(), 2);
        for (index, control) in shrinking.controls.iter().enumerate() {
            let image = control.image.as_ref().unwrap();
            assert_eq!(
                (control.x, control.y, control.width, control.height),
                (50 * index as i32, 0, 50, 30)
            );
            assert_eq!(
                (image.x, image.y, image.width, image.height),
                (5, 5, 40, 20)
            );
            assert_eq!(shrinking.control_at(control.x + 49, 29), Some(control.node));
        }
        let stretched = image_control_page("<body style='margin:0'><div style='display:flex;width:200px;height:100px'><input type=image src=go.gif style='padding:4px;border:1px solid red'></div></body>", 300, 200, 1.0, Some((120, 60)));
        let control = &stretched.controls[0];
        let image = control.image.as_ref().unwrap();
        assert_eq!((control.width, control.height), (130, 100));
        assert_eq!(
            (image.x, image.y, image.width, image.height),
            (5, 5, 120, 90)
        );
        assert_eq!(stretched.control_at(129, 99), Some(control.node));
        let column = image_control_page("<body style='margin:0'><div style='display:flex;flex-direction:column;width:200px;height:100px'><input type=image src=go.gif style='flex:1 1 0px;min-height:0;padding:4px;border:1px solid red'></div></body>", 300, 200, 1.0, Some((120, 60)));
        let control = &column.controls[0];
        let image = control.image.as_ref().unwrap();
        assert_eq!((control.width, control.height), (200, 100));
        assert_eq!((image.width, image.height), (190, 90));
        assert_eq!(column.control_at(199, 99), Some(control.node));
        let intrinsic = image_control_page("<body style='margin:0'><span style='display:inline-block;background:blue'><input type=image src=go.gif height=20 style='padding:4px;border:1px solid red'></span></body>", 300, 200, 1.0, Some((120, 60)));
        assert_eq!(intrinsic.controls[0].width, 50);
        assert_eq!(rectangle(&intrinsic, 0x0000ff).2, 50);
    }

    #[test]
    fn image_control_clipping_and_relative_movement_share_paint_and_hit_geometry() {
        let page = image_control_page("<body style='margin:0'><div style='position:relative;left:20px;top:10px;width:50px;height:30px;overflow:hidden'><input type=image src=go.gif style='position:relative;left:10px;top:5px;padding:4px;border:1px solid red'></div></body>", 300, 200, 1.0, Some((80, 40)));
        let control = &page.controls[0];
        assert_eq!(
            (control.x, control.y, control.width, control.height),
            (30, 15, 90, 50)
        );
        assert_eq!(control.image.as_ref().unwrap().x, 5);
        let item = page
            .items
            .iter()
            .position(|item| matches!(item, PaintItem::Control(_)))
            .unwrap();
        assert_eq!(page.paint_clip(item), Some(ClipRect::new(20, 10, 50, 30)));
        assert_eq!(page.control_at(31, 16), Some(control.node));
        assert_eq!(page.control_at(69, 39), Some(control.node));
        assert_eq!(page.control_at(70, 39), None);
        assert_eq!(page.control_at(69, 40), None);
    }

    #[test]
    fn overflow_collapsed_content_has_no_paint_hit_or_scroll_extent() {
        let source = "<body style='margin:0'><div style='max-height:0;overflow:hidden'><a href=hidden>Hidden</a><input name=q><div style='height:3000px;background:red'>Long</div></div><div style='height:20px;background:blue'>After</div></body>";
        let page = layout(&parse(source), 400);
        assert_eq!(page.height, 36);
        assert_eq!(rectangle(&page, 0x0000ff), (0, 0, 400, 20));
        let hidden = page
            .runs
            .iter()
            .position(|run| run.text == "Hidden")
            .unwrap();
        let clip = page.run_clip(hidden).unwrap();
        assert_eq!(clip.top, clip.bottom);
        assert_eq!(page.link_at(2, 2), None);
        let control = &page.controls[0];
        assert_eq!(page.control_at(control.x + 2, control.y + 2), None);
        let visible = layout(
            &parse(&source.replace("overflow:hidden", "overflow:visible")),
            400,
        );
        assert!(visible.height >= 3000);
        assert!(visible.run_clip(hidden).is_none());
    }

    #[test]
    fn overflow_nested_padding_clips_move_with_relative_and_flex_layout() {
        let doc = parse("<body style='margin:0'><div style='width:100px;height:40px;padding:5px;border:2px solid blue;overflow:hidden;position:relative;left:20px;top:10px'><div style='width:100px;height:100px;overflow:hidden;position:relative;left:30px;top:15px'><a href=wide style='display:block;width:200px;height:100px;background:red'>Wide</a></div></div></body>");
        let page = layout(&doc, 400);
        let red = page
            .items
            .iter()
            .position(|item| {
                matches!(
                    item,
                    PaintItem::Rect {
                        color: 0xff0000,
                        ..
                    }
                )
            })
            .unwrap();
        assert_eq!(page.paint_clip(red), Some(ClipRect::new(57, 32, 75, 30)));
        assert_eq!(page.link_at(60, 35), Some("wide"));
        assert_eq!(page.link_at(134, 35), None);
        assert_eq!(page.link_at(60, 64), None);
        let flex = layout(&parse("<body style='margin:0'><div style='display:flex;width:200px;height:100px;justify-content:flex-end;align-items:flex-end'><div style='width:60px;height:25px;overflow:hidden'><a href=flex style='display:block;width:100px;height:80px;background:red'>Flex</a></div></div></body>"), 400);
        let red = flex
            .items
            .iter()
            .position(|item| {
                matches!(
                    item,
                    PaintItem::Rect {
                        color: 0xff0000,
                        ..
                    }
                )
            })
            .unwrap();
        assert_eq!(flex.paint_clip(red), Some(ClipRect::new(140, 75, 60, 25)));
        assert_eq!(flex.link_at(145, 80), Some("flex"));
        assert_eq!(flex.link_at(205, 80), None);
        assert_eq!(flex.link_at(145, 101), None);
    }

    #[test]
    fn overflow_axes_nested_clips_intersect_across_paint_and_link_hits() {
        let doc = parse(concat!(
            "<body style='margin:0'><div style='width:80px;height:90px;overflow:clip visible'>",
            "<div style='width:120px;height:60px;overflow:visible clip'>",
            "<a href=inside style='display:block;width:120px;height:90px;background:red'></a>",
            "</div></div></body>"
        ));
        let page = layout(&doc, 300);
        let item = page.items.iter().position(|item| {
            matches!(item, PaintItem::Rect { color: 0xff0000, .. })
        }).unwrap();
        assert_eq!(page.paint_clip(item), Some(ClipRect::new(0, 0, 80, 60)));
        assert_eq!(page.link_at(79, 59), Some("inside"));
        assert_eq!(page.link_at(80, 59), None);
        assert_eq!(page.link_at(79, 60), None);
    }

    #[test]
    fn overflow_positioned_descendants_use_their_containing_block() {
        let doc = parse("<body style='margin:0'><div style='position:relative;width:200px;height:100px;overflow:hidden'><div style='height:0;overflow:hidden'><a href=absolute style='position:absolute;left:50px;top:10px;width:200px;height:30px;background:red'>Absolute</a><a href=fixed style='position:fixed;left:260px;top:20px;width:60px;height:30px;background:blue'>Fixed</a></div></div><div style='position:fixed;left:20px;top:140px;width:50px;height:30px;overflow:hidden'><a href=child style='display:block;width:100px;height:60px;background:green'>Child</a></div></body>");
        let mut page = layout(&doc, 400);
        assert_eq!(page.link_at(60, 20), Some("absolute"));
        assert_eq!(page.link_at(205, 20), None);
        assert_eq!(page.link_at(265, 25), Some("fixed"));
        assert_eq!(page.link_at(25, 145), Some("child"));
        assert_eq!(page.link_at(75, 145), None);
        let child = page
            .runs
            .iter()
            .position(|run| run.text == "Child")
            .unwrap();
        let initial = page.run_clip(child).unwrap();
        page.set_scroll(200);
        assert_eq!(page.run_clip(child), Some(initial.translate_y(200)));
        assert_eq!(page.link_at(25, 345), Some("child"));
        assert_eq!(page.link_at(75, 345), None);
        page.set_scroll(0);
        assert_eq!(page.run_clip(child), Some(initial));
        assert_eq!(page.link_at(25, 145), Some("child"));
    }

    fn rectangle(page: &Layout, wanted: u32) -> (i32, i32, i32, i32) {
        page.items
            .iter()
            .find_map(|item| match item {
                PaintItem::Rect {
                    x,
                    y,
                    width,
                    height,
                    color,
                    ..
                } if *color == wanted => Some((*x, *y, *width, *height)),
                _ => None,
            })
            .unwrap()
    }

    #[test]
    fn inline_blocks_center_and_keep_block_controls_in_one_row() {
        // The old full-width button rows came from an inline-block wrapper
        // containing a block box. This original fixture isolates that pattern.
        let doc = parse("<style>body{margin:0}.outer{display:inline-block}.inner{display:block;height:30px;border:1px solid #777;background:#eee}input{height:30px}</style><body><center><span class=outer><span class=inner><input id=first type=submit value=Search style='width:130px'></span></span> <span class=outer><span class=inner><input id=second type=submit value=Explore style='width:150px'></span></span></center></body>");
        let page = layout(&doc, 500);
        assert_eq!(page.controls.len(), 2);
        let first = &page.controls[0];
        let second = &page.controls[1];
        assert_eq!((first.x, first.y, first.width), (103, 1, 130));
        assert_eq!((second.x, second.y, second.width), (247, 1, 150));
        assert_eq!(
            page.control_at(first.x + 20, first.y + 10),
            Some(first.node)
        );
        assert_eq!(
            page.control_at(second.x + 20, second.y + 10),
            Some(second.node)
        );
        let panels: Vec<_> = page
            .items
            .iter()
            .filter_map(|item| match item {
                PaintItem::Rect {
                    x,
                    y,
                    width,
                    height,
                    color: 0xeeeeee,
                    ..
                } => Some((*x, *y, *width, *height)),
                _ => None,
            })
            .collect();
        assert_eq!(panels, [(102, 0, 132, 32), (246, 0, 152, 32)]);
        let narrow = layout(&doc, 220);
        assert!(narrow.controls[1].y > narrow.controls[0].y);
        for control in &narrow.controls {
            assert_eq!(
                narrow.control_at(control.x + 1, control.y + 1),
                Some(control.node)
            );
        }
    }

    #[test]
    fn nested_inline_blocks_move_links_anchors_and_decorations_together() {
        let doc = parse("<body style='margin:0;text-align:center'><span style='background:red'>X<span style='display:inline-block;width:100px;padding:10px;background:blue;position:relative;left:5px'><div><a id=inside href=target>Top</a></div><div style='display:inline-block;background:yellow'>Bottom</div></span>Y</span><p>After</p></body>");
        let page = layout(&doc, 400);
        let blue = rectangle(&page, 0x0000ff);
        assert_eq!(blue.0, 145);
        assert_eq!(blue.2, 120);
        let top = page.runs.iter().find(|run| run.text == "Top").unwrap();
        assert_eq!(page.link_at(top.x + 2, top.y + 2), Some("target"));
        let anchor = page.anchors.iter().find(|(id, _)| id == "inside").unwrap();
        assert_eq!(anchor.1, top.y);
        let red = rectangle(&page, 0xff0000);
        assert_eq!(red.0, 128);
        assert_eq!(red.2, 144);
        assert_eq!(
            page.items
                .iter()
                .filter(|item| matches!(
                    item,
                    PaintItem::Rect {
                        color: 0xff0000,
                        ..
                    }
                ))
                .count(),
            1
        );
        let yellow = rectangle(&page, 0xffff00);
        let bottom = page.runs.iter().find(|run| run.text == "Bottom").unwrap();
        assert_eq!(yellow.0, bottom.x);
        assert_eq!(yellow.1, bottom.y);
    }

    #[test]
    fn inline_blocks_have_shrink_width_and_source_text_order() {
        let doc = parse("<body style='margin:0'>Before <span style='display:inline-block;padding:4px;border:1px solid blue'><div>One</div><div>Two</div></span> After</body>");
        let page = layout(&doc, 500);
        let one = page.runs.iter().find(|run| run.text == "One").unwrap();
        let two = page.runs.iter().find(|run| run.text == "Two").unwrap();
        assert_eq!(one.x, two.x);
        assert!(two.y > one.y);
        let words: Vec<_> = page
            .runs
            .iter()
            .map(|run| (run.text.as_str(), run.separator_before))
            .collect();
        assert_eq!(
            words,
            [("Before", ""), ("One", " "), ("Two", "\n"), ("After", " ")]
        );
        let before = &page.runs[0];
        let after = &page.runs[3];
        assert_eq!(before.y, two.y);
        assert_eq!(after.y, two.y);
        assert!(after.x - before.x < 250);
    }

    #[test]
    fn table_columns_center_form_controls_and_share_row_widths() {
        let doc = parse("<body style='margin:0'><center><table cellpadding=0 cellspacing=0><tr valign=top><td width=25%><td align=center nowrap><input id=search style='width:200px;height:30px'><br><span style='display:inline-block'><span style='display:block'><input type=submit value=Go style='width:50px;height:25px'></span></span> <span style='display:inline-block'><span style='display:block'><input type=submit value=Next style='width:70px;height:25px'></span></span><td align=left width=25%><a href=help>Help</a><tr><td>A<td align=center><a id=second href=second>B</a><td>C</table></center></body>");
        let page = layout(&doc, 700);
        assert_eq!(page.controls.len(), 3);
        let field = &page.controls[0];
        assert_eq!((field.x, field.width), (250, 200));
        assert_eq!(page.control_at(field.x + 2, field.y + 2), Some(field.node));
        let help = page.runs.iter().find(|run| run.text == "Help").unwrap();
        assert_eq!((help.x, help.y), (450, 0));
        assert_eq!(page.link_at(help.x + 1, help.y + 1), Some("help"));
        assert_eq!(page.controls[1].y, page.controls[2].y);
        assert_eq!(page.controls[1].x, 284);
        assert_eq!(page.controls[2].x, 346);
        let a = page.runs.iter().find(|run| run.text == "A").unwrap();
        let b = page.runs.iter().find(|run| run.text == "B").unwrap();
        let c = page.runs.iter().find(|run| run.text == "C").unwrap();
        assert_eq!((a.x, b.x, c.x), (194, 344, 494));
        assert_eq!(a.y, b.y);
        assert_eq!(b.y, c.y);
        assert!(b.y >= page.controls[1].y + page.controls[1].height);
        assert_eq!(page.link_at(b.x + 1, b.y + 1), Some("second"));
        assert_eq!(
            page.anchors
                .iter()
                .find(|(id, _)| id == "second")
                .unwrap()
                .1,
            b.y
        );
    }

    #[test]
    fn table_spacing_padding_nested_boxes_and_vertical_alignment_are_geometric() {
        let doc = parse("<body style='margin:0'><table style='width:220px;border-spacing:4px 6px' cellpadding=3><tbody><tr><td style='background:red;height:50px' valign=top><a id=tall href=tall>A</a></td><td style='background:blue;position:relative;left:7px' valign=bottom><a id=bottom href=bottom>B</a></td></tr><tr><td><table cellpadding=0 cellspacing=0><tr><td><a href=nested>N</a></td><td>Z</td></tr></table></td><td>C</td></tr></tbody></table></body>");
        let page = layout(&doc, 400);
        let a = page.runs.iter().find(|run| run.text == "A").unwrap();
        let b = page.runs.iter().find(|run| run.text == "B").unwrap();
        let n = page.runs.iter().find(|run| run.text == "N").unwrap();
        assert_eq!((a.x, a.y), (7, 9));
        assert!(b.y > a.y);
        assert_eq!(page.link_at(b.x + 1, b.y + 1), Some("bottom"));
        assert_eq!(
            page.anchors
                .iter()
                .find(|(id, _)| id == "bottom")
                .unwrap()
                .1,
            b.y
        );
        assert!(n.y > b.y);
        assert_eq!(page.link_at(n.x + 1, n.y + 1), Some("nested"));
        let red = rectangle(&page, 0xff0000);
        let blue = rectangle(&page, 0x0000ff);
        assert_eq!((red.1, red.3), (6, 56));
        assert_eq!((blue.1, blue.3), (6, 56));
        assert_eq!(b.x, blue.0 + 3);
        assert_eq!(b.y + 24 + 3, blue.1 + blue.3);
    }

    #[test]
    fn table_row_and_group_client_boxes_follow_final_rendered_row_extents() {
        let doc = parse("<body style='margin:0'><table cellpadding=0 style='width:220px;border-spacing:4px 6px'><thead id=head><tr id=heading style='background:#123456'><td style='height:11px'></td></tr></thead><tbody id=body><tr id=first style='background:red'><td style='height:20px;padding:3px;border:2px solid black'></td></tr><tr id=second style='background:blue'><td style='height:40px'></td></tr></tbody><tbody id=hidden style='display:none'><tr id=hidden-row><td style='height:500px'></td></tr></tbody><tbody id=empty></tbody><tfoot id=foot><tr id=last style='background:#654321'><td style='height:17px'></td></tr></tfoot></table></body>");
        let node = |id| {
            (0..doc.nodes.len())
                .find(|&node| doc.attr(node, "id") == Some(id))
                .unwrap()
        };
        for zoom in [1.0, 2.0] {
            let page = build_viewport(&doc, 600, 400, zoom, &HashMap::new(), &HashMap::new());
            let client = |id| page.client_boxes[node(id)].expect("rendered table box");
            for (id, color) in [
                ("heading", 0x123456),
                ("first", 0xff0000),
                ("second", 0x0000ff),
                ("last", 0x654321),
            ] {
                let painted = rectangle(&page, color);
                let row = client(id);
                assert_eq!(
                    (row.width, row.height, row.left, row.top),
                    (painted.2, painted.3, 0, 0)
                );
            }
            let first = rectangle(&page, 0xff0000);
            let second = rectangle(&page, 0x0000ff);
            let body = client("body");
            assert_eq!(
                (body.width, body.height, body.left, body.top),
                (first.2, second.1 + second.3 - first.1, 0, 0)
            );
            assert_eq!(body.height, (76.0 * zoom) as i32);
            assert_eq!(client("head").height, client("heading").height);
            assert_eq!(client("foot").height, client("last").height);
            assert!(page.client_boxes[node("hidden")].is_none());
            assert!(page.client_boxes[node("hidden-row")].is_none());
            assert!(page.client_boxes[node("empty")].is_none());
        }
    }

    #[test]
    fn author_alignment_overrides_legacy_center_and_nowrap_still_collapses_spaces() {
        let doc = parse("<body style='margin:0'><center style='text-align:left'><div style='width:80px;background:red'>X</div></center><div style='width:35px;white-space:nowrap'>One  two\nthree</div></body>");
        let page = layout(&doc, 400);
        assert_eq!(rectangle(&page, 0xff0000).0, 0);
        let words: Vec<_> = page.runs.iter().filter(|run| run.text != "X").collect();
        assert_eq!(
            words
                .iter()
                .map(|run| run.text.as_str())
                .collect::<Vec<_>>(),
            ["One", "two", "three"]
        );
        assert!(words.iter().all(|run| run.y == words[0].y));
        assert_eq!(words[1].x - words[0].x, 48);
    }

    #[test]
    fn atomic_inline_wrapping_uses_the_parent_whitespace_context() {
        for (parent, child, expected) in [
            ("normal", "nowrap", (0, 24)),
            ("nowrap", "normal", (84, 0)),
            ("pre", "normal", (84, 0)),
        ] {
            let doc = parse(&format!("<body style='margin:0;white-space:{parent}'>Before <span style='display:inline-block;width:150px;white-space:{child};background:red'>X</span></body>"));
            let page = layout(&doc, 200);
            let red = rectangle(&page, 0xff0000);
            assert_eq!((red.0, red.1), expected);
        }
    }

    #[test]
    fn link_box_padding_children_overlays_and_fixed_scroll_share_paint_order() {
        let doc = parse("<body style='margin:0'><a href=outer style='display:inline-block;width:160px;height:80px;padding:10px;background:red'><a href=inner style='display:block;width:60px;height:30px;background:blue'>Inner</a><input name=q style='width:100px;height:25px'></a><a href=empty style='display:inline-block;width:50px;height:40px'></a><div style='position:absolute;left:2px;top:2px;width:5px;height:5px;background:black'></div><a href=fixed style='position:fixed;right:5px;bottom:5px;width:60px;height:40px;background:green'>Pin</a></body>");
        let mut page = build_viewport(&doc, 400, 300, 1.0, &HashMap::new(), &HashMap::new());
        assert_eq!(page.link_at(8, 8), Some("outer"));
        assert_eq!(page.link_at(12, 12), Some("inner"));
        let control = &page.controls[0];
        assert_eq!(
            page.control_at(control.x + 2, control.y + 2),
            Some(control.node)
        );
        assert_eq!(page.link_at(control.x + 2, control.y + 2), None);
        assert_eq!(page.link_at(3, 3), None);
        // Empty inline-block alignment uses its bottom margin edge as baseline.
        let empty = page
            .items
            .iter()
            .find_map(|item| match item {
                PaintItem::HitBox {
                    x,
                    y,
                    width: 50,
                    height: 40,
                    ..
                } => Some((*x, *y)),
                _ => None,
            })
            .unwrap();
        assert_eq!(page.link_at(empty.0 + 20, empty.1 + 10), Some("empty"));
        assert_eq!(page.link_at(390, 290), Some("fixed"));
        page.set_scroll(200);
        assert_eq!(page.link_at(390, 490), Some("fixed"));
        page.set_scroll(0);
        assert_eq!(page.link_at(390, 290), Some("fixed"));
        let decorated = parse("<body style='margin:0'><a href=target style='display:inline-block;width:100px;padding:10px;background:red'><span style='padding:5px;background:blue'>X</span></a></body>");
        let decorated = layout(&decorated, 300);
        let blue = rectangle(&decorated, 0x0000ff);
        assert_eq!(decorated.link_at(blue.0 + 1, blue.1 + 1), Some("target"));
    }

    #[test]
    fn relative_offsets_move_geometry_without_changing_normal_flow() {
        let doc = parse("<style>body{margin:0}div{margin:0}</style><body><div style='width:100px;height:40px;background:red;position:relative;left:20px;right:99px;top:10px;bottom:99px'><a id=target href='#target'>Moved</a></div><div style='height:20px;background:blue'>Next</div></body>");
        let page = layout(&doc, 400);
        assert_eq!(rectangle(&page, 0xff0000), (20, 10, 100, 40));
        assert_eq!(rectangle(&page, 0x0000ff), (0, 40, 400, 20));
        let run = page.runs.iter().find(|run| run.text == "Moved").unwrap();
        assert_eq!((run.x, run.y), (20, 10));
        assert_eq!(page.link_at(run.x + 2, run.y + 2), Some("#target"));
        assert_eq!(
            page.anchors
                .iter()
                .find(|(id, _)| id == "target")
                .unwrap()
                .1,
            10
        );
        let source: Vec<_> = page.runs.iter().map(|run| run.text.as_str()).collect();
        assert_eq!(source, ["Moved", "Next"]);
        for offset in [0, 30] {
            let doc = parse(&format!("<body style='margin:0'><span style='background:red;position:relative;left:{offset}px'><span style='position:relative;left:100px;background:blue'><a href=one>A</a></span><a href=two>B</a></span></body>"));
            let page = layout(&doc, 400);
            assert_eq!(rectangle(&page, 0xff0000), (offset, 0, 24, 18));
            assert_eq!(rectangle(&page, 0x0000ff), (offset + 100, 0, 12, 18));
            assert_eq!(page.link_at(offset + 13, 2), Some("two"));
            assert_eq!(page.link_at(offset + 101, 2), Some("one"));
        }
        let doc = parse("<body style='margin:0'><span style='position:relative;top:100px;padding-bottom:40px;border-bottom:2px solid blue;background:red'>A</span></body>");
        let page = layout(&doc, 400);
        let background = rectangle(&page, 0xff0000);
        assert!(page.height >= background.1 + background.3 + 16);
    }

    #[test]
    fn absolute_padding_containing_block_stretch_percentages_and_margins() {
        let doc = parse("<style>body{margin:0}</style><body><div style='position:relative;margin:20px 0 0 30px;width:200px;height:120px;padding:10px;border:4px solid black'><div style='position:absolute;inset:10px 20px;padding:5px;border:2px solid black;background:red'></div><div style='position:absolute;right:8px;bottom:6px;width:50px;height:30px;margin:2px;background:blue'></div><div style='position:absolute;left:25%;top:50%;width:10px;height:10px;background:green'></div></div><div style='height:10px;background:purple'></div></body>");
        let page = layout(&doc, 500);
        assert_eq!(rectangle(&page, 0xff0000), (54, 34, 180, 120));
        assert_eq!(rectangle(&page, 0x0000ff), (194, 126, 50, 30));
        assert_eq!(rectangle(&page, 0x008000), (89, 94, 10, 10));
        assert_eq!(rectangle(&page, 0x800080), (0, 168, 500, 10));
        let doc = parse("<body style='margin:0'><div style='position:relative;width:200px;height:100px'><div style='position:absolute;inset:0;width:260px;height:160px;margin:auto;background:red'></div></div></body>");
        assert_eq!(rectangle(&layout(&doc, 400), 0xff0000), (0, -30, 260, 160));
    }

    #[test]
    fn positioned_nodes_do_not_consume_flex_space_and_keep_source_order() {
        let doc = parse("<style>body{margin:0}</style><body><div style='position:relative;width:300px;height:200px'><div style='display:flex;gap:10px;height:50px'><div style='flex:1;min-width:0;background:red'>Before</div><div style='position:absolute;right:5px;top:0;width:90px'>Middle</div><div style='flex:1;min-width:0;background:blue'>After</div></div><div style='position:absolute;left:30px;top:60px;width:100px;height:60px;background:green'><div style='position:absolute;left:50%;top:50%;width:20px;height:10px;background:purple'></div></div></div></body>");
        let page = layout(&doc, 400);
        assert_eq!(rectangle(&page, 0xff0000), (0, 0, 145, 50));
        assert_eq!(rectangle(&page, 0x0000ff), (155, 0, 145, 50));
        assert_eq!(rectangle(&page, 0x008000), (30, 60, 100, 60));
        assert_eq!(rectangle(&page, 0x800080), (80, 90, 20, 10));
        let middle = page.runs.iter().find(|run| run.text == "Middle").unwrap();
        assert_eq!((middle.x, middle.y), (205, 0));
        assert_eq!(
            page.runs
                .iter()
                .map(|run| run.text.as_str())
                .collect::<Vec<_>>(),
            ["Before", "Middle", "After"]
        );
    }

    #[test]
    fn fixed_viewport_geometry_scrolls_exactly_without_growing_document() {
        let doc = parse("<style>body{margin:0}</style><body><div style='position:relative;left:50px;top:20px;height:2000px'><div style='position:fixed;right:10px;bottom:20px;width:100px;height:80px;background:blue'><a id=pin href='#pin'>Fixed</a><input name=q style='width:80px'><span style='background:yellow'>Label</span><div id=nested style='position:absolute;right:0;bottom:0;width:10px;height:10px;background:red'></div></div><div style='position:fixed;top:9000px;width:1px;height:100px;background:green'></div></div></body>");
        let mut page = build_viewport(&doc, 500, 300, 1.0, &HashMap::new(), &HashMap::new());
        assert_eq!(rectangle(&page, 0x0000ff), (390, 200, 100, 80));
        assert_eq!(rectangle(&page, 0xff0000), (480, 270, 10, 10));
        assert_eq!(page.height, 2036);
        let run = page
            .runs
            .iter()
            .position(|run| run.text == "Fixed")
            .unwrap();
        let node = page.controls[0].node;
        assert!(
            page.run_is_fixed(run)
                && page.control_is_fixed(node)
                && page.anchor_is_fixed("pin")
                && page.anchor_is_fixed("nested")
        );
        let initial = (page.runs[run].y, page.controls[0].y, page.anchors.clone());
        let decoration = rectangle(&page, 0xffff00);
        page.set_scroll(700);
        assert_eq!(rectangle(&page, 0x0000ff), (390, 900, 100, 80));
        assert_eq!(rectangle(&page, 0xff0000), (480, 970, 10, 10));
        assert_eq!(page.runs[run].y, initial.0 + 700);
        assert_eq!(page.controls[0].y, initial.1 + 700);
        assert_eq!(rectangle(&page, 0xffff00).1, decoration.1 + 700);
        assert_eq!(
            page.link_at(page.runs[run].x + 2, page.runs[run].y + 2),
            Some("#pin")
        );
        assert_eq!(
            page.control_at(page.controls[0].x + 2, page.controls[0].y + 2),
            Some(node)
        );
        for scroll in [1_000_000, 1_000_000, 21, 0] {
            page.set_scroll(scroll);
        }
        assert_eq!(
            (page.runs[run].y, page.controls[0].y, page.anchors.clone()),
            initial
        );
        assert_eq!(rectangle(&page, 0x0000ff), (390, 200, 100, 80));
        let zoomed = build_viewport(&doc, 600, 450, 1.5, &HashMap::new(), &HashMap::new());
        assert_eq!(rectangle(&page, 0xffff00), decoration);
        assert_eq!(rectangle(&zoomed, 0x0000ff), (435, 300, 150, 120));
    }

    #[test]
    fn stage_observations_distinguish_viewport_candidates_from_occluded_hits() {
        let doc = parse("<body style='margin:0'><a href=visible>Visible</a><a href=empty></a><a href=hidden style='display:none'>Hidden</a><div style='height:0;overflow:hidden'><a href=clipped>Clipped</a></div><a href=fixed style='position:fixed;left:150px;top:20px'>Fixed</a><div style='height:1200px'></div><a href=below>Below</a><div style='position:absolute;left:0;top:0;width:120px;height:60px;background:red'></div></body>");
        let mut page = build_viewport(&doc, 400, 200, 1.0, &HashMap::new(), &HashMap::new());
        let fixed = page.runs.iter().find(|r| r.text == "Fixed").unwrap();
        assert_eq!(page.link_at(fixed.x + 1, fixed.y + 1), Some("fixed"));
        assert_eq!(page.link_at(1, 1), None); // Visible link is genuinely occluded.
        assert_eq!(page.viewport_link_regions(400, 0, 200), 3); // Fixed link has box and text regions.
        assert!(page.link_targets().count() > page.viewport_link_regions(400, 0, 200));
        page.set_scroll(500);
        assert_eq!(page.viewport_link_regions(400, 500, 200), 2);
        assert_eq!(page.viewport_link_regions(0, 500, 200), 0);
    }

    #[test]
    fn positioned_paint_order_blocks_click_through_opaque_backgrounds() {
        let doc = parse("<style>body{margin:0}</style><body><a href=behind>UnderlyingLink</a><div style='position:absolute;left:0;top:0;width:200px;height:60px;background:red'><a href=front>Top</a></div></body>");
        let page = layout(&doc, 400);
        assert_eq!(page.link_at(2, 2), Some("front"));
        assert_eq!(page.link_at(80, 2), None);
        assert_eq!(page.control_at(80, 2), None);
        let red = page
            .items
            .iter()
            .position(|item| {
                matches!(
                    item,
                    PaintItem::Rect {
                        color: 0xff0000,
                        ..
                    }
                )
            })
            .unwrap();
        assert!(page.paint_is_positioned(red));
        assert!(!page.run_is_fixed(0));
        let nested = parse("<style>body{margin:0}</style><body><div style='position:absolute;left:0;top:0;width:100px;height:60px;background:red'><a style='position:absolute;left:0;top:0' href=early>Early</a></div><a style='position:absolute;left:0;top:0' href=late>Later</a></body>");
        assert_eq!(layout(&nested, 400).link_at(2, 2), Some("late"));
        let relative = parse("<style>body{margin:0}</style><body><div style='position:relative;top:20px;height:20px;background:red'><a href=relative>Relative</a></div><div style='height:20px;background:blue'>Normal</div></body>");
        assert_eq!(layout(&relative, 400).link_at(2, 22), Some("relative"));
    }

    #[test]
    fn positioned_replaced_images_preserve_dimensions_decoration_and_link_hits() {
        let doc = parse("<style>body{margin:0}</style><body><a href=image><img width=60 height=40 style='position:absolute;right:0;bottom:0;padding:5px;border:2px solid red'></a></body>");
        let node = doc
            .nodes
            .iter()
            .position(|node| matches!(&node.kind, NodeKind::Element { tag, .. } if tag == "img"))
            .unwrap();
        let page = build_viewport(
            &doc,
            200,
            100,
            1.0,
            &HashMap::new(),
            &HashMap::from([(node, (120, 80))]),
        );
        let bounds = page
            .items
            .iter()
            .find_map(|item| match item {
                PaintItem::Image {
                    x,
                    y,
                    width,
                    height,
                    ..
                } => Some((*x, *y, *width, *height)),
                _ => None,
            })
            .unwrap();
        assert_eq!(bounds, (133, 53, 60, 40));
        assert_eq!(page.link_at(134, 54), Some("image"));
        assert_eq!(page.link_at(127, 47), None);
        assert_eq!(page.height, 116);
    }

    #[test]
    fn relative_percent_height_requires_a_definite_container_and_offsets_are_bounded() {
        for (height, expected_y) in [("height:100px", 10), ("height:50%", 0), ("", 0)] {
            let doc = parse(&format!("<style>body{{margin:0}}</style><body><div style='width:200px;{height}'><div style='position:relative;left:10%;top:10%;width:20px;height:20px;background:red'></div></div></body>"));
            let page = layout(&doc, 400);
            assert_eq!(rectangle(&page, 0xff0000), (20, expected_y, 20, 20));
        }
        let mut source = String::from("<style>body{margin:0}.box{position:absolute;left:1e30px;top:-1e30px;width:1px;height:1px;background:red}</style><body>");
        source.push_str(&"<div class=box></div>".repeat(MAX_POSITIONED + 5));
        source.push_str("<p>Still here</p></body>");
        let page = layout(&parse(&source), 400);
        assert_eq!(
            page.items
                .iter()
                .filter(|item| matches!(
                    item,
                    PaintItem::Rect {
                        color: 0xff0000,
                        ..
                    }
                ))
                .count(),
            MAX_POSITIONED
        );
        assert!(page.runs.iter().any(|run| run.text == "Still"));
        assert!(page.height <= MAX_HEIGHT);
    }

    #[test]
    fn flex_growth_shrink_and_min_max_freezing_use_available_space() {
        for (first, second, justify, expected) in [
            (
                "flex:1 1 0px",
                "flex:2 1 0px",
                "flex-start",
                (0, 100, 100, 200),
            ),
            (
                "flex:1 1 0px;max-width:80px",
                "flex:2 1 0px",
                "flex-start",
                (0, 80, 80, 220),
            ),
            (
                "flex:0 1 200px;min-width:180px",
                "flex:0 1 200px",
                "flex-start",
                (0, 180, 180, 120),
            ),
            (
                "flex:.25 1 0px",
                "flex:.25 1 0px",
                "center",
                (75, 75, 150, 75),
            ),
        ] {
            let doc = parse(&format!("<div style='display:flex;width:300px;justify-content:{justify}'><div style='background:red;min-width:0;{first}'>A</div><div style='background:blue;min-width:0;{second}'>B</div></div>"));
            let page = layout(&doc, 400);
            let red = rectangle(&page, 0xff0000);
            let blue = rectangle(&page, 0x0000ff);
            assert_eq!(
                (red.0, red.2, blue.0, blue.2),
                expected,
                "{first} / {second}"
            );
            assert_eq!(red.1, blue.1);
        }
    }

    #[test]
    fn flex_wrap_gaps_border_box_and_percentage_basis_are_geometric() {
        let doc = parse("<div style='display:flex;flex-wrap:wrap;gap:20px 10px;width:250px'><div style='flex:0 0 120px;box-sizing:border-box;padding:10px;border:2px solid black;height:60px;background:red'>A</div><div style='flex:0 0 120px;height:60px;background:blue'>B</div><div style='flex:0 0 120px;height:60px;background:green'>C</div></div>");
        let page = layout(&doc, 500);
        assert_eq!(rectangle(&page, 0xff0000), (0, 0, 120, 60));
        assert_eq!(rectangle(&page, 0x0000ff), (130, 0, 120, 60));
        assert_eq!(rectangle(&page, 0x008000), (0, 80, 120, 60));
        let doc = parse("<div style='display:flex;width:300px'><div style='flex:0 0 25%;background:red'>A</div><div style='flex:0 0 75%;background:blue'>B</div></div>");
        let page = layout(&doc, 500);
        assert_eq!(
            (rectangle(&page, 0xff0000).2, rectangle(&page, 0x0000ff).2),
            (75, 225)
        );
    }

    #[test]
    fn flex_fractional_main_sizes_fit_before_pixel_rounding_and_keep_real_overflow() {
        for sizing in ["width:50%;flex:0 0 auto", "flex:0 0 50%", "width:50%;flex:none;min-width:50%;max-width:50%"] {
            let doc=parse(&format!("<div style='display:flex;flex-wrap:wrap;width:101px'><div style='min-width:0;{sizing};box-sizing:border-box;padding:4px;border:2px solid black;height:40px;background:red'>A</div><div style='min-width:0;{sizing};box-sizing:border-box;padding:4px;border:2px solid black;height:40px;background:blue'>B</div></div>"));
            let page=layout(&doc,500);let first=rectangle(&page,0xff0000);let second=rectangle(&page,0x0000ff);
            assert_eq!((first.0,first.1,first.2,second.0,second.1,second.2),(0,0,51,51,0,50),"{sizing}");
        }
        for sizing in ["box-sizing:content-box;width:50%;min-width:0", "box-sizing:border-box;width:50%;min-width:51px"] {
            let doc=parse(&format!("<div style='display:flex;flex-wrap:wrap;width:101px'><div style='flex:none;{sizing};padding:4px;border:2px solid black;height:40px;background:red'>A</div><div style='flex:none;{sizing};padding:4px;border:2px solid black;height:40px;background:blue'>B</div></div>"));
            let page=layout(&doc,500);assert!(rectangle(&page,0x0000ff).1>=rectangle(&page,0xff0000).3,"Real outer-size overflow must still wrap: {sizing}");
        }
    }

    #[test]
    fn flex_percentage_line_fit_uses_physical_reference_at_fractional_zoom() {
        let doc=parse("<div style='display:flex;flex-wrap:wrap;width:100%'><div style='width:50%;flex:none;min-width:0;box-sizing:border-box;padding:4px;border:2px solid black;height:40px;background:red'>A</div><div style='width:50%;flex:none;min-width:0;box-sizing:border-box;padding:4px;border:2px solid black;height:40px;background:blue'>B</div></div>");
        for (width,zoom) in [(64,1.5),(60,1.1),(101,1.25)] {
            let page=build(&doc,width,zoom,&HashMap::new(),&HashMap::new());
            let first=rectangle(&page,0xff0000);let second=rectangle(&page,0x0000ff);
            assert_eq!(first.1,second.1,"Two halves must fit before pixel allocation at zoom{zoom}: {first:?}, {second:?}");
            assert_eq!(first.2+second.2,width);
            assert_eq!(first.0+first.2,second.0);
        }
    }

    #[test]
    fn flex_nested_stretch_moves_text_links_and_anchors_without_repainting() {
        for direction in ["row", "column"] {
            let doc = parse(&format!("<div style='display:flex;width:300px;min-height:120px'><div style='display:flex;flex-direction:{direction};justify-content:center;align-items:center;flex:1;min-width:0;background:red'><a id=mark href=destination>A</a></div><div style='flex:1;min-width:0;background:blue'>B</div></div>"));
            let page = layout(&doc, 500);
            assert_eq!(rectangle(&page, 0xff0000), (0, 0, 150, 120));
            assert_eq!(rectangle(&page, 0x0000ff), (150, 0, 150, 120));
            let first = page.runs.iter().find(|run| run.text == "A").unwrap();
            assert_eq!((first.x, first.y), (69, 48), "{direction}");
            assert_eq!(
                page.anchors.iter().find(|(id, _)| id == "mark").unwrap().1,
                48
            );
            assert_eq!(page.link_at(first.x + 1, first.y + 1), Some("destination"));
            assert_eq!(
                page.runs
                    .iter()
                    .map(|run| run.text.as_str())
                    .collect::<Vec<_>>(),
                ["A", "B"]
            );
        }
    }

    #[test]
    fn flex_column_growth_reverse_and_logical_start_keep_source_order() {
        let doc = parse("<div style='display:flex;flex-direction:column;width:200px;height:200px;gap:10px'><div style='flex:1;min-height:0;background:red'>A</div><div style='flex:3;min-height:0;background:blue'>B</div></div>");
        let page = layout(&doc, 400);
        assert_eq!(rectangle(&page, 0xff0000), (0, 0, 200, 48));
        assert_eq!(rectangle(&page, 0x0000ff), (0, 58, 200, 142));
        let doc = parse("<div style='display:flex;flex-direction:column;width:200px;height:100px'><div style='height:80px;background:red'>A</div><div style='height:80px;background:blue'>B</div></div>");
        let page = layout(&doc, 400);
        assert_eq!(rectangle(&page, 0xff0000), (0, 0, 200, 50));
        assert_eq!(rectangle(&page, 0x0000ff), (0, 50, 200, 50));
        for (justify, x) in [("flex-start", 160), ("start", 40)] {
            let doc = parse(&format!("<div style='display:flex;flex-direction:row-reverse;width:200px;justify-content:{justify}'><a href=one style='flex:none;width:40px;background:red'>A</a><a href=two style='flex:none;width:40px;background:blue'>B</a></div>"));
            let page = layout(&doc, 400);
            assert_eq!(rectangle(&page, 0xff0000).0, x);
            assert_eq!(rectangle(&page, 0x0000ff).0, x - 40);
            assert_eq!(
                page.runs
                    .iter()
                    .map(|run| run.text.as_str())
                    .collect::<Vec<_>>(),
                ["A", "B"]
            );
            assert_eq!(page.link_at(x + 1, 1), Some("one"));
        }
    }

    #[test]
    fn flex_intrinsics_ignore_whitespace_boxes_and_text_node_margins() {
        let doc = parse("<div style='display:flex;width:200px'><div style='display:flex;flex:none;gap:10px;margin:20px;background:red'>A\n <input type=hidden name=ignored><span>B</span>\n </div><div style='flex:none;width:40px;background:blue'>C</div></div>");
        let page = layout(&doc, 400);
        assert_eq!(rectangle(&page, 0xff0000).2, 34);
        assert_eq!(rectangle(&page, 0xff0000).0, 20);
        assert_eq!(rectangle(&page, 0x0000ff).0, 74);
        let a = page.runs.iter().find(|r| r.text == "A").unwrap();
        let b = page.runs.iter().find(|r| r.text == "B").unwrap();
        assert_eq!((a.x, b.x), (20, 42));
    }

    #[test]
    fn flex_auto_margins_and_wrapped_line_alignment_consume_spare_space() {
        let doc = parse("<div style='display:flex;width:200px;height:100px'><div style='flex:none;width:40px;height:20px;margin-right:auto;margin-top:auto;margin-bottom:auto;background:red'>A</div><div style='flex:none;width:40px;background:blue'>B</div></div>");
        let page = layout(&doc, 400);
        assert_eq!(rectangle(&page, 0xff0000), (0, 40, 40, 20));
        assert_eq!(rectangle(&page, 0x0000ff), (160, 0, 40, 100));
        let doc = parse("<div style='display:flex;flex-wrap:wrap;width:120px;height:200px;align-content:normal;align-items:center'><div style='flex:none;width:120px;height:20px;background:red'>A</div><div style='flex:none;width:120px;height:20px;background:blue'>B</div></div>");
        let page = layout(&doc, 400);
        assert_eq!(rectangle(&page, 0xff0000).1, 40);
        assert_eq!(rectangle(&page, 0x0000ff).1, 140);
        let doc = parse("<div style='display:flex;flex-direction:column;width:200px'><div style='margin-left:auto;background:red'>A</div></div>");
        let page = layout(&doc, 400);
        assert_eq!(rectangle(&page, 0xff0000), (188, 0, 12, 24));
    }

    #[test]
    fn flex_controls_images_and_zoom_use_final_hit_regions() {
        let doc = parse("<div style='display:flex;width:300px;height:100px;align-items:flex-end;gap:10px'><input style='width:80px;height:30px'><a href=picture><img id=pic width=60 height=40></a></div>");
        for zoom in [1.0, 1.5] {
            let page = build(&doc, 800, zoom, &HashMap::new(), &HashMap::new());
            let control = &page.controls[0];
            assert_eq!(
                (control.x, control.y, control.width, control.height),
                (
                    0,
                    (70.0 * zoom) as i32,
                    (80.0 * zoom) as i32,
                    (30.0 * zoom) as i32
                )
            );
            assert_eq!(
                page.control_at(control.x + 1, control.y + 1),
                Some(control.node)
            );
            let image = page
                .items
                .iter()
                .find_map(|item| match item {
                    PaintItem::Image {
                        x,
                        y,
                        width,
                        height,
                        ..
                    } => Some((*x, *y, *width, *height)),
                    _ => None,
                })
                .unwrap();
            assert_eq!(
                image,
                (
                    (90.0 * zoom) as i32,
                    (60.0 * zoom) as i32,
                    (60.0 * zoom) as i32,
                    (40.0 * zoom) as i32
                )
            );
            assert_eq!(page.link_at(image.0 + 1, image.1 + 1), Some("picture"));
        }
        for direction in ["row", "column"] {
            for (css, expected_height) in [("", 40), ("height:70px", 70)] {
                let doc = parse(&format!("<div style='display:flex;flex-direction:{direction};width:300px;height:100px'><img width=60 height=40 style='{css}'></div>"));
                let page = layout(&doc, 400);
                let image = page
                    .items
                    .iter()
                    .find_map(|item| match item {
                        PaintItem::Image { width, height, .. } => Some((*width, *height)),
                        _ => None,
                    })
                    .unwrap();
                assert_eq!(image, (60, expected_height), "{direction} / {css}");
            }
        }
    }

    #[test]
    fn tall_normal_flow_is_not_limited_by_the_horizontal_width_bound() {
        let doc = parse(&format!(
            "<pre style='margin:0'>{}</pre><p style='margin:0'>After</p>",
            "Line\n".repeat(2_000)
        ));
        let page = layout(&doc, 400);
        let after = page.runs.iter().find(|run| run.text == "After").unwrap();
        assert!(after.y >= 48_000);
        assert!(page.height > after.y && page.height <= MAX_HEIGHT);
    }

    #[test]
    fn narrower_page_wraps_and_every_run_stays_in_column() {
        let doc = parse("<body><h1>A heading</h1><p>These words wrap onto several lines when the viewport becomes narrow.</p></body>");
        let wide = layout(&doc, 900);
        let narrow = layout(&doc, 300);
        assert!(narrow.height > wide.height);
        for run in &narrow.runs {
            assert!(run.x >= 8 && run.x + run.width() <= 292);
        }
    }

    #[test]
    fn hidden_content_does_not_paint_and_link_matches_its_text() {
        let doc = parse("<head><title>Secret title</title><style>.hidden{display:none}</style></head><p class=hidden>Hidden</p><p><a href='#end'>Click here</a></p><h2 id='end'>End</h2>");
        let page = layout(&doc, 640);
        assert!(!page
            .runs
            .iter()
            .any(|run| matches!(run.text.as_str(), "Secret" | "Hidden")));
        let run = page.runs.iter().find(|run| run.text == "Click").unwrap();
        assert_eq!(page.link_at(run.x + 1, run.y + 1), Some("#end"));
        assert!(page.anchors.iter().any(|(id, _)| id == "end"));
    }

    #[test]
    fn long_link_target_is_stored_once_for_many_words() {
        let target = "x".repeat(50_000);
        let doc = parse(&format!(
            "<a href='{}'>{}</a>",
            target,
            "word ".repeat(2000)
        ));
        let page = layout(&doc, 640);
        assert_eq!(page.targets.len(), 1);
        assert_eq!(page.runs.len(), 2000);
        assert!(page.runs.iter().all(|run| run.link == Some(0)));
    }

    #[test]
    fn centered_boxes_and_nested_backgrounds_paint_in_order() {
        let doc = parse("<div style='width:200px;margin:0 auto;padding:10px;background:red;border:2px solid black'><p style='margin:0;background:blue;color:white'>Hello <span style='background:yellow;color:black'>there</span></p></div>");
        let page = layout(&doc, 600);
        let first = page.runs.iter().find(|run| run.text == "Hello").unwrap();
        assert_eq!(first.x, 200);
        let red = page
            .items
            .iter()
            .position(|item| {
                matches!(
                    item,
                    PaintItem::Rect {
                        color: 0xff0000,
                        ..
                    }
                )
            })
            .unwrap();
        let blue = page
            .items
            .iter()
            .position(|item| {
                matches!(
                    item,
                    PaintItem::Rect {
                        color: 0x0000ff,
                        ..
                    }
                )
            })
            .unwrap();
        let yellow = page
            .items
            .iter()
            .position(|item| {
                matches!(
                    item,
                    PaintItem::Rect {
                        color: 0xffff00,
                        ..
                    }
                )
            })
            .unwrap();
        let text = page
            .items
            .iter()
            .position(
                |item| matches!(item, PaintItem::Text(index) if page.runs[*index].text == "there"),
            )
            .unwrap();
        assert!(red < blue && blue < yellow && yellow < text);
    }

    #[test]
    fn image_links_follow_resized_images_and_text_align() {
        let doc = parse("<div style='text-align:center'><a href=target><img id=picture style='width:100px'></a></div>");
        let node = doc
            .nodes
            .iter()
            .position(|node| matches!(&node.kind, NodeKind::Element { tag, .. } if tag == "img"))
            .unwrap();
        let page = build(
            &doc,
            400,
            1.0,
            &HashMap::new(),
            &HashMap::from([(node, (200, 100))]),
        );
        assert!(page.items.iter().any(|item| matches!(
            item,
            PaintItem::Image {
                x: 150,
                width: 100,
                height: 50,
                ..
            }
        )));
        assert_eq!(page.link_at(151, 1), Some("target"));
    }

    #[test]
    fn preserved_whitespace_lists_and_extreme_values_are_bounded() {
        let doc = parse("<pre>A  B\nC\tD</pre><ol start=3><li>First</li><li>Second</li></ol><p style='font-size:999999px;width:999999px;padding:999999px'>End</p>");
        let page = build(&doc, 400, f32::NAN, &HashMap::new(), &HashMap::new());
        assert!(page.runs.iter().any(|run| run.text == "A  B"));
        assert!(page.runs.iter().any(|run| run.text == "3."));
        assert!(page.runs.iter().any(|run| run.text == "4."));
        assert!(page.runs.iter().all(|run| (1..=16).contains(&run.scale)));
        assert!(page.height <= MAX_HEIGHT);
    }

    #[test]
    fn text_stream_preserves_real_separators_across_wraps_and_font_changes() {
        let doc = parse("<div style='width:90px'>Supercalifragilistic <span style='font-size:32px'>mi</span><b>x</b>ed <em>style</em>.</div><p>next<br>line</p><pre>A  B\nC\tD</pre>");
        let page = layout(&doc, 300);
        let mut text = String::new();
        for run in &page.runs {
            text.push_str(run.separator_before);
            text.push_str(&run.text);
        }
        assert_eq!(
            text,
            "Supercalifragilistic mixed style.\nnext\nline\nA  B\nC   D"
        );
        assert!(page
            .runs
            .windows(2)
            .any(|runs| runs[0].y != runs[1].y && runs[1].separator_before.is_empty()));
    }
}
