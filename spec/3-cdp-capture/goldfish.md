# Goldfish review: spec/3-cdp-capture/spec.md

| Reviewer | Status |
|---|---|
| comprehension | completed |
| zoom-out | completed |

One round. No finding was a Blocker, so no second round was run. Several findings turned on how Chrome behaves, which the first draft had only read about, so the author measured it before triage with a throwaway script against Chrome 158 on Windows 11. The results are in the spec under "What was checked", and they changed the design in three places: a hidden tab is now recorded with focus emulation in place of being refused, the first-frame screenshot fallback is gone, and the click binding is added again after each navigation.

## Paraphrase (comprehension)

Written about the first draft.

What this proposes: `cdp-capture` adds a second kind of recording target to recdr: a browser tab, picked with `--tab-id` or `--tab` plus a `--cdp` endpoint (port, `http://` URL, `ws://` URL, or the word `chrome`). recdr attaches to an already-running Chromium browser as an extra CDP client, takes frames from `Page.startScreencast`, and feeds them into the existing frame store, encoders and agent view. Clicks come from a `mousedown` listener injected into the page and reported through a CDP binding, because the Windows mouse hook cannot see them. It adds a `tabs` command, a `list_tabs` MCP tool, three error codes and the `tab_closed` stop reason. Because tab capture needs no Windows API, the module also makes recdr build and run on Linux and macOS for tab targets, with CI on three hosted runners.

How the existing system works, per this document: `agent-recorder` is a Windows-only CLI and MCP server that records a window, region or monitor into a directory of PNG frames plus a manifest, then encodes a GIF or MP4 and writes a contact sheet and `diff.json`. A target is resolved once in the command the user ran. `start` then spawns a detached recorder process that reports through a state file, while `record` runs the same recorder in-process. Clicks come from a Win32 mouse hook and are drawn as rings at encode time. The code is layered, and `tests/layers.rs` enforces rules about which file may use which crate or `unsafe`.

## Findings

### 1. Two click paths, and rules that make the old one unreachable (Major)

- Lens: comprehension and zoom-out (both raised it)
- Where: "Clicks", "Project structure"
- Problem: tab clicks came through a new `CaptureEvent::Click` while desktop clicks stayed in `ClickHook`, which `src/recorder.rs` imports. With `input.rs` compiled only on Windows, `cfg` allowed in three files only, and `capture` unable to import `input` (same layer row), `recorder.rs` could not compile on Linux without breaking a stated rule.
- Why: the plan would have had to break one rule, and the spec did not say which.
- Decision: Fix
- Evidence or edit: one path for every source. `src/input.rs` becomes `src/capture/hook.rs`, owned by the Windows source, which reports clicks as `CaptureEvent::Click`. The recorder no longer names the hook. The layer table loses `input`; the `unsafe` rule and the `cfg` rule (now scoped to `src/`) are updated. Success criterion 19 checks a real desktop click still reaches the manifest.

### 2. The owner's everyday Chrome got the worst flow (Major)

- Lens: zoom-out
- Where: "The `chrome` endpoint", Alternatives, Decision 1, user story 3
- Problem: resolving the tab in the calling command and reconnecting in the recorder cost two Allow dialogs per recording, three for `list_tabs` then `start_recording`, patched with 30 and 65 second waits.
- Why: the one browser a person watches was the most tedious to use.
- Decision: Fix
- Evidence or edit: the recorder now does everything that touches the browser over one connection, so each command is one dialog. A new section, "Start-up errors from a detached recorder", says how `start` reports the recorder's own error. The rejected design moved to Alternatives.

### 3. The recorder's second connection was unspecified (Major)

- Lens: comprehension
- Where: "How a tab recording runs"
- Problem: the draft did not say what the resolving command handed to the recorder, nor what the caller saw when the recorder's connection failed. `start` and in-process `record` would have reported different codes for the same cause, and the 65-second figure was not derived.
- Why: `control::start` turns any recorder failure into `recorder_start_failed`.
- Decision: Fix
- Evidence or edit: there is no second connection now (finding 2). `start` reports the recorder's own code; success criteria 5 and 18 check it. The wait for a tab target is 40 seconds and the spec says what it is made of.

### 4. A `ws://` endpoint that shows the Allow dialog had 5 seconds (Major)

- Lens: comprehension
- Where: "The `chrome` endpoint", "Error codes"
- Problem: the longer waits applied only to `--cdp chrome`, while Chrome Beta, Edge and custom profiles were sent to the `ws://` form and show the same dialog.
- Why: the documented route for those browsers would time out.
- Decision: Fix
- Evidence or edit: the limits are now the same for all four forms: 5 seconds to reach the endpoint, 30 for the first reply.

### 5. "Front tab" was not the same thing as "visible" (Major)

- Lens: comprehension
- Where: "Words used here", step 3, `tab_not_visible`, Objective
- Problem: the error was defined as "not the front tab" but tested `document.visibilityState`, and the draft did not say what a minimized or fully covered window reports, while the Objective promised a covered tab could be recorded.
- Why: user story 3 and the Playwright case were at risk.
- Decision: Fix
- Evidence or edit: measured. A covered window and a minimized window both report `hidden` and send no frames, so the Objective's promise was false as drafted. `Emulation.setFocusEmulationEnabled` makes all three hidden cases draw at 57 to 60 frames per second without changing the screen. recdr now turns it on for the recorded tab; `tab_not_visible` and the visibility check are removed. Success criterion 6 covers a background tab and a minimized window.

