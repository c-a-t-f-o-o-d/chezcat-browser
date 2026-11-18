//! Page-owned, transparent sRGB 2D contexts for genuine HTML canvases.
use super::*;
use crate::canvas_2d_bitmap::CanvasBitmap;

const BASE: u32 = 0x8500_0000;
const END: u32 = 0x8600_0000;
const MAX_CONTEXTS: usize = 256;
const PAGE_BITMAP_BYTES: usize = 64 * 1024 * 1024;
/// Drawing-state attributes must not become instance expandos that claim a
/// new setting while subsequent rectangles continue using the default state.
pub(super) fn unsupported_state(property: &str) -> bool {
    matches!(
        property,
        "fillStyle"
            | "strokeStyle"
            | "globalAlpha"
            | "globalCompositeOperation"
            | "filter"
            | "imageSmoothingEnabled"
            | "imageSmoothingQuality"
            | "lineWidth"
            | "lineCap"
            | "lineJoin"
            | "miterLimit"
            | "lineDashOffset"
            | "shadowOffsetX"
            | "shadowOffsetY"
            | "shadowBlur"
            | "shadowColor"
            | "font"
            | "textAlign"
            | "textBaseline"
            | "direction"
            | "lang"
            | "letterSpacing"
            | "fontKerning"
            | "fontStretch"
            | "fontVariantCaps"
            | "textRendering"
            | "wordSpacing"
    )
}

const SETTINGS: &[&str] = &[
    "alpha",
    "colorSpace",
    "colorType",
    "desynchronized",
    "toneMapping",
    "willReadFrequently",
];

pub(super) struct Contexts {
    surfaces: HashMap<usize, CanvasBitmap>,
    remaining_bytes: usize,
}

impl Default for Contexts {
    fn default() -> Self {
        Self {
            surfaces: HashMap::new(),
            remaining_bytes: PAGE_BITMAP_BYTES,
        }
    }
}

impl Contexts {
    pub(super) fn commit_resize(&mut self, node: usize, dimensions: Option<(u32, u32)>) {
        if let (Some(surface), Some((width, height))) = (self.surfaces.get_mut(&node), dimensions) {
            self.remaining_bytes += surface.reset(width, height);
            debug_assert!(self.remaining_bytes <= PAGE_BITMAP_BYTES);
        }
    }
}

impl Page {
    pub(crate) fn canvas_bitmap(&self, node: usize) -> Option<&crate::image::Bitmap> {
        self.state.canvas_contexts.surfaces.get(&node)?.bitmap()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Operation {
    GetContext,
    Canvas,
    FillRect,
    ClearRect,
}

impl Operation {
    pub(super) fn parse(method: &str) -> Option<Self> {
        match method {
            "\0HTMLCanvasElement.getContext" => Some(Self::GetContext),
            "\0CanvasRenderingContext2D.get.canvas" => Some(Self::Canvas),
            "\0CanvasRenderingContext2D.fillRect" => Some(Self::FillRect),
            "\0CanvasRenderingContext2D.clearRect" => Some(Self::ClearRect),
            _ => None,
        }
    }

    pub(super) fn arity(self) -> usize {
        match self {
            Self::GetContext => 1,
            Self::Canvas => 0,
            Self::FillRect | Self::ClearRect => 4,
        }
    }

    pub(super) fn conversions(self) -> &'static [(usize, bool)] {
        match self {
            Self::GetContext => &[(0, true)],
            Self::Canvas => &[],
            Self::FillRect | Self::ClearRect => &[(0, false), (1, false), (2, false), (3, false)],
        }
    }

    pub(super) fn accepts(self, bridge: &Bridge<'_>, object: u32) -> bool {
        match self {
            Self::GetContext => bridge.canvas_receiver(object),
            _ => bridge.canvas_context_receiver(object),
        }
    }
}

fn dimension(raw: Option<&str>, name: &str) -> u32 {
    // Geometry can retain larger natural dimensions; every value above u32::MAX
    // already exceeds the raster limit. It must never become the IDL fallback.
    crate::html_canvas::natural_dimension(raw, name).bounded_value(f64::from(u32::MAX)) as u32
}

pub(super) fn option_validation(name: &str, value: &Value) -> Result<(), (&'static str, String)> {
    match (name, value) {
        (_, Value::Undefined) => Ok(()),
        ("colorSpace", Value::String(s))
            if !matches!(
                s.as_str(),
                "srgb" | "srgb-linear" | "display-p3" | "display-p3-linear"
            ) =>
        {
            Err(("TypeError", "Unknown PredefinedColorSpace.".into()))
        }
        ("colorType", Value::String(s)) if !matches!(s.as_str(), "unorm8" | "float16") => {
            Err(("TypeError", "Unknown CanvasColorType.".into()))
        }
        ("toneMapping", Value::Null) => Ok(()),
        // Nested tone-mapping dictionary conversion is outside this renderer.
        ("toneMapping", Value::Host(_) | Value::Object(_) | Value::Function(_)) => Err((
            "NotSupportedError",
            "Explicit canvas tone-mapping settings are not supported.".into(),
        )),
        ("toneMapping", _) => Err(("TypeError", "toneMapping requires a dictionary.".into())),
        _ => Ok(()),
    }
}

fn default_settings(fields: &[(&str, Value)]) -> bool {
    fields.iter().all(|(name, value)| {
        if matches!(value, Value::Undefined) {
            return true;
        }
        match *name {
            "alpha" => value.truthy(),
            "colorSpace" => matches!(value, Value::String(s) if s == "srgb"),
            "colorType" => matches!(value, Value::String(s) if s == "unorm8"),
            "desynchronized" | "willReadFrequently" => !value.truthy(),
            "toneMapping" => matches!(value, Value::Null),
            _ => false,
        }
    })
}

impl Bridge<'_> {
    fn canvas_context_node(&self, object: u32) -> Option<usize> {
        if !(BASE..END).contains(&object) {
            return None;
        }
        let node = (object - BASE) as usize;
        (self.state.canvas_contexts.surfaces.contains_key(&node)
            && self.canvas_receiver(node_handle(node)))
        .then_some(node)
    }

