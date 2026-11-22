//! A bounded CSS subset for block, inline and basic flex layout.
use std::collections::HashMap;
use std::time::Instant;
#[path="css_stylesheet_cache.rs"]
mod stylesheet_cache;
#[path="css_rule_candidates.rs"]
mod rule_candidates;
pub(crate) use stylesheet_cache::StylesheetCache;

#[path="css_media.rs"]
mod media;
pub use media::{Hover, MediaEnvironment};

pub(crate) fn serialize_media_query_list(source: &str) -> String {
    media::serialize_list(source)
}

pub(crate) fn media_query_list_matches(query: &str, environment: Option<MediaEnvironment>) -> bool {
    query.is_empty() || media::matches(query, environment)
}
#[path="css_sizing_math.rs"]
mod sizing_math;
pub use sizing_math::CalculatedLength;
#[path="css_custom_properties.rs"]
mod custom_properties;
#[path="css_svg_geometry.rs"]
mod svg_geometry;
#[path="css_svg_paint.rs"]
mod svg_paint;
#[path="css_computed_style.rs"]
mod computed_style;
pub(crate) use computed_style::{CascadeOutput, ComputedProperties, ComputedStyleUnavailable, CssBudget};
use computed_style::check_css_deadline;
pub(crate) fn computed_custom_property_name(name: &str) -> bool { custom_properties::custom_name(name) }
pub(crate) fn inline_custom_value(value: &str) -> bool { value.len() <= 2048 && custom_properties::valid_syntax(value) }
#[path="css_overflow.rs"]
mod overflow;
pub use overflow::{Overflow, OverflowValue};
#[path="css_position.rs"]
mod position;
#[path="css_touch_action.rs"]
mod touch_action;
pub(crate) use touch_action::{TouchAction, specified as specified_touch_action};
#[path="css_opacity.rs"]
mod opacity;
pub(crate) use opacity::{serialize as serialize_opacity, specified as specified_opacity, Specified as SpecifiedOpacity};
#[path="css_box_spacing.rs"]
mod box_spacing;
#[path="css_radius.rs"]
mod radius;
#[path="css_background_clip.rs"]
mod background_clip;
pub use background_clip::BackgroundClip;
#[path="css_line_height.rs"]
mod line_height;
#[path = "css_grid_alignment.rs"]
mod grid_alignment;
pub use grid_alignment::Justify;
#[cfg(test)]
#[path="css_height_variable_tests.rs"]
mod height_variable_tests;
#[cfg(test)]
#[path="css_root_selector_tests.rs"]
mod root_selector_tests;
#[path="css_translate.rs"]
mod translate;
#[path="css_easing.rs"]
mod easing;
pub(crate) use easing::{normalize as normalize_easing, specified as specified_easing, source_cost as easing_source_cost};
pub(crate) use easing::valid as inline_easing_value;
pub(crate) use easing::{priority as easing_priority, trim as easing_trim};
pub use translate::Transform;
pub(crate) use translate::{normalize_transform, TransformSyntax};
pub(crate) use translate::source_cost as transform_source_cost;

use crate::html::{Document, NodeKind};

// Current browser-owned initial font basis. Media em units use this initial
// value, independently of author font declarations and physical display zoom.
const INITIAL_FONT_SIZE: f32 = 16.0;
const INITIAL_COLOR:u32=0x26364a;

const MAX_DECLARATIONS: usize = 4096;
const MAX_GROUPED_BOX_BYTES: usize = 2048;
const MAX_SELECTOR_BYTES: usize = 512;
const MAX_SELECTOR_NESTING: usize = 8;
const MAX_MATCH_STEPS: usize = 128_000_000;
const STYLESHEET_DOCUMENT_BYTES: usize = 64 * 1024 * 1024;
const SOURCE_WORK_CHUNK: usize = 64;
const SOURCE_WORK_PER_CHUNK: usize = 512;
pub(crate) const FULL_STYLESHEET_SOURCE_WORK: usize =
    STYLESHEET_DOCUMENT_BYTES / SOURCE_WORK_CHUNK * SOURCE_WORK_PER_CHUNK;

/// Host policy, shared by resource decoding, retention and the complete parser.
/// These are allowances, never sizes used for maximum-sized preallocation.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct StylesheetLimits { pub sheet_bytes:usize, pub document_bytes:usize, pub rules:usize }
impl Default for StylesheetLimits {
    fn default()->Self {Self{sheet_bytes:8*1024*1024,document_bytes:STYLESHEET_DOCUMENT_BYTES,rules:65_536}}
}
#[derive(Clone,Copy,Debug,Default,PartialEq,Eq)]
pub struct StylesheetUsage {pub bytes:usize,pub rules:usize}
impl StylesheetLimits {
    pub fn accept(self,source:&str,usage:StylesheetUsage)->Result<StylesheetUsage,String> {
        let bytes=usage.bytes.checked_add(source.len()).ok_or("Stylesheet byte accounting overflow.")?;
        if source.len()>self.sheet_bytes || bytes>self.document_bytes {
            return Err(format!("Complete stylesheet refused: {} decoded UTF-8 bytes; allowances {} per sheet / {} per document.",source.len(),self.sheet_bytes,self.document_bytes));
        }
        let remaining=self.rules.checked_sub(usage.rules).ok_or("Stylesheet rule accounting overflow.")?;
        let mut declarations_exhausted=false;
        let count=media::walk(source,remaining,None,|_,body,_| {
            // Admission counts declarations before executing the additional
            // computed-property grammars with a caller's reserved work.
            if parse_declarations_inner(body,MAX_DECLARATIONS+1,false,None).expect("unmetered admission").len()>MAX_DECLARATIONS {declarations_exhausted=true;return false;}
            true
        })?;
        if declarations_exhausted {return Err(format!("Complete stylesheet refused: per-block declaration allowance {MAX_DECLARATIONS} reached."));}
        let rules=usage.rules.checked_add(count).ok_or("Stylesheet rule accounting overflow.")?;
        Ok(StylesheetUsage{bytes,rules})
    }
}

/// The same ordered, whole-sheet admission is used for rendering and reports.
/// A refusal never supplies a prefix to the parser or consumes the next sheet's
/// allowance. The DOM retains the author's complete text.
enum SheetCandidate<'a> {
    Source(usize, std::borrow::Cow<'a, str>),
    Error(String),
}

// One source/scope traversal for admission, fresh parsing and cache validation.
// Keep errors in document order, including refused inline source allocation.
fn stylesheet_sources_with<'a>(doc:&'a Document,external:&'a HashMap<usize,String>,replacement:Option<(usize,&'a str)>,mut meter:Option<&mut CssBudget<'_>>) -> Result<Vec<SheetCandidate<'a>>,String> {
    if let Some(meter)=meter.as_deref_mut() {
        meter.copies(doc.nodes.len().saturating_mul(std::mem::size_of::<bool>()+2*std::mem::size_of::<usize>()+2*std::mem::size_of::<SheetCandidate<'_>>()))?;
    }
    let mut sources=Vec::new();
    let mut seen=vec![false;doc.nodes.len()];let mut stack=if doc.nodes.is_empty(){Vec::new()}else{vec![0]};
    while let Some(node)=stack.pop() {
        if let Some(meter)=meter.as_deref_mut() {meter.work(3)?;}
        if node>=doc.nodes.len() || seen[node] {continue;}seen[node]=true;
        let entry=&doc.nodes[node];
        let tag=match &entry.kind {NodeKind::Element{tag,..}=>tag.as_str(),_=>""};
        if matches!(tag,"template"|"iframe"|"object") || tag=="noscript"&&doc.scripting_enabled {continue;}
        if let Some(meter)=meter.as_deref_mut() {meter.work(entry.children.len())?;}
        stack.extend(entry.children.iter().rev().copied().filter(|&child|child!=0));
        if !matches!(tag,"style"|"link") {continue;}
        if tag == "style" && doc.withheld_inline_styles.contains(&node) {continue;}
        if let Some(meter)=meter.as_deref_mut() {
            if let NodeKind::Element{attrs,..}=&entry.kind {
                meter.work(attrs.len().saturating_mul(2))?;
                for attr in attrs {
                    meter.work(attr.name.bytes().max(1).div_ceil(64))?;
                    if attr.name.html_is("media") {meter.work(attr.value.len().max(1).div_ceil(64).saturating_mul(4))?;}
                }
            }
        }
        if !media_supported(doc.attr(node,"media").unwrap_or("")) {continue;}
        let source=if tag=="link" {
            let source=if let Some((_,source))=replacement.filter(|(owner,_)|*owner==node) {source} else {let Some(source)=external.get(&node) else {continue;};source.as_str()};
            std::borrow::Cow::Borrowed(source)
        } else {
            if let Some(meter)=meter.as_deref_mut() {meter.work(entry.children.len().saturating_mul(2))?;}
            let mut length=Some(0usize);
            for &child in &entry.children {if let Some(crate::html::Node{kind:NodeKind::Text(text) | NodeKind::Cdata(text),..})=doc.nodes.get(child) {length=length.and_then(|n|n.checked_add(text.len()));}}
            let Some(length)=length.filter(|&n|n<=doc.stylesheet_limits.sheet_bytes) else {sources.push(SheetCandidate::Error(format!("CSS node {node}: complete inline stylesheet exceeds its byte allowance.")));continue;};
            if let Some(meter)=meter.as_deref_mut() {meter.work(length.max(1).div_ceil(64).saturating_mul(2))?;meter.copies(length)?;}
            let mut source=String::new();
            if source.try_reserve(length).is_err() {sources.push(SheetCandidate::Error(format!("CSS node {node}: stylesheet allocation failed.")));continue;}
            for &child in &entry.children {if let Some(crate::html::Node{kind:NodeKind::Text(text) | NodeKind::Cdata(text),..})=doc.nodes.get(child) {source.push_str(text);}}
            std::borrow::Cow::Owned(source)
        };
        sources.push(SheetCandidate::Source(node,source));
    }
    Ok(sources)
}

fn admit_stylesheets<'a>(doc:&Document,sources:Vec<SheetCandidate<'a>>,deadline:Option<Instant>) -> Result<(Vec<(usize,std::borrow::Cow<'a,str>)>,Vec<String>,StylesheetUsage),String> {
    let mut accepted=Vec::new();let mut errors=Vec::new();let mut usage=StylesheetUsage::default();
    for candidate in sources {
        check_css_deadline(deadline)?;
        let (node,source)=match candidate {SheetCandidate::Source(node,source)=>(node,source),SheetCandidate::Error(error)=>{errors.push(error);continue;}};
        match doc.stylesheet_limits.accept(&source,usage) {
            Ok(next)=>{usage=next;accepted.push((node,source));},
            Err(error)=>errors.push(format!("CSS node {node}: {error}")),
        }
    }
    check_css_deadline(deadline)?;
    Ok((accepted,errors,usage))
}
fn accepted_stylesheets_with<'a>(doc:&'a Document,external:&'a HashMap<usize,String>,replacement:Option<(usize,&'a str)>) -> (Vec<(usize,std::borrow::Cow<'a,str>)>,Vec<String>,StylesheetUsage) {
    let sources = stylesheet_sources_with(doc,external,replacement,None).expect("unbudgeted stylesheet source collection");
    admit_stylesheets(doc,sources,None).expect("unbudgeted stylesheet admission")
}
pub(crate) fn accepted_stylesheets<'a>(doc:&'a Document,external:&'a HashMap<usize,String>)->(Vec<(usize,std::borrow::Cow<'a,str>)>,Vec<String>,StylesheetUsage) {accepted_stylesheets_with(doc,external,None)}
pub fn check_stylesheet_candidate(doc:&Document,external:&HashMap<usize,String>,node:usize,source:&str)->Result<StylesheetUsage,String> {
    let (accepted,errors,usage)=accepted_stylesheets_with(doc,external,Some((node,source)));
    if accepted.iter().any(|(owner,_)|*owner==node) {return Ok(usage);}
    let prefix=format!("CSS node {node}:");
    Err(errors.into_iter().find(|error|error.starts_with(&prefix)).unwrap_or_else(||"Stylesheet is not active in the current connected document.".into()))
}
pub(crate) fn document_order(doc:&Document)->Vec<usize> {
    let mut order=Vec::new();let mut seen=vec![false;doc.nodes.len()];let mut stack=if doc.nodes.is_empty(){Vec::new()}else{vec![0]};
    while let Some(node)=stack.pop(){if node>=seen.len()||seen[node]{continue;}seen[node]=true;order.push(node);stack.extend(doc.nodes[node].children.iter().rev().copied().filter(|&child|child!=0));}order
}
pub(crate) fn inline_stylesheet(doc:&Document,node:usize)->Result<String,String> {
    let mut source=String::new();
    for &child in &doc.nodes[node].children {if let Some(crate::html::Node{kind:NodeKind::Text(text) | NodeKind::Cdata(text),..})=doc.nodes.get(child) {
        let length=source.len().checked_add(text.len()).ok_or("Stylesheet text length overflow.")?;
        if length>doc.stylesheet_limits.sheet_bytes {return Err("Complete inline stylesheet exceeds its byte allowance.".into());}
        source.try_reserve(text.len()).map_err(|_|"Inline stylesheet allocation failed.")?;source.push_str(text);
    }}Ok(source)
}
pub fn stylesheet_usage(doc:&Document,external:&HashMap<usize,String>)->StylesheetUsage {accepted_stylesheets(doc,external).2}
pub fn stylesheet_diagnostics(doc:&Document,external:&HashMap<usize,String>)->Vec<String> {accepted_stylesheets(doc,external).1}
pub(crate) fn media_supported(value:&str)->bool {let value=value.trim();value.is_empty()||value.eq_ignore_ascii_case("all")||value.eq_ignore_ascii_case("screen")}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Length {
    Auto,
    Px(f32),
    Percent(f32),
    Em(f32),
    Rem(f32),
    Calc(CalculatedLength),
}

