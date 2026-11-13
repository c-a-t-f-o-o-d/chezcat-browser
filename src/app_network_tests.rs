//! A real loopback page exercises the loader, concurrent resource requests,
//! shared trace, dock/window geometry and input routes together.
use super::*;
use crate::network_trace::Outcome;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

struct Site {
    url: String,
    stop: Arc<AtomicBool>,
    count: Arc<AtomicUsize>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Site {
    fn new() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!(
            "http://127.0.0.1:{}/",
            listener.local_addr().unwrap().port()
        );
        let stop = Arc::new(AtomicBool::new(false));
        let count = Arc::new(AtomicUsize::new(0));
        let stopping = stop.clone();
        let requests = count.clone();
        let thread = std::thread::spawn(move || {
            let mut workers = Vec::new();
            while !stopping.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        // Accepted Winsock sockets inherit the polling listener mode.
                        stream.set_nonblocking(false).unwrap();
                        let requests = requests.clone();
                        workers.push(std::thread::spawn(move || serve(stream, &requests)));
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(2))
                    }
                    Err(error) => panic!("loopback accept: {error}"),
                }
            }
            for worker in workers {
                worker.join().unwrap();
            }
        });
        Self {
            url,
            stop,
            count,
            thread: Some(thread),
        }
    }
    fn load(&self) -> App {
        let mut app = App::new();
        app.open_address(&self.url);
        let deadline = Instant::now() + Duration::from_secs(6);
        while app.is_loading() && Instant::now() < deadline {
            app.poll_load();
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(!app.is_loading(), "{}", app.status);
        assert!(!app.failed, "{}", app.status);
        app
    }
}
impl Drop for Site {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            if let Err(failure) = thread.join() {
                if !std::thread::panicking() {
                    std::panic::resume_unwind(failure);
                }
            }
        }
    }
}
fn serve(mut stream: TcpStream, count: &AtomicUsize) {
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let mut head = Vec::new();
    let mut byte = [0];
    while !head.ends_with(b"\r\n\r\n") {
        if stream.read(&mut byte).unwrap_or(0) == 0 {
            return;
        }
        head.push(byte[0]);
        assert!(head.len() < 32768);
    }
    count.fetch_add(1, Ordering::AcqRel);
    let request = String::from_utf8(head).unwrap();
    let path = request.split_whitespace().nth(1).unwrap();
    let (mime, body) = if path == "/" {
        ("text/html", b"<!doctype html><title>Measured network</title><link rel=stylesheet href=/site.css><h1>Actual loopback requests</h1><img src=/image-1.ppm><img src=/image-2.ppm><img src=/image-3.ppm><img src=/image-4.ppm><img src=/image-5.ppm><img src=/image-6.ppm><img src=/image-1.ppm><script src=http://blocked.invalid/blocked.js></script>".to_vec())
    } else if path == "/site.css" {
        ("text/css", b"body { background:#eef6f4; color:#173342; margin:24px } img { width:48px; height:48px }".to_vec())
    } else {
        (
            "image/x-portable-pixmap",
            b"P6\n2 1\n255\n\x20\xc0\xa0\x50\x80\xd0".to_vec(),
        )
    };
    std::thread::sleep(Duration::from_millis(if path.starts_with("/image-") {
        30
    } else {
        5
    }));
    let headers = format!("HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
    if stream.write_all(headers.as_bytes()).is_err() {
        return;
    }
    let split = body.len() / 2;
    if stream.write_all(&body[..split]).is_err() {
        return;
    }
    std::thread::sleep(Duration::from_millis(20));
    let _ = stream.write_all(&body[split..]);
}
fn save_shot(app: &App, name: &str, detached: bool) {
    let Some(folder) = std::env::var_os("CHEZCAT_NETWORK_SHOTS") else {
        return;
    };
    let folder = std::path::PathBuf::from(folder);
    std::fs::create_dir_all(&folder).unwrap();
    let canvas = if detached {
        app.draw_inspector_window()
    } else {
        app.draw()
    };
    let mut out = std::io::BufWriter::new(std::fs::File::create(folder.join(name)).unwrap());
    writeln!(out, "P6\n{} {}\n255", canvas.width, canvas.height).unwrap();
    for pixel in canvas.pixels {
        out.write_all(&[(pixel >> 16) as u8, (pixel >> 8) as u8, pixel as u8])
            .unwrap();
    }
}

#[test]
fn network_table_click_highlights_real_timeline_docked_and_detached() {
    let site = Site::new();
    let mut app = site.load();
    assert_eq!(site.count.load(Ordering::Acquire), 8);
    let trace = app.network.snapshot();
    assert_eq!(
        trace
            .rows
            .iter()
            .filter(|r| r.outcome == Outcome::Complete)
            .count(),
        8
    );
    assert_eq!(
        trace
            .rows
            .iter()
            .filter(|r| r.outcome == Outcome::Blocked)
            .count(),
        1
    );
    assert_eq!(
        trace
            .rows
            .iter()
            .filter(|r| r.outcome == Outcome::Reused)
            .count(),
        1
    );
    let images = trace
        .rows
        .iter()
        .filter(|r| r.kind == "image" && r.outcome == Outcome::Complete)
        .take(3)
        .collect::<Vec<_>>();
    assert_eq!(images.len(), 3);
    assert!(
        images.iter().map(|r| r.start).max().unwrap()
            < images.iter().map(|r| r.end.unwrap()).min().unwrap()
    );
    app.resize(1180, 840);
    app.open_inspector();
    app.inspector.height = 500;
    app.inspector.font_scale = 1;
    app.inspector_tab(Tab::Network);
    app.reflow();
    let area = app.network_geometry();
    assert!(area.table_rows.y < area.lanes.y);
    let id = (app.inspector_snapshot().rows[1].number - 1) as u64;
    let x = area.table_rows.x + 70;
    let y = area.table_rows.y + app.network_row_height() + 5;
    assert_eq!(app.inspector_hit(x, y), Some(Action::Request(id)));
    app.inspector_pointer_down(x, y);
    app.inspector_pointer_up(x, y);
    assert_eq!(app.inspector.network_selected, Some(id));
    let canvas = app.draw();
    assert_eq!(
        canvas.pixels[y as usize * canvas.width + 1],
        app.browser_palette().selected
    );
    let graph_y = area.lanes.y + app.network_lane_height() + 2;
    assert_eq!(
        canvas.pixels[graph_y as usize * canvas.width + 1],
        app.browser_palette().selected
    );
    save_shot(&app, "network-table-docked.ppm", false);
    app.detach_inspector();
    app.inspector_window_size(1180, 700);
    let area = app.network_geometry();
    let id = (app.inspector_snapshot().rows[3].number - 1) as u64;
    let (x, y) = (
        area.lanes.x + 35,
        area.lanes.y + app.network_lane_height() * 3 + 3,
    );
    app.inspector_pointer_down(x, y);
    app.inspector_pointer_up(x, y);
    assert_eq!(app.inspector.network_selected, Some(id));
    save_shot(&app, "network-table-detached.ppm", true);
    let mut preferences = app.profile.data().clone();
    preferences.settings.dark_mode = true;
    app.profile.save(preferences).unwrap();
    let canvas = app.draw_inspector_window();
    assert_eq!(
        canvas.pixels[(area.table_rows.y + 2) as usize * canvas.width + 1],
        app.browser_palette().panel
    );
    save_shot(&app, "network-table-dark.ppm", true);
    assert!(app
        .inspector_copy()
        .contains("Wait for first response byte"));
    assert!(app.inspector_copy().contains("no request sent"));
    assert_eq!(site.count.load(Ordering::Acquire), 8);
}

#[test]
fn network_filter_scroll_and_split_resize_never_issue_requests() {
    let site = Site::new();
    let mut app = site.load();
    app.open_inspector();
    app.inspector_tab(Tab::Network);
    app.inspector.font_scale = 1;
    app.detach_inspector();
    app.inspector_window_size(820, 430);
    app.inspector.pane = Pane::Filter;
    app.inspector_paste("/image-");
    assert_eq!(app.inspector_snapshot().rows.len(), 7);
    let area = app.network_geometry();
    app.inspector_wheel(
        area.table_rows.x + 20,
        area.table_rows.y + 5,
        -120,
        false,
        false,
    );
    assert!(app.inspector.scroll > 0);
    let old_split = app.inspector.network_split;
    app.inspector_pointer_down(area.divider.x + 100, area.divider.y + 2);
    assert!(app.inspector.network_resize);
    app.inspector_pointer_move(area.divider.x + 100, area.divider.y - 25, true);
    app.inspector_pointer_up(area.divider.x + 100, area.divider.y - 25);
    assert_ne!(app.inspector.network_split, old_split);
    save_shot(&app, "network-filter-scroll.ppm", true);
    app.inspector_window_size(380, 260);
    let canvas = app.draw_inspector_window();
    assert_eq!((canvas.width, canvas.height), (380, 260));
    assert_eq!(site.count.load(Ordering::Acquire), 8);
}
