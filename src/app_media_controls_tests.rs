//! Actual owned-App controls/decoder witnesses. No OS input or audio device.
use super::*;

fn open(name:&str)->App {
    let mut app=App::new();app.scripts_enabled=true;app.images_enabled=false;
    app.open(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/fixtures").join(name));
    app.wait_for_load();assert!(!app.failed,"{}",app.status);
    assert_eq!(app.scripting.as_ref().unwrap().error_count(),0,"{}",app.scripting.as_ref().unwrap().report());
    app
}
fn eval(app:&mut App,source:&str) {
    let page=app.scripting.as_mut().unwrap();
    let out=page.console_eval(&mut app.document,&mut app.forms,source);
    assert_eq!(out.errors,0,"{source}\n{}",page.report());
    if out.layout_changed {app.reflow();}
}
fn settle(app:&mut App) {
    let deadline=Instant::now()+Duration::from_secs(5);
    while app.media.as_ref().is_some_and(|p|p.pending.is_some()) {
        assert!(Instant::now()<deadline,"real decoder completion timed out");
        app.poll_load();std::thread::sleep(Duration::from_millis(1));
    }
    app.flush_embedded_video();
}
fn reference(app:&App,index:usize) {
    let bitmap=&app.images[&app.media.as_ref().unwrap().node];
    let expected=&include_bytes!("../tools/fixtures/scene-intra.rgba")[index*64*48*4..(index+1)*64*48*4];
    assert_eq!((bitmap.width,bitmap.height,bitmap.pixels.len()),(64,48,64*48));
    let(mut error,mut maximum)=(0u64,0u32);
    for(pixel,rgba)in bitmap.pixels.iter().zip(expected.chunks_exact(4)) {
        for(actual,expected)in [((pixel>>16)&255,rgba[0]),((pixel>>8)&255,rgba[1]),(pixel&255,rgba[2])] {
            let delta=actual.abs_diff(u32::from(expected));error+=u64::from(delta);maximum=maximum.max(delta);
        }
        assert_eq!(pixel>>24,255);
    }
    // Reuse the unchanged original fixture's independently decoded reference
    // and established differing-chroma-upsampling bounds.
    assert!(error<64*48*3*4&&maximum<=80,"frame {index}: error={error}, max={maximum}");
}

#[test]
fn authored_video_controls_reflection_changes_actual_bar_and_releases_only_native_capture() {
    let mut app=open("authored-video-no-controls.html");
    let player=app.media.as_ref().unwrap();let node=player.node;let clip=player.clip.clone();
    let generation=player.generation;let pixels=app.images[&node].clone();
    let source=app.embedded_video.as_ref().unwrap().retired.clone();
    assert!(app.media_control_rect().is_none());assert!(app.input_media_action("Play").is_err());
    eval(&mut app,"var v=document.getElementById('authored-video'),c=Object.getOwnPropertyDescriptor(HTMLMediaElement.prototype,'controls'),coerced=0,arg={toString:function(){coerced++;throw 'Boolean coercion';},valueOf:function(){coerced++;throw 'Boolean coercion';}};v.controls=arg;if(!v.controls||coerced)throw 'actual reflected presence';");
    let bar=app.media_control_rect().unwrap();let(x,y)=(bar.left+12,bar.top+6);
    assert!(app.media_hit(x,y).is_some());
    assert_eq!(app.draw().pixels[y as usize*app.width as usize+x as usize],0x346277,"new controls must be actual painted button pixels");
    assert!(app.media_pointer_down(bar.left+40,bar.top+16));
    assert!(app.media.as_ref().unwrap().pressed.is_some());assert!(app.media.as_ref().unwrap().controls_focused);
    eval(&mut app,"Object.defineProperty(v,'controls',{configurable:true,value:'own-shadow'});c.set.call(v,false);if(v.controls!=='own-shadow'||c.get.call(v)||v.hasAttribute('controls'))throw 'native removal through shadow';");
    assert!(app.media_control_rect().is_none());assert!(app.media.as_ref().unwrap().pressed.is_none());
    assert!(!app.media.as_ref().unwrap().controls_focused);assert!(!app.media_key(0x20));
    eval(&mut app,"c.set.call(v,true);if(v.controls!=='own-shadow'||!c.get.call(v))throw 'restored native bar preserves shadow';delete v.controls;");
    assert!(!app.media_pointer_up(bar.left+40,bar.top+16),"removed press cannot revive on readdition");
    assert!(!app.media_key(0x20),"removed native control focus cannot revive");
    let player=app.media.as_ref().unwrap();assert!(!player.playing());assert!(player.paused);
    assert_eq!(player.clock.position(Instant::now()),Duration::ZERO);assert_eq!(player.generation,generation);
    assert!(Arc::ptr_eq(&player.clip,&clip));assert!(Arc::ptr_eq(&app.images[&node],&pixels));
    assert!(Arc::ptr_eq(&app.embedded_video.as_ref().unwrap().retired,&source));assert!(!source.load(Ordering::Relaxed));
    assert_eq!(app.document.attr(node,"src"),Some("scene-intra.ivf"));reference(&app,0);
    app.input_media_action("Play").unwrap();assert!(app.media.as_ref().unwrap().playing());
    app.input_media_action("Pause").unwrap();assert!(!app.media.as_ref().unwrap().playing());
    assert_eq!(app.scripting.as_ref().unwrap().error_count(),0,"{}",app.scripting.as_ref().unwrap().report());
}

#[test]
fn authored_video_controls_attr_changes_preserve_pending_seek_and_real_selected_pixels() {
    let mut app=open("authored-video.html");app.input_media_action("Play").unwrap();
    let player=app.media.as_ref().unwrap();let clip=player.clip.clone();
    let generation=player.generation;let source=app.embedded_video.as_ref().unwrap().retired.clone();
    let tasks=app.scripting.as_ref().unwrap().media_tasks().unwrap();
    eval(&mut app,"video.currentTime=1.25;if(!video.seeking||video.paused||video.currentTime!==1.25)throw 'real accepted playing seek';");
    let queued=tasks.lock().unwrap().len();assert!(queued>0);
    eval(&mut app,"var controlAttr=video.getAttributeNode('controls');video.removeAttributeNode(controlAttr);video.setAttributeNode(controlAttr);controlAttr.value='false';video.removeAttribute('controls');if(video.controls||!video.seeking||video.paused||video.currentTime!==1.25)throw 'controls mutation disturbed pending seek';");
    assert_eq!(tasks.lock().unwrap().len(),queued);assert_eq!(app.media.as_ref().unwrap().generation,generation);
    assert!(app.media_control_rect().is_none());assert!(app.input_media_action("Pause").is_err());
    assert!(!app.media_key(0x20));assert!(!app.scripting.as_ref().unwrap().has_media_selection());
    app.apply_media_pause();
    let player=app.media.as_ref().unwrap();let seek_generation=player.generation;
    assert_eq!(player.pending,Some(2));assert!(player.seeking);
    eval(&mut app,"video.pause();if(!video.paused||!video.seeking||video.currentTime!==1.25||video.controls)throw 'real JS pause works without UA controls';");
    app.apply_media_pause();assert_eq!(app.media.as_ref().unwrap().generation,seek_generation);
    assert_eq!(app.media.as_ref().unwrap().pending,Some(2));settle(&mut app);
    let player=app.media.as_ref().unwrap();assert_eq!(player.displayed,2);assert!(!player.seeking);
    assert!(!player.playing());assert!(player.paused);assert_eq!(player.clock.position(Instant::now()),Duration::from_millis(1250));
    assert!(Arc::ptr_eq(&player.clip,&clip));assert!(Arc::ptr_eq(&app.embedded_video.as_ref().unwrap().retired,&source));assert!(!source.load(Ordering::Relaxed));reference(&app,2);
    eval(&mut app,"video.setAttribute('controls','false');if(!video.controls||video.currentTime!==1.25||!video.paused)throw 'restored reflected controls';");
    assert!(app.media_control_rect().is_some());app.input_media_action("Play").unwrap();assert!(app.media.as_ref().unwrap().playing());
    app.input_media_action("Pause").unwrap();assert!(!app.media.as_ref().unwrap().playing());
    assert_eq!(app.scripting.as_ref().unwrap().error_count(),0,"{}",app.scripting.as_ref().unwrap().report());
}