impl Length {
    pub fn has_percentage(self)->bool{matches!(self,Self::Percent(_))||matches!(self,Self::Calc(value)if value.percentage)}
    pub fn resolve(self, reference: f32, font: f32, root_font: f32) -> Option<f32> {
        let value = match self {
            Self::Auto => return None,
            Self::Px(value) => value,
            Self::Percent(value) => reference * value / 100.0,
            Self::Em(value) => font * value,
            Self::Rem(value) => root_font * value,
            Self::Calc(value)=>value.resolve(reference,font,root_font)?.clamp(-100_000.0,100_000.0)as f32,
        };
        Some(value.clamp(-100_000.0, 100_000.0))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Display {
    None,
    Block,
    Inline,
    InlineBlock,
    InlineFlex,
    Flex,
    Grid,
    Table,
    TableRowGroup,
    TableRow,
    TableCell,
}

impl Display {
    pub fn flex_container(self)->bool{matches!(self,Self::Flex|Self::InlineFlex)}
    pub fn atomic_inline(self)->bool{matches!(self,Self::InlineBlock|Self::InlineFlex)}
    pub fn block_level(self) -> bool {
        matches!(
            self,
            Self::Block
                | Self::Flex
                | Self::Grid
                | Self::Table
                | Self::TableRowGroup
                | Self::TableRow
                | Self::TableCell
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Position {
    Static,
    Relative,
    Absolute,
    Fixed,
    /// Computed sticky keyword; scroll-dependent clamping is not implemented.
    Sticky,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlexDirection {
    Row,
    RowReverse,
    Column,
    ColumnReverse,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlexPack {
    Start,
    End,
    LogicalStart,
    LogicalEnd,
    Center,
    Between,
    Around,
    Evenly,
    Stretch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlexAlign {
    Stretch,
    Start,
    End,
    Center,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WhiteSpace {
    Normal,
    NoWrap,
    Pre,
    PreWrap,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectFit {
    Fill,
    Contain,
    Cover,
    None,
    ScaleDown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageRendering {
    Auto,
    Smooth,
    HighQuality,
    CrispEdges,
    Pixelated,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ObjectPosition {
    pub fraction: f32,
    pub offset: Length,
}

impl ObjectPosition {
    const START: Self = Self {
        fraction: 0.0,
        offset: Length::Px(0.0),
    };
    const CENTER: Self = Self {
        fraction: 0.5,
        offset: Length::Px(0.0),
    };
}

/// One supported background layer, with its winning declaration's base owner.
/// A style attribute has no stylesheet owner and resolves against the document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackgroundImage {
    pub url: String,
    pub stylesheet: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackgroundSize {
    Auto,
    Contain,
    Cover,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListStyle {
    Disc,
    Decimal,
    None,
}

#[derive(Clone, Copy, Debug)]
pub enum LineHeight {
    Factor(f32),
    Length(Length),
}

#[derive(Clone, Debug)]
pub struct Style {
    pub display: Display,
    pub(crate) display_alias:Option<&'static str>,
    pub position: Position,
    /// A non-none identity translation still establishes a containing block.
    pub transform: Transform,
    /// Computed physical axes; scrollbars and scroll APIs are separate.
    pub overflow: Overflow,
    pub visible:bool,
    pub pointer_events:bool,
    pub(crate) z_index:Option<std::rc::Rc<z_index::Value>>,
    /// Physical offsets: top, right, bottom, left.
    pub inset: [Length; 4],
    pub color: u32,
    pub(crate) svg_paint:Option<std::sync::Arc<crate::image::SvgPaint>>,
    pub(crate) svg_paint_known:bool,
    /// Computed scalar. Ordinary HTML paint groups composite after descendants.
    pub opacity: f64,
    pub background: Option<u32>,
    /// Retain currentColor through explicit inheritance; background holds the
    /// used opaque color after this element's foreground cascade finishes.
    pub background_current_color: bool,
    /// None retains an unsupported or resource-refused clip winner.
    pub background_clip: Option<BackgroundClip>,
    pub background_image: Option<BackgroundImage>,
    /// Repeat the single background layer along the horizontal/vertical axes.
    pub background_repeat: [bool; 2],
    pub background_position: [ObjectPosition; 2],
    pub background_size: BackgroundSize,
    pub object_fit: ObjectFit,
    pub image_rendering: ImageRendering,
    pub object_position: [ObjectPosition; 2],
    pub font_size: f32,
    pub root_font_size: f32,
    pub font_families: Vec<String>,
    pub font_weight: u16,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub align: Align,
    pub legacy_align: Option<Align>,
    pub line_height: LineHeight,
    pub white_space: WhiteSpace,
    pub list_style: ListStyle,
    pub margin: [Length; 4],
    pub padding: [Length; 4],
    pub border_width: [Length; 4],
    pub border_color: [Option<u32>; 4],
    /// Border-only straight alpha. Other CSS color consumers remain opaque.
    pub border_alpha: [u8; 4],
    pub border_solid: [bool; 4],
    /// Clockwise from top-left. None preserves an unsupported winning radius.
    /// The current paint consumer admits equal circular absolute values only.
    pub border_radius: [Option<Length>; 4],
    pub width: Length,
    pub max_width: Option<Length>,
    pub height: Length,
    pub min_width: Length,
    pub min_height: Length,
    pub max_height: Option<Length>,
    pub border_box: bool,
    pub flex_direction: FlexDirection,
    pub flex_wrap: bool,
    pub justify_content: FlexPack,
    pub align_content: FlexPack,
    pub align_items: FlexAlign,
    pub align_self: Option<FlexAlign>,
    pub justify_items: Justify,
    pub justify_self: Justify,
    pub flex_grow: f32,
    pub flex_shrink: f32,
    pub flex_basis: Length,
    /// Row gap, column gap; percentages use the respective content-box axis.
    pub gap: [Length; 2],
    pub border_spacing: [Length; 2],
}

impl Default for Style {
    fn default() -> Self {
        Self {
            display: Display::Inline,
            display_alias:None,
            position: Position::Static,
            transform: Transform::None,
            overflow: Overflow::default(),
            visible:true,pointer_events:true,z_index:None,
            inset: [Length::Auto; 4],
            color: INITIAL_COLOR,
            svg_paint:None,svg_paint_known:true,
            opacity: 1.0,
            background: None,
            background_current_color: false,
            background_clip: Some(BackgroundClip::BorderBox),
            background_image: None,
            background_repeat: [true; 2],
            background_position: [ObjectPosition::START; 2],
            background_size: BackgroundSize::Auto,
            object_fit: ObjectFit::Fill,
            image_rendering: ImageRendering::Auto,
            object_position: [ObjectPosition::CENTER; 2],
            font_size: INITIAL_FONT_SIZE,
            root_font_size: INITIAL_FONT_SIZE,
            font_families: vec!["serif".into()],
            font_weight: 400,
            bold: false,
            italic: false,
            underline: false,
            align: Align::Left,
            legacy_align: None,
            line_height: LineHeight::Factor(1.5),
            white_space: WhiteSpace::Normal,
            list_style: ListStyle::Disc,
            margin: [Length::Px(0.0); 4],
            padding: [Length::Px(0.0); 4],
            border_width: [Length::Px(3.0); 4],
            border_color: [None; 4],
            border_alpha: [255; 4],
            border_solid: [false; 4],
            border_radius: [Some(Length::Px(0.0)); 4],
            width: Length::Auto,
            max_width: None,
            height: Length::Auto,
            min_width: Length::Auto,
            min_height: Length::Auto,
            max_height: None,
            border_box: false,
            flex_direction: FlexDirection::Row,
            flex_wrap: false,
            justify_content: FlexPack::Start,
            align_content: FlexPack::Stretch,
            align_items: FlexAlign::Stretch,
            align_self: None,
            justify_items: Justify::Normal,
            justify_self: Justify::Auto,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            flex_basis: Length::Auto,
            gap: [Length::Px(0.0); 2],
            border_spacing: [Length::Px(2.0); 2],
        }
    }
}

impl Style {
    pub(crate) fn circular_radius(&self) -> Option<f32> {
        radius::uniform(self.border_radius)
    }

    fn inherited(parent: &Self) -> Self {
        Self {
            visible:parent.visible,pointer_events:parent.pointer_events,
            color: parent.color,
            svg_paint:parent.svg_paint.clone(),svg_paint_known:parent.svg_paint_known,
            image_rendering: parent.image_rendering,
            font_size: parent.font_size,
            root_font_size: parent.root_font_size,
            font_families: parent.font_families.clone(),
            font_weight: parent.font_weight,
            bold: parent.bold,
            italic: parent.italic,
            underline: parent.underline,
            align: parent.align,
            legacy_align: parent.legacy_align,
            line_height: parent.line_height,
            white_space: parent.white_space,
            list_style: parent.list_style,
            border_spacing: parent.border_spacing,
            ..Self::default()
        }
    }
}

fn object_position(value: &str) -> Option<[ObjectPosition; 2]> {
    fn keyword(value: &str) -> Option<(usize, f32)> {
        match value {
            "left" => Some((0, 0.0)),
            "right" => Some((0, 1.0)),
            "top" => Some((1, 0.0)),
            "bottom" => Some((1, 1.0)),
            _ => None,
        }
    }
    fn component(value: &str) -> Option<ObjectPosition> {
        if value == "center" {
            return Some(ObjectPosition::CENTER);
        }
        match parse_length(value, false, true)? {
            Length::Percent(n) => Some(ObjectPosition {
                fraction: n / 100.0,
                offset: Length::Px(0.0),
            }),
            offset => Some(ObjectPosition {
                fraction: 0.0,
                offset,
            }),
        }
    }
    fn edge(value: &str, offset: Option<&str>) -> Option<(usize, ObjectPosition)> {
        let (axis, fraction) = keyword(value)?;
        let mut result = ObjectPosition {
            fraction,
            offset: Length::Px(0.0),
        };
        if let Some(offset) = offset {
            let parsed = component(offset)?;
            if offset == "center" {
                return None;
            }
            result.fraction += parsed.fraction * if fraction == 0.0 { 1.0 } else { -1.0 };
            result.offset = match parsed.offset {
                Length::Px(n) => Length::Px(if fraction == 0.0 { n } else { -n }),
                Length::Em(n) => Length::Em(if fraction == 0.0 { n } else { -n }),
                Length::Rem(n) => Length::Rem(if fraction == 0.0 { n } else { -n }),
                _ => return None,
            };
        }
        Some((axis, result))
    }
    let words: Vec<_> = value.split_ascii_whitespace().collect();
    if words.is_empty() || words.len() > 4 {
        return None;
    }
    let mut result = [ObjectPosition::CENTER; 2];
    if words.len() == 1 {
        if let Some((axis, p)) = edge(words[0], None) {
            result[axis] = p;
        } else {
            result[0] = component(words[0])?;
        }
        return Some(result);
    }
    if words.len() == 2 {
        match (keyword(words[0]), keyword(words[1])) {
            (Some((a, _)), Some((b, _))) if a != b => {
                result[a] = edge(words[0], None)?.1;
                result[b] = edge(words[1], None)?.1;
            }
            (Some((0, _)), None) => {
                result[0] = edge(words[0], None)?.1;
                result[1] = component(words[1])?;
            }
            (Some((1, _)), None) if words[1] == "center" => result[1] = edge(words[0], None)?.1,
            (None, Some((1, _))) => {
                result[0] = component(words[0])?;
                result[1] = edge(words[1], None)?.1;
            }
            (None, Some((0, _))) if words[0] == "center" => result[0] = edge(words[1], None)?.1,
            (None, None) => {
                result = [component(words[0])?, component(words[1])?];
            }
            _ => return None,
        }
        return Some(result);
    }
    // CSS Values <position> has 1, 2 or 4 values. Three-value syntax
    // belongs to background-position and is not valid for object-position.
    if words.len() != 4 {
        return None;
    }
    let (first, a) = edge(words[0], Some(words[1]))?;
    let (second, b) = edge(words[2], Some(words[3]))?;
    if first == second {
        return None;
    }
    result[first] = a;
    result[second] = b;
    Some(result)
}

#[derive(Debug)]
struct Declaration {
    name: String,
    value: String,
    important: bool,
    stylesheet: Option<usize>,
}

#[derive(Debug)]
struct Compound {
    tag: Option<String>,
    ids: Vec<String>,
    classes: Vec<String>,
    attributes: Vec<AttributeSelector>,
    root: bool,
    checked: bool,
    focus:bool,
    focus_within:bool,
    hover:bool,
    disabled: bool,
    negations: Vec<Vec<Selector>>,
    structural:Vec<structural::Predicate>,
}

#[derive(Debug)]
struct AttributeSelector {
    name: String,
    value: Option<String>,
    operator: AttributeOperator,
    ascii_insensitive: bool,
    substring_prefix: Vec<usize>,
}
#[derive(Clone,Copy,Debug)]
enum AttributeOperator {Exact,Token,Dash,Prefix,Suffix,Substring}

#[path="css_attribute_substrings.rs"]
mod attribute_substrings;

#[derive(Clone, Copy, Debug)]
enum Relation {
    Descendant,
    Child,
    AdjacentSibling,
    GeneralSibling,
}

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
enum SelectorTarget { Element, Before, After }

#[derive(Debug)]
struct Selector {
    target: SelectorTarget,
    parts: Vec<Compound>,
    relations: Vec<Relation>,
    specificity: (u16, u16, u16),
}

impl Selector{fn depends_on_hover(&self)->bool{self.target==SelectorTarget::Element&&self.parts.iter().any(|p|p.hover||p.negations.iter().flatten().any(Selector::depends_on_hover))}}

struct Rule {
    selectors: Vec<Selector>,
    declarations: Vec<Declaration>,
}

pub fn compute(doc: &Document, external: &HashMap<usize, String>) -> Vec<Style> {
    compute_with_state(doc, external, None)
}

/// DOM queries use the same selector grammar as styles. Candidates are supplied
/// in connected document order by the DOM bridge, excluding the query root.
/// Every traversal shares the caller's remaining job budget.
#[cfg(test)]
pub fn query_first(
    doc: &Document,
    source: &str,
    candidates: &[usize],
    checked: Option<&dyn Fn(usize) -> Option<bool>>,
    budget: &mut usize,
) -> Result<Option<usize>, String> {
    let mut copy_budget=usize::MAX;
    query_matches(doc, source, candidates, checked, budget, true,usize::MAX,None,&mut copy_budget)
        .map(|matches| matches.first().copied())
}

/// Match the supported selector grammar in candidate order, once per candidate.
/// Test convenience entry; production callers supply their retained allowance.
#[cfg(test)]
pub fn query_all(
    doc: &Document,
    source: &str,
    candidates: &[usize],
    checked: Option<&dyn Fn(usize) -> Option<bool>>,
    budget: &mut usize,
) -> Result<Vec<usize>, String> {
    let mut copy_budget=usize::MAX;
    query_all_with_storage(doc,source,candidates,checked,budget,usize::MAX,None,&mut copy_budget)
}

/// Result backing is fallibly admitted against the caller's current storage.
/// The caller retains responsibility for charging the returned capacity and
/// atomically publishing its registry record.
#[cfg(test)]
pub fn query_all_with_storage(
    doc:&Document,source:&str,candidates:&[usize],checked:Option<&dyn Fn(usize)->Option<bool>>,
    budget:&mut usize,result_bytes:usize,result_limit:Option<usize>,copy_budget:&mut usize,
)->Result<Vec<usize>,String> {
    query_matches(doc,source,candidates,checked,budget,false,result_bytes,result_limit,copy_budget)
}

/// No author callbacks run here. The caller owns a byte-admitted candidate
/// vector and checked topology; compaction reuses that backing for the result.
pub fn query_with_topology(
    doc:&Document,source:&str,mut candidates:Vec<usize>,checked:Option<&dyn Fn(usize)->Option<bool>>,
    budget:&mut usize,parents:&[Option<usize>],sibling_slots:&[usize],first_only:bool,copy_budget:&mut usize,
)->Result<Vec<usize>,String> {
    if source.len()>MAX_SELECTOR_BYTES*16{return Err("Selector query exceeds 8 KiB.".into());}
    let storage=structural::query_cost(source);
    let syntax_work=if storage==0{0}else{source.len().saturating_mul(4)};
    if syntax_work>*budget{*budget=0;return Err("DOM structural selector syntax work allowance exhausted.".into());}
    *budget-=syntax_work;
    if storage>*copy_budget{*copy_budget=0;return Err("DOM structural selector syntax copy allowance exhausted.".into());}
    *copy_budget-=storage;
    let selectors=parse_selector_list(source,false).ok_or("Invalid selector or selector syntax outside the supported subset.")?;
    // Detached leaves and parked inert slots can extend the document arena
    // without participating in the checked connected topology. Per-node
    // lookups treat their missing parent/sibling metadata as absent ancestry.
    let context=MatchContext{doc,parents,previous_elements:&[],sibling_slots:Some(sibling_slots),checked,charge_text:true};
    let mut kept=0;
    for position in 0..candidates.len() {
        let node=candidates[position];
        if node>=doc.nodes.len(){return Err("Selector candidate identity is unavailable.".into());}
        let matched=selectors.iter().any(|selector|matches_selector(selector,node,&context,budget));
        if *budget==0{return Err("DOM selector work limit reached.".into());}
        if matched {
            if kept!=position {
                let bytes=std::mem::size_of::<usize>();
                if bytes>*copy_budget{*copy_budget=0;return Err("DOM selector result copy work limit reached.".into());}
                *copy_budget-=bytes;candidates[kept]=node;
            }
            kept+=1;if first_only{break;}
        }
    }
    if kept==0{return Ok(Vec::new());}
    candidates.truncate(kept);Ok(candidates)
}

pub fn valid_query_selector(source: &str) -> bool {
    source.len() <= MAX_SELECTOR_BYTES * 16 && parse_selector_list(source, false).is_some()
}

#[cfg(test)]
fn query_matches(
    doc: &Document,
    source: &str,
    candidates: &[usize],
    checked: Option<&dyn Fn(usize) -> Option<bool>>,
    budget: &mut usize,
    first_only: bool,
    result_bytes:usize,
    result_limit:Option<usize>,
    copy_budget:&mut usize,
) -> Result<Vec<usize>, String> {
    if source.len() > MAX_SELECTOR_BYTES * 16 {
        return Err("Selector query exceeds 8 KiB.".into());
    }
    let selectors = parse_selector_list(source, false)
        .ok_or("Invalid selector or selector syntax outside the supported subset.")?;
    let cost = doc.nodes.len().saturating_mul(3);
    if cost > *budget {
        *budget = 0;
        return Err("DOM selector work limit reached.".into());
    }
    *budget -= cost;
    let mut parents = vec![None; doc.nodes.len()];
    let mut previous_elements = vec![None; doc.nodes.len()];
    for (parent, node) in doc.nodes.iter().enumerate() {
        let mut previous = None;
        for &child in &node.children {
            if child != 0 && child < parents.len() && parents[child].is_none() {
                parents[child] = Some(parent);
                if matches!(doc.nodes[child].kind, NodeKind::Element { .. }) {
                    previous_elements[child] = previous;
                    previous = Some(child);
                }
            }
        }
    }
    let context = MatchContext {
        doc,
        parents: &parents,
        previous_elements: &previous_elements,
        sibling_slots:None,
        checked,
        charge_text: true,
    };
    let mut matched = Vec::new();
    for &node in candidates {
        if selectors
            .iter()
            .any(|selector| matches_selector(selector, node, &context, budget))
        {
            if result_limit.is_some_and(|limit|matched.len()>=limit) {
                return Err(format!("DOM selector result limit reached ({} elements).",result_limit.unwrap()));
            }
            if matched.len()==matched.capacity() {
                let before=matched.capacity();let slot=std::mem::size_of::<usize>();
                let available=(result_bytes/slot).saturating_sub(before);
                let growth=before.max(4).min(available);
                if growth==0{return Err("DOM retained selector result allowance exhausted.".into());}
                let mut next=Vec::new();next.try_reserve_exact(before.checked_add(growth).ok_or("Selector result capacity overflow.")?).map_err(|_|"Could not reserve selector result storage.")?;
                if next.capacity()>result_bytes/slot{return Err("DOM retained selector result allocation exceeds allowance.".into());}
                // Admit movement before publishing the replacement. Work is
                // cumulative; a refusal leaves no partial snapshot exposed.
                if matched.len()>*budget{*budget=0;return Err("DOM selector work limit reached.".into());}
                *budget-=matched.len();
                let bytes=matched.len()*slot;
                if bytes>*copy_budget{*copy_budget=0;return Err("DOM selector result copy work limit reached.".into());}
                *copy_budget-=bytes;next.append(&mut matched);matched=next;
            }
            matched.push(node);
            if first_only {
                return Ok(matched);
            }
        }
        if *budget == 0 {
            return Err("DOM selector work limit reached.".into());
        }
    }
    Ok(matched)
}

// Form controls can have live checkedness different from their original HTML.
// Returning None leaves an element's initial checked/selected attribute in use.
pub fn compute_with_state(
    doc: &Document,
    external: &HashMap<usize, String>,
    checked: Option<&dyn Fn(usize) -> Option<bool>>,
) -> Vec<Style> {
    compute_with_environment(doc,external,checked,doc.media_environment)
}

pub(crate) fn compute_with_environment(
    doc: &Document,
    external: &HashMap<usize,String>,
    checked: Option<&dyn Fn(usize)->Option<bool>>,
    environment: Option<MediaEnvironment>,
) -> Vec<Style> {
    compute_with_environment_and_properties(doc, external, checked, environment).styles
}

pub(crate) fn compute_with_environment_and_properties(
    doc: &Document,
    external: &HashMap<usize, String>,
    checked: Option<&dyn Fn(usize) -> Option<bool>>,
    environment: Option<MediaEnvironment>,
) -> CascadeOutput {
    // Only an explicitly supplied script budget makes the shared core fallible.
    compute_cascade(doc, external, checked, environment, None, None, None, false).expect("unbudgeted CSS cascade")
}

pub(crate) fn compute_with_environment_and_properties_budgeted(
    doc: &Document,
    external: &HashMap<usize, String>,
    checked: Option<&dyn Fn(usize) -> Option<bool>>,
    environment: Option<MediaEnvironment>,
    budget: &mut CssBudget<'_>,
) -> Result<CascadeOutput, String> {
    compute_cascade(doc, external, checked, environment, Some(budget), None, None, false)
}

/// Preserve the established layout allowance while reserving the additional
/// transform/opacity parser, cascade and snapshot work before it runs.
pub(crate) fn compute_with_environment_and_properties_transform_budgeted(
    doc: &Document,
    external: &HashMap<usize, String>,
    checked: Option<&dyn Fn(usize) -> Option<bool>>,
    environment: Option<MediaEnvironment>,
    budget: &mut CssBudget<'_>,
) -> Result<CascadeOutput, String> {
    compute_cascade(doc, external, checked, environment, None, Some(budget), None, false)
}

pub(crate) fn compute_with_environment_and_properties_cached(
    doc: &Document, external: &HashMap<usize,String>, checked: Option<&dyn Fn(usize)->Option<bool>>,
    environment: Option<MediaEnvironment>, cache: Option<&StylesheetCache>,
) -> CascadeOutput {
    compute_cascade(doc,external,checked,environment,None,None,cache,true).expect("unbudgeted cached CSS cascade")
}

pub(crate) fn compute_with_environment_and_properties_budgeted_cached(
    doc: &Document, external: &HashMap<usize,String>, checked: Option<&dyn Fn(usize)->Option<bool>>,
    environment: Option<MediaEnvironment>, budget: &mut CssBudget<'_>, cache: Option<&StylesheetCache>,
) -> Result<CascadeOutput,String> {
    compute_cascade(doc,external,checked,environment,Some(budget),None,cache,true)
}

pub(crate) fn compute_with_environment_and_properties_transform_budgeted_cached(
    doc: &Document, external: &HashMap<usize,String>, checked: Option<&dyn Fn(usize)->Option<bool>>,
    environment: Option<MediaEnvironment>, budget: &mut CssBudget<'_>, cache: Option<&StylesheetCache>,
) -> Result<CascadeOutput,String> {
    compute_cascade(doc,external,checked,environment,None,Some(budget),cache,true)
}

fn reserve_transform_storage(
    nodes: usize, known: &mut Vec<bool>, meter: Option<&mut CssBudget<'_>>,
) -> Result<(), String> {
    if !known.is_empty() { return Ok(()); }
    if let Some(meter) = meter {
        meter.work(nodes)?;
        meter.copies(nodes.saturating_mul(std::mem::size_of::<Option<Transform>>() + std::mem::size_of::<bool>()))?;
    }
    known.try_reserve_exact(nodes).map_err(|_| "Transform certainty storage reservation failed.")?;
    known.resize(nodes, true);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn compute_cascade(
    doc: &Document,
    external: &HashMap<usize, String>,
    checked: Option<&dyn Fn(usize) -> Option<bool>>,
    environment: Option<MediaEnvironment>,
    mut meter: Option<&mut CssBudget<'_>>,
    mut transform_meter: Option<&mut CssBudget<'_>>,
    previous_sheets: Option<&StylesheetCache>,
    retain_sheets: bool,
) -> Result<CascadeOutput, String> {
    if let Some(meter) = meter.as_deref_mut() {
        meter.document_structure(doc)?;
    }
    let mut parents = vec![None; doc.nodes.len()];
    let mut previous_elements = vec![None; doc.nodes.len()];
    for (parent, node) in doc.nodes.iter().enumerate() {
        let mut previous = None;
        for &child in &node.children {
            if child != 0 && child < parents.len() && parents[child].is_none() {
                parents[child] = Some(parent);
                if matches!(doc.nodes[child].kind, NodeKind::Element { .. }) {
                    previous_elements[child] = previous;
                    previous = Some(child);
                }
            }
        }
    }
    let match_context = MatchContext {
        doc,
        parents: &parents,
        previous_elements: &previous_elements,
        sibling_slots:None,
        checked,
        charge_text: meter.is_some(),
    };
    // Script moves preserve arena identity, so a parent can have a later arena
    // index than its child. Visit the connected physical tree in preorder for
    // inheritance and stylesheet order. Retained detached nodes keep defaults.
    let mut order = Vec::new();
    let mut visited = vec![false; doc.nodes.len()];
    let mut stack = if doc.nodes.is_empty() {
        Vec::new()
    } else {
        vec![0]
    };
    while let Some(index) = stack.pop() {
        if index >= doc.nodes.len() || visited[index] {
            continue;
        }
        visited[index] = true;
        order.push(index);
        stack.extend(
            doc.nodes[index]
                .children
                .iter()
                .rev()
                .copied()
                .filter(|&child| child != 0 && parents.get(child) == Some(&Some(index))),
        );
    }
    // Inert content does not supply stylesheets or change the live document's
    // root font size, even when its children were created before their parent.
    let mut inert = vec![false; doc.nodes.len()];
    for &index in &order {
        if let Some(parent) = parents[index] {
            inert[index] = inert[parent]
                || matches!(&doc.nodes[parent].kind, NodeKind::Element { tag, .. }
                    if matches!(tag.as_str(), "template" | "iframe" | "object") || doc.scripting_enabled && tag == "noscript");
        }
    }
    let prepared = stylesheet_cache::prepare(doc,external,environment,previous_sheets,retain_sheets,
        meter.as_deref_mut(),transform_meter.as_deref_mut())?;
    let rules = prepared.rules();
    let has_transform_rules = prepared.has_transform;

    let mut computed = vec![Style::default(); doc.nodes.len()];
    let mut variables = vec![custom_properties::Variables::default(); doc.nodes.len()];
    let mut eligible = vec![false; doc.nodes.len()];
    let mut complete = true;
    let mut overflow_known = vec![[true; 2]; doc.nodes.len()];
    let mut position_known = vec![true; doc.nodes.len()];
    let mut canvas_foregrounds = HashMap::new();
    let mut svg_geometry = HashMap::new();
    let mut touch_actions: HashMap<usize, Option<TouchAction>> = HashMap::new();
    let mut font_sizes: HashMap<usize, Option<f32>> = HashMap::new();
    // Default opacity 1 requires no retained per-node allocation. A None entry
    // records an indeterminate winner rather than fabricating that default.
    let mut opacities: HashMap<usize, Option<f64>> = HashMap::new();
    let mut transform_known = Vec::new();
    if has_transform_rules {
        if transform_meter.is_some() {
            reserve_transform_storage(doc.nodes.len(), &mut transform_known, transform_meter.as_deref_mut())?;
        } else {
            reserve_transform_storage(doc.nodes.len(), &mut transform_known, meter.as_deref_mut())?;
        }
    }
    let mut variable_budget = custom_properties::Budget::default();
    // Paint has no independent count/scratch ceiling: supplied CSS job budgets,
    // checked allocation, and the existing image/cache allowances govern it.
    let mut paint_budget=custom_properties::Budget::with_remaining(usize::MAX,usize::MAX);
    let paint_needed=svg_paint::needed(doc,&order,&parents,&inert,meter.as_deref_mut(),transform_meter.as_deref_mut())?;
    let mut budget = MAX_MATCH_STEPS;
    let mut root_font_size = INITIAL_FONT_SIZE;
    let mut root_font_known = true;
    for &index in &order {
        let node = &doc.nodes[index];
        if let Some(meter) = meter.as_deref_mut() {
            if let Some(parent) = parents[index].and_then(|parent| computed.get(parent)) { meter.parent(parent)?; }
        }
        let parent = parents[index]
            .and_then(|parent| computed.get(parent))
            .cloned()
            .unwrap_or_default();
        let inherited_variables = parents[index].and_then(|parent|variables.get(parent)).cloned().unwrap_or_default();
        let parent_overflow_known = parents[index].map_or([true; 2], |parent| overflow_known[parent]);
        let parent_position_known = parents[index].map_or(true, |parent| position_known[parent]);
        let parent_transform_known = parents[index].and_then(|parent| transform_known.get(parent)).copied().unwrap_or(true);
        let parent_opacity_known = parents[index].and_then(|parent| opacities.get(&parent)).is_none_or(Option::is_some);
        variables[index] = inherited_variables.clone();
        if inert[index]
            || doc.scripting_enabled
                && matches!(&node.kind, NodeKind::Element { tag, .. } if tag == "noscript")
        {
            let mut style = Style::inherited(&parent);
            style.display = Display::None;
            computed[index] = style;
            continue;
        }
        // Comments are real DOM leaves with no CSS box or inherited display.
        if node.kind.is_nonrendered_node() {
            let mut style = Style::inherited(&parent);
            style.display = Display::None;
            computed[index] = style;
            continue;
        }
        let NodeKind::Element { tag, .. } = &node.kind else {
            let mut inherited = parent;
            inherited.transform = Transform::None;
            inherited.opacity = 1.0;
            computed[index] = inherited;
            continue;
        };
        eligible[index] = true;
        let mut style = Style::inherited(&parent);
        style.root_font_size = root_font_size;
        user_agent(
            &mut style,
            tag,
            doc.attr(index, "href").is_some(),
            doc.attr(index, "hidden").is_some(),
        );
        if tag == "th" && parent.align == Align::Left {
            style.align = Align::Center;
        }
        // Legacy HTML presentation is a zero-specificity author hint. Apply it
        // before stylesheets and inline declarations so ordinary CSS wins.
        if tag == "font" {
            if let Some(meter) = meter.as_deref_mut() { meter.source(doc.attr(index, "face").map_or(0, str::len))?; }
            if let Some(families) = doc.attr(index, "face").and_then(font_families) {
                style.font_families = families;
            }
        }
        if matches!(
            tag.as_str(),
            "div"
                | "p"
                | "h1"
                | "h2"
                | "h3"
                | "h4"
                | "h5"
                | "h6"
                | "td"
                | "th"
                | "tr"
                | "thead"
                | "tbody"
                | "tfoot"
        ) {
            match doc
                .attr(index, "align")
                .map(str::trim)
                .map(str::to_ascii_lowercase)
                .as_deref()
            {
                Some("left") => {
                    style.align = Align::Left;
                    style.legacy_align = Some(Align::Left);
                }
                Some("center") => {
                    style.align = Align::Center;
                    style.legacy_align = Some(Align::Center);
                }
                Some("right") => {
                    style.align = Align::Right;
                    style.legacy_align = Some(Align::Right);
                }
                _ => {}
            }
        }
        if matches!(tag.as_str(), "table" | "td" | "th") {
            if let Some(width) = doc.attr(index, "width").and_then(html_dimension) {
                style.width = width;
            }
        }
        if tag == "table" {
            if let Some(spacing) = doc.attr(index, "cellspacing").and_then(html_pixels) {
                style.border_spacing = [Length::Px(spacing); 2];
            }
            if doc
                .attr(index, "align")
                .is_some_and(|align| align.eq_ignore_ascii_case("center"))
            {
                style.margin[1] = Length::Auto;
                style.margin[3] = Length::Auto;
            }
        }
        if matches!(tag.as_str(), "td" | "th") {
            if doc.attr(index, "nowrap").is_some() {
                style.white_space = WhiteSpace::NoWrap;
            }
            let mut ancestor = parents[index];
            while let Some(node) = ancestor {
                if matches!(&doc.nodes[node].kind, NodeKind::Element { tag, .. } if tag == "table")
                {
                    if let Some(padding) = doc.attr(node, "cellpadding").and_then(html_pixels) {
                        style.padding = [Length::Px(padding); 4];
                    }
                    break;
                }
                ancestor = parents[node];
            }
        }
        let mut applicable = Vec::new();
        let mut has_transform = false;
        let candidates = if let Some(indexed) = prepared.rule_candidates() {
            indexed.for_element(&match_context, index, rules.len(), meter.as_deref_mut())?
        } else { None };
        let count = candidates.as_ref().map_or(rules.len(), |indices| indices.len());
        for candidate in 0..count {
            if budget == 0 {
                complete = false;
                break;
            }
            let order = candidates.as_ref().map_or(candidate, |indices| indices[candidate]);
            let rule = &rules[order];
            let mut rule_budget = budget;
            if let Some(meter) = meter.as_deref_mut() {
                meter.work(1)?;
                rule_budget = rule_budget.min(meter.remaining_work());
            }
            let before = rule_budget;
            let specificity = rule
                .selectors
                .iter()
                .filter(|selector| matches_selector(selector, index, &match_context, &mut rule_budget))
                .map(|selector| selector.specificity)
                .max();
            let spent = before - rule_budget;
            budget -= spent;
            if let Some(meter) = meter.as_deref_mut() {
                meter.work(spent)?;
                if rule_budget == 0 && meter.remaining_work() == 0 { return Err("CSS selector job work limit reached.".into()); }
            }
            if budget == 0 { complete = false; }
            if let Some(specificity) = specificity {
                if let Some(meter) = meter.as_deref_mut() {
                    meter.work(rule.declarations.len())?;
                    meter.copies(rule.declarations.len().saturating_mul(128))?;
                }
                for (declaration_order, declaration) in rule.declarations.iter().enumerate() {
                    has_transform |= declaration.name == "transform";
                    applicable.push((
                        (
                            declaration.important,
                            false,
                            specificity,
                            order,
                            declaration_order,
                        ),
                        declaration,
                    ));
                }
            }
        }
        if let Some(meter) = meter.as_deref_mut() { meter.source(doc.attr(index, "style").map_or(0, |text| prefix(text, 16_384).len()))?; }
        let inline = if let Some(text) = doc.attr(index, "style") {
            parse_declarations_inner(prefix(text, 16_384),MAX_DECLARATIONS,true,transform_meter.as_deref_mut())?
        } else { Vec::new() };
        for (order, declaration) in inline.iter().enumerate() {
            if declaration.name == "transform" {
                has_transform = true;
                if transform_meter.is_some() {
                    reserve_transform_storage(doc.nodes.len(), &mut transform_known, transform_meter.as_deref_mut())?;
                } else {
                    reserve_transform_storage(doc.nodes.len(), &mut transform_known, meter.as_deref_mut())?;
                }
            }
            if let Some(meter) = meter.as_deref_mut() { meter.work(1)?; meter.copies(128)?; }
            applicable.push((
                (declaration.important, true, (0, 0, 0), 0, order),
                declaration,
            ));
        }
        if let Some(meter) = meter.as_deref_mut() {
            let count = applicable.len();
            let comparisons = if count < 2 { 0 } else { count.saturating_mul(usize::BITS as usize - count.leading_zeros() as usize) };
            meter.work(comparisons)?;
        }
        applicable.sort_by_key(|(priority, _)| *priority);
        let cascade = |budget: &mut custom_properties::Budget| custom_properties::Variables::cascade(&inherited_variables,
            applicable.iter().map(|(_,declaration)|*declaration), budget);
        variables[index] = if let Some(meter) = meter.as_deref_mut() { meter.variables(&mut variable_budget, cascade)? } else { cascade(&mut variable_budget) };
        if let Some(meter) = meter.as_deref_mut() {
            // Source preflight and reverse winner lookups inspect the list
            // for each independently computed position/transform consumer.
            meter.work(applicable.len().saturating_mul(if has_transform { 4 } else { 2 }))?;
            for (_, declaration) in applicable.iter().filter(|(_, declaration)| matches!(declaration.name.as_str(), "position" | "transform")) {
                meter.source(declaration.name.len().saturating_add(declaration.value.len()))?;
            }
        }
        if has_transform {
            if let Some(meter) = transform_meter.as_deref_mut() {
                meter.work(applicable.len().saturating_mul(2))?;
                for (_, declaration) in applicable.iter().filter(|(_, declaration)| declaration.name == "transform") {
                    meter.source(declaration.name.len().saturating_add(declaration.value.len()))?;
                }
            }
        }
        let resolve_position = |budget: &mut custom_properties::Budget| {
            let candidates = applicable.iter().rev()
                .filter(|(_, declaration)| declaration.name == "position")
                .map(|(priority, declaration)| (*declaration, priority.1));
            position::cascade(parent.position, parent_position_known, candidates, &variables[index], budget)
        };
        let (computed_position, known) = if let Some(meter) = meter.as_deref_mut() {
            meter.variables(&mut variable_budget, resolve_position)?
        } else {
            resolve_position(&mut variable_budget)
        };
        style.position = computed_position;
        position_known[index] = known;
        let mut has_radius = false;
        let mut sheet_background_clip = Some(BackgroundClip::BorderBox);
        let mut sheet_line_height = parent.line_height;
        let mut sheet_z_index=None;
        let mut sheet_interaction=[parent.visible,parent.pointer_events];
        let mut sheet_grid_alignment = (style.justify_items, style.justify_self);
        let parent_font_known=parents[index].and_then(|parent|font_sizes.get(&parent)).is_none_or(Option::is_some);
        let mut font_known=parent_font_known;
        if node.namespace==crate::html::Namespace::Svg {
            if let Some(text)=doc.attr(index,"font-size") {
                let hint=|budget:&mut custom_properties::Budget|svg_geometry::font_hint(&mut style,&parent,text,&variables[index],budget,&mut font_known,parent_font_known,root_font_known);
                if let Some(meter)=meter.as_deref_mut(){meter.variables(&mut variable_budget,hint)?;}
                else if let Some(meter)=transform_meter.as_deref_mut(){meter.variables(&mut variable_budget,hint)?;}
                else {let mut hint=hint;hint(&mut variable_budget);}
            }
        }
        let mut sheet_touch_action = Some(TouchAction::AUTO);
        let mut touch_action = Some(TouchAction::AUTO);
        let parent_touch_action = parents[index].and_then(|parent| touch_actions.get(&parent)).copied().unwrap_or(Some(TouchAction::AUTO));
        let mut sheet_opacity = (1.0, true);
        let mut opacity_known = true;
        for (priority, declaration) in &applicable {
            if declaration.name == "transform" {
                continue;
            }
            if declaration.name == "position" { continue; }
            if let Some(meter) = meter.as_deref_mut() { meter.source(declaration.name.len().saturating_add(declaration.value.len()))?; }
            if radius::property(&declaration.name) { has_radius = true; continue; }
            if declaration.name == "font-size" {
                let apply=|budget:&mut custom_properties::Budget| custom_properties::apply_font_size(&mut style,&parent,declaration,&variables[index],budget,&mut font_known,parent_font_known,root_font_known);
                if let Some(meter)=meter.as_deref_mut() {meter.variables(&mut variable_budget,apply)?;}
                else if let Some(meter)=transform_meter.as_deref_mut() {
                    meter.source(declaration.name.len().saturating_add(declaration.value.len()))?;
                    meter.variables(&mut variable_budget,apply)?;
                } else {let mut apply=apply;apply(&mut variable_budget);}
                continue;
            }
            if declaration.name == "touch-action" {
                let apply = |budget: &mut custom_properties::Budget| touch_action::apply(&mut touch_action, &mut sheet_touch_action, parent_touch_action, declaration, priority.1, &variables[index], budget);
                if let Some(meter)=meter.as_deref_mut() {meter.variables(&mut variable_budget,apply)?;}
                else if let Some(meter)=transform_meter.as_deref_mut() {
                    meter.source(declaration.name.len().saturating_add(declaration.value.len()))?;
                    meter.variables(&mut variable_budget,apply)?;
                } else {let mut apply=apply;apply(&mut variable_budget);}
                continue;
            }
            if declaration.name == "opacity" {
                let mut apply_opacity = |budget: &mut custom_properties::Budget| {
                    opacity::apply(&mut style,declaration,priority.1,&mut sheet_opacity,
                        (parent.opacity,parent_opacity_known),&mut opacity_known,&variables[index],budget)
                };
                if let Some(meter) = meter.as_deref_mut() {
                    meter.variables(&mut variable_budget,apply_opacity)?;
                } else if let Some(meter) = transform_meter.as_deref_mut() {
                    // This force-layout entry meters newly introduced property
                    // work without charging the established full cascade twice.
                    meter.source(declaration.name.len().saturating_add(declaration.value.len()))?;
                    meter.variables(&mut variable_budget,apply_opacity)?;
                } else { apply_opacity(&mut variable_budget); }
                continue;
            }
            if declaration.name == "font" {
                let value = declaration.value.trim().to_ascii_lowercase();
                if apply_font_with_availability(&mut style,&parent,declaration,&value,&mut font_known,parent_font_known,root_font_known) && !priority.1 {
                    sheet_line_height = style.line_height;
                }
                continue;
            }
            let mut apply_box = |budget: &mut custom_properties::Budget| {
                if z_index::apply(&mut style,&parent,declaration,priority.1,&mut sheet_z_index,&variables[index],budget){true}
                else if interaction_style::apply(&mut style,&parent,declaration,priority.1,&mut sheet_interaction,&variables[index],budget){true}
                else if grid_alignment::apply(&mut style,&parent,declaration,priority.1,&mut sheet_grid_alignment,&variables[index],budget) {
                    true
                } else if line_height::apply(&mut style,&parent,declaration,priority.1,&mut sheet_line_height,&variables[index],budget) {
                    true
                } else if background_clip::apply(&mut style,&parent,declaration,priority.1,&mut sheet_background_clip,&variables[index],budget) {
                    true
                } else if custom_properties::apply_overflow(&mut style,&parent,declaration,&variables[index],budget,&mut overflow_known[index],parent_overflow_known) {
                    true
                } else {
                    custom_properties::apply_box(&mut style,&parent,declaration,&variables[index],budget)
                }
            };
            let applied = if let Some(meter) = meter.as_deref_mut() { meter.variables(&mut variable_budget, apply_box)? } else { apply_box(&mut variable_budget) };
            if !applied {
                apply(&mut style, &parent, declaration);
            }
            if !priority.1 && declaration.name == "background"
                && (applied || background_clip::resets_background(&declaration.value.trim().to_ascii_lowercase())) {
                sheet_background_clip = style.background_clip;
            }
        }
        // currentColor computes to itself, including across explicit inherit;
        // its used value is this element's final foreground, independent of
        // declaration order. Existing foreground and border policy is unchanged.
        if style.background_current_color {style.background=Some(style.color);}
        style.overflow.computed_availability(&mut overflow_known[index]);
        style.overflow = style.overflow.computed();
        if tag == "html" {
            root_font_size = style.font_size;
            root_font_known = font_known;
            style.root_font_size = root_font_size;
        }
        style.line_height = line_height::computed(style.line_height,style.font_size,style.root_font_size);
        if paint_needed.get(index)==Some(&true) {
            let paint=|budget:&mut custom_properties::Budget|svg_paint::compute(doc,index,&parent,applicable.iter().map(|(_,d)|*d),&variables[index],budget);
            let (paint,known)=if let Some(meter)=meter.as_deref_mut(){meter.variables(&mut paint_budget,paint)?}else if let Some(meter)=transform_meter.as_deref_mut(){meter.variables(&mut paint_budget,paint)?}else{paint(&mut paint_budget)};
            style.svg_paint=paint;style.svg_paint_known=known;
        }
        if node.namespace==crate::html::Namespace::Svg {
            let parent_geometry=parents[index].and_then(|id|svg_geometry.get(&id)).and_then(Option::as_ref);
            let outer=tag=="svg" && parents[index].is_none_or(|id|doc.nodes[id].namespace!=crate::html::Namespace::Svg);
            let charge=|meter:&mut CssBudget<'_>|->Result<(),String>{
                meter.work(1)?;
                meter.copies(std::mem::size_of::<(usize,Option<crate::image::SvgStyle>)>().checked_mul(6).and_then(|n|n.checked_add(9*32)).ok_or("SVG computed geometry backing overflow.")?)
            };
            if let Some(meter)=meter.as_deref_mut() {charge(meter)?;}
            else if let Some(meter)=transform_meter.as_deref_mut() {charge(meter)?;}
            let resolve=|budget:&mut custom_properties::Budget|svg_geometry::compute(doc,index,tag,outer,parent_geometry,parent_geometry.is_some(),style.font_size,style.root_font_size,font_known&&root_font_known,applicable.iter().map(|(_,d)|*d),&variables[index],budget);
            let geometry=if let Some(meter)=meter.as_deref_mut() {meter.variables(&mut variable_budget,resolve)?} else if let Some(meter)=transform_meter.as_deref_mut() {meter.variables(&mut variable_budget,resolve)?} else {resolve(&mut variable_budget)};
            svg_geometry.try_reserve(1).map_err(|_|"SVG computed geometry snapshot reservation failed.")?;
            svg_geometry.insert(index,geometry.map(|mut geometry|{geometry.set_paint(style.svg_paint.clone(),style.svg_paint_known,0xff000000|style.color);geometry}));
        }
        let viewport_length=|value|sizing_math::resolve_viewport(value,environment);
        style.width=viewport_length(style.width);style.height=viewport_length(style.height);style.min_width=viewport_length(style.min_width);style.min_height=viewport_length(style.min_height);style.max_width=style.max_width.map(viewport_length);style.max_height=style.max_height.map(viewport_length);style.flex_basis=viewport_length(style.flex_basis);style.margin=style.margin.map(viewport_length);style.padding=style.padding.map(viewport_length);style.gap=style.gap.map(viewport_length);
        // Height inherits its computed length, not a parent's em/rem token.
        // Percentages and auto retain their containing-block/used-value role.
        if matches!(style.height, Length::Em(_) | Length::Rem(_)) {
            style.height = Length::Px(style.height.resolve(0.0,style.font_size,style.root_font_size).unwrap());
        }
        if has_radius {
            if let Some(meter) = meter.as_deref_mut() { meter.work(applicable.len())?; }
            let resolve_radius = |budget: &mut custom_properties::Budget| {
                radius::cascade(parent.border_radius, style.font_size, style.root_font_size,
                    applicable.iter().rev().map(|(priority, declaration)| (*declaration, priority.1)),
                    &variables[index], budget)
            };
            style.border_radius = if let Some(meter) = meter.as_deref_mut() {
                meter.variables(&mut variable_budget, resolve_radius)?
            } else { resolve_radius(&mut variable_budget) };
        }
        // Computed transform lengths use final font metrics, so inherited em
        // and rem translations retain the parent's absolute computed lengths.
        let resolve_transform = |budget: &mut custom_properties::Budget| {
            let candidates = applicable.iter().rev()
                .filter(|(_, declaration)| declaration.name == "transform")
                .map(|(priority, declaration)| (*declaration, priority.1));
            translate::cascade(parent.transform, parent_transform_known, style.font_size,
                style.root_font_size, candidates, &variables[index], budget)
        };
        let (computed_transform, known) = if !has_transform {
            (Transform::None, true)
        } else if let Some(meter) = transform_meter.as_deref_mut() {
            meter.variables(&mut variable_budget, resolve_transform)?
        } else if let Some(meter) = meter.as_deref_mut() {
            meter.variables(&mut variable_budget, resolve_transform)?
        } else {
            resolve_transform(&mut variable_budget)
        };
        style.transform = computed_transform;
        if !transform_known.is_empty() { transform_known[index] = known; }
        if style.display==Display::InlineFlex && (parent.display.flex_container() || parent.display==Display::Grid || matches!(style.position,Position::Absolute|Position::Fixed)) {style.display=Display::Flex;}
        // Grid children establish independent formatting contexts. Retain the
        // computed blockification for geometry and transform applicability too.
        if parent.display == Display::Grid && matches!(style.display,
            Display::Inline | Display::InlineBlock | Display::TableRowGroup | Display::TableRow | Display::TableCell)
        {
            style.display = Display::Block;
        }
        if crate::html_canvas::element(doc, index) {
            let first = canvas_foregrounds.is_empty();
            if let Some(meter) = meter.as_deref_mut() { meter.canvas_foreground(first)?; }
            else if let Some(meter) = transform_meter.as_deref_mut() { meter.canvas_foreground(first)?; }
            canvas_foregrounds.try_reserve(1).map_err(|_| "Canvas foreground snapshot reservation failed.")?;
            canvas_foregrounds.insert(index, style.color);
        }
        if !font_known || style.font_size != INITIAL_FONT_SIZE {
            if let Some(meter)=meter.as_deref_mut() {meter.font_size(font_sizes.is_empty())?;}
            else if let Some(meter)=transform_meter.as_deref_mut() {meter.font_size(font_sizes.is_empty())?;}
            font_sizes.try_reserve(1).map_err(|_|"Font-size snapshot reservation failed.")?;
            font_sizes.insert(index,font_known.then_some(style.font_size));
        }
        if touch_action != Some(TouchAction::AUTO) {
            if let Some(meter)=meter.as_deref_mut() {meter.touch_action(touch_actions.is_empty())?;}
            else if let Some(meter)=transform_meter.as_deref_mut() {meter.touch_action(touch_actions.is_empty())?;}
            touch_actions.try_reserve(1).map_err(|_|"Touch-action snapshot reservation failed.")?;
            touch_actions.insert(index,touch_action);
        }
        if !opacity_known || style.opacity != 1.0 {
            let first = opacities.is_empty();
            if let Some(meter) = meter.as_deref_mut() { meter.opacity(first)?; }
            else if let Some(meter) = transform_meter.as_deref_mut() { meter.opacity(first)?; }
            opacities.try_reserve(1).map_err(|_| "Opacity snapshot reservation failed.")?;
            opacities.insert(index, opacity_known.then_some(style.opacity));
        }
        computed[index] = style;
    }
    let overflows = computed.iter().zip(overflow_known).map(|(style, known)| known.iter().all(|known| *known).then_some(style.overflow)).collect();
    let positions = computed.iter().zip(position_known).map(|(style, known)| known.then_some(style.position)).collect();
    let mut has_transform_geometry = false;
    let has_transform_declarations = !transform_known.is_empty();
    let transforms = computed.iter().zip(transform_known).zip(&eligible).map(|((style, known), eligible)| {
        has_transform_geometry |= *eligible && (!known || !style.transform.is_none());
        known.then_some(style.transform)
    }).collect();
    if has_transform_declarations {
        has_transform_geometry |= !complete;
    }
    check_css_deadline(meter.as_deref().and_then(CssBudget::deadline)
        .or_else(|| transform_meter.as_deref().and_then(CssBudget::deadline)))?;
    let mut presentation=Vec::new();
    // The transform-only layout path keeps its existing no-property cost.
    // Presentation reads can request a funded complete snapshot separately.
    if transform_meter.is_none()||meter.is_some(){
        if let Some(meter)=meter.as_deref_mut(){meter.presentation(computed.len())?;}
        presentation.try_reserve_exact(computed.len()).map_err(|_|"Computed presentation snapshot reservation failed.")?;
        let extra=presentation.capacity().checked_sub(computed.len()).and_then(|n|n.checked_mul(std::mem::size_of::<computed_style::ComputedPresentation>())).ok_or("Computed presentation extra backing overflow.")?;
        if extra!=0{if let Some(meter)=meter.as_deref_mut(){meter.copies(extra)?;}}
        presentation.extend(computed.iter().map(|style|computed_style::ComputedPresentation{display:computed_style::style_display(style),color:style.color,visible:style.visible,pointer_events:style.pointer_events,z_index:style.z_index.clone()}));
    }
    Ok(CascadeOutput { has_hover:prepared.has_hover,styles: computed, computed_properties: ComputedProperties::new(variables, eligible, complete, overflows, positions, transforms, has_transform_geometry, canvas_foregrounds, opacities, touch_actions, font_sizes, presentation, svg_geometry), stylesheet_cache:prepared.cache })
}

fn prefix(text: &str, maximum: usize) -> &str {
    let mut end = text.len().min(maximum);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

fn user_agent(style: &mut Style, tag: &str, link: bool, hidden: bool) {
    if tag=="video" {style.object_fit=ObjectFit::Contain;}
    if matches!(
        tag,
        "#document"
            | "html"
            | "body"
            | "div"
            | "center"
            | "p"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "section"
            | "article"
            | "header"
            | "footer"
            | "main"
            | "nav"
            | "aside"
            | "ul"
            | "ol"
            | "li"
            | "blockquote"
            | "pre"
            | "hr"
            | "form"
            | "fieldset"
            | "figure"
            | "figcaption"
            | "address"
            | "dl"
            | "dt"
            | "dd"
            | "table"
            | "thead"
            | "tbody"
            | "tfoot"
            | "tr"
    ) {
        style.display = Display::Block;
    }
    if tag=="li"{style.display_alias=Some("list-item");}
    if hidden
        || matches!(
            tag,
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
        )
    {
        style.display = Display::None;
    }
    match tag {
        "center" => {
            style.align = Align::Center;
            style.legacy_align = Some(Align::Center);
        }
        "table" => style.display = Display::Table,
        "thead" | "tbody" | "tfoot" => {style.display=Display::TableRowGroup;style.display_alias=match tag{"thead"=>Some("table-header-group"),"tfoot"=>Some("table-footer-group"),_=>None};},
        "tr" => style.display = Display::TableRow,
        "td" | "th" => {
            style.display = Display::TableCell;
            style.padding = [Length::Px(1.0); 4];
        }
        "body" => style.margin = [Length::Px(8.0); 4],
        "p" | "pre" | "ul" | "ol" | "dl" | "figure" => {
            style.margin[0] = Length::Em(1.0);
            style.margin[2] = Length::Em(1.0);
        }
        "blockquote" => {
            style.margin = [
                Length::Em(1.0),
                Length::Px(40.0),
                Length::Em(1.0),
                Length::Px(40.0),
            ]
        }
        "dd" => style.margin[3] = Length::Px(40.0),
        "hr" => {
            style.margin[0] = Length::Px(8.0);
            style.margin[2] = Length::Px(8.0);
            style.border_width[0] = Length::Px(1.0);
            style.border_solid[0] = true;
            style.border_color[0] = Some(0xb9c2cc);
        }
        _ => {}
    }
    if matches!(tag, "ul" | "ol") {
        style.padding[3] = Length::Px(32.0);
        style.list_style = if tag == "ol" {
            ListStyle::Decimal
        } else {
            ListStyle::Disc
        };
    }
    if let Some(size) = match tag {
        "h1" => Some(32.0),
        "h2" => Some(24.0),
        "h3" => Some(20.0),
        "h4" => Some(16.0),
        "h5" => Some(14.0),
        "h6" => Some(12.0),
        _ => None,
    } {
        style.font_size = size;
        style.bold = true;
        style.font_weight = 700;
        style.margin[0] = Length::Em(0.65);
        style.margin[2] = Length::Em(0.5);
    }
    if matches!(tag, "strong" | "b" | "th") {
        style.bold = true;
        style.font_weight = 700;
    }
    if matches!(tag, "em" | "i" | "cite" | "address") {
        style.italic = true;
    }
    if tag == "u" {
        style.underline = true;
    }
    if tag == "small" {
        style.font_size *= 0.8;
    }
    if tag == "pre" {
        style.white_space = WhiteSpace::Pre;
    }
    if matches!(tag, "pre" | "code" | "kbd" | "samp" | "tt") {
        style.font_families = vec!["monospace".into()];
    }
    if tag == "a" && link {
        style.color = 0x1569b6;
        style.underline = true;
    }
}

pub(crate) fn without_comments(source: &str) -> String {
    let bytes = source.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut position = 0;
    let mut quote = 0;
    while position < bytes.len() {
        let byte = bytes[position];
        if quote != 0 {
            output.push(byte);
            position += 1;
            if byte == b'\\' && position < bytes.len() {
                output.push(bytes[position]);
                position += 1;
            } else if byte == quote {
                quote = 0;
            }
        } else if matches!(byte, b'\'' | b'"') {
            quote = byte;
            output.push(byte);
            position += 1;
        } else if byte == b'/' && bytes.get(position + 1) == Some(&b'*') {
            position += 2;
            while position + 1 < bytes.len() && &bytes[position..position + 2] != b"*/" {
                position += 1;
            }
            position = (position + 2).min(bytes.len());
            output.push(b' ');
        } else {
            output.push(byte);
            position += 1;
        }
    }
    String::from_utf8(output).unwrap_or_default()
}

#[cfg(test)]
fn parse_sheet(source:&str,rules:&mut Vec<Rule>) {
    let _=parse_sheet_with_media(source,rules,None,StylesheetLimits::default().rules);
}

fn parse_sheet_with_media(source:&str,rules:&mut Vec<Rule>,environment:Option<MediaEnvironment>,maximum_blocks:usize)->Result<(),String> {
    parse_sheet_with_media_inner(source,rules,environment,maximum_blocks,None,None).map_err(|error| match error {
        SheetParseFailure::Budget(message) | SheetParseFailure::Rejected(message) => message,
    })
}
enum SheetParseFailure { Budget(String), Rejected(String) }
fn parse_sheet_with_media_inner(source:&str,rules:&mut Vec<Rule>,environment:Option<MediaEnvironment>,maximum_blocks:usize,
    mut transform_meter:Option<&mut CssBudget<'_>>,deadline:Option<Instant>)->Result<(),SheetParseFailure> {
    let first=rules.len();
    let mut allocation_failed=false;
    let mut budget_failure=None;
    let mut blocks=0u8;
    let result=media::walk(source,maximum_blocks,environment,|prelude,body,active| {
        if blocks==0 {
            if let Err(error)=check_css_deadline(deadline) {
                budget_failure=Some(error);
                return false;
            }
        }
        blocks=blocks.wrapping_add(1);
        if active && !prelude.trim_start().starts_with('@') {
            if let Some(selectors)=parse_selector_list(prelude, true) {
                let declarations=match parse_declarations_inner(body,MAX_DECLARATIONS,true,transform_meter.as_deref_mut()) {
                    Ok(declarations)=>declarations,
                    Err(error)=>{budget_failure=Some(error);return false;}
                };
                if !selectors.is_empty() && !declarations.is_empty() {
                    if rules.try_reserve(1).is_err() {allocation_failed=true;return false;}
                    rules.push(Rule{selectors,declarations});
                }
            }
        }
        true
    });
    if let Some(error)=budget_failure.or_else(|| check_css_deadline(deadline).err()) {
        rules.truncate(first);
        return Err(SheetParseFailure::Budget(error));
    }
    if let Err(error)=result {rules.truncate(first);return Err(SheetParseFailure::Rejected(error));}
    if allocation_failed {rules.truncate(first);return Err(SheetParseFailure::Rejected("Stylesheet rule allocation failed.".into()));}
    Ok(())
}

#[derive(Clone, Debug)]
pub enum FontSource {
    Local(String),
    Url {
        url: String,
        format: Option<String>,
        unsupported_technology: bool,
    },
}

#[derive(Clone, Debug)]
pub struct FontFace {
    pub family: String,
    pub weight: u16,
    pub italic: bool,
    pub sources: Vec<FontSource>,
    pub ranges: Vec<(u32, u32)>,
    pub error: Option<String>,
}

/// Descriptors retain source order and URL case; the loader supplies the URL of
/// the stylesheet which owns them. Unsupported data never grants a fetch.
#[cfg(test)]
pub fn font_faces(source: &str) -> Vec<FontFace> {
    font_faces_with_limits(source,StylesheetLimits::default())
}
pub fn font_faces_with_limits(source:&str,limits:StylesheetLimits)->Vec<FontFace> {
    let mut faces = Vec::new();
    if limits.accept(source,StylesheetUsage::default()).is_err() {return faces;}
    sheet_blocks(source, |prelude, body| {
        if prelude.trim().eq_ignore_ascii_case("@font-face") {
            let mut face = FontFace {
                family: String::new(),
                weight: 400,
                italic: false,
                sources: Vec::new(),
                ranges: Vec::new(),
                error: None,
            };
            for declaration in parse_declarations(body) {
                if declaration.important {
                    continue;
                }
                let value = declaration.value.trim();
                match declaration.name.as_str() {
                    "font-family" => match font_families(value).filter(|names| names.len() == 1) {
                        Some(mut names) => face.family = names.remove(0),
                        None => face.error = Some("Invalid font-face family descriptor.".into()),
                    },
                    "font-weight" => {
                        let lower = value.to_ascii_lowercase();
                        match font_weight(&lower, 400) {
                            Some(weight) if !matches!(lower.as_str(), "inherit" | "unset" | "initial" | "bolder" | "lighter") => face.weight = weight,
                            _ => face.error = Some("Font-face weight ranges/variables are not implemented; use a single weight.".into()),
                        }
                    }
                    "font-style" => match value.to_ascii_lowercase().as_str() {
                        "normal" => face.italic = false,
                        "italic" | "oblique" => face.italic = true,
                        _ => {
                            face.error =
                                Some("Font-face style ranges/angles are not implemented.".into())
                        }
                    },
                    "font-stretch" if !value.eq_ignore_ascii_case("normal") && value != "100%" => {
                        face.error =
                            Some("Font-face width/stretch matching is not implemented.".into());
                    }
                    "src" => match font_sources(value) {
                        Some(sources) => face.sources = sources,
                        None => {
                            face.error =
                                Some("Invalid or unsupported font-face src descriptor.".into())
                        }
                    },
                    "unicode-range" => match unicode_ranges(value) {
                        Some(ranges) => face.ranges = ranges,
                        None => {
                            face.error = Some(
                                "Invalid or oversized font-face unicode-range descriptor.".into(),
                            )
                        }
                    },
                    _ => {}
                }
            }
            if face.family.is_empty() || face.sources.is_empty() {
                face.error = Some("Font-face requires a family and a supported src list.".into());
            }
            faces.push(face);
        }
        faces.len() < 64
    });
    faces
}

fn font_sources(value: &str) -> Option<Vec<FontSource>> {
    let mut sources = Vec::new();
    for source in css_list(value)? {
        if sources.len() == 8 {
            return None;
        }
        let (name, argument, mut rest) = css_function(source)?;
        if name.eq_ignore_ascii_case("local") {
            if !rest.trim().is_empty() {
                return None;
            }
            let mut family = font_families(argument)?;
            if family.len() != 1 {
                return None;
            }
            sources.push(FontSource::Local(family.remove(0)));
        } else if name.eq_ignore_ascii_case("url") {
            let argument = argument.trim();
            let url = if argument.starts_with(['\'', '"']) {
                css_string(argument)?
            } else {
                if argument
                    .chars()
                    .any(|ch| ch.is_ascii_whitespace() || matches!(ch, '\'' | '"' | '(' | ')'))
                {
                    return None;
                }
                css_unescape(argument)?
            };
            if url.is_empty() || url.len() > 2048 {
                return None;
            }
            let mut format = None;
            let mut unsupported_technology = false;
            while !rest.trim().is_empty() {
                let (name, argument, tail) = css_function(rest)?;
                if name.eq_ignore_ascii_case("format") && format.is_none() {
                    format = Some(
                        if argument.trim().starts_with(['\'', '"']) {
                            css_string(argument.trim())?
                        } else {
                            argument.trim().to_owned()
                        }
                        .to_ascii_lowercase(),
                    );
                } else if name.eq_ignore_ascii_case("tech") {
                    unsupported_technology = true;
                } else {
                    return None;
                }
                rest = tail;
            }
            sources.push(FontSource::Url {
                url,
                format,
                unsupported_technology,
            });
        } else {
            return None;
        }
    }
    (!sources.is_empty()).then_some(sources)
}

// This first layer accepts only one URL function. Unsupported layers, image
// functions and trailing shorthand tokens do not replace an earlier valid URL.
pub(crate) fn background_image_url(value: &str) -> Option<String> {
    let value = value.trim();
    if !value.get(..4)?.eq_ignore_ascii_case("url(") {
        return None;
    }
    let (name, argument, rest) = css_function(value)?;
    if !name.eq_ignore_ascii_case("url") || !rest.trim().is_empty() {
        return None;
    }
    let argument = argument.trim();
    let url = if argument.starts_with(['\'', '"']) {
        css_string(argument)?
    } else {
        if argument.chars().any(|ch| {
            ch.is_ascii_whitespace() || ch.is_control() || matches!(ch, '\'' | '"' | '(' | ')')
        }) {
            return None;
        }
        css_unescape(argument)?
    };
    (!url.is_empty() && url.len() <= 2048 && !url.chars().any(char::is_control))
        .then_some(url)
}

fn unicode_ranges(value: &str) -> Option<Vec<(u32, u32)>> {
    let mut ranges = Vec::new();
    for range in value.split(',') {
        if ranges.len() == 128 {
            return None;
        }
        let value = range.trim();
        if !value.get(..2)?.eq_ignore_ascii_case("u+") {
            return None;
        }
        let value = &value[2..];
        let parse = |part: &str| {
            (!part.is_empty()
                && part.len() <= 6
                && part.bytes().all(|byte| byte.is_ascii_hexdigit()))
            .then(|| u32::from_str_radix(part, 16).ok())
            .flatten()
        };
        let (start, end) = if let Some((start, end)) = value.split_once('-') {
            (parse(start)?, parse(end)?)
        } else if value.contains('?') {
            if value.len() > 6 || value.is_empty() {
                return None;
            }
            let first = value.find('?')?;
            if !value[first..].bytes().all(|byte| byte == b'?') {
                return None;
            }
            (
                parse(&value.replace('?', "0"))?,
                parse(&value.replace('?', "F"))?,
            )
        } else {
            let code = parse(value)?;
            (code, code)
        };
        if start > end || end > 0x10ffff {
            return None;
        }
        ranges.push((start, end));
    }
    (!ranges.is_empty()).then_some(ranges)
}

fn css_list(value: &str) -> Option<Vec<&str>> {
    let mut pieces = Vec::new();
    let mut start = 0;
    let mut quote = '\0';
    let mut escaped = false;
    let mut depth = 0usize;
    for (index, ch) in value.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if quote != '\0' {
            if ch == quote {
                quote = '\0';
            }
            continue;
        }
        match ch {
            '\'' | '"' => quote = ch,
            '(' => depth += 1,
            ')' => {
                depth = depth.checked_sub(1)?;
            }
            ',' if depth == 0 => {
                let piece = value[start..index].trim();
                if piece.is_empty() {
                    return None;
                }
                pieces.push(piece);
                start = index + 1;
            }
            _ => {}
        }
    }
    if quote != '\0' || escaped || depth != 0 {
        return None;
    }
    let last = value[start..].trim();
    if last.is_empty() {
        return None;
    }
    pieces.push(last);
    Some(pieces)
}

fn css_function(value: &str) -> Option<(&str, &str, &str)> {
    let value = value.trim();
    let open = value.find('(')?;
    let name = value[..open].trim();
    if name.is_empty()
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphabetic() || byte == b'-')
    {
        return None;
    }
    let mut quote = '\0';
    let mut escaped = false;
    for (index, ch) in value[open + 1..].char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if quote != '\0' {
            if ch == quote {
                quote = '\0';
            }
            continue;
        }
        match ch {
            '\'' | '"' => quote = ch,
            '(' => return None,
            ')' => {
                let end = open + 1 + index;
                return Some((name, &value[open + 1..end], &value[end + 1..]));
            }
            _ => {}
        }
    }
    None
}

fn css_string(value: &str) -> Option<String> {
    let quote = value.chars().next()?;
    if !matches!(quote, '\'' | '"') || value.len() < 2 || !value.ends_with(quote) {
        return None;
    }
    let body = &value[1..value.len() - 1];
    let mut escaped = false;
    for ch in body.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
        } else if ch == quote || matches!(ch, '\n' | '\r' | '\u{c}') {
            return None;
        }
    }
    if escaped {
        return None;
    }
    css_unescape(body)
}

fn css_unescape(value: &str) -> Option<String> {
    let mut out = String::new();
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        let ch = chars.next()?;
        if ch.is_ascii_hexdigit() {
            let mut number = ch.to_digit(16)?;
            for _ in 1..6 {
                if !chars.peek().is_some_and(|ch| ch.is_ascii_hexdigit()) {
                    break;
                }
                number = number * 16 + chars.next()?.to_digit(16)?;
            }
            if chars.peek().is_some_and(char::is_ascii_whitespace) {
                let whitespace = chars.next();
                if whitespace == Some('\r') && chars.peek() == Some(&'\n') {
                    chars.next();
                }
            }
            out.push(
                char::from_u32(number)
                    .filter(|ch| *ch != '\0')
                    .unwrap_or('\u{fffd}'),
            );
        } else if ch == '\r' {
            if chars.peek() == Some(&'\n') {
                chars.next();
            }
        } else if !matches!(ch, '\n' | '\u{c}') {
            out.push(ch);
        }
    }
    Some(out)
}

fn font_families(value: &str) -> Option<Vec<String>> {
    let mut names = Vec::new();
    for family in css_list(value)? {
        if names.len() == 16 {
            return None;
        }
        let name = if family.starts_with(['\'', '"']) {
            css_string(family)?
        } else {
            if family
                .chars()
                .any(|ch| matches!(ch, '\'' | '"' | '(' | ')' | '/' | ';' | ':' | '{' | '}'))
            {
                return None;
            }
            let name = css_unescape(family)?;
            if name.chars().next().is_some_and(|ch| ch.is_ascii_digit()) {
                return None;
            }
            name.split_ascii_whitespace().collect::<Vec<_>>().join(" ")
        };
        if name.is_empty() || name.len() > 256 {
            return None;
        }
        names.push(name);
    }
    Some(names)
}

fn font_weight(value: &str, parent: u16) -> Option<u16> {
    match value {
        "normal" | "initial" => Some(400),
        "bold" => Some(700),
        "inherit" | "unset" => Some(parent),
        "bolder" => Some(if parent < 350 {
            400
        } else if parent < 550 {
            700
        } else {
            900
        }),
        "lighter" => Some(if parent < 550 {
            100
        } else if parent < 750 {
            400
        } else {
            700
        }),
        _ => value
            .parse::<u16>()
            .ok()
            .filter(|weight| (1..=1000).contains(weight)),
    }
}

type FontShorthand = (String, Option<String>, Vec<String>, u16, bool);
fn font_shorthand(value: &str, parent_weight: u16) -> Option<FontShorthand> {
    let bytes = value.as_bytes();
    let mut position = 0;
    let mut weight = 400;
    let mut italic = false;
    let mut weight_seen = false;
    let mut style_seen = false;
    let mut tokens = 0;
    loop {
        while bytes.get(position).is_some_and(u8::is_ascii_whitespace) {
            position += 1;
        }
        let start = position;
        while bytes
            .get(position)
            .is_some_and(|byte| !byte.is_ascii_whitespace() && *byte != b'/')
        {
            position += 1;
        }
        if position == start || tokens > 5 {
            return None;
        }
        tokens += 1;
        let token = value[start..position].to_ascii_lowercase();
        let is_size = matches!(
            token.as_str(),
            "xx-small"
                | "x-small"
                | "small"
                | "medium"
                | "large"
                | "x-large"
                | "xx-large"
                | "smaller"
                | "larger"
        ) || parse_length(&token, false, false).is_some();
        if is_size {
            while bytes.get(position).is_some_and(u8::is_ascii_whitespace) {
                position += 1;
            }
            let line = if bytes.get(position) == Some(&b'/') {
                position += 1;
                while bytes.get(position).is_some_and(u8::is_ascii_whitespace) {
                    position += 1;
                }
                let start = position;
                while bytes
                    .get(position)
                    .is_some_and(|byte| !byte.is_ascii_whitespace())
                {
                    position += 1;
                }
                let line = value[start..position].to_ascii_lowercase();
                if line != "normal"
                    && !line
                        .parse::<f32>()
                        .ok()
                        .is_some_and(|n| n.is_finite() && n >= 0.0)
                    && parse_length(&line, false, false).is_none()
                {
                    return None;
                }
                Some(line)
            } else {
                None
            };
            let families = font_families(value[position..].trim())?;
            return Some((token, line, families, weight, italic));
        }
        match token.as_str() {
            "normal" | "small-caps" => {}
            "italic" | "oblique" if !style_seen => {
                italic = true;
                style_seen = true;
            }
            _ if !weight_seen && !matches!(token.as_str(), "inherit" | "unset" | "initial") => {
                weight = font_weight(&token, parent_weight)?;
                weight_seen = true;
            }
            _ => return None,
        }
    }
}

// Only top-level blocks are enabled. Conditional rules stay outside the current
// screen/all stylesheet subset, consistently for style rules and font faces.
fn sheet_blocks(source: &str, mut visit: impl FnMut(&str, &str) -> bool) {
    let text = without_comments(source);
    let bytes = text.as_bytes();
    let mut position = 0;
    while position < bytes.len() {
        while bytes.get(position).is_some_and(u8::is_ascii_whitespace) {
            position += 1;
        }
        if position >= bytes.len() {
            break;
        }
        let start = position;
        let mut quote = 0;
        let mut parentheses = 0usize;
        while position < bytes.len() {
            let byte = bytes[position];
            if quote != 0 {
                if byte == b'\\' {
                    position = (position + 2).min(bytes.len());
                    continue;
                }
                if byte == quote {
                    quote = 0;
                }
            } else if matches!(byte, b'\'' | b'"') {
                quote = byte;
            } else if byte == b'(' {
                parentheses = parentheses.saturating_add(1);
            } else if byte == b')' {
                parentheses = parentheses.saturating_sub(1);
            } else if parentheses == 0 && (byte == b'{' || byte == b';') {
                break;
            }
            position += 1;
        }
        if position >= bytes.len() {
            break;
        }
        if bytes[position] == b';' {
            position += 1;
            continue;
        }
        let prelude_end = position;
        position += 1;
        let body_start = position;
        let mut depth = 1usize;
        quote = 0;
        while position < bytes.len() && depth != 0 {
            let byte = bytes[position];
            if quote != 0 {
                if byte == b'\\' {
                    position = (position + 2).min(bytes.len());
                    continue;
                }
                if byte == quote {
                    quote = 0;
                }
            } else if matches!(byte, b'\'' | b'"') {
                quote = byte;
            } else if byte == b'{' {
                depth += 1;
            } else if byte == b'}' {
                depth -= 1;
            }
            if depth != 0 {
                position += 1;
            }
        }
        let body_end = position;
        position = (position + 1).min(bytes.len());
        if !visit(&text[start..prelude_end], &text[body_start..body_end]) {
            break;
        }
    }
}

fn parse_selector_list(source: &str, pseudo_targets: bool) -> Option<Vec<Selector>> {
    parse_selector_list_at_depth(source,pseudo_targets,0)
}

fn parse_selector_list_at_depth(source:&str,pseudo_targets:bool,depth:usize)->Option<Vec<Selector>> {
    if depth>MAX_SELECTOR_NESTING || source.len()>MAX_SELECTOR_BYTES*16 {return None;}
    let mut selectors = Vec::new();
    let mut start = 0;
    let mut bracket = false;
    let mut quote = None;
    let mut parentheses=0usize;
    for (position, byte) in source.bytes().enumerate() {
        if let Some(expected) = quote {
            if byte == b'\\' {
                return None; // Escaped selector strings are outside this subset.
            }
            if byte == expected {
                quote = None;
            }
            continue;
        }
        match byte {
            b'[' if !bracket => bracket = true,
            b']' if bracket => bracket = false,
            b'[' | b']' => return None,
            b'\'' | b'"' if bracket => quote = Some(byte),
            b'(' if !bracket => {
                parentheses+=1;
                if parentheses>MAX_SELECTOR_NESTING {return None;}
            }
            b')' if !bracket => {parentheses=parentheses.checked_sub(1)?;}
            b',' if !bracket && parentheses==0 => {
                if selectors.len() == 15 {
                    return None;
                }
                let selector=parse_selector_at_depth(&source[start..position],depth)?;
                if !pseudo_targets && selector.target!=SelectorTarget::Element {return None;}
                selectors.push(selector);
                start = position + 1;
            }
            _ => {}
        }
    }
    if bracket || quote.is_some() || parentheses!=0 {
        return None;
    }
    let selector=parse_selector_at_depth(&source[start..],depth)?;
    if !pseudo_targets && selector.target!=SelectorTarget::Element {return None;}
    selectors.push(selector);
    Some(selectors)
}

fn parse_selector(source: &str) -> Option<Selector> {
    parse_selector_at_depth(source,0)
}

fn parse_selector_at_depth(source:&str,depth:usize)->Option<Selector> {
    if depth>MAX_SELECTOR_NESTING {return None;}
    let source = source.trim_matches(|character: char| character.is_ascii_whitespace());
    if source.is_empty() || source.len() > MAX_SELECTOR_BYTES {
        return None;
    }
    let bytes = source.as_bytes();
    let mut position = 0;
    let mut parts = Vec::new();
    let mut relations = Vec::new();
    let mut specificity = (0u16, 0u16, 0u16);
    let mut target=SelectorTarget::Element;
    while position < bytes.len() && parts.len() < 12 {
        let compound_start = position;
        let mut part = Compound {
            tag: None,
            ids: Vec::new(),
            classes: Vec::new(),
            attributes: Vec::new(),
            root: false,
            checked: false,
            focus:false,
            focus_within:false,
            hover:false,
            disabled: false,
            negations: Vec::new(),
            structural:Vec::new(),
        };
        if bytes[position] == b'*' {
            position += 1;
        } else if starts_selector_ident(bytes, position) {
            let start = position;
            while bytes.get(position).is_some_and(|b| ident(*b)) {
                position += 1;
            }
            part.tag = Some(source[start..position].to_owned());
            specificity.2 += 1;
        }
        loop {
            match bytes.get(position) {
                Some(b'.' | b'#') => {
                    let id = bytes[position] == b'#';
                    position += 1;
                    if !starts_selector_ident(bytes, position) {
                        return None;
                    }
                    let start = position;
                    while bytes.get(position).is_some_and(|b| ident(*b)) {
                        position += 1;
                    }
                    if start == position {
                        return None;
                    }
                    if id {
                        part.ids.push(source[start..position].to_owned());
                        specificity.0 += 1;
                    } else {
                        part.classes.push(source[start..position].to_owned());
                        specificity.1 += 1;
                    }
                }
                Some(b'[') => {
                    part.attributes
                        .push(parse_attribute(source, &mut position)?);
                    specificity.1 += 1;
                }
                Some(b':') => {
                    position += 1;
                    if bytes.get(position)==Some(&b':') {
                        position+=1;
                        let start=position;
                        while bytes.get(position).is_some_and(|b|ident(*b)) {position+=1;}
                        target=if source[start..position].eq_ignore_ascii_case("before") {SelectorTarget::Before}
                            else if source[start..position].eq_ignore_ascii_case("after") {SelectorTarget::After}
                            else {return None;};
                        // This finite slice admits only a terminal pseudo-element.
                        // It never repairs a malformed suffix or later combinator.
                        if bytes[position..].iter().any(|b|!b.is_ascii_whitespace()) {return None;}
                        specificity.2+=1;
                        break;
                    }
                    let start = position;
                    while bytes.get(position).is_some_and(|b| ident(*b)) {
                        position += 1;
                    }
                    let name=&source[start..position];
                    if name.eq_ignore_ascii_case("not") {
                        let arguments=parse_negation_arguments(source,&mut position,depth)?;
                        let maximum=arguments.iter().map(|selector|selector.specificity).max()?;
                        specificity.0=specificity.0.checked_add(maximum.0)?;
                        specificity.1=specificity.1.checked_add(maximum.1)?;
                        specificity.2=specificity.2.checked_add(maximum.2)?;
                        part.negations.push(arguments);
                    } else if structural::name(name) {
                        part.structural.push(structural::parse(name,source,&mut position)?);specificity.1+=1;
                    } else if name.eq_ignore_ascii_case("root") {
                        part.root=true;specificity.1+=1;
                    } else if name.eq_ignore_ascii_case("hover") {
                        part.hover=true;specificity.1+=1;
                    } else if name.eq_ignore_ascii_case("focus") {
                        part.focus=true;specificity.1+=1;
                    } else if name.eq_ignore_ascii_case("focus-within") {
                        part.focus_within=true;specificity.1+=1;
                    } else if name.eq_ignore_ascii_case("checked") {
                        part.checked=true;specificity.1+=1;
                    } else if name.eq_ignore_ascii_case("disabled") {
                        part.disabled=true;specificity.1+=1;
                    } else {
                        return None;
                    }
                }
                _ => break,
            }
        }
        if compound_start == position {
            return None;
        }
        parts.push(part);
        let before_space = position;
        while bytes.get(position).is_some_and(u8::is_ascii_whitespace) {
            position += 1;
        }
        if position == bytes.len() {
            break;
        }
        if matches!(bytes[position], b'>' | b'+' | b'~') {
            let relation = match bytes[position] {
                b'>' => Relation::Child,
                b'+' => Relation::AdjacentSibling,
                _ => Relation::GeneralSibling,
            };
            position += 1;
            while bytes.get(position).is_some_and(u8::is_ascii_whitespace) {
                position += 1;
            }
            if position == bytes.len() {
                return None;
            }
            relations.push(relation);
        } else if position > before_space {
            relations.push(Relation::Descendant);
        } else {
            return None;
        }
    }
    if position != bytes.len() || relations.len() + 1 != parts.len() {
        return None;
    }
    Some(Selector {
        target,
        parts,
        relations,
        specificity,
    })
}

// The same bounded selector grammar is admitted inside a strict :not list.
// Pseudo-elements and malformed/unsupported members invalidate the whole list.
fn parse_negation_arguments(
    source: &str,
    position: &mut usize,
    depth: usize,
) -> Option<Vec<Selector>> {
    if depth == MAX_SELECTOR_NESTING || source.as_bytes().get(*position) != Some(&b'(') {
        return None;
    }
    *position += 1;
    let start = *position;
    let mut parentheses = 1usize;
    let mut bracket = false;
    let mut quote = None;
    while let Some(&byte) = source.as_bytes().get(*position) {
        if let Some(expected) = quote {
            if byte == b'\\' {
                return None;
            }
            if byte == expected {
                quote = None;
            }
        } else {
            match byte {
                b'[' if !bracket => bracket = true,
                b']' if bracket => bracket = false,
                b'[' | b']' => return None,
                b'\'' | b'"' if bracket => quote = Some(byte),
                b'(' if !bracket => {
                    parentheses += 1;
                    if parentheses > MAX_SELECTOR_NESTING {
                        return None;
                    }
                }
                b')' if !bracket => {
                    parentheses -= 1;
                    if parentheses == 0 {
                        let arguments = parse_selector_list_at_depth(
                            &source[start..*position],
                            false,
                            depth + 1,
                        )?;
                        *position += 1;
                        return Some(arguments);
                    }
                }
                _ => {}
            }
        }
        *position += 1;
    }
    None
}

fn parse_attribute(source: &str, position: &mut usize) -> Option<AttributeSelector> {
    let bytes = source.as_bytes();
    *position += 1; // '['
    skip_selector_space(bytes, position);
    let start = *position;
    if !starts_selector_ident(bytes, *position) {
        return None;
    }
    while bytes.get(*position).is_some_and(|b| ident(*b)) {
        *position += 1;
    }
    let name = source[start..*position].to_ascii_lowercase();
    let mut ascii_insensitive = html_attribute_insensitive(&name);
    skip_selector_space(bytes, position);
    if bytes.get(*position) == Some(&b']') {
        *position += 1;
        return Some(AttributeSelector {
            name,
            value: None,
            operator:AttributeOperator::Exact,
            ascii_insensitive,
            substring_prefix: Vec::new(),
        });
    }
    // An operator is contiguous. Namespace syntax and malformed operators must
    // still reject the entire selector, rather than broaden an attribute test.
    let operator = match bytes.get(*position) {
        Some(b'~') => AttributeOperator::Token,
        Some(b'|') => AttributeOperator::Dash,
        Some(b'^') => AttributeOperator::Prefix,
        Some(b'$') => AttributeOperator::Suffix,
        Some(b'*') => AttributeOperator::Substring,
        _ => AttributeOperator::Exact,
    };
    if !matches!(operator, AttributeOperator::Exact) { *position += 1; }
    if bytes.get(*position) != Some(&b'=') {
        return None;
    }
    *position += 1;
    skip_selector_space(bytes, position);
    let value = if let Some(&quote @ (b'\'' | b'"')) = bytes.get(*position) {
        *position += 1;
        let start = *position;
        while bytes.get(*position).is_some_and(|byte| *byte != quote) {
            let byte = bytes[*position];
            if byte == b'\\' || byte < b' ' && byte != b'\t' {
                return None;
            }
            *position += 1;
        }
        if bytes.get(*position) != Some(&quote) {
            return None;
        }
        let value = source[start..*position].to_owned();
        *position += 1;
        value
    } else {
        let start = *position;
        if !starts_selector_ident(bytes, *position) {
            return None;
        }
        while bytes.get(*position).is_some_and(|b| ident(*b)) {
            *position += 1;
        }
        source[start..*position].to_owned()
    };
    skip_selector_space(bytes, position);
    if let Some(byte) = bytes.get(*position) {
        match byte.to_ascii_lowercase() {
            b'i' => ascii_insensitive = true,
            b's' => ascii_insensitive = false,
            b']' => {}
            _ => return None,
        }
        if *byte != b']' {
            *position += 1;
            skip_selector_space(bytes, position);
        }
    }
    if bytes.get(*position) != Some(&b']') {
        return None;
    }
    *position += 1;
    let substring_prefix = if matches!(operator, AttributeOperator::Substring) {
        attribute_substrings::prefix_table(&value, ascii_insensitive)?
    } else {
        Vec::new()
    };
    Some(AttributeSelector {
        name,
        value: Some(value),
        operator,
        ascii_insensitive,
        substring_prefix,
    })
}

fn skip_selector_space(bytes: &[u8], position: &mut usize) {
    while bytes.get(*position).is_some_and(u8::is_ascii_whitespace) {
        *position += 1;
    }
}

fn starts_selector_ident(bytes: &[u8], position: usize) -> bool {
    match bytes.get(position) {
        Some(b'-') => bytes
            .get(position + 1)
            .is_some_and(|byte| ident_start(*byte)),
        Some(byte) => byte.is_ascii_alphabetic() || *byte == b'_',
        None => false,
    }
}

fn html_attribute_insensitive(name: &str) -> bool {
    // HTML's attribute-selector comparison rules apply regardless of whether
    // an attribute is meaningful on the particular HTML element.
    matches!(
        name,
        "accept"
            | "accept-charset"
            | "align"
            | "alink"
            | "axis"
            | "bgcolor"
            | "charset"
            | "checked"
            | "clear"
            | "codetype"
            | "color"
            | "compact"
            | "declare"
            | "defer"
            | "dir"
            | "direction"
            | "disabled"
            | "enctype"
            | "face"
            | "frame"
            | "hreflang"
            | "http-equiv"
            | "lang"
            | "language"
            | "link"
            | "media"
            | "method"
            | "multiple"
            | "nohref"
            | "noresize"
            | "noshade"
            | "nowrap"
            | "readonly"
            | "rel"
            | "rev"
            | "rules"
            | "scope"
            | "scrolling"
            | "selected"
            | "shape"
            | "target"
            | "text"
            | "type"
            | "valign"
            | "valuetype"
            | "vlink"
    )
}

fn ident_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte>=0x80 || matches!(byte, b'_' | b'-')
}
fn ident(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte>=0x80 || matches!(byte, b'_' | b'-')
}

struct MatchContext<'a> {
    doc: &'a Document,
    parents: &'a [Option<usize>],
    previous_elements: &'a [Option<usize>],
    // Browser queries reuse checked native topology instead of allocating a
    // second whole-document previous-sibling table. Style passes use the table.
    sibling_slots:Option<&'a [usize]>,
    checked: Option<&'a dyn Fn(usize) -> Option<bool>>,
    // Query jobs share their finite host-work allowance with string matching.
    // Style computation retains its existing node/compound accounting.
    charge_text: bool,
}

impl<'a> MatchContext<'a> {
    fn focus_matches(&self,node:usize,within:bool,budget:&mut usize)->bool{self.interaction_matches(self.doc.native_focus.get(),node,within,budget)}
    fn interaction_matches(&self,target:Option<usize>,node:usize,within:bool,budget:&mut usize)->bool{
        let Some(focus)=target else{return false;};
        if !within && focus!=node{return false;}
        if !self.doc.nodes.get(focus).is_some_and(|n|matches!(n.kind,NodeKind::Element{..})){return false;}
        let(mut current,mut contains)=(Some(focus),false);
        for _ in 0..self.doc.nodes.len(){
            if *budget==0{return false;}*budget-=1;
            let Some(index)=current else{return false;};contains|=index==node;
            if index==0{return contains;}
            current=self.parents.get(index).copied().flatten();
        }
        false
    }

    fn previous_element(&self,node:usize,budget:&mut usize)->Option<usize> {
        let Some(slots)=self.sibling_slots else{return self.previous_elements.get(node).copied().flatten();};
        let parent=self.parents.get(node).copied().flatten()?;
        let slot=*slots.get(node)?;
        let children=&self.doc.nodes.get(parent)?.children;
        for &child in children.get(..slot)?.iter().rev() {
            if *budget==0{return None;}*budget-=1;
            if matches!(self.doc.nodes.get(child)?.kind,NodeKind::Element{..}){return Some(child);}
        }
        None
    }

    fn text_work(&self, bytes: usize, budget: &mut usize) -> bool {
        if !self.charge_text {
            return true;
        }
        // One work unit covers at most 64 inspected bytes. This caps selector
        // string work below 6.4 MB for a 100000-unit job, while short attribute
        // checks still cost at least one unit. Charge before inspecting bytes.
        let cost = bytes.max(1).div_ceil(64);
        if cost > *budget {
            *budget = 0;
            return false;
        }
        *budget -= cost;
        true
    }

    fn equal(&self, actual: &str, expected: &str, insensitive: bool, budget: &mut usize) -> bool {
        // Unequal lengths terminate string equality without scanning either
        // value. Equal lengths may inspect every byte.
        let cost = if actual.len() == expected.len() {
            actual.len()
        } else {
            1
        };
        self.text_work(cost, budget)
            && if insensitive {
                actual.eq_ignore_ascii_case(expected)
            } else {
                actual == expected
            }
    }

    fn attribute(&self,node:usize,name:&str,budget:&mut usize)->Option<&'a str>{self.dom_attribute(node,name,budget).map(crate::dom_string::DomString::render_projection)}

    fn dom_attribute(&self, node: usize, name: &str, budget: &mut usize) -> Option<&'a crate::dom_string::DomString> {
        if !self.charge_text {
            return self.doc.dom_attr(node, name);
        }
        let NodeKind::Element { attrs, .. } = &self.doc.nodes.get(node)?.kind else {
            return None;
        };
        for attr in attrs {
            if attr.name.namespace.is_empty() && self.equal(&attr.name.qualified, name, false, budget) {
                return Some(&attr.value);
            }
            if *budget == 0 {
                return None;
            }
        }
        None
    }
}

fn matches_selector(
    selector: &Selector,
    node: usize,
    context: &MatchContext<'_>,
    budget: &mut usize,
) -> bool {
    if selector.target!=SelectorTarget::Element {
        // Recognizing a valid pseudo target preserves atomic style lists; no
        // generated node exists in this renderer and none matches an Element.
        if *budget!=0 {*budget-=1;}
        return false;
    }
    fn at(
        selector: &Selector,
        part: usize,
        node: usize,
        context: &MatchContext<'_>,
        budget: &mut usize,
    ) -> bool {
        if *budget == 0 {
            return false;
        }
        *budget -= 1;
        let Some(crate::html::Node {
            kind: NodeKind::Element { tag, id, expanded_name, .. },
            namespace,
            ..
        }) = context.doc.nodes.get(node)
        else {
            return false;
        };
        let compound = &selector.parts[part];
        if compound.root {
            // :root is the document element, not every parentless element or
            // every HTML-tagged node. The parser's node0 is a synthetic Element;
            // fragment/context arenas must not acquire document-root semantics.
            // Existing parent/sibling tables identify the first real Element in
            // physical document order without another walk or retained vector.
            // https://drafts.csswg.org/selectors/#the-root-pseudo
            let document = context.doc.nodes.first().is_some_and(|root| {
                root.namespace == crate::html::Namespace::None
                    && matches!(&root.kind, NodeKind::Element { tag, .. } if tag == "#document")
            });
            if !document || node == 0
                || context.parents.get(node) != Some(&Some(0))
                || context.previous_element(node,budget).is_some()
            {
                return false;
            }
        }
        if compound
            .tag
            .as_ref()
            .is_some_and(|expected| !context.text_work(expanded_name.first().and_then(|n|n.local.as_ref()).map_or_else(||if tag.len()==expected.len(){tag.len()}else{1},|local|local.used_bytes().saturating_add(expected.len())),budget)||!crate::html::element_names::local_matches(tag,expanded_name,expected,*namespace==crate::html::Namespace::Html&&context.doc.kind==crate::html::DocumentKind::Html))
        {
            return false;
        }
        if compound.ids.iter().any(|expected| {
            !id.as_ref()
                .is_some_and(|id| context.equal(id, expected, false, budget))
        }) {
            return false;
        }
        if !compound.classes.is_empty() {
            let class = context.attribute(node, "class", budget).unwrap_or("");
            for expected in &compound.classes {
                // Splitting scans the attribute, and equality can scan matched
                // length tokens again. Repeated classes and ancestor attempts
                // pay this cost again against the same caller-owned budget.
                let cost = class.len().saturating_mul(2).saturating_add(expected.len());
                if !context.text_work(cost, budget)
                    || !class
                        .split_ascii_whitespace()
                        .any(|actual| actual == expected)
                {
                    return false;
                }
            }
        }
        for attribute in &compound.attributes {
            let Some(native) = context.dom_attribute(node, &attribute.name, budget) else {
                return false;
            };
            if let Some(expected) = &attribute.value {
                if let Some(units)=native.raw_units(){
                    if !attribute_substrings::matches_units(attribute,units,expected,context,budget){return false;}
                    continue;
                }
                let actual=native.render_projection();
                let matches=match attribute.operator {
                    AttributeOperator::Exact=>context.equal(actual, expected, attribute.ascii_insensitive, budget),
                    AttributeOperator::Token=>{
                        // Pay for token scanning/comparisons and expected-token
                        // validation before scanning borrowed text. No token Vec.
                        let cost=actual.len().saturating_mul(2).saturating_add(expected.len());
                        context.text_work(cost,budget) && !expected.is_empty()
                            && !expected.as_bytes().iter().any(u8::is_ascii_whitespace)
                            && actual.split_ascii_whitespace().any(|token|if attribute.ascii_insensitive {token.eq_ignore_ascii_case(expected)}else{token==expected})
                    }
                    _ => attribute_substrings::matches(attribute, actual, expected, context, budget),
                };
                if !matches {
                    return false;
                }
            }
        }
        if compound.hover && !context.interaction_matches(context.doc.native_hover.get(),node,true,budget){return false;}
        if compound.focus {if !context.focus_matches(node,false,budget){return false;}}
        else if compound.focus_within && !context.focus_matches(node,true,budget){return false;}
        for predicate in &compound.structural{if !predicate.matches(context,node,budget){return false;}}
        if compound.checked {
            let initial = match tag.as_str() {
                "input"
                    if context.attribute(node, "type", budget).is_some_and(|kind| {
                        context.equal(kind, "checkbox", true, budget)
                            || context.equal(kind, "radio", true, budget)
                    }) =>
                {
                    context.attribute(node, "checked", budget).is_some()
                }
                "option" => context.attribute(node, "selected", budget).is_some(),
                _ => return false,
            };
            if !context
                .checked
                .and_then(|checked| checked(node))
                .unwrap_or(initial)
            {
                return false;
            }
        }
        if compound.disabled {
            let disabled=context.actually_disabled(node,budget);
            if !disabled || *budget==0 {return false;}
        }
        for arguments in &compound.negations {
            let excluded=arguments.iter().any(|argument|matches_selector(argument,node,context,budget));
            // Exhaustion is not evidence that a negated selector failed.
            if excluded || *budget==0 {return false;}
        }
        // A failed bounded attribute lookup must not be mistaken for an
        // absent HTML attribute and then overridden by live checkedness.
        if context.charge_text && *budget == 0 {
            return false;
        }
        if part == 0 {
            return true;
        }
        let mut parent = context.parents.get(node).copied().flatten();
        match selector.relations[part - 1] {
            Relation::Child => {
                parent.is_some_and(|parent| at(selector, part - 1, parent, context, budget))
            }
            Relation::Descendant => {
                for _ in 0..128 {
                    let Some(index) = parent else {
                        break;
                    };
                    if at(selector, part - 1, index, context, budget) {
                        return true;
                    }
                    if *budget == 0 {
                        break;
                    }
                    parent = context.parents.get(index).copied().flatten();
                }
                false
            }
            Relation::AdjacentSibling => context.previous_element(node,budget)
                .is_some_and(|previous| at(selector, part - 1, previous, context, budget)),
            Relation::GeneralSibling => {
                let mut previous = context.previous_element(node,budget);
                while let Some(index) = previous {
                    if at(selector, part - 1, index, context, budget) {
                        return true;
                    }
                    if *budget == 0 {
                        break;
                    }
                    previous = context.previous_element(index,budget);
                }
                false
            }
        }
    }
    at(selector, selector.parts.len() - 1, node, context, budget)
}

fn parse_declarations(source: &str) -> Vec<Declaration> {
    parse_declarations_bounded(source,MAX_DECLARATIONS)
}

/// CSSOM specified order keeps the winning occurrence, including its original
/// position. Use the renderer's declaration tokenizer and custom-name grammar.
pub(crate) fn inline_declaration_names(source: &str, supported: &[&str], mut meter: Option<&mut CssBudget<'_>>) -> Result<Vec<String>, String> {
    use std::collections::BTreeMap;
    let declarations = parse_declarations_inner(source, MAX_DECLARATIONS + 1, true, meter.as_deref_mut())?;
    if declarations.len() > MAX_DECLARATIONS { return Err("Inline CSS declaration limit reached.".into()); }
    let mut winners: BTreeMap<String, (usize, bool)> = BTreeMap::new();
    let mut order = 0;
    for decl in declarations {
        let name = decl.name.as_str();
        if !name.starts_with("--") && supported.binary_search(&name).is_err() { continue; }
        let value = decl.value.to_ascii_lowercase();
        let variable = value.contains("var(");
        let wide = matches!(value.as_str(), "inherit" | "initial" | "unset" | "revert" | "revert-layer");
        if name == "animation-timing-function" {
            if let Some(meter) = meter.as_deref_mut() { meter.work(easing_source_cost(value.len()).0)?; }
        }
        let expanded: &[&str] = match name {
            "margin" | "padding" => {
                if !variable && !wide && box_spacing::shorthand(&value, name == "margin").is_none() { continue; }
                if name == "margin" { &["margin-top","margin-right","margin-bottom","margin-left"] }
                else { &["padding-top","padding-right","padding-bottom","padding-left"] }
            }
            "inset" => {
                if !variable && !wide && lengths(&value, true, true).is_none() { continue; }
                &["top","right","bottom","left"]
            }
            "flex" => {
                if !variable && !wide && flex_shorthand(&value).is_none() { continue; }
                &["flex-grow","flex-shrink","flex-basis"]
            }
            "gap" => {
                if !variable && !wide && gap_values(&value).is_none() { continue; }
                &["row-gap", "column-gap"]
            }
            // These shorthands reset additional longhands outside the inline
            // bridge. A partial expansion would expose incorrect indices.
            "background" | "border" | "border-color" | "border-width" | "border-style" |
            "font" | "overflow" | "text-decoration" => {
                return Err(format!("Indexed inline CSS does not support the full {name} shorthand expansion."));
            }
            "z-index" if !z_index::valid(&decl.value)=>continue,
            "visibility"|"pointer-events" if !interaction_style::valid(name,&decl.value)=>continue,
            "color" | "background-color" if !inline_color_value(&value) => continue,
            "animation-timing-function" if !inline_easing_value(&value) => continue,
            "touch-action" if touch_action::specified(&value).is_none() => continue,
            "width" | "height" | "min-width" | "min-height" if !variable && !wide && sizing_value(&value,Length::Auto).is_none() => continue,
            "max-width" | "max-height" if !variable && !wide && max_sizing_value(&value).is_none() => continue,
            "top" | "right" | "bottom" | "left" if !variable && !wide && parse_length(&value, true, true).is_none() => continue,
            "row-gap" | "column-gap" if !variable && !wide && value != "normal" && max_sizing_value(&value).is_none() => continue,
            s if s.starts_with("margin-") && !variable && !wide && box_spacing::length(&value, true).is_none() => continue,
            s if s.starts_with("padding-") && !variable && !wide && box_spacing::length(&value, false).is_none() => continue,
            _ => std::slice::from_ref(&name),
        };
        for &longhand in expanded {
            if !winners.get(longhand).is_some_and(|(_, important)| *important && !decl.important) {
                winners.insert(longhand.into(), (order, decl.important));
            }
            order += 1;
        }
        if winners.len() > MAX_DECLARATIONS { return Err("Expanded inline CSS declaration limit reached.".into()); }
    }
    let mut names: Vec<_> = winners.into_iter().collect();
    names.sort_unstable_by_key(|(_, (order, _))| *order);
    Ok(names.into_iter().map(|(name, _)| name).collect())
}

/// Parsed CSSOM block values use the same declaration/value grammar as layout.
pub(crate) fn inline_text_declarations(source:&str,supported:&[&str],meter:&mut CssBudget<'_>) -> Result<Vec<(String,String,bool)>,String> {
    let declarations=parse_declarations_inner(source,MAX_DECLARATIONS+1,true,Some(meter))?;
    if declarations.len()>MAX_DECLARATIONS{return Err("Inline CSS declaration limit reached.".into());}
    let mut result=Vec::new();
    result.try_reserve_exact(declarations.len()).map_err(|_|"Cannot reserve inline declaration block.")?;
    for decl in declarations {
        if !decl.name.starts_with("--") && supported.binary_search(&decl.name.as_str()).is_err(){continue;}
        let single=format!("{}:{}{};",decl.name,decl.value,if decl.important{" !important"}else{""});
        let names=inline_declaration_names(&single,supported,Some(meter))?;
        if names.is_empty(){continue;}
        let lower=decl.value.to_ascii_lowercase();
        let wide=matches!(lower.as_str(),"inherit"|"initial"|"unset"|"revert"|"revert-layer");
        let variable=lower.contains("var(");
        if !wide&&!variable {
            let valid=match decl.name.as_str(){
                "display"=>display_value(&lower).is_some(),
                "box-sizing"=>matches!(lower.as_str(),"content-box"|"border-box"),
                "flex-direction"=>matches!(lower.as_str(),"row"|"row-reverse"|"column"|"column-reverse"),
                "flex-wrap"=>matches!(lower.as_str(),"nowrap"|"wrap"),
                "flex-grow"|"flex-shrink"=>flex_factor(&lower).is_some(),
                "flex-basis"=>sizing_value(&lower,Length::Auto).is_some(),
                "justify-content"=>flex_pack(&lower).is_some(),
                "align-items"=>flex_align(&lower).is_some(),
                "align-self"=>lower=="auto"||flex_align(&lower).is_some(),
                "font-family"=>font_families(&decl.value).is_some(),
                "font-size"=>font_size_value(&lower,16.0,16.0).is_some(),
                "font-weight"=>font_weight(&lower,400).is_some(),
                "font-style"=>matches!(lower.as_str(),"normal"|"italic"|"oblique"),
                "line-height"=>line_height::literal(&lower,LineHeight::Factor(1.5)).is_some(),
                "white-space"=>matches!(lower.as_str(),"normal"|"nowrap"|"pre"|"pre-wrap"|"break-spaces"),
                "text-align"=>matches!(lower.as_str(),"left"|"start"|"center"|"right"|"end"),
                "background-position"=>object_position(&lower).is_some(),
                "background-repeat"=>matches!(lower.as_str(),"repeat"|"no-repeat"|"repeat-x"|"repeat-y"),
                "background-size"=>matches!(lower.as_str(),"auto"|"contain"|"cover"),
                _=>true,
            };
            if !valid{continue;}
        }
        result.push((decl.name,decl.value,decl.important));
    }
    Ok(result)
}

pub(crate) fn inline_interaction_value(name:&str,value:&str)->bool{interaction_style::valid(name,value)}
pub(crate) fn specified_interaction_value(name:&str,value:&str)->Option<String>{if !interaction_style::valid(name,value){return None;}Some(if custom_properties::has_var(value){value.trim_ascii().to_owned()}else{value.trim_ascii().to_ascii_lowercase()})}

pub(crate) fn inline_color_value(value: &str) -> bool {
    let value = value.trim().to_ascii_lowercase();
    matches!(value.as_str(), "inherit" | "initial" | "unset" | "revert" | "revert-layer")
        || value.contains("var(") && custom_properties::valid_syntax(&value)
        || parse_solid_color(&value).is_some()
}

fn gap_values(value: &str) -> Option<[Length; 2]> {
    let grouped=border_parts(value)?;let mut parts=grouped.iter().copied();
    let length = |part: &str| if part == "normal" {Some(Length::Px(0.0))} else if part.starts_with("calc("){sizing_math::length(part,false)}else{sizing_math::viewport_literal(part,false).or_else(||parse_length(part, false, false))};
    let row = length(parts.next()?)?;
    let column = match parts.next() { Some(part) => length(part)?, None => row };
    parts.next().is_none().then_some([row, column])
}
// Shared raw declaration boundaries for the parser and native style bridge.
// This preserves quoted/parenthesized URL payloads without allocating a split
// snapshot or inventing a second interpretation of semicolons.
pub(crate) struct DeclarationItems<'a> {
    source: &'a str,
    chars: std::str::CharIndices<'a>,
    start: usize,
    quote: char,
    depth: usize,
    escaped: bool,
    open_comment: bool,
    finished: bool,
}
pub(crate) fn declaration_items(source: &str) -> DeclarationItems<'_> {
    DeclarationItems { source, chars: source.char_indices(), start: 0, quote: '\0', depth: 0, escaped: false, open_comment: false, finished: false }
}
impl DeclarationItems<'_> {
    pub(crate) fn complete(&self) -> bool { self.finished && self.quote == '\0' && self.depth == 0 && !self.escaped && !self.open_comment }
}
impl<'a> Iterator for DeclarationItems<'a> {
    type Item = &'a str;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let (index, character) = match self.chars.next() {
                Some(next) => next,
                None if !self.finished => { self.finished = true; (self.source.len(), ';') },
                None => return None,
            };
            if self.escaped { self.escaped = false; continue; }
            if self.quote != '\0' {
                if character == '\\' { self.escaped = true; }
                else if character == self.quote { self.quote = '\0'; }
                continue;
            }
            if matches!(character, '\'' | '"') { self.quote = character; continue; }
            if character == '/' && self.chars.as_str().starts_with('*') {
                self.chars.next();
                self.open_comment = true;
                while let Some((_, next)) = self.chars.next() {
                    if next == '*' && self.chars.as_str().starts_with('/') {
                        self.chars.next(); self.open_comment = false; break;
                    }
                }
                continue;
            }
            if matches!(character,'('| '[' | '{') { self.depth += 1; }
            if matches!(character,')'| ']' | '}') { self.depth = self.depth.saturating_sub(1); }
            if character != ';' || self.depth != 0 { continue; }
            let item = &self.source[self.start..index];
            self.start = (index + 1).min(self.source.len());
            return Some(item);
        }
    }
}
pub(crate) fn declaration_priority(value: &str) -> (&str, bool) {
    let value = value.trim();
    if let Some(bang) = value.rfind('!') {
        if value[bang + 1..].trim().eq_ignore_ascii_case("important") {
            return (value[..bang].trim_end(), true);
        }
    }
    (value, false)
}
fn parse_declarations_bounded(source:&str,maximum:usize)->Vec<Declaration> {
    parse_declarations_inner(source,maximum,true,None).expect("unmetered declarations")
}
fn parse_declarations_inner(source:&str,maximum:usize,validate_values:bool,
    mut transform_meter:Option<&mut CssBudget<'_>>)->Result<Vec<Declaration>,String> {
    let source = without_comments(source);
    let mut declarations = Vec::new();
    for item in declaration_items(&source) {
        if let Some((name, value)) = item.split_once(':') {
            let raw_name = if name.trim().eq_ignore_ascii_case("animation-timing-function") { easing_trim(name) } else { name.trim() };
            if raw_name.len() > 64 { continue; }
            let name = if raw_name.starts_with("--") { raw_name.to_owned() } else { raw_name.to_ascii_lowercase() };
            let (value, important) = if name == "animation-timing-function" { easing_priority(value) } else { declaration_priority(value) };
            let custom = name.starts_with("--");
            if !name.is_empty() && name.len() <= 64 && value.len() <= 2_048 && (!value.is_empty() || custom)
                && (!custom || custom_properties::custom_name(&name))
                && (name != "position" || position::valid_declaration(value))
                && (name != "touch-action" || touch_action::specified(value).is_some())
                && (!(custom || value.to_ascii_lowercase().contains("var(")) || custom_properties::valid_syntax(&value)) {
                if name == "transform" && validate_values {
                    if let Some(meter) = transform_meter.as_deref_mut() {
                        meter.source(name.len().saturating_add(value.len()))?;
                    }
                    if !translate::valid_declaration(value) {continue;}
                }
                if name == "opacity" && validate_values {
                    if let Some(meter) = transform_meter.as_deref_mut() {
                        meter.source(name.len().saturating_add(value.len()))?;
                    }
                    if !opacity::valid_declaration(value) { continue; }
                }
                declarations.push(Declaration {
                    name,
                    value: value.to_owned(),
                    important,
                    stylesheet: None,
                });
            }
        }
        if declarations.len() == maximum {
            break;
        }
    }
    Ok(declarations)
}

