//! Original native base-URI readback, owner and refusal controls.
use super::*;
fn location(source: &str) -> crate::navigation::Location {
    crate::navigation::address(source, &crate::navigation::Location::home()).unwrap()
}
fn fixture(explicit: bool) -> (Document, Forms, Page) {
    let mut doc =
        crate::html::parse("<html><body><p id=loaded>native<!--comment--></p></body></html>");
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::with_limits(crate::javascript::limits::BROWSER);
    let source = location("https://node-base.test/folder/index.html?doc=1#source");
    page.set_location(&source);
    page.set_resource_base(&if explicit {
        location("https://node-base.test/assets/?base=1#base")
    } else {
        source
    });
    assert_eq!(page.start(&mut doc, &mut forms, &[]).errors, 0);
    (doc, forms, page)
}
fn run(doc: &mut Document, forms: &mut Forms, page: &mut Page, source: &str) {
    let out = page.console_eval(doc, forms, source);
    assert_eq!(out.errors, 0, "{source}\n{}", page.report());
}
#[test]
fn node_base_uri_main_represented_nodes_saved_getter_readonly_and_author_override() {
    let (mut doc, mut forms, mut page) = fixture(true);
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var d=Object.getOwnPropertyDescriptor(Node.prototype,'baseURI');if(!d||typeof d.get!=='function'||d.get.length!==0||d.get.name!=='get baseURI'||d.set!==undefined||!d.enumerable||!d.configurable)throw 'real Node base getter';var get=d.get,p=document.getElementById('loaded'),text=p.firstChild,fragment=document.createDocumentFragment(),attr=document.createAttribute('data-original');
for(var node of [document,p,text,text.nextSibling,fragment,attr,document.createElement('span'),document.createTextNode('made'),document.createProcessingInstruction('target','data')])if(node.baseURI!=='https://node-base.test/assets/?base=1#base'||get.call(node)!==node.baseURI)throw 'actual owner base';
if(document.URL!=='https://node-base.test/folder/index.html?doc=1#source'||new URL('icons/mark.svg',p.baseURI).href!=='https://node-base.test/assets/icons/mark.svg')throw 'resource base is not document URL';
var effects=0,bad={toString:function(){effects++;throw 'coerced';}};fragment.baseURI=bad;if(effects||Object.hasOwn(fragment,'baseURI')||fragment.baseURI!==document.baseURI)throw 'readonly';var strict=0;try{(function(){'use strict';fragment.baseURI=bad;})();}catch(e){if(e instanceof TypeError)strict++;else throw e;}if(strict!==1||effects)throw 'strict readonly';Object.defineProperty(fragment,'baseURI',{value:'author',configurable:true});if(fragment.baseURI!=='author'||get.call(fragment)!==document.baseURI)throw 'author versus native';delete fragment.baseURI;delete Node.prototype.baseURI;if(fragment.baseURI!==undefined||get.call(fragment)!=='https://node-base.test/assets/?base=1#base')throw 'deleted getter fallback';Object.defineProperty(Node.prototype,'baseURI',d);
for(var receiver of [{},window,Node.prototype]){var caught=0;try{get.call(receiver);}catch(e){if(e instanceof TypeError)caught++;else throw e;}if(caught!==1)throw 'brand';}
"#,
    );
}
#[test]
fn node_base_uri_inert_document_first_base_live_attributes_import_and_promotion() {
    let (mut doc, mut forms, mut page) = fixture(true);
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var get=Object.getOwnPropertyDescriptor(Node.prototype,'baseURI').get,parsed=new DOMParser().parseFromString('<html><head><base href="https://inert-node-base.test/one/"><base href="https://ignored-node-base.test/"></head><body><p id="p">inert</p></body></html>','text/html'),p=parsed.getElementById('p'),base=parsed.head.firstChild;
if(parsed.baseURI!=='https://inert-node-base.test/one/'||p.baseURI!==parsed.baseURI||get.call(p.firstChild)!==parsed.baseURI)throw 'inert actual first base';base.setAttribute('href','https://inert-node-base.test/two/');if(p.baseURI!=='https://inert-node-base.test/two/')throw 'live inert base attribute';base.setAttribute('href','data:text/plain,blocked');if(p.baseURI!=='about:blank')throw 'disallowed base scheme fallback';
var template=document.createElement('template');template.innerHTML='<span>staged</span>';var staged=template.content.firstChild,owner=staged.ownerDocument;if(staged.baseURI!=='about:blank'||owner.baseURI!=='about:blank')throw 'template owner base';var imported=document.importNode(p,true);if(imported.baseURI!==document.baseURI||p.baseURI!=='about:blank')throw 'import owner';document.body.appendChild(template.content);if(staged.ownerDocument!==document||staged.baseURI!==document.baseURI)throw 'promoted owner base';
"#,
    );
}
#[test]
fn node_base_uri_history_fallback_changes_and_explicit_base_stays_native() {
    for explicit in [false, true] {
        let (mut doc, mut forms, mut page) = fixture(explicit);
        page.set_history_context(HistoryData::Null, !explicit, 1);
        run(&mut doc,&mut forms,&mut page,"var get=Object.getOwnPropertyDescriptor(Node.prototype,'baseURI').get,p=document.getElementById('loaded');history.replaceState(null,'','/changed/path.html?next=2#current');");
        run(
            &mut doc,
            &mut forms,
            &mut page,
            if explicit {
                "if(p.baseURI!=='https://node-base.test/assets/?base=1#base'||get.call(document)!==p.baseURI)throw 'explicit base changed with History';"
            } else {
                "if(p.baseURI!=='https://node-base.test/changed/path.html?next=2#current'||get.call(p)!==location.href)throw 'fallback base not updated';"
            },
        );
    }
}
#[test]
fn node_base_uri_child_inherited_base_foreign_saved_getters_and_child_self_recovery() {
    let (mut doc, mut forms, mut page) = fixture(true);
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var get=Object.getOwnPropertyDescriptor(Node.prototype,'baseURI').get,frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var w=frame.contentWindow,d=frame.contentDocument,b=d.body;if(d.baseURI!==document.baseURI||b.baseURI!==d.baseURI||get.call(b)!==d.baseURI)throw 'inherited blank base';var own=w.Function('return document.baseURI===document.body.baseURI;'),read=w.Function('getter','node','return getter.call(node);');if(!own()||read(get,document)!==document.baseURI)throw 'same origin';
"#,
    );
    page.set_location(&location("https://other-node-base.test/changed"));
    for source in ["get.call(b);", "b.baseURI;", "read(get,document);"] {
        let out = page.console_eval(&mut doc, &mut forms, source);
        assert_eq!(out.errors, 1, "{source}: {}", page.report());
        assert!(page.report().contains("origin") || page.report().contains("Security"));
    }
    run(&mut doc,&mut forms,&mut page,"if(!own()||document.baseURI!=='https://node-base.test/assets/?base=1#base')throw 'actual child owner or main recovery';");
}
