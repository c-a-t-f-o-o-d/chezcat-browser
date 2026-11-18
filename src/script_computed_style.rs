//! Live, read-only computed custom properties, overflow axes, position and transform.
//! CSSOM getComputedStyle is NewObject: handles retain only the genuine element
//! and owner, never inline text, a stale cascade, or a second style resolver.
use super::*;

#[path = "script_computed_transform.rs"]
mod transform;

const BASE: u32 = 0x8400_0000;
const END: u32 = 0x8500_0000;
pub(super) fn index(handle: u32) -> Option<usize> {
    (BASE..END)
        .contains(&handle)
        .then(|| (handle - BASE) as usize)
}

#[cfg(test)]
mod canvas_tests {
    use super::*;

    #[test]
    fn canvas_current_color_samples_the_live_cascade_only_when_setting_fill_style() {
        let mut doc = crate::html::parse(
            "<html><head><style>.harbor{color:#164063}.brass{color:#aa7722}</style></head>\
             <body><div class=harbor id=parent><canvas id=sheet></canvas></div></body></html>",
        );
        doc.scripting_enabled = true;
        let canvas = (0..doc.nodes.len()).find(|&node| doc.attr(node, "id") == Some("sheet")).unwrap();
        let mut forms = Forms::new(&doc);
        let mut page = Page::for_test();
        page.set_geometry(crate::script_geometry::Geometry::new(
            640, 480, 1.0, &HashMap::new(), HashMap::new(), &crate::fonts::Book::empty(),
        ));
        assert_eq!(page.start(&mut doc, &mut forms, &[]).errors, 0);
        for source in [
            "var sheet=document.getElementById('sheet'), container=document.getElementById('parent'), ctx=sheet.getContext('2d');\
             ctx.fillStyle='currentColor';if(ctx.fillStyle!=='#164063')throw 'initial inherited class';",
            "container.className='brass';if(ctx.fillStyle!=='#164063')throw 'stored color changed prematurely';\
             ctx.fillStyle='currentColor';if(ctx.fillStyle!=='#aa7722')throw 'live inherited class';",
            "sheet.setAttribute('style','color:#764291');if(ctx.fillStyle!=='#aa7722')throw 'stored class changed';\
             ctx.fillStyle='currentColor';if(ctx.fillStyle!=='#764291')throw 'own foreground';",
            "container.removeChild(sheet);ctx.fillStyle='currentColor';if(ctx.fillStyle!=='#000000')throw 'detached color';\
             container.appendChild(sheet);ctx.fillStyle='currentColor';if(ctx.fillStyle!=='#764291')throw 'reattached color';",
        ] {
            let result = page.console_eval(&mut doc, &mut forms, source);
            assert_eq!(result.errors, 0, "{source}\n{}", page.report());
        }

        page.begin_job();
        let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
        bridge.state.copy_work = 0;
        assert_eq!(bridge.canvas_current_color(canvas).unwrap(), Some(0xff76_4291), "cached scalar read");
        bridge.state.geometry.as_mut().unwrap().invalidate();
        assert!(bridge.canvas_current_color(canvas).unwrap_err().contains("limit"));
        assert_eq!(bridge.state.copy_work, 0);
        bridge.state.host_work = 0;
        assert!(bridge.canvas_current_color(canvas).unwrap_err().contains("limit"));
    }
}

#[cfg(test)]
mod overflow_tests {
    use super::*;

    fn fixture() -> (Document, Forms, Page, usize) {
        let mut doc=crate::html::parse("<!doctype html><head><link id=sheet rel=stylesheet href=theme.css></head><body><div id=target class=mixed>Original</div>");
        doc.scripting_enabled = true;
        let find = |name: &str| {
            (0..doc.nodes.len())
                .find(|&node| doc.attr(node, "id") == Some(name))
                .unwrap()
        };
        let (sheet, target) = (find("sheet"), find("target"));
        let mut sheets = HashMap::new();
        sheets.insert(
            sheet,
            ".mixed{overflow:hidden visible;--control:actual}.open{overflow:visible clip}".into(),
        );
        let mut forms = Forms::new(&doc);
        let mut page = Page::for_test();
        let location = crate::navigation::address(
            "https://overflow-members.test/page",
            &crate::navigation::Location::home(),
        )
        .unwrap();
        page.set_location(&location);
        page.set_resource_base(&location);
        page.set_geometry(crate::script_geometry::Geometry::new(
            640,
            480,
            1.0,
            &sheets,
            HashMap::new(),
            &crate::fonts::Book::empty(),
        ));
        assert_eq!(
            page.start(&mut doc, &mut forms, &[]).errors,
            0,
            "{}",
            page.report()
        );
        (doc, forms, page, target)
    }