fn parse_length(value: &str, auto: bool, negative: bool) -> Option<Length> {
    let value = value.trim().to_ascii_lowercase();
    if auto && value == "auto" {
        return Some(Length::Auto);
    }
    let (number, unit) = if let Some(number) = value.strip_suffix("rem") {
        (number, "rem")
    } else if let Some(number) = value.strip_suffix("px") {
        (number, "px")
    } else if let Some(number) = value.strip_suffix("em") {
        (number, "em")
    } else if let Some(number) = value.strip_suffix("pt") {
        (number, "pt")
    } else if let Some(number) = value.strip_suffix("pc") {
        (number, "pc")
    } else if let Some(number) = value.strip_suffix('%') {
        (number, "%")
    } else {
        (value.as_str(), "")
    };
    let number: f32 = number.trim().parse().ok()?;
    if !number.is_finite() || (!negative && number < 0.0) || (unit.is_empty() && number != 0.0) {
        return None;
    }
    length_with_unit(number, unit)
}

fn length_with_unit(number: f32, unit: &str) -> Option<Length> {
    let number = number.clamp(-10_000.0, 10_000.0);
    Some(match unit {
        "%" => Length::Percent(number),
        "em" => Length::Em(number),
        "rem" => Length::Rem(number),
        "pt" => Length::Px(number * (96.0 / 72.0)),
        "pc" => Length::Px(number * 16.0),
        "px" | "" => Length::Px(number),
        _ => return None,
    })
}

