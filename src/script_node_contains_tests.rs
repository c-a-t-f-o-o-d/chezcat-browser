//! Physical menu ancestry, Node interface branding and live frame access.
use super::*;

#[test]
fn node_contains_choice_original_physical_menu_then_live_nodes_and_attr_discriminator() {
    let mut doc = crate::html::parse(concat!(
        "<!doctype html><title>Original contains choice</title><body>",
        "<ul id=menu><li id=item><button id=inside type=button>Original choice</button></li></ul>",
        "<button id=outside type=button>Outside choice</button><div id=destination></div>"
    ));
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::for_test();
    page.set_location(
        &crate::navigation::address(
            "https://contains-choice.test/original",
            &crate::navigation::Location::home(),
        )
        .unwrap(),
    );
    assert_eq!(
        page.start(&mut doc, &mut forms, &[]).errors,
        0,
        "{}",
        page.report()
    );

    let prefix = page.console_eval(
        &mut doc,
        &mut forms,
        r#"
var menu = document.getElementById('menu'),
    item = document.getElementById('item'),
    inside = document.getElementById('inside'),
    outside = document.getElementById('outside'),
    destination = document.getElementById('destination'),
    text = inside.firstChild,
    attr = menu.getAttributeNode('id');
if (menu.nodeName !== 'UL' || inside.nodeName !== 'BUTTON' || text.nodeType !== 3
    || inside.parentNode !== item || item.parentNode !== menu
    || outside.parentNode !== document.body || menu.ownerDocument !== document
    || document.querySelector('#inside') !== inside)
    throw 'genuine physical menu/choice identity';
if (menu.getRootNode() !== document || inside.getRootNode() !== document
    || text.getRootNode() !== document || attr.getRootNode() !== attr
    || menu.compareDocumentPosition(inside) !== 20 || menu.compareDocumentPosition(attr) !== 20)
    throw 'established physical roots and special Attr position ordering';
document.title = 'Contains choice physical prefix passed';
"#,
    );
    assert_eq!(prefix.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "Contains choice physical prefix passed");

    let outcome = page.console_eval(
        &mut doc,
        &mut forms,
        r#"
var first = menu.contains(inside);
if (first !== true || menu.contains(menu) !== true || menu.contains(outside) !== false
    || inside.contains(menu) !== false || menu.contains(text) !== true
    || document.contains(menu) !== true || menu.contains(null) !== false)
    throw 'inclusive genuine Node ancestry, outside target and null';
if (menu.contains(attr) !== false || document.contains(attr) !== false
    || attr.contains(attr) !== true || attr.contains(menu) !== false)
    throw 'Attr is not a child despite compareDocumentPosition owner ordering';
menu.removeChild(item);
if (menu.contains(inside) !== false || item.contains(inside) !== true
    || inside.getRootNode() !== item || document.contains(inside) !== false)
    throw 'actual detached subtree ancestry';
destination.appendChild(item);
if (destination.contains(inside) !== true || menu.contains(inside) !== false
    || document.contains(inside) !== true || item.parentNode !== destination)
    throw 'live movement must change actual containing Node';
menu.appendChild(item);
if (menu.contains(inside) !== true || destination.contains(inside) !== false
    || inside.parentNode !== item || item.parentNode !== menu)
    throw 'physical identity after reattachment';
var conversions = 0, shaped = {
    nodeType: 1,
    toString: function () { conversions++; return 'not a Node'; },
    valueOf: function () { conversions++; return 1; }
};
try { menu.contains(shaped); throw 'Node-shaped argument accepted'; }
catch (e) { if (!(e instanceof TypeError)) throw e; }
var borrowed = menu.contains;
try { borrowed.call(shaped, inside); throw 'Node-shaped receiver accepted'; }
catch (e) { if (!(e instanceof TypeError)) throw e; }
if (conversions !== 0 || menu.contains(inside) !== true || attr.value !== 'menu')
    throw 'brand checks coerced input or changed real tree';
"#,
    );
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert!(!page.has_navigation_request());
    assert!(page.take_dynamic_scripts().is_empty());
    assert!(page.take_dynamic_stylesheets().is_empty());
    let (a, b, c) = page.take_image_changes();
    assert!(a.is_empty() && b.is_empty() && c.is_empty());
}


