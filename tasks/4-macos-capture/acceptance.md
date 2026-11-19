# Acceptance: macos-capture

Mac: Apple M5, one built-in display of 1512 by 982 points at scale 2, macOS 26.6.2, Xcode command line tools only, Rust 1.99.1. Terminal has the Screen Recording and Input Monitoring permissions.

## Task 4: try-out output

Program: `spike/sck` on branch `spike/4-sck` (never merged). Lines trimmed to the numbers; `…` marks a cut.

### Step 1, `list`
```
preflight: screen=true listen=true
display 0: id=13 frame=CGRect { origin: (0.0, 0.0), size: (1512.0, 982.0) } width=1512 height=982 filter.content_rect=(0, 0, 1512, 982) filter.point_pixel_scale=2
windows: 170 total
windows passing the filter (layer 0, title, application): 21
```
First `cargo build`: 99.46 s real (Swift packages included). Linking needed `-L native=/Library/Developer/CommandLineTools/usr/lib/swift/macosx` and running needed `-Wl,-rpath,/usr/lib/swift` (plan decision 15).

### Step 2, `frames display|region|window`
```
display stream 3024x1964
[display] callbacks=23 by status={"Some(Complete)": 10, "Some(Idle)": 13}
[display] last frame: size: (3024, 1964), bytes_per_row: 12160, content_rect (0,0,1512,982), scale_factor 2.0
region source_rect (100,100,200,150) points, stream 400x300
[region] callbacks=10 by status={"Some(Complete)": 10}
[region] last frame: size: (400, 300), bytes_per_row: 1664
window id=36030 title=Some("recdr-spike-title") app=Some("Google Chrome") layer=0 on_screen=true frame=(22, 52, 800, 600)
filter.content_rect=(22, 52, 800, 600) point_pixel_scale=2 -> stream 1600x1200; CDP bounds={"height":600,"left":22,"top":52,"width":800}; 800x600*scale=1600x1200
[window] callbacks=10 by status={"Some(Complete)": 10}
[window] last frame: size: (1600, 1200), bytes_per_row: 6400
```
Without `CGMainDisplayID()` first, the window filter aborted: `Assertion failed: (did_initialize), function CGS_REQUIRE_INIT, file CGInitialization.c, line 44.`

### Step 3, `resize` (made smaller through CDP `Browser.setWindowBounds`, not by hand)
```
[before] callbacks=31 by status={"Some(Complete)": 31}
[before] last frame: size: (1600, 1200), content_rect (0,0,800,600), content_scale 1.0
setWindowBounds {"height":400,"width":500} -> "ok"
[after smaller] last frame: size: (1600, 1200), content_rect (0,0,750,600), content_scale 1.5
after resize: new filter content_rect=(22, 52, 500, 400)
after: pixel (10,600)=(255,255,255) (1590,600)=(0,0,0)
```
Padded with black on the right, not stretched (frame saved outside the repo and looked at).

### Step 4, `away`
```
[visible] callbacks=31 by status={"Some(Complete)": 31}
setWindowBounds {"windowState":"minimized"} -> "ok"
[minimized] callbacks=1 by status={"Some(Suspended)": 1}
[minimized] stop notices so far: []
minimized: listed=true on_screen=Some(false)
[restored] callbacks=22 by status={"Some(Complete)": 22}
[closed] callbacks=1 by status={"Some(Stopped)": 1}
[closed] stop notices so far: ["SCStream error (No capture source provided): Failed to find any displays or windows to capture"]
```
Another Space: could not be tried without a person (`hand.sh space`, on the owner's list).

### Step 5, `tap` (permission on, no click made)
```
tap: listen access allowed=true
tap: mask=0x200000a
tap: created; click three times anywhere within 30 s
tap: 0 presses; stopping
tap: run loop returned
tap: drop returned in 528.459µs
```
`CFRetained<CFRunLoop>` is not `Send` (compile error E0277 on a `Send` bound). Presses and the permission-off run need a person.

### Step 6, `detached`
```
detached child pid 25745 logs to …/detached.log
(child) preflight: screen=true listen=true
(child) display stream 3024x1964
(child) [display] callbacks=150 by status={"Some(Complete)": 2, "Some(Idle)": 148}
(child) tap: created
  PID   SESS COMM
25745      0 …/sck-spike
```
The child runs in its own session and has both permissions: macOS counts Terminal. No dialog was shown, so which program a dialog names was not seen.

### Step 7, the alert of fact 6
Could not be tried without a person watching the screen; on the owner's list.

### Step 8, `system_profiler SPDisplaysDataType -json`
```
['_name', 'spdisplays_mtlgpufamilysupport', 'spdisplays_vendor', 'sppci_bus', 'sppci_cores', 'sppci_device_type', 'sppci_model']
```
No `spdisplays_ndrvs` and no display at all; the plain text output also lists only the GPU. `ioreg` has `DisplayWidth = 3024`, `DisplayHeight = 1964` under `IOMobileFramebuffer`. The criterion 1 test uses CoreGraphics display modes.

### Step 9, CI (draft pull request #12, closed unmerged)
```
dyld[13446]: Library not loaded: @rpath/libswift_Concurrency.dylib
##[error]Process completed with exit code 134.            (without the rpath, Xcode present)
preflight: screen=true listen=true                        (with -Wl,-rpath,/usr/lib/swift)
display stream 1024x768
[display] callbacks=14 by status={"Some(Complete)": 12, "Some(Idle)": 2}
```
The Windows and Linux jobs passed too.

### Step 10, macOS 13
Could not be tried: no such Mac.