fn html_pixels(value: &str) -> Option<f32> {
    let value = value.trim().parse::<f32>().ok()?;
    (value.is_finite() && value >= 0.0).then_some(value.min(10_000.0))
}

fn html_dimension(value: &str) -> Option<Length> {
    let value = value.trim();
    if let Some(percent) = value.strip_suffix('%') {
        Some(Length::Percent(html_pixels(percent)?))
    } else {
        Some(Length::Px(html_pixels(value)?))
    }
}

fn four<T: Copy>(values: &[T]) -> Option<[T; 4]> {
    Some(match values {
        [a] => [*a; 4],
        [a, b] => [*a, *b, *a, *b],
        [a, b, c] => [*a, *b, *c, *b],
        [a, b, c, d] => [*a, *b, *c, *d],
        _ => return None,
    })
}

fn lengths(value: &str, auto: bool, negative: bool) -> Option<[Length; 4]> {
    let values: Option<Vec<_>> = value
        .split_ascii_whitespace()
        .take(5)
        .map(|v| parse_length(v, auto, negative))
        .collect();
    four(&values?)
}

fn border_length(value: &str) -> Option<Length> {
    match value {
        "thin" => Some(Length::Px(1.0)),
        "medium" => Some(Length::Px(3.0)),
        "thick" => Some(Length::Px(5.0)),
        _ => parse_length(value, false, false).filter(|value| !matches!(value, Length::Percent(_))),
    }
}

