//! Original checked table-fragment leaf ownership and refusal cases.
//! https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-intabletext
use super::*;

fn name(document: &Document, node: usize) -> &str {
    match &document.nodes[node].kind { NodeKind::Element { tag, .. } => tag, _ => "" }
}
fn table(document: &Document) -> usize {
    document.nodes.iter().position(|node| matches!(&node.kind, NodeKind::Element { tag, .. } if tag == "table")).unwrap()
}
fn text(document: &Document, node: usize) -> &str {
    match &document.nodes[node].kind { NodeKind::Text(value) => value, _ => panic!("expected real Text child") }
}
fn comment(document: &Document, node: usize) -> &str {
    match &document.nodes[node].kind { NodeKind::Comment(value) => value, _ => panic!("expected real Comment child") }
}

#[test]
fn fragment_table_leaves_keep_whitespace_attributes_and_surrounding_siblings() {
    for scripting in [false, true] {
        let document = parse_fragment("before<TABLE id=grid data-label='a&amp;b'>\r\n \t\u{c}</TaBlE >after", "div", scripting).unwrap();
        assert_eq!(document.scripting_enabled, scripting);
        assert_eq!(document.mode, DocumentMode::NoQuirks);
        assert_eq!(document.nodes.len(), 5);
        let roots = &document.nodes[0].children;
        assert_eq!(roots.len(), 3);
        assert_eq!(text(&document, roots[0]), "before");
        assert_eq!(name(&document, roots[1]), "table");
        assert_eq!(text(&document, roots[2]), "after");
        let table = roots[1];
        assert_eq!(document.attr(table, "id"), Some("grid"));
        assert_eq!(document.attr(table, "data-label"), Some("a&b"));
        assert_eq!(document.nodes[table].namespace, Namespace::Html);
        assert_eq!(document.nodes[table].children.len(), 1);
        let child = document.nodes[table].children[0];
        assert_eq!(text(&document, child), "\n \t\u{c}");
        assert_eq!(document.nodes[child].namespace, Namespace::None);
    }
}

#[test]
fn fragment_table_leaves_decode_references_before_ascii_whitespace_classification() {
    for (source, expected) in [("&#32;&#x9;&NewLine;&#13;&#x0c;", " \t\n\r\u{c}"),
        ("&Tab; &#10;\r\n&#xD;", "\t \n\n\r"), ("&#32&#9;", " \t")] {
        let document = parse_fragment(&format!("<table>{source}</table>"), "div", false).unwrap();
        let table = table(&document);
        assert_eq!(document.nodes[table].children.len(), 1);
        assert_eq!(text(&document, document.nodes[table].children[0]), expected);
    }
}

#[test]
fn fragment_table_leaves_preserve_comment_data_and_exact_text_comment_order() {
    let document = parse_fragment("<table> \n<!--&amp; <b>雪-->\t<!----><!--dash--!></table>", "div", true).unwrap();
    let table = table(&document);
    let children = &document.nodes[table].children;
    assert_eq!(children.len(), 5);
    assert_eq!(text(&document, children[0]), " \n");
    assert_eq!(comment(&document, children[1]), "&amp; <b>雪");
    assert_eq!(text(&document, children[2]), "\t");
    assert_eq!(comment(&document, children[3]), "");
    assert_eq!(comment(&document, children[4]), "dash");
    assert!(children.iter().all(|&node| document.nodes[node].namespace == Namespace::None));
}

#[test]
fn fragment_table_leaves_reuse_bogus_comment_and_processing_instruction_tokens() {
    let document = parse_fragment("<table><!marker></9><?pi value?><![CDATA[x]]></table>", "div", true).unwrap();
    let table = table(&document);
    let children = &document.nodes[table].children;
    assert_eq!(children.len(), 4);
    assert_eq!(comment(&document, children[0]), "marker");
    assert_eq!(comment(&document, children[1]), "9");
    assert!(matches!(&document.nodes[children[2]].kind, NodeKind::ProcessingInstruction { target, data } if target == "pi" && data == "value"));
    assert_eq!(comment(&document, children[3]), "[CDATA[x]]");
    assert!(children.iter().all(|&node| document.nodes[node].namespace == Namespace::None));
}

#[test]
fn fragment_table_leaves_ignore_doctypes_and_empty_end_tokens_without_splitting_text() {
    let document = parse_fragment("<table> <!doctype other>\t</><!DOCTYPE html PUBLIC 'p'>\n</table>", "div", true).unwrap();
    assert_eq!(document.mode, DocumentMode::NoQuirks);
    assert_eq!(document.nodes.len(), 3);
    assert!(document.nodes.iter().all(|node| !matches!(node.kind, NodeKind::DocumentType(_))));
    let table = table(&document);
    assert_eq!(document.nodes[table].children.len(), 1);
    assert_eq!(text(&document, document.nodes[table].children[0]), " \t\n");
    let empty = parse_fragment("<table><!doctype html></></table>", "div", false).unwrap();
    assert_eq!(empty.nodes.len(), 2);
    assert!(empty.nodes[table].children.is_empty());
}

