# Goldfish review: spec/6-gui-overlay/spec.md

| Reviewer | Status |
|---|---|
| comprehension | completed |
| zoom-out | completed |

One round, run on Opus. No finding was a Blocker, so no second round was run. The first launch of both reviewers failed on a usage limit before they produced anything; the reports below are from the second launch.

The comprehension reviewer reported 4 Major and 6 Nit, the zoom-out reviewer 1 Major and 4 Nit.

## Paraphrase (comprehension)

Written about the first draft.

What this proposes: a desktop app, opened with `recdr gui` on Windows and macOS, that draws a frame over the area to record, or over a window picked from a menu. The person presses Record and Stop, trims and crops in an editor, and saves a GIF or an MP4. The app adds no recording code: it uses the same `control` functions as the CLI and records stored frames only. The one new capability is export with a trim and a crop, a library function in `src/export.rs` that is also the command `recdr export` and the MCP tool `export`, and that never changes the recording directory.

How the existing system works, per this document: recdr has two front ends, the CLI and the MCP server, over `control`. `control::start` runs the current executable again with a hidden first argument to start a detached recorder. The recorder writes stored frames and a manifest, and writes `recording.gif` or `recording.mp4` only when `RecordOptions.formats` names them. `control::frames` returns stored-frame paths by time. `encode::timeline` and `encode::render` build the pictures of a recording with click rings. `tests/layers.rs` enforces the layers.

## Findings

### 1. A system call in `platform/` broke the front-end rule (Major)

- Lens: comprehension
- Where: "Project structure", layer rules
- Problem: the spec said a system call needed for the frame "goes in `platform/`". The same list says `gui` imports only `control`, `request`, `model` and `error`, and `platform/` cannot take an `egui` window.
- Why: the plan would have had to break one of two rules.
- Decision: Fix
- Evidence or edit: the app is built with what `egui` offers. If the first checks show a system call is needed, the spec is changed before anything else is built. Boundaries and the Risks row say the same.

### 2. Unit tests on Linux for code that is not compiled there (Major)

- Lens: comprehension
- Where: "Testing strategy", layer rules
- Problem: the tests of `state.rs` and `geometry.rs` were said to run on all three systems, while `gui` was compiled on Windows and macOS only.
- Why: a criterion that cannot pass.
- Decision: Fix
- Evidence or edit: `src/gui/` has a logic part that names no `egui` and is compiled everywhere, and a windows part compiled on Windows and macOS. `src/gui/mod.rs` decides which, and joins the files where `cfg` may appear.

### 3. No mechanism for noticing a recording that ended elsewhere (Major)

- Lens: comprehension
- Where: "Recording"
- Problem: "is noticed within one second" did not say how, and the list of things the state function can ask for did not include it.
- Why: the behavior had no place in the design.
- Decision: Fix
- Evidence or edit: while it records, the app asks for the recording's status once a second with the library function behind `recdr status`. Asking for status is listed among the things the logic part decides.

### 4. No layout when the chosen window fills the monitor (Major)

- Lens: comprehension
- Where: "Idle: the overlay"
- Problem: the toolbar sits under or above the frame, and a maximized window leaves room for neither.
- Why: the common case of recording a maximized window had no defined result.
- Decision: Fix
- Evidence or edit: the toolbar then sits inside the frame's bottom edge. It is not recorded, because a window target records only the window's own pixels. Criterion 14 checks both halves by hand, and the unit tests cover the placement.

### 5. Export could not use the encoders as the spec described (Major)

- Lens: zoom-out
- Where: "Project structure", `recdr export` rules
- Problem: export was to write "through encode", with rendering taking a time range. `encode::gif::encode` and `encode::ffmpeg::encode` can only write into the recording directory, which export must not change, and neither file was on the list. A time range in rendering would also spread trim logic over `timeline` and every encoder's loop.
- Why: export would either break its own rule or copy the encoder loops, and `native-mp4` adds a third encoder.
- Decision: Fix
- Evidence or edit: export builds a manifest in memory for the kept part and hands it to the existing encoders. `render` takes a crop and each encoder takes the path to write to. Facts 4 and 5, a paragraph under "Project structure", the file list and a row in Alternatives say so. Criterion 4 is now a unit test on that manifest.

### 6. Four terms from the first spec were used without a definition (Nit)

