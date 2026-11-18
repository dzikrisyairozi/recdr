# Goldfish review: tasks/4-macos-capture/plan.md

| Reviewer | Status |
|---|---|
| readiness | completed, two rounds |
| codebase | completed |

Round 1 ran both lenses on Opus. The readiness lens raised one Blocker, which was fixed, so that lens ran a second time on the fixed plan. Findings 1 to 21 are round 1. Round 2 found no Blocker; its 6 Majors and 4 Nits are findings 22 to 31. Where both lenses raised the same thing it is one finding with both named, at the higher of the two severities.

Counts: 1 Blocker, 17 Major, 13 Nit. Decisions: 31 Fix, no Contest, no Escalate, no Decline.

## Findings

### 1. `tests/tabs.rs` would fail on macOS and was outside the File boundary (Blocker)

- Lens: readiness (codebase rated it Major)
- Where: File boundary; todo.md Task 5
- Problem: after `cdp-capture`, `tests/tabs.rs` has `#[cfg(not(windows))] mod unsupported`, which asserts that `recdr windows`, `recdr monitors` and a monitor recording answer `unsupported_platform`. From Task 5 they do not on macOS, the test is not `#[ignore]`, and the file was in no task.
- Why: the macOS CI job goes red at Checkpoint B, and the fix touches a file the plan did not allow.
- Decision: Fix
- Evidence or edit: new contract "`tests/tabs.rs`", the file is in the File boundary, and Task 5 changes the attribute to `#[cfg(not(any(windows, target_os = "macos")))]`.

### 2. Task 4 did not cover two of the eight unchecked items (Major)

- Lens: readiness
- Where: todo.md Task 4
- Problem: no step for how often the macOS 15 alert appears, none for a stream opened by a detached recorder, and no place where the build time is measured.
- Why: Task 10 and the README depend on the detached answer.
- Decision: Fix
- Evidence or edit: Task 4 is now ten numbered steps, each a subcommand of the try-out program, with the spec item each answers.

### 3. Task 4 left the tap's and the fallback's mechanics to the build (Major)

- Lens: readiness
- Where: plan "`src/capture/tap.rs`", decision 6
- Problem: undecided: how `Drop` stops a run loop on another thread, how the callback reaches its sender, what is reported back, and for the resize fallback which value announces a new size and how the stream is changed.
- Why: these would be designed during Tasks 8 and 9.
- Decision: Fix
- Evidence or edit: the tap contract now gives the struct's fields, the report (`Option` of the run loop), `user_info` and when it is freed, `CFRunLoopStop` in `Drop`, and the wrapper if the type is not `Send`. Task 4 step 3 and the `macos.rs` contract name the two fallback facts to write down.

### 4. Task 4's CI run had no place (Major)

- Lens: readiness
- Where: todo.md Task 4
- Problem: no location for the program, no workflow step, and no word that it needs a pull request.
- Why: the builder would invent a workflow in the middle of a measuring task.
- Decision: Fix
- Evidence or edit: decision 12 (branch `spike/4-sck`, directory `spike/sck/`, never merged) and Task 4 step 9 with the step's text.

### 5. The channel between the stream and the recorder was not specified (Major)

- Lens: readiness
- Where: plan decision 5 and `next_event`
- Problem: no item type, no rule for the stop notice when the channel is full, and no answer for a disconnected channel.
- Why: a stop notice sent with `try_send` into a full channel is lost, and a closed window then never ends the recording.
- Decision: Fix
- Evidence or edit: `enum Item { Frame(Frame), Stopped(String) }`; frames use `try_send`, the stop notice the blocking `send`; a disconnected channel is `capture_failed` with a fixed text.

### 6. The tap's callback did not say what to do when macOS disables the tap (Major)

- Lens: readiness
- Where: plan "`src/capture/tap.rs`"
- Problem: a tap callback also gets the two "tap disabled" notices.
- Why: clicks could stop without a sign, or be sent with a made-up button.
- Decision: Fix
- Evidence or edit: the callback is specified by event type: a timeout notice switches the tap on again, every other type is ignored. Task 9 has a by-hand check with clicks at both ends of a 30 second recording.

### 7. The test window's title was not unique (Major)

