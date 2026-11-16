//! Browser-owned tab strip and pure main-window input/render routing. Native
//! HWNDs/clipboard/dialogs live elsewhere and receive owned typed outcomes only.
use super::{Session,TabId,WindowId};
use crate::app::{PlatformAction,PlatformRequest};
use crate::paint::{Canvas,ClipRect,GLYPH_HEIGHT};
use crate::scripting::KeyboardInput;
use crate::ui;

pub const STRIP_HEIGHT:i32=32;
const MAX_WINDOWS:usize=8;
const MIN_TAB:i32=72;
const MAX_TAB:i32=192;
#[derive(Default)]
pub struct Outcome {
    pub request:Option<RoutedRequest>,pub opened:Option<(WindowId,TabId)>,pub closed:Option<WindowId>,
    pub repaint:bool,pub suppress_text:bool,
}
pub struct RoutedRequest {pub window:WindowId,pub tab:TabId,pub request:PlatformRequest}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
enum Hit {Select(TabId),Close(TabId),New}
#[derive(Clone,PartialEq,Eq)]
struct Token {window:WindowId,members:Vec<TabId>,active:Option<TabId>,width:i32,height:i32,first:usize,revision:u64}
struct Geometry {token:Token,controls:Vec<(Hit,ClipRect)>}
struct Press {target:Target,token:Token,x:i32,y:i32,moved:bool}
enum Target {Strip(Hit,ClipRect),Page(TabId)}
struct Window {
    id:WindowId,width:i32,height:i32,first:usize,revision:u64,
    members:Vec<TabId>,active:Option<TabId>,press:Option<Press>,hover:Option<Hit>,swallowed:u8,
}
impl Window {
    fn preflight(&self)->Result<(),String>{self.revision.checked_add(2).ok_or("Tab strip interaction identity exhausted.")?;Ok(())}
    fn bump(&mut self)->Result<(),String>{let revision=self.revision.checked_add(1).ok_or("Tab strip interaction identity exhausted.")?;self.press=None;self.revision=revision;Ok(())}
}
#[derive(Default)]
pub struct SessionUi {windows:Vec<Window>,logical_active:Option<WindowId>}
impl SessionUi {
    pub fn new()->Self{Self::default()}
    fn index(&self,id:WindowId)->Result<usize,String>{self.windows.iter().position(|window|window.id==id).ok_or_else(||"This browser window has no UI binding.".into())}
    fn visible(width:i32,total:usize)->usize{let room=(width-40).max(0);if room==0{0}else{((room/MIN_TAB).max(1)as usize).min(total)}}
    fn first_with_active(first:usize,total:usize,visible:usize,position:Option<usize>)->usize{
        let mut first=first.min(total.saturating_sub(visible));
        if let Some(position)=position {
            if position<first{first=position;}else if visible!=0&&position>=first+visible{first=position+1-visible;}
        }
        first
    }
    fn geometry(&self,session:&Session,id:WindowId)->Result<Geometry,String>{
        let window=&self.windows[self.index(id)?];let members=session.window_tabs(id)?.to_vec();let active=session.active(id)?;
        let visible=Self::visible(window.width,members.len());let mut first=window.first.min(members.len().saturating_sub(visible));
        if active!=window.active||members!=window.members {
            first=Self::first_with_active(first,members.len(),visible,active.and_then(|tab|members.iter().position(|id|*id==tab)));
        }
        let token=Token{window:id,members,active,width:window.width,height:window.height,first,revision:window.revision};
        let mut controls=Vec::new();controls.try_reserve_exact(visible.saturating_mul(2).saturating_add(1)).map_err(|_|"Cannot reserve tab strip controls.")?;
        let plus_width=28.min(window.width).max(1);let plus_x=(window.width-plus_width-4).max(0);
        let room=(plus_x-4).max(0);let tab_width=if visible==0{0}else{(room/visible as i32).clamp(1,MAX_TAB)};
        for(index,tab)in token.members.iter().skip(first).take(visible).enumerate(){
            let x=4+index as i32*tab_width;let rect=ClipRect::new(x,3,tab_width,STRIP_HEIGHT-4).intersect(ClipRect::new(0,0,plus_x,STRIP_HEIGHT.min(window.height)));
            if rect.right<=rect.left||rect.bottom<=rect.top{continue;}controls.push((Hit::Select(*tab),rect));
            if rect.right-rect.left>=28 {controls.push((Hit::Close(*tab),ClipRect::new(rect.right-21,7,18,18).intersect(rect)));}
        }
        let plus=ClipRect::new(plus_x,4,plus_width,24).intersect(ClipRect::new(0,0,window.width,window.height));
        if plus.right>plus.left&&plus.bottom>plus.top{controls.push((Hit::New,plus));}
        Ok(Geometry{token,controls})
    }
    fn hit(geometry:&Geometry,x:i32,y:i32)->Option<(Hit,ClipRect)>{
        geometry.controls.iter().rev().find(|(_,rect)|rect.contains(x,y)).copied()
    }
    fn refresh(&mut self,session:&Session,id:WindowId)->Result<(),String>{
        let geometry=self.geometry(session,id)?;let index=self.index(id)?;
        let window=&mut self.windows[index];if window.members!=geometry.token.members||window.active!=geometry.token.active||window.first!=geometry.token.first{
            window.bump()?;window.members=geometry.token.members;window.active=geometry.token.active;window.first=geometry.token.first;window.hover=None;
        }Ok(())
    }
    pub fn resize(&mut self,session:&mut Session,id:WindowId,width:i32,height:i32)->Result<(),String>{
        session.window_tabs(id)?;let(width,height)=(width.clamp(1,8192),height.clamp(1,8192));
        let index=match self.windows.iter().position(|window|window.id==id){Some(index)=>index,None=>{
            if self.windows.len()==MAX_WINDOWS{return Err("Browser UI window limit was reached.".into());}
            self.windows.try_reserve(1).map_err(|_|"Cannot reserve browser window UI.")?;
            self.windows.push(Window{id,width,height,first:0,revision:0,members:Vec::new(),active:None,press:None,hover:None,swallowed:0});self.windows.len()-1
        }};
        let changed=self.windows[index].width!=width||self.windows[index].height!=height;
        self.windows[index].preflight()?;
        if changed{self.pointer_cancel(session,id)?;self.windows[index].bump()?;}
        // Native always supplies full client bounds. Reserve strip space here
        // once; App keeps its complete original chrome/page coordinate grammar.
        session.resize_window(id,width,(height-STRIP_HEIGHT).max(1))?;
        self.windows[index].width=width;self.windows[index].height=height;
        if changed {
            let members=session.window_tabs(id)?;let active=session.active(id)?;
            self.windows[index].first=Self::first_with_active(self.windows[index].first,members.len(),Self::visible(width,members.len()),active.and_then(|tab|members.iter().position(|id|*id==tab)));
        }
        self.refresh(session,id)
    }
    pub fn forget(&mut self,id:WindowId){self.windows.retain(|window|window.id!=id);if self.logical_active==Some(id){self.logical_active=None;}}
    pub fn activate_window(&mut self,session:&mut Session,id:WindowId)->Result<(),String>{
        session.window_tabs(id)?;
        if self.logical_active!=Some(id){
            if let Some(old)=self.logical_active {if let Ok(Some(tab))=session.active(old){let _=session.with_tab(tab,|app|app.invalidate_platform_requests());}}
            self.logical_active=Some(id);
        }Ok(())
    }
    pub fn title(&self,session:&Session,id:WindowId)->Result<String,String>{session.observe_active(id,|app|app.title())}
    pub fn frame(&self,session:&Session,id:WindowId)->Result<Canvas,String>{
        let geometry=self.geometry(session,id)?;let window=&self.windows[self.index(id)?];
        let palette=session.observe_active(id,|app|app.browser_palette()).unwrap_or_else(|_|ui::Palette::new(false));
        let mut canvas=Canvas::new(window.width as usize,window.height as usize);if canvas.width==0{return Err("Cannot allocate browser window canvas.".into());}
        canvas.fill_rect(0,0,window.width,window.height,palette.background);
        if geometry.token.active.is_some(){
            let page=session.observe_active(id,|app|app.draw())?;if page.width==0{return Err("Cannot allocate active page canvas.".into());}
            let rows=page.height.min(window.height.saturating_sub(STRIP_HEIGHT).max(0)as usize);let columns=page.width.min(canvas.width);
            for y in 0..rows{let from=y*page.width;let to=(y+STRIP_HEIGHT as usize)*canvas.width;canvas.pixels[to..to+columns].copy_from_slice(&page.pixels[from..from+columns]);}
        }
        canvas.fill_rect(0,0,window.width,STRIP_HEIGHT,palette.chrome);
        for(hit,rect)in &geometry.controls{match hit{
            Hit::Select(tab)=>{
                let active=geometry.token.active==Some(*tab);let hover=window.hover==Some(*hit);
                let color=if active{palette.panel}else if hover{palette.panel_alt}else{palette.chrome};
                canvas.fill_rect(rect.left,rect.top,rect.right-rect.left,rect.bottom-rect.top,color);
                if active{canvas.fill_rect(rect.left,STRIP_HEIGHT-2,rect.right-rect.left,2,palette.accent);}
                let title=session.observe_tab(*tab,|app|{let title=&app.document.title;if title.is_empty(){"New tab".into()}else{title.chars().take(80).collect::<String>()}})?;
                let clip=ClipRect::new(rect.left+7,rect.top,((rect.right-rect.left)-31).max(0),rect.bottom-rect.top).intersect(*rect);canvas.set_clip(Some(clip));
                canvas.text(rect.left+7,(STRIP_HEIGHT-GLYPH_HEIGHT)/2,&title,1,if active{palette.foreground}else{palette.chrome_text});canvas.set_clip(None);
            },
            Hit::Close(_)=>{if window.hover==Some(*hit){canvas.fill_rect(rect.left,rect.top,rect.right-rect.left,rect.bottom-rect.top,palette.error);}canvas.text(rect.left+5,rect.top+4,"x",1,palette.chrome_text);},
            Hit::New=>{if window.hover==Some(*hit){canvas.fill_rect(rect.left,rect.top,rect.right-rect.left,rect.bottom-rect.top,palette.panel_alt);}canvas.text(rect.left+9,rect.top+7,"+",1,palette.chrome_text);},
        }}Ok(canvas)
    }
    fn route(window:WindowId,tab:TabId,app:&mut crate::app::App,action:PlatformAction)->Option<RoutedRequest>{
        match action{PlatformAction::None=>None,action=>Some(RoutedRequest{window,tab,request:app.platform_request(action)})}
    }
    pub fn pointer_down(&mut self,session:&mut Session,id:WindowId,x:i32,y:i32,shift:bool)->Result<Outcome,String>{
        self.activate_window(session,id)?;self.pointer_cancel(session,id)?;self.refresh(session,id)?;let geometry=self.geometry(session,id)?;let index=self.index(id)?;
        if y<STRIP_HEIGHT{
            if let Some((hit,rect))=Self::hit(&geometry,x,y){self.windows[index].press=Some(Press{target:Target::Strip(hit,rect),token:geometry.token,x,y,moved:false});}
            return Ok(Outcome{repaint:true,..Outcome::default()});
        }
        if x<0||x>=self.windows[index].width||y>=self.windows[index].height{return Ok(Outcome::default());}
        let tab=geometry.token.active.ok_or("This window has no active tab.")?;
        let request=session.with_tab(tab,|app|{let action=app.pointer_down(x,y-STRIP_HEIGHT,shift);Self::route(id,tab,app,action)})?;
        self.windows[index].press=Some(Press{target:Target::Page(tab),token:geometry.token,x,y,moved:false});
        Ok(Outcome{request,repaint:true,..Outcome::default()})
    }
    pub fn pointer_move(&mut self,session:&mut Session,id:WindowId,x:i32,y:i32,dragging:bool)->Result<bool,String>{
        let geometry=self.geometry(session,id)?;let index=self.index(id)?;
        if self.windows[index].press.as_ref().is_some_and(|press|press.token!=geometry.token){self.pointer_cancel(session,id)?;return Ok(true);}
        if let Some(press)=&mut self.windows[index].press{press.moved|=(x-press.x).abs()>4||(y-press.y).abs()>4;}
        let hover=if y<STRIP_HEIGHT{Self::hit(&geometry,x,y).map(|(hit,_)|hit)}else{None};let changed=self.windows[index].hover!=hover;self.windows[index].hover=hover;
        let page=match self.windows[index].press.as_ref().map(|press|&press.target){Some(Target::Page(tab))=>Some(*tab),Some(Target::Strip(..))=>None,None if !dragging&&y>=STRIP_HEIGHT=>geometry.token.active,_=>None};
        if let Some(tab)=page{Ok(session.with_tab(tab,|app|app.pointer_move(x,y-STRIP_HEIGHT,dragging))?||changed)}else{Ok(changed)}
    }
    pub fn pointer_up(&mut self,session:&mut Session,id:WindowId,x:i32,y:i32)->Result<Outcome,String>{
        let geometry=self.geometry(session,id)?;let index=self.index(id)?;let Some(press)=self.windows[index].press.take()else{return Ok(Outcome::default());};
        if press.token!=geometry.token{if let Target::Page(tab)=press.target{let _=session.with_tab(tab,|app|app.pointer_cancel());}return Ok(Outcome{repaint:true,..Outcome::default()});}
        match press.target{
            Target::Page(tab)=>{let request=session.with_tab(tab,|app|{let action=app.pointer_up(x,y-STRIP_HEIGHT);Self::route(id,tab,app,action)})?;Ok(Outcome{request,repaint:true,..Outcome::default()})},
            Target::Strip(hit,rect)=>{
                if press.moved||(x-press.x).abs()>4||(y-press.y).abs()>4||Self::hit(&geometry,x,y)!=Some((hit,rect)){return Ok(Outcome{repaint:true,..Outcome::default()});}
                match hit{Hit::Select(tab)=>{self.windows[index].preflight()?;session.switch_tab(id,tab)?;self.refresh(session,id)?;Ok(Outcome{repaint:true,..Outcome::default()})},Hit::Close(tab)=>self.close_tab(session,id,tab),Hit::New=>self.new_tab(session,id)}
            },
        }
    }
    pub fn pointer_cancel(&mut self,session:&mut Session,id:WindowId)->Result<(),String>{
        // Pointer/native focus cancellation retains consumed browser keyups.
        // A fresh down of that key clears stale suppression before dispatch.
        let index=self.index(id)?;let press=self.windows[index].press.take();
        let tab=match press.map(|press|press.target){Some(Target::Page(tab))=>Some(tab),_=>session.active(id)?};
        if let Some(tab)=tab{let _=session.with_tab(tab,|app|{app.pointer_cancel();app.pending_surrogate=None;});}Ok(())
    }
    pub fn pointer_leave(&mut self,session:&mut Session,id:WindowId)->Result<bool,String>{
        let index=self.index(id)?;let changed=self.windows[index].hover.take().is_some();
        let page=session.active(id)?;if let Some(tab)=page{Ok(session.with_tab(tab,|app|app.pointer_leave())?||changed)}else{Ok(changed)}
    }
    pub fn wants_hand(&self,session:&Session,id:WindowId,x:i32,y:i32)->Result<bool,String>{
        let geometry=self.geometry(session,id)?;if y<STRIP_HEIGHT{return Ok(Self::hit(&geometry,x,y).is_some());}
        if geometry.token.active.is_none(){return Ok(false);}session.observe_active(id,|app|app.wants_hand(x,y-STRIP_HEIGHT))
    }
    pub fn resize_cursor(&self,session:&Session,id:WindowId,x:i32,y:i32)->Result<bool,String>{
        if y<STRIP_HEIGHT{return Ok(false);}session.observe_active(id,|app|app.inspector_resize_cursor(x,y-STRIP_HEIGHT))
    }
    pub fn new_tab(&mut self,session:&mut Session,id:WindowId)->Result<Outcome,String>{
        let index=self.index(id)?;self.windows[index].preflight()?;let tab=session.new_tab(id)?;session.with_tab(tab,|app|app.focus_address())?;self.refresh(session,id)?;Ok(Outcome{repaint:true,..Outcome::default()})
    }
    pub fn close_tab(&mut self,session:&mut Session,id:WindowId,tab:TabId)->Result<Outcome,String>{
        let index=self.index(id)?;self.windows[index].preflight()?;if !session.window_tabs(id)?.contains(&tab){return Err("This tab is not in the requested browser window.".into());}
        session.close_tab(tab)?;if session.window_tabs(id)?.is_empty(){return self.close_window(session,id);}
        self.refresh(session,id)?;Ok(Outcome{repaint:true,..Outcome::default()})
    }
    pub fn close_window(&mut self,session:&mut Session,id:WindowId)->Result<Outcome,String>{session.close_window(id)?;self.forget(id);Ok(Outcome{closed:Some(id),repaint:true,..Outcome::default()})}
    pub fn key_event(&mut self,session:&mut Session,id:WindowId,input:KeyboardInput)->Result<Outcome,String>{
        self.activate_window(session,id)?;let index=self.index(id)?;let bit=match input.key_code{0x54=>1,0x57=>2,0x09=>4,0x4e=>8,_=>0};
        if !input.down&&bit!=0&&self.windows[index].swallowed&bit!=0{self.windows[index].swallowed&=!bit;return Ok(Outcome{suppress_text:true,..Outcome::default()});}
        if input.down&&bit!=0{self.windows[index].swallowed&=!bit;}
        if input.down&&input.ctrl&&!input.alt&&!input.meta&&bit!=0{
            self.pointer_cancel(session,id)?;self.windows[index].swallowed|=bit;
            let mut result=match input.key_code{
                0x54=>self.new_tab(session,id)?,
                0x57=>{let tab=session.active(id)?.ok_or("This window has no active tab.")?;self.close_tab(session,id,tab)?},
                0x09=>{let tabs=session.window_tabs(id)?.to_vec();let current=session.active(id)?.ok_or("This window has no active tab.")?;let position=tabs.iter().position(|tab|*tab==current).ok_or("Active tab routing is unavailable.")?;
                    let next=if input.shift{(position+tabs.len()-1)%tabs.len()}else{(position+1)%tabs.len()};self.windows[index].preflight()?;session.switch_tab(id,tabs[next])?;self.refresh(session,id)?;Outcome{repaint:true,..Outcome::default()}},
                0x4e=>{let(width,height)=(self.windows[index].width,self.windows[index].height);if self.windows.len()==MAX_WINDOWS{return Err("Browser UI window limit was reached.".into());}self.windows.try_reserve(1).map_err(|_|"Cannot reserve browser window UI.")?;let(window,tab)=session.new_window(width,(height-STRIP_HEIGHT).max(1))?;if let Err(error)=self.resize(session,window,width,height){let _=session.close_window(window);self.forget(window);return Err(error);}self.activate_window(session,window)?;session.with_tab(tab,|app|app.focus_address())?;Outcome{opened:Some((window,tab)),repaint:true,..Outcome::default()}},
                _=>unreachable!(),
            };result.suppress_text=true;return Ok(result);
        }
        let tab=session.active(id)?.ok_or("This window has no active tab.")?;
        let(request,suppress_text)=session.with_tab(tab,|app|{let(action,suppress)=app.key_event(input);(Self::route(id,tab,app,action),suppress)})?;
        Ok(Outcome{request,suppress_text,repaint:true,..Outcome::default()})
    }
    pub fn utf16_input(&mut self,session:&mut Session,id:WindowId,unit:u16)->Result<(),String>{session.with_active(id,|app|app.utf16_input(unit))?;Ok(())}
    pub fn wheel_at(&mut self,session:&mut Session,id:WindowId,x:i32,y:i32,delta:i32,ctrl:bool,shift:bool)->Result<(),String>{
        if y<STRIP_HEIGHT{
            self.pointer_cancel(session,id)?;let geometry=self.geometry(session,id)?;let index=self.index(id)?;let visible=Self::visible(self.windows[index].width,geometry.token.members.len());
            let maximum=geometry.token.members.len().saturating_sub(visible);let next=(geometry.token.first as i64-i64::from(delta.signum())).clamp(0,maximum as i64)as usize;
            if next!=self.windows[index].first{self.windows[index].bump()?;self.windows[index].first=next;self.windows[index].members=geometry.token.members;self.windows[index].active=geometry.token.active;}return Ok(());
        }
        session.with_active(id,|app|app.wheel_at(x,y-STRIP_HEIGHT,delta,ctrl,shift))?;Ok(())
    }
    pub fn download_at(&mut self,session:&mut Session,id:WindowId,x:i32,y:i32)->Result<bool,String>{if y<STRIP_HEIGHT{return Ok(false);}session.with_active(id,|app|app.download_at(x,y-STRIP_HEIGHT))}
    pub fn mouse_navigation(&mut self,session:&mut Session,id:WindowId,forward:bool)->Result<(),String>{self.pointer_cancel(session,id)?;session.with_active(id,|app|{app.invalidate_platform_requests();app.mouse_navigation(forward);})?;Ok(())}
}

#[cfg(test)]
#[path="app_session_ui_tests.rs"]
mod tests;
