# Acceptance run: recdr on Windows 11

Run on 2026-11-18, Windows 11 Home 10.0.26200, two monitors (1920x1080 at 0,0 and at -1920,0). Release build unless stated.

1. pass: `windows_prints_a_list`, `monitors_lists_index_zero_first` (tests/desktop.rs); `recdr windows` and `recdr monitors` checked by hand.
2. pass: `record --window-id <Docker Desktop> --duration 3s` gave a 1272x721 GIF that ffprobe reads as 3.06 s; the window's listed bounds are 1286x728, the difference being the invisible resize border. Window size also covered by `window_recording_is_about_the_window_size`.
3. pass: `start_returns_quickly_and_stop_ends_the_recorder` (tests/control.rs); by hand, capture ended 102 ms after `stop` was called and the recorder pid was gone 1 s later.
4. pass: `wrapped_command_sets_the_length_and_keeps_stdout_clean` (tests/record.rs) uses `ping -n 3` for the 2 s wait. The spec's `timeout /t 2` exits at once with "Input redirection is not supported" when stdin is not a console, as in this harness; recdr still printed one JSON line, `hi` went to stderr, and `stop_reason` was `command_exited`.
5. pass: `time_cap_finishes_a_detached_recording` (tests/control.rs, 1 s cap).
6. pass: every code has a test: `usage_errors`, `unknown_window_is_not_found`, `two_matching_windows_are_ambiguous`, `minimized_window_is_refused`, `region_off_every_monitor_is_out_of_bounds`, `mp4_without_ffmpeg_is_refused_before_anything_is_written`, `broken_source_fails_to_start_with_its_reason`, `stop_unknown_id_is_not_found`, `mark_lands_at_the_time_it_was_made` (recording_not_active), `killed_recorder_is_crashed`, `record_then_get_frames` (too_many_frames), `broken_source_is_capture_failed`, `a_zero_size_recording_fails_to_encode`, `output_root_that_is_a_file_is_io_error`.
7. pass: `mp4_is_h264_of_the_recording_length` and `mp4_without_ffmpeg_is_refused_before_anything_is_written` (ffmpeg 7.0.2 on PATH).
8. pass: `moving_gets_keyframes_a_diff_and_a_contact_sheet`, `static_screen_has_one_keyframe`, `pick_tiles_keeps_small_lists_and_spreads_long_ones`.
9. pass: a static WinForms window recorded for 5 s with `--no-cursor` stored 1 frame; `static_screen_stores_one_frame` on the synthetic source.
10. pass: by hand, three runs put the mark 26 to 27 ms after the `mark` command was invoked; the label is drawn on the contact sheet (checked by eye, and `a_mark_shows_in_the_first_tile_strip`).
11. pass: `at_one_time`, `max_width_writes_a_sample` (tests/frames.rs; 160 px, which also covers 512).
12. pass: by hand, an injected click at the centre of a 640x480 region was stored as `{x: 320, y: 240}` and drawn as a ring on the contact sheet; `a_click_adds_frames_for_its_ring` and `render_draws_rings_without_touching_the_stored_frame` show the GIF ring and the clean PNG. `--no-click-highlight` and `--no-cursor` with a real click were not checked by hand.
13. pass: `closing_the_window_ends_the_recording` (tests/desktop.rs).
14. pass for the protocol: `lists_the_eight_tools` and `record_then_get_frames` (tests/mcp.rs) show the eight tools and the contact sheet as an image block. Not run: registering with `claude mcp add` and asking Claude Code to record, which changes the owner's Claude Code configuration.
15. pass without the video condition: `criterion_15_dropped_frames` stored 259 frames and dropped 0 in release. The debug build first dropped 236 of 273 because PNG encoding was unoptimised; with dependencies built at opt-level 3 in the dev profile it drops 0. Not yet run with a full-screen video playing, which the criterion asks for.
16. pass: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`.
17. pass: `record --monitor 0 --max-size 1MB` ended with `size_cap_reached` after 2 frames (76 ms); `size_cap_stops_the_recording` unit test.