- Lens: readiness
- Where: plan "`tests/desktop.rs`"; todo.md Tasks 6 and 8
- Problem: every test window was titled `anim`, and `--window` matches a part of a title. The plan also did not say how a test finds its own window or where the expected process name comes from.
- Why: criteria 2 and 4 would be flaky on the owner's Mac.
- Decision: Fix
- Evidence or edit: decision 11: the pages take their title from the address's fragment, and a test opens `/anim#<title(name)>`. The contract gives the three lines that get a window, the two titles of the ambiguity test, and what is asserted about `process`.

### 8. The values the tests compare against were left open (Major)

- Lens: readiness
- Where: criterion 3 and criterion 1 tests
- Problem: Chrome's window bounds need not equal what ScreenCaptureKit captures, and `system_profiler` may list displays in another order.
- Why: a wrong expected value fails a correct implementation.
- Decision: Fix
- Evidence or edit: the contract fixes each: sorted lists for criterion 1; `Client::window_size` times `devicePixelRatio` within 2 pixels for criterion 3, confirmed or changed by Task 4 step 2; and for `--process`, success or an `ambiguous_target` that includes the window.

### 9. The check of the tap's teardown had nobody to run it (Major)

- Lens: readiness
- Where: todo.md Task 9
- Problem: "a second recording in the same process records clicks too" needs a real mouse press and was not marked by hand.
- Why: the one check of teardown would not be run.
- Decision: Fix
- Evidence or edit: decision 14: every recording has its own process, so the case does not exist. What must hold is that dropping a tap returns, and `start_then_drop_returns` tests that.

### 10. `tests/desktop.rs` kept `#![cfg(windows)]`, and Windows-only items stayed at the top (Major)

- Lens: codebase (readiness rated it Nit)
- Where: plan "`tests/desktop.rs`", decision 9
- Problem: the plan said the file is compiled on both systems but not what the attribute becomes, and it did not place `title` or the import of `test_window`.
- Why: an unused import fails `clippy -D warnings` on macOS.
- Decision: Fix
- Evidence or edit: the contract gives the new attribute, what stays at the top, and the `use` lines of each module. Task 5 has an acceptance line for clippy on both systems.

### 11. The message of `unsupported_platform` would say "need Windows" (Major)

- Lens: codebase
- Where: plan "`src/error.rs`"
- Problem: the cdp plan's text is false once macOS records the desktop.
- Why: Linux users would be told the wrong thing.
- Decision: Fix
- Evidence or edit: the contract gives the new text; "Before Task 1" finds the tests that quote the old one, and Task 5 changes them.

### 12. `Server::start` was relied on and not checked (Major)

- Lens: codebase
- Decision: Fix
- Evidence or edit: it is in the "exists after `cdp-capture`" list and in "Before Task 1".

### 13. "Before Task 1" named the wrong task for the two test modules (Nit)

- Lens: readiness
- Decision: Fix
- Evidence or edit: Task 5 adds them; "Before Task 1" no longer names a task.

### 14. The layer table and Task 9 disagreed about `tap.rs` (Nit)

- Lens: readiness
- Decision: Fix
- Evidence or edit: the `objc2_core_graphics` line is whole in Task 5; Task 9 adds the `objc2_core_foundation` line and rule 3.

### 15. Which tests gain a recording in Task 8 was hard to read (Nit)

- Lens: readiness
- Decision: Fix
- Evidence or edit: Tasks 6 and 8 name the test and its two halves.

### 16. Two stubs were left open (Nit)

- Lens: readiness
- Decision: Fix
- Evidence or edit: in Tasks 5 and 6 the macOS `desktop_source` returns a fixed error and `SckSource` does not exist; between Tasks 7 and 8 a window target gets a fixed error.

### 17. `to_pixels` did not say whether a display's edge belongs to it (Nit)

- Lens: readiness
- Decision: Fix
- Evidence or edit: half-open ranges in the contract, and a test case in Task 2.

### 18. The ending-Chrome test did not say how it ends Chrome (Nit)

- Lens: readiness
- Decision: Fix
- Evidence or edit: Task 8: `start`, sleep, drop the `Browser`, poll `status`, then `stop`.

### 19. Task 10 asked for a README line without listing the README (Nit)

- Lens: readiness
- Decision: Fix
- Evidence or edit: the result goes into `acceptance.md` in Task 10, and Task 11 writes the README.

### 20. The sixth layer test was not mentioned (Nit)

