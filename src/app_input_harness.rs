//! Local development input driver, using the same App paths as the native shell.
//! No JavaScript evaluation, direct form mutation or direct link navigation.
use super::{App, Focus, PlatformAction};
use crate::scripting::KeyboardInput;
use std::io::Write;
use std::path::Path;
use std::time::Duration;

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
                if matches!(fields[0], "fill" | "click" | "link" | "link-target" | "key" | "back" | "address" | "media-surface" | "media-probe") && self.input_access_blocked() {
                    return Err("Visible access denial/human verification or document rate limit; stopping without retry.".into());
                }
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
                    ["snapshot", name] if !name.is_empty() && name.len() <= 48 && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') => {
                        self.input_snapshot(output, &format!("{:02}-{name}", index + 1))
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
        self.pointer_down(x, y, false);
        self.pointer_up(x, y);
        Ok(())
    }

    fn input_snapshot(&self, output: &Path, name: &str) -> Result<(), String> {
        // Draw before reporting so the numeric stamp describes this PPM. Keep
        // text evidence even if the surface allocation or file output fails.
        let rendered = (|| -> Result<(), String> {
            let canvas = self.draw();
            if canvas.pixels.is_empty() { return Err("Could not allocate harness render surface.".into()); }
            let mut file = std::io::BufWriter::new(std::fs::File::create(output.join(format!("{name}.ppm"))).map_err(|e| e.to_string())?);
            write!(file, "P6\n{} {}\n255\n", canvas.width, canvas.height).map_err(|e| e.to_string())?;
            for pixel in &canvas.pixels { file.write_all(&[(pixel >> 16) as u8, (pixel >> 8) as u8, *pixel as u8]).map_err(|e| e.to_string())?; }
            file.flush().map_err(|e| e.to_string())
        })();
        let mut activity = self.activity_report();
        activity.push_str(&format!("Stage paired_ppm_written={}\n", u8::from(rendered.is_ok())));
        std::fs::write(output.join(format!("{name}.activity.txt")), activity).map_err(|e| e.to_string())?;
        std::fs::write(output.join(format!("{name}.source.html")), self.document_source()).map_err(|e| e.to_string())?;
        let mut layout = String::new();
        for run in &self.page.runs {
            let target = run.link.and_then(|index| self.page.link_targets().nth(index));
            layout.push_str(&format!("x={} y={} w={} h={} text={:?} href={target:?}\n", run.x, run.y, run.width(), run.height(), run.text));
        }
        for rect in &self.page.controls {
            let value = self.forms.controls.get(&rect.node).map(|control| control.value.text());
            layout.push_str(&format!("control={} name={:?} value={value:?} x={} y={} w={} h={}\n", rect.node, self.document.attr(rect.node, "name"), rect.x, rect.y, rect.width, rect.height));
        }
        std::fs::write(output.join(format!("{name}.layout.txt")), layout).map_err(|e| e.to_string())?;
        if let Some(text)=self.about_displayed_text() {
            std::fs::write(output.join(format!("{name}.about.txt")),text).map_err(|e|e.to_string())?;
            std::fs::write(output.join(format!("{name}.about-state.txt")),self.about_state_report().ok_or("About state unavailable.")?).map_err(|e|e.to_string())?;
        }
        rendered
    }
}

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
}
