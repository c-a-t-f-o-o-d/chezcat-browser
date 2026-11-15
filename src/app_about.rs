//! Offline browser-owned About text and controls. No page script, consent,
//! receipt, runtime Git, external URL or update operation is involved.
use super::*;
use std::borrow::Cow;

pub(super) const SCOPE_TEXT:&str=include_str!("../LICENSE_SCOPE.md");

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub(super) enum Section { Build, License, Scope, Notices }
impl Section {
    pub(super) fn name(self)->&'static str {match self {Self::Build=>"Build",Self::License=>"License",Self::Scope=>"Scope",Self::Notices=>"Notices"}}
    fn text(self)->Cow<'static,str> {match self {
        Self::Build=>Cow::Owned(crate::build_identity::current().safe_copy()),
        Self::License=>Cow::Borrowed(crate::build_identity::license_text()),
        Self::Scope=>Cow::Borrowed(SCOPE_TEXT),
        Self::Notices=>Cow::Borrowed(crate::build_identity::THIRD_PARTY_NOTICES),
    }}
}

pub(super) struct Contents {
    pub(super) section:Section,
    pub(super) text:Cow<'static,str>,
    pub(super) document:Document,
    pub(super) layout:Layout,
    width:i32,
}
impl Contents {
    fn new(section:Section,width:i32,fonts:&crate::fonts::Book)->Self {
        let text=section.text();
        let document=html::parse(&format!("<style>html,body,pre{{margin:0;padding:0}}pre{{font-size:14px;white-space:pre-wrap}}</style><pre>{}</pre>",escape(&text)));
        let width=width.max(1);
        let layout=layout::build_viewport_with_fonts(&document,width,760,1.0,&HashMap::new(),&HashMap::new(),None,fonts);
        Self{section,text,document,layout,width}
    }
    fn reflow(&mut self,width:i32,fonts:&crate::fonts::Book) {
        let width=width.max(1);
        if self.width!=width {
            self.layout=layout::build_viewport_with_fonts(&self.document,width,760,1.0,&HashMap::new(),&HashMap::new(),None,fonts);
            self.width=width;
        }
    }
}

impl App {
    pub fn show_about(&mut self) {
        if self.browser_panel.as_ref().is_some_and(|p|matches!(p.kind,PanelKind::About)) {self.close_browser_panel();return;}
        let mut panel=BrowserPanel::new(PanelKind::About,Action::About(Section::Build));
        panel.about=Some(Contents::new(Section::Build,(self.browser_body().w-16).max(1),&self.base_fonts));
        self.begin_browser_panel(panel);
    }

    fn about_columns(&self)->i32 {
        let margin=20.min((self.width/8).max(0));
        ((self.width-margin*2+6)/100).clamp(1,5)
    }
    pub(super) fn about_footer_height(&self)->i32 {((5+self.about_columns()-1)/self.about_columns())*40+12}

