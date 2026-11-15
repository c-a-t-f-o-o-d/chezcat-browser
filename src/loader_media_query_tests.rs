//! Initial font selection follows the same captured media context as CSS.
//! Original local font fixture only; network scheduling and devices are not run.
use super::*;
use std::sync::atomic::{AtomicUsize,Ordering};

struct Fixture(std::path::PathBuf);
impl Fixture {
    fn new()->Self {
        static NEXT:AtomicUsize=AtomicUsize::new(0);
        let folder=std::env::temp_dir().join(format!("chezcat-media-font-{}-{}",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));
        std::fs::create_dir(&folder).unwrap();
        std::fs::write(folder.join("original.ttf"),crate::font::tests::fixture_font()).unwrap();
        std::fs::write(folder.join("page.html"),r#"<style>
@font-face{font-family:Narrow;src:url(original.ttf)}@font-face{font-family:Wide;src:url(original.ttf)}
@media screen{@font-face{font-family:Nested;src:url(never-open.ttf)}}
p{font-family:Narrow}@media screen and (min-width:1024px){p{font-family:Wide}}
</style><p>AV</p><span style='font-family:Nested'>AV</span>"#).unwrap();Self(folder)
    }
    fn load(&self,width:f64)->Loaded {
        let location=navigation::explicit(&self.0.join("page.html")).unwrap();
        local_with_options(&location,LoadOptions{javascript:false,media_environment:Some(css::MediaEnvironment{width,height:660.0,hover:css::Hover::Hover}),..LoadOptions::default()}).unwrap()
    }
}
impl Drop for Fixture{fn drop(&mut self){let _=std::fs::remove_dir_all(&self.0);}}

#[test]
fn css_media_loader_initial_font_use_is_consistent_with_fractional_captured_viewport(){
    let fixture=Fixture::new();
    for (width,chosen,unused) in [(1023.6,"Narrow","Wide"),(1024.0,"Wide","Narrow")] {
        let loaded=fixture.load(width);assert_eq!(loaded.document.media_environment.unwrap().width,width);
        assert_eq!(loaded.fonts.len(),1,"{:?}",loaded.events);
        assert!(loaded.fonts.select(&[chosen.into()],400,false).is_some());
        assert!(loaded.fonts.select(&[unused.into()],400,false).is_none());
        assert!(loaded.fonts.select(&["Nested".into()],400,false).is_none());
        assert_eq!(loaded.skipped,0,"{:?}",loaded.events);
        assert!(!loaded.events.iter().any(|event|event.contains("never-open.ttf")));
    }
}
