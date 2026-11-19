# Acceptance: 6-gui-overlay

One line per success criterion in `spec/6-gui-overlay/spec.md`. Criteria 1 to 10 are tests and run in CI on three systems. Criteria 11 to 23 were run on Windows 11 Home 10.0.26200 (three 1920 by 1080 monitors at scale 1) on 2026-11-19 by the agent, with the release build. The agent may not move the pointer or press keys on the owner's desktop: it pressed the app's buttons through UI Automation (egui's AccessKit tree, no input sent), filled the save dialog with window messages, read window rectangles and the window under a point with Win32 queries, and compared screenshots. What needs a real drag or click is marked "owner". Recordings of the desktop stayed in the scratchpad. macOS: not run for any of 11 to 23 (Task 14, needs a Mac).

1. Pass. `export_whole` (tests/export.rs): size of the recording, length checked against the exported `duration_ms` within 5 ms per frame (the spec's 2.9 to 3.1 s band failed on GIF centisecond rounding; see todo, Task 3).
2. Pass. `export_trimmed`, and the unit test `a_trim_starts_at_the_frame_on_screen`.
3. Pass. `export_cropped`.
4. Pass. `a_crop_keeps_and_moves_the_clicks_inside_it` and `a_click_at_from_is_kept_and_one_at_until_is_not` (src/export.rs).
5. Pass where an MP4 can be written. `export_mp4`; it skips on `ffmpeg_not_found`, and on `encode_failed` on the Windows runner.
6. Pass. `export_errors` and `export_of_a_running_recording_is_not_found`.
7. Pass. `export_leaves_the_recording_alone`; `export_replaces_only_on_success` for the target file.
8. Pass. `lists_the_tools` and `export_returns_the_result` (tests/mcp.rs).
9. Pass. `format_none_writes_no_gif` and `format_none_with_gif_is_usage_and_records_nothing` (tests/record.rs).
10. Pass. The `geometry`, `state` and `worker` unit tests and `tests/layers.rs`; `gui_is_refused_where_there_is_no_app` runs on Linux in CI.
11. Windows, partly. The four border windows and the toolbar are `WS_EX_TOPMOST`. The inside between the border windows is x 640 to 1279, y 300 to 779 by `GetClientRect`, which is the 640 x 480 the toolbar shows (`evidence/T10-toolbar-idle.png`). Each border window shows a thin gray outline drawn by the system (plan, "What was measured"). Owner: look at it.
12. Owner. Needs drags. The arithmetic is unit-tested in `geometry` (`resized`, `keep_inside`).
13. Windows, before recording: `WindowFromPoint` at the frame's center returns the window under the frame. During a recording the border windows also let presses through (`with_mouse_passthrough`). Owner: click a real button through the frame.
14. Owner. Needs the target menu opened by a click; UI Automation refuses the combo box's expand action. The toolbar window grows by 240 points while the list is open, because egui keeps a popup inside its window.
15. Windows, pass. Record, 5 s, Stop: the editor's "New recording" button appeared 221 ms after Stop, and `recdr status` listed the recording `finished` at 5378 ms (82 frames, synthetic source). With the real screen: 257 ms, 5241 ms. The debug build took 6.8 s, all of it in recdr's analysis of the frames, which is unoptimized there.
16. Windows, pass. A region recording stored 640 x 480 frames, the size the toolbar showed, and the request was `region 640,300,640,480`. No pixel on any edge of six stored frames from three recordings is the blue or red of the border.
17. Windows, pass. While recording the line is red (`(220, 0, 0)`, sampled from a screenshot) and the toolbar shows `0:02` and Stop (`evidence/T12-toolbar-recording.png`). Moving and resizing are switched off while recording; that the frame then ignores a drag is for the owner.
18. Windows, pass. `recdr stop <id>` from a terminal during a recording: the editor appeared 931 ms after the command returned.
19. Windows, partly. Play shows motion: the position line advanced and the picture changed between screenshots (`evidence/T13-editor-playing.png`); Play and Pause swap. The timeline drag, the two handles and the crop rectangle need the pointer. Owner.
20. Windows, partly. "Save GIF" with no trim and no crop wrote a file byte-for-byte equal to `recdr export` of the same recording (320 x 240, 51 frames, 3337 ms), and the editor showed its path. With a trim and a crop: owner, as 19.
21. Windows, partly. "Save MP4" wrote a 320 x 240, 51-frame H.264 file matching `recdr export` (bytes differ only in the container's timestamps). During the export the line said "exporting" and the three buttons were disabled. The error case was not reached: given a path in a folder the user could not write, the Windows save dialog saved under Videos instead (that file was removed). Owner: a target the dialog accepts but recdr cannot write.
22. Windows, pass. Closing the main window during a recording ended the process with exit code 0 and left the recording `finished`. "Close" in the toolbar and closing the editor window also exit 0.
23. Windows, partly. With both switches off the recording's `state.json` has `"cursor": false` and `"clicks": false`, and the manifest has no clicks. The pointer was not inside the frame, so the stored frames do not show whether the cursor would have been drawn. Owner: record with the pointer inside the frame.
