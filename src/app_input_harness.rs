//! Local development input driver, using the same App paths as the native shell.
//! No JavaScript evaluation, direct form mutation or direct link navigation.
use super::{App, Focus, PlatformAction};
use crate::scripting::KeyboardInput;
use crate::scripting::AnchorMouseInput;
use super::anchor::{AnchorDefault,AnchorTrigger};
use std::io::Write;
use std::path::Path;
use std::time::Duration;

// The canvas uses the existing checked Canvas allocation bound. Text comes
// from the same bounded App/document reports; only one snapshot is retained.
struct InputSnapshot {
    canvas: crate::paint::Canvas,
    activity: String,
    source: String,
    layout: String,
    about: Option<(String, Option<String>)>,
}

fn input_snapshot_token(value: &str, limit: usize) -> bool {
    !value.is_empty() && value.len() <= limit
        && value.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

impl InputSnapshot {
    fn write(mut self, output: &Path, name: &str) -> Result<(), String> {
        // Keep all pre-action text evidence even if raster output fails.
        let rendered = (|| -> Result<(), String> {
            if self.canvas.pixels.is_empty() { return Err("Could not allocate harness render surface.".into()); }
            let mut file = std::io::BufWriter::new(std::fs::File::create(output.join(format!("{name}.ppm"))).map_err(|e| e.to_string())?);
            write!(file, "P6\n{} {}\n255\n", self.canvas.width, self.canvas.height).map_err(|e| e.to_string())?;
            for pixel in &self.canvas.pixels { file.write_all(&[(pixel >> 16) as u8, (pixel >> 8) as u8, *pixel as u8]).map_err(|e| e.to_string())?; }
            file.flush().map_err(|e| e.to_string())
        })();
        self.activity.push_str(&format!("Stage paired_ppm_written={}\n", u8::from(rendered.is_ok())));
        std::fs::write(output.join(format!("{name}.activity.txt")), self.activity).map_err(|e| e.to_string())?;
        std::fs::write(output.join(format!("{name}.source.html")), self.source).map_err(|e| e.to_string())?;
        std::fs::write(output.join(format!("{name}.layout.txt")), self.layout).map_err(|e| e.to_string())?;
        if let Some((text, state)) = self.about {
            std::fs::write(output.join(format!("{name}.about.txt")), text).map_err(|e| e.to_string())?;
            std::fs::write(output.join(format!("{name}.about-state.txt")), state.ok_or("About state unavailable.")?).map_err(|e| e.to_string())?;
        }
        rendered
    }
}

impl App {
    fn input_access_blocked(&self) -> bool {
        let status_denied = self.network.snapshot().rows.iter().any(|row|
            row.kind == "document" && matches!(row.status, Some(401 | 403 | 429)));
        let rendered = self.page.runs.iter().map(|run| run.text.as_str()).collect::<Vec<_>>().join(" ").to_lowercase();
        status_denied || ["unusual traffic", "verify you are human", "verify that you're not a robot",
            "verify that you are not a robot", "confirm that you are not a robot", "access denied",
            "access to this page has been denied", "complete the captcha", "before you continue to google",
            "rate limit exceeded", "too many requests"].iter().any(|signal| rendered.contains(signal))
    }

    fn input_guard_action(&self, action: &str) -> Result<(), String> {
        if matches!(action, "fill" | "click" | "snapshot-click" | "link" | "link-target" | "key" | "back" | "address" | "media-surface" | "media-probe") && self.input_access_blocked() {
            return Err("Visible access denial/human verification or document rate limit; stopping without retry.".into());
        }
        Ok(())
    }

    pub fn run_input_script(&mut self, script: &Path, output: &Path) -> Result<(), String> {
        if std::fs::metadata(script).map_err(|e| e.to_string())?.len() > 16 * 1024 {
            return Err("Development input script exceeds 16 KiB.".into());
        }
        let script = std::fs::read_to_string(script).map_err(|e| e.to_string())?;
        let lines: Vec<_> = script.lines().filter(|line| !line.is_empty() && !line.starts_with('#')).collect();
        if lines.len() > 64 { return Err("Development input script exceeds 64 actions.".into()); }
        if let Some(parent) = output.parent() { std::fs::create_dir_all(parent).map_err(|e| e.to_string())?; }
        std::fs::create_dir(output).map_err(|e| format!("Input output directory must be new: {e}"))?;
        let mut log = std::fs::File::create(output.join("input.log")).map_err(|e| e.to_string())?;
        writeln!(log, "Same-engine application harness; no native desktop interaction.\nExecutable: {:?}\nJavaScript enabled: {}", std::env::current_exe(), self.scripts_enabled).map_err(|e| e.to_string())?;
        self.input_snapshot(output, "00-initial")?;
        for (index, line) in lines.iter().enumerate() {
            writeln!(log, "{}: {}", index + 1, line).map_err(|e| e.to_string())?;
            log.flush().map_err(|e| e.to_string())?;
            let fields: Vec<_> = line.splitn(3, '\t').collect();
            let result = (|| {
                self.input_guard_action(fields[0])?;
                match fields.as_slice() {
                    ["provider", endpoint] => {
                        self.input_search_provider=Some(crate::search::local_fixture(endpoint)?);
                        Ok(())
                    }
                    ["settings", action] => self.input_settings_action(action),
                    ["about", "Open"] => {
                        self.input_key("f","KeyF",0x46,false,true,None)?;
                        self.input_key("End","End",0x23,false,false,None)?;
                        self.input_key("Enter","Enter",0x0d,false,false,None)?;
                        if self.about_displayed_text().is_none() {return Err("Native menu did not open About.".into());}
                        Ok(())
                    },
                    ["about", name] if matches!(*name,"End"|"Home") => {
                        if !matches!(self.input_about_control("Text")?,PlatformAction::None) {return Err("Read-only About focus requested a platform operation.".into());}
                        let code=if *name=="End" {0x23}else{0x24};
                        self.input_key(name,name,code,false,false,None)
                    },
                    ["about", name] => {
                        let action=self.input_about_control(name)?;
                        match action {
                            PlatformAction::None if *name!="Copy"=>Ok(()),
                            PlatformAction::CopyBuildInformation(text) if *name=="Copy" && text==crate::build_identity::current().safe_copy()=>{
                                let target=output.join(format!("about-copy-{:02}.txt",index+1));
                                let mut file=std::fs::OpenOptions::new().write(true).create_new(true).open(target).map_err(|e|e.to_string())?;
                                file.write_all(text.as_bytes()).map_err(|e|e.to_string())?;file.flush().map_err(|e|e.to_string())?;
                                writeln!(log,"Captured actual CopyBuildInformation App payload; native clipboard was not called.").map_err(|e|e.to_string())?;
                                Ok(())
                            },
                            _=>Err("About input returned an unexpected platform action; clipboard not touched.".into()),
                        }
                    },
                    ["media", action] => {let (x,y)=self.input_media_action(action)?;writeln!(log,"Pointer native media control={action} x={x} y={y}").map_err(|e|e.to_string())?;Ok(())},
                    ["media-surface"] => {
                        let (x,y)=self.input_video_point(false)?;
                        let down=self.pointer_down(x,y,false);let up=self.pointer_up(x,y);
                        if !matches!(down,PlatformAction::None)||!matches!(up,PlatformAction::None) {return Err("Video surface input requested an external platform action.".into());}
                        if self.focus!=Focus::Page {return Err("Actual video surface click did not focus the page.".into());}
                        writeln!(log,"Pointer native video surface x={x} y={y} focus=Page").map_err(|e|e.to_string())?;Ok(())
                    },
                    ["media-probe",expected] if matches!(*expected,"hidden"|"visible") => {
                        let (x,y)=self.input_video_point(true)?;
                        let hit=self.media_hit(x,y);
                        writeln!(log,"Media native hit probe expected={expected} hit={hit:?} x={x} y={y}").map_err(|e|e.to_string())?;
                        if hit.is_some()!=(*expected=="visible") {return Err("Actual media hit test differs from expected visibility.".into());}
                        Ok(())
                    },
                    ["address", text] if text.chars().count()<=4096 => {
                        self.input_key("l","KeyL",0x4c,true,false,None)?;
                        if self.focus!=Focus::Address { return Err("Address shortcut did not focus browser chrome.".into()); }
                        self.input_key("a","KeyA",0x41,true,false,None)?;
                        for ch in text.chars() { self.input_key(&ch.to_string(),"Unidentified",0,false,false,Some(ch))?; }
                        if self.address.text()!=*text {return Err("Actual address input differs from requested text.".into());}
                        Ok(())
                    }
                    ["snapshot", name] if input_snapshot_token(name, 48) => {
                        self.input_snapshot(output, &format!("{:02}-{name}", index + 1))
                    }
                    ["snapshot-click", name, control] if input_snapshot_token(name, 48) && input_snapshot_token(control, 48) => {
                        self.input_snapshot_click(output, &format!("{:02}-{name}", index + 1), control, &mut log)
                    }
                    ["fill", name, text] if text.chars().count() <= 512 => {
                        self.input_click_control(name, true, &mut log)?;
                        self.input_key("a", "KeyA", 0x41, true, false, None)?;
                        for ch in text.chars() {
                            let key = ch.to_string();
                            let code = if ch.is_ascii_alphabetic() { format!("Key{}", ch.to_ascii_uppercase()) }
                                else if ch.is_ascii_digit() { format!("Digit{ch}") }
                                else if ch == ' ' { "Space".into() } else { "Unidentified".into() };
                            let key_code = if ch.is_ascii_alphabetic() { ch.to_ascii_uppercase() as u32 }
                                else if ch.is_ascii_digit() || ch == ' ' { ch as u32 } else { 0 };
                            self.input_key(&key, &code, key_code, false, false, Some(ch))?;
                        }
                        let Focus::Control(node) = self.focus else { return Err("Control lost focus while typing.".into()) };
                        let actual = self.forms.controls.get(&node).ok_or("Focused control disappeared")?.value.text();
                        writeln!(log, "Typed control value: {actual:?}").map_err(|e| e.to_string())?;
                        if actual != *text { return Err("Actual input value differs from requested text.".into()); }
                        Ok(())
                    }
                    ["click", name] => self.input_click_control(name, false, &mut log),
                    ["key", "Enter"] => self.input_key("Enter", "Enter", 0x0d, false, false, None),
                    ["key", "Space"] => self.input_key(" ", "Space", 0x20, false, false, None),
                    ["back"] => self.input_key("ArrowLeft", "ArrowLeft", 0x25, false, true, None),
                    ["link", needle] if !needle.is_empty() => self.input_click_link(needle, false, &mut log),
                    ["link-target", needle] if !needle.is_empty() && needle.len()<=256 => self.input_click_link(needle, true, &mut log),
                    ["wait", value] => {
                        let millis: u64 = value.parse().map_err(|_| "Invalid wait milliseconds")?;
                        if millis > 10000 { return Err("Wait exceeds 10000 milliseconds.".into()); }
                        // Explicit input-script waits honor wall time even when
                        // the page has no queued timers (e.g. paused video).
                        let deadline=std::time::Instant::now()+Duration::from_millis(millis);
                        while std::time::Instant::now()<deadline {
                            self.poll_load();self.poll_page_timer();
                            std::thread::sleep(deadline.saturating_duration_since(std::time::Instant::now()).min(Duration::from_millis(10)));
                        }
                        Ok(())
                    }
                    _ => Err("Unknown/invalid development input action.".into()),
                }
            })();
            if let Err(error) = result {
                writeln!(log, "FAILED: {error}").map_err(|e| e.to_string())?;
                log.flush().map_err(|e| e.to_string())?;
                self.input_snapshot(output, "failed")?;
                return Err(error);
            }
            self.wait_for_load();
            self.wait_for_script_timers(Duration::from_millis(250));
            if self.failed {
                self.input_snapshot(output, "navigation-failed")?;
                return Err(self.status.clone());
            }
        }
        self.input_snapshot(output, "final")?;
        writeln!(log, "All requested input actions completed; inspect evidence before claiming a search pass.").map_err(|e| e.to_string())?;
        Ok(())
    }

    // Owned development input uses the real rendered authored VIDEO box, not
    // arbitrary desktop coordinates or a fabricated controls rectangle. The
    // probe only queries the same hit test as native pointer delivery.
    fn input_video_point(&self,bar:bool)->Result<(i32,i32),String> {
        if self.view!=super::View::Page||self.menu||self.browser_panel.is_some()||self.downloads_open()
            ||self.embedded_video.is_none()||self.embedded_video_retired() {
            return Err("A current unobstructed authored video page is required.".into());
        }
        let node=self.media.as_ref().ok_or("No actual video player is installed.")?.node;
        let (x,y,width,height)=self.page.items.iter().find_map(|item|match item {
            crate::layout::PaintItem::Image {node:owner,x,y,width,height,..} if *owner==node=>Some((*x,*y,*width,*height)),
            _=>None,
        }).ok_or("The actual video has no rendered image box.")?;
        if width<200||height<44 {return Err("The actual video box is too small for this control probe.".into());}
        let px=i64::from(x)+if bar {40}else{i64::from(width)/2};
        let py=i64::from(y)+if bar {i64::from(height)-28}else{i64::from(height)/2};
        let screen_y=py+i64::from(self.content_top())-i64::from(self.scroll);
        if px<0||px>=i64::from(self.width)||screen_y<i64::from(self.content_top())||screen_y>=i64::from(self.content_bottom()) {
            return Err("The actual video probe point is outside the visible viewport.".into());
        }
        let x=i32::try_from(px).map_err(|_|"Video probe coordinate overflow.")?;
        let page_y=i32::try_from(py).map_err(|_|"Video probe coordinate overflow.")?;
        let y=i32::try_from(screen_y).map_err(|_|"Video probe coordinate overflow.")?;
        if !self.page.image_topmost_at(node,x,page_y) {
            return Err("The actual video probe point is not a visible image hit target.".into());
        }
        Ok((x,y))
    }

    fn input_key(&mut self, key: &str, code: &str, key_code: u32, ctrl: bool, alt: bool, text: Option<char>) -> Result<(), String> {
        let mut input = KeyboardInput { down: true, key: key.into(), code: code.into(), key_code, ctrl, alt, ..KeyboardInput::default() };
        let (action, suppressed) = self.key_event(input.clone());
        if !matches!(action, PlatformAction::None) { return Err("Input requested an external platform action.".into()); }
        if !suppressed { if let Some(ch) = text { self.text_input(ch); } }
        input.down = false;
        let (action, _) = self.key_event(input);
        if !matches!(action, PlatformAction::None) { return Err("Key release requested an external platform action.".into()); }
        Ok(())
    }

    fn input_click_control(&mut self, name: &str, require_focus: bool, log: &mut impl Write) -> Result<(), String> {
        let node = self.page.controls.iter().find(|rect| self.document.attr(rect.node, "name") == Some(name))
            .ok_or_else(|| format!("No rendered control named {name:?}"))?.node;
        let y = self.page.controls.iter().find(|rect| rect.node == node).unwrap().y;
        self.scroll_by(y - self.scroll - self.viewport_height() / 3);
        let rect = self.page.controls.iter().find(|rect| rect.node == node).ok_or("Control disappeared while scrolling")?;
        let (x, page_y) = (rect.x + (rect.width / 2).max(1), rect.y + (rect.height / 2).max(1));
        let y = page_y + self.content_top() - self.scroll;
        if x < 0 || x >= self.width || y < self.content_top() || y >= self.content_bottom()
            || self.page.control_at(x, page_y) != Some(node) { return Err("Control is not a visible hit target.".into()); }
        writeln!(log, "Pointer control node={node} x={x} y={y}").map_err(|e| e.to_string())?;
        self.pointer_down(x, y, false);
        self.pointer_up(x, y);
        if require_focus && self.focus != Focus::Control(node) { return Err("Pointer click did not focus the control.".into()); }
        Ok(())
    }

    fn input_click_link(&mut self, needle: &str, by_target: bool, log: &mut impl Write) -> Result<(), String> {
        let index = self.page.runs.iter().position(|run| run.link.is_some() && if by_target {
            run.link.and_then(|index|self.page.link_targets().nth(index)).is_some_and(|target|target.contains(needle))
        } else {run.text.contains(needle)})
            .ok_or_else(|| format!("No rendered link {} containing {needle:?}",if by_target {"target"} else {"title"}))?;
        let y = self.page.runs[index].y;
        self.scroll_by(y - self.scroll - self.viewport_height() / 3);
        let run = &self.page.runs[index];
        let (x, page_y) = (run.x + (run.width() / 2).max(1), run.y + (run.height() / 2).max(1));
        let expected = self.page.link_targets().nth(run.link.unwrap()).ok_or("Link target absent")?.to_owned();
        let y = page_y + self.content_top() - self.scroll;
        if x < 0 || x >= self.width || y < self.content_top() || y >= self.content_bottom()
            || self.page.link_at(x, page_y) != Some(expected.as_str()) { return Err("Rendered link is not a visible hit target.".into()); }
        writeln!(log, "Pointer link text={:?} href={expected:?} x={x} y={y}", run.text).map_err(|e| e.to_string())?;
        let(action,hit)=self.primary_anchor_down(x,y,false);
        if !matches!(action,PlatformAction::None) {
            self.pointer_cancel();
            return Err("The single-App link driver cannot realize this native press action.".into());
        }
        // The software driver's coordinate system is the App canvas. It does
        // not query or claim a physical desktop/window position.
        let input=AnchorMouseInput {screen_x:f64::from(x),screen_y:f64::from(y),..AnchorMouseInput::default()};
        writeln!(log,"Primary link input button=0 modifiers=none software_screen_x={x} software_screen_y={y}")
            .map_err(|error|{self.pointer_cancel();error.to_string()})?;
        let(action,default)=self.release_anchor_pointer(hit,x,y,false,input);
        if !matches!(action,PlatformAction::None) {
            return Err("The single-App link driver cannot realize this native release action.".into());
        }
        match default {
            Some(AnchorDefault::Cancelled)=>writeln!(log,"Primary link outcome=cancelled").map_err(|e|e.to_string()),
            Some(AnchorDefault::Download)=>writeln!(log,"Primary link outcome=download").map_err(|e|e.to_string()),
            Some(AnchorDefault::Link(target,AnchorTrigger::Pointer(actual)))=>{
                if actual.button!=0 || actual.ctrl || actual.shift {
                    return Err("The single-App link driver cannot realize a non-current tab/window destination.".into());
                }
                self.open_link_target(target);
                writeln!(log,"Primary link outcome=current").map_err(|e|e.to_string())
            },
            Some(AnchorDefault::Link(_,AnchorTrigger::Context))=>Err("The link driver did not issue a context-menu command.".into()),
            None=>Err("The original rendered link press was not eligible or its release was refused.".into()),
        }
    }

    fn prepare_input_snapshot(&self) -> InputSnapshot {
        // Draw and sample the corresponding reports before any output I/O.
        // Real playback continues; no clock, task or media state is suspended.
        let canvas = self.draw();
        let activity = self.activity_report();
        let source = self.document_source().to_owned();
        let mut layout = String::new();
        for run in &self.page.runs {
            let target = run.link.and_then(|index| self.page.link_targets().nth(index));
            layout.push_str(&format!("x={} y={} w={} h={} text={:?} href={target:?}\n", run.x, run.y, run.width(), run.height(), run.text));
        }
        for rect in &self.page.controls {
            let value = self.forms.controls.get(&rect.node).map(|control| control.value.text());
            layout.push_str(&format!("control={} name={:?} value={value:?} x={} y={} w={} h={}\n", rect.node, self.document.attr(rect.node, "name"), rect.x, rect.y, rect.width, rect.height));
        }
        let about = self.about_displayed_text().map(|text| (text.to_owned(), self.about_state_report()));
        InputSnapshot { canvas, activity, source, layout, about }
    }

    fn input_snapshot(&self, output: &Path, name: &str) -> Result<(), String> {
        self.prepare_input_snapshot().write(output, name)
    }

    fn input_snapshot_click(&mut self, output: &Path, name: &str, control: &str, log: &mut impl Write) -> Result<(), String> {
        // The script's raw label/control are each <=48 ASCII token bytes;
        // the generated label also has its bounded two-digit action prefix.
        if !input_snapshot_token(name, 51) || !input_snapshot_token(control, 48) {
            return Err("Invalid bounded snapshot-click label or control name.".into());
        }
        self.input_guard_action("snapshot-click")?;
        let mut click_log = Vec::new();
        click_log.try_reserve(128).map_err(|_| "Snapshot-click log reservation failed.")?;
        let snapshot = self.prepare_input_snapshot();
        if snapshot.canvas.pixels.is_empty() || snapshot.about.as_ref().is_some_and(|(_, state)| state.is_none()) {
            // Preserve the normal snapshot's text/error evidence without input.
            return snapshot.write(output, name);
        }
        // The same trusted App pointer path runs before raster/text/log writes
        // and before the outer driver's per-action settling. No WM shortcut.
        let clicked = self.input_click_control(control, false, &mut click_log);
        let written = snapshot.write(output, name);
        let logged = log.write_all(&click_log).map_err(|error| error.to_string());
        let mut errors = Vec::new();
        if let Err(error) = clicked { errors.push(error); }
        if let Err(error) = written { errors.push(format!("Snapshot output failed: {error}")); }
        if let Err(error) = logged { errors.push(format!("Snapshot-click log failed: {error}")); }
        if errors.is_empty() { Ok(()) } else { Err(errors.join("; ")) }
    }

}

#[cfg(test)]
#[path = "app_link_driver_tests.rs"]
mod link_driver_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_guard_uses_document_status_and_keeps_compatibility_response_distinct() {
        for status in [401,403,429] {
            let app=App::new();
            app.network.begin("GET","https://fixture.test/","document").response(status);
            assert!(app.input_access_blocked());
        }
        let mut app=App::new();
        app.network.begin("GET","https://fixture.test/font.woff","font").response(403);
        app.document=crate::html::parse("<p>Update your browser. Your browser is not supported anymore.</p>");
        app.forms=crate::forms::Forms::new(&app.document);
        app.reflow();
        assert!(!app.input_access_blocked(),"Resource denial and compatibility text do not deny the whole search service");
        app.document=crate::html::parse("<p>Our systems have detected unusual traffic. Verify that you are not a robot.</p>");
        app.forms=crate::forms::Forms::new(&app.document);
        app.reflow();
        assert!(app.input_access_blocked());
    }


    struct SnapshotFiles(std::path::PathBuf);
    impl SnapshotFiles {
        fn new() -> Self {
            let mut random = [0u8; 8];
            crate::random::fill(&mut random).unwrap();
            let suffix: String = random.iter().map(|byte| format!("{byte:02x}")).collect();
            let root = std::env::temp_dir().join(format!("chezcat-snapshot-click-{suffix}"));
            std::fs::create_dir(&root).unwrap();
            std::fs::write(root.join("source.html"), r#"<!doctype html><title>Before snapshot click</title><body><button id=change name=change type=button>Change</button><p id=marker>BEFORE_MARKER</p><script>
document.getElementById('change').onclick=function(e){if(!e.isTrusted)throw Error('untrusted snapshot click');document.title='After snapshot click';document.getElementById('marker').textContent='AFTER_MARKER';};
</script></body>"#).unwrap();
            Self(root)
        }
        fn app(&self) -> App {
            let mut app = App::new();
            app.resize(640, 480);
            app.open(&self.0.join("source.html"));
            assert!(!app.failed, "{}", app.status);
            assert_eq!(app.document.title, "Before snapshot click");
            assert!(app.scripting.as_ref().unwrap().report().contains("0 error(s)."));
            app
        }
    }
    impl Drop for SnapshotFiles {
        fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); }
    }

    #[test]
    fn input_snapshot_click_keeps_pre_click_pixels_and_text_and_clicks_before_output_failure() {
        let files = SnapshotFiles::new();
        for collision in [false, true] {
            let mut app = files.app();
            let before = app.draw();
            let original_source = app.document_source().to_owned();
            let output = files.0.join(if collision { "blocked-output" } else { "output" });
            std::fs::create_dir(&output).unwrap();
            if collision { std::fs::create_dir(output.join("before.ppm")).unwrap(); }
            let mut log = Vec::new();
            let result = app.input_snapshot_click(&output, "before", "change", &mut log);
            assert_eq!(result.is_err(), collision);
            assert_eq!(app.document.title, "After snapshot click", "Output failure must not move output ahead of the real click");
            assert!(app.page.runs.iter().any(|run| run.text.contains("AFTER_MARKER")));
            assert!(app.scripting.as_ref().unwrap().report().contains("0 error(s)."));
            assert!(String::from_utf8(log).unwrap().contains("Pointer control node="));
            let activity = std::fs::read_to_string(output.join("before.activity.txt")).unwrap();
            assert!(activity.contains("Page title: Before snapshot click"));
            assert!(!activity.contains("Page title: After snapshot click"));
            assert!(activity.contains(if collision { "Stage paired_ppm_written=0" } else { "Stage paired_ppm_written=1" }));
            let layout = std::fs::read_to_string(output.join("before.layout.txt")).unwrap();
            assert!(layout.contains("BEFORE_MARKER"));
            assert!(!layout.contains("AFTER_MARKER"));
            assert_eq!(std::fs::read_to_string(output.join("before.source.html")).unwrap(), original_source);
            if !collision {
                let mut expected = format!("P6\n{} {}\n255\n", before.width, before.height).into_bytes();
                for pixel in &before.pixels { expected.extend_from_slice(&[(pixel >> 16) as u8, (pixel >> 8) as u8, *pixel as u8]); }
                assert_eq!(std::fs::read(output.join("before.ppm")).unwrap(), expected, "Pixels must describe the same pre-click page as the text evidence");
                assert_ne!(app.draw().pixels, before.pixels);
            }
        }
    }

    #[test]
    fn input_snapshot_click_refuses_malformed_tokens_and_access_denial_without_input() {
        let files = SnapshotFiles::new();
        let invalid = [
            "snapshot-click\t../before\tchange\n".to_owned(),
            "snapshot-click\tbefore\n".to_owned(),
            "snapshot-click\tbefore\t\n".to_owned(),
            "snapshot-click\tbefore\tchange\textra\n".to_owned(),
            format!("snapshot-click\t{}\tchange\n", "x".repeat(49)),
            format!("snapshot-click\tbefore\t{}\n", "x".repeat(49)),
        ];
        for (index, source) in invalid.iter().enumerate() {
            let mut app = files.app();
            let script = files.0.join("input.tsv");
            std::fs::write(&script, source).unwrap();
            let output = files.0.join(format!("invalid-{index}"));
            assert!(app.run_input_script(&script, &output).is_err());
            assert_eq!(app.document.title, "Before snapshot click");
            assert!(!app.page.runs.iter().any(|run| run.text.contains("AFTER_MARKER")));
            assert!(!output.join("01-before.activity.txt").exists());
        }
        let mut app = files.app();
        app.network.begin("GET", "https://fixture.test/", "document").response(403);
        let script = files.0.join("input.tsv");
        std::fs::write(&script, "snapshot-click\tbefore\tchange\n").unwrap();
        let output = files.0.join("denied");
        let error = app.run_input_script(&script, &output).unwrap_err();
        assert!(error.contains("Visible access denial/human verification or document rate limit"));
        assert_eq!(app.document.title, "Before snapshot click");
        assert!(!app.page.runs.iter().any(|run| run.text.contains("AFTER_MARKER")));
        assert!(!output.join("01-before.activity.txt").exists());
    }
}