fn contains_location(source: &str) -> crate::navigation::Location {
    crate::navigation::address(source, &crate::navigation::Location::home()).unwrap()
}

fn contains_fixture() -> (Document, Forms, Page) {
    contains_fixture_with_page(Page::for_test())
}

fn contains_fixture_with_page(mut page: Page) -> (Document, Forms, Page) {
    let mut doc = crate::html::parse(concat!(
        "<!doctype html><html><head><title>Contains controls</title>",
        "<style>#menu{padding:5px;border:2px solid;width:90px;color:#123456}#inside{width:60px;height:24px}</style>",
        "</head><body><ul id=menu><li id=item><button id=inside type=button>Choice</button></li></ul>",
        "<button id=outside type=button>Outside</button><div id=destination></div>",
        "<form id=form><input id=neighbor name=neighbor value=original><input id=shadow name=contains></form></body></html>"
    ));
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let location = contains_location("https://contains-controls.test/source");
    page.set_location(&location);
    page.set_resource_base(&location);
    assert_eq!(
        page.start(&mut doc, &mut forms, &[]).errors,
        0,
        "{}",
        page.report()
    );
    (doc, forms, page)
}

fn contains_run(doc: &mut Document, forms: &mut Forms, page: &mut Page, source: &str) {
    let outcome = page.console_eval(doc, forms, source);
    assert_eq!(outcome.errors, 0, "{source}\n{}", page.report());
    assert!(!page.has_navigation_request());
    assert!(page.take_dynamic_scripts().is_empty());
    assert!(page.take_dynamic_stylesheets().is_empty());
    let (a, b, c) = page.take_image_changes();
    assert!(a.is_empty() && b.is_empty() && c.is_empty());
}

fn contains_id(doc: &Document, id: &str) -> usize {
    (0..doc.nodes.len())
        .find(|&node| doc.attr(node, "id") == Some(id))
        .unwrap()
}

fn contains_editor(
    forms: &Forms,
    node: usize,
) -> (String, usize, usize, usize, bool, Option<usize>) {
    let control = &forms.controls[&node];
    (
        control.value.text(),
        control.value.cursor,
        control.value.anchor,
        control.value.offset,
        control.dirty_value,
        control.owner,
    )
}

#[test]
fn node_contains_required_nullable_and_genuine_node_brands_do_not_coerce_arguments() {
    let (mut doc, mut forms, mut page) = contains_fixture();
    contains_run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var method=Node.prototype.contains,menu=document.getElementById('menu'),inside=document.getElementById('inside');
var form=document.getElementById('form'),shadow=document.getElementById('shadow');
if(form.contains!==shadow||!method.call(form,shadow)||method.call(form,inside))throw 'named form control and genuine borrowed operation';
var text=inside.firstChild,comment=document.createComment('contains'),fragment=document.createDocumentFragment(),attr=menu.getAttributeNode('id');
var nodes=[document,menu,text,comment,fragment,attr],missing=0;
for(var receiver of nodes){
 if(method.call(receiver,receiver)!==true||method.call(receiver,null)!==false||method.call(receiver,undefined)!==false)throw 'inclusive self or explicit nullable argument';
 try{method.call(receiver);throw 'missing required argument accepted';}catch(error){if(!(error instanceof TypeError))throw error;missing++;}
}
if(missing!==nodes.length)throw 'required argument witness';
var touches=0,shaped={get nodeType(){touches++;return 1;},get ownerDocument(){touches++;return document;},toString:function(){touches++;return 'Node';},valueOf:function(){touches++;return 1;}};
Object.defineProperty(shaped,Symbol.toPrimitive,{get:function(){touches++;return function(){touches++;return 1;};}});
for(var candidate of [false,0,1,'',Symbol('not a Node'),{},function(){},shaped]){
 try{method.call(menu,candidate);throw 'non-Node argument accepted';}catch(error){if(!(error instanceof TypeError))throw error;}
}
for(var receiver of [null,undefined,window,1,'Node',shaped]){
 try{method.call(receiver,inside);throw 'non-Node receiver accepted';}catch(error){if(!(error instanceof TypeError))throw error;}
}
if(method.call(menu,inside,shaped)!==true||touches!==0||attr.value!=='menu'||text.data!=='Choice')throw 'interface conversion or ignored argument caused author conversion';
"#,
    );
}

