//! Original controls for the finite conventional select fragment parser.
//! Native form ownership and App interaction are verified by separate controls.
use super::*;

fn node_tag(document: &Document, node: usize) -> &str {
    match &document.nodes[node].kind { NodeKind::Element { tag, .. } => tag, _ => "" }
}
fn all_text(document: &Document) -> String {
    document.nodes.iter().filter_map(|node| match &node.kind {
        NodeKind::Text(text) => Some(text.as_str()), _ => None,
    }).collect()
}

#[test]
fn select_fragment_balanced_tree_keeps_boolean_unknown_attributes_and_cr_preprocessing() {
    let document = parse_fragment("<a id=a></a><select id='probe-\r\\' msallowcapture=''><option selected=''></option></select>", "div", true).unwrap();
    assert_eq!(document.nodes[0].children.len(), 2);
    let anchor = document.nodes[0].children[0];
    let select = document.nodes[0].children[1];
    assert_eq!(node_tag(&document, anchor), "a");
    assert_eq!(node_tag(&document, select), "select");
    assert_eq!(document.attr(select, "id"), Some("probe-\n\\"));
    assert_eq!(document.attr(select, "msallowcapture"), Some(""));
    assert_eq!(document.nodes[select].namespace, Namespace::Html);
    assert_eq!(document.nodes[select].children.len(), 1);
    let option = document.nodes[select].children[0];
    assert_eq!(node_tag(&document, option), "option");
    assert_eq!(document.attr(option, "selected"), Some(""));
    assert!(document.scripting_enabled);
}

#[test]
fn select_fragment_omitted_option_and_group_endings_keep_exact_sibling_ownership() {
    let document = parse_fragment("<select><option value=a>Alpha<option value=b selected>Beta<optgroup label=First><option>One<option>Two<optgroup label=Second disabled><option>Three</select><p>After</p>", "div", false).unwrap();
    let select = document.nodes[0].children[0];
    let children = &document.nodes[select].children;
    assert_eq!(children.len(), 4);
    assert_eq!(document.attr(children[0], "value"), Some("a"));
    assert_eq!(document.attr(children[1], "value"), Some("b"));
    assert_eq!(document.attr(children[1], "selected"), Some(""));
    assert_eq!(document.attr(children[2], "label"), Some("First"));
    assert_eq!(document.attr(children[3], "label"), Some("Second"));
    assert_eq!(document.attr(children[3], "disabled"), Some(""));
    assert_eq!(document.nodes[children[2]].children.len(), 2);
    assert_eq!(document.nodes[children[3]].children.len(), 1);
    assert_eq!(document.nodes[0].children.len(), 2);
    assert_eq!(all_text(&document), "AlphaBetaOneTwoThreeAfter");
    assert!(!document.scripting_enabled);
}

#[test]
fn select_fragment_contexts_are_explicit_and_never_use_an_escapable_markup_wrapper() {
    let select = parse_fragment("<option value=a>A<option value=b selected>B<optgroup label=G disabled><option>C<option>D", "select", true).unwrap();
    assert_eq!(select.nodes[0].children.len(), 3);
    assert_eq!(node_tag(&select, select.nodes[0].children[2]), "optgroup");
    let group = parse_fragment("<option>A<option>B", "optgroup", true).unwrap();
    assert_eq!(group.nodes[0].children.len(), 2);
    let option = parse_fragment("A&amp;B\r\nC", "option", true).unwrap();
    assert_eq!(option.nodes[0].children.len(), 1);
    assert_eq!(all_text(&option), "A&B\nC");
    for (source, context) in [("<b>x</b>", "select"), ("<select><option>x", "select"),
        ("</select><div>escape</div>", "select"), ("<optgroup><option>x", "optgroup"),
        ("</optgroup><option>x", "optgroup"), ("<b>rich</b>", "option"), ("</option>escape", "option")] {
        assert!(parse_fragment(source, context, true).is_err(), "{context}: {source}");
    }
}

#[test]
fn select_fragment_nonconventional_recovery_resources_and_existing_bounds_still_refuse() {
    let siblings = parse_fragment("<select><option>x</select><table></table>", "div", true).unwrap();
    let children = &siblings.nodes[0].children;
    assert_eq!(children.len(), 2);
    assert_eq!(node_tag(&siblings, children[0]), "select");
    assert_eq!(node_tag(&siblings, children[1]), "table");
    assert!(siblings.nodes[children[1]].children.is_empty());
    for source in ["<option>orphan</option>", "<optgroup><option>orphan</optgroup>",
        "<select><option>outer<select><option>inner</select></select>",
        "<select><div>rich descendant</div></select>", "<select><option><b>rich</b></option></select>",
        "<select><script>injected()</script></select>", "<select><input value=x></select>",
        "<select><option>x</div>", "<select><optgroup></option></select>",
        "<select><option>x</select><table><tr></tr></table>", "<select></select><svg></svg>"] {
        assert!(parse_fragment(source, "div", true).is_err(), "{source}");
    }
    assert!(parse_fragment(&format!("<select>{}</select>", "<option>x".repeat(MAX_NODES)), "div", true).is_err());
    assert!(parse_fragment(&format!("{}<select><option>x", "<div>".repeat(MAX_DEPTH - 2)), "div", true).is_err());
    let template = parse_fragment("<select><option>x</option></select>", "template", true).unwrap();
    assert_eq!(template.nodes[0].children.len(), 1);
    assert_eq!(node_tag(&template, template.nodes[0].children[0]), "select");
    for context in ["table", "tr", "svg"] {
        assert!(parse_fragment("<select><option>x</option></select>", context, true).is_err());
    }
}
