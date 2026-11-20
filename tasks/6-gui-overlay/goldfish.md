# Goldfish review: tasks/6-gui-overlay/plan.md

| Reviewer | Status |
|---|---|
| readiness | completed |
| codebase | completed, two rounds |

Round 1 ran both lenses on Opus. The codebase lens raised two Blockers, which were fixed, so that lens ran a second time on the fixed plan. Findings 1 to 21 are round 1. Round 2 found no Blocker and no Major; its 4 Nits are findings 22 to 25. Where both lenses raised the same thing it is one finding with both named, at the higher of the two severities.

Counts: 2 Blocker, 7 Major, 16 Nit. Decisions: 24 Fix, 1 Decline (finding 20), no Contest, no Escalate.

The review changed the spec in one more place (finding 6). The plan lists it under "Spec changes made while planning".

## Findings

### 1. `App` and `Event` could not derive `PartialEq` (Blocker)

- Lens: codebase (readiness rated it Major)
- Where: plan "`src/gui/state.rs`" and "`src/model.rs`"
- Problem: both hold `Vec<MonitorInfo>` and `Vec<WindowInfo>`, and neither type derives `PartialEq` in `src/model.rs`.
- Why: Task 7 would not compile, and its Files list had no `src/model.rs`.
- Decision: Fix
- Evidence or edit: the `model.rs` contract adds `PartialEq` to both, and Task 7 lists the file and the step.

### 2. The save dialog's file name broke layer rule 1 (Blocker)

- Lens: codebase
- Where: plan, the save bullet of the windows part
- Problem: `set_file_name("recording.gif")` in `src/gui/editor.rs`. The first layer test allows the texts `"recording.gif"` and `"recording.mp4"` in `src/store.rs` only, and reads test code too.
- Why: `cargo test --test layers` would fail in the editor task.
- Decision: Fix
- Evidence or edit: decision 15: the dialog proposes `export.gif` and `export.mp4`. The layer section says rule 1 binds this module, and Tasks 1 and 2 assert through `dir.gif_path()` or the file's extension.

### 3. The toolbar could end inside a recorded region (Major)

- Lens: readiness
- Where: plan `keep_inside` and `toolbar_place`
- Problem: a tall region near the top of a monitor fits the toolbar neither under nor above, so it went inside, over pixels the region records.
- Why: it breaks the spec's "never put the toolbar in a recording" and criterion 16, and the tests would have locked it in.
- Decision: Fix
- Evidence or edit: `keep_inside` ends by moving the frame up when the place would be `Inside`. Task 6 has the case, and a check over 200 rectangles that the place is never `Inside` after `keep_inside`.

### 4. Task 10 did not build (Major)

- Lens: readiness
- Where: todo.md Tasks 10 and 11
- Problem: `mod editor;` was added a task before `editor.rs` existed, and the live action loop could reach `Editing` with nothing to draw.
- Why: a task must leave the repository building.
- Decision: Fix
- Evidence or edit: Phase 3 is now four tasks. Task 10 creates the three files together, with a toolbar that records and stops and a placeholder editor; Task 11 adds the frame; Task 12 the look of a recording; Task 13 the editor.

### 5. "Leaves the `App` equal" contradicted `step` (Major)

- Lens: readiness
- Where: todo.md Task 7; plan `step`
- Problem: `step` sets `last_ms` on every event, so the `App` is never equal. `now_ms - last_ms` also panics on an earlier time.
- Decision: Fix
- Evidence or edit: `saturating_sub`, and the acceptance compares with a copy whose `last_ms` was set to the new time. A test passes an earlier time.

### 6. The example `1.5s` is refused by `parse_time` (Major)

- Lens: readiness
- Where: plan `ExportRequest`; spec, flag table of `recdr export`
- Problem: `time::parse_time` takes whole numbers only, and the spec says no second parser is written.
- Why: an agent following the tool's own description would get `usage`.
- Decision: Fix
- Evidence or edit: spec change 3: the example is `2s`. The doc comment says `"1500ms" or "2s"`.

### 7. How a drag becomes an event was left open (Major)

- Lens: readiness
- Where: plan, windows part
- Problem: undecided: what `Moved { x, y }` holds, from where a distance is counted, in which coordinates, and where points become pixels.
- Why: a border window moves under the pointer, so the wrong choice feeds on itself.
- Decision: Fix
- Evidence or edit: decision 13 and the drags bullet: the inside and the pointer's screen position at the press are kept, and each pass sends the pressed rectangle changed by the whole distance. Task 9 has a new question for how the screen position is read.

### 8. Closing the app between Record and the recorder's start left a recording running (Major)

