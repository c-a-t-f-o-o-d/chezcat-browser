//! Owned native-operation continuations. The native shell keeps its full typed
//! WindowId/TabId separately and resolves the original App before completion.
//! No App/Editor reference or raw pointer survives capture; no OS API runs here.
use super::*;
use std::rc::Rc;

/// One native action and its optional one-shot original-editor continuation.
/// Copy/folder actions already own their data and require no App mutation.
pub struct PlatformRequest {
    action: PlatformAction,
    continuation: Option<Continuation>,
}
impl PlatformRequest {
    pub fn action(&self) -> &PlatformAction { &self.action }
}

/// Only successful native results can mutate the original logical target.
/// Cancellation or a native error drops the request without completing it.
pub enum PlatformCompletion { CutWritten, Paste(String), OpenFile(PathBuf) }

#[derive(Clone, PartialEq, Eq)]
struct EditorState { chars: Vec<char>, cursor: usize, anchor: usize }
impl EditorState {
    fn capture(editor: &Editor) -> Self {
        Self {chars:editor.chars.clone(),cursor:editor.cursor,anchor:editor.anchor}
    }
}
#[derive(Clone, PartialEq, Eq)]
struct ControlState {
    owner:Option<usize>,kind:Kind,name:String,disabled:bool,readonly:bool,max_length:usize,
}
#[derive(Clone, PartialEq, Eq)]
enum Target {
    Main {focus:Focus,editor:EditorState,control:Option<ControlState>},
    Panel {panel:browser_ui::PlatformPanelState,editor:EditorState},
    Inspector {inspector:inspector::PlatformInspectorState,editor:EditorState},
}
#[derive(PartialEq, Eq)]
struct Context {
    document:u64,request:u64,committed:Option<u64>,pending:Option<(u64,bool)>,
    confirmation:Option<(u64,bool)>,location:String,focus:Focus,view:View,find_open:bool,
    panel:Option<browser_ui::PlatformPanelState>,inspector:inspector::PlatformInspectorState,
    selection:Option<(usize,usize,usize,usize)>,menu:bool,chrome_focus:bool,
    downloads:bool,bookmark_popup:bool,pending_surrogate:Option<u16>,
}
#[derive(Clone, Copy)]
enum Origin { Main, Inspector }
struct Continuation {scope:Rc<()>,context:Context,target:Option<Target>,origin:Origin}

impl App {
    /// Invalidate on a real logical activation/input/navigation change, never
    /// merely because an owned native dialog temporarily takes OS focus.
    pub fn invalidate_platform_requests(&mut self) {
        self.platform_request_scope=Rc::new(());
    }