    pub(super) fn about_controls(&self,body:Rect)->Vec<Control> {
        let mut controls=vec![Control{action:Action::Close,rect:Rect{x:(self.width-52).max(0),y:self.chrome_bottom()+12,w:32,h:32},label:"Close",enabled:true},
            Control{action:Action::AboutText,rect:body,label:"Read-only information",enabled:true}];
        let columns=self.about_columns();let width=((body.w-(columns-1)*6)/columns).max(1);
        let top=self.content_bottom()-self.about_footer_height()+6;
        for (index,(action,label)) in [
            (Action::About(Section::Build),"Build"),(Action::About(Section::License),"License"),
            (Action::About(Section::Scope),"Scope"),(Action::About(Section::Notices),"Notices"),
            (Action::AboutCopy,"Copy info")].into_iter().enumerate() {
            let index=index as i32;
            controls.push(Control{action,rect:Rect{x:body.x+(index%columns)*(width+6),y:top+(index/columns)*40,w:width,h:34},label,enabled:true});
        }
        controls
    }
    pub(super) fn sync_about_layout(&mut self) {
        let width=(self.browser_body().w-16).max(1);
        if let Some(contents)=self.browser_panel.as_mut().and_then(|p|p.about.as_mut()) {contents.reflow(width,&self.base_fonts);}
    }
    pub(super) fn paint_about(&self,canvas:&mut Canvas,body:Rect,panel:&BrowserPanel) {
        let Some(contents)=&panel.about else {return;};
        let palette=self.browser_palette();
        ui::rounded_rect(canvas,body.tuple(),5.0,palette.panel);
        for run in &contents.layout.runs {
            let x=body.x+8+run.x;let y=body.y+run.y-panel.scroll;
            if y.saturating_add(run.height())<=body.y||y>=body.y+body.h {continue;}
            if let Some(shape)=&run.shaped {shape.draw(canvas,x,y,palette.foreground);}
            else {canvas.text(x,y,&run.text,run.scale,palette.foreground);}
        }
        if panel.focus==Action::AboutText {ui::rounded_border(canvas,body.tuple(),5.0,1.5,palette.accent);}
    }
    pub(super) fn browser_about_activation(&mut self,action:Action)->Option<PlatformAction> {
        if !self.browser_panel.as_ref().is_some_and(|p|matches!(p.kind,PanelKind::About)) {return None;}
        match action {
            Action::About(section)=>{
                let contents=Contents::new(section,(self.browser_body().w-16).max(1),&self.base_fonts);
                if let Some(panel)=&mut self.browser_panel {panel.about=Some(contents);panel.scroll=0;panel.focus=Action::AboutText;panel.error.clear();}
                self.sync_browser_panel();Some(PlatformAction::None)
            }
            Action::AboutCopy=>Some(PlatformAction::CopyBuildInformation(crate::build_identity::current().safe_copy())),
            Action::AboutText=>Some(PlatformAction::None),
            _=>None,
        }
    }
    pub(super) fn about_key(&mut self,key:u32,ctrl:bool,shift:bool,alt:bool)->Option<PlatformAction> {
        let panel=self.browser_panel.as_ref()?;
        if !matches!(panel.kind,PanelKind::About) {return None;}
        if ctrl&&shift&&!alt&&key==0x43 {return self.browser_about_activation(Action::AboutCopy);}
        if panel.focus!=Action::AboutText||alt {return None;}
        let body=self.browser_body();let max=(self.browser_content_height()-body.h).max(0);
        let scroll=panel.scroll;let page=(body.h-16).max(16);
        let next=match (key,ctrl) {
            (0x24,_)=>0,(0x23,_)=>max,
            (0x26,false)=>scroll.saturating_sub(16),(0x28,false)=>scroll.saturating_add(16),
            (0x21,false)=>scroll.saturating_sub(page),(0x22,false)=>scroll.saturating_add(page),
            (0x20,false)=>if shift{scroll.saturating_sub(page)}else{scroll.saturating_add(page)},
            _=>return None,
        }.clamp(0,max);
        if let Some(panel)=&mut self.browser_panel {panel.scroll=next;panel.hover=None;}
        Some(PlatformAction::None)
    }

    /// Fixed native controls only. The helper changes focus/visibility, then
    /// requires the same real hit/press/release path as owned settings input.
    pub(in crate::app) fn input_about_control(&mut self,name:&str)->Result<PlatformAction,String> {
        let action=match name {"Build"=>Action::About(Section::Build),"License"=>Action::About(Section::License),
            "Scope"=>Action::About(Section::Scope),"Notices"=>Action::About(Section::Notices),
            "Copy"=>Action::AboutCopy,"Close"=>Action::Close,"Text"=>Action::AboutText,_=>return Err("Unknown About control.".into())};
        let panel=self.browser_panel.as_mut().filter(|p|matches!(p.kind,PanelKind::About)).ok_or("About panel is not open.")?;
        panel.focus=action;self.ensure_browser_focus_visible();
        let control=self.browser_controls().into_iter().find(|c|c.action==action&&c.enabled).ok_or("About control unavailable.")?;
        let (x,y)=(control.rect.x+control.rect.w/2,control.rect.y+control.rect.h/2);
        if x<0||x>=self.width||y<self.chrome_bottom()||y>=self.content_bottom()||self.browser_action_at(x,y)!=Some(action) {return Err("About control is not a visible native hit.".into());}
        if !matches!(self.pointer_down(x,y,false),PlatformAction::None) {return Err("About press requested an unexpected platform operation.".into());}
        Ok(self.pointer_up(x,y))
    }
    pub(in crate::app) fn about_displayed_text(&self)->Option<&str> {
        self.browser_panel.as_ref()?.about.as_ref().map(|contents|contents.text.as_ref())
    }
    pub(in crate::app) fn about_state_report(&self)->Option<String> {
        let panel=self.browser_panel.as_ref()?;let contents=panel.about.as_ref()?;
        let digest:String=crate::crypto_hash::sha256(contents.text.as_bytes()).iter().map(|byte|format!("{byte:02x}")).collect();
        Some(format!("scope=owned App panel and payload; native clipboard not exercised\nsection={}\ntext_bytes={}\ntext_sha256={digest}\nscroll={}\ntext_height={}\nfocus={:?}\n",contents.section.name(),contents.text.len(),panel.scroll,contents.layout.height,panel.focus))
    }
}