fn flex_factor(value: &str) -> Option<f32> {
    value
        .parse::<f32>()
        .ok()
        .filter(|n| n.is_finite() && *n >= 0.0)
        .map(|n| n.min(10_000.0))
}

fn flex_direction(value: &str) -> Option<FlexDirection> {
    match value {
        "row" => Some(FlexDirection::Row),
        "row-reverse" => Some(FlexDirection::RowReverse),
        "column" => Some(FlexDirection::Column),
        "column-reverse" => Some(FlexDirection::ColumnReverse),
        _ => None,
    }
}

fn flex_wrap(value: &str) -> Option<bool> {
    match value {
        "nowrap" => Some(false),
        "wrap" => Some(true),
        _ => None,
    }
}

fn flex_pack(value: &str) -> Option<FlexPack> {
    match value {
        "flex-start" | "normal" => Some(FlexPack::Start),
        "flex-end" => Some(FlexPack::End),
        "start" => Some(FlexPack::LogicalStart),
        "end" => Some(FlexPack::LogicalEnd),
        "center" => Some(FlexPack::Center),
        "space-between" => Some(FlexPack::Between),
        "space-around" => Some(FlexPack::Around),
        "space-evenly" => Some(FlexPack::Evenly),
        "stretch" => Some(FlexPack::Stretch),
        _ => None,
    }
}

fn flex_align(value: &str) -> Option<FlexAlign> {
    match value {
        "normal" | "stretch" => Some(FlexAlign::Stretch),
        "start" | "flex-start" => Some(FlexAlign::Start),
        "end" | "flex-end" => Some(FlexAlign::End),
        "center" => Some(FlexAlign::Center),
        _ => None,
    }
}

fn flex_shorthand(value: &str) -> Option<(f32, f32, Length)> {
    match value {
        "none" => return Some((0.0, 0.0, Length::Auto)),
        "auto" => return Some((1.0, 1.0, Length::Auto)),
        "initial" => return Some((0.0, 1.0, Length::Auto)),
        _ => {}
    }
    let mut factors = Vec::new();
    let mut basis = None;
    let mut factors_ended = false;
    let mut count = 0;
    for part in border_parts(value)? {
        count += 1;
        if count > 3 {
            return None;
        }
        if factors.len() < 2 {
            if let Some(factor) = flex_factor(part) {
                if factors_ended {
                    return None;
                }
                factors.push(factor);
                continue;
            }
        }
        if basis.is_some() {
            return None;
        }
        basis = Some(sizing_value(part,Length::Auto)?);
        factors_ended = !factors.is_empty();
    }
    if count == 0 {
        return None;
    }
    Some((
        factors.first().copied().unwrap_or(1.0),
        factors.get(1).copied().unwrap_or(1.0),
        basis.unwrap_or(Length::Percent(0.0)),
    ))
}

fn display_value(value:&str)->Option<Display>{Some(match value{
    "none"=>Display::None,"block"|"list-item"=>Display::Block,"inline"=>Display::Inline,"inline-block"=>Display::InlineBlock,"inline-flex"=>Display::InlineFlex,"flex"=>Display::Flex,"grid"=>Display::Grid,"table"=>Display::Table,"table-row-group"|"table-header-group"|"table-footer-group"=>Display::TableRowGroup,"table-row"=>Display::TableRow,"table-cell"=>Display::TableCell,_=>return None,
})}
fn flex_flow_value(value:&str)->Option<(FlexDirection,bool)>{
    let mut direction=None;let mut wrap=None;let mut count=0;
    for part in value.split_ascii_whitespace(){count+=1;if let Some(value)=flex_direction(part){if direction.replace(value).is_some(){return None;}}else if let Some(value)=flex_wrap(part){if wrap.replace(value).is_some(){return None;}}else{return None;}}
    (count!=0).then_some((direction.unwrap_or(FlexDirection::Row),wrap.unwrap_or(false)))
}
fn max_sizing_value(value:&str)->Option<Option<Length>>{
    let value=value.trim();if value.eq_ignore_ascii_case("none"){return Some(None);}
    if value.starts_with("calc("){return sizing_math::length(value,false).map(Some);}
    if let Some(value)=sizing_math::viewport_literal(value,false){return Some(Some(value));}
    if !value.is_ascii()||value.bytes().any(|b|b.is_ascii_whitespace()){return None;}
    parse_length(value,false,false).map(Some)
}

// Shared finite sizing grammar for literal and substituted longhands. Separate
// tokens cannot form a dimension, including var(--number)px after substitution.
fn sizing_value(value: &str, parent: Length) -> Option<Length> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("inherit") { return Some(parent); }
    if value.eq_ignore_ascii_case("initial") || value.eq_ignore_ascii_case("unset") {
        return Some(Length::Auto);
    }
    if value.to_ascii_lowercase().starts_with("calc("){return sizing_math::length(value,false);}
    if let Some(value)=sizing_math::viewport_literal(value,false){return Some(value);}
    if !value.is_ascii() || value.bytes().any(|byte|byte.is_ascii_whitespace()) { return None; }
    parse_length(value,true,false)
}

// Shared finite font-size grammar for literal and substituted longhands.
// Font-size em/% use the parent's computed size; rem uses the current root
// metric (the initial root metric while computing the root's own font-size).
fn font_size_value(value:&str,parent:f32,root:f32)->Option<f32> {
    // One ASCII size token only. parse_length's numeric trimming must not join a
    // substituted number with an adjoining source unit, e.g. var(--n)px.
    if !value.is_ascii() || value.bytes().any(|byte|byte.is_ascii_whitespace()) {return None;}
    let size=match value {
        "inherit"|"unset"=>Some(parent),
        "initial"|"medium"=>Some(INITIAL_FONT_SIZE),
        "xx-small"=>Some(9.0),"x-small"=>Some(10.0),"small"=>Some(13.0),
        "large"=>Some(18.0),"x-large"=>Some(24.0),"xx-large"=>Some(32.0),
        "smaller"=>Some(parent*0.8),"larger"=>Some(parent*1.2),
        _=>parse_length(value,false,false).and_then(|size|size.resolve(parent,parent,root)),
    }?;
    Some(size.clamp(1.0,256.0))
}

/// Apply the existing finite font shorthand once. Its line-height component
/// participates in the same stylesheet checkpoint as the longhand.
fn font_size_available(value:&str,parent:bool,root:bool)->bool {
    if value.ends_with("rem") {root}
    else if value.ends_with("em") || value.ends_with('%') || matches!(value,"inherit"|"unset"|"larger"|"smaller") {parent}
    else {true}
}
fn apply_font(style: &mut Style, parent: &Style, declaration: &Declaration, value: &str) -> bool {
    apply_font_with_availability(style,parent,declaration,value,&mut true,true,true)
}
fn apply_font_with_availability(style: &mut Style,parent:&Style,declaration:&Declaration,value:&str,known:&mut bool,parent_known:bool,root_known:bool)->bool {
    if matches!(value, "inherit" | "unset") {
        *known=parent_known;
        style.font_size = parent.font_size;
        style.font_families = parent.font_families.clone();
        style.font_weight = parent.font_weight;
        style.bold = parent.bold;
        style.italic = parent.italic;
        style.line_height = parent.line_height;
    } else if value == "initial" {
        *known=true;
        style.font_size = INITIAL_FONT_SIZE;
        style.font_families = vec!["serif".into()];
        style.font_weight = 400;
        style.bold = false;
        style.italic = false;
        style.line_height = LineHeight::Factor(1.5);
    } else if let Some((size, line, families, weight, italic)) =
        font_shorthand(&declaration.value, parent.font_weight)
    {
        *known=font_size_available(&size.to_ascii_lowercase(),parent_known,root_known);
        style.font_families = families;
        style.font_weight = weight;
        style.bold = weight >= 600;
        style.italic = italic;
        style.line_height = LineHeight::Factor(1.5);
        apply(style,parent,&Declaration {
            name: "font-size".into(), value: size, important: false,
            stylesheet: declaration.stylesheet,
        });
        if let Some(line) = line {
            if let Some(value) = line_height::literal(&line,parent.line_height) {
                style.line_height = value;
            }
        }
    } else { return false; }
    true
}