#[test]
fn fragment_table_leaves_keep_each_supported_flow_parent_and_table_separate() {
    for context in ["body", "main", "div", "section", "article", "header", "footer", "aside", "nav", "blockquote", "ul", "ol", "li", "dl", "dt", "dd"] {
        let document = parse_fragment("<table> </table><table><!--second--></table>", context, true).unwrap();
        let children = &document.nodes[0].children;
        assert_eq!(children.len(), 2, "{context}");
        assert!(children.iter().all(|&node| name(&document, node) == "table"));
        assert_eq!(text(&document, document.nodes[children[0]].children[0]), " ");
        assert_eq!(comment(&document, document.nodes[children[1]].children[0]), "second");
    }
    let document = parse_fragment("<section><figure><table>\t</table></figure><table>\n</table></section>", "div", true).unwrap();
    let section = document.nodes[0].children[0];
    assert_eq!(document.nodes[section].children.len(), 2);
    let figure = document.nodes[section].children[0];
    assert_eq!(name(&document, figure), "figure");
    assert_eq!(name(&document, document.nodes[figure].children[0]), "table");
    assert_eq!(name(&document, document.nodes[section].children[1]), "table");
}

#[test]
fn fragment_table_leaves_require_clean_start_end_and_explicit_close() {
    for source in ["<table>", "<table> ", "<table><!--closed-->", "<table><!--</table>",
        "<table>\n<?p unfinished", "<table><!doctype unfinished", "<table/>", "<table/></table>",
        "<table / ></table>", "</table>", "<table> </tabl", "<table> </table", "<table> </table x>",
        "<table><!--m--></table/>", "<table> </table/ >", "<table> </div></table>",
        "<table> </table></table>", "<p><table> </table></p>", "<b><table> </table></b>",
        "<select><table> </table></select>", "<table><table></table></table>"] {
        assert!(parse_fragment(source, "div", true).is_err(), "{source}");
    }
    for context in ["p", "span", "b", "select", "template", "table", "tbody", "tr", "td", "svg"] {
        assert!(parse_fragment("<table> <!--m--></table>", context, true).is_err(), "{context}");
    }
    assert!(parse_fragment("<svg><table> </table></svg>", "template", true).is_err());
}

#[test]
fn fragment_table_leaves_refuse_non_whitespace_and_all_element_child_recovery() {
    for content in ["x", " x ", "\u{b}", "\u{a0}", "&#160;", "&#0;", "&lt;", "&unknown;",
        "<3>", "<", "<<", "<tr><td>x</td></tr>", "<col>", "<caption>x</caption>",
        "<b>x</b>", "<input type=hidden>", "<img src=unrequested>", "<form></form>",
        "<script>unexecuted()</script>", "<style>p{color:red}</style>", "<link href=unrequested>",
        "<template></template>"] {
        assert!(parse_fragment(&format!("<table> \n{content}</table>"), "div", true).is_err(), "{content:?}");
    }
    assert!(parse_fragment("<table><!--m-->\0</table>", "div", true).unwrap_err().contains("NULL handling"));
}

#[test]
fn fragment_table_leaves_do_not_reenter_markup_or_reconstruct_formatting_in_whitespace() {
    let document = parse_fragment("<table><!--</table><img src=unrequested><script nonce=x nonce=y>unexecuted()</script>--></table>", "div", true).unwrap();
    assert_eq!(document.nodes.len(), 3);
    assert!(document.nonce_unsafe_nodes.is_empty());
    assert!(document.withheld_inline_styles.is_empty());
    assert_eq!(comment(&document, document.nodes[table(&document)].children[0]), "</table><img src=unrequested><script nonce=x nonce=y>unexecuted()</script>");
    let document = parse_fragment("<p><b>x</p><table> <!--m-->\t</table>tail", "div", true).unwrap();
    let roots = &document.nodes[0].children;
    assert_eq!(roots.len(), 3);
    assert_eq!(name(&document, roots[0]), "p");
    assert_eq!(name(&document, roots[1]), "table");
    assert_eq!(name(&document, roots[2]), "b");
    let children = &document.nodes[roots[1]].children;
    assert_eq!(children.len(), 3);
    assert_eq!(text(&document, children[0]), " ");
    assert_eq!(comment(&document, children[1]), "m");
    assert_eq!(text(&document, children[2]), "\t");
    assert_eq!(text(&document, document.nodes[roots[2]].children[0]), "tail");
}