#[test]
fn node_contains_canonical_descriptors_prototype_overrides_deletion_and_nonconstruction() {
    let (mut doc, mut forms, mut page) = contains_fixture();
    contains_run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var P=Node.prototype,D=Object.getOwnPropertyDescriptor(P,'contains'),method=D.value,inside=document.getElementById('inside'),text=inside.firstChild;
if(typeof method!=='function'||method.name!=='contains'||method.length!==1||!D.writable||!D.enumerable||!D.configurable||D.get!==undefined||D.set!==undefined)throw 'canonical operation descriptor';
if(Object.getPrototypeOf(method)!==Function.prototype||method.constructor!==Function||Object.hasOwn(method,'prototype')||method.prototype!==undefined)throw 'operation function inheritance';
for(var key of ['name','length']){var d=Object.getOwnPropertyDescriptor(method,key);if(d.writable||d.enumerable||!d.configurable)throw 'operation metadata descriptor';}
if(document.contains!==method||inside.contains!==method||text.contains!==method||Object.hasOwn(document,'contains')||Object.hasOwn(inside,'contains')||Object.hasOwn(text,'contains')||Object.hasOwn(Element.prototype,'contains'))throw 'one inherited Node operation';
try{new method(document);throw 'operation was constructible';}catch(error){if(!(error instanceof TypeError))throw error;}
var reads=0;
Object.defineProperty(P,'contains',{configurable:true,get:function(){if(this!==text)throw 'inherited getter receiver';reads++;return method;}});
if(text.contains!==method||reads!==1||method.call(inside,text)!==true||Object.hasOwn(text,'contains'))throw 'ordinary inherited getter or saved native method';
delete P.contains;
for(var receiver of [document,inside,text])if(receiver.contains!==undefined||'contains' in receiver)throw 'deleted native operation resurrected';
Object.defineProperty(P,'contains',D);
inside.contains=function(value){return this===inside&&value===17?'author own':'wrong';};
if(inside.contains(17)!=='author own'||!Object.hasOwn(inside,'contains')||method.call(inside,text)!==true)throw 'ordinary Element own shadow';
delete inside.contains;
if(inside.contains!==method||!delete document.contains||document.contains!==method)throw 'deleting absent own method changed prototype';
var savedNode=Node;Node=17;
if(inside.contains!==method||method.call(inside,text)!==true||P.constructor!==savedNode)throw 'canonical prototype depended on global constructor value';
Node=savedNode;
"#,
    );
}