    #[test]
    fn computed_overflow_protocol_membership_and_attribute_case_track_live_declaration() {
        let (mut doc, mut forms, mut page, _) = fixture();
        let out=page.console_eval(&mut doc,&mut forms,r#"
var target=document.getElementById('target'),saved=getComputedStyle(target);
if(saved.getPropertyValue('--control').trim()!=='actual')throw 'real sheet positive';
for(var property of ['overflow','overflowX','overflowY','getPropertyValue']){if(!(property in saved))throw 'admitted member absent';}
for(var property of ['Overflow','OverflowX','overflowx','OVERFLOW']){if(property in saved)throw 'ordinary attribute case was folded';}
if(saved.overflow!=='hidden auto'||saved.overflowX!=='hidden'||saved.overflowY!=='auto'||saved.getPropertyValue('OVERFLOW-X')!=='hidden'||saved.getPropertyValue('OvErFlOw-Y')!=='auto')throw 'CSS-name case and camel-case attributes disagree';
target.className='open';if(saved.overflow!=='visible clip'||saved.overflowY!=='clip')throw 'membership/reads detached from live cascade';
document.body.removeChild(target);if(!('overflowX' in saved)||saved.overflow!==''||saved.overflowY!=='')throw 'detached declaration lost interface or leaked values';
document.body.appendChild(target);if(saved.overflow!=='visible clip')throw 'reconnection ignored current cascade';
"#);
        assert_eq!(out.errors, 0, "{}", page.report());
        assert!(!page.has_navigation_request());
    }

    #[test]
    fn computed_overflow_protocol_unequal_axes_charge_exact_serialization_before_copy() {
        let (mut doc, mut forms, mut page, target) = fixture();
        let commits = page.state.dom_commits;
        let mut bridge = Bridge {
            doc: &mut doc,
            forms: &mut forms,
            state: &mut page.state,
        };
        let Value::Host(handle) = bridge
            .create_computed_style(&[Value::Host(node_handle(target))])
            .unwrap()
        else {
            panic!("genuine computed declaration");
        };
        assert!(
            matches!(bridge.get(handle,"overflow").unwrap(),Value::String(value) if value=="hidden auto")
        );
        for (property, value) in [
            ("overflow", "hidden auto"),
            ("overflowX", "hidden"),
            ("overflowY", "auto"),
        ] {
            bridge.state.copy_work = value.len() - 1;
            assert!(
                bridge.get(handle, property).is_err(),
                "underfunded {property}"
            );
            assert_eq!(bridge.state.copy_work, 0);
            bridge.state.copy_work = value.len();
            assert!(
                matches!(bridge.get(handle,property).unwrap(),Value::String(text) if text==value)
            );
            assert_eq!(bridge.state.copy_work, 0);
        }
        bridge.state.copy_work = "hidden auto".len();
        assert!(
            matches!(bridge.computed_property_value(handle,&[Value::String("OVERFLOW".into())]).unwrap(),Value::String(value) if value=="hidden auto")
        );
        assert_eq!(bridge.state.copy_work, 0);
        assert_eq!(bridge.state.dom_commits, commits);
    }
}

pub(super) struct Declaration {
    node: usize,
    owner: u32,
}

#[derive(Clone, Copy)]
enum ComputedProperty {
    Position,
    Transform,
    Overflow,
    OverflowX,
    OverflowY,
}
impl ComputedProperty {
    fn attribute(name: &str) -> Option<Self> {
        match name {
            "position" => Some(Self::Position),
            "transform" => Some(Self::Transform),
            "overflow" => Some(Self::Overflow),
            "overflowX" => Some(Self::OverflowX),
            "overflowY" => Some(Self::OverflowY),
            _ => None,
        }
    }
    fn css_name(name: &str) -> Option<Self> {
        if name.eq_ignore_ascii_case("position") {
            Some(Self::Position)
        } else if name.eq_ignore_ascii_case("transform") {
            Some(Self::Transform)
        } else if name.eq_ignore_ascii_case("overflow") {
            Some(Self::Overflow)
        } else if name.eq_ignore_ascii_case("overflow-x") {
            Some(Self::OverflowX)
        } else if name.eq_ignore_ascii_case("overflow-y") {
            Some(Self::OverflowY)
        } else {
            None
        }
    }
}

pub(super) fn attribute_member(name: &str) -> bool {
    ComputedProperty::attribute(name).is_some()
}

impl Bridge<'_> {
    pub(super) fn computed_style_owner(&self, handle: u32) -> Option<u32> {
        index(handle)
            .and_then(|index| self.state.computed_styles.get(index))
            .map(|style| style.owner)
    }
    pub(super) fn computed_style_receiver(&self, handle: u32) -> bool {
        self.computed_style_owner(handle).is_some()
    }

