//! Original inline-style clone/import, atomic source budgets and CSP tests.
use super::*;
fn fixture() -> (Document, Forms, Page) {
    let mut doc =
        crate::html::parse("<html><head></head><body><div id=target></div></body></html>");
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::with_limits(crate::javascript::limits::BROWSER);
    assert_eq!(page.start(&mut doc, &mut forms, &[]).errors, 0);
    (doc, forms, page)
}
fn run(doc: &mut Document, forms: &mut Forms, page: &mut Page, source: &str) {
    let out = page.console_eval(doc, forms, source);
    assert_eq!(out.errors, 0, "{source}\n{}", page.report());
}
#[test]
fn style_import_clone_batches_move_replace_and_retire_real_sources() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r##"
var t=document.createElement('template');t.innerHTML='<style id="one" is="custom-style">.a{color:red}</style><style id="two">.b{color:blue}</style><original-box class="a">Real</original-box>';var source=t.content,fragment=document.importNode(source,true),one=fragment.querySelector('#one'),two=fragment.querySelector('#two'),clone=one.cloneNode(true),empty=one.cloneNode(false),target=document.getElementById('target');
if(one.ownerDocument!==document||source.ownerDocument===document||clone===one||clone.ownerDocument!==document||clone.textContent!==one.textContent||clone.getAttribute('is')!=='custom-style'||empty.textContent!==''||one.isConnected||typeof customElements!=='undefined')throw 'actual passive style copies';
"##,
    );
    assert_eq!(page.state.stylesheets.source_bytes, 0);
    assert!(page.state.stylesheets.sources.is_empty());
    run(
        &mut doc,
        &mut forms,
        &mut page,
        "target.appendChild(fragment);",
    );
    assert_eq!(
        page.state.stylesheets.source_bytes,
        ".a{color:red}".len() + ".b{color:blue}".len()
    );
    assert_eq!(page.state.stylesheets.source_rules, 2);
    assert_eq!(page.state.stylesheets.sources.len(), 2);
    run(&mut doc,&mut forms,&mut page,"var parked=document.createDocumentFragment();parked.appendChild(one);if(one.isConnected||one.parentNode!==parked)throw 'real disconnected style';target.replaceChild(clone,two);if(two.parentNode!==null||clone.parentNode!==target||!clone.isConnected||target.querySelectorAll('style').length!==1)throw 'real stylesheet replacement';");
    assert_eq!(page.state.stylesheets.source_bytes, ".a{color:red}".len());
    assert_eq!(page.state.stylesheets.source_rules, 1);
    assert_eq!(page.state.stylesheets.sources.len(), 1);
    run(&mut doc,&mut forms,&mut page,"target.insertBefore(parked,target.firstChild);if(one.parentNode!==target||!one.isConnected||parked.firstChild!==null)throw 'actual fragment reinsertion';");
    assert_eq!(
        page.state.stylesheets.source_bytes,
        2 * ".a{color:red}".len()
    );
    assert_eq!(page.state.stylesheets.source_rules, 2);
}
#[test]
fn style_import_batch_rejects_combined_bytes_and_rules_without_partial_publication() {
    let (mut doc, mut forms, mut page) = fixture();
    run(&mut doc,&mut forms,&mut page,"var t=document.createElement('template');t.innerHTML='<style>.a{color:red}</style><style>.b{color:blue}</style>';var f=document.importNode(t.content,true),first=f.firstChild,last=f.lastChild,target=document.getElementById('target');");
    page.state.rebuild(&doc).unwrap();
    let original = doc.stylesheet_limits;
    for limits in [
        crate::css::StylesheetLimits {
            document_bytes: ".a{color:red}".len() + ".b{color:blue}".len() - 1,
            ..original
        },
        crate::css::StylesheetLimits {
            rules: 1,
            ..original
        },
    ] {
        doc.stylesheet_limits = limits;
        let commits = page.state.dom_commits;
        let out = page.console_eval(&mut doc, &mut forms, "target.appendChild(f);");
        assert_eq!(out.errors, 1, "{}", page.report());
        assert_eq!(page.state.dom_commits, commits);
        assert_eq!(page.state.stylesheets.source_bytes, 0);
        assert_eq!(page.state.stylesheets.source_rules, 0);
        assert!(page.state.stylesheets.sources.is_empty());
        run(&mut doc,&mut forms,&mut page,"if(f.firstChild!==first||f.lastChild!==last||first.parentNode!==f||last.parentNode!==f||first.isConnected||target.childNodes.length)throw 'failed batch mutated real tree';");
    }
    doc.stylesheet_limits = original;
    run(&mut doc, &mut forms, &mut page, "target.appendChild(f);");
    assert_eq!(page.state.stylesheets.sources.len(), 2);
    assert_eq!(page.state.stylesheets.source_rules, 2);
}
#[test]
fn style_import_batch_preserves_csp_mime_and_duplicate_nonce_guards() {
    let (mut doc, mut forms, mut page) = fixture();
    page.state
        .dynamic
        .policy
        .add_header("style-src 'nonce-safe'");
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r##"
var t=document.createElement('template');t.innerHTML='<style id="allowed" nonce="safe">.a{color:red}</style><style id="denied">.b{color:blue}</style><style id="plain" type="text/plain">.c{color:green}</style><style id="duplicate" nonce="safe" nonce="other">.d{color:yellow}</style>';var f=document.importNode(t.content,true),allowed=f.querySelector('#allowed'),denied=f.querySelector('#denied'),plain=f.querySelector('#plain'),duplicate=f.querySelector('#duplicate');document.getElementById('target').appendChild(f);
if(!allowed.isConnected||!denied.isConnected||!plain.isConnected||!duplicate.isConnected)throw 'withheld styles are real DOM';
"##,
    );
    let index = |name: &str| {
        (0..doc.nodes.len())
            .find(|&n| doc.attr(n, "id") == Some(name))
            .unwrap()
    };
    assert!(!doc.withheld_inline_styles.contains(&index("allowed")));
    for name in ["denied", "plain", "duplicate"] {
        assert!(doc.withheld_inline_styles.contains(&index(name)), "{name}");
    }
    assert_eq!(page.state.stylesheets.sources.len(), 1);
    assert_eq!(page.state.stylesheets.source_bytes, ".a{color:red}".len());
    assert_eq!(page.state.stylesheets.source_rules, 1);
}
#[test]
fn style_import_promoted_multiple_sources_keep_identity_and_atomic_quota_guards() {
    let (mut doc, mut forms, mut page) = fixture();
    run(&mut doc,&mut forms,&mut page,"var t=document.createElement('template');t.innerHTML='<style>.a{color:red}</style><style>.b{color:blue}</style>';var source=t.content,one=source.firstChild,two=source.lastChild;");
    let original = doc.stylesheet_limits;
    doc.stylesheet_limits.rules = 1;
    let out = page.console_eval(&mut doc, &mut forms, "document.head.appendChild(source);");
    assert_eq!(out.errors, 1);
    assert_eq!(page.state.stylesheets.source_rules, 0);
    assert!(page.state.stylesheets.sources.is_empty());
    doc.stylesheet_limits = original;
    run(&mut doc,&mut forms,&mut page,"if(source.firstChild!==one||source.lastChild!==two||one.parentNode!==source)throw 'failed promoted batch';document.head.appendChild(source);if(document.head.firstChild!==one||document.head.lastChild!==two||one.ownerDocument!==document||source.firstChild!==null)throw 'actual multiple promoted style identities';");
    assert_eq!(page.state.stylesheets.source_rules, 2);
    assert_eq!(page.state.stylesheets.sources.len(), 2);
}