    pub fn platform_request(&mut self, action:PlatformAction) -> PlatformRequest {
        self.capture_platform_request(action,Origin::Main)
    }
    pub fn inspector_platform_request(&mut self, action:PlatformAction) -> PlatformRequest {
        self.capture_platform_request(action,Origin::Inspector)
    }
    fn platform_context(&self) -> Context {
        Context {
            document:self.script_epoch,request:self.next_request,committed:self.committed_request,
            pending:self.pending.as_ref().map(|pending|(pending.id,pending.committed)),
            confirmation:self.resubmission.as_ref().map(|question|(question.token,question.offered)),
            location:self.location.display(),focus:self.focus,view:self.view,find_open:self.find_open,
            panel:self.browser_panel.as_ref().map(BrowserPanel::platform_state),
            inspector:self.inspector.platform_state(),
            selection:self.selection.map(|selection|(selection.start.run,selection.start.offset,selection.end.run,selection.end.offset)),
            menu:self.menu,chrome_focus:self.chrome.focus.is_some(),downloads:self.downloads_open(),
            bookmark_popup:self.bookmark_popup_open(),pending_surrogate:self.pending_surrogate,
        }
    }
    fn platform_editor_target(&self,origin:Origin,editable:bool)->Option<Target> {
        let inspector=|app:&Self|app.inspector.platform_editor().map(|editor|Target::Inspector {
            inspector:app.inspector.platform_state(),editor:EditorState::capture(editor),
        });
        if matches!(origin,Origin::Inspector) { return inspector(self); }
        if self.downloads_open() || self.bookmark_popup_open() {return None;}
        if self.inspector.focused && self.inspector.mode!=inspector::Mode::Closed {return inspector(self);}
        if self.menu || self.chrome.focus.is_some() {return None;}
        if let Some(panel)=&self.browser_panel {
            return panel.platform_editor().map(|editor|Target::Panel {panel:panel.platform_state(),editor:EditorState::capture(editor)});
        }
        let (editor,control)=match self.focus {
            Focus::Address=>(&self.address,None),
            Focus::Find if self.find_open=>(&self.find,None),
            Focus::Control(node)=>{
                let control=self.forms.controls.get(&node)?;
                if !matches!(control.kind,Kind::Text|Kind::TextArea) || editable&&!control.editable()
                    || self.document.nodes.get(node).is_none()
                    || !self.page.controls.iter().any(|rect|rect.node==node) {return None;}
                (&control.value,Some(ControlState {owner:control.owner,kind:control.kind,name:control.name.clone(),
                    disabled:control.disabled,readonly:control.readonly,max_length:control.max_length}))
            },
            _=>return None,
        };
        Some(Target::Main {focus:self.focus,editor:EditorState::capture(editor),control})
    }
    fn capture_platform_request(&mut self,action:PlatformAction,origin:Origin)->PlatformRequest {
        let needs_editor=matches!(&action,PlatformAction::Cut(_)|PlatformAction::Paste);
        let guarded=needs_editor||matches!(&action,PlatformAction::OpenFile);
        let continuation=if guarded {
            // A newer request supersedes an older suspended operation even if
            // their original fields and text happened to be identical.
            self.invalidate_platform_requests();
            let target=self.platform_editor_target(origin,needs_editor);
            let valid=if let PlatformAction::Cut(text)=&action {
                let selected=match &target {
                    Some(Target::Main {editor,..})|Some(Target::Panel {editor,..})|Some(Target::Inspector {editor,..})=>{
                        let a=editor.cursor.min(editor.anchor);let b=editor.cursor.max(editor.anchor);
                        editor.chars.get(a..b).map(|chars|chars.iter().collect::<String>())
                    },
                    None=>None,
                };
                !text.is_empty()&&selected.as_ref()==Some(text)
            }else{!needs_editor||target.is_some()};
            valid.then(||Continuation {scope:self.platform_request_scope.clone(),context:self.platform_context(),target,origin})
        }else{None};
        PlatformRequest {action,continuation}
    }

    /// Called only after the native shell re-resolves the captured full TabId.
    /// A stale or foreign request refuses before editing, navigation or script
    /// input. The consumed token cannot be replayed by safe callers.
    pub fn complete_platform_request(&mut self,request:PlatformRequest,result:PlatformCompletion)->bool {
        let Some(continuation)=request.continuation else{return false;};
        let matching=matches!((&request.action,&result),
            (PlatformAction::Cut(_),PlatformCompletion::CutWritten)
            |(PlatformAction::Paste,PlatformCompletion::Paste(_))
            |(PlatformAction::OpenFile,PlatformCompletion::OpenFile(_)));
        if !matching || !Rc::ptr_eq(&continuation.scope,&self.platform_request_scope)
            || continuation.context!=self.platform_context()
            || continuation.target!=self.platform_editor_target(continuation.origin,matches!(&request.action,PlatformAction::Cut(_)|PlatformAction::Paste)) {
            return false;
        }
        self.invalidate_platform_requests();
        match (request.action,result,continuation.origin) {
            (PlatformAction::Cut(text),PlatformCompletion::CutWritten,Origin::Main)=>self.cut_copied(&text),
            (PlatformAction::Cut(text),PlatformCompletion::CutWritten,Origin::Inspector)=>self.inspector_cut_copied(&text),
            (PlatformAction::Paste,PlatformCompletion::Paste(text),Origin::Main)=>self.paste(&text),
            (PlatformAction::Paste,PlatformCompletion::Paste(text),Origin::Inspector)=>self.inspector_paste(&text),
            (PlatformAction::OpenFile,PlatformCompletion::OpenFile(path),_)=>self.open(&path),
            _=>return false,
        }
        true
    }
}

#[cfg(test)]
#[path="app_platform_continuation_tests.rs"]
mod tests;