fn apply(style: &mut Style, parent: &Style, declaration: &Declaration) {
    let value = declaration.value.trim().to_ascii_lowercase();
    if custom_properties::layout_property(&declaration.name)&&matches!(value.as_str(),"inherit"|"initial"|"unset"){custom_properties::reset_box(style,parent,&declaration.name,value=="inherit");return;}
    match declaration.name.as_str() {
        "display" => {if let Some(parsed)=display_value(&value){style.display=parsed;style.display_alias=match value.as_str(){"list-item"=>Some("list-item"),"table-header-group"=>Some("table-header-group"),"table-footer-group"=>Some("table-footer-group"),_=>None};}},
        "overflow" | "overflow-x" | "overflow-y" => { overflow::apply(style, parent, &declaration.name, &value); }
        "inset" => {
            if value == "inherit" {
                style.inset = parent.inset;
            } else if value == "initial" || value == "unset" {
                style.inset = [Length::Auto; 4];
            } else if let Some(values) = lengths(&value, true, true) {
                style.inset = values;
            }
        }
        "top" | "right" | "bottom" | "left" => {
            let side = match declaration.name.as_str() {
                "top" => 0,
                "right" => 1,
                "bottom" => 2,
                _ => 3,
            };
            if value == "inherit" {
                style.inset[side] = parent.inset[side];
            } else if value == "initial" || value == "unset" {
                style.inset[side] = Length::Auto;
            } else if let Some(length) = parse_length(&value, true, true) {
                style.inset[side] = length;
            }
        }
        "color" => {
            // color is inherited; currentColor on color itself is inherit.
            // Keep the finite initial palette rather than adding system colors.
            if matches!(value.as_str(), "inherit" | "unset" | "currentcolor") {
                style.color = parent.color;
            } else if value == "initial" {
                style.color = Style::default().color;
            } else if let Some(color) = color(&value) {
                style.color = color;
            }
        }
        "object-fit" => {
            if let Some(fit) = match value.as_str() {
                "fill" | "initial" | "unset" => Some(ObjectFit::Fill),
                "contain" => Some(ObjectFit::Contain),
                "cover" => Some(ObjectFit::Cover),
                "none" => Some(ObjectFit::None),
                "scale-down" => Some(ObjectFit::ScaleDown),
                "inherit" => Some(parent.object_fit),
                _ => None,
            } {
                style.object_fit = fit;
            }
        }
        "image-rendering" => {
            if let Some(rendering) = match value.as_str() {
                "auto" | "initial" => Some(ImageRendering::Auto),
                "smooth" | "optimizequality" => Some(ImageRendering::Smooth),
                "high-quality" => Some(ImageRendering::HighQuality),
                "crisp-edges" | "optimizespeed" => Some(ImageRendering::CrispEdges),
                "pixelated" => Some(ImageRendering::Pixelated),
                "inherit" | "unset" => Some(parent.image_rendering),
                _ => None,
            } {
                style.image_rendering = rendering;
            }
        }
        "object-position" => {
            if value == "inherit" {
                style.object_position = parent.object_position;
            } else if value == "initial" || value == "unset" {
                style.object_position = [ObjectPosition::CENTER; 2];
            } else if let Some(position) = object_position(&value) {
                style.object_position = position;
            }
        }
        "background-color" => {
            if value == "inherit" {
                style.background = parent.background;
                style.background_current_color = parent.background_current_color;
            } else if value == "currentcolor" {
                style.background = Some(style.color);
                style.background_current_color = true;
            } else if matches!(value.as_str(), "transparent" | "none" | "initial" | "unset") {
                style.background = None;
                style.background_current_color = false;
            } else if let Some(color) = opaque_background_color(&value) {
                style.background = Some(color);
                style.background_current_color = false;
            }
        }
        "background-image" => {
            if value == "inherit" {
                style.background_image = parent.background_image.clone();
            } else if matches!(value.as_str(), "none" | "initial" | "unset") {
                style.background_image = None;
            } else if let Some(url) = background_image_url(&declaration.value) {
                style.background_image = Some(BackgroundImage {
                    url,
                    stylesheet: declaration.stylesheet,
                });
            }
        }
        "background-repeat" => {
            if let Some(repeat) = match value.as_str() {
                "repeat" | "initial" | "unset" => Some([true, true]),
                "no-repeat" => Some([false, false]),
                "repeat-x" => Some([true, false]),
                "repeat-y" => Some([false, true]),
                "inherit" => Some(parent.background_repeat),
                _ => None,
            } {
                style.background_repeat = repeat;
            }
        }
        "background-position" => {
            if value == "inherit" {
                style.background_position = parent.background_position;
            } else if value == "initial" || value == "unset" {
                style.background_position = [ObjectPosition::START; 2];
            } else if let Some(position) = object_position(&value) {
                style.background_position = position;
            }
        }
        "background-size" => {
            if let Some(size) = match value.as_str() {
                "auto" | "initial" | "unset" => Some(BackgroundSize::Auto),
                "contain" => Some(BackgroundSize::Contain),
                "cover" => Some(BackgroundSize::Cover),
                "inherit" => Some(parent.background_size),
                _ => None,
            } {
                style.background_size = size;
            }
        }
        "background" => {
            if value == "inherit" {
                style.background = parent.background;
                style.background_current_color = parent.background_current_color;
                style.background_image = parent.background_image.clone();
                style.background_clip = parent.background_clip;
                style.background_repeat = parent.background_repeat;
                style.background_position = parent.background_position;
                style.background_size = parent.background_size;
            } else {
                // Only the existing color/none shorthand subset is supported.
                // A valid shorthand resets all the other background longhands;
                // unsupported compound syntax must not partially reset them.
                let background = if matches!(value.as_str(), "transparent" | "none" | "initial" | "unset") {
                    Some(None)
                } else if value == "currentcolor" {
                    Some(Some(style.color))
                } else {
                    opaque_background_color(&value).map(Some)
                };
                if let Some(background) = background {
                    style.background = background;
                    style.background_current_color = value == "currentcolor";
                    style.background_clip = Some(BackgroundClip::BorderBox);
                    style.background_image = None;
                    style.background_repeat = [true; 2];
                    style.background_position = [ObjectPosition::START; 2];
                    style.background_size = BackgroundSize::Auto;
                }
            }
        }
        "font" => {
            apply_font(style,parent,declaration,&value);
        }
        "font-family" => {
            if matches!(value.as_str(), "inherit" | "unset") {
                style.font_families = parent.font_families.clone();
            } else if value == "initial" {
                style.font_families = vec!["serif".into()];
            } else if let Some(families) = font_families(&declaration.value) {
                style.font_families = families;
            }
        }
        "font-size" => {
            if let Some(size)=font_size_value(&value,parent.font_size,style.root_font_size) {style.font_size=size;}
        }
        "font-weight" => {
            if let Some(weight) = font_weight(&value, parent.font_weight) {
                style.font_weight = weight;
                style.bold = weight >= 600;
            }
        }
        "font-style" => match value.as_str() {
            "italic" | "oblique" => style.italic = true,
            "normal" | "initial" => style.italic = false,
            "inherit" | "unset" => style.italic = parent.italic,
            _ => {}
        },
        "text-decoration" | "text-decoration-line" => {
            if value == "none" {
                style.underline = false;
            } else if value
                .split_ascii_whitespace()
                .any(|part| part == "underline")
            {
                style.underline = true;
            }
        }
        "text-align" => {
            let align = match value.as_str() {
                "left" | "start" => Some(Align::Left),
                "center" => Some(Align::Center),
                "right" | "end" => Some(Align::Right),
                "inherit" => Some(parent.align),
                _ => None,
            };
            if let Some(align) = align {
                style.align = align;
                style.legacy_align = if value == "inherit" {
                    parent.legacy_align
                } else {
                    None
                };
            }
        }
        "line-height" => {
            if let Some(value) = line_height::literal(&value,parent.line_height) {style.line_height=value;}
        }
        "white-space" => match value.as_str() {
            "normal" => style.white_space = WhiteSpace::Normal,
            "nowrap" => style.white_space = WhiteSpace::NoWrap,
            "pre" => style.white_space = WhiteSpace::Pre,
            "pre-wrap" | "break-spaces" => style.white_space = WhiteSpace::PreWrap,
            _ => {}
        },
        "border-spacing" => {
            let values: Option<Vec<_>> = value
                .split_ascii_whitespace()
                .take(3)
                .map(|v| parse_length(v, false, false).filter(|v| !matches!(v, Length::Percent(_))))
                .collect();
            if let Some(values) = values {
                match values.as_slice() {
                    [all] => style.border_spacing = [*all; 2],
                    [horizontal, vertical] => style.border_spacing = [*horizontal, *vertical],
                    _ => {}
                }
            }
        }
        "list-style" | "list-style-type" => {
            for part in value.split_ascii_whitespace() {
                match part {
                    "none" => style.list_style = ListStyle::None,
                    "decimal" => style.list_style = ListStyle::Decimal,
                    "disc" | "circle" | "square" => style.list_style = ListStyle::Disc,
                    _ => {}
                }
            }
        }
        "margin" => {
            if let Some(values) = box_spacing::shorthand(&value, true) {
                style.margin = values;
            }
        }
        "padding" => {
            if let Some(values) = box_spacing::shorthand(&value, false) {
                style.padding = values;
            }
        }
        "width" => {if let Some(value)=sizing_value(&value,parent.width){style.width=value;}}
        "height" => {if let Some(value)=sizing_value(&value,parent.height){style.height=value;}}
        "min-width" => {if let Some(value)=sizing_value(&value,parent.min_width){style.min_width=value;}}
        "min-height" => {if let Some(value)=sizing_value(&value,parent.min_height){style.min_height=value;}}
        "max-width" => {if let Some(value)=max_sizing_value(&value){style.max_width=value;}}
        "max-height" => {if let Some(value)=max_sizing_value(&value){style.max_height=value;}}
        "box-sizing" => match value.as_str() {
            "content-box" | "initial" | "unset" => style.border_box = false,
            "border-box" => style.border_box = true,
            "inherit" => style.border_box = parent.border_box,
            _ => {}
        },
        "flex-direction" => {
            if let Some(value) = flex_direction(&value) {
                style.flex_direction = value;
            }
        }
        "flex-wrap" => {
            if let Some(value) = flex_wrap(&value) {
                style.flex_wrap = value;
            }
        }
        "flex-flow"=>{if let Some((direction,wrap))=flex_flow_value(&value){style.flex_direction=direction;style.flex_wrap=wrap;}}
        "justify-content" => {
            if let Some(value) = flex_pack(&value) {
                style.justify_content = value;
            }
        }
        "align-content" => {
            if value == "normal" {
                style.align_content = FlexPack::Stretch;
            } else if let Some(value) = flex_pack(&value) {
                style.align_content = value;
            }
        }
        "align-items" => {
            if let Some(value) = flex_align(&value) {
                style.align_items = value;
            }
        }
        "align-self" => {
            if value == "auto" {
                style.align_self = None;
            } else if let Some(value) = flex_align(&value) {
                style.align_self = Some(value);
            }
        }
        "flex-grow" => {
            if let Some(value) = flex_factor(&value) {
                style.flex_grow = value;
            }
        }
        "flex-shrink" => {
            if let Some(value) = flex_factor(&value) {
                style.flex_shrink = value;
            }
        }
        "flex-basis" => {
            if let Some(value) = sizing_value(&value,parent.flex_basis) {
                style.flex_basis = value;
            }
        }
        "flex" => {
            if let Some((grow, shrink, basis)) = flex_shorthand(&value) {
                style.flex_grow = grow;
                style.flex_shrink = shrink;
                style.flex_basis = basis;
            }
        }
        "gap" => {
            if let Some(gap) = gap_values(&value) { style.gap = gap; }
        }
        "row-gap" | "column-gap" => {
            if let Some(value) = if value == "normal" {
                Some(Length::Px(0.0))
            } else {
                if value.starts_with("calc("){sizing_math::length(&value,false)}else{sizing_math::viewport_literal(&value,false).or_else(||parse_length(&value,false,false))}
            } {
                style.gap[usize::from(declaration.name == "column-gap")] = value;
            }
        }
        "border" => apply_border(style, None, &value),
        "border-width" => {
            let values: Option<Vec<_>> = value
                .split_ascii_whitespace()
                .take(5)
                .map(border_length)
                .collect();
            if let Some(values) = values.and_then(|values| four(&values)) {
                style.border_width = values;
            }
        }
        "border-color" => {
            if value=="inherit" {
                style.border_color=parent.border_color.map(|rgb|Some(rgb.unwrap_or(parent.color)));
                style.border_alpha=parent.border_alpha;
            } else if matches!(value.as_str(),"initial"|"unset") {
                style.border_color=[None;4];style.border_alpha=[255;4];
            } else {
                let values=border_parts(&value).and_then(|parts|parts.into_iter().map(border_color_value).collect::<Option<Vec<_>>>());
                if let Some(values) = values.and_then(|values| four(&values)) {
                    style.border_color = values.map(|(rgb,_)|rgb);
                    style.border_alpha = values.map(|(_,alpha)|alpha);
                }
            }
        }
        "border-style" => {
            let values: Option<Vec<_>> = value
                .split_ascii_whitespace()
                .take(5)
                .map(|v| match v {
                    "solid" => Some(true),
                    "none" | "hidden" => Some(false),
                    _ => None,
                })
                .collect();
            if let Some(values) = values.and_then(|values| four(&values)) {
                style.border_solid = values;
            }
        }
        name => {
            for (side, index) in [("top", 0), ("right", 1), ("bottom", 2), ("left", 3)] {
                if name == format!("margin-{side}") {
                    if let Some(value) = box_spacing::length(&value, true) {
                        style.margin[index] = value;
                    }
                } else if name == format!("padding-{side}") {
                    if let Some(value) = box_spacing::length(&value, false) {
                        style.padding[index] = value;
                    }
                } else if name == format!("border-{side}") {
                    apply_border(style, Some(index), &value);
                } else if name == format!("border-{side}-width") {
                    if let Some(value) = border_length(&value) {
                        style.border_width[index] = value;
                    }
                } else if name == format!("border-{side}-color") {
                    let inherited=if value=="inherit" {Some((Some(parent.border_color[index].unwrap_or(parent.color)),parent.border_alpha[index]))}
                        else if matches!(value.as_str(),"initial"|"unset") {Some((None,255))} else {border_color_value(&value)};
                    if let Some((rgb,alpha)) = inherited {
                        style.border_color[index] = rgb;
                        style.border_alpha[index] = alpha;
                    }
                } else if name == format!("border-{side}-style") {
                    match value.as_str() {
                        "solid" => style.border_solid[index] = true,
                        "none" | "hidden" => style.border_solid[index] = false,
                        _ => {}
                    }
                }
            }
        }
    }
}

fn apply_border(style: &mut Style, side: Option<usize>, value: &str) {
    let Some((width,solid,rgb,alpha))=parse_border(value) else {return;};
    for index in 0..4 {
        if side.is_none() || side == Some(index) {
            style.border_width[index] = width;
            style.border_solid[index] = solid;
            style.border_color[index] = rgb;
            style.border_alpha[index] = alpha;
        }
    }
}

// Border component boundaries are whitespace outside functions. Splitting an
// rgba() at its internal spaces would turn a single color into several tokens.
// The existing declaration/output limit also bounds this finite scanner.
fn border_parts(value:&str)->Option<Vec<&str>> {
    if value.len()>MAX_GROUPED_BOX_BYTES {return None;}
    let mut result=Vec::new();let mut start=None;let mut depth=0usize;
    for (index,ch) in value.char_indices() {
        if ch.is_ascii_whitespace() && depth==0 {
            if let Some(start)=start.take() {
                if result.len()==4 {return None;}result.push(&value[start..index]);
            }
            continue;
        }
        if start.is_none() {start=Some(index);}
        match ch {
            '('=>{depth+=1;if depth>8{return None;}},
            ')'=>{depth=depth.checked_sub(1)?;},
            '\''|'"'|'['|']'|'{'|'}'|'\\'=>return None,
            _=>{},
        }
    }
    if depth!=0 {return None;}
    if let Some(start)=start {if result.len()==4{return None;}result.push(&value[start..]);}
    if result.is_empty(){None}else{Some(result)}
}

/// Border-only RGBA; alpha is never discarded or premixed against a guessed
/// backdrop. Existing text/background color parsing stays unchanged.
fn rgba_number(text:&str)->Option<f64> {
    // A Rust float parser also accepts `1.` and `1.e2`; CSS tokenizes those as
    // a number followed by other tokens. Require one complete CSS number first.
    if text.is_empty() || text.len()>2048 || !text.is_ascii() {return None;}
    let bytes=text.as_bytes();let mut index=0;
    if matches!(bytes.get(index),Some(b'+'|b'-')) {index+=1;}
    let integer=index;while bytes.get(index).is_some_and(u8::is_ascii_digit) {index+=1;}
    let has_integer=index>integer;
    if bytes.get(index)==Some(&b'.') {
        index+=1;let fraction=index;
        while bytes.get(index).is_some_and(u8::is_ascii_digit) {index+=1;}
        if index==fraction {return None;}
    } else if !has_integer {return None;}
    if matches!(bytes.get(index),Some(b'e'|b'E')) {
        index+=1;if matches!(bytes.get(index),Some(b'+'|b'-')) {index+=1;}
        let exponent=index;while bytes.get(index).is_some_and(u8::is_ascii_digit) {index+=1;}
        if index==exponent {return None;}
    }
    if index!=bytes.len() {return None;}
    text.parse::<f64>().ok().filter(|value|value.is_finite())
}
fn border_color_value(value:&str)->Option<(Option<u32>,u8)> {
    if value=="currentcolor" {return Some((None,255));}
    if value=="transparent" {return Some((Some(0),0));}
    if let Some(body)=value.strip_prefix("rgba(").and_then(|body|body.strip_suffix(')')) {
        if value.len()>2048 {return None;}
        let mut fields=body.split(',').map(|field|field.trim_matches(|ch:char|ch.is_ascii_whitespace()));
        let channels=[fields.next()?,fields.next()?,fields.next()?];let alpha=fields.next()?;
        if fields.next().is_some() {return None;}
        let percent=channels[0].ends_with('%');
        if channels.iter().any(|channel|channel.ends_with('%')!=percent) {return None;}
        let mut rgb=0u32;
        for (channel,shift) in channels.into_iter().zip([16,8,0]) {
            let channel=if percent {channel.strip_suffix('%')?}else{channel};
            let number=rgba_number(channel)?;
            let number=if percent {number.clamp(0.0,100.0)*255.0/100.0}else{number.clamp(0.0,255.0)};
            rgb|=(number.round()as u32)<<shift;
        }
        let alpha=rgba_number(alpha)?;
        return Some((Some(rgb),(alpha.clamp(0.0,1.0)*255.0).round()as u8));
    }
    color(value).map(|rgb|(Some(rgb),255))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ParsedColor {
    Solid(u32),
    CurrentColor,
}

/// A solid color stores straight sRGB as 0xAARRGGBB. The caller resolves
/// currentColor against the element when the color is assigned.
pub(crate) fn parse_solid_color(value: &str) -> Option<ParsedColor> {
    if value.len() > 2048 || !value.is_ascii() {
        return None;
    }
    let value = value.trim_matches(color_whitespace).to_ascii_lowercase();
    if value == "currentcolor" {
        return Some(ParsedColor::CurrentColor);
    }
    if value == "transparent" {
        return Some(ParsedColor::Solid(0));
    }
    if let Some(hex) = value.strip_prefix('#') {
        if !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return None;
        }
        let number = u32::from_str_radix(hex, 16).ok()?;
        let expand = |number: u32| {
            ((number >> 8) & 15) * 0x110000
                + ((number >> 4) & 15) * 0x1100
                + (number & 15) * 0x11
        };
        let (rgb, alpha) = match hex.len() {
            3 => (expand(number), 255),
            4 => (expand(number >> 4), (number & 15) * 17),
            6 => (number, 255),
            8 => (number >> 8, number & 255),
            _ => return None,
        };
        return Some(ParsedColor::Solid((alpha << 24) | rgb));
    }
    if let Some((rgb, alpha)) = functional_rgb(&value) {
        return Some(ParsedColor::Solid(((alpha.round() as u32) << 24) | rgb));
    }
    named_color(&value).map(|rgb| ParsedColor::Solid(0xff00_0000 | rgb))
}

fn color_whitespace(ch: char) -> bool {
    matches!(ch, ' ' | '\t' | '\n' | '\r' | '\x0c')
}

fn opaque_background_color(value: &str) -> Option<u32> {
    if value.starts_with("rgb(") || value.starts_with("rgba(") {
        let (rgb, alpha) = functional_rgb(value)?;
        return (alpha == 255.0).then_some(rgb);
    }
    color(value)
}

// Keep alpha unrounded on the 0–255 scale. Canvas stores it in an 8-bit
// channel; an opaque-only background must recognize partial alpha first.
fn functional_rgb(value: &str) -> Option<(u32, f64)> {
    if value.len() > 2048 {
        return None;
    }
    let body = value.strip_prefix("rgb(")
        .or_else(|| value.strip_prefix("rgba("))?
        .strip_suffix(')')?;
    let (channels, alpha, legacy) = if body.contains(',') {
        if body.contains('/') {
            return None;
        }
        let mut fields = body.split(',').map(|part| part.trim_matches(color_whitespace));
        let channels = [fields.next()?, fields.next()?, fields.next()?];
        let alpha = fields.next();
        if fields.next().is_some() {
            return None;
        }
        (channels, alpha, true)
    } else {
        let mut sections = body.split('/');
        let mut fields = sections
            .next()?
            .split(color_whitespace)
            .filter(|part| !part.is_empty());
        let channels = [fields.next()?, fields.next()?, fields.next()?];
        let alpha = sections.next().map(|part| part.trim_matches(color_whitespace));
        if fields.next().is_some() || sections.next().is_some() {
            return None;
        }
        (channels, alpha, false)
    };
    // Legacy comma syntax requires all three channels to use the same unit.
    if legacy && channels.iter().any(|part| part.ends_with('%') != channels[0].ends_with('%')) {
        return None;
    }
    let mut rgb = 0;
    for (channel, shift) in channels.into_iter().zip([16, 8, 0]) {
        let number = if let Some(percent) = channel.strip_suffix('%') {
            rgba_number(percent)?.clamp(0.0, 100.0) * 255.0 / 100.0
        } else {
            rgba_number(channel)?.clamp(0.0, 255.0)
        };
        rgb |= (number.round() as u32) << shift;
    }
    let alpha = match alpha {
        None => 255.0,
        Some(alpha) => match alpha.strip_suffix('%') {
            Some(percent) => rgba_number(percent)?.clamp(0.0, 100.0) * 255.0 / 100.0,
            None => rgba_number(alpha)?.clamp(0.0, 1.0) * 255.0,
        },
    };
    Some((rgb, alpha))
}

fn parse_border(value:&str)->Option<(Length,bool,Option<u32>,u8)> {
    let mut width = Length::Px(3.0);
    let mut solid = false;
    let(mut rgb,mut alpha)=(None,255);
    let(mut seen_width,mut seen_style,mut seen_color)=(false,false,false);
    for part in border_parts(value)? {
        if let Some(value) = border_length(part) {
            if seen_width{return None;}width=value;seen_width=true;
        } else if matches!(part,"solid"|"none"|"hidden") {
            if seen_style{return None;}solid=part=="solid";seen_style=true;
        } else if let Some((value,opacity)) = border_color_value(part) {
            if seen_color{return None;}rgb=value;alpha=opacity;seen_color=true;
        } else {
            return None;
        }
    }
    Some((width,solid,rgb,alpha))
}

fn color(value: &str) -> Option<u32> {
    if let Some(hex) = value.strip_prefix('#') {
        return match hex.len() {
            3 => {
                let value = u32::from_str_radix(hex, 16).ok()?;
                Some(
                    ((value >> 8) & 15) * 0x110000
                        + ((value >> 4) & 15) * 0x1100
                        + (value & 15) * 0x11,
                )
            }
            6 => u32::from_str_radix(hex, 16).ok(),
            _ => None,
        };
    }
    if let Some(body) = value
        .strip_prefix("rgb(")
        .and_then(|body| body.strip_suffix(')'))
    {
        let channels: Option<Vec<u32>> = body
            .split(|c: char| c == ',' || c.is_ascii_whitespace())
            .filter(|part| !part.is_empty())
            .map(|part| {
                let number = if let Some(percent) = part.strip_suffix('%') {
                    percent.parse::<f32>().ok()? * 2.55
                } else {
                    part.parse::<f32>().ok()?
                };
                number
                    .is_finite()
                    .then(|| number.clamp(0.0, 255.0).round() as u32)
            })
            .collect();
        let channels = channels?;
        if let [red, green, blue] = channels.as_slice() {
            return Some((red << 16) | (green << 8) | blue);
        }
        return None;
    }
    named_color(value)
}

fn named_color(value: &str) -> Option<u32> {
    Some(match value {
        "black" => 0x000000,
        "silver" => 0xc0c0c0,
        "gray" | "grey" => 0x808080,
        "white" => 0xffffff,
        "maroon" => 0x800000,
        "red" => 0xff0000,
        "purple" => 0x800080,
        "fuchsia" | "magenta" => 0xff00ff,
        "green" => 0x008000,
        "lime" => 0x00ff00,
        "olive" => 0x808000,
        "yellow" => 0xffff00,
        "navy" => 0x000080,
        "blue" => 0x0000ff,
        "teal" => 0x008080,
        "aqua" | "cyan" => 0x00ffff,
        "orange" => 0xffa500,
        "rebeccapurple" => 0x663399,
        "aliceblue" => 0xf0f8ff,
        "whitesmoke" => 0xf5f5f5,
        "lightgray" | "lightgrey" => 0xd3d3d3,
        "darkgray" | "darkgrey" => 0xa9a9a9,
        "darkslategray" | "darkslategrey" => 0x2f4f4f,
        "slategray" | "slategrey" => 0x708090,
        "steelblue" => 0x4682b4,
        "royalblue" => 0x4169e1,
        "cornflowerblue" => 0x6495ed,
        "tomato" => 0xff6347,
        "coral" => 0xff7f50,
        "gold" => 0xffd700,
        "pink" => 0xffc0cb,
        "brown" => 0xa52a2a,
        "beige" => 0xf5f5dc,
        "ivory" => 0xfffff0,
        _ => return None,
    })
}


