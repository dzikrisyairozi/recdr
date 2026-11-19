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

## Criteria

"Runner" is GitHub's hosted `macos-latest` (one 1024 by 768 display at scale 1), where CI runs `cargo test --test desktop -- --ignored --test-threads=1` since Task 11. "This Mac" is the machine above, where all ten macOS desktop tests passed on 2026-11-20 (72 s). A locked Mac lists every window as not on screen, so the window tests need an unlocked screen.

1. Pass. `monitors_have_one_primary_and_pixel_sizes`, on this Mac (3024 by 1964) and the runner. The size is checked against CoreGraphics' display mode in pixels, because `system_profiler` lists no display here (Task 4, step 8).
2. Pass on this Mac and the runner. `windows_lists_the_test_window`.
3. Pass on this Mac and the runner. `window_recording_has_the_content_size`.
4. Pass on this Mac and the runner. `window_and_process_find_it_and_two_are_ambiguous`.
5. Pass on this Mac (`region_inside_the_green_page_is_green`). Fails on the runner every time: the center reads (223, 250, 223) where the page is #00aa00, and a monitor recording reads the same value at the same screen point, so the region matches the screen and what the runner shows there is pale. Cause not known; the test is unchanged, for the owner to decide.
6. Pass on this Mac and the runner. `monitor_recording_has_the_monitor_size`.
7. Pass on this Mac and the runner. `still_page_stores_few_frames`.
8. Pass on this Mac and the runner. `ending_chrome_ends_the_recording`.
9. Pass on this Mac and the runner. `minimized_window_is_listed_and_refused`.
10. Pass on this Mac and the runner. `detached_recording_and_mcp_work_on_a_window`. On this Mac `start --monitor 0`, then `stop`, recorded 4 frames at 3024 by 1964 once the lists came from CoreGraphics (plan decision 16).
11. Owner. Needs the Screen Recording permission switched off for Terminal: `recdr windows` and `recdr record --monitor 0 --duration 1s --out /tmp/r11` should exit 1 with `permission_denied`, and `/tmp/r11` should have no recording directory.
12. Owner. Needs a real click: `recdr record --window <a window title> --duration 5s`, click inside it, and check `clicks` in `manifest.json` and the ring in the GIF. The click filter is unit-tested (16); the tap starts and stops (`start_then_drop_returns`).
13. Owner. Needs Input Monitoring switched off for Terminal: the same recording exits 0, prints `clicks are not recorded: ...` on stderr, and has `"clicks": []`.
14. Owner. Needs the pointer over the window: record with and without `--no-cursor` and look at the frames. `region_inside_the_green_page_is_green` records with `--no-cursor`.
15. Owner. Needs a window resized during a recording. Task 4, step 3 showed macOS pads a smaller window with black on the right at the starting size.
16. Pass. The `capture::tests` unit tests on the three CI jobs, among them `display_bounds_do_not_overlap_either_way_round` and `a_rectangle_inside_each_display_goes_to_pixels_and_back` (2x next to 1x), and `to_event_scales_points_to_pixels`.
17. Pass on this Mac and the runner. `mask_is_the_three_presses` (`0x0200000A`).
18. Pass on the Linux job. `desktop_commands_are_unsupported_off_windows` (tests/tabs.rs).
19. Partly. The Windows job runs the browser and unit tests; the Windows desktop tests passed at Checkpoint A. A click by hand on Windows: owner (Checkpoint A).
20. Partly. `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` and the layer tests pass on the three jobs. The macOS job is red from 5 on the runner, and at times from `background_tab_is_recorded` and `minimized_window_is_recorded` in tests/browser.rs (cdp-capture), which pass here 3 runs out of 3 and failed on the runner before this module too.