    pub(super) fn canvas_context_receiver(&self, object: u32) -> bool {
        self.canvas_context_node(object).is_some()
    }

    pub(super) fn canvas_context_dictionary(
        &self,
        object: u32,
        args: &[Value],
    ) -> Option<(usize, &'static [&'static str])> {
        let node = node_index(object)?;
        (self.canvas_receiver(object)
            && matches!(args.first(), Some(Value::String(id)) if id == "2d")
            && !self.state.canvas_contexts.surfaces.contains_key(&node))
        .then_some((1, SETTINGS))
    }

    /// Preflight while the new raw attribute is still available and failure is
    /// atomic. The caller commits only after an actual addition/change/removal.
    pub(super) fn prepare_canvas_resize(
        &mut self,
        node: usize,
        name: &crate::html::AttributeName,
        raw: Option<&str>,
    ) -> Result<Option<(u32, u32)>, String> {
        let Some(surface) = self.state.canvas_contexts.surfaces.get(&node) else {
            return Ok(None);
        };
        let (width, height) = surface.dimensions();
        let axis = if name.html_is("width") {
            "width"
        } else if name.html_is("height") {
            "height"
        } else {
            return Ok(None);
        };
        self.state
            .work(raw.map_or(1, |text| text.len().saturating_add(1)))?;
        let next = dimension(raw, axis);
        Ok(Some(if axis == "width" {
            (next, height)
        } else {
            (width, next)
        }))
    }

    fn canvas_context_get(
        &mut self,
        object: u32,
        args: &[Value],
        fields: Option<&[(&str, Value)]>,
    ) -> Result<Value, String> {
        let node = self.node(object)?;
        let Some(Value::String(id)) = args.first() else {
            return Err("getContext requires a converted context identifier.".into());
        };
        if id != "2d" {
            return Ok(Value::Null);
        }
        if fields.is_some_and(|settings| !default_settings(settings)) {
            return Ok(Value::Null);
        }
        let handle = BASE
            .checked_add(u32::try_from(node).map_err(|_| "Canvas node identity overflow.")?)
            .filter(|handle| *handle < END)
            .ok_or("Canvas context identity overflow.")?;
        if self.state.canvas_contexts.surfaces.contains_key(&node) {
            return Ok(Value::Host(handle));
        }
        if fields.is_none()
            && matches!(
                args.get(1),
                Some(Value::Object(_) | Value::Function(_) | Value::Host(_))
            )
        {
            return Err("Canvas settings require ordered native dictionary conversion.".into());
        }
        if self.state.canvas_contexts.surfaces.len() >= MAX_CONTEXTS {
            return Err("Canvas context count limit reached.".into());
        }
        let NodeKind::Element { attrs, .. } = &self.doc.nodes[node].kind else {
            unreachable!()
        };
        self.state.work(attrs.len().saturating_mul(2))?;
        let width = self.doc.attr(node, "width");
        let height = self.doc.attr(node, "height");
        self.state.work(
            width
                .map_or(0, str::len)
                .saturating_add(height.map_or(0, str::len)),
        )?;
        let surface = CanvasBitmap::new(dimension(width, "width"), dimension(height, "height"));
        self.state
            .canvas_contexts
            .surfaces
            .try_reserve(1)
            .map_err(|_| "Could not reserve a canvas context.")?;
        self.state.canvas_contexts.surfaces.insert(node, surface);
        Ok(Value::Host(handle))
    }

    pub(super) fn canvas_context_call(
        &mut self,
        object: u32,
        operation: Operation,
        args: &[Value],
        fields: Option<&[(&str, Value)]>,
    ) -> Result<Value, String> {
        self.state.work(1)?;
        if operation == Operation::GetContext {
            if !self.canvas_receiver(object) {
                return Err("Invalid HTML canvas receiver.".into());
            }
            return self.canvas_context_get(object, args, fields);
        }
        let node = self
            .canvas_context_node(object)
            .ok_or("Unavailable canvas context.")?;
        if operation == Operation::Canvas {
            return Ok(Value::Host(node_handle(node)));
        }
        let [Value::Number(x), Value::Number(y), Value::Number(w), Value::Number(h), ..] = args
        else {
            return Err("Canvas rectangle operations require four converted numbers.".into());
        };
        let rect = [*x, *y, *w, *h];
        let state = &mut *self.state;
        let contexts = &mut state.canvas_contexts;
        let surface = contexts
            .surfaces
            .get_mut(&node)
            .ok_or("Unavailable canvas context.")?;
        if operation == Operation::FillRect {
            surface.fill_rect(
                rect,
                0xff00_0000,
                &mut contexts.remaining_bytes,
                &mut state.host_work,
            )?;
        } else {
            surface.clear_rect(rect, &mut contexts.remaining_bytes, &mut state.host_work)?;
        }
        Ok(Value::Undefined)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (Document, Forms, Page, usize) {
        let mut doc = crate::html::parse(
            "<html><body><canvas id=first width=4 height=3></canvas>\
             <canvas id=next width=3 height=1></canvas></body></html>",
        );
        doc.scripting_enabled = true;
        let node = (0..doc.nodes.len())
            .find(|&n| doc.attr(n, "id") == Some("first"))
            .unwrap();
        let mut forms = Forms::new(&doc);
        let mut page = Page::for_test();
        assert_eq!(page.start(&mut doc, &mut forms, &[]).errors, 0);
        (doc, forms, page, node)
    }

    fn eval(doc: &mut Document, forms: &mut Forms, page: &mut Page, source: &str) {
        let outcome = page.console_eval(doc, forms, source);
        assert_eq!(outcome.errors, 0, "{source}\n{}", page.report());
    }

    #[test]
    fn canvas_2d_protocol_options_brand_and_abrupt_reentrant_acquisition() {
        let (mut doc, mut forms, mut page, first) = fixture();
        let address =
            |url| crate::navigation::address(url, &crate::navigation::Location::home()).unwrap();
        let origin = address("https://canvas-context.test/own");
        page.set_location(&origin);
        page.set_resource_base(&origin);
        eval(
            &mut doc,
            &mut forms,
            &mut page,
            r#"
            var a=document.getElementById('first'), b=document.getElementById('next');
            var method=HTMLCanvasElement.prototype.getContext, token={}, reads=0, inner;
            var traps={get alpha(){reads++;throw token;}};
            if(method.call(a,'missing-kind',traps)!==null||reads!==0)throw 'unknown mode options';
            var key={toString(){reads++;return '2d';}};
            try{method.call({},key);throw 'invalid receiver';}catch(e){if(!(e instanceof TypeError))throw e;}
            if(reads!==0)throw 'coercion before canvas brand';
            if(a.getContext('2d',{alpha:false})!==null)throw 'unsupported opaque bitmap was accepted';
            try{a.getContext('2d',{get alpha(){inner=a.getContext('2d',{});throw token;}});throw 'getter completed';}
            catch(e){if(e!==token)throw e;}
            if(inner.canvas!==a||a.getContext('2d',traps)!==inner||reads!==0)throw 'lost nested context';
            var order='',opts={get alpha(){order+='A';return true;},
                get colorSpace(){order+='B';return 'srgb';},get colorType(){order+='C';return 'unorm8';},
                get desynchronized(){order+='D';return false;},get toneMapping(){order+='E';},
                get willReadFrequently(){order+='F';return false;}};
            var other=b.getContext('2d',opts);
            if(order!=='ABCDEF'||!other||other===inner||other.canvas!==b)throw 'ordered default options';
            var bad={valueOf(){reads++;throw token;}},fill=inner.fillRect;
            try{fill.call(b,bad,0,1,1);throw 'canvas used as context';}catch(e){if(!(e instanceof TypeError))throw e;}
            if(reads!==0)throw 'coercion before context brand';
            try{fill.call(inner,0,bad,1,1);throw 'coordinate getter completed';}catch(e){if(e!==token)throw e;}
            if(reads!==1)throw 'coordinate conversion count';
            var proto=CanvasRenderingContext2D.prototype, getter=Object.getOwnPropertyDescriptor(proto,'canvas');
            if(Object.getPrototypeOf(inner)!==proto||Object.getPrototypeOf(proto)!==Object.prototype||
               !(inner instanceof CanvasRenderingContext2D)||inner instanceof Node||
               Object.prototype.toString.call(inner)!=='[object CanvasRenderingContext2D]'||
               !getter.enumerable||!getter.configurable||getter.set!==undefined||
               getter.get.call(other)!==b||Object.hasOwn(inner,'canvas'))throw 'context prototype and descriptor';
            var illegal=false;try{new CanvasRenderingContext2D();}catch(e){illegal=e instanceof TypeError;}
            if(!illegal)throw 'constructor allocated a context';
            inner.caption='sheet';inner.canvas=b;
            if(Object.keys(inner).join(',')!=='caption'||inner.canvas!==a)throw 'author property and readonly backlink';
            var denied=false;try{inner.fillStyle='red';}catch(e){denied=true;}
            if(!denied||Object.hasOwn(inner,'fillStyle'))throw 'unsupported drawing state was stored';
            var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);
            var child=frame.contentWindow, childProto=child.CanvasRenderingContext2D.prototype;
            var borrowed=childProto.fillRect;
            if(childProto===proto||childProto.constructor===CanvasRenderingContext2D||
               Object.getPrototypeOf(borrowed)!==child.Function.prototype||
               Object.getOwnPropertyDescriptor(childProto,'canvas').get.call(inner)!==a)throw 'context method realm';
        "#,
        );
        page.set_location(&address("https://changed-canvas.test/other"));
        let denied = page.console_eval(&mut doc, &mut forms, "borrowed.call(inner,bad,0,1,1);");
        assert_eq!(denied.errors, 1, "{}", page.report());
        assert!(page.report().contains("same-origin"), "{}", page.report());
        page.set_location(&origin);
        eval(
            &mut doc,
            &mut forms,
            &mut page,
            "if(reads!==1)throw 'origin refusal ran conversion';borrowed.call(inner,0,0,0,0);",
        );
        assert_eq!(page.state.canvas_contexts.surfaces.len(), 2);
        assert!(page.canvas_bitmap(first).is_none());
    }

    #[test]
    fn canvas_2d_protocol_dimension_commit_refunds_pixels_and_preserves_attr_identity() {
        let (mut doc, mut forms, mut page, first) = fixture();
        // Exactly one 4x3 backing fits. Reservation and refunds use the
        // ordinary page store; a smaller document cap keeps the test compact.
        page.state.canvas_contexts.remaining_bytes = 48;
        eval(
            &mut doc,
            &mut forms,
            &mut page,
            r#"
            var a=document.getElementById('first'), b=document.getElementById('next');
            var ctx=a.getContext('2d'), other=b.getContext('2d'), width=a.getAttributeNode('width');
            ctx.fillRect(0,0,1,1);
            a.setAttributeNS('urn:canvas-test','m:width','4');
        "#,
        );
        assert_eq!(page.canvas_bitmap(first).unwrap().pixels[0], 0xff00_0000);
        assert_eq!(page.state.canvas_contexts.remaining_bytes, 0);
        let outcome = page.console_eval(&mut doc, &mut forms, "other.fillRect(0,0,1,1)");
        assert_eq!(outcome.errors, 1, "{}", page.report());
        assert_eq!(page.state.canvas_contexts.remaining_bytes, 0);
        assert_eq!(page.canvas_bitmap(first).unwrap().pixels[0], 0xff00_0000);
        eval(
            &mut doc,
            &mut forms,
            &mut page,
            r#"
            width.value='004';
            if(a.width!==4||a.getAttributeNode('width')!==width||a.getContext('2d')!==ctx)throw 'real resize state';
        "#,
        );
        assert!(page.canvas_bitmap(first).is_none());
        assert_eq!(page.state.canvas_contexts.remaining_bytes, 48);
        eval(
            &mut doc,
            &mut forms,
            &mut page,
            "other.fillRect(0,0,1,1);a.width=2147483647;",
        );
        assert!(page.canvas_bitmap(first).is_none());
        assert_eq!(page.state.canvas_contexts.remaining_bytes, 36);
        let outcome = page.console_eval(&mut doc, &mut forms, "ctx.fillRect(0,0,1,1)");
        assert_eq!(outcome.errors, 1, "{}", page.report());
        assert!(page.canvas_bitmap(first).is_none());
        assert_eq!(page.state.canvas_contexts.remaining_bytes, 36);
        eval(
            &mut doc,
            &mut forms,
            &mut page,
            r#"
            a.width=2;ctx.fillRect(0,0,1,1);
            if(a.getContext('2d')!==ctx||width.ownerElement!==a)throw 'saved context recovery';
        "#,
        );
        assert_eq!(page.canvas_bitmap(first).unwrap().pixels[0], 0xff00_0000);
        assert_eq!(page.state.canvas_contexts.remaining_bytes, 12);
    }
}