#[cfg(test)]
mod background_tests {
    use super::*;

    fn node(doc: &Document, id: &str) -> usize {
        doc.nodes.iter().enumerate()
            .find_map(|(index, _)| (doc.attr(index, "id") == Some(id)).then_some(index))
            .unwrap()
    }

    fn assert_initial(style: &Style) {
        assert!(style.background_image.is_none());
        assert_eq!(style.background_repeat, [true, true]);
        assert_eq!(style.background_position, [ObjectPosition::START; 2]);
        assert_eq!(style.background_size, BackgroundSize::Auto);
    }

    #[test]
    fn opaque_functional_background_checks_opacity_before_byte_rounding() {
        for value in [
            "rgba(12,34,56,.999)", "rgb(12 34 56 / 99.9%)", "rgba(12,34,56,0)",
            "rgb(12,,34,56)", "rgba(12,34%,56,1)", "rgb(12 34 56 / 1.)",
        ] {
            assert_eq!(opaque_background_color(value), None, "{value}");
        }
        assert_eq!(opaque_background_color(&format!("rgba(12,34,56,{}1)", " ".repeat(2048))), None);
        assert_eq!(opaque_background_color("rgba(12,34,56)"), Some(0x0c2238));
        assert_eq!(opaque_background_color("rgb(12 34 56 / 120%)"), Some(0x0c2238));
        assert_eq!(parse_solid_color("rgba(12,34,56,.999)"), Some(ParsedColor::Solid(0xff0c2238)));
    }

