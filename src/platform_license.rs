//! Pre-App offline consent through owned native EDIT/BUTTON controls.
//! The optional bounded driver addresses this window only, never system input.
use super::*;
use crate::license_acceptance::{Scope, LICENSE_SHA256, LICENSE_TEXT, LICENSE_VERSION};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;

const CLASS: &str = "ChezCatLicensePrompt";
const STEP: u32 = 0x804c;
const TEXT: usize = 0x6101;
const PERSONAL: usize = 0x6102;
const ORGANIZATION: usize = 0x6103;
const DECLINE: usize = 0x6104;
const CAPTIONS: [&str; 3] = ["Accept personal/internal use", "Accept for authorized organization", "Decline and exit"];
const SUMMARY: &str = "Read the full license below. Personal/internal use is subject to its terms. Redistribution, modification and commercial uses have limits: the full license controls. Accept for an organization only if you are authorized to bind it. Decline exits before browsing or network access. Acceptance is recorded locally; no account or online activation is used.";

#[link(name = "user32")]
extern "system" {
    fn GetFocus() -> Handle;
    fn GetWindowTextLengthW(window: Handle) -> i32;
    fn GetWindowTextW(window: Handle, text: *mut u16, count: i32) -> i32;
    fn MoveWindow(window: Handle, x: i32, y: i32, width: i32, height: i32, repaint: i32) -> i32;
    fn GetClassNameW(window: Handle, text: *mut u16, count: i32) -> i32;
}
#[link(name = "gdi32")]
extern "system" { fn GetStockObject(object: i32) -> Handle; }

#[derive(Clone, Copy, PartialEq, Eq)]
enum Target { Text, Personal, Organization, Decline }
impl Target {
    fn name(self) -> &'static str {match self {Self::Text=>"text",Self::Personal=>"personal",Self::Organization=>"organization",Self::Decline=>"decline"}}
    fn index(self) -> usize {match self {Self::Text=>0,Self::Personal=>1,Self::Organization=>2,Self::Decline=>3}}
    fn parse(value:&str) -> Result<Self,String> {match value {"text"=>Ok(Self::Text),"personal"=>Ok(Self::Personal),"organization"=>Ok(Self::Organization),"decline"=>Ok(Self::Decline),_=>Err("Unknown license UI target.".into())}}
}
#[derive(Clone, Copy)]
enum Key { Tab, ShiftTab, Enter, Space, Escape, PageDown, PageUp, CtrlHome, CtrlEnd }
impl Key {
    fn name(self)->&'static str {match self {Self::Tab=>"Tab",Self::ShiftTab=>"ShiftTab",Self::Enter=>"Enter",Self::Space=>"Space",Self::Escape=>"Escape",Self::PageDown=>"PageDown",Self::PageUp=>"PageUp",Self::CtrlHome=>"CtrlHome",Self::CtrlEnd=>"CtrlEnd"}}
    fn parse(value:&str)->Result<Self,String>{match value {"Tab"=>Ok(Self::Tab),"ShiftTab"=>Ok(Self::ShiftTab),"Enter"=>Ok(Self::Enter),"Space"=>Ok(Self::Space),"Escape"=>Ok(Self::Escape),"PageDown"=>Ok(Self::PageDown),"PageUp"=>Ok(Self::PageUp),"CtrlHome"=>Ok(Self::CtrlHome),"CtrlEnd"=>Ok(Self::CtrlEnd),_=>Err("Unknown license UI key.".into())}}
}
#[derive(Clone, Copy)]
enum Action { Focus(Target), Expect(Target), Choose(Target), Key(Key) }
impl Action {
    fn fields(self)->(&'static str,&'static str) {match self {Self::Focus(t)=>("focus",t.name()),Self::Expect(t)=>("expect-focus",t.name()),Self::Choose(t)=>("choose",t.name()),Self::Key(k)=>("key",k.name())}}
}
#[derive(Clone, Copy)]
struct ActionRecord { action:Action, before:&'static str, after:&'static str }
struct State {
    window:Handle, controls:[Handle;4], labels:[Handle;2],
    done:Option<Option<Scope>>, error:Option<String>, scripted:bool,
    actions:Vec<Action>, records:Vec<ActionRecord>, next:usize,
    initial_focus:&'static str, last_focus:&'static str,
    full_text:bool, readonly:bool, native_controls:bool, labels_match:bool, units:usize,
}

