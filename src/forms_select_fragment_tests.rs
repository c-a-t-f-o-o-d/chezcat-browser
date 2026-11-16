//! Original plan-boundary checks; no script/browser or network execution here.
use super::*;
#[test]
fn select_fragment_plan_keeps_existing_editor_and_checks_native_caps_before_commit() {
    let mut doc = crate::html::parse("<body><input id=keep value=original><div id=host></div></body>");
    let mut forms = Forms::new(&doc);
    let keep = *forms.controls.keys().next().unwrap();
    forms.controls.get_mut(&keep).unwrap().value.set("owner edited");
    forms.controls.get_mut(&keep).unwrap().value.cursor = 2;
    forms.controls.get_mut(&keep).unwrap().value.anchor = 5;
    let fragment = crate::html::parse_fragment("<select name=choice><option disabled>No<optgroup label=Choices><option value=a>Alpha<option value=b selected>Beta</select>", "div", true).unwrap();
    let plan = forms.prepare_selects(Some((&fragment, doc.nodes.len()))).unwrap();
    assert_eq!(plan.additions.len(), 1);
    let control = plan.additions.values().next().unwrap();
    assert_eq!(control.value.text(), "b");
    assert_eq!(control.select.as_ref().unwrap().selected, Some(2));
    assert!(control.select.as_ref().unwrap().options[0].disabled);
    assert_eq!(forms.controls.len(), 1, "Preparation must not publish controls");
    assert_eq!((forms.controls[&keep].value.cursor, forms.controls[&keep].value.anchor), (2, 5));
    assert_eq!(forms.controls[&keep].value.text(), "owner edited");
    let oversized = crate::html::parse_fragment(&format!("<select>{}</select>", "<option value=x>X".repeat(MAX_SELECT_OPTIONS + 1)), "div", true).unwrap();
    assert!(forms.prepare_selects(Some((&oversized, doc.nodes.len()))).is_err());
    assert_eq!(forms.controls.len(), 1);
    for source in ["<select multiple><option>x</select>", "<select form=external><option>x</select>"] {
        let fragment = crate::html::parse_fragment(source, "div", true).unwrap();
        assert!(forms.prepare_selects(Some((&fragment, doc.nodes.len()))).is_err());
    }
    // Real initial native control capacity, without a fake allocator or
    // weakening the dynamic document node limit.
    doc = crate::html::parse(&format!("<body>{}</body>", "<input value=x>".repeat(MAX_CONTROLS)));
    let full = Forms::new(&doc);
    assert_eq!(full.controls.len(), MAX_CONTROLS);
    assert!(full.prepare_selects(Some((&fragment, doc.nodes.len()))).is_err());
    let groups = format!("<select>{}</select>", "<option value=x>X".repeat(MAX_SELECT_OPTIONS));
    let option_full_doc = crate::html::parse(&format!("<body>{}</body>", groups.repeat(MAX_TOTAL_OPTIONS / MAX_SELECT_OPTIONS)));
    let option_full = Forms::new(&option_full_doc);
    assert_eq!(option_full.controls.values().filter_map(|control| control.select.as_ref()).map(|select| select.options.len()).sum::<usize>(), MAX_TOTAL_OPTIONS);
    let small = crate::html::parse_fragment("<select><option value=y>Y</select>", "div", true).unwrap();
    assert!(option_full.prepare_selects(Some((&small, option_full_doc.nodes.len()))).is_err());
}
