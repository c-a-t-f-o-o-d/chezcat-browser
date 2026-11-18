//! Readback of a genuine element's own resolved translation, not ancestor motion.
//! https://www.w3.org/TR/css-transforms-2/#serialization-of-the-computed-value
use super::*;
use std::fmt::Write as _;

// CSS lengths use f32 precision. Its shortest decimal form (including the
// smallest nonzero value in fixed-point notation) fits this stack buffer.
// No heap string is produced before charging the final CSSOM serialization.
const NUMBER_BYTES: usize = 64;
struct NumberText {
    bytes: [u8; NUMBER_BYTES],
    length: usize,
}
impl NumberText {
    fn new() -> Self {
        Self {
            bytes: [0; NUMBER_BYTES],
            length: 0,
        }
    }
    fn text(&self) -> &str {
        std::str::from_utf8(&self.bytes[..self.length]).expect("formatted numbers are ASCII")
    }
}
impl std::fmt::Write for NumberText {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        let end = self.length.checked_add(text.len()).ok_or(std::fmt::Error)?;
        let destination = self
            .bytes
            .get_mut(self.length..end)
            .ok_or(std::fmt::Error)?;
        destination.copy_from_slice(text.as_bytes());
        self.length = end;
        Ok(())
    }
}

impl Bridge<'_> {
    #[inline(never)]
    pub(super) fn computed_transform_value(&mut self, node: usize) -> Result<Value, String> {
        let translation = self
            .state
            .geometry
            .as_mut()
            .ok_or("Computed style requires the actual document renderer.")?
            .resolved_transform(
                self.doc,
                self.forms,
                node,
                &mut self.state.host_work,
                &mut self.state.copy_work,
            )?;
        let Some([x, y]) = translation else {
            self.state.copies(4)?;
            return Ok(Value::String("none".into()));
        };
        if !x.is_finite() || !y.is_finite() {
            return Err(
                "Computed transform has unavailable finite translation coordinates.".into(),
            );
        }
        self.state.work(NUMBER_BYTES * 2)?;
        let (mut first, mut second) = (NumberText::new(), NumberText::new());
        write!(&mut first, "{}", if x == 0.0 { 0.0 } else { x as f32 })
            .map_err(|_| "Computed transform number exceeds its finite serialization bound.")?;
        write!(&mut second, "{}", if y == 0.0 { 0.0 } else { y as f32 })
            .map_err(|_| "Computed transform number exceeds its finite serialization bound.")?;
        // A translate3d(x,y,0) matrix is two-dimensional too. Identity remains
        // a matrix, since a non-none transform has containing-block effects.
        const PREFIX: &str = "matrix(1, 0, 0, 1, ";
        let bytes = PREFIX.len() + first.length + 2 + second.length + 1;
        self.state.copies(bytes)?;
        let mut result = String::new();
        result
            .try_reserve_exact(bytes)
            .map_err(|_| "Computed transform string reservation failed.")?;
        result.push_str(PREFIX);
        result.push_str(first.text());
        result.push_str(", ");
        result.push_str(second.text());
        result.push(')');
        Ok(Value::String(result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(doc: &Document, id: &str) -> usize {
        (0..doc.nodes.len())
            .find(|&node| doc.attr(node, "id") == Some(id))
            .unwrap()
    }
    fn fixture() -> (Document, Forms, Page, usize) {
        let mut doc = crate::html::parse(concat!(
            "<!doctype html><html><head><link id=sheet rel=stylesheet href=theme.css></head><body>",
            "<div id=ancestor><div id=target class=translated>Original</div></div>",
            "<form id=form><input id=neighbor value=original></form></body></html>"
        ));
        doc.scripting_enabled = true;
        let target = node(&doc, "target");
        let sheets = HashMap::from([(node(&doc, "sheet"), concat!(
            "#ancestor{transform:translate(100px,200px)}#target{width:80px;height:40px;--control:sheet;overflow:clip;position:relative}",
            ".translated{transform:translate3d(12.5px,-7px,0)}.later{transform:translateX(-9px)}",
            ".percent{border:2px solid black;transform:translate(25%,-50%)}",
            ".identity{transform:translate(0)}.plain{transform:none}.inherit{transform:inherit}",
            ".unsupported{transform:rotate(10deg)}"
        ).into())]);
        let mut forms = Forms::new(&doc);
        let mut page = Page::for_test();
        let location = crate::navigation::address(
            "https://computed-transform.test/page",
            &crate::navigation::Location::home(),
        )
        .unwrap();
        page.set_location(&location);
        page.set_resource_base(&location);
        page.set_stylesheet_usage(crate::css::stylesheet_usage(&doc, &sheets));
        page.set_geometry(crate::script_geometry::Geometry::new(
            800,
            600,
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
        let neighbor = node(&doc, "neighbor");
        forms.script_set_value(neighbor, "edited neighbor").unwrap();
        forms.controls.get_mut(&neighbor).unwrap().value.cursor = 4;
        forms.controls.get_mut(&neighbor).unwrap().value.anchor = 1;
        (doc, forms, page, target)
    }

    fn kept(doc: &Document, forms: &Forms, page: &mut Page) {
        let neighbor = node(doc, "neighbor");
        let control = &forms.controls[&neighbor];
        assert_eq!(
            (
                control.value.text(),
                control.value.cursor,
                control.value.anchor,
                control.owner,
                control.dirty_value
            ),
            (
                "edited neighbor".into(),
                4,
                1,
                Some(node(doc, "form")),
                true
            )
        );
        assert_eq!(forms.text_default_value(neighbor).unwrap(), "original");
        assert_eq!(doc.attr(neighbor, "value"), Some("original"));
        assert!(!page.has_navigation_request());
        assert!(page.take_dynamic_scripts().is_empty());
        assert!(page.take_dynamic_stylesheets().is_empty());
        let (a, b, c) = page.take_image_changes();
        assert!(a.is_empty() && b.is_empty() && c.is_empty());
    }
    fn run(doc: &mut Document, forms: &mut Forms, page: &mut Page, source: &str) {
        let outcome = page.console_eval(doc, forms, source);
        assert_eq!(outcome.errors, 0, "{source}\n{}", page.report());
        kept(doc, forms, page);
    }

    #[test]
    fn computed_transform_live_matrix_case_identity_percentage_detached_and_readonly() {
        let (mut doc, mut forms, mut page, _) = fixture();
        run(
            &mut doc,
            &mut forms,
            &mut page,
            r#"
var target=document.getElementById('target'),ancestor=document.getElementById('ancestor'),saved=getComputedStyle(target);
if(saved.getPropertyValue('--control').trim()!=='sheet'||saved.position!=='relative'||saved.overflow!=='clip')throw 'real existing computed prefix';
if(!('transform' in saved)||'Transform' in saved||'TRANSFORM' in saved||'transformOrigin' in saved||'webkitTransform' in saved)throw 'ordinary transform member case/scope';
if(saved.transform!=='matrix(1, 0, 0, 1, 12.5, -7)'||saved.getPropertyValue('TrAnSfOrM')!==saved.transform||getComputedStyle(target)===saved)throw 'own 2D matrix from translate3d z0 and fresh declaration identity';
if(getComputedStyle(ancestor).transform!=='matrix(1, 0, 0, 1, 100, 200)')throw 'ancestor transform is separate from self';
target.className='percent';if(saved.transform!=='matrix(1, 0, 0, 1, 21, -22)')throw 'percentage uses own untransformed border box';
target.className='identity';if(saved.transform!=='matrix(1, 0, 0, 1, 0, 0)')throw 'identity is not none';
target.className='plain';if(saved.transform!=='none')throw 'known none';
target.className='inherit';if(saved.transform!=='matrix(1, 0, 0, 1, 100, 200)')throw 'explicit computed inheritance';
var names=0;var value=saved.getPropertyValue({toString:function(){names++;target.className='later';return 'TRANSFORM';}});
if(names!==1||value!=='matrix(1, 0, 0, 1, -9, 0)'||saved.transform!==value)throw 'name conversion precedes current cascade';
ancestor.removeChild(target);if(saved.transform!==''||saved.getPropertyValue('TRANSFORM')!==''||!('transform' in saved))throw 'detached empty declaration retains its interface';
ancestor.appendChild(target);if(saved.transform!==value||target.ownerDocument!==document||document.getElementById('neighbor').value!=='edited neighbor')throw 'live reconnection and neighboring native state';
"#,
        );
        let commits = page.state.dom_commits;
        for source in [
            "saved.transform='none';",
            "saved.setProperty('transform','none');",
            "saved.removeProperty('transform');",
            "saved.getPropertyValue('transform-origin');",
        ] {
            page.clear_console();
            let outcome = page.console_eval(&mut doc, &mut forms, source);
            assert_eq!(outcome.errors, 1, "{source}\n{}", page.report());
            assert_eq!(page.state.dom_commits, commits);
            run(&mut doc,&mut forms,&mut page,"if(saved.transform!=='matrix(1, 0, 0, 1, -9, 0)')throw 'refused write/read changed actual transform';");
        }
    }

    #[test]
    fn computed_transform_copy_and_host_refusal_are_real_and_unsupported_is_not_none() {
        let (mut doc, mut forms, mut page, target) = fixture();
        {
            let mut bridge = Bridge {
                doc: &mut doc,
                forms: &mut forms,
                state: &mut page.state,
            };
            let Value::Host(handle) = bridge
                .create_computed_style(&[Value::Host(node_handle(target))])
                .unwrap()
            else {
                panic!("real declaration")
            };
            const EXPECTED: &str = "matrix(1, 0, 0, 1, 12.5, -7)";
            assert!(
                matches!(bridge.get(handle,"transform").unwrap(),Value::String(value) if value==EXPECTED)
            );
            bridge.state.copy_work = EXPECTED.len() - 1;
            assert!(bridge.get(handle, "transform").is_err());
            assert_eq!(bridge.state.copy_work, 0);
            bridge.state.copy_work = EXPECTED.len();
            assert!(
                matches!(bridge.get(handle,"transform").unwrap(),Value::String(value) if value==EXPECTED)
            );
            assert_eq!(bridge.state.copy_work, 0);
            bridge.state.copy_work = EXPECTED.len();
            assert!(
                matches!(bridge.computed_property_value(handle,&[Value::String("TRANSFORM".into())]).unwrap(),Value::String(value) if value==EXPECTED)
            );
            assert_eq!(bridge.state.copy_work, 0);
            bridge.state.host_work = 0;
            bridge.state.copy_work = EXPECTED.len();
            assert!(bridge.get(handle, "transform").is_err());
            assert_eq!(bridge.state.host_work, 0);
            assert_eq!(bridge.state.copy_work, EXPECTED.len());
        }
        run(&mut doc,&mut forms,&mut page,"var target=document.getElementById('target'),saved=getComputedStyle(target);target.className='unsupported';");
        let commits = page.state.dom_commits;
        let outcome = page.console_eval(&mut doc, &mut forms, "saved.transform;");
        assert_eq!(outcome.errors, 1, "{}", page.report());
        assert_eq!(page.state.dom_commits, commits);
        run(&mut doc,&mut forms,&mut page,"if(saved.getPropertyValue('--control').trim()!=='sheet')throw 'unrelated custom value';target.className='translated';if(saved.transform!=='matrix(1, 0, 0, 1, 12.5, -7)')throw 'supported recovery';");
    }
}
