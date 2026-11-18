//! Guarded App anchor activation. Session/native owners retain their full typed
//! IDs and the original opaque gesture/context across native reentry.
use super::*;
use crate::scripting::AnchorMouseInput;
use std::rc::Rc;

/// An opaque, one-shot snapshot of an actual rendered anchor before author code.
pub(crate) struct AnchorHit {
    scope:Rc<()>,geometry:Rc<()>,document:u64,source:Location,base:Location,
    node:usize,href:String,download:Option<String>,x:i32,y:i32,
    width:i32,height:i32,scroll:i32,zoom:u32,button:Option<u8>,
}
/// Native menus keep the original snapshot; command selection never re-hits or
/// invokes a fake author click. Logical input/navigation invalidates its scope.
pub(crate) struct AnchorContext(AnchorHit);
/// Source/geometry identity before native menu preparation can reenter. No hit
/// is acquired after a changed source merely because its full owner IDs survive.
pub(crate) struct AnchorContextOrigin {scope:Rc<()>,geometry:Rc<()>}
/// Only an accepted source anchor can construct this capability. The destination
/// keeps its exact content root and source top URL; no POST/policy authority.
pub(crate) struct OwnedLinkTarget {
    location:Location,initiator:Option<crate::url::HttpUrl>,fragment_navigation:bool,
}
pub(crate) enum AnchorTrigger { Pointer(AnchorMouseInput), Context }
pub(crate) enum AnchorDefault {
    Cancelled,
    /// The source App attempted its existing bounded download path.
    Download,
    Link(OwnedLinkTarget,AnchorTrigger),
}
impl App {
    fn anchor_connected(&self,node:usize)->bool{
        if !self.document.nodes.get(node).is_some_and(|entry|entry.namespace==crate::html::Namespace::Html
            &&matches!(&entry.kind,crate::html::NodeKind::Element{tag,..}if tag=="a")){return false;}
        // Main Document node identities are retained in the arena. Reachability,
        // rather than an index's continued presence, rejects detached old nodes.
        let mut seen=vec![false;self.document.nodes.len()];let mut todo=vec![0usize];
        while let Some(current)=todo.pop(){let Some(visited)=seen.get_mut(current)else{return false;};
            if *visited{continue;}*visited=true;if current==node{return true;}
            let Some(entry)=self.document.nodes.get(current)else{return false;};todo.extend(entry.children.iter().copied());
        }false
    }
    fn anchor_surface(&self,x:i32,y:i32)->bool{
        self.view==View::Page&&!self.menu&&self.browser_panel.is_none()
            &&!self.downloads_open()&&!self.bookmark_popup_open()&&self.select_popup.is_none()
            &&x>=0&&x<self.width&&y>=self.content_top()&&y<self.content_bottom()
            &&(x<self.width-12||self.max_scroll()==0)
            &&self.page.control_at(x,y-self.content_top()+self.scroll).is_none()
            &&self.media_hit(x,y).is_none()
            &&!self.scripting.as_ref().is_some_and(|page|page.has_navigation_request())
    }
    fn anchor_snapshot(&self,x:i32,y:i32,button:Option<u8>)->Option<AnchorHit>{
        if !self.anchor_surface(x,y){return None;}
        let page_y=y-self.content_top()+self.scroll;let node=self.page.link_node_at(x,page_y)?;
        if !self.anchor_connected(node){return None;}
        let href=self.document.attr(node,"href")?.to_owned();
        if self.page.link_at(x,page_y)!=Some(href.as_str()){return None;}
        Some(AnchorHit{scope:self.platform_request_scope.clone(),geometry:self.anchor_geometry_scope.clone(),
            document:self.script_epoch,source:self.location.clone(),base:self.base_location.clone(),node,href,
            download:self.document.attr(node,"download").map(str::to_owned),x,y,
            width:self.width,height:self.height,scroll:self.scroll,zoom:self.zoom.to_bits(),button})
    }
    pub(crate) fn anchor_hit(&self,x:i32,y:i32,button:u8)->Option<AnchorHit>{if !matches!(button,0|1){return None;}self.anchor_snapshot(x,y,Some(button))}
    pub(crate) fn anchor_context(&self,x:i32,y:i32)->Option<AnchorContext>{self.anchor_snapshot(x,y,None).map(AnchorContext)}
    /// Preserve the actual primary down grammar before arming a link. A down
    /// which merely closes an overlay cannot capture a newly exposed anchor.
    pub(crate) fn primary_anchor_down(&mut self,x:i32,y:i32,shift:bool)->(PlatformAction,Option<AnchorHit>){
        self.pointer_cancel();let action=self.pointer_down(x,y,shift);
        let text=matches!(&self.drag,Drag::Text{x:start_x,y:start_y,href:Some(_),..}if *start_x==x&&*start_y==y);
        let hit=if text{self.anchor_hit(x,y,0)}else{None};(action,hit)
    }
    /// Middle input is anchor-only; it never enters primary controls/media/text.
    pub(crate) fn middle_anchor_down(&mut self,x:i32,y:i32)->Option<AnchorHit>{
        self.pointer_cancel();self.invalidate_platform_requests();self.anchor_hit(x,y,1)
    }
    pub(crate) fn anchor_hit_valid(&self,hit:&AnchorHit,x:i32,y:i32,dragged:bool,button:u8)->bool{
        !dragged&&(i64::from(x)-i64::from(hit.x)).abs()<=4&&(i64::from(y)-i64::from(hit.y)).abs()<=4
            &&hit.button==Some(button)&&matches!(button,0|1)&&self.anchor_before(hit,x,y)
    }
    pub(crate) fn anchor_context_valid(&self,context:&AnchorContext)->bool{
        let hit=&context.0;self.anchor_before(hit,hit.x,hit.y)
    }
    pub(crate) fn anchor_context_origin(&self)->AnchorContextOrigin{
        AnchorContextOrigin{scope:self.platform_request_scope.clone(),geometry:self.anchor_geometry_scope.clone()}
    }
    pub(crate) fn anchor_context_from_origin(&self,origin:AnchorContextOrigin,x:i32,y:i32)->Option<AnchorContext>{
        if !Rc::ptr_eq(&origin.scope,&self.platform_request_scope)||!Rc::ptr_eq(&origin.geometry,&self.anchor_geometry_scope){return None;}
        self.anchor_context(x,y)
    }
    /// Use the same page status failure surface as the existing follow resolver.
    pub(crate) fn anchor_default_error(&mut self,error:String){self.error(error);}
    fn anchor_source_unchanged(&self,hit:&AnchorHit,scope:&Rc<()>)->bool{
        Rc::ptr_eq(scope,&self.platform_request_scope)&&self.script_epoch==hit.document
            &&self.location==hit.source&&self.view==View::Page
            &&!self.scripting.as_ref().is_some_and(|page|page.has_navigation_request())
    }
    fn anchor_before(&self,hit:&AnchorHit,x:i32,y:i32)->bool{
        self.anchor_source_unchanged(hit,&hit.scope)&&self.base_location==hit.base&&Rc::ptr_eq(&hit.geometry,&self.anchor_geometry_scope)
            &&(self.width,self.height,self.scroll,self.zoom.to_bits())==(hit.width,hit.height,hit.scroll,hit.zoom)
            &&self.anchor_surface(x,y)&&self.anchor_connected(hit.node)
            &&self.document.attr(hit.node,"href")==Some(hit.href.as_str())
            &&self.document.attr(hit.node,"download")==hit.download.as_deref()
            &&self.page.link_node_at(x,y-self.content_top()+self.scroll)==Some(hit.node)
            &&self.page.link_at(x,y-self.content_top()+self.scroll)==Some(hit.href.as_str())
    }
    fn anchor_default_target(&mut self,node:usize,trigger:AnchorTrigger)->Result<AnchorDefault,String>{
        if !self.anchor_connected(node){return Ok(AnchorDefault::Cancelled);}
        let Some(href)=self.document.attr(node,"href").map(str::to_owned)else{return Ok(AnchorDefault::Cancelled);};
        // Resolve only after author callbacks/microtasks. This existing resolver
        // enforces local roots and web/file boundaries; explicit is never used.
        let location=navigation::resolve(&href,&self.base_location)?;
        if let Some(name)=self.document.attr(node,"download").map(str::to_owned){
            self.download_location(location,(!name.is_empty()).then_some(name));return Ok(AnchorDefault::Download);
        }
        Ok(AnchorDefault::Link(OwnedLinkTarget{location,initiator:self.location.url.clone(),fragment_navigation:href.contains('#')},trigger))
    }
    /// Caller supplies the original physical button and whether the gesture ever
    /// dragged. Client coordinates are computed from the App page viewport here;
    /// native screen coordinates and modifiers remain the actual supplied input.
    pub(crate) fn activate_anchor(&mut self,hit:AnchorHit,x:i32,y:i32,dragged:bool,mut input:AnchorMouseInput)->Result<AnchorDefault,String>{
        if !self.anchor_hit_valid(&hit,x,y,dragged,input.button)||!input.screen_x.is_finite()||!input.screen_y.is_finite()
            {return Err("The original anchor gesture is stale or cancelled.".into());}
        self.invalidate_platform_requests();let scope=self.platform_request_scope.clone();
        self.update_script_geometry();self.script_begin_job();input.client_x=f64::from(x)/f64::from(self.zoom);
        input.client_y=f64::from(y-self.content_top())/f64::from(self.zoom);
        let outcome=if let Some(page)=&mut self.scripting{page.anchor_mouse(&mut self.document,&mut self.forms,hit.node,input)}else{crate::scripting::Outcome::default()};
        // No take_navigation peek: the source's queued author request remains
        // available to normal App polling, including when default is cancelled.
        self.apply_history_replacement();if outcome.layout_changed{self.reflow();}
        self.apply_script_scroll();
        self.validate_control_focus();self.sync_editors();
        if outcome.errors!=0{self.status=format!("JavaScript subset: {} error(s). Ctrl+I shows details.",outcome.errors);self.failed=false;}
        if outcome.prevented||!self.anchor_source_unchanged(&hit,&scope){return Ok(AnchorDefault::Cancelled);}
        // The callback may move the anchor or change its href/download. Default
        // follows the same original live node, never a replacement coordinate hit.
        self.anchor_default_target(hit.node,AnchorTrigger::Pointer(input))
    }
    /// Browser-owned Open commands revalidate their exact original menu target.
    /// Authored download retains source ownership; no DOM click is fabricated.
    pub(crate) fn open_anchor_context(&mut self,context:AnchorContext)->Result<AnchorDefault,String>{
        let AnchorContext(hit)=context;if !self.anchor_before(&hit,hit.x,hit.y){return Err("The original anchor menu target is stale.".into());}
        self.invalidate_platform_requests();self.anchor_default_target(hit.node,AnchorTrigger::Context)
    }
    /// Internal destination execution of an already-authorized, one-shot target.
    /// Browser owners still preflight caps and source routes before consuming it.
    pub(crate) fn open_link_target(&mut self,target:OwnedLinkTarget){
        self.navigate_location_mode_with_link_origin(target.location,target.fragment_navigation,false,Some(target.initiator));
    }
}

#[cfg(test)]
#[path="app_anchor_tests.rs"]
mod tests;

#[cfg(test)]
#[path="app_native_link_tests.rs"]
mod native_link_tests;