    pub(super) fn validate_computed_target(
        &self,
        value: Option<&Value>,
    ) -> Result<usize, (&'static str, String)> {
        let Some(Value::Host(handle)) = value else {
            return Err((
                "TypeError",
                "getComputedStyle requires a genuine Element.".into(),
            ));
        };
        if self.is_frame_handle(*handle) {
            if let Some(owner) = self.frame_instance_window(*handle) {
                self.frame_window_access(Some(WINDOW), owner)
                    .map_err(|message| ("SecurityError", message))?;
            }
            return if self.frame_element_sibling_receiver(*handle) {
                Err((
                    "NotSupportedError",
                    "Computed style for child Elements has no renderer binding.".into(),
                ))
            } else {
                Err((
                    "TypeError",
                    "getComputedStyle requires a genuine Element.".into(),
                ))
            };
        }
        if !self.attr_element_receiver(*handle) {
            return Err((
                "TypeError",
                "getComputedStyle requires a genuine accessible Element.".into(),
            ));
        }
        Ok(node_index(*handle).unwrap())
    }

    pub(super) fn validate_computed_call(
        &self,
        object: u32,
        method: &str,
        args: &[Value],
    ) -> Result<(), (&'static str, String)> {
        if object == WINDOW && method == "getComputedStyle" {
            self.validate_computed_target(args.first())?;
            if args.get(1).is_some_and(|value| match value {
                Value::Null | Value::Undefined => false,
                Value::String(text) => !text.is_empty(),
                _ => true,
            }) {
                return Err((
                    "NotSupportedError",
                    "Computed pseudo-element style is not implemented.".into(),
                ));
            }
        } else if index(object).is_some() {
            if !self.computed_style_receiver(object) {
                return Err((
                    "TypeError",
                    "Computed declaration receiver is unavailable.".into(),
                ));
            }
            if matches!(method, "setProperty" | "removeProperty") {
                return Err((
                    "NoModificationAllowedError",
                    "Computed style declarations are read-only.".into(),
                ));
            }
        }
        Ok(())
    }

    pub(super) fn create_computed_style(&mut self, args: &[Value]) -> Result<Value, String> {
        self.validate_computed_call(WINDOW, "getComputedStyle", args)
            .map_err(|(_, message)| message)?;
        let node = self
            .validate_computed_target(args.first())
            .map_err(|(_, message)| message)?;
        // Recheck physical connection after observable pseudo argument conversion.
        self.state.rebuild(self.doc)?;
        let length = self.state.computed_styles.len();
        let handle = u32::try_from(length)
            .ok()
            .and_then(|index| BASE.checked_add(index))
            .filter(|&handle| handle < END)
            .ok_or("Computed declaration identity space exhausted.")?;
        self.state
            .computed_styles
            .try_reserve(1)
            .map_err(|_| "Computed declaration storage reservation failed.")?;
        self.state.work(1)?;
        self.state
            .replace_bytes(0, std::mem::size_of::<Declaration>())?;
        self.state.computed_styles.push(Declaration {
            node,
            owner: WINDOW,
        });
        Ok(Value::Host(handle))
    }

    pub(super) fn computed_property_value(
        &mut self,
        object: u32,
        args: &[Value],
    ) -> Result<Value, String> {
        let node = self.computed_declaration_node(object)?;
        let Some(Value::String(name)) = args.first() else {
            return Err("getPropertyValue requires runtime DOMString conversion.".into());
        };
        if name.is_empty() {
            return Ok(Value::String(String::new()));
        }
        if let Some(property) = ComputedProperty::css_name(name) {
            self.state.work(name.len() + 1)?;
            return self.computed_keyword_value(node, property);
        }
        if !crate::css::computed_custom_property_name(name) {
            return Err(
                "Computed readback supports position, overflow, transform, and the admitted custom-property names.".into(),
            );
        }
        self.state.work(
            name.len()
                .checked_add(1)
                .ok_or("Computed name work overflow.")?,
        )?;
        // CSSOM's declaration list is empty for a disconnected Element. The
        // retained handle remains live, so a later reconnection reads anew.
        if !self.computed_node_connected(node)? {
            return Ok(Value::String(String::new()));
        }
        let geometry = self
            .state
            .geometry
            .as_mut()
            .ok_or("Computed style requires the actual document renderer.")?;
        geometry
            .computed_custom_property(
                self.doc,
                self.forms,
                node,
                name,
                &mut self.state.host_work,
                &mut self.state.copy_work,
            )
            .map(|value| Value::String(value.unwrap_or_default()))
    }