#[test]
fn node_contains_child_realms_use_actual_documents_and_revalidate_cached_callers_after_navigation()
{
    // This exercises real realm behavior, not the adversarial Function budget.
    // Keep the explicit work-refusal fixtures on their existing test profile.
    let (mut doc, mut forms, mut page) = contains_fixture_with_page(Page::new());
    contains_run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var mainBody=document.body,mainContains=Node.prototype.contains;
var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);
var w=frame.contentWindow,F=w.Function,childDoc=w.document,childHtml=childDoc.documentElement,childHead=childDoc.head,childBody=childDoc.body;
var childContains=w.Node.prototype.contains,childRead=F('node','return node.contains(node);'),childInvoke=F('method','node','return method.call(node,node);');
var ownChild=F('return document.contains(document.documentElement)&&document.documentElement.contains(document.head)&&document.documentElement.contains(document.body)&&document.body.contains(document.body)&&window===self;');
var descriptor=Object.getOwnPropertyDescriptor(w.Node.prototype,'contains');
if(childContains===mainContains||childContains.name!=='contains'||childContains.length!==1||childContains.constructor!==w.Function||!descriptor.writable||!descriptor.enumerable||!descriptor.configurable)throw 'genuine owner-realm operation';
if(childDoc.contains(childHtml)!==true||childHtml.contains(childHead)!==true||childHtml.contains(childBody)!==true||childHead.contains(childBody)!==false||childBody.contains(childHtml)!==false||childDoc.contains(childDoc)!==true)throw 'actual child physical tree';
if(document.contains(childDoc)!==false||mainBody.contains(childBody)!==false||childDoc.contains(mainBody)!==false||childBody.contains(mainBody)!==false)throw 'different node documents were conflated';
if(mainContains.call(childHtml,childBody)!==true||childContains.call(mainBody,mainBody)!==true||childRead(mainBody)!==true||childInvoke(mainContains,mainBody)!==true||!ownChild())throw 'genuine same-origin borrowed operations';
try{new childContains(childBody);throw 'child operation was constructible';}catch(error){if(!(error instanceof TypeError))throw error;}
"#,
    );
    page.set_location(&contains_location(
        "https://contains-controls.test/same-origin-navigation",
    ));
    contains_run(&mut doc, &mut forms, &mut page,
        "if(mainContains.call(childHtml,childBody)!==true||childRead(mainBody)!==true||!ownChild())throw 'same-origin navigation changed genuine owner access';");
    page.set_location(&contains_location("https://other-contains.test/changed"));
    for source in [
        "childBody.contains(childBody);",
        "mainContains.call(childHtml,childBody);",
        "mainContains.call(mainBody,childBody);",
        "mainContains.call(childBody,null);",
        "childContains.call(childBody,childBody);",
        "childRead(mainBody);",
        "childInvoke(mainContains,mainBody);",
    ] {
        page.clear_console();
        let windows = page.runtime.window_allocation_state_for_test();
        let outcome = page.console_eval(&mut doc, &mut forms, source);
        assert_eq!(outcome.errors, 1, "{source}\n{}", page.report());
        assert!(
            page.report().contains("same-origin"),
            "{source}\n{}",
            page.report()
        );
        assert_eq!(
            page.runtime.window_allocation_state_for_test(),
            windows,
            "{source}"
        );
        contains_run(&mut doc, &mut forms, &mut page,
            "if(!ownChild()||mainContains.call(mainBody,mainBody)!==true||mainBody.ownerDocument!==document)throw 'owner-local recovery after refused access';");
    }
}

#[test]
fn node_contains_actual_parent_reads_preserve_css_layout_and_native_edited_default_state() {
    let (mut doc, mut forms, mut page) = contains_fixture();
    let neighbor = contains_id(&doc, "neighbor");
    forms.script_set_value(neighbor, "edited neighbor").unwrap();
    forms.controls.get_mut(&neighbor).unwrap().value.cursor = 5;
    forms.controls.get_mut(&neighbor).unwrap().value.anchor = 2;
    forms.controls.get_mut(&neighbor).unwrap().value.offset = 1;
    contains_run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var menu=document.getElementById('menu'),item=document.getElementById('item'),inside=document.getElementById('inside'),outside=document.getElementById('outside'),destination=document.getElementById('destination');
destination.appendChild(item);menu.appendChild(item);
var fragment=document.createDocumentFragment(),fresh=document.createElement('span'),text=document.createTextNode('detached'),comment=document.createComment('comment');
fragment.appendChild(fresh);fresh.appendChild(text);fresh.appendChild(comment);
"#,
    );
    let nodes = format!("{:?}", doc.nodes);
    let commits = page.state.dom_commits;
    let editor = contains_editor(&forms, neighbor);
    let before =
        crate::layout::build_viewport(&doc, 640, 480, 1.0, &HashMap::new(), &HashMap::new());
    let styles = crate::css::compute(&doc, &HashMap::new());
    let menu = contains_id(&doc, "menu");
    let css = (styles[menu].width, styles[menu].padding, styles[menu].color);
    contains_run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