fn script(path:&Path)->Result<Vec<Action>,String> {
    let mut input=String::new();
    File::open(path).map_err(|_|"Cannot open the explicit license UI script.")?.take(16385)
        .read_to_string(&mut input).map_err(|_|"Cannot read the license UI script as UTF-8.")?;
    if input.len()>16384 {return Err("License UI script exceeds its finite allowance.".into());}
    let mut lines=input.lines();
    if lines.next()!=Some(format!("license\t{LICENSE_SHA256}").as_str()) {return Err("License UI script must first identify the exact displayed license digest.".into());}
    let mut actions=Vec::new();
    for line in lines {
        if actions.len()>=64 {return Err("License UI script action allowance reached.".into());}
        let(kind,value)=line.split_once('\t').ok_or("License UI actions require an explicit tab-separated value.")?;
        let action=match kind {"focus"=>Action::Focus(Target::parse(value)?),"expect-focus"=>Action::Expect(Target::parse(value)?),"choose"=>{
            let target=Target::parse(value)?;if target==Target::Text{return Err("Full license text is not an acceptance choice.".into());}Action::Choose(target)
        },"key"=>Action::Key(Key::parse(value)?),_=>return Err("Unknown license UI action.".into())};
        actions.try_reserve(1).map_err(|_|"Could not retain license UI actions.")?;actions.push(action);
    }
    if actions.is_empty(){return Err("License UI script has no explicit actions.".into());}
    Ok(actions)
}
fn utf16(value:&str)->Result<Vec<u16>,String>{
    let count=value.encode_utf16().count().checked_add(1).filter(|n|*n<=1024*1024).ok_or("Native license text exceeds its finite allowance.")?;
    if value.contains('\0'){return Err("Native license text contains NUL.".into());}
    let mut result=Vec::new();result.try_reserve_exact(count).map_err(|_|"Could not allocate native license text.")?;
    result.extend(value.encode_utf16());result.push(0);Ok(result)
}
unsafe fn native_text(window:Handle)->Result<String,String>{
    let length=GetWindowTextLengthW(window);
    if !(0..1024*1024).contains(&length){return Err("Native control text exceeds its finite allowance.".into());}
    let mut buffer=Vec::new();buffer.try_reserve_exact(length as usize+1).map_err(|_|"Could not inspect native license text.")?;buffer.resize(length as usize+1,0);
    let read=GetWindowTextW(window,buffer.as_mut_ptr(),buffer.len() as i32);
    if read!=length{return Err("Native control text was truncated.".into());}
    String::from_utf16(&buffer[..read as usize]).map_err(|_|"Native control text is not valid UTF-16.".into())
}
unsafe fn full_text_matches(control:Handle)->Result<(bool,usize),String>{
    let text=native_text(control)?;let units=text.encode_utf16().count();
    Ok((text.replace("\r\n","\n")==LICENSE_TEXT,units))
}
unsafe fn actual_target(state:*mut State)->Option<Target>{
    let focused=GetFocus();[Target::Text,Target::Personal,Target::Organization,Target::Decline].into_iter()
        .find(|target|(&(*state).controls)[target.index()]==focused&&!focused.is_null())
}
unsafe fn focus_name(state:*mut State)->&'static str {actual_target(state).map_or("none",Target::name)}
unsafe fn focus(state:*mut State,target:Target)->bool {
    let handle=(&(*state).controls)[target.index()];SetFocus(handle);GetFocus()==handle
}
unsafe fn finish(state:*mut State,scope:Option<Scope>){
    if (*state).done.is_some(){return;}
    (*state).last_focus=focus_name(state);
    if (*state).scripted && (*state).next+1<(&(*state).actions).len(){(*state).error=Some("Terminal license UI action must be the final script line.".into());}
    (*state).done=Some(if (*state).error.is_some(){None}else{scope});
    let window=(*state).window;if !window.is_null(){DestroyWindow(window);}
}
unsafe fn fail(state:*mut State,error:&str){(*state).error=Some(error.into());finish(state,None);}
unsafe fn choose(state:*mut State,target:Target){
    let handle=(&(*state).controls)[target.index()];
    if target!=Target::Text {SendMessageW(handle,0x00f5,0,0);} // BM_CLICK -> normal WM_COMMAND
}
unsafe fn key(state:*mut State,key:Key)->bool {
    let current=actual_target(state);
    match key {
        Key::Tab|Key::ShiftTab=>{
            let index=current.unwrap_or(Target::Decline).index();let step=if matches!(key,Key::ShiftTab){3}else{1};
            focus(state,[Target::Text,Target::Personal,Target::Organization,Target::Decline][(index+step)%4]);true
        }
        Key::Escape=>{choose(state,Target::Decline);true}
        Key::Enter|Key::Space=>{
            match current {Some(Target::Text)=>false,Some(target)=>{choose(state,target);true},None=>{choose(state,Target::Decline);true}}
        }
        Key::PageDown|Key::PageUp|Key::CtrlHome|Key::CtrlEnd=>{
            if current!=Some(Target::Text){return false;}
            let control=(&(*state).controls)[0];
            match key {
                Key::PageDown|Key::PageUp=>{SendMessageW(control,0x0100,if matches!(key,Key::PageDown){0x22}else{0x21},1);}
                Key::CtrlHome|Key::CtrlEnd=>{let position=if matches!(key,Key::CtrlHome){0}else{GetWindowTextLengthW(control)};SendMessageW(control,0x00b1,position as usize,position as isize);SendMessageW(control,0x00b7,0,0);}
                _=>{}
            };true
        }
    }
}
unsafe fn layout(state:*mut State){
    let window=(*state).window;let controls=(*state).controls;let labels=(*state).labels;
    if controls[0].is_null(){return;}
    let mut rect:Rect=zeroed();if GetClientRect(window,&mut rect)==0{return;}
    let width=rect.right.max(680);let height=rect.bottom.max(430);let gap=16;let button=(width-gap*4)/3;
    MoveWindow(labels[0],gap,12,width-gap*2,76,1);MoveWindow(labels[1],gap,92,width-gap*2,40,1);
    MoveWindow(controls[0],gap,138,width-gap*2,height-210,1);
    for index in 0..3 {MoveWindow(controls[index+1],gap+(button+gap)*index as i32,height-56,button,38,1);}
}
unsafe extern "system" fn procedure(window:Handle,message:u32,wparam:usize,lparam:isize)->isize{
    if message==0x0081 {
        let create=&*(lparam as *const CreateStruct);SetLastError(0);
        if SetWindowLongPtrW(window,-21,create.parameters as isize)==0&&GetLastError()!=0{return 0;}
        (*(create.parameters as *mut State)).window=window;return 1;
    }
    let state=GetWindowLongPtrW(window,-21) as *mut State;
    if state.is_null(){return DefWindowProcW(window,message,wparam,lparam);}
    match message {
        0x0082=>{(*state).window=null_mut();SetWindowLongPtrW(window,-21,0);DefWindowProcW(window,message,wparam,lparam)}
        0x0010=>{finish(state,None);0}
        0x0002=>{if (*state).done.is_none(){(*state).done=Some(None);}0}
        0x0024=>{(&mut *(lparam as *mut MinMaxInfo)).min_track_size=Point{x:740,y:560};0}
        0x0005=>{layout(state);0}
        0x0111=>{
            let id=wparam&0xffff;let notification=wparam>>16;
            let target=match id {PERSONAL=>Some(Target::Personal),ORGANIZATION=>Some(Target::Organization),DECLINE=>Some(Target::Decline),_=>None};
            if let Some(target)=target.filter(|target|notification==0&&lparam as Handle==(&(*state).controls)[target.index()]) {
                if target==Target::Decline {finish(state,None);}else{
                    match full_text_matches((&(*state).controls)[0]) {
                        Ok((true,_))=>finish(state,Some(if target==Target::Personal{Scope::Personal}else{Scope::Organization})),
                        _=>fail(state,"Native license text differs from the exact embedded terms; no acceptance was recorded."),
                    }
                }
            };0
        }
        STEP=>{
            if !(*state).scripted||(*state).done.is_some(){return 0;}
            let Some(action)=(&(*state).actions).get((*state).next).copied() else {fail(state,"License UI script ended without an explicit final choice.");return 0;};
            let before=focus_name(state);
            let accepted=match action {
                Action::Focus(target)=>focus(state,target),
                Action::Expect(target)=>actual_target(state)==Some(target),
                Action::Choose(target)=>{if focus(state,target){choose(state,target);true}else{false}},
                Action::Key(value)=>key(state,value),
            };
            let after=if (*state).done.is_some(){(*state).last_focus}else{focus_name(state)};
            (&mut (*state).records).push(ActionRecord{action,before,after});
            if !accepted&&(*state).done.is_none(){fail(state,"License UI action could not reach its native control or expected focus.");}
            (*state).next+=1;
            if (*state).done.is_none()&&PostMessageW(window,STEP,0,0)==0{fail(state,"Could not advance the owned license UI script.");}
            0
        }
        _=>DefWindowProcW(window,message,wparam,lparam),
    }
}
struct WindowOwner {state:*mut State,instance:Handle,class:Vec<u16>}
impl Drop for WindowOwner {fn drop(&mut self){unsafe{if !(*self.state).window.is_null(){DestroyWindow((*self.state).window);}UnregisterClassW(self.class.as_ptr(),self.instance);}}}
unsafe fn child(window:Handle,instance:Handle,class:&str,text:&str,style:u32,id:usize)->Result<Handle,String>{
    let class=utf16(class)?;let text=utf16(text)?;
    let handle=CreateWindowExW(0,class.as_ptr(),text.as_ptr(),0x50000000|style,0,0,0,0,window,id as Handle,instance,null_mut());
    if handle.is_null(){return Err("Could not create a native license control.".into());}
    SendMessageW(handle,0x0030,GetStockObject(17) as usize,1);Ok(handle)
}
unsafe fn prompt(state:*mut State)->Result<Option<Scope>,String>{
    if LICENSE_TEXT.contains('\r')||LICENSE_TEXT.contains('\0'){return Err("Embedded license must be exact NUL-free LF text.".into());}
    SetProcessDPIAware();let instance=GetModuleHandleW(null());if instance.is_null(){return Err("Cannot initialize the native license prompt.".into());}
    let class=utf16(CLASS)?;let title=utf16(&format!("ChezCat — CSAPUL {LICENSE_VERSION} license acceptance"))?;
    let descriptor=WindowClass{style:3,procedure:Some(procedure),class_extra:0,window_extra:0,instance,icon:null_mut(),cursor:LoadCursorW(null_mut(),32512usize as *const u16),background:6usize as Handle,menu_name:null(),class_name:class.as_ptr()};
    if RegisterClassW(&descriptor)==0{return Err("Cannot register the native license prompt.".into());}
    let _owner=WindowOwner{state,instance,class};
    let scripted=(*state).scripted;
    let window=CreateWindowExW(if scripted{0x08000080}else{0},_owner.class.as_ptr(),title.as_ptr(),0x00cf0000,
        if scripted{-32000}else{i32::MIN},if scripted{-32000}else{i32::MIN},1040,780,null_mut(),null_mut(),instance,state.cast());
    if window.is_null(){return Err("Cannot create the native license prompt.".into());}
    (&mut (*state).labels)[0]=child(window,instance,"STATIC",SUMMARY,0x80,0)?;
    let details=format!("License version: {LICENSE_VERSION}\r\nSHA-256: {LICENSE_SHA256}");
    (&mut (*state).labels)[1]=child(window,instance,"STATIC",&details,0x80,0)?;
    let control=child(window,instance,"EDIT","",0x00a10844,TEXT)?; // border, scroll, tab; multiline/read-only
    (&mut (*state).controls)[0]=control;
    let display=LICENSE_TEXT.replace('\n',"\r\n");let full=utf16(&display)?;
    SendMessageW(control,0x00c5,full.len(),0); // EM_SETLIMITTEXT
    if SetWindowTextW(control,full.as_ptr())==0{return Err("Cannot expose the full native license text.".into());}
    for index in 0..3 {(&mut (*state).controls)[index+1]=child(window,instance,"BUTTON",CAPTIONS[index],0x00010000|if index==2{1}else{0},[PERSONAL,ORGANIZATION,DECLINE][index])?;}
    let(matches,units)=full_text_matches(control)?;(*state).full_text=matches;(*state).units=units;
    (*state).readonly=GetWindowLongPtrW(control,-16) as u32&0x0800!=0;
    (*state).labels_match=native_text((&(*state).labels)[0])?==SUMMARY&&native_text((&(*state).labels)[1])?==details;
    let mut class_name=[0u16;32];(*state).native_controls=GetClassNameW(control,class_name.as_mut_ptr(),32)>0&&String::from_utf16_lossy(&class_name).trim_end_matches('\0').eq_ignore_ascii_case("edit");
    for index in 0..3 {let button=(&(*state).controls)[index+1];class_name.fill(0);(*state).native_controls&=GetClassNameW(button,class_name.as_mut_ptr(),32)>0&&String::from_utf16_lossy(&class_name).trim_end_matches('\0').eq_ignore_ascii_case("button")&&native_text(button)?==CAPTIONS[index];}
    if !matches||!(*state).readonly||!(*state).native_controls||!(*state).labels_match{return Err("Native license controls did not expose the complete accepted terms.".into());}
    layout(state);ShowWindow(window,if scripted{4}else{5});UpdateWindow(window);
    focus(state,Target::Decline);(*state).initial_focus=focus_name(state);
    if scripted&&(*state).initial_focus!="decline"{return Err("Native license default focus could not be established.".into());}
    if scripted&&PostMessageW(window,STEP,0,0)==0{return Err("Cannot start the explicit owned license UI script.".into());}
    while (*state).done.is_none(){
        let mut message:Message=zeroed();let code=GetMessageW(&mut message,null_mut(),0,0);
        if code<=0{return Err("Native license prompt was interrupted.".into());}
        if message.message==0x0100&&(message.window==window||(*state).controls.contains(&message.window))&&actual_target(state).is_some(){
            let shift=GetKeyState(0x10)<0;let ctrl=GetKeyState(0x11)<0;
            let value=match message.wparam {9=>Some(if shift{Key::ShiftTab}else{Key::Tab}),13=>Some(Key::Enter),32=>Some(Key::Space),27=>Some(Key::Escape),0x21=>Some(Key::PageUp),0x22=>Some(Key::PageDown),0x24 if ctrl=>Some(Key::CtrlHome),0x23 if ctrl=>Some(Key::CtrlEnd),_=>None};
            if value.is_some_and(|value|key(state,value)){continue;}
        }
        TranslateMessage(&message);DispatchMessageW(&message);
    }
    if let Some(error)=(*state).error.take(){Err(error)}else{Ok((*state).done.take().unwrap_or(None))}
}
fn report(file:&mut File,state:&State,result:&Result<Option<Scope>,String>)->Result<(),String>{
    let outcome=match result{Ok(Some(Scope::Personal))=>"personal",Ok(Some(Scope::Organization))=>"organization",Ok(None)=>"declined",Err(_)=>"error"};
    write!(file,"{{\"schema_version\":1,\"license_version\":\"{}\",\"license_sha256\":\"{}\",\"license_bytes\":{},\"scripted_offscreen\":{},\"native_full_text_matches\":{},\"native_text_utf16_units\":{},\"native_readonly\":{},\"native_edit_and_buttons\":{},\"native_summary_and_identity\":{},\"initial_focus\":\"{}\",\"final_focus\":\"{}\",\"outcome\":\"{}\",\"actions\":[",LICENSE_VERSION,LICENSE_SHA256,LICENSE_TEXT.len(),state.scripted,state.full_text,state.units,state.readonly,state.native_controls,state.labels_match,state.initial_focus,state.last_focus,outcome).map_err(|_|"Could not write the license UI report.")?;
    for(index,row)in state.records.iter().enumerate(){let(kind,value)=row.action.fields();write!(file,"{}{{\"action\":\"{}\",\"value\":\"{}\",\"before\":\"{}\",\"after\":\"{}\"}}",if index==0{""}else{","},kind,value,row.before,row.after).map_err(|_|"Could not write license UI actions.")?;}
    writeln!(file,"]}}").and_then(|_|file.sync_all()).map_err(|_|"Could not finish the license UI report.".into())
}
pub(super) fn run(input:Option<&Path>,output:Option<&Path>)->Result<Option<Scope>,String>{
    if input.is_some()&&output.is_none(){return Err("An explicit license UI script requires its owned local report.".into());}
    let actions=input.map(script).transpose()?.unwrap_or_default();
    let mut records=Vec::new();records.try_reserve_exact(actions.len()).map_err(|_|"Could not retain native license UI evidence.")?;
    let mut output=output.map(|path|OpenOptions::new().write(true).create_new(true).open(path).map_err(|_|"Cannot create a new license UI report; existing reports are preserved.")).transpose()?;
    let mut state=Box::new(State{window:null_mut(),controls:[null_mut();4],labels:[null_mut();2],done:None,error:None,scripted:input.is_some(),actions,records,next:0,initial_focus:"none",last_focus:"none",full_text:false,readonly:false,native_controls:false,labels_match:false,units:0});
    let result=unsafe{prompt(&mut *state)};
    if let Some(file)=&mut output{report(file,&state,&result)?;}result
}
