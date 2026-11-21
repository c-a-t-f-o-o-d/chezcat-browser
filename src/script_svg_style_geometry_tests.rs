//! SVG CSSInlineStyle prototype/forwarding, brands and resource policy.
use super::*;
#[test]
fn svg_geometry_style_native_prototype_forwarding_shadows_brands_and_resource_guards_are_real() {
    let mut doc =
        crate::html::parse("<html><body><svg id=icon></svg><div id=html></div></body></html>");
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::new();
    assert_eq!(page.start(&mut doc, &mut forms, &[]).errors, 0);
    let result=page.console_eval(&mut doc,&mut forms,r#"
var svg=document.getElementById('icon'),html=document.getElementById('html'),d=Object.getOwnPropertyDescriptor(SVGElement.prototype,'style'),h=Object.getOwnPropertyDescriptor(HTMLElement.prototype,'style'),calls=0;
if(!d||!d.enumerable||!d.configurable||d.get===h.get||d.get.name!=='get style'||d.set.name!=='set style'||d.get.length!==0||d.set.length!==1)throw 'own SVG inline style interface';var native=svg.style;if(d.get.call(svg)!==native||native!==svg.style)throw 'real same style object';
d.set.call(svg,{toString:function(){calls++;return 'width:20px;height:10px';}});if(calls!==1||native.width!=='20px'||native.height!=='10px')throw 'genuine forwarding';
for(var bad of [html,document,{},Object.create(SVGElement.prototype),document.createTextNode('x')]){var caught=false;try{d.set.call(bad,{toString:function(){calls++;return 'width:50px';}});}catch(e){caught=e instanceof TypeError;}if(!caught||calls!==1)throw 'SVG brand before conversion';}
var caught=false;try{h.get.call(svg);}catch(e){caught=e instanceof TypeError;}if(!caught)throw 'HTML accessor keeps HTML brand';
Object.defineProperty(svg,'style',{value:'author',writable:true,configurable:true});d.set.call(svg,'width:30px');if(svg.style!=='author'||d.get.call(svg)!==native||native.width!=='30px')throw 'native declaration independent of author override';delete svg.style;if(svg.style!==native)throw 'restored canonical getter';
"#);
    assert_eq!(result.errors, 0, "{}", page.report());
    let before = doc
        .attr(
            (0..doc.nodes.len())
                .find(|&n| doc.attr(n, "id") == Some("icon"))
                .unwrap(),
            "style",
        )
        .unwrap()
        .to_owned();
    let denied = page.console_eval(
        &mut doc,
        &mut forms,
        "svg.setAttribute('fill','url(https://unrequested.invalid/image)');",
    );
    assert_eq!(denied.errors, 1, "{}", page.report());
    assert!(page.report().contains("passive"), "{}", page.report());
    assert_eq!(
        page.console_eval(
            &mut doc,
            &mut forms,
            "if(svg.style.width!=='30px')throw 'refusal changed style';"
        )
        .errors,
        0,
        "{}",
        page.report()
    );
    assert_eq!(
        doc.attr(
            (0..doc.nodes.len())
                .find(|&n| doc.attr(n, "id") == Some("icon"))
                .unwrap(),
            "style"
        ),
        Some(before.as_str())
    );
}