#[test]
fn fragment_table_leaves_use_exact_shared_node_capacity_and_ignored_token_costs() {
    let parse = |source: &str, nodes| parse_internal(source, Some("div"), true, Some(LoadedTree { nodes, check_cancel: &|| Ok(()) }), None);
    let source = "<table> <!--m--></table>";
    assert_eq!(parse(source, 4).unwrap().nodes.len(), 4);
    assert!(parse(source, 3).unwrap_err().contains("node limit"));
    assert!(parse("<table> </table>", 2).unwrap_err().contains("node limit"));
    assert!(parse("<table><?p?></table>", 2).unwrap_err().contains("node limit"));
    assert_eq!(parse("<table><!doctype other></table>", 2).unwrap().nodes.len(), 2);
    assert_eq!(parse("<table> <!doctype other> </table>", 3).unwrap().nodes.len(), 3);
    let full = format!("<table>{}</table>", "<!---->".repeat(MAX_NODES - 2));
    assert_eq!(parse_fragment(&full, "div", false).unwrap().nodes.len(), MAX_NODES);
    assert!(parse_fragment(&format!("{full}<!---->"), "div", false).unwrap_err().contains("node limit"));
    assert!(parse_fragment(&format!("{}<table> </table>", "<div>".repeat(MAX_DEPTH - 1)), "div", false).unwrap_err().contains("node/depth limit"));
}

#[test]
fn fragment_table_leaves_cancel_long_scans_without_prefix_publication_and_recover() {
    for source in [format!("<table>{}</table>", " ".repeat(32_768)),
        format!("<table><!--{}--></table>", "雪".repeat(16_384)),
        format!("<table><!doctype html SYSTEM '{}'> </table>", "x".repeat(32_768))] {
        let calls = std::cell::Cell::new(0);
        let check = || { calls.set(calls.get() + 1); if calls.get() == 8 { Err("original table leaf cancellation".into()) } else { Ok(()) } };
        let error = parse_internal(&source, Some("div"), true, Some(LoadedTree { nodes: 3, check_cancel: &check }), None).unwrap_err();
        assert_eq!(error, "original table leaf cancellation");
        assert_eq!(calls.get(), 8);
        let recovered = parse_internal(&source, Some("div"), true, Some(LoadedTree { nodes: 3, check_cancel: &|| Ok(()) }), None).unwrap();
        assert_eq!(recovered.nodes.len(), 3);
        assert_eq!(recovered.nodes[table(&recovered)].children.len(), 1);
    }
}

#[test]
fn fragment_table_leaves_later_refusals_return_no_tree_then_fresh_parse_succeeds() {
    for suffix in ["<svg></svg>", "<table><tr></tr></table>", "<select><input></select>", "<textarea/>"] {
        assert!(parse_fragment(&format!("<span>prefix</span><table> <!--m--></table>{suffix}"), "div", true).is_err());
        let document = parse_fragment("<span>prefix</span><table> <!--m--></table><span>after</span>", "div", true).unwrap();
        assert_eq!(document.nodes[0].children.len(), 3);
        assert_eq!(document.nodes[table(&document)].children.len(), 2);
    }
}

#[test]
fn fragment_table_leaves_full_document_table_modes_remain_independent() {
    let source = "<!doctype html><div id=root><table> \n<!--m--><tr><td>x</td></tr></table>tail</div>";
    for document in [parse_loaded_document(source, &|| Ok(())).unwrap(), parse_inert_html(source, 40, &|| Ok(())).unwrap().0] {
        assert_eq!(document.mode, DocumentMode::NoQuirks);
        let table = table(&document);
        let children = &document.nodes[table].children;
        assert_eq!(children.len(), 3);
        assert_eq!(text(&document, children[0]), " \n");
        assert_eq!(comment(&document, children[1]), "m");
        assert_eq!(name(&document, children[2]), "tbody");
        let row = document.nodes[children[2]].children[0];
        assert_eq!(name(&document, row), "tr");
        assert_eq!(name(&document, document.nodes[row].children[0]), "td");
    }
    assert!(parse_fragment("<table> \n<!--m--><tr><td>x</td></tr></table>", "div", true).is_err());
}

#[test]
fn fragment_table_leaves_raw_contexts_keep_markup_literal() {
    let source = "<table> &#32;<!--m--></table>";
    for context in ["title", "textarea", "style", "script", "iframe", "plaintext"] {
        let document = parse_fragment(source, context, true).unwrap();
        assert_eq!(document.nodes.len(), 2);
        assert_eq!(text(&document, 1), if matches!(context, "title" | "textarea") { "<table>  <!--m--></table>" } else { source });
        assert!(document.scripting_enabled);
    }
}