    fn computed_declaration_node(&self, object: u32) -> Result<usize, String> {
        let style = index(object)
            .and_then(|index| self.state.computed_styles.get(index))
            .ok_or("Invalid computed declaration receiver.")?;
        if style.owner != WINDOW {
            return Err("Computed declaration owner is unavailable.".into());
        }
        Ok(style.node)
    }

    fn computed_node_connected(&mut self, node: usize) -> Result<bool, String> {
        self.state.rebuild(self.doc)?;
        if !self.attr_element_receiver(node_handle(node)) {
            return Err("Computed declaration Element is inert or unavailable.".into());
        }
        Ok(self.state.dom.connected.get(node).copied().unwrap_or(false))
    }

    pub(super) fn canvas_current_color(&mut self, node: usize) -> Result<Option<u32>, String> {
        self.state.work(1)?;
        if !crate::html_canvas::element(self.doc, node) {
            return Err("Canvas foreground requires a genuine HTML canvas.".into());
        }
        if !self.computed_node_connected(node)? {
            // A detached HTML canvas resolves currentColor to opaque black.
            return Ok(Some(0xff00_0000));
        }
        let snapshot = self.state.geometry.as_mut()
            .ok_or("Canvas foreground requires the actual document renderer.")?
            .computed_snapshot(self.doc, self.forms, &mut self.state.host_work, &mut self.state.copy_work)?;
        let color = snapshot.canvas_foreground(node)
            .map_err(|_| "Canvas foreground unavailable within the CSS cascade limits.")?;
        Ok(color.map(|rgb| 0xff00_0000 | rgb))
    }

    pub(super) fn computed_style_get(&mut self, object: u32, name: &str) -> Result<Value, String> {
        let node = self.computed_declaration_node(object)?;
        let property = ComputedProperty::attribute(name)
            .ok_or_else(|| format!("Computed declaration property {name:?} is not implemented."))?;
        self.computed_keyword_value(node, property)
    }

    fn computed_keyword_value(
        &mut self,
        node: usize,
        property: ComputedProperty,
    ) -> Result<Value, String> {
        self.state.work(1)?;
        if !self.computed_node_connected(node)? {
            return Ok(Value::String(String::new()));
        }
        if matches!(property, ComputedProperty::Transform) {
            return self.computed_transform_value(node);
        }
        let snapshot = self
            .state
            .geometry
            .as_mut()
            .ok_or("Computed style requires the actual document renderer.")?
            .computed_snapshot(
                self.doc,
                self.forms,
                &mut self.state.host_work,
                &mut self.state.copy_work,
            )?;
        let (first, second) = match property {
            ComputedProperty::Transform => unreachable!("transform serialization uses actual resolved geometry"),
            ComputedProperty::Position => {
                let position = snapshot.read_position(node)
                    .map_err(|_| "Computed position unavailable within the CSS cascade limits.")?;
                (position.map_or("", |value| value.as_str()), None)
            }
            ComputedProperty::Overflow | ComputedProperty::OverflowX | ComputedProperty::OverflowY => {
                let Some(pair) = snapshot.read_overflow(node)
                    .map_err(|_| "Computed overflow unavailable within the CSS cascade limits.")?
                else { return Ok(Value::String(String::new())); };
                let (x, y) = (pair.x.as_css(), pair.y.as_css());
                let first = if matches!(property, ComputedProperty::OverflowY) { y } else { x };
                let second = (matches!(property, ComputedProperty::Overflow) && x != y).then_some(y);
                (first, second)
            }
        };
        // The cascade returns finite keywords. Charge the complete CSSOM
        // serialization before allocating the string.
        let bytes = first.len() + second.map_or(0, |value| 1 + value.len());
        self.state.copies(bytes)?;
        let mut value = String::new();
        value
            .try_reserve_exact(bytes)
            .map_err(|_| "Computed style string reservation failed.")?;
        value.push_str(first);
        if let Some(second) = second {
            value.push(' ');
            value.push_str(second);
        }
        Ok(Value::String(value))
    }
}