- Lens: comprehension
- Where: throughout
- Problem: "synthetic source", "marks", "click rings" and "window target" were not defined or linked.
- Why: a reader new to the project cannot follow the criteria.
- Decision: Fix
- Evidence or edit: "Words used here" has a second list, "From the first spec".

### 7. `unsupported_platform` was presented as an existing code (Nit)

- Lens: comprehension
- Where: `recdr gui`
- Problem: the code is added by `cdp-capture` and is not in the code today.
- Why: a reader checking the code would not find it.
- Decision: Fix
- Evidence or edit: the introduction lists what the spec takes from `cdp-capture`, and the `recdr gui` paragraph says where the code comes from.

### 8. Three phrases described a future state as the present (Nit)

- Lens: comprehension
- Where: layer rules, Boundaries
- Problem: "files that already hold such decisions", `platform/` and "the three CI jobs" all come from `cdp-capture`.
- Why: the same as finding 7.
- Decision: Fix
- Evidence or edit: the new second paragraph of the introduction.

### 9. The layer table being changed was not linked (Nit)

- Lens: comprehension
- Where: "Project structure"
- Problem: the spec changed layer rules without saying where the rules are.
- Why: the reader cannot see the whole rule set.
- Decision: Fix
- Evidence or edit: the introduction says the table and rules are in the `cdp-capture` spec under "Project structure", as changed by the two specs after it.

### 10. "MP4 as the MP4 tests of the day" was vague (Nit)

- Lens: comprehension
- Where: "Testing strategy"
- Problem: the phrase did not say what an MP4 export test needs.
- Why: the plan could not tell when the test is skipped.
- Decision: Fix
- Evidence or edit: an MP4 export needs what an MP4 recording needs on that system, and is skipped where the MP4 recording tests are skipped.

### 11. An open question that was already decided (Nit)

- Lens: comprehension
- Where: "Open questions"
- Problem: question 2 described a change to the capability map that had been made, and said MP4 export is better after `native-mp4`, which the map does not say.
- Why: an open question should need an answer.
- Decision: Fix
- Evidence or edit: the map change is under "Decisions made while writing", and the sentence about `native-mp4` is gone.

### 12. New parsers for formats that already have one (Nit)

- Lens: zoom-out
- Where: "Project structure", the `src/request.rs` line
- Problem: `request::parse_region` reads `x,y,w,h` and `time::parse_time` reads the times `--duration` takes. The spec read as if `--crop`, `--from` and `--until` get new parsing.
- Why: a second parser can drift from the first.
- Decision: Fix
- Evidence or edit: fact 7, the `recdr export` rules and the file list all say the two existing parsers are used.

### 13. Recording with no format had no reason and no route (Nit)

- Lens: zoom-out
- Where: "Recording", Decisions
- Problem: the app was to record with an empty format list. No reason was given, and `request::parse_formats` refuses an empty value, so a front end cannot ask for that through `request`.
- Why: either `request` changes or the app skips the shared validation.
- Decision: Fix
- Evidence or edit: `--format` gains the value `none`, usable from every front end, and the app records with it. The reason is stated (Stop does not wait for a GIF nobody asked for) and the default-GIF option is a row in Alternatives. Criterion 9 tests it. This adds a value to an existing flag, which the owner should confirm.

### 14. A frame that follows the window was more than the user story asks (Nit)

- Lens: zoom-out
- Where: "Idle: the overlay", Risks
- Problem: following a moving window twice a second brought a polling loop, a risk row and half a criterion, while a window target records the window wherever it is.
- Why: behavior nobody asked for.
- Decision: Fix
- Evidence or edit: the frame is placed around the window once. Following is a non-goal and a row in Alternatives, and the risk row is removed.

### 15. The file split and the state-function shape were plan detail (Nit)

- Lens: zoom-out
- Where: "Project structure", "Code style"
- Problem: the spec fixed five files under `src/gui/` and a pattern for the state function, while saying the way the frame is built is decided by the plan's first throwaway task.
- Why: if that task goes the other way, the plan inherits a structure to argue against.
- Decision: Fix
- Evidence or edit: the spec fixes the line between the logic part and the windows part, and leaves the files to the plan.

## Counts

15 findings: 0 Blocker, 5 Major, 10 Nit. Decisions: 15 Fix, 0 Contest, 0 Escalate, 0 Decline.
