# Acceptance: 3-cdp-capture

One line per success criterion in `spec/3-cdp-capture/spec.md`, with what showed it. Browser tests are in `tests/browser.rs` and ran on Windows 11 with Chrome 158 (`cargo test --test browser -- --ignored --test-threads=1`, 17 passed; evidence in `evidence/T16-test.log`). CI runs the same tests on `ubuntu-latest` and `macos-latest`.

1. Pass. `tabs_lists_only_pages`.
2. Pass. `records_a_tab_at_its_viewport_size`, at scale factor 1 and 2.
3. Pass. `still_page_stores_one_or_two_frames`.
4. Pass. `animation_keeps_fifteen_frames_per_second`.
5. Pass. `tab_text_matches_title_and_url`, from both `record` and `start`.
6. Pass. `background_tab_is_recorded` and `minimized_window_is_recorded`.
7. Pass. `closing_the_tab_ends_with_tab_closed` and `ending_the_browser_ends_with_tab_closed`.
8. Pass. `navigation_does_not_end_the_recording` (to `localhost` from `127.0.0.1`).
9. Pass. `press_is_recorded_in_frame_pixels`, at scale factor 1 and 2, before and after a cross-origin navigation.
10. Pass. `no_click_highlight_installs_nothing`.
11. Pass. `tabs_on_a_closed_port_fails_quickly`, `record_and_start_on_a_closed_port_fail_to_connect` and `tab_flag_mistakes_are_usage_errors` in `tests/tabs.rs`.
12. Pass. `start_mark_stop_frames_on_a_tab`.
13. Pass. `navigation_does_not_end_the_recording` drives the tab through its own CDP client during the recording; every call returned without an error.
14. By hand, owner. Switch on remote debugging at `chrome://inspect/#remote-debugging` in Chrome 144 or later, then run `recdr tabs --cdp chrome` (click Allow once) and `recdr record --cdp chrome --tab-id <id> --duration 3s` (click Allow once); a GIF is printed. Switch remote debugging off and run both again: each fails with `cdp_connect_failed` and names `chrome://inspect/#remote-debugging`. The missing-file message is covered by `tabs_chrome_without_remote_debugging_says_how_to_switch_it_on`, and the file parsing by `tabs_through_each_endpoint_form`.
15. By hand, owner. Start Chrome with `--remote-debugging-port=9222 --user-data-dir=<empty dir>`, attach Playwright MCP with `--cdp-endpoint http://127.0.0.1:9222`, have an agent run `recdr record --cdp 9222 --tab-id <id> --duration 20s` while Playwright MCP drives the tab and the terminal covers the browser window, and check the GIF shows the actions. Repeat with agent-browser, passing the URL from `agent-browser get cdp-url` to `--cdp`.
    - 2026-11-19, the Playwright half, run by the agent on Windows 11 with a headless Chrome 158 on port 9333: Playwright's `connectOverCDP` (what `--cdp-endpoint` does) loaded a form page, `recdr start --cdp 9333 --tab "recdr form"` started, Playwright typed `recdr` and clicked Go, and `recdr stop` stored 5 frames and one click at (103, 160). The GIF shows the empty form, the typed name, the ring on Go with the page turned green, and the result: `evidence/acceptance-playwright-cdp-gif-frames.png`. Not run through Playwright MCP itself, whose configuration in this session has its own browser.
    - The agent-browser half was not done: `agent_browser_connect` to that Chrome and `agent_browser_open` each hung past 120 seconds. Left for the owner.
16. Pass. `lists_the_nine_tools` in `tests/mcp.rs` and `mcp_records_a_tab`.
17. CI. `desktop_commands_are_unsupported_off_windows` in `tests/tabs.rs` and `wraps_a_command_while_recording_a_tab` run on `ubuntu-latest` and `macos-latest`; the result is on pull request #7.
18. Pass. `broken_source_fails_to_start_with_its_own_code` in `tests/control.rs`.
19. Partly. Every other test passes on Windows, including `cargo test --test desktop -- --ignored` (12 passed; `criterion_15_dropped_frames` passes run alone). The real click during a monitor recording is left for the owner: click once during `recdr record --monitor 0 --duration 5s` and check `manifest.json` has one click and the GIF shows a ring.
20. CI. `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` run on all three systems in the workflow.
21. Pass. `tests/layers.rs`, six rules.