if(menu.contains(inside)!==true||destination.contains(inside)!==false||document.contains(inside)!==true||menu.contains(outside)!==false||inside.parentNode!==item||item.parentNode!==menu)throw 'live genuine parent graph';
if(fragment.contains(fresh)!==true||fragment.contains(text)!==true||fresh.contains(comment)!==true||document.contains(fresh)!==false||text.contains(fresh)!==false||comment.contains(comment)!==true)throw 'detached fragment topology';
if(document.getElementById('neighbor').value!=='edited neighbor'||document.getElementById('neighbor').defaultValue!=='original')throw 'native edited/default values changed';
"#,
    );
    assert_eq!(format!("{:?}", doc.nodes), nodes);
    assert_eq!(page.state.dom_commits, commits);
    assert_eq!(contains_editor(&forms, neighbor), editor);
    assert_eq!(forms.text_default_value(neighbor).unwrap(), "original");
    let after =
        crate::layout::build_viewport(&doc, 640, 480, 1.0, &HashMap::new(), &HashMap::new());
    let styles = crate::css::compute(&doc, &HashMap::new());
    assert_eq!(
        (styles[menu].width, styles[menu].padding, styles[menu].color),
        css
    );
    for id in ["menu", "inside", "outside", "neighbor"] {
        let index = contains_id(&doc, id);
        let a = before.border_rects[index].unwrap();
        let b = after.border_rects[index].unwrap();
        assert_eq!(
            (a.x, a.y, a.width, a.height, a.fixed),
            (b.x, b.y, b.width, b.height, b.fixed),
            "{id}"
        );
    }
}

#[test]
fn node_contains_detached_frame_checks_current_reachability_before_self_or_null() {
    let (mut doc, mut forms, mut page) = contains_fixture();
    contains_run(&mut doc, &mut forms, &mut page, r#"
var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);
var child=frame.contentDocument.body,method=Node.prototype.contains;
if(!method.call(child,child))throw 'connected genuine child';
document.body.removeChild(frame);
"#);
    for source in ["method.call(child,child);", "method.call(child,null);", "method.call(document,child);"] {
        let commits = page.state.dom_commits;
        let outcome = page.console_eval(&mut doc, &mut forms, source);
        assert_eq!(outcome.errors, 1, "{source}\n{}", page.report());
        assert_eq!(page.state.dom_commits, commits);
        contains_run(&mut doc, &mut forms, &mut page, "if(!method.call(document,document.body))throw 'main recovery';");
    }
}

#[test]
fn node_contains_warmed_candidate_ancestry_work_refuses_without_refill_or_native_mutation() {
    let (mut doc, mut forms, mut page) = contains_fixture();
    let menu = contains_id(&doc, "menu");
    let inside = contains_id(&doc, "inside");
    let neighbor = contains_id(&doc, "neighbor");
    forms.script_set_value(neighbor, "edited neighbor").unwrap();
    page.state.rebuild(&doc).unwrap();
    {
        let mut bridge = Bridge {
            doc: &mut doc,
            forms: &mut forms,
            state: &mut page.state,
        };
        assert!(matches!(
            bridge
                .node_contains(node_handle(menu), &[Value::Host(node_handle(inside))])
                .unwrap(),
            Value::Bool(true)
        ));
    }
    let nodes = format!("{:?}", doc.nodes);
    let commits = page.state.dom_commits;
    let editor = contains_editor(&forms, neighbor);
    let allowance = page.state.host_work;
    page.state.host_work = 1;
    {
        let mut bridge = Bridge {
            doc: &mut doc,
            forms: &mut forms,
            state: &mut page.state,
        };
        let error = bridge
            .node_contains(node_handle(menu), &[Value::Host(node_handle(inside))])
            .unwrap_err();
        assert!(error.contains("work limit"), "{error}");
    }
    assert_eq!(page.state.host_work, 0);
    assert_eq!(format!("{:?}", doc.nodes), nodes);
    assert_eq!(page.state.dom_commits, commits);
    assert_eq!(contains_editor(&forms, neighbor), editor);
    assert_eq!(forms.text_default_value(neighbor).unwrap(), "original");
    page.state.host_work = allowance;
    let mut bridge = Bridge {
        doc: &mut doc,
        forms: &mut forms,
        state: &mut page.state,
    };
    assert!(matches!(
        bridge
            .node_contains(node_handle(menu), &[Value::Host(node_handle(inside))])
            .unwrap(),
        Value::Bool(true)
    ));
}