### 6. No incoming events were named (Major)

- Lens: comprehension
- Where: "The CDP methods recdr uses", "Stop reasons"
- Problem: the method table was declared complete but did not say which events report a closed tab, a crash or a click.
- Why: detecting them might have needed a method that was not listed.
- Decision: Fix
- Evidence or edit: the section now has an event table. Measured: a closed tab gives `Target.detachedFromTarget` and a dead browser closes the socket, with no extra method. A renderer crash was not measured; it is in "Not checked" and in Risks.

### 7. `dropped_frames` had no meaning for a tab, and acknowledging first removed back-pressure (Major)

- Lens: comprehension and zoom-out (both raised it)
- Where: "How a tab recording runs" step 4, Boundaries, success criterion 4
- Problem: every frame was acknowledged on receipt, as an Always rule, and nothing said when the CDP source counts a drop. Criterion 4 capped drops at 5% and could pass trivially.
- Why: the PNG-or-JPEG decision hung on a number that was not defined.
- Decision: Fix
- Evidence or edit: recdr now acknowledges a frame once the recorder has taken it and 1/fps has passed, so the browser sends at most `--fps` frames per second and none are discarded. `dropped_frames` is 0 for a tab by definition; criterion 4 asks for at least 100 stored frames in 10 seconds. The Always rule is removed.

### 8. `agent-browser` was not defined (Major)

- Lens: comprehension
- Where: "Browsers this must work with", success criterion 15
- Problem: no link, no vendor, no command, and the four browsers were not mapped to the four `--cdp` forms.
- Why: a reader who does not know the tool could not run criterion 15.
- Decision: Fix
- Evidence or edit: the section is now a table with a link for Playwright MCP and agent-browser, how each is reached, and the `--cdp` form each uses. The form of the URL agent-browser prints is stated as not checked.

### 9. The fake CDP server duplicated the browser tests (Major)

- Lens: zoom-out
- Where: "Testing strategy", Risks
- Problem: a WebSocket server answering a dozen methods, written from the same reading of CDP as the client, with every path also covered by a browser test.
- Why: a second protocol implementation to keep in step, for a need the spec did not state.
- Decision: Fix
- Evidence or edit: removed. Unit tests read CDP messages from fixed JSON text, `tests/tabs.rs` covers what needs no browser, and `tests/browser.rs` covers the rest.

### 10. The tab id was not defined (Nit)

- Lens: comprehension
- Where: Commands, `--tab-id`
- Decision: Fix
- Evidence or edit: it is the CDP target id and does not change while the tab exists.

### 11. `cargo test -- --ignored` would run browser tests too (Nit)

- Lens: comprehension
- Where: Commands
- Decision: Fix
- Evidence or edit: the two ignored suites now have their own commands, `--test desktop` and `--test browser`.

### 12. Click scaling left details open (Nit)

- Lens: comprehension
- Where: "Clicks"
- Decision: Fix
- Evidence or edit: x and y each use their own ratio against the latest frame; buttons 0, 1, 2 map to `left`, `middle`, `right` and others are ignored; a click after the viewport changes size is listed as a limit and a non-goal.

### 13. The Unix signal was not named (Nit)

- Lens: comprehension
- Where: "Linux and macOS"
- Decision: Fix
- Evidence or edit: `SIGKILL` to the process group, and the Windows side is written as `taskkill /T /F`, which is what the code runs.

### 14. Tab matching repeated `single`, and `AmbiguousTarget` was said to be untouched (Nit)

- Lens: comprehension and zoom-out (both raised it)
- Where: Commands, "Project structure"
- Decision: Fix
- Evidence or edit: the spec now says `single` in `src/capture/mod.rs` is made to work for windows and tabs, and the `src/error.rs` line says `AmbiguousTarget` can hold tabs.

### 15. "Within 1 second of the start" of what; criterion 12 on a static page (Nit)

- Lens: comprehension
- Where: step 5, success criterion 12
- Decision: Fix
- Evidence or edit: the step is gone (finding 19), so nothing delays `start` on a static page.

### 16. How extension pages are excluded was not said (Nit)

- Lens: comprehension
- Where: "Words used here"
- Decision: Fix
- Evidence or edit: a tab is any target of type `page`, with no further filter. Measured: the browser's own interface is type `browser_ui`. Criterion 1 now says "no target of another type".

### 17. "The table above" pointed 150 lines up (Nit)

- Lens: comprehension
- Where: "Testing strategy"
- Decision: Fix
- Evidence or edit: the paragraph went with the fake server (finding 9).

### 18. The Objective presented portability as free (Nit)

- Lens: zoom-out
- Where: Objective, "Linux and macOS"
- Decision: Fix
- Evidence or edit: the Objective now says what the choice costs: a split of the process code by system, Unix versions, and CI on three systems. User story 5 names the Mac.

### 19. The screenshot fallback rested on an unchecked premise (Nit)

- Lens: zoom-out
- Where: step 5, Risks
- Decision: Fix
- Evidence or edit: measured. A page that is not changing gets one frame when the screencast starts. The fallback, `Page.captureScreenshot` and its risk row are removed.

### 20. Plan-level detail in the spec (Nit)

- Lens: zoom-out
- Where: "Code style", "Tech stack", Risks
- Decision: Fix
- Evidence or edit: the Cargo feature flags and the pre-decided fallback branch are removed. The listener source stays as the Code style sample, which the spec template asks for, with a sentence saying "Clicks" is what it must do.
