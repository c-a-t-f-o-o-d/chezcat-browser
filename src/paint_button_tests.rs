//! Original native button pixel, caption and clipping fixtures.
use crate::{css, font, fonts, forms, html, image, layout, paint};
use std::collections::HashMap;
const WHITE:u32=0xffffff;
const BORDER:u32=0x7c8b96;
const FOCUS:u32=0x178b78;
const CLEAR:u32=0x123456;
const TILE:&[u8]=b"P6\n2 2\n255\n\xff\x00\x00\x00\xff\x00\x00\x00\xff\xff\xff\x00";
const BASE:&str="html,body{margin:0;padding:0}button,input{display:block;margin:0;padding:0;width:24px;height:20px;color:#000;background-color:#fff;font-size:16px}#target{background-image:url(Original.ppm);background-repeat:no-repeat;background-position:3px 4px;image-rendering:pixelated}";
fn id(doc:&html::Document,name:&str)->usize{(0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some(name)).expect(name)}
fn document(markup:&str,rules:&str)->html::Document{html::parse(&format!("<!doctype html><html><head><style id=sheet>{BASE}{rules}</style></head><body>{markup}</body></html>"))}
fn page(doc:&html::Document,zoom:f32,fonts:&fonts::Book,cache:Option<&css::StylesheetCache>)->layout::Layout{
    layout::build_viewport_with_fonts_cached(doc,800,480,zoom,&HashMap::new(),&HashMap::new(),None,fonts,cache)
}
fn draw_button(canvas:&mut paint::Canvas,rect:&layout::ControlBox,y:i32,control:&forms::Control,focused:bool,bitmap:Option<&image::Bitmap>){
    paint::draw_native_button(canvas,rect,y,control,focused,bitmap);
}
fn raster(doc:&html::Document,page:&layout::Layout,name:&str,bitmap:Option<&image::Bitmap>,focused:bool,clip:Option<paint::ClipRect>)->paint::Canvas{
    let node=id(doc,name);let rect=page.controls.iter().find(|rect|rect.node==node).expect("real native control");
    let forms=forms::Forms::new(doc);let mut canvas=paint::Canvas::new(160,120);canvas.pixels.fill(CLEAR);
    let item=page.items.iter().position(|item|matches!(item,layout::PaintItem::Control(i)if page.controls[*i].node==node)).expect("one native paint item");
    let actual=page.paint_clip(item);canvas.set_clip(match(actual,clip){(Some(a),Some(b))=>Some(a.intersect(b)),(a,None)|(None,a)=>a});
    draw_button(&mut canvas,rect,rect.y,&forms.controls[&node],focused,bitmap);canvas
}
fn pixel(canvas:&paint::Canvas,x:i32,y:i32)->u32{canvas.pixels[y as usize*canvas.width+x as usize]}
fn exact(canvas:&paint::Canvas,rect:[i32;4],background:impl Fn(i32,i32)->u32,focused:bool,clip:Option<[i32;4]>){
    let [left,top,width,height]=rect;
    for y in 0..canvas.height as i32{for x in 0..canvas.width as i32{
        let shown=clip.map_or(true,|[l,t,r,b]|x>=l&&x<r&&y>=t&&y<b);
        let (dx,dy)=(x-left,y-top);
        let expected=if !shown||dx<0||dy<0||dx>=width||dy>=height{CLEAR}
            else if dx==0||dy==0||dx==width-1||dy==height-1||focused&&dy>=height-2 {if focused{FOCUS}else{BORDER}}
            else{background(dx,dy)};
        assert_eq!(pixel(canvas,x,y),expected,"pixel ({x},{y})");
    }}
}
fn tile()->image::Bitmap{let b=image::decode(TILE).unwrap();assert_eq!((b.width,b.height),(2,2));b}
#[test]
fn icon_pixels_and_native_edges(){
    let doc=document("<button id=target type=submit></button>","");let page=page(&doc,1.0,&fonts::Book::empty(),None);let bitmap=tile();
    let canvas=raster(&doc,&page,"target",Some(&bitmap),false,None);
    exact(&canvas,[0,0,24,20],|x,y|match(x,y){(4,5)=>0xff0000,(5,5)=>0x00ff00,(4,6)=>0x0000ff,(5,6)=>0xffff00,_=>WHITE},false,None);
    let node=id(&doc,"target");let sources:Vec<_>=page.background_sources().filter(|(n,_)|*n==node).collect();assert_eq!(sources.len(),1);
    assert_eq!(sources[0].1.url,"Original.ppm");assert_eq!(sources[0].1.stylesheet,Some(id(&doc,"sheet")));
    for (x,y)in[(0,0),(12,10),(23,19)]{assert_eq!(page.control_at(x,y),Some(node));}assert_eq!(page.control_at(24,19),None);
}
#[test]
fn sizes_repeats_and_zoom(){
    let bitmap=tile();
    let doc=document("<button id=target></button>","#target{width:10px;height:10px;background-size:contain;background-position:center}");let p=page(&doc,1.0,&fonts::Book::empty(),None);
    let canvas=raster(&doc,&p,"target",Some(&bitmap),false,None);
    exact(&canvas,[0,0,10,10],|x,y|match(x<=4,y<=4){(true,true)=>0xff0000,(false,true)=>0x00ff00,(true,false)=>0x0000ff,(false,false)=>0xffff00},false,None);
    let wide=image::decode(b"P6\n4 2\n255\n\xff\x00\x00\xff\x00\x00\x00\xff\x00\x00\xff\x00\x00\x00\xff\x00\x00\xff\xff\xff\x00\xff\xff\x00").unwrap();
    for size in ["contain","cover"]{
        let doc=document("<button id=target></button>",&format!("#target{{width:10px;height:10px;background-size:{size};background-position:center}}"));let p=page(&doc,1.0,&fonts::Book::empty(),None);
        let canvas=raster(&doc,&p,"target",Some(&wide),false,None);
        exact(&canvas,[0,0,10,10],|x,y|if size=="contain"&&!(3..7).contains(&y){WHITE}else{match(x<=4,y<=4){(true,true)=>0xff0000,(false,true)=>0x00ff00,(true,false)=>0x0000ff,(false,false)=>0xffff00}},false,None);
    }
    for repeat in ["repeat-x","repeat-y","repeat"]{
        let doc=document("<button id=target></button>",&format!("#target{{width:10px;height:10px;background-repeat:{repeat};background-position:right 2px bottom 3px}}"));let p=page(&doc,1.0,&fonts::Book::empty(),None);
        let canvas=raster(&doc,&p,"target",Some(&bitmap),false,None);
        exact(&canvas,[0,0,10,10],|x,y|{
            if repeat=="repeat-x"&&!(4..6).contains(&y)||repeat=="repeat-y"&&!(5..7).contains(&x){return WHITE;}
            match((x-5).rem_euclid(2),(y-4).rem_euclid(2)){(0,0)=>0xff0000,(1,0)=>0x00ff00,(0,1)=>0x0000ff,_=>0xffff00}
        },false,None);
    }
    let doc=document("<button id=target></button>","#target{width:10px;height:10px}");let p=page(&doc,2.0,&fonts::Book::empty(),None);
    let canvas=raster(&doc,&p,"target",Some(&bitmap),false,None);
    exact(&canvas,[0,0,20,20],|x,y|if (7..11).contains(&x)&&(9..13).contains(&y){match(x<9,y<11){(true,true)=>0xff0000,(false,true)=>0x00ff00,(true,false)=>0x0000ff,_=>0xffff00}}else{WHITE},false,None);
}
#[test]
fn alpha_focus_and_disabled(){
    let bitmap=image::Bitmap{width:1,height:1,pixels:vec![0x80ff0000]};
    for focus in [false,true]{
        let doc=document("<button id=target></button>","#target{background-color:#0000ff;background-repeat:repeat;background-position:0 0}");let p=page(&doc,1.0,&fonts::Book::empty(),None);
        let canvas=raster(&doc,&p,"target",Some(&bitmap),focus,None);exact(&canvas,[0,0,24,20],|_,_|0x80007f,focus,None);
    }
    let green=image::Bitmap{width:1,height:1,pixels:vec![0xff00ff00]};
    let doc=document("<button id=target disabled></button>","#target{background-repeat:repeat;background-position:0 0}");let p=page(&doc,1.0,&fonts::Book::empty(),None);
    let canvas=raster(&doc,&p,"target",Some(&green),false,None);exact(&canvas,[0,0,24,20],|_,_|0x00ff00,false,None);
}
#[test]
fn font_and_bitmap_caption_foreground() {
    let font = std::sync::Arc::new(font::Font::parse(include_bytes!("../pages/fonts/chezcat-geometry.ttf")).unwrap());
    let mut book = fonts::Book::empty();
    book.add_with_ranges("Original".into(), 400, false, font.clone(), Vec::new()).unwrap();
    let empty = fonts::Book::empty();
    let green = image::Bitmap { width: 1, height: 1, pixels: vec![0xff00ff00] };
    // This original font's AV fixture is 20x19, centered at (33,13) with
    // ascent15. Inspect its actual glyph coverage independently of the button
    // painter: a source-colored pixel over one backdrop need not be opaque.
    let a = font.glyph_index('A');
    let v = font.glyph_index('V');
    let second = font.advance(a, 16.0) + font.kerning(a, v, 16.0);
    assert_eq!((second + font.advance(v, 16.0)).round() as i32, 20);
    let metrics = font.metrics(16.0);
    assert_eq!((metrics.ascent.ceil() as i32, metrics.descent.ceil() as i32), (15, 4));
    let mut opaque = Vec::new();
    for (glyph, offset) in [(a, 0), (v, second.round() as i32)] {
        let bitmap = font.rasterize(glyph, 16.0).unwrap();
        for row in 0..bitmap.height {
            for col in 0..bitmap.width {
                let point = (33 + offset + bitmap.left + col as i32, 13 + 15 + bitmap.top + row as i32);
                let coverage = bitmap.coverage[row * bitmap.width + col];
                if coverage == 255 { opaque.push(point); }
                if point == (39, 20) { assert_eq!(coverage, 254); }
            }
        }
    }
    assert!(opaque.len() > 4);
    for use_font in [false, true] {
        for disabled in [false, true] {
            let doc = document(&format!("<button id=target {}>AV</button>", if disabled { "disabled" } else { "" }), "#target{width:80px;height:40px;padding:3px;font-family:Original;background-repeat:repeat;background-position:0 0}");
            let p = page(&doc, 1.0, if use_font { &book } else { &empty }, None);
            let node = id(&doc, "target");
            let rect = p.controls.iter().find(|r| r.node == node).unwrap();
            assert_eq!(rect.text_font.is_some(), use_font);
            let plain = raster(&doc, &p, "target", None, false, None);
            let actual = raster(&doc, &p, "target", Some(&green), false, None);
            let color = if disabled { 0x78858e } else { 0 };
            let mut foreground = 0;
            for y in 1..rect.height - 1 {
                for x in 1..rect.width - 1 {
                    let solid = if use_font { opaque.contains(&(x, y)) } else { pixel(&plain, x, y) == color };
                    if solid {
                        foreground += 1;
                        assert_eq!(pixel(&actual, x, y), color, "caption foreground ({x},{y})");
                    }
                }
            }
            assert!(foreground > 4, "real caption pixels");
            if use_font {
                let blended: u32 = [0, 8, 16].into_iter().map(|shift| ((((color >> shift) & 255) * 254 + ((0x00ff00u32 >> shift) & 255) + 127) / 255) << shift).sum();
                assert_eq!(pixel(&actual, 39, 20), blended, "original glyph coverage254 over green");
            }
            assert_eq!(pixel(&actual, 2, 2), 0x00ff00);
            assert_eq!(pixel(&actual, 0, 0), BORDER);
        }
    }
}
#[test]
fn clipped_moved_and_scrolled(){
    let bitmap=image::Bitmap{width:1,height:1,pixels:vec![0xff00ff00]};
    let doc=document("<div style='width:20px;height:18px;overflow:hidden;transform:translate(10px,12px)'><button id=target style='position:relative;left:3px;top:4px'></button></div>","#target{background-repeat:repeat;background-position:0 0}");let p=page(&doc,1.0,&fonts::Book::empty(),None);
    let canvas=raster(&doc,&p,"target",Some(&bitmap),false,None);exact(&canvas,[13,16,24,20],|_,_|0x00ff00,false,Some([10,12,30,30]));
    let doc=document("<button id=target style='position:fixed;left:15px;top:20px'></button>","#target{background-repeat:repeat;background-position:0 0}");let mut p=page(&doc,1.0,&fonts::Book::empty(),None);
    p.set_scroll(25);let canvas=raster(&doc,&p,"target",Some(&bitmap),false,None);exact(&canvas,[15,45,24,20],|_,_|0x00ff00,false,None);
    p.set_scroll(0);let canvas=raster(&doc,&p,"target",Some(&bitmap),false,None);exact(&canvas,[15,20,24,20],|_,_|0x00ff00,false,None);
}

#[test]
fn empty_button_boxes_leave_pixels_and_clip_unchanged() {
    let doc = document("<button id=target>Caption</button>", "");
    let page = page(&doc, 1.0, &fonts::Book::empty(), None);
    let node = id(&doc, "target");
    let rect = page.controls.iter().find(|rect| rect.node == node).unwrap();
    let forms = forms::Forms::new(&doc);
    let bitmap = tile();
    for (width, height) in [(0, 20), (24, 0), (0, 0), (-1, 20), (24, -1)] {
        let mut empty = rect.clone();
        empty.x = 10;
        empty.y = 10;
        empty.width = width;
        empty.height = height;
        for focused in [false, true] {
            let mut canvas = paint::Canvas::new(80, 60);
            canvas.pixels.fill(CLEAR);
            let clip = paint::ClipRect::new(5, 5, 60, 40);
            canvas.set_clip(Some(clip));
            draw_button(&mut canvas, &empty, empty.y, &forms.controls[&node], focused, Some(&bitmap));
            assert!(canvas.pixels.iter().all(|pixel| *pixel == CLEAR), "empty {width}x{height}");
            assert_eq!(canvas.clip(), clip);
        }
    }
}
