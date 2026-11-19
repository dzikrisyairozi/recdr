//! Task 4 try-out program (branch spike/4-sck, never merged). One subcommand per step
//! of tasks/4-macos-capture/todo.md Task 4.

use std::ffi::c_void;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::ptr::NonNull;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use objc2_core_foundation::{CFMachPort, CFRetained, CFRunLoop, kCFRunLoopCommonModes};
use objc2_core_graphics::{
    CGEvent, CGEventMask, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement,
    CGEventTapProxy, CGEventType, CGPreflightListenEventAccess, CGPreflightScreenCaptureAccess,
    CGRequestListenEventAccess, CGRequestScreenCaptureAccess,
};
use screencapturekit::cv::CVPixelBufferLockFlags;
use screencapturekit::prelude::*;
use screencapturekit::stream::delegate_trait::StreamCallbacks;
use serde_json::{Value, json};

const PAGE: &str = "<!doctype html><title>spike</title>\
<style>body{margin:0;background:#fff}#b{position:absolute;top:100px;left:0;\
width:100px;height:100px;background:#f00}</style><div id=b></div>\
<script>if (location.hash) document.title = decodeURIComponent(location.hash.slice(1));\
let x=0;const b=document.getElementById('b');\
function f(){x=(x+4)%600;b.style.left=x+'px';requestAnimationFrame(f)}\
requestAnimationFrame(f)</script>";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let arg = |i: usize| args.get(i).map(String::as_str).unwrap_or("");
    println!(
        "preflight: screen={} listen={}",
        CGPreflightScreenCaptureAccess(),
        CGPreflightListenEventAccess()
    );
    match arg(0) {
        "list" => list(),
        "frames" => frames(arg(1)),
        "resize" => resize(),
        "away" => away(arg(1) == "space"),
        "tap" => tap(),
        "detached" => detached(),
        "child" => {
            frames("display");
            tap();
        }
        other => eprintln!("unknown subcommand {other:?}"),
    }
}

// ---- step 1 ----

fn list() {
    let content = SCShareableContent::get().expect("shareable content");
    for (i, d) in content.displays().iter().enumerate() {
        let f = SCContentFilter::create()
            .with_display(d)
            .with_excluding_windows(&[])
            .build()
            .unwrap();
        println!(
            "display {i}: id={} frame={:?} width={} height={} filter.content_rect={:?} filter.point_pixel_scale={}",
            d.display_id(),
            d.frame(),
            d.width(),
            d.height(),
            f.content_rect(),
            f.point_pixel_scale()
        );
    }
    let windows = content.windows();
    println!("windows: {} total", windows.len());
    let mut shown = 0;
    for w in &windows {
        let app = w.owning_application();
        let title = w.title().unwrap_or_default();
        if w.window_layer() == 0 && !title.is_empty() && app.is_some() {
            shown += 1;
        }
    }
    println!("windows passing the filter (layer 0, title, application): {shown}");
    // Only print windows owned by this program's own Chrome later; here print one
    // layer-0 window's numbers without its title, so no personal name is printed.
    if let Some(w) = windows
        .iter()
        .find(|w| w.window_layer() == 0 && w.owning_application().is_some())
    {
        let f = SCContentFilter::create().with_window(w).build().unwrap();
        println!(
            "one window: id={} layer={} on_screen={} frame={:?} filter.content_rect={:?} filter.point_pixel_scale={}",
            w.window_id(),
            w.window_layer(),
            w.is_on_screen(),
            w.frame(),
            f.content_rect(),
            f.point_pixel_scale()
        );
    }
}

// ---- capture helpers ----

#[derive(Debug, Clone)]
struct Info {
    ms: u128,
    status: String,
    size: (usize, usize),
    bytes_per_row: usize,
    content_rect: Option<CGRect>,
    scale_factor: Option<f64>,
    content_scale: Option<f64>,
}

struct Capture {
    stream: SCStream,
    infos: Arc<Mutex<Vec<Info>>>,
    stopped: Arc<Mutex<Vec<String>>>,
    save: Arc<Mutex<Option<PathBuf>>>,
}

