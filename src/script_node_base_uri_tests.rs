//! Original native base-URI readback, owner and refusal controls.
use super::*;
fn location(source: &str) -> crate::navigation::Location {
    crate::navigation::address(source, &crate::navigation::Location::home()).unwrap()
}
fn fixture(explicit: bool) -> (Document, Forms, Page) {
    let mut doc =
        crate::html::parse("<html><body><p id=loaded>native<!--comment--></p><form id=owner><input id=field name=baseURI value=kept></form></body></html>");
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
var effects=0,bad={toString:function(){effects++;throw 'coerced';}};fragment.baseURI=bad;if(effects||Object.hasOwn(fragment,'baseURI')||fragment.baseURI!==document.baseURI)throw 'readonly';var strict=0;try{(function(){'use strict';fragment.baseURI=bad;})();}catch(e){if(e instanceof TypeError)strict++;else throw e;}if(strict!==1||effects)throw 'strict readonly';Object.defineProperty(fragment,'baseURI',{value:'author',configurable:true});if(fragment.baseURI!=='author'||get.call(fragment)!==document.baseURI)throw 'author versus native';delete fragment.baseURI;delete Node.prototype.baseURI;if(fragment.baseURI!==undefined||document.baseURI!==undefined||get.call(fragment)!=='https://node-base.test/assets/?base=1#base')throw 'deleted getter fallback';Object.defineProperty(Node.prototype,'baseURI',d);
var form=document.getElementById('owner'),field=document.getElementById('field');if(form.baseURI!==field||get.call(form)!==document.baseURI||field.form!==form||field.value!=='kept')throw 'legacy form named URI field';field.setAttribute('name','renamed');if(form.baseURI!==document.baseURI||get.call(form)!==document.baseURI)throw 'native getter after named control rename';
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
var get=Object.getOwnPropertyDescriptor(Node.prototype,'baseURI').get,parsed=new DOMParser().parseFromString('<html><head><base href="https://inert-node-base.test/one/"><base href="https://ignored-node-base.test/"></head><body><p id="p">inert</p></body></html>','text/html'),p=parsed.querySelector('#p'),base=parsed.head.firstChild;
if(parsed.baseURI!=='https://inert-node-base.test/one/'||p.baseURI!==parsed.baseURI||get.call(p.firstChild)!==parsed.baseURI)throw 'inert actual first base';var probes=0,nativeQuery=Document.prototype.querySelector,nativeAttribute=Element.prototype.getAttribute;Document.prototype.querySelector=function(){probes++;throw 'author query';};Element.prototype.getAttribute=function(){probes++;throw 'author attribute';};if(p.baseURI!=='https://inert-node-base.test/one/'||probes)throw 'base getter ran author code';Document.prototype.querySelector=nativeQuery;Element.prototype.getAttribute=nativeAttribute;base.setAttribute('href','https://inert-node-base.test/two/');if(p.baseURI!=='https://inert-node-base.test/two/')throw 'live inert base attribute';base.setAttribute('href','data:text/plain,blocked');if(p.baseURI!==parsed.URL)throw 'disallowed base scheme fallback';
var template=document.createElement('template');template.innerHTML='<span>staged</span>';var staged=template.content.firstChild,owner=staged.ownerDocument;if(staged.baseURI!=='about:blank'||owner.baseURI!=='about:blank')throw 'template owner base';var imported=document.importNode(p,true);if(imported.baseURI!==document.baseURI||p.baseURI!==parsed.URL)throw 'import owner';document.body.appendChild(template.content);if(staged.ownerDocument!==document||staged.baseURI!==document.baseURI)throw 'promoted owner base';
"#,
    );
    run(
        &mut doc,
        &mut forms,
        &mut page,
        "var longPath='x';while(longPath.length<20000)longPath=longPath+longPath;base.setAttribute('href','https://inert-node-base.test/'+longPath);",
    );
    let commits = page.state.dom_commits;
    let out = page.console_eval(&mut doc, &mut forms, "p.baseURI;");
    assert_eq!(out.errors, 1, "{}", page.report());
    assert!(
        page.report().contains("URL length limit"),
        "{}",
        page.report()
    );
    assert_eq!(page.state.dom_commits, commits);
    run(&mut doc,&mut forms,&mut page,"base.setAttribute('href','https://inert-node-base.test/recovered/');if(p.baseURI!=='https://inert-node-base.test/recovered/')throw 'projection refusal recovery';");
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

#[test]
fn node_base_uri_local_serialization_copy_work_size_refusal_and_fresh_job_recovery() {
    let (mut doc, mut forms, mut page) = fixture(true);
    let before = (doc.nodes.len(), page.state.dom_commits, page.state.retained);
    for kind in ["copy", "work", "size"] {
        let old = (
            page.state.copy_work,
            page.state.host_work,
            page.state.limits.host.string_bytes,
        );
        match kind {
            "copy" => page.state.copy_work = 0,
            "work" => page.state.host_work = 0,
            _ => page.state.limits.host.string_bytes = 4,
        }
        let result = Bridge {
            doc: &mut doc,
            forms: &mut forms,
            state: &mut page.state,
        }
        .get(DOCUMENT, "baseURI");
        assert!(result.is_err(), "{kind}: {result:?}");
        assert_eq!(
            (doc.nodes.len(), page.state.dom_commits, page.state.retained),
            before
        );
        (
            page.state.copy_work,
            page.state.host_work,
            page.state.limits.host.string_bytes,
        ) = old;
    }
    run(&mut doc,&mut forms,&mut page,"if(document.baseURI!=='https://node-base.test/assets/?base=1#base')throw 'fresh getter job';");
    let path = std::env::temp_dir().join("original node base #percent%.html");
    let local = crate::navigation::Location {
        root: path.parent().map(|p| p.to_path_buf()),
        path: Some(path),
        url: None,
        fragment: "section with space".into(),
    };
    page.set_location(&local);
    page.set_resource_base(&local);
    run(&mut doc,&mut forms,&mut page,"if(document.baseURI!==location.href||document.body.baseURI!==document.baseURI||document.baseURI.indexOf('file:')!==0||document.baseURI.indexOf('%20')<0||document.baseURI.indexOf('%23')<0||document.baseURI.indexOf('%25')<0)throw 'native file URI serialization';");
    assert!(page.take_dynamic_scripts().is_empty());
    assert!(page.take_dynamic_stylesheets().is_empty());
}