- Lens: readiness
- Where: plan, state table
- Problem: `CloseRequested` in `Idle` quit at once, also while a `Start` was running on a worker thread.
- Why: the spec says closing the app stops the recording first.
- Decision: Fix
- Evidence or edit: `Idle` has a `closing` flag. Closing while starting waits for `Started`, stops that recording and then quits; a `Failed` quits. Task 7 tests the sequence.

### 9. The MP4 export test did not skip where the MP4 recording tests skip (Major)

- Lens: codebase
- Where: todo.md Task 4, `export_mp4`
- Problem: it skipped only on `ffmpeg_not_found`. On a Windows runner with no encoder the code is `encode_failed`, and the `native-mp4` plan has a skip for that.
- Why: the Windows CI job would fail.
- Decision: Fix
- Evidence or edit: `export_mp4` has both skips, and "Before Task 1" notes whether the `native-mp4` skip went in.

### 10. `gif_facts` returned no frame count (Nit)

- Lens: readiness
- Decision: Fix
- Evidence or edit: it returns `GifFacts` with `frames`.

### 11. Task 2 used a test recording that no helper builds (Nit)

- Lens: readiness
- Decision: Fix
- Evidence or edit: it calls the `recording` helper of `native-mp4` with the picture described.

### 12. The worker did not say what an empty list gives (Nit)

- Lens: readiness
- Decision: Fix
- Evidence or edit: a `Failed` with a fixed text for each.

### 13. Some widgets had no event (Nit)

- Lens: readiness
- Decision: Fix
- Evidence or edit: "Close", "Clear crop", the frames-per-second widget, the 6 point zone of a handle, and the disabled buttons are in the contract.

### 14. A step of Task 5 did nothing (Nit)

- Lens: readiness and codebase
- Problem: `RecordRequest.format` has no doc comment, so there was no description to add `none` to.
- Decision: Fix
- Evidence or edit: Task 1 adds the doc comment, which is what the MCP tools show. The step in Task 5 is gone.

### 15. Task 9's answers had two homes (Nit)

- Lens: readiness
- Decision: Fix
- Evidence or edit: the plan's "What was measured".

### 16. "`recdr --help` lists `export` and `gui`" had no test (Nit)

- Lens: readiness
- Decision: Fix
- Evidence or edit: `help_lists_export_and_gui` in `tests/gui.rs`.

### 17. "Before Task 1" did not check who calls `render` (Nit)

- Lens: readiness
- Decision: Fix
- Evidence or edit: added, and the `encode` contract says where it is called after `native-mp4`.

### 18. "Six layer tests" is seven after `native-mp4` (Nit)

- Lens: codebase
- Decision: Fix
- Evidence or edit: the count is gone from the cdp line, and the `native-mp4` line names the seventh.

### 19. How `write` returns the count was not said (Nit)

- Lens: codebase
- Decision: Fix
- Evidence or edit: `pushed.and_then(|count| finished.map(|()| count))`, with the rule it keeps.

### 20. `recdr gui` on Linux prints a message about capture (Nit)

- Lens: codebase
- Problem: `unsupported_platform` has one text, and it is about window, region and monitor capture.
- Decision: Decline
- Evidence or edit: the spec fixes only the code. A text of its own means a change to `src/error.rs` and to tests of two earlier modules, for a command whose README entry says where it runs. The plan records the choice as decision 14.

### 21. The `gui` layer list left out `crate::{` without saying why (Nit)

- Lens: codebase
- Decision: Fix
- Evidence or edit: it is in the list.

## Round 2

### 22. Decision 14 quoted half of the message (Nit)

- Lens: codebase
- Problem: the text of `unsupported_platform` ends "; on this system recdr records browser tabs with --cdp", which means nothing to someone who asked for the app, and the plan named the cdp plan as the source of a wording that `macos-capture` sets.
- Decision: Fix
- Evidence or edit: decision 14 quotes the whole text, says its second half does not help, and keeps the choice of finding 20. The list of what is taken from `macos-capture` has the text.

### 23. The contract and the task disagreed about a test in `gif.rs` (Nit)

- Lens: codebase
- Problem: `a_click_adds_frames_for_its_ring` reads the path `encode` returns, which becomes a count.
- Decision: Fix
- Evidence or edit: the contract names the test and says it reads the path it passed, as Task 2 already did.

### 24. "Test code included" overstated layer rule 1 (Nit)

- Lens: codebase
- Problem: `source_files()` walks `src/` only, so files under `tests/` are not held to the rule.
- Decision: Fix
- Evidence or edit: decision 15 and the layer section say the rule covers every file under `src/` with its test modules, and not `tests/`.

### 25. `geometry::to_pixels` and `to_points` share names with two functions in `capture` (Nit)

- Lens: codebase
- Decision: Fix
- Evidence or edit: a paragraph says the `capture` ones convert a rectangle with a list of displays, and that rule 4 keeps the app from using them.