fn capture(filter: &SCContentFilter, width: u32, height: u32, source: Option<CGRect>) -> Capture {
    let mut config = SCStreamConfiguration::new()
        .with_width(width)
        .with_height(height)
        .with_minimum_frame_interval(&CMTime::new(1, 15))
        .with_shows_cursor(true)
        .with_pixel_format(PixelFormat::BGRA)
        .with_scales_to_fit(true);
    if let Some(rect) = source {
        config = config.with_source_rect(rect);
    }
    let infos = Arc::new(Mutex::new(Vec::new()));
    let stopped = Arc::new(Mutex::new(Vec::new()));
    let save: Arc<Mutex<Option<PathBuf>>> = Arc::new(Mutex::new(None));
    let start = Instant::now();
    let s2 = stopped.clone();
    let delegate =
        StreamCallbacks::new().on_error(move |e| s2.lock().unwrap().push(format!("{e}")));
    let mut stream = SCStream::new_with_delegate(filter, &config, delegate).expect("stream");
    let (i2, sv) = (infos.clone(), save.clone());
    stream
        .add_output_handler(
            move |sample: CMSampleBuffer, kind: SCStreamOutputType| {
                if !matches!(kind, SCStreamOutputType::Screen) {
                    return;
                }
                let status = sample.frame_status();
                let mut info = Info {
                    ms: start.elapsed().as_millis(),
                    status: format!("{status:?}"),
                    size: (0, 0),
                    bytes_per_row: 0,
                    content_rect: sample.content_rect(),
                    scale_factor: sample.scale_factor(),
                    content_scale: sample.content_scale(),
                };
                if let Some(pb) = sample.pixel_buffer()
                    && let Ok(g) = pb.lock(CVPixelBufferLockFlags::READ_ONLY)
                {
                    info.size = (g.width(), g.height());
                    info.bytes_per_row = g.bytes_per_row();
                    if let Some(path) = sv.lock().unwrap().take()
                        && let Some(bytes) = unsafe { g.as_slice() }
                    {
                        write_ppm(&path, bytes, g.width(), g.height(), g.bytes_per_row());
                    }
                }
                i2.lock().unwrap().push(info);
            },
            SCStreamOutputType::Screen,
        )
        .expect("handler");
    stream.start_capture().expect("start");
    Capture {
        stream,
        infos,
        stopped,
        save,
    }
}

fn write_ppm(path: &PathBuf, bgra: &[u8], w: usize, h: usize, stride: usize) {
    let mut out = format!("P6 {w} {h} 255\n").into_bytes();
    for y in 0..h {
        for x in 0..w {
            let p = &bgra[y * stride + x * 4..];
            out.extend_from_slice(&[p[2], p[1], p[0]]);
        }
    }
    std::fs::write(path, out).unwrap();
    let png = path.with_extension("png");
    let _ = Command::new("sips")
        .args(["-s", "format", "png"])
        .arg(path)
        .arg("--out")
        .arg(&png)
        .output();
    println!("saved {}", png.display());
}

fn report(c: &Capture, label: &str, from: usize) -> usize {
    let infos = c.infos.lock().unwrap();
    let mut counts: std::collections::BTreeMap<String, usize> = Default::default();
    for i in &infos[from..] {
        *counts.entry(i.status.clone()).or_default() += 1;
    }
    println!(
        "[{label}] callbacks={} by status={counts:?}",
        infos.len() - from
    );
    if let Some(last) = infos[from..].iter().rev().find(|i| i.size.0 > 0) {
        println!("[{label}] last frame: {last:?}");
    }
    println!(
        "[{label}] stop notices so far: {:?}",
        c.stopped.lock().unwrap()
    );
    infos.len()
}

// ---- Chrome test window and CDP ----

fn serve() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for mut s in listener.incoming().flatten() {
            let mut r = BufReader::new(s.try_clone().unwrap());
            let mut line = String::new();
            while r.read_line(&mut line).unwrap_or(0) > 0 && !line.ends_with("\r\n\r\n") {}
            let _ = write!(
                s,
                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{PAGE}",
                PAGE.len()
            );
        }
    });
    port
}

struct Chrome {
    child: Child,
    dir: PathBuf,
    ws: tungstenite::WebSocket<TcpStream>,
    next: u64,
}

