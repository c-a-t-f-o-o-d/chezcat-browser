//! Original semantic continuation controls over real App editors/documents.
//! These tests use no OS clipboard, file dialog, physical input or native window.
use super::*;
use std::sync::atomic::{AtomicUsize,Ordering};

struct Fixture(PathBuf);
impl Fixture {
    fn new()->Self {
        static NEXT:AtomicUsize=AtomicUsize::new(0);
        let path=std::env::temp_dir().join(format!("chezcat-platform-continuation-{}-{}",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap();
        for (name,title) in [("original.html","Original"),("other.html","Other"),("chosen.html","Chosen")] {
            std::fs::write(path.join(name),format!("<!doctype html><title>{title}</title><style>input,textarea{{display:block}}</style><input id=one value='same'><input id=two value='same'><textarea id=area>same</textarea>")).unwrap();
        }
        Self(path)
    }
    fn path(&self,name:&str)->PathBuf {self.0.join(name)}
    fn app(&self)->App {let mut app=App::new();app.open(&self.path("original.html"));assert!(!app.failed,"{}",app.status);app}
}
impl Drop for Fixture {fn drop(&mut self){let _=std::fs::remove_dir_all(&self.0);}}
fn node(app:&App,id:&str)->usize {(0..app.document.nodes.len()).find(|&node|app.document.attr(node,"id")==Some(id)).unwrap()}
fn value(app:&App,id:&str)->String {app.forms.controls[&node(app,id)].value.text()}
fn focus(app:&mut App,id:&str) {
    let node=node(app,id);let rect=app.page.controls.iter().find(|rect|rect.node==node).unwrap();
    let(x,y)=(rect.x+rect.width/2,app.content_top()+rect.y+rect.height/2);
    app.pointer_down(x,y,false);app.pointer_up(x,y);app.key(0x41,true,false,false);
}
fn request(app:&mut App,key:u32)->PlatformRequest {let action=app.key(key,true,false,false);app.platform_request(action)}
fn panel_editor(app:&mut App) {
    for _ in 0..64 {
        if app.browser_panel.as_ref().unwrap().platform_editor().is_some() {app.key(0x41,true,false,false);return;}
        app.key(0x09,false,false,false);
    }
    panic!("Actual Settings editor was not reachable by native App Tab input.");
}
fn inspector_request(app:&mut App,key:u32)->PlatformRequest {
    let action=app.inspector_key(key,true,false,false);app.inspector_platform_request(action)
}

#[test]
fn platform_continuation_unchanged_cut_paste_keep_existing_unicode_and_control_rules() {
    let fixture=Fixture::new();let mut app=fixture.app();focus(&mut app,"one");
    let cut=request(&mut app,0x58);assert!(matches!(cut.action(),PlatformAction::Cut(text) if text=="same"));
    assert!(app.complete_platform_request(cut,PlatformCompletion::CutWritten));
    assert_eq!(value(&app,"one"),"");assert_eq!(value(&app,"two"),"same");
    let paste=request(&mut app,0x56);assert!(matches!(paste.action(),PlatformAction::Paste));
    assert!(app.complete_platform_request(paste,PlatformCompletion::Paste("é🦀\r\nvalue".into())));
    assert_eq!(value(&app,"one"),"é🦀value");
    focus(&mut app,"area");let paste=request(&mut app,0x56);
    assert!(app.complete_platform_request(paste,PlatformCompletion::Paste("é🦀\r\nvalue".into())));
    assert_eq!(value(&app,"area"),"é🦀\nvalue");
}

#[test]
fn platform_continuation_same_text_foreign_field_and_changed_selection_never_cut() {
    let fixture=Fixture::new();let mut app=fixture.app();focus(&mut app,"one");let cut=request(&mut app,0x58);
    focus(&mut app,"two");assert!(!app.complete_platform_request(cut,PlatformCompletion::CutWritten));
    assert_eq!((value(&app,"one"),value(&app,"two")),("same".into(),"same".into()));
    focus(&mut app,"one");app.paste("same same");let one=node(&app,"one");
    {let editor=&mut app.forms.controls.get_mut(&one).unwrap().value;editor.move_to(0,false);editor.move_to(4,true);}
    let cut=request(&mut app,0x58);
    // The selected text is still "same"; old cut_copied(text) could remove the
    // second occurrence. The owned continuation also retains the exact range.
    {let editor=&mut app.forms.controls.get_mut(&one).unwrap().value;editor.move_to(5,false);editor.move_to(9,true);}
    assert!(!app.complete_platform_request(cut,PlatformCompletion::CutWritten));assert_eq!(value(&app,"one"),"same same");
    let paste=request(&mut app,0x56);app.forms.controls.get_mut(&one).unwrap().readonly=true;
    assert!(!app.complete_platform_request(paste,PlatformCompletion::Paste("refused".into())));assert_eq!(value(&app,"one"),"same same");
}

#[test]
fn platform_continuation_equal_editors_in_different_and_replaced_apps_are_not_same_owner() {
    let fixture=Fixture::new();let mut first=fixture.app();let mut second=fixture.app();
    focus(&mut first,"one");focus(&mut second,"one");let cut=request(&mut first,0x58);
    assert!(!second.complete_platform_request(cut,PlatformCompletion::CutWritten));assert_eq!(value(&second,"one"),"same");
    let paste=request(&mut first,0x56);drop(first);let mut replacement=fixture.app();focus(&mut replacement,"one");
    assert!(!replacement.complete_platform_request(paste,PlatformCompletion::Paste("refused".into())));assert_eq!(value(&replacement,"one"),"same");
    let cut=request(&mut second,0x58);assert!(second.complete_platform_request(cut,PlatformCompletion::CutWritten));assert_eq!(value(&second,"one"),"");
}

#[test]
fn platform_continuation_navigation_and_reopened_panel_aba_leave_new_targets_untouched() {
    let fixture=Fixture::new();let mut app=fixture.app();focus(&mut app,"one");let paste=request(&mut app,0x56);
    app.open(&fixture.path("other.html"));app.open(&fixture.path("original.html"));focus(&mut app,"one");
    assert!(!app.complete_platform_request(paste,PlatformCompletion::Paste("refused".into())));assert_eq!(value(&app,"one"),"same");
    app.show_settings();panel_editor(&mut app);let old_text=app.browser_panel.as_ref().unwrap().platform_editor().unwrap().text();let paste=request(&mut app,0x56);
    app.close_browser_panel();app.show_settings();panel_editor(&mut app);
    assert_eq!(app.browser_panel.as_ref().unwrap().platform_editor().unwrap().text(),old_text);
    assert!(!app.complete_platform_request(paste,PlatformCompletion::Paste("refused".into())));
    assert_eq!(app.browser_panel.as_ref().unwrap().platform_editor().unwrap().text(),old_text);
}

#[test]
fn platform_continuation_owned_dialog_focus_boundary_survives_but_navigation_refuses_file() {
    let fixture=Fixture::new();let mut app=fixture.app();focus(&mut app,"one");let paste=request(&mut app,0x56);
    // Ordinary native capture/focus cleanup is not a logical editor change.
    app.pointer_cancel();app.main_window_focus();
    assert!(app.complete_platform_request(paste,PlatformCompletion::Paste("kept".into())));assert_eq!(value(&app,"one"),"kept");
    let file=request(&mut app,0x4f);assert!(matches!(file.action(),PlatformAction::OpenFile));
    app.pointer_cancel();app.main_window_focus();
    assert!(app.complete_platform_request(file,PlatformCompletion::OpenFile(fixture.path("chosen.html"))));assert_eq!(app.document.title,"Chosen");
    let file=request(&mut app,0x4f);app.open(&fixture.path("other.html"));let before=(app.raw.clone(),app.script_epoch,app.location.display());
    assert!(!app.complete_platform_request(file,PlatformCompletion::OpenFile(fixture.path("chosen.html"))));
    assert_eq!((app.raw.clone(),app.script_epoch,app.location.display()),before);assert_eq!(app.document.title,"Other");
}

#[test]
fn platform_continuation_inspector_pane_and_reopen_aba_cannot_retarget_clipboard() {
    let fixture=Fixture::new();let mut app=fixture.app();app.open_inspector();assert!(app.inspector_tab_by_name("Console"));
    app.inspector_paste("same");app.inspector_key(0x41,true,false,false);let cut=inspector_request(&mut app,0x58);
    app.inspector_key(0x46,true,false,false);app.inspector_paste("same");app.inspector_key(0x41,true,false,false);
    assert!(!app.complete_platform_request(cut,PlatformCompletion::CutWritten));assert_eq!(app.inspector.platform_editor().unwrap().text(),"same");
    let paste=inspector_request(&mut app,0x56);app.close_inspector();app.open_inspector();app.inspector_key(0x46,true,false,false);app.inspector_key(0x41,true,false,false);
    assert!(!app.complete_platform_request(paste,PlatformCompletion::Paste("refused".into())));assert_eq!(app.inspector.platform_editor().unwrap().text(),"same");
    let cut=inspector_request(&mut app,0x58);assert!(app.complete_platform_request(cut,PlatformCompletion::CutWritten));assert_eq!(app.inspector.platform_editor().unwrap().text(),"");assert_eq!(value(&app,"one"),"same");
}

#[test]
fn platform_continuation_newer_request_supersedes_old_and_wrong_completion_is_atomic() {
    let fixture=Fixture::new();let mut app=fixture.app();focus(&mut app,"one");
    let old=request(&mut app,0x56);let fresh=request(&mut app,0x56);
    assert!(!app.complete_platform_request(old,PlatformCompletion::Paste("old".into())));assert_eq!(value(&app,"one"),"same");
    assert!(app.complete_platform_request(fresh,PlatformCompletion::Paste("fresh".into())));assert_eq!(value(&app,"one"),"fresh");
    focus(&mut app,"one");let cut=request(&mut app,0x58);
    assert!(!app.complete_platform_request(cut,PlatformCompletion::Paste("mismatched".into())));assert_eq!(value(&app,"one"),"fresh");
    let canceled=request(&mut app,0x56);drop(canceled);assert_eq!(value(&app,"one"),"fresh");
}
