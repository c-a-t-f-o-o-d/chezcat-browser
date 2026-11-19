use super::*;

#[test]
fn autonomous_element_identity_trusted_inherited_method_updates_real_nodes() {
    let files = Fixture::new();
    let path = files.write("unregistered-elements.html", r#"<!doctype html><html><head>
<title>Ready</title><style>body{margin:0}button{height:32px;width:130px}notice-line,output-line,panel-line{display:block}</style>
</head><body><button type=button id=apply>Update notices</button><section id=items><notice-line id=first>Pending</notice-line></section><script>
var list=document.getElementById('items'),first=document.getElementById('first');
var button=document.getElementById('apply');
button.onclick=function(event){
    if(!event.isTrusted||event.target!==button||this!==button)throw 'trusted input';
    document.title='Updating';
    var created=document.createElement('output-line');
    var ns=document.createElementNS('http://www.w3.org/1999/xhtml','panel-line');
    for(var element of [first,created,ns]){
        if(Object.getPrototypeOf(element)!==HTMLElement.prototype||element.constructor!==HTMLElement||
           !(element instanceof HTMLElement)||!(element instanceof Element)||element.ownerDocument!==document)
            throw 'unregistered HTML element prototype or owner';
    }
    if(created.isConnected||ns.isConnected||!first.isConnected)throw 'initial connection';
    HTMLElement.prototype.displayNotice=function(text){
        this.noticeLabel=text;
        if(this.firstChild)this.firstChild.nodeValue=text;
        else this.appendChild(document.createTextNode(text));
        return this;
    };
    if(first.displayNotice('Awake')!==first||created.displayNotice('Fresh')!==created||
       ns.displayNotice('Namespaced')!==ns)throw 'inherited method receiver';
    if(list.appendChild(created)!==created||list.appendChild(ns)!==ns||
       list.firstChild!==first||created.previousSibling!==first||list.lastChild!==ns||
       !created.isConnected||!ns.isConnected||created.noticeLabel!=='Fresh'||
       document.getElementById('first')!==first)throw 'node identity after insertion';
    document.title='Updated';
};
</script></body></html>"#);
    let mut app = App::new();
    app.resize(480, 300);
    app.set_javascript_enabled(true);
    app.open(&path);
    app.wait_for_load();
    assert_eq!(app.document.title, "Ready");
    assert!(app.page.runs.iter().any(|run| run.text == "Pending"));
    let button = app.page.controls.iter().find(|control| {
        app.document.attr(control.node, "id") == Some("apply")
    }).unwrap();
    let x = button.x + button.width / 2;
    let y = button.y + button.height / 2;
    assert_eq!(app.page.control_at(x,y), Some(button.node));
    let screen_y = app.content_top() + y - app.scroll;
    assert!(matches!(app.pointer_down(x,screen_y,false), PlatformAction::None));
    assert!(matches!(app.pointer_up(x,screen_y), PlatformAction::None));
    let script = app.scripting.as_ref().unwrap();
    assert_eq!(script.error_count(), 0, "{}", script.report());
    assert_eq!(app.document.title, "Updated");
    for text in ["Awake","Fresh","Namespaced"] {
        assert!(app.page.runs.iter().any(|run| run.text == text), "Missing {text}");
    }
    assert!(app.page.runs.iter().all(|run| run.text != "Pending"));
}