impl Chrome {
    fn app(title: &str) -> Chrome {
        let port = serve();
        let dir = std::env::temp_dir().join(format!("sck-spike-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let child = Command::new("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome")
            .arg(format!("--app=http://127.0.0.1:{port}/#{title}"))
            .arg(format!("--user-data-dir={}", dir.display()))
            .args([
                "--window-size=800,600",
                "--remote-debugging-port=0",
                "--no-first-run",
                "--no-default-browser-check",
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let file = dir.join("DevToolsActivePort");
        let deadline = Instant::now() + Duration::from_secs(20);
        let (dport, path) = loop {
            let text = std::fs::read_to_string(&file).unwrap_or_default();
            if let [p, path] = text.lines().collect::<Vec<_>>()[..] {
                break (p.parse::<u16>().unwrap(), path.to_owned());
            }
            assert!(Instant::now() < deadline, "no DevToolsActivePort");
            std::thread::sleep(Duration::from_millis(100));
        };
        let stream = TcpStream::connect(("127.0.0.1", dport)).unwrap();
        let (ws, _) =
            tungstenite::client(format!("ws://127.0.0.1:{dport}{path}").as_str(), stream).unwrap();
        Chrome {
            child,
            dir,
            ws,
            next: 1,
        }
    }

    fn call(&mut self, method: &str, params: Value) -> Value {
        let id = self.next;
        self.next += 1;
        self.ws
            .send(tungstenite::Message::text(
                json!({"id": id, "method": method, "params": params}).to_string(),
            ))
            .unwrap();
        loop {
            let tungstenite::Message::Text(t) = self.ws.read().unwrap() else {
                continue;
            };
            let v: Value = serde_json::from_str(t.as_str()).unwrap();
            if v["id"] == id {
                return v;
            }
        }
    }

    fn window(&mut self) -> (i64, Value) {
        let targets = self.call("Target.getTargets", json!({}));
        let tab = targets["result"]["targetInfos"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["type"] == "page")
            .unwrap()["targetId"]
            .clone();
        let w = self.call("Browser.getWindowForTarget", json!({"targetId": tab}));
        (
            w["result"]["windowId"].as_i64().unwrap(),
            w["result"]["bounds"].clone(),
        )
    }

    fn bounds(&mut self, bounds: Value) {
        let (id, _) = self.window();
        let r = self.call(
            "Browser.setWindowBounds",
            json!({"windowId": id, "bounds": bounds}),
        );
        println!(
            "setWindowBounds {bounds} -> {}",
            r.get("error").unwrap_or(&json!("ok"))
        );
    }
}

impl Drop for Chrome {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn find_window(title: &str) -> SCWindow {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let content = SCShareableContent::get().unwrap();
        eprintln!("mark: content got");
        if let Some(w) = content
            .windows()
            .into_iter()
            .find(|w| w.title().as_deref() == Some(title))
        {
            return w;
        }
        assert!(Instant::now() < deadline, "no window titled {title}");
        std::thread::sleep(Duration::from_millis(200));
    }
}

fn window_capture(chrome: &mut Chrome, title: &str) -> (Capture, SCWindow, f32) {
    let w = find_window(title);
    eprintln!("mark: found window");
    // A window filter asserts in CGS_REQUIRE_INIT unless CoreGraphics has a
    // connection to the window server; CGMainDisplayID makes one.
    eprintln!(
        "mark: CGMainDisplayID={}",
        objc2_core_graphics::CGMainDisplayID()
    );
    let f = SCContentFilter::create().with_window(&w).build().unwrap();
    eprintln!("mark: filter built");
    let (rect, scale) = (f.content_rect(), f.point_pixel_scale());
    eprintln!("mark: rect and scale read");
    let (_, cdp) = chrome.window();
    let width = (rect.size.width * f64::from(scale)).round() as u32;
    let height = (rect.size.height * f64::from(scale)).round() as u32;
    println!(
        "window id={} title={:?} app={:?} layer={} on_screen={} frame={:?}",
        w.window_id(),
        w.title(),
        w.owning_application().map(|a| a.application_name()),
        w.window_layer(),
        w.is_on_screen(),
        w.frame()
    );
    println!(
        "filter.content_rect={rect:?} point_pixel_scale={scale} -> stream {width}x{height}; CDP bounds={cdp}; 800x600*scale={}x{}",
        800.0 * scale,
        600.0 * scale
    );
    (capture(&f, width, height, None), w, scale)
}

fn shots() -> PathBuf {
    let dir = PathBuf::from(std::env::var("SPIKE_OUT").unwrap_or_else(|_| "/tmp".into()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

// ---- step 2 ----

fn frames(kind: &str) {
    let content = SCShareableContent::get().unwrap();
    let d = &content.displays()[0];
    let (c, chrome) = match kind {
        "display" | "region" => {
            let f = SCContentFilter::create()
                .with_display(d)
                .with_excluding_windows(&[])
                .build()
                .unwrap();
            let s = f.point_pixel_scale();
            let r = f.content_rect();
            if kind == "display" {
                let (w, h) = (
                    (r.size.width * f64::from(s)) as u32,
                    (r.size.height * f64::from(s)) as u32,
                );
                println!("display stream {w}x{h}");
                (capture(&f, w, h, None), None)
            } else {
                // 200 by 150 points at (100, 100) of display 0: 400x300 pixels at scale 2.
                let (w, h) = ((200.0 * s) as u32, (150.0 * s) as u32);
                println!("region source_rect (100,100,200,150) points, stream {w}x{h}");
                (
                    capture(&f, w, h, Some(CGRect::new(100.0, 100.0, 200.0, 150.0))),
                    None,
                )
            }
        }
        "window" => {
            let mut chrome = Chrome::app("recdr-spike-title");
            std::thread::sleep(Duration::from_secs(2));
            let (c, _, _) = window_capture(&mut chrome, "recdr-spike-title");
            (c, Some(chrome))
        }
        other => panic!("frames {other:?}"),
    };
    let deadline = Instant::now() + Duration::from_secs(10);
    while c
        .infos
        .lock()
        .unwrap()
        .iter()
        .filter(|i| i.size.0 > 0)
        .count()
        < 10
        && Instant::now() < deadline
    {
        std::thread::sleep(Duration::from_millis(50));
    }
    c.stream.stop_capture().unwrap();
    for i in c.infos.lock().unwrap().iter().take(12) {
        println!("frame {i:?}");
    }
    report(&c, kind, 0);
    drop(chrome);
}

// ---- step 3 ----

fn resize() {
    let mut chrome = Chrome::app("recdr-spike-resize");
    std::thread::sleep(Duration::from_secs(2));
    let (c, w, _) = window_capture(&mut chrome, "recdr-spike-resize");
    let out = shots();
    std::thread::sleep(Duration::from_secs(1));
    *c.save.lock().unwrap() = Some(out.join("resize-before.ppm"));
    std::thread::sleep(Duration::from_secs(1));
    let n = report(&c, "before", 0);
    chrome.bounds(json!({"width": 500, "height": 400}));
    std::thread::sleep(Duration::from_secs(2));
    *c.save.lock().unwrap() = Some(out.join("resize-after.ppm"));
    std::thread::sleep(Duration::from_secs(1));
    report(&c, "after smaller", n);
    let f = SCContentFilter::create()
        .with_window(&find_window("recdr-spike-resize"))
        .build()
        .unwrap();
    println!(
        "after resize: SCWindow frame={:?} new filter content_rect={:?}",
        w.frame(),
        f.content_rect()
    );
    c.stream.stop_capture().unwrap();
}

// ---- step 4 ----

fn away(space: bool) {
    let mut chrome = Chrome::app("recdr-spike-away");
    std::thread::sleep(Duration::from_secs(2));
    let (c, w, _) = window_capture(&mut chrome, "recdr-spike-away");
    std::thread::sleep(Duration::from_secs(2));
    let mut n = report(&c, "visible", 0);
    chrome.bounds(json!({"windowState": "minimized"}));
    std::thread::sleep(Duration::from_secs(3));
    n = report(&c, "minimized", n);
    let listed = SCShareableContent::get()
        .unwrap()
        .windows()
        .into_iter()
        .find(|x| x.window_id() == w.window_id());
    println!(
        "minimized: listed={} on_screen={:?}",
        listed.is_some(),
        listed.map(|x| x.is_on_screen())
    );
    chrome.bounds(json!({"windowState": "normal"}));
    std::thread::sleep(Duration::from_secs(2));
    n = report(&c, "restored", n);
    if space {
        println!(">>> switch to another Space now (Ctrl+Right); waiting 10 s, then switch back");
        std::thread::sleep(Duration::from_secs(10));
        n = report(&c, "other space (10 s window)", n);
        let listed = SCShareableContent::get()
            .unwrap()
            .windows()
            .into_iter()
            .find(|x| x.window_id() == w.window_id());
        println!(
            "after space: on_screen={:?}",
            listed.map(|x| x.is_on_screen())
        );
    }
    drop(chrome);
    std::thread::sleep(Duration::from_secs(3));
    report(&c, "closed", n);
    let _ = c.stream.stop_capture();
}

// ---- step 5 ----

/// The tap's run loop, sent to the thread that drops the tap.
struct SendRunLoop(CFRetained<CFRunLoop>);
// SAFETY: CFRunLoopStop may be called from any thread (CFRunLoop reference).
unsafe impl Send for SendRunLoop {}

struct State {
    tx: mpsc::Sender<(u128, f64, f64, &'static str)>,
    port: Option<CFRetained<CFMachPort>>,
}

unsafe extern "C-unwind" fn on_event(
    _proxy: CGEventTapProxy,
    kind: CGEventType,
    event: NonNull<CGEvent>,
    user: *mut c_void,
) -> *mut CGEvent {
    // SAFETY: user is the State box this program leaked for the tap's lifetime.
    let state = unsafe { &*(user as *const State) };
    let button = match kind {
        CGEventType::LeftMouseDown => Some("left"),
        CGEventType::RightMouseDown => Some("right"),
        CGEventType::OtherMouseDown => Some("middle"),
        _ => None,
    };
    if let Some(button) = button {
        // SAFETY: the event pointer is valid for the duration of the callback.
        let p = CGEvent::location(Some(unsafe { event.as_ref() }));
        let ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let _ = state.tx.send((ms, p.x, p.y, button));
    } else if kind == CGEventType::TapDisabledByTimeout {
        if let Some(port) = &state.port {
            CGEvent::tap_enable(port, true);
        }
    } else {
        println!("tap: other event type {kind:?}");
    }
    event.as_ptr()
}

fn tap() {
    let allowed = CGPreflightListenEventAccess() || CGRequestListenEventAccess();
    println!("tap: listen access allowed={allowed} (continuing anyway to see what macOS does)");
    let mask: CGEventMask = (1 << CGEventType::LeftMouseDown.0)
        | (1 << CGEventType::RightMouseDown.0)
        | (1 << CGEventType::OtherMouseDown.0);
    println!("tap: mask={mask:#x}");
    let (tx, rx) = mpsc::channel();
    let (rtx, rrx) = mpsc::sync_channel::<Option<SendRunLoop>>(1);
    let thread = std::thread::spawn(move || {
        let state = Box::into_raw(Box::new(State { tx, port: None }));
        // SAFETY: on_event matches CGEventTapCallBack; state lives until after run returns.
        let port = unsafe {
            CGEvent::tap_create(
                CGEventTapLocation::SessionEventTap,
                CGEventTapPlacement::HeadInsertEventTap,
                CGEventTapOptions::ListenOnly,
                mask,
                Some(on_event),
                state.cast(),
            )
        };
        let Some(port) = port else {
            rtx.send(None).unwrap();
            // SAFETY: state came from Box::into_raw above and no tap uses it.
            drop(unsafe { Box::from_raw(state) });
            return;
        };
        let source = CFMachPort::new_run_loop_source(None, Some(&port), 0).unwrap();
        // SAFETY: only this thread touches state's port field before the loop runs.
        unsafe { (*state).port = Some(port) };
        let rl = CFRunLoop::current().unwrap();
        // SAFETY: kCFRunLoopCommonModes is a valid static.
        rl.add_source(Some(&source), unsafe { kCFRunLoopCommonModes });
        rtx.send(Some(SendRunLoop(rl))).unwrap();
        CFRunLoop::run();
        // SAFETY: the run loop has returned, the callback no longer runs.
        drop(unsafe { Box::from_raw(state) });
        println!("tap: run loop returned");
    });
    let Some(rl) = rrx.recv().unwrap() else {
        thread.join().unwrap();
        println!("tap: CGEventTapCreate returned null");
        return;
    };
    println!("tap: created; click three times anywhere within 30 s");
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut got = 0;
    while got < 3 && Instant::now() < deadline {
        if let Ok(p) = rx.recv_timeout(Duration::from_millis(200)) {
            got += 1;
            println!("tap: press {got}: {p:?}");
        }
    }
    println!("tap: {got} presses; stopping");
    let t = Instant::now();
    rl.0.stop();
    thread.join().unwrap();
    println!("tap: drop returned in {:?}", t.elapsed());
}

// ---- step 6 ----

fn detached() {
    use std::os::unix::process::CommandExt;
    let log = shots().join("detached.log");
    let file = std::fs::File::create(&log).unwrap();
    let mut cmd = Command::new(std::env::current_exe().unwrap());
    cmd.arg("child")
        .stdin(Stdio::null())
        .stdout(file.try_clone().unwrap())
        .stderr(file);
    // SAFETY: setsid is async-signal-safe and touches no Rust state.
    unsafe {
        cmd.pre_exec(|| {
            if libc::setsid() == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let child = cmd.spawn().unwrap();
    println!(
        "detached child pid {} logs to {}",
        child.id(),
        log.display()
    );
    let _ = CGRequestScreenCaptureAccess;
}
