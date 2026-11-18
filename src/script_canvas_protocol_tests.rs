//! Canvas reflection, ordinary descriptors and finite context-none geometry.
use super::*;

fn id(doc: &Document, name: &str) -> usize {
    (0..doc.nodes.len()).find(|&node| doc.attr(node, "id") == Some(name)).unwrap()
}
fn location(url: &str) -> crate::navigation::Location {
    crate::navigation::address(url, &crate::navigation::Location::home()).unwrap()
}
fn fixture() -> (Document, Forms, Page) {
    let mut doc = crate::html::parse(concat!(
        "<html><head><style>body{margin:0}canvas{position:absolute;left:10px;top:20px}",
        "#scaled{left:350px;width:60px}#other{position:absolute;left:500px;top:200px}</style></head>",
        "<body><form id=f><input id=neighbor name=keep value=seed></form>",
        "<canvas id=canvas width=20 height=10 title=raw><span>fallback</span></canvas>",
        "<canvas id=scaled width=20 height=10></canvas><div id=other></div></body></html>"
    ));
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::for_test();
    page.set_location(&location("https://canvas-dimension.test/original"));
    page.set_resource_base(&location("https://canvas-dimension.test/original"));
    page.set_geometry(crate::script_geometry::Geometry::new(
        800, 600, 1.0, &HashMap::new(), HashMap::new(), &crate::fonts::Book::empty(),
    ));
    assert_eq!(page.start(&mut doc, &mut forms, &[]).errors, 0, "{}", page.report());
    run(&mut doc, &mut forms, &mut page, r#"
        var canvas=document.getElementById('canvas'), neighbor=document.getElementById('neighbor');
        var proto=HTMLCanvasElement.prototype;
        var widthDescriptor=Object.getOwnPropertyDescriptor(proto,'width');
        var heightDescriptor=Object.getOwnPropertyDescriptor(proto,'height');
        var getWidth=widthDescriptor.get, setWidth=widthDescriptor.set;
        neighbor.value='edited neighbor';
    "#);
    let value = &mut forms.controls.get_mut(&id(&doc, "neighbor")).unwrap().value;
    value.cursor = 2;
    value.anchor = 5;
    value.offset = 1;
    (doc, forms, page)
}
fn run(doc: &mut Document, forms: &mut Forms, page: &mut Page, source: &str) {
    assert_eq!(page.console_eval(doc, forms, source).errors, 0, "{source}\n{}", page.report());
    assert!(!page.has_navigation_request());
    assert!(page.take_dynamic_scripts().is_empty());
    assert!(page.take_dynamic_stylesheets().is_empty());
    let (added, removed, changed) = page.take_image_changes();
    assert!(added.is_empty() && removed.is_empty() && changed.is_empty());
}
fn kept(doc: &Document, forms: &Forms) {
    let control = &forms.controls[&id(doc, "neighbor")];
    assert_eq!(
        (control.value.text(),control.value.cursor,control.value.anchor,control.value.offset,control.dirty_value,control.owner),
        ("edited neighbor".into(),2,5,1,true,Some(id(doc,"f"))),
    );
    assert_eq!(forms.text_default_value(id(doc,"neighbor")).unwrap(), "seed");
    assert_eq!(forms.controls.len(), 1);
    assert_eq!(doc.attr(id(doc,"canvas"),"title"), Some("raw"));
}

#[test]
fn canvas_dimensions_numeric_conversion_raw_attribute_identity_and_namespace() {
    let (mut doc, mut forms, mut page) = fixture();
    run(&mut doc, &mut forms, &mut page, r#"
        var attr=canvas.getAttributeNode('width');
        var values=[null,undefined,true,false,NaN,Infinity,-Infinity,1.9,-1,2147483648,4294967297];
        var expected=[0,0,1,0,0,0,0,1,300,300,1];
        for(var i=0;i<values.length;i++){
            canvas.width=values[i];
            if(canvas.width!==expected[i]||attr.value!==String(expected[i])||canvas.getAttributeNode('width')!==attr)
                throw 'unsigned long conversion/reflection';
        }
        canvas.height=-1;
        if(canvas.height!==150||canvas.getAttribute('height')!=='150')throw 'height fallback';
        for(var pair of [['  +0020px',20],['-0',0],['0x20',0],['1.9',1],['-1',300],['',300],
                         ['2147483647',2147483647],['2147483648',300],['99999999999999999999999',300]]){
            canvas.setAttribute('WIDTH',pair[0]);
            if(canvas.width!==pair[1]||attr.value!==pair[0])throw 'raw integer parser';
        }
        canvas.setAttributeNS('urn:canvas-metadata','m:width','73');
        canvas.setAttributeNS(null,'WIDTH','42');
        if(canvas.width!==300||canvas.getAttributeNS(null,'WIDTH')!=='42'||
           canvas.getAttributeNS('urn:canvas-metadata','width')!=='73')throw 'namespace lookalikes';
        attr.value='31 suffix';
        if(canvas.width!==31||attr.ownerElement!==canvas)throw 'live attached Attr';
        canvas.removeAttributeNode(attr);
        if(canvas.width!==300||attr.ownerElement!==null||attr.value!=='31 suffix')throw 'Attr removal';
        if(canvas.setAttributeNode(attr)!==null||canvas.width!==31)throw 'Attr restoration';
        canvas.removeAttributeNS(null,'width');
        if(canvas.width!==300||attr.ownerElement!==null)throw 'expanded-name removal';
    "#);
    kept(&doc, &forms);
}

#[test]
fn canvas_dimensions_descriptors_brand_order_shadows_and_no_context_claim() {
    let (mut doc, mut forms, mut page) = fixture();
    run(&mut doc, &mut forms, &mut page, r#"
        if(Object.getPrototypeOf(canvas)!==proto||!(canvas instanceof HTMLCanvasElement)||
           !(canvas instanceof HTMLElement)||Object.prototype.toString.call(canvas)!=='[object HTMLCanvasElement]'||
           !widthDescriptor.enumerable||!widthDescriptor.configurable||getWidth.name!=='get width'||
           getWidth.length!==0||setWidth.name!=='set width'||setWidth.length!==1||
           getWidth===heightDescriptor.get||Object.getOwnPropertyDescriptor(canvas,'width')!==undefined||
           !('width' in canvas))throw 'canonical descriptor';
        var count=0,marker={},conversion={valueOf:function(){count++;return 42;},toString:function(){throw marker;}};
        setWidth.call(canvas,conversion);
        if(count!==1||canvas.width!==42)throw 'numeric conversion once';
        var failures=0;
        for(var receiver of [null,undefined,{},proto,document,neighbor,document.getElementById('other')]){
            try{setWidth.call(receiver,conversion);}catch(e){if(e instanceof TypeError)failures++;}
            try{getWidth.call(receiver);}catch(e){if(e instanceof TypeError)failures++;}
        }
        if(failures!==14||count!==1)throw 'brand precedes author conversion';
        try{canvas.width=Symbol('width');throw 'Symbol accepted';}catch(e){if(!(e instanceof TypeError))throw e;}
        try{canvas.width={valueOf:function(){canvas.height=3;throw marker;}};throw 'throw lost';}
        catch(e){if(e!==marker)throw e;}
        if(canvas.width!==42||canvas.height!==3)throw 'abrupt conversion committed outer write';
        try{new HTMLCanvasElement();throw 'constructor allocated';}catch(e){if(!(e instanceof TypeError))throw e;}
        try{new getWidth();throw 'getter constructible';}catch(e){if(!(e instanceof TypeError))throw e;}
        var raw={},writes=0;
        Object.defineProperty(canvas,'width',{configurable:true,get:function(){return getWidth.call(this);},
            set:function(v){writes++;setWidth.call(this,v);}});
        canvas.width=12;
        if(writes!==1||canvas.width!==12)throw 'own forwarding accessor';
        delete canvas.width;
        Object.defineProperty(proto,'width',{configurable:true,get:function(){return 'author';},
            set:function(v){if(this!==canvas||v!==raw)throw 'raw setter';writes++;}});
        canvas.width=raw;
        if(writes!==2||canvas.width!=='author'||getWidth.call(canvas)!==12)throw 'prototype override';
        delete proto.width;
        if(canvas.width!==undefined||'width' in canvas)throw 'ghost native fallback';
        canvas.width=raw;
        if(canvas.width!==raw||getWidth.call(canvas)!==12)throw 'ordinary own property after deletion';
        delete canvas.width;Object.defineProperty(proto,'width',widthDescriptor);
        if(canvas.width!==12)throw 'canonical restoration';
    "#);
    let before = format!("{:?}", doc.nodes);
    let outcome = page.console_eval(&mut doc, &mut forms, "canvas.getContext('2d');");
    assert_eq!(outcome.errors, 1, "{}", page.report());
    assert_eq!(format!("{:?}", doc.nodes), before);
    kept(&doc, &forms);
}

#[test]
fn canvas_dimensions_live_geometry_css_scale_zero_detachment_and_finite_bounds() {
    let (mut doc, mut forms, mut page) = fixture();
    run(&mut doc, &mut forms, &mut page, r#"
        var scaled=document.getElementById('scaled'),rect=canvas.getBoundingClientRect();
        if(rect.x!==10||rect.y!==20||rect.width!==20||rect.height!==10||
           scaled.getBoundingClientRect().width!==60||scaled.getBoundingClientRect().height!==30)
            throw 'natural canvas box and CSS scaling';
        canvas.width=30;canvas.height=15;scaled.width=30;
        if(canvas.getBoundingClientRect().width!==30||canvas.getBoundingClientRect().height!==15||
           scaled.getBoundingClientRect().width!==60||scaled.getBoundingClientRect().height!==20)
            throw 'live dimension invalidation';
        scaled.setAttribute('width','100000');scaled.setAttribute('height','200000');
        if(scaled.getBoundingClientRect().width!==60||scaled.getBoundingClientRect().height!==120)
            throw 'raw natural ratio precedes the layout cap';
        scaled.setAttribute('width','1000000000000000000000000000000000000000');
        scaled.setAttribute('height','2000000000000000000000000000000000000000');
        if(scaled.width!==300||scaled.height!==150||scaled.getBoundingClientRect().width!==60||
           scaled.getBoundingClientRect().height!==120)throw 'bounded decimal ratio and reflected defaults';
        scaled.setAttribute('style','position:absolute;left:350px;top:20px;width:auto;height:20px');
        if(scaled.getBoundingClientRect().width!==10||scaled.getBoundingClientRect().height!==20)
            throw 'explicit height scales the original natural ratio';
        scaled.setAttribute('width','200000000');scaled.setAttribute('height','100000');
        // The established parser caps each numeric token at 10000. A finite
        // admitted em value resolves to 100000px without changing that policy.
        scaled.setAttribute('style','position:absolute;left:350px;top:20px;font-size:16px;width:6250em;height:auto');
        var capped=scaled.getBoundingClientRect();
        if(capped.width!==32768||capped.height!==50)
            throw 'final coordinate cap does not replace CSS width: '+capped.width+'x'+capped.height;
        scaled.setAttribute('style','position:absolute;left:350px;top:20px;width:60px;height:20px;box-sizing:border-box;padding:4px;border:2px solid black');
        if(scaled.getBoundingClientRect().width!==60||scaled.getBoundingClientRect().height!==20)
            throw 'CSS border-box sizing remains independent of bitmap dimensions';
        canvas.width=0;
        if(canvas.getBoundingClientRect().width!==0||canvas.getBoundingClientRect().height!==15)
            throw 'zero dimension remains a real box';
        canvas.removeAttribute('width');canvas.removeAttribute('height');
        if(canvas.width!==300||canvas.height!==150||canvas.getBoundingClientRect().width!==300||
           canvas.getBoundingClientRect().height!==150)throw 'removed defaults update geometry';
        canvas.setAttribute('width','999999999999999999999999999999999999');
        if(canvas.width!==300||canvas.getBoundingClientRect().width!==32768)throw 'bounded raw oversized natural width';
        var parent=canvas.parentNode;parent.removeChild(canvas);
        if(canvas.width!==300||canvas.height!==150||canvas.getBoundingClientRect().width!==0||
           canvas.ownerDocument!==document||canvas.parentNode!==null)throw 'detached reflection';
        parent.appendChild(canvas);canvas.width=9;
        if(canvas.getBoundingClientRect().width!==9||canvas.getBoundingClientRect().height!==150)
            throw 'reattached actual canvas';
    "#);
    kept(&doc, &forms);
}

#[test]
fn canvas_dimensions_cached_child_accessors_preserve_realm_and_origin_checks() {
    let (mut doc, mut forms, mut page) = fixture();
    run(&mut doc, &mut forms, &mut page, r#"
        var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);
        var w=frame.contentWindow,ChildError=w.TypeError;
        var childDescriptor=Object.getOwnPropertyDescriptor(w.HTMLCanvasElement.prototype,'width');
        var childGet=childDescriptor.get,childSet=childDescriptor.set;
        if(childGet===getWidth||w.HTMLCanvasElement===HTMLCanvasElement||
           Object.getPrototypeOf(childGet)!==w.Function.prototype||childGet.call(canvas)!==20)
            throw 'owner realm and borrowed receiver';
        var childError=false;
        try{childGet.call(w.document.body);}catch(e){childError=Object.getPrototypeOf(e)===ChildError.prototype;}
        if(!childError)throw 'brand error realm';
        var conversions=0,conversion={valueOf:function(){conversions++;return 27;}};
        childSet.call(canvas,24);
    "#);
    let before = format!("{:?}", doc.nodes);
    page.set_location(&location("https://other-canvas.test/changed"));
    for source in ["childGet.call(canvas);", "childSet.call(canvas,conversion);"] {
        assert_eq!(page.console_eval(&mut doc, &mut forms, source).errors, 1, "{}", page.report());
        assert!(page.report().contains("same-origin"), "{}", page.report());
        assert_eq!(format!("{:?}", doc.nodes), before);
    }
    page.set_location(&location("https://canvas-dimension.test/original"));
    run(&mut doc, &mut forms, &mut page, r#"
        if(conversions!==0||childGet.call(canvas)!==24)throw 'origin refusal before conversion';
        childSet.call(canvas,conversion);
        if(conversions!==1||canvas.width!==27)throw 'caller realm restored';
    "#);
    kept(&doc, &forms);
}

#[test]
fn canvas_dimensions_attribute_failure_is_atomic_without_native_or_bitmap_allocation() {
    for mode in ["work", "copy", "retained"] {
        let (mut doc, mut forms, mut page) = fixture();
        let before = format!("{:?}", doc.nodes);
        let retained = page.state.retained;
        match mode {
            "work" => page.state.host_work = 0,
            "copy" => page.state.copy_work = 0,
            _ => page.state.retained_limit = retained,
        }
        let node = id(&doc, "canvas");
        let result = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state }
            .canvas_dimension_set(node_handle(node), "width", &Value::Number(1000.0));
        assert!(result.is_err(), "{mode}");
        assert_eq!(format!("{:?}", doc.nodes), before, "{mode}");
        assert_eq!(page.state.retained, retained, "{mode}");
        kept(&doc, &forms);
    }
}

#[test]
fn canvas_dimensions_forced_layout_preflights_new_parse_and_storage_work() {
    let mut doc = crate::html::parse("<html><body><canvas id=c width='00000000000000000000000000020' height=10></canvas></body></html>");
    doc.scripting_enabled = true;
    let forms = Forms::new(&doc);
    let node = id(&doc, "c");
    let mut geometry = crate::script_geometry::Geometry::new(
        640,480,1.0,&HashMap::new(),HashMap::new(),&crate::fonts::Book::empty(),
    ).with_limits(crate::script_geometry::Limits { layouts_per_job: 1 });
    // The pre-existing no-canvas rebuild debit is insufficient for the newly
    // added natural-dimension parser/cache. A refusal must leave its rebuild.
    let mut work = doc.nodes.len() + crate::layout::offset_parent_work(&doc);
    let mut copies = usize::MAX;
    assert!(geometry.bounding_rect(&doc,&forms,node,&mut work,&mut copies).is_err());
    let (mut work,mut copies) = (usize::MAX,usize::MAX);
    assert_eq!(geometry.bounding_rect(&doc,&forms,node,&mut work,&mut copies).unwrap()[2..], [20.0,10.0]);
}

#[test]
fn canvas_dimensions_layout_uses_replaced_box_or_disabled_script_fallback() {
    let mut doc = crate::html::parse("<html><body><canvas id=c width=20 height=10>OriginalFallback</canvas></body></html>");
    let node = id(&doc, "c");
    for enabled in [false, true] {
        doc.scripting_enabled = enabled;
        let layout = crate::layout::build_viewport(&doc,640,480,1.0,&HashMap::new(),&HashMap::new());
        assert_eq!(layout.runs.iter().any(|run| run.text.contains("OriginalFallback")), !enabled);
        let image = layout.items.iter().find_map(|item| match item {
            crate::layout::PaintItem::Image { node: owner,width,height,.. } if *owner == node => Some((*width,*height)),
            _ => None,
        });
        assert_eq!(image, enabled.then_some((20,10)));
        assert!(layout.controls.is_empty());
    }
}