    #[test]
    fn opaque_functional_background_variable_and_shorthand_share_cascade_rules() {
        let doc = crate::html::parse(r#"<style>
            .base{--paint:rgba(24,68,101,1);background-image:url(Tile.ppm);background-clip:content-box}
            #long{background-color:var(--paint)}#short{background:var(--paint)}
            #sheet{background:rgba(24,68,101,1)}#partial{background:rgba(24,68,101,.999)}
            </style><div class=base id=long></div><div class=base id=short></div>
            <div class=base id=sheet style='background-clip:revert-layer'></div>
            <div class=base id=partial style='background-clip:revert-layer'></div>"#);
        let styles = compute(&doc, &HashMap::new());
        let long = &styles[node(&doc, "long")];
        assert_eq!(long.background, Some(0x184465));
        assert_eq!(long.background_image.as_ref().unwrap().url, "Tile.ppm");
        assert_eq!(long.background_clip, Some(BackgroundClip::ContentBox));
        for id in ["short", "sheet"] {
            let style = &styles[node(&doc, id)];
            assert_eq!(style.background, Some(0x184465), "{id}");
            assert_eq!(style.background_clip, Some(BackgroundClip::BorderBox), "{id}");
            assert_initial(style);
        }
        let partial = &styles[node(&doc, "partial")];
        assert_eq!(partial.background, None);
        assert_eq!(partial.background_image, long.background_image);
        assert_eq!(partial.background_clip, Some(BackgroundClip::ContentBox));
    }

    #[test]
    fn background_url_preserves_case_and_decodes_existing_css_strings() {
        for (css, url) in [
            ("url(Pixels/CornerBG.PPM)", "Pixels/CornerBG.PPM"),
            ("URL(\"Mixed Case/CornerBG.PPM\")", "Mixed Case/CornerBG.PPM"),
            (r"url('Pixels/\43 ornerBG.PPM')", "Pixels/CornerBG.PPM"),
            ("url('Images/paren(1).ppm')", "Images/paren(1).ppm"),
            ("url(../Textures/Camel.ppm?V=AbC#Part)", "../Textures/Camel.ppm?V=AbC#Part"),
        ] {
            assert_eq!(background_image_url(css).as_deref(), Some(url), "{css}");
        }
    }

    #[test]
    fn background_url_rejects_malformed_or_unsupported_layers_without_guessing() {
        for css in [
            "", "none", "url()", "url('')", "url (a.ppm)", "url(a b.ppm)",
            "url(a.ppm", "url('a.ppm)", "url(a.ppm))", "url(a.ppm) red",
            "url(a.ppm),url(b.ppm)", "url(a.ppm) url(b.ppm)",
            "linear-gradient(red,blue)", "image-set(url(a.ppm) 1x)",
            "url('a\n.ppm')", "url(a\0.ppm)", r"url('a\a .ppm')",
        ] {
            assert!(background_image_url(css).is_none(), "{css:?}");
        }
    }

    #[test]
    fn background_winner_keeps_raw_url_and_exact_stylesheet_owner() {
        let doc = crate::html::parse(r#"
            <style id=first>
              #order{background-image:url(FirstCase.ppm)}
              #specific{background-image:url(SpecificCase.ppm)}
              .fallback{background-image:url(ClassCase.ppm)}
              #missing{background-image:url(UnmatchedCase.ppm)}
            </style>
            <link id=external rel=stylesheet href=sheet>
            <style id=last>
              #order{background-image:url(LastCase.PPM)}
              .fallback{background-image:url(LaterClass.ppm)}
            </style>
            <div id=order></div><div id=specific class=fallback></div>
            <div id=priority class=priority style="background-image:url(InlineLoses.ppm)"></div>
            <div id=inline-important class=priority style="background-image:url(InlineWins.PPM)!important"></div>
            <div id=inline style="background-image:url(InlineCase.PPM)"></div>
            <div id=invalid></div><div id=plain></div>
        "#);
        let external = HashMap::from([(node(&doc, "external"),
            ".priority{background-image:url(Pixels/ExternalCase.PPM)!important}#inline{background-image:url(SheetLoses.ppm)}#invalid{background-image:url(ValidCase.PPM);background-image:url(a.ppm),url(b.ppm)}".into())]);
        let styles = compute(&doc, &external);
        for (id, url, owner) in [
            ("order", "LastCase.PPM", Some(node(&doc, "last"))),
            ("specific", "SpecificCase.ppm", Some(node(&doc, "first"))),
            ("priority", "Pixels/ExternalCase.PPM", Some(node(&doc, "external"))),
            ("inline-important", "InlineWins.PPM", None),
            ("inline", "InlineCase.PPM", None),
            ("invalid", "ValidCase.PPM", Some(node(&doc, "external"))),
        ] {
            assert_eq!(styles[node(&doc, id)].background_image,
                Some(BackgroundImage { url: url.into(), stylesheet: owner }), "{id}");
        }
        assert!(styles[node(&doc, "plain")].background_image.is_none());
    }

    #[test]
    fn background_is_noninherited_and_explicit_inherit_keeps_base_owner() {
        let doc = crate::html::parse(r#"
            <style id=sheet>#parent{background-image:url(Pixels/ParentCase.ppm);background-repeat:repeat-x;background-position:right bottom;background-size:cover;background-color:#123456}</style>
            <div id=parent>
              <div id=normal></div>
              <div id=inherited style="background-image:inherit;background-repeat:inherit;background-position:inherit;background-size:inherit;background-color:inherit"></div>
              <div id=shorthand style="background:inherit"></div>
              <div id=unset style="background-image:url(Own.ppm);background-repeat:no-repeat;background-position:center;background-size:contain;background-image:unset;background-repeat:unset;background-position:unset;background-size:unset"></div>
            </div>
        "#);
        let styles = compute(&doc, &HashMap::new());
        let parent = &styles[node(&doc, "parent")];
        assert_initial(&styles[node(&doc, "normal")]);
        assert_eq!(styles[node(&doc, "normal")].background, None);
        assert_initial(&styles[node(&doc, "unset")]);
        for id in ["inherited", "shorthand"] {
            let inherited = &styles[node(&doc, id)];
            assert_eq!(inherited.background_image, parent.background_image, "{id}");
            assert_eq!(inherited.background_image.as_ref().unwrap().stylesheet, Some(node(&doc, "sheet")));
            assert_eq!(inherited.background_repeat, [true, false]);
            assert_eq!(inherited.background_position, parent.background_position);
            assert_eq!(inherited.background_size, BackgroundSize::Cover);
            assert_eq!(inherited.background, Some(0x123456));
        }
    }

    #[test]
    fn background_color_shorthand_resets_layer_but_longhands_and_invalid_compounds_do_not() {
        let doc = crate::html::parse(r#"
            <style id=sheet>.seed{background-image:url(SeedCase.ppm);background-repeat:no-repeat;background-position:center;background-size:cover;background-color:#112233}</style>
            <div id=color class=seed style="background-color:#abcdef"></div>
            <div id=reset class=seed style="background:#abcdef"></div>
            <div id=transparent class=seed style="background:transparent"></div>
            <div id=none class=seed style="background:none"></div>
            <div id=initial class=seed style="background:initial"></div>
            <div id=unset class=seed style="background:unset"></div>
            <div id=invalid class=seed style="background:url(Other.ppm) red"></div>
            <div id=image-none class=seed style="background-image:none"></div>
        "#);
        let styles = compute(&doc, &HashMap::new());
        let color = &styles[node(&doc, "color")];
        assert_eq!(color.background, Some(0xabcdef));
        assert_eq!(color.background_image.as_ref().unwrap().url, "SeedCase.ppm");
        assert_eq!(color.background_image.as_ref().unwrap().stylesheet, Some(node(&doc, "sheet")));
        assert_eq!(color.background_repeat, [false, false]);
        assert_eq!(color.background_size, BackgroundSize::Cover);
        assert_initial(&styles[node(&doc, "reset")]);
        assert_eq!(styles[node(&doc, "reset")].background, Some(0xabcdef));
        for id in ["transparent", "none", "initial", "unset"] {
            assert_initial(&styles[node(&doc, id)]);
            assert_eq!(styles[node(&doc, id)].background, None, "{id}");
        }
        let invalid = &styles[node(&doc, "invalid")];
        assert_eq!(invalid.background, Some(0x112233));
        assert_eq!(invalid.background_image, color.background_image);
        assert_eq!(invalid.background_repeat, [false, false]);
        assert_eq!(invalid.background_position, [ObjectPosition::CENTER; 2]);
        assert_eq!(invalid.background_size, BackgroundSize::Cover);
        let none = &styles[node(&doc, "image-none")];
        assert!(none.background_image.is_none());
        assert_eq!(none.background_repeat, [false, false]);
        assert_eq!(none.background_position, [ObjectPosition::CENTER; 2]);
        assert_eq!(none.background_size, BackgroundSize::Cover);
        assert_eq!(none.background, Some(0x112233));
    }

    #[test]
    fn background_repeat_position_and_size_accept_only_the_finite_supported_values() {
        for (repeat, expected) in [
            ("repeat", [true, true]), ("no-repeat", [false, false]),
            ("repeat-x", [true, false]), ("repeat-y", [false, true]),
        ] {
            let doc = crate::html::parse(&format!("<div id=target style='background-repeat:{repeat};background-repeat:round'>T</div>"));
            assert_eq!(compute(&doc, &HashMap::new())[node(&doc, "target")].background_repeat, expected);
        }
        for (size, expected) in [
            ("auto", BackgroundSize::Auto), ("contain", BackgroundSize::Contain),
            ("cover", BackgroundSize::Cover),
        ] {
            let doc = crate::html::parse(&format!("<div id=target style='background-size:{size};background-size:32px 24px'>T</div>"));
            assert_eq!(compute(&doc, &HashMap::new())[node(&doc, "target")].background_size, expected);
        }
        let doc = crate::html::parse("<div id=target style='background-position:right 2px bottom 25%;background-position:top 4px right;background-repeat:repeat-y;background-repeat:repeat no-repeat;background-size:contain;background-size:contain,cover'>T</div>");
        let styles = compute(&doc, &HashMap::new());
        let style = &styles[node(&doc, "target")];
        assert_eq!(style.background_position, [
            ObjectPosition { fraction: 1.0, offset: Length::Px(-2.0) },
            ObjectPosition { fraction: 0.75, offset: Length::Px(0.0) },
        ]);
        assert_eq!(style.background_repeat, [false, true]);
        assert_eq!(style.background_size, BackgroundSize::Contain);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn shared_stylesheet_declarations_beyond_old_prefix_apply_or_refuse_whole_block() {
        let doc=crate::html::parse("<link href=sheet><p id=target>Target</p>");
        let node=doc.nodes.iter().enumerate().find_map(|(n,_)|(doc.attr(n,"id")==Some("target")).then_some(n)).unwrap();
        let link=doc.nodes.iter().enumerate().find_map(|(n,_)|(doc.attr(n,"href")==Some("sheet")).then_some(n)).unwrap();
        let source=format!("#target{{width:1px;{}width:99px}}","height:1px;".repeat(512));
        let mut external=HashMap::from([(link,source)]);assert_eq!(compute(&doc,&external)[node].width,Length::Px(99.0));
        external.insert(link,format!("#target{{width:1px;{}width:99px}}","height:1px;".repeat(MAX_DECLARATIONS)));
        assert_eq!(compute(&doc,&external)[node].width,Length::Auto);assert!(stylesheet_diagnostics(&doc,&external)[0].contains("declaration allowance"));
    }

    #[test]
    fn shared_stylesheet_candidate_rechecks_changed_inline_sources_before_publication() {
        let mut doc=crate::html::parse("<style>/*changed after request*/</style><link rel=stylesheet href=sheet><div id=target>T</div>");
        doc.stylesheet_limits=StylesheetLimits{sheet_bytes:128,document_bytes:100,rules:8};
        let link=doc.nodes.iter().enumerate().find_map(|(n,_)|(doc.attr(n,"href")==Some("sheet")).then_some(n)).unwrap();
        let text=doc.nodes.iter().position(|node|matches!(&node.kind,NodeKind::Text(value) if value.contains("changed after request"))).unwrap();
        let source="#target{width:77px}";let external=HashMap::new();
        assert!(check_stylesheet_candidate(&doc,&external,link,source).is_ok());
        doc.nodes[text].kind=NodeKind::Text(format!("/*{}*/#target{{width:199px}}","x".repeat(70)).into());
        assert!(check_stylesheet_candidate(&doc,&external,link,source).unwrap_err().contains("per document"));
        assert!(external.is_empty(),"Preflight must not publish the source");
        doc.nodes[text].kind=NodeKind::Text(String::new().into());assert!(check_stylesheet_candidate(&doc,&external,link,source).is_ok());
    }

    #[test]
    fn shared_stylesheet_parser_admits_complete_sources_or_reports_atomic_refusal() {
        let mut doc=crate::html::parse("<link rel=stylesheet href=sheet><div id=target>Target</div>");
        let link=doc.nodes.iter().enumerate().find_map(|(n,_)|(doc.attr(n,"href")==Some("sheet")).then_some(n)).unwrap();
        let target=doc.nodes.iter().enumerate().find_map(|(n,_)|(doc.attr(n,"id")==Some("target")).then_some(n)).unwrap();
        let source=String::from_utf8(crate::compression::gzip(include_bytes!("../tools/fixtures/large-tail.css.gz"),8*1024*1024,&||Ok(())).unwrap()).unwrap();
        let mut external=HashMap::from([(link,source)]);
        assert_eq!(compute(&doc,&external)[target].width,Length::Px(173.0));
        assert_eq!(stylesheet_usage(&doc,&external).rules,4097);
        doc.stylesheet_limits=StylesheetLimits{sheet_bytes:2*1024*1024,document_bytes:2*1024*1024,rules:32};
        assert_eq!(compute(&doc,&external)[target].width,Length::Auto);assert!(stylesheet_diagnostics(&doc,&external)[0].contains("parser block allowance"));
        external.insert(link,"#target{width:47px}".into());assert_eq!(compute(&doc,&external)[target].width,Length::Px(47.0));
        let limits=StylesheetLimits::default();assert!(limits.accept("a{}",StylesheetUsage{bytes:usize::MAX,rules:0}).is_err());assert!(limits.accept("a{}",StylesheetUsage{bytes:0,rules:usize::MAX}).is_err());
        let huge_comment=format!("/*{}*/@font-face{{font-family:TailFace;src:local(TailFace)}}","x".repeat(1500000));
        assert_eq!(font_faces(&huge_comment)[0].family,"TailFace");
    }

    #[test]
    fn image_rendering_inherits_and_preserves_valid_cascade_values() {
        use super::ImageRendering::*;
        let doc = crate::html::parse("<style>img{image-rendering:auto}.sharp{image-rendering:pixelated!important}</style><div style='image-rendering:crisp-edges'><span id=inherited></span><img id=auto><img id=unset style='image-rendering:unset'><img id=smooth style='image-rendering:smooth;image-rendering:bogus'><img id=pixel class=sharp style='image-rendering:smooth'><img id=initial style='image-rendering:initial'><img id=quality style='image-rendering:high-quality'><img id=speed style='image-rendering:optimizeSpeed'><img id=legacy-quality style='image-rendering:optimizeQuality'></div>");
        let styles = super::compute(&doc, &std::collections::HashMap::new());
        for (id, expected) in [
            ("inherited", CrispEdges),
            ("auto", Auto),
            ("unset", CrispEdges),
            ("smooth", Smooth),
            ("pixel", Pixelated),
            ("initial", Auto),
            ("quality", HighQuality),
            ("speed", CrispEdges),
            ("legacy-quality", Smooth),
        ] {
            let node = doc
                .nodes
                .iter()
                .enumerate()
                .find(|(i, _)| doc.attr(*i, "id") == Some(id))
                .unwrap()
                .0;
            assert_eq!(styles[node].image_rendering, expected, "{id}");
        }
    }
    #[test]
    fn image_fit_and_position_parse_axis_keywords_offsets_and_cascade() {
        use super::{Length, ObjectFit, ObjectPosition};
        let axis = |fraction, px| ObjectPosition {
            fraction,
            offset: Length::Px(px),
        };
        let center = axis(0.5, 0.0);
        for (text, expected) in [
            ("center", [center, center]),
            ("top", [center, axis(0.0, 0.0)]),
            ("25%", [axis(0.25, 0.0), center]),
            ("bottom right", [axis(1.0, 0.0), axis(1.0, 0.0)]),
            ("left 10px", [axis(0.0, 0.0), axis(0.0, 10.0)]),
            ("10px top", [axis(0.0, 10.0), axis(0.0, 0.0)]),
            ("right 10px bottom 20%", [axis(1.0, -10.0), axis(0.8, 0.0)]),
            ("left -3px bottom 4px", [axis(0.0, -3.0), axis(1.0, -4.0)]),
            ("top 7px right 0", [axis(1.0, 0.0), axis(0.0, 7.0)]),
            ("-50% 125%", [axis(-0.5, 0.0), axis(1.25, 0.0)]),
        ] {
            assert_eq!(super::object_position(text), Some(expected), "{text}");
        }
        for text in [
            "",
            "center bottom 4px",
            "left -3px center",
            "top 7px right",
            "top 10px",
            "10px left",
            "right left",
            "10px center right",
            "left top bottom",
            "left 4px right 5px",
            "center 3px bottom",
            "auto",
            "NaN",
            "infinitypx",
        ] {
            assert!(super::object_position(text).is_none(), "{text}");
        }
        assert_eq!(
            super::object_position("right 2em bottom 1rem").unwrap(),
            [
                ObjectPosition {
                    fraction: 1.0,
                    offset: Length::Em(-2.0)
                },
                ObjectPosition {
                    fraction: 1.0,
                    offset: Length::Rem(-1.0)
                }
            ]
        );
        let doc=crate::html::parse("<div style='object-fit:cover;object-position:left top'><img id=a style='object-fit:contain;object-fit:bogus;object-position:25% 75%;object-position:top 4px'><img id=b><img id=c style='object-fit:inherit;object-position:inherit'></div>");
        let styles = super::compute(&doc, &std::collections::HashMap::new());
        let node = |id| {
            doc.nodes
                .iter()
                .enumerate()
                .find(|(i, _)| doc.attr(*i, "id") == Some(id))
                .unwrap()
                .0
        };
        assert_eq!(styles[node("a")].object_fit, ObjectFit::Contain);
        assert_eq!(
            styles[node("a")].object_position,
            [axis(0.25, 0.0), axis(0.75, 0.0)]
        );
        assert_eq!(styles[node("b")].object_fit, ObjectFit::Fill);
        assert_eq!(styles[node("b")].object_position, [center, center]);
        assert_eq!(styles[node("c")].object_fit, ObjectFit::Cover);
        assert_eq!(styles[node("c")].object_position, [axis(0.0, 0.0); 2]);
    }

    #[test]
    fn token_attribute_selectors_match_complete_ascii_words_and_preserve_case_rules() {
        for (attribute,selector,expected) in [
            ("rel='alternate STYLESHEET'","link[rel~=\"stylesheet\"]",true),
            ("rel='alternate STYLESHEET'","link[rel~=stylesheet s]",false),
            ("rel='alternate STYLESHEET'","link[rel~=STYLESHEET S]",true),
            ("rel='alternate STYLESHEET'","link[rel~=stylesheet I]",true),
            ("rel='xstylesheet stylesheetx'","[rel~=stylesheet]",false),
            ("rel=''","[rel~='']",false),
            ("rel='alternate stylesheet'","[rel~='alternate stylesheet']",false),
            ("rel='alternate stylesheet'","[rel~='\t']",false),
            ("data-words='left&#9;ready&#10;middle&#12;other&#13;last end'","[data-words~=ready]",true),
            ("data-words='left&#9;ready&#10;middle&#12;other&#13;last end'","[data-words~=other]",true),
            ("data-words='left&#11;ready'","[data-words~=ready]",false),
            ("data-words='left&#160;ready'","[data-words~=ready]",false),
            ("data-words='left&#160;ready'","[data-words~='left\u{a0}ready']",true),
            ("data-words='Ready'","[data-words~=ready]",false),
            ("data-words='Ready'","[data-words~=ready i]",true),
            ("data-words='Ä'","[data-words~='ä' i]",false),
            ("data-words='Ä'","[data-words~='Ä' i]",true),
            ("data-empty=''","[data-words~=ready]",false),
        ] {
            assert!(super::valid_query_selector(selector),"{selector}");
            let doc=parse(&format!("<style>{selector}{{color:#123456}}</style><link id=target {attribute}>"));let node=doc.nodes.iter().position(|item|matches!(&item.kind,NodeKind::Element {id:Some(id),..} if id=="target")).unwrap();
            assert_eq!(compute(&doc,&HashMap::new())[node].color==0x123456,expected,"{attribute} {selector}");let mut budget=1_000_000;assert_eq!(super::query_first(&doc,selector,&[node],None,&mut budget).unwrap().is_some(),expected,"{selector}");
        }
    }

    #[test]
    fn query_text_scans_share_the_job_budget_without_charging_unread_values() {
        let large_class = "unrelated ".repeat(190_000);
        let doc = crate::html::parse(&format!("<style>.missing{{color:red}} div{{color:#123456}}</style><div class='{large_class}'></div>"));
        let node = doc
            .nodes
            .iter()
            .position(|node| {
                matches!(&node.kind,
            crate::html::NodeKind::Element { tag, .. } if tag == "div")
            })
            .unwrap();
        let mut budget = 100_000;
        assert_eq!(
            super::query_first(&doc, "div", &[node], None, &mut budget).unwrap(),
            Some(node)
        );
        // A tag query does not inspect the huge unrelated class attribute.
        assert!(budget > 99_000);
        assert_eq!(
            super::query_first(&doc, ".missing", &[node], None, &mut budget).unwrap(),
            None
        );
        assert!(super::query_first(&doc, ".missing", &[node], None, &mut budget).is_err());
        assert_eq!(budget, 0);
        // The rendering path keeps its established selector budget behavior.
        assert_eq!(
            super::compute(&doc, &std::collections::HashMap::new())[node].color,
            0x123456
        );
        let mut budget=1000; // Explicit small text-scan profile, not a normal cap.
        assert!(super::query_first(&doc,"[class~=missing]",&[node],None,&mut budget).is_err());assert_eq!(budget,0);
        assert!(super::query_first(&doc,"[class~=missing]",&[node],None,&mut budget).is_err());let mut fresh=1000;assert_eq!(super::query_first(&doc,"div",&[node],None,&mut fresh).unwrap(),Some(node));
    }

    #[test]
    fn repeated_compounds_ancestors_and_attribute_lookups_cannot_hide_query_work() {
        let attrs = (0..256)
            .map(|index| format!(" data-{index}=x"))
            .collect::<String>();
        let doc = crate::html::parse(&format!(
            "<div{attrs} class='{}'><div class='{}'><span></span></div></div>",
            "hit ".repeat(2048),
            "hit ".repeat(2048)
        ));
        let node = doc
            .nodes
            .iter()
            .position(|node| {
                matches!(&node.kind,
            crate::html::NodeKind::Element { tag, .. } if tag == "span")
            })
            .unwrap();
        for selector in [
            ".hit.hit.hit.hit.hit.hit.hit.hit span",
            "[data-missing] span",
        ] {
            let mut budget = 1000;
            let mut exhausted = false;
            for _ in 0..8 {
                if super::query_first(&doc, selector, &[node], None, &mut budget).is_err() {
                    exhausted = true;
                    break;
                }
            }
            assert!(exhausted, "unaccounted repeated matching for {selector}");
            assert_eq!(budget, 0);
        }
    }

    #[test]
    fn overflow_hidden_is_noninherited_and_invalid_values_preserve_cascade() {
        let doc = crate::html::parse("<div id=parent style='overflow:hidden'><div id=normal>N</div><div id=inherited style='overflow:inherit'>I</div><div id=reset style='overflow:hidden;overflow:unset'>R</div><div id=invalid style='overflow:hidden;overflow:potato'>X</div></div>");
        let styles = super::compute(&doc, &std::collections::HashMap::new());
        for (id, expected) in [
            ("parent", OverflowValue::Hidden),
            ("normal", OverflowValue::Visible),
            ("inherited", OverflowValue::Hidden),
            ("reset", OverflowValue::Visible),
            ("invalid", OverflowValue::Hidden),
        ] {
            let node = doc.nodes.iter().position(|node| matches!(&node.kind, crate::html::NodeKind::Element { id: Some(value), .. } if value == id)).unwrap();
            assert_eq!(styles[node].overflow, Overflow { x: expected, y: expected }, "{id}");
        }
    }
    use super::*;
    use crate::html::parse;

    #[test]
    fn legacy_table_hints_and_center_yield_to_author_css() {
        let doc = parse("<style>.override{text-align:right;padding:7px;width:80px}</style><center id=outer><table id=table cellpadding=3 cellspacing=4 width=50%><tr><td id=cell align=center nowrap width=25% class=override>X</td></tr></table></center>");
        let styles = compute(&doc, &HashMap::new());
        let by_id = |id: &str| {
            &styles[doc
                .nodes
                .iter()
                .position(
                    |n| matches!(&n.kind, NodeKind::Element {id:Some(actual), ..} if actual == id),
                )
                .unwrap()]
        };
        assert_eq!(by_id("outer").display, Display::Block);
        assert_eq!(by_id("outer").legacy_align, Some(Align::Center));
        assert_eq!(by_id("table").display, Display::Table);
        assert_eq!(by_id("table").width, Length::Percent(50.0));
        assert_eq!(by_id("table").border_spacing, [Length::Px(4.0); 2]);
        assert_eq!(by_id("cell").display, Display::TableCell);
        assert_eq!(by_id("cell").align, Align::Right);
        assert_eq!(by_id("cell").legacy_align, None);
        assert_eq!(by_id("cell").white_space, WhiteSpace::NoWrap);
        assert_eq!(by_id("cell").padding, [Length::Px(7.0); 4]);
        assert_eq!(by_id("cell").width, Length::Px(80.0));
    }

    #[test]
    fn font_shorthand_size_line_height_and_absolute_units_are_validated_together() {
        let doc = parse("<div id=good style=\"font:italic 700 12pt/1.25 'A Font',sans-serif;width:2pc\"><span id=reset style='font:400 13px/27px Arial,sans-serif'>A</span></div><div id=bad style='font-size:17px;font-weight:bold;font:30px / 1 / Arial'></div><div id=bad2 style='font-size:17px;font-weight:bold;font:30px Arial / 2'></div>");
        let styles = compute(&doc, &HashMap::new());
        let by_id = |id: &str| {
            &styles[doc
                .nodes
                .iter()
                .position(
                    |n| matches!(&n.kind, NodeKind::Element {id:Some(actual), ..} if actual == id),
                )
                .unwrap()]
        };
        let good = by_id("good");
        assert_eq!(good.font_size, 16.0);
        assert_eq!(good.font_families, ["A Font", "sans-serif"]);
        assert_eq!(good.font_weight, 700);
        assert!(good.bold && good.italic);
        assert!(matches!(good.line_height, LineHeight::Factor(1.25)));
        assert_eq!(good.width, Length::Px(32.0));
        let reset = by_id("reset");
        assert_eq!(reset.font_families, ["Arial", "sans-serif"]);
        assert_eq!(reset.font_weight, 400);
        assert_eq!(reset.font_size, 13.0);
        assert!(!reset.bold && !reset.italic);
        assert!(matches!(
            reset.line_height,
            LineHeight::Length(Length::Px(27.0))
        ));
        for id in ["bad", "bad2"] {
            assert_eq!(by_id(id).font_size, 17.0);
            assert!(by_id(id).bold);
        }
    }

    #[test]
    fn font_families_inherit_and_relative_weights_select_numeric_faces() {
        let doc = parse(
            r#"<div style="font-family:'Case Sensitive URL Is Separate', Times New Roman, serif;font-weight:500"><span id=child style="font-weight:bolder">A</span><span id=light style="font-weight:lighter;font-family:'Quoted, Family',monospace">V</span></div><font id=legacy face="Verdana,Arial">A</font><font id=legacyoverride face=Verdana style="font-family:serif">A</font>"#,
        );
        let styles = compute(&doc, &HashMap::new());
        let by_id = |id: &str| {
            &styles[doc
                .nodes
                .iter()
                .position(
                    |n| matches!(&n.kind, NodeKind::Element {id:Some(actual),..} if actual == id),
                )
                .unwrap()]
        };
        assert_eq!(
            by_id("child").font_families,
            ["Case Sensitive URL Is Separate", "Times New Roman", "serif"]
        );
        assert_eq!(by_id("child").font_weight, 700);
        assert_eq!(by_id("light").font_weight, 100);
        assert_eq!(by_id("legacy").font_families, ["Verdana", "Arial"]);
        assert_eq!(by_id("legacyoverride").font_families, ["serif"]);
        assert_eq!(
            by_id("light").font_families,
            ["Quoted, Family", "monospace"]
        );
        assert_eq!(
            font_families(r#""A\20 Font", sans-serif"#).unwrap(),
            ["A Font", "sans-serif"]
        );
        assert!(font_families("Arial,,serif").is_none());
    }

    #[test]
    fn font_face_sources_keep_case_alternatives_and_unicode_subsets() {
        let faces = font_faces(
            r#"/* ignored */ @media print { @font-face{font-family:Never;src:url(print.ttf)} }
        @font-face { font-family:'Page Font';font-weight:700;font-style:italic;
          src:local('Local Font'),url('../Fonts/Case.WOFF2?v=1') format('woff2'),url(../Fonts/Case.ttf) format(truetype);
          unicode-range:U+0041-005A, U+E9, U+1F6??; }
        @font-face {font-family:Broken;src:url(a.ttf);unicode-range:U+110000;}
        p{font-family:'Page Font'}"#,
        );
        assert_eq!(faces.len(), 2);
        let face = &faces[0];
        assert!(face.error.is_none());
        assert_eq!(face.family, "Page Font");
        assert_eq!(face.weight, 700);
        assert!(face.italic);
        assert_eq!(face.ranges, [(65, 90), (233, 233), (0x1f600, 0x1f6ff)]);
        assert!(matches!(&face.sources[0],FontSource::Local(name) if name == "Local Font"));
        assert!(
            matches!(&face.sources[1],FontSource::Url{url,format:Some(format),..} if url == "../Fonts/Case.WOFF2?v=1" && format == "woff2")
        );
        assert!(matches!(&face.sources[2],FontSource::Url{url,..} if url == "../Fonts/Case.ttf"));
        assert!(faces[1].error.as_ref().unwrap().contains("unicode-range"));
    }

    #[test]
    fn flex_properties_shorthands_validation_and_noninheritance() {
        let doc = parse("<div id=parent style='display:flex;flex-flow:column-reverse wrap;flex:2 3 25%;gap:8px 2em;align-content:normal;align-items:center;justify-content:start;box-sizing:border-box;min-width:0;min-height:12px;max-height:200px'><span id=child>X</span></div>");
        let styles = compute(&doc, &HashMap::new());
        let by_id = |id: &str| {
            &styles[doc.nodes.iter().position(|n| matches!(&n.kind, NodeKind::Element { id:Some(actual), .. } if actual == id)).unwrap()]
        };
        let parent = by_id("parent");
        assert_eq!(parent.display, Display::Flex);
        assert_eq!(parent.flex_direction, FlexDirection::ColumnReverse);
        assert!(parent.flex_wrap && parent.border_box);
        assert_eq!(
            (parent.flex_grow, parent.flex_shrink, parent.flex_basis),
            (2.0, 3.0, Length::Percent(25.0))
        );
        assert_eq!(parent.gap, [Length::Px(8.0), Length::Em(2.0)]);
        assert_eq!(parent.align_content, FlexPack::Stretch);
        assert_eq!(parent.justify_content, FlexPack::LogicalStart);
        assert_eq!(parent.align_items, FlexAlign::Center);
        assert_eq!(parent.min_width, Length::Px(0.0));
        assert_eq!(parent.min_height, Length::Px(12.0));
        assert_eq!(parent.max_height, Some(Length::Px(200.0)));
        let child = by_id("child");
        assert_eq!(
            (
                child.display,
                child.flex_direction,
                child.flex_grow,
                child.flex_shrink
            ),
            (Display::Inline, FlexDirection::Row, 0.0, 1.0)
        );
        assert_eq!(child.gap, [Length::Px(0.0); 2]);
        assert!(!child.border_box);
        for invalid in [
            "1 10px 2",
            "1 auto 2",
            "1 2 3 4",
            "-1",
            "1 -2",
            "NaN",
            "1 2 -3px",
            "auto auto",
        ] {
            assert!(flex_shorthand(invalid).is_none(), "{invalid}");
        }
        assert_eq!(
            flex_shorthand("10px 1 2"),
            Some((1.0, 2.0, Length::Px(10.0)))
        );
        assert_eq!(flex_shorthand("0 0 0"), Some((0.0, 0.0, Length::Px(0.0))));
        assert_eq!(flex_shorthand("2"), Some((2.0, 1.0, Length::Percent(0.0))));
        assert_eq!(flex_shorthand("none"), Some((0.0, 0.0, Length::Auto)));
    }

    #[test]
    fn positioning_offsets_are_atomic_bounded_and_not_inherited() {
        let doc = parse(concat!(
            "<style>#parent{position:relative;inset:1px 2% -3em auto}",
            "#override{position:absolute;inset:8px;inset:1px broken;left:-4px;top:nan;position:sticky}",
            "#inherit{position:inherit;inset:inherit}#invalid{position:absolute;position:sideways}</style>",
            "<div id=parent><span id=child>Child</span><span id=inherit>Inherited</span></div>",
            "<div id=override>Override</div><div id=invalid>Invalid keyword</div>",
        ));
        let styles = compute(&doc, &HashMap::new());
        let by_id = |id: &str| {
            &styles[doc.nodes.iter().position(|node| matches!(&node.kind, NodeKind::Element { id: Some(actual), .. } if actual == id)).unwrap()]
        };
        assert_eq!(by_id("parent").position, Position::Relative);
        assert_eq!(
            by_id("parent").inset,
            [
                Length::Px(1.0),
                Length::Percent(2.0),
                Length::Em(-3.0),
                Length::Auto
            ]
        );
        assert_eq!(by_id("child").position, Position::Static);
        assert_eq!(by_id("child").inset, [Length::Auto; 4]);
        assert_eq!(by_id("inherit").position, Position::Relative);
        assert_eq!(by_id("inherit").inset, by_id("parent").inset);
        assert_eq!(by_id("override").position, Position::Sticky);
        assert_eq!(by_id("invalid").position, Position::Absolute);
        assert_eq!(
            by_id("override").inset,
            [
                Length::Px(8.0),
                Length::Px(8.0),
                Length::Px(8.0),
                Length::Px(-4.0)
            ]
        );
    }

    #[test]
    fn cascade_respects_specificity_source_order_inline_and_important() {
        let doc = parse("<style>p{color:red}.note{color:blue}#a{color:green}#a{color:purple}p{font-weight:bold!important}</style><p id=a class=note style='color:#123456;font-weight:normal'>Text</p>");
        let styles = compute(&doc, &HashMap::new());
        let node = doc
            .nodes
            .iter()
            .position(
                |node| matches!(&node.kind, NodeKind::Element { id: Some(id), .. } if id == "a"),
            )
            .unwrap();
        assert_eq!(styles[node].color, 0x123456);
        assert!(styles[node].bold);
    }

    #[test]
    fn selectors_distinguish_descendants_children_and_class_tokens() {
        let doc = parse("<style>.box > p{color:red}.box span em{color:blue}.ox{font-weight:bold}</style><div class=box><p id=a>A</p><span><p id=b>B</p><em id=c>C</em></span></div>");
        let styles = compute(&doc, &HashMap::new());
        let by_id = |id: &str| {
            styles[doc.nodes.iter().position(|node| matches!(&node.kind, NodeKind::Element { id: Some(actual), .. } if actual == id)).unwrap()].clone()
        };
        assert_eq!(by_id("a").color, 0xff0000);
        assert_ne!(by_id("b").color, 0xff0000);
        assert_eq!(by_id("c").color, 0x0000ff);
        assert!(!by_id("a").bold);
    }

    #[test]
    fn sibling_selectors_skip_text_stay_in_the_parent_and_backtrack() {
        let doc = parse(
            "<style>
            #source + .item {color:red}
            #source ~ .item {font-weight:bold}
            .root > .gate + .group .target {color:blue}
            .lead + .mid ~ #backtrack {margin-left:11px}
            </style>
            <section class=root>
              <span id=before class=item>Before</span>
              <input id=source>
              text <!-- element adjacency ignores this -->
              <label id=adjacent class=item>Adjacent</label>
              <span id=later class=item>Later</span>
              <div><span id=nested class=item>Other parent</span></div>
              <div class=gate></div><div class=group><em id=mixed class=target>Mixed</em></div>
              <span class=lead></span><i class=mid></i><i class=mid></i><b id=backtrack>Backtrack</b>
            </section><span id=outside class=item>Outside</span>",
        );
        let styles = compute(&doc, &HashMap::new());
        let by_id = |id: &str| {
            &styles[doc.nodes.iter().position(|node| matches!(&node.kind, NodeKind::Element { id: Some(actual), .. } if actual == id)).unwrap()]
        };
        assert_eq!(by_id("adjacent").color, 0xff0000);
        assert!(by_id("adjacent").bold);
        assert_ne!(by_id("later").color, 0xff0000);
        assert!(by_id("later").bold);
        for id in ["before", "nested", "outside"] {
            assert!(!by_id(id).bold, "{id}");
            assert_ne!(by_id(id).color, 0xff0000, "{id}");
        }
        assert_eq!(by_id("mixed").color, 0x0000ff);
        assert_eq!(by_id("backtrack").margin[3], Length::Px(11.0));
    }

    #[test]
    fn checked_sibling_accordion_uses_live_state_without_changing_attributes() {
        let doc = parse(
            "<style>
            .panel {max-height:0;overflow:hidden}
            .accordion input:checked ~ .panel {max-height:14rem}
            .accordion input:CHECKED ~ .label {color:#636363}
            input[checked] {margin-top:7px}
            </style>
            <div class=accordion><input id=on type=checkbox checked>
              <label id=label class=label>Toggle</label><div id=panel class=panel>Details</div>
            </div>
            <div class=accordion><input id=off type=checkbox>
              <div id=other class=panel>Other details</div>
            </div>",
        );
        let index = |id: &str| {
            doc.nodes.iter().position(|node| matches!(&node.kind, NodeKind::Element { id: Some(actual), .. } if actual == id)).unwrap()
        };
        let on = index("on");
        let off = index("off");
        let initial = compute(&doc, &HashMap::new());
        assert_eq!(initial[index("panel")].max_height, Some(Length::Rem(14.0)));
        assert_eq!(initial[index("label")].color, 0x636363);
        assert_eq!(initial[index("other")].max_height, Some(Length::Px(0.0)));
        let checked = |node| {
            if node == on {
                Some(false)
            } else if node == off {
                Some(true)
            } else {
                None
            }
        };
        let changed = compute_with_state(&doc, &HashMap::new(), Some(&checked));
        assert_eq!(changed[index("panel")].max_height, Some(Length::Px(0.0)));
        assert_ne!(changed[index("label")].color, 0x636363);
        assert_eq!(changed[index("other")].max_height, Some(Length::Rem(14.0)));
        assert_eq!(changed[index("panel")].overflow, Overflow { x: OverflowValue::Hidden, y: OverflowValue::Hidden });
        assert_eq!(changed[on].margin[0], Length::Px(7.0));
        assert_eq!(changed[off].margin[0], Length::Px(0.0));
    }

    #[test]
    fn checked_only_applies_to_checkable_inputs_and_options() {
        let doc = parse(
            "<style>:checked{color:#123456}</style>
            <input id=checkbox type=CHECKBOX checked><input id=radio type=radio checked>
            <input id=text type=text checked><input id=missing checked>
            <div id=div checked>Not a control</div>
            <option id=selected selected>Selected</option><option id=unselected>Not selected</option>",
        );
        let index = |id: &str| {
            doc.nodes.iter().position(|node| matches!(&node.kind, NodeKind::Element { id: Some(actual), .. } if actual == id)).unwrap()
        };
        let initial = compute(&doc, &HashMap::new());
        for id in ["checkbox", "radio", "selected"] {
            assert_eq!(initial[index(id)].color, 0x123456, "{id}");
        }
        for id in ["text", "missing", "div", "unselected"] {
            assert_ne!(initial[index(id)].color, 0x123456, "{id}");
        }
        let checked = |_| Some(true);
        let all_checked = compute_with_state(&doc, &HashMap::new(), Some(&checked));
        assert_eq!(all_checked[index("unselected")].color, 0x123456);
        for id in ["text", "missing", "div"] {
            assert_ne!(all_checked[index(id)].color, 0x123456, "{id}");
        }
        let unchecked = |_| Some(false);
        let all_unchecked = compute_with_state(&doc, &HashMap::new(), Some(&unchecked));
        for id in ["checkbox", "radio", "selected", "unselected"] {
            assert_ne!(all_unchecked[index(id)].color, 0x123456, "{id}");
        }
    }

    #[test]
    fn attribute_selectors_respect_html_case_rules_quotes_and_flags() {
        let html = "<input id=target type=CHECKBOX checked data-token=Camel data-empty='' data-text='A, B > C + D ~ E' data-unicode='Ä' lang=EN role=BUTTON>";
        for (selector, expected) in [
            ("[DATA-TOKEN]", true),
            ("[missing]", false),
            ("[data-token=Camel]", true),
            ("[data-token=camel]", false),
            ("[data-token=camel I]", true),
            ("[data-token=Camel s]", true),
            ("[data-token=camel s]", false),
            ("[data-token=\"Camel\"i]", true),
            ("[type=checkbox]", true),
            ("[type=checkbox s]", false),
            ("[type=CHECKBOX S]", true),
            ("[data-empty='']", true),
            ("[data-empty]", true),
            ("[data-text='A, B > C + D ~ E']", true),
            (".missing,[data-text='A, B > C + D ~ E'],.other", true),
            ("[data-unicode='ä' i]", false),
            ("[data-unicode='Ä' i]", true),
            ("[lang=en]", true),
            ("[role=button]", false),
            ("input:checked[TYPE=checkbox]", true),
            ("[type~=checkbox]",true),
            ("input, [type~=checkbox]",true),
        ] {
            let doc = parse(&format!("<style>{selector}{{color:#123456}}</style>{html}"));
            let styles = compute(&doc, &HashMap::new());
            let target = doc.nodes.iter().position(|node| matches!(&node.kind, NodeKind::Element { id: Some(id), .. } if id == "target")).unwrap();
            assert_eq!(styles[target].color == 0x123456, expected, "{selector}");
        }
    }

    #[test]
    fn attribute_and_checked_specificity_participate_in_the_cascade() {
        let doc = parse(
            "<style>
            .note{color:red}
            [data-kind=info]{color:blue}
            #winner{color:purple}
            .note:checked{color:green}
            [type=checkbox][checked]{color:#123456}
            .box input:checked{font-weight:bold}
            input[type=checkbox]{font-weight:normal}
            </style>
            <p id=equal class=note data-kind=info>Attribute ties class</p>
            <div class=box><input id=compound class=note type=checkbox checked>
              <input id=winner class=note type=checkbox checked></div>",
        );
        let styles = compute(&doc, &HashMap::new());
        let by_id = |id: &str| {
            &styles[doc.nodes.iter().position(|node| matches!(&node.kind, NodeKind::Element { id: Some(actual), .. } if actual == id)).unwrap()]
        };
        assert_eq!(by_id("equal").color, 0x0000ff);
        assert_eq!(by_id("compound").color, 0x123456);
        assert!(by_id("compound").bold);
        assert_eq!(by_id("winner").color, 0x800080);
    }

    #[test]
    fn unsupported_and_malformed_selectors_do_not_broaden_a_rule() {
        for selector in [
            "input:unknown",
            "input:checked()",
            "input::checked",
            "input:not(:checked,)",
            "input:checked +",
            "input + ~ div",
            "[type~=]",
            "[type~ =checkbox]",
            "[type~==checkbox]",
            "[type| =checkbox]",
            "[type^==check]",
            "[type$=]",
            "[type**=check]",
            "[ns|type]",
            "[*|type]",
            "[type=]",
            "[type=1]",
            "[type=-1]",
            "[type=-]",
            "[type=checkbox q]",
            "[type=checkbox i s]",
            "[type i]",
            "[type='check\\box']",
            "[type='check\nbox']",
            "[type='checkbox'",
            "[type[checked]]",
            "input,",
            ",input",
            "input,[broken",
            "input, [type~=]",
            "\u{2003}input",
        ] {
            let mut rules = Vec::new();
            parse_sheet(&format!("{selector}{{color:red}}"), &mut rules);
            assert!(
                rules.is_empty(),
                "unsupported selector became a rule: {selector}"
            );
        }
        assert!(parse_selector(&["div"; 12].join(" ~ ")).is_some());
        assert!(parse_selector(&["div"; 13].join(" ~ ")).is_none());
        assert!(
            parse_selector(&format!("[data-x='{}']", "a".repeat(MAX_SELECTOR_BYTES))).is_none()
        );
        assert!(parse_selector_list(&["input"; 16].join(","), false).is_some());
        assert!(parse_selector_list(&["input"; 17].join(","), false).is_none());
    }

    #[test]
    fn external_sheets_keep_document_order_and_at_rules_do_not_leak() {
        let doc = parse("<style>p{color:red}@media print{p{display:none}}</style><link rel=stylesheet href=x><style>p{font-weight:bold}</style><p id=a>A</p>");
        let link = doc
            .nodes
            .iter()
            .position(|node| matches!(&node.kind, NodeKind::Element { tag, .. } if tag == "link"))
            .unwrap();
        let styles = compute(&doc, &HashMap::from([(link, "p{color:blue}".to_owned())]));
        let node = doc
            .nodes
            .iter()
            .position(
                |node| matches!(&node.kind, NodeKind::Element { id: Some(id), .. } if id == "a"),
            )
            .unwrap();
        assert_eq!(styles[node].color, 0x0000ff);
        assert_eq!(styles[node].display, Display::Block);
        assert!(styles[node].bold);
    }

    #[test]
    fn inert_subtrees_do_not_supply_styles_but_display_none_does_not_disable_a_sheet() {
        let doc = parse("<style>p{color:blue}</style><template><style>p{color:red}</style><link rel=stylesheet href=x></template><iframe><style>p{display:none}</style></iframe><object><style>p{font-size:99px}</style></object><div style='display:none'><style>p{font-weight:bold}</style></div><p id=live>Visible</p>");
        let link = doc
            .nodes
            .iter()
            .position(|node| matches!(&node.kind, NodeKind::Element { tag, .. } if tag == "link"))
            .unwrap();
        let styles = compute(
            &doc,
            &HashMap::from([(link, "p{color:green;display:none}".to_owned())]),
        );
        let live = doc
            .nodes
            .iter()
            .position(
                |node| matches!(&node.kind, NodeKind::Element { id: Some(id), .. } if id == "live"),
            )
            .unwrap();
        assert_eq!(styles[live].color, 0x0000ff);
        assert_eq!(styles[live].display, Display::Block);
        assert_eq!(styles[live].font_size, 16.0);
        assert!(styles[live].bold);
    }

    #[test]
    fn reparented_older_nodes_inherit_from_newer_parents_in_physical_tree_order() {
        let mut doc = parse("<p id=older>child<b id=inner>nested</b></p><div id=newer style='color:#123456;font-size:23px'>parent</div>");
        let older = (0..doc.nodes.len())
            .find(|&node| doc.attr(node, "id") == Some("older"))
            .unwrap();
        let inner = (0..doc.nodes.len())
            .find(|&node| doc.attr(node, "id") == Some("inner"))
            .unwrap();
        let newer = (0..doc.nodes.len())
            .find(|&node| doc.attr(node, "id") == Some("newer"))
            .unwrap();
        assert!(older < newer);
        doc.nodes[0].children.retain(|&node| node != older);
        doc.nodes[newer].children.push(older);
        let styles = compute(&doc, &HashMap::new());
        assert_eq!(styles.len(), doc.nodes.len());
        assert_eq!(styles[older].color, 0x123456);
        assert_eq!(styles[inner].color, 0x123456);
        assert_eq!(styles[older].font_size, 23.0);
        assert_eq!(styles[inner].font_size, 23.0);
        assert!(styles[inner].bold);

        doc.nodes[0].children.retain(|&node| node != newer);
        let detached = compute(&doc, &HashMap::new());
        assert_eq!(detached[newer].color, Style::default().color);
        assert_eq!(detached[older].font_size, Style::default().font_size);
    }

    #[test]
    fn reparented_older_stylesheets_remain_inert_under_newer_containers() {
        let mut doc = parse("<style id=older>p{color:red}</style><style>p{color:blue}</style><p id=live>visible</p><template id=newer></template>");
        let older = (0..doc.nodes.len())
            .find(|&node| doc.attr(node, "id") == Some("older"))
            .unwrap();
        let newer = (0..doc.nodes.len())
            .find(|&node| doc.attr(node, "id") == Some("newer"))
            .unwrap();
        let live = (0..doc.nodes.len())
            .find(|&node| doc.attr(node, "id") == Some("live"))
            .unwrap();
        doc.nodes[0].children.retain(|&node| node != older);
        doc.nodes[newer].children.push(older);
        let styles = compute(&doc, &HashMap::new());
        assert_eq!(styles[live].color, 0x0000ff);
        assert_eq!(styles[older].display, Display::None);

        // A detached sheet must not affect the connected document either.
        doc.nodes[0].children.retain(|&node| node != newer);
        let styles = compute(&doc, &HashMap::new());
        assert_eq!(styles[live].color, 0x0000ff);
    }

    #[test]
    fn stylesheet_media_only_enables_empty_all_or_screen() {
        let doc = parse("<style media=print>p{display:none}</style><style media='(max-width:600px)'>p{font-size:99px}</style><style media=' SCREEN '>p{color:blue}</style><style media=all>p{font-weight:bold}</style><p id=live>Visible</p>");
        let styles = compute(&doc, &HashMap::new());
        let live = doc
            .nodes
            .iter()
            .position(
                |node| matches!(&node.kind, NodeKind::Element { id: Some(id), .. } if id == "live"),
            )
            .unwrap();
        assert_eq!(styles[live].color, 0x0000ff);
        assert_eq!(styles[live].display, Display::Block);
        assert_eq!(styles[live].font_size, 16.0);
        assert!(styles[live].bold);
    }
}

#[cfg(test)]
#[path="css_media_tests.rs"]
mod media_tests;

#[path="css_selector_state.rs"]
mod selector_state;

#[cfg(test)]
#[path="css_selector_negation_tests.rs"]
mod selector_negation_tests;

#[cfg(test)]
#[path="css_selector_negation_protocol_tests.rs"]
mod selector_negation_protocol_tests;

#[cfg(test)]
#[path="css_responsive_tests.rs"]
mod responsive_tests;

#[cfg(test)]
#[path="css_rgba_border_tests.rs"]
mod rgba_border_tests;

#[cfg(test)]
#[path="css_background_variable_tests.rs"]
mod background_variable_tests;

#[cfg(test)]
#[path="css_color_variable_tests.rs"]
mod color_variable_tests;

#[cfg(test)]
#[path="css_font_size_variable_tests.rs"]
mod font_size_variable_tests;

#[cfg(test)]
#[path="css_computed_style_tests.rs"]
mod computed_style_tests;

#[cfg(test)]
#[path="css_overflow_tests.rs"]
mod overflow_tests;

#[cfg(test)]
#[path="css_position_tests.rs"]
mod position_tests;

#[cfg(test)]
#[path="css_translate_tests.rs"]
mod translate_tests;

#[cfg(test)]
#[path="css_attribute_substring_tests.rs"]
mod attribute_substring_tests;

#[cfg(test)]
#[path="css_layout_variable_tests.rs"]
mod layout_variable_tests;

#[cfg(test)]
#[path="css_sizing_math_tests.rs"]
mod sizing_math_tests;

#[cfg(test)]
#[path="css_viewport_sizing_tests.rs"]
mod viewport_sizing_tests;

#[cfg(test)]
#[path="css_structural_tests.rs"]
mod structural_tests;

#[path="css_structural.rs"]
mod structural;

#[path="css_interaction_style.rs"]
mod interaction_style;

#[path="css_z_index.rs"]
pub(crate) mod z_index;

#[cfg(test)]
#[path="css_svg_geometry_tests.rs"]
mod svg_geometry_tests;

#[cfg(test)]
#[path="css_svg_paint_tests.rs"]
mod svg_paint_tests;