- Lens: codebase
- Decision: Fix
- Evidence or edit: the `capture/mod.rs` contract says the two new files hold no `cfg`.

### 21. The snippet for `windows.rs` passed an `Option` as the size (Nit)

- Lens: codebase
- Decision: Fix
- Evidence or edit: the contract says `size` is the pair taken out of `self.size`.

## Round 2

### 22. Task 5 could not pass clippy on macOS (Major)

- Lens: readiness
- Where: todo.md Task 5; plan "`tests/desktop.rs`"
- Problem: `on_macos` imported `serve`, `wait_for_window` and `Browser` a task before any test used them, and the helpers at the top had no macOS caller yet.
- Why: the Task 5 commit fails `clippy -D warnings` on the Mac.
- Decision: Fix
- Evidence or edit: in Task 5 the module's only import is `use super::*;`, and Task 6 adds the import from `common`. The file gets `#![allow(dead_code)]`, as `tests/common/mod.rs` has, with the reason.

### 23. Windows and Linux had nowhere to be checked from Task 5 on (Major)

- Lens: readiness
- Where: plan decision 13; todo.md Tasks 2 and 5
- Problem: Phases 2 to 4 are built on the Mac and CI runs only on pull requests, yet tasks claimed that Windows is unchanged and Linux still refuses.
- Why: a break on another system would show several commits later.
- Decision: Fix
- Evidence or edit: decision 13 is rewritten. The draft pull request is opened at Checkpoint A, each task from Task 5 is pushed to it, and the three jobs are green before the next task. The build asks once whether it may push.

### 24. Task 4 step 6 used `setsid`, a command macOS does not have (Major)

- Lens: readiness
- Decision: Fix
- Evidence or edit: the program starts itself again with `Command` and `libc::setsid()` in `pre_exec`, as `platform::spawn_detached` does.

### 25. What Task 4 writes into the contract had no rules (Major)

- Lens: readiness
- Where: plan "`src/capture/macos.rs`", "`Cargo.toml`", "`tests/desktop.rs`"
- Problem: the exact calls, the cargo features, the `system_profiler` fields, the allowed difference for criterion 3 and the type of `listed`'s `layer` were each left to the builder at Task 4.
- Why: a measuring task should measure, and not choose.
- Decision: Fix
- Evidence or edit: the table "What Task 4 fills in, and the rule for each" under `macos.rs`, and an acceptance line in Task 4 that every row is filled in by its rule.

### 26. The fallback of decision 6 had no task (Major)

- Lens: readiness
- Where: plan decision 6; todo.md Task 8
- Problem: if ScreenCaptureKit stretches a resized window, the source had to restart its stream, with no step, signature or test for it.
- Decision: Fix
- Evidence or edit: decision 6 no longer describes a fallback to build. If Task 4 shows stretching, the build stops, the plan gains a task with its contract and test, and the owner approves it before Task 5.

### 27. Two results of Task 4 had no rule for the code (Major)

- Lens: readiness
- Where: todo.md Task 4 steps 4 and 5; plan `tap.rs` `start`
- Problem: nothing said what `next_event` does for a minimized window or one on another Space, and `start` judged refusal only by a null tap.
- Why: a tap that macOS creates and sends nothing would record no clicks and print no warning, against criterion 13.
- Decision: Fix
- Evidence or edit: only the stop notice ends a window recording, and a stop notice on minimize is `window_closed` too. `start` returns `None` with the warning when the permission is still refused after the request, without creating a tap.

### 28. Task 4 never checked the window title the tests rely on (Nit)

- Lens: readiness
- Decision: Fix
- Evidence or edit: step 2 opens a page that sets its title and prints the title ScreenCaptureKit reports. A difference stops the build.

### 29. How a test gets a tab id and a session was not said (Nit)

- Lens: readiness
- Decision: Fix
- Evidence or edit: `Client::tab_ids()` and `Client::attach(tab)`, and `launch` adds `--no-sandbox` under `CI` as `start` does.

### 30. The first window of the ambiguity test had no name (Nit)

- Lens: readiness
- Decision: Fix
- Evidence or edit: `title("one")`, which `title("two")` does not match.

### 31. Two places for the build time (Nit)

- Lens: readiness
- Decision: Fix
- Evidence or edit: the plan's "What was measured", in both.
