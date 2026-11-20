# Goldfish review: tasks/3-cdp-capture/plan.md

| Reviewer | Status |
|---|---|
| readiness | completed |
| codebase | completed, two rounds |

Round 1 ran both lenses. The codebase lens raised one Blocker, which was fixed, so that lens ran a second time on the fixed plan. Round 2 found no Blocker and no Major, and its five Nits are findings 14 to 18. Where both lenses raised the same thing it is one finding with both named.

Before the review, the author measured the plan's assumptions with a throwaway Rust program against Chrome 158 (the list is in the plan under "What was measured for this plan"). Those measurements changed the spec in three places, listed in the plan under "Spec changes made while planning".

## Findings

### 1. A unit test in `src/capture/cdp.rs` needed `unsafe` (Blocker)

- Lens: codebase and readiness (readiness rated it Major)
- Where: todo.md Task 8, last acceptance bullet; plan "`tests/layers.rs`" rule 3
- Problem: the test set `RECDR_CHROME_USER_DATA_DIR` in the test process. `Cargo.toml` has `edition = "2024"`, where `std::env::set_var` is unsafe, and rule 3 forbids `unsafe` in that file, test modules included.
- Why: Task 8 could not pass `cargo test` as written.
- Decision: Fix
- Evidence or edit: the plan adds `fn chrome_url(dir: &Path) -> Result<String>`, which takes the directory as an argument, and Task 8 tests that. The check of the environment variable moved to `tests/tabs.rs` in Task 9, where it is set on the child process with `Command::env`.

### 2. `code()` returning `&str` breaks a test in `src/request.rs` (Major)

- Lens: codebase
- Where: plan "`src/error.rs`"; todo.md Task 1 Files
- Problem: `src/request.rs` line 265 keeps `.unwrap_err().code()` in a variable after the error is dropped, which compiles only while `code()` returns `&'static str`.
- Why: Task 1 would fail to compile in a file it did not list.
- Decision: Fix
- Evidence or edit: the plan names the line and the change under `src/error.rs`, and Task 1 lists `src/request.rs` with the step.

### 3. `ResolvedKind::Tab` breaks the match in `WgcSource::open` (Major)

- Lens: codebase and readiness
- Where: todo.md Task 11; plan "`src/capture/windows.rs`"
- Problem: `WgcSource::open` matches `resolved.kind` with no wildcard arm, and Task 11 neither named the new arm nor listed the file.
- Why: the Windows build would fail in Task 11.
- Decision: Fix
- Evidence or edit: the contract says the last arm becomes `ResolvedKind::Fake(_) | ResolvedKind::Tab(_)` and the match is on `&resolved.kind`. Task 11 has the step and the file.

### 4. The MCP browser test would have recorded the synthetic source (Major)

- Lens: codebase and readiness
- Where: todo.md Task 15
- Problem: `Server::start` sets `RECDR_FAKE_SOURCE=moving` and was to move "unchanged". The override replaces any target, so `mcp_records_a_tab` would pass without recording a tab.
- Why: criterion 16 would be marked as met without being checked.
- Decision: Fix
- Evidence or edit: `Server::start(fake: Option<&str>)`; `tests/mcp.rs` passes `Some("moving")` and the browser test `None`. The test also compares `width` with the tab's own size, which the synthetic source's 320 cannot equal.

### 5. A failed in-process `record` left a directory that reads as `recording` (Major)

- Lens: codebase
- Where: plan "`src/control.rs`"
- Problem: only `run_recorder` wrote `State.error`. A tab is now found inside the recorder, after `prepare` made the directory, so a failed `recdr record --cdp ...` left status `Starting` with pid 0, which `derived_status` calls `recording` for 40 seconds.
- Why: `mark` would succeed on a dead recording and `stop` would block.
- Decision: Fix
- Evidence or edit: `run_noting_error` is used by both `record` and `run_recorder`. Task 1 has a second test, `failed_record_is_crashed_at_once`. The doc comment on `prepare` is corrected.

### 6. The new click path had no automated check (Major)

- Lens: codebase (readiness raised it as a Nit)
- Where: todo.md Task 3 and Checkpoint A; plan Risks; spec "Testing strategy" and Risks
- Problem: `tests/desktop.rs` has no click test, and the spec said the desktop tests cover clicks. After Task 3 the hook drain, the origin subtraction and the bounds filter were checked only by one click by hand.
- Why: a regression in the moved filter would not be caught.
- Decision: Fix
- Evidence or edit: the filter is a private function `to_event` with a unit test (five cases in Task 3), next to the unit test of the recorder's time conversion. No test sends a real mouse press, because an injected press lands on whatever is under the pointer; the plan says so, and the spec's Testing strategy row and Risks row now say the same in place of the false claim.

### 7. Several tasks would fail clippy at their own commit (Major)

- Lens: readiness
- Where: todo.md Tasks 5, 8, 9, 10, 11
- Problem: code was added one task before its first use: two fields of `Incoming`, four `Client` helpers and the test pages, `pick`, the `clicks` parameter and two fields of `CdpSource`, and on Unix the unused parameters of the desktop arm of `capture::open`.
- Why: `cargo clippy --all-targets -- -D warnings` ends every task, and the builder would have had to pick a workaround five times.
- Decision: Fix
- Evidence or edit: the plan now says which task adds each item ("Items arrive in the task that first uses them" under `src/capture/cdp.rs`, and the same for `tests/browser.rs`). `pick` and `TabQuery` moved from Task 10 to Task 11. The desktop arm is a function `desktop_source` with two definitions, the second with underscore parameters.

### 8. The navigation test could not fail (Major)

- Lens: readiness
- Where: plan "`tests/browser.rs`"; todo.md Task 13
- Problem: `serve` served one page, so both origins showed `GREEN` and the pixel check held with or without a frame after the navigation.
- Why: criterion 8 was not tested.
- Decision: Fix
- Evidence or edit: `serve()` serves `/anim`, `/still` and `/green` by path. The test goes from `http://127.0.0.1:<port>/anim` to `http://localhost:<port>/green` and checks that the first stored frame is white at the center and the last is green.

### 9. "Except for the last row" pointed at the wrong row (Nit)

- Lens: readiness
- Where: plan, `next_event` step 4
- Decision: Fix
- Evidence or edit: the sentence names `Target.detachedFromTarget` and says why it is outside the session.

### 10. Some error messages were left to the builder (Nit)

- Lens: readiness
- Where: plan "`src/request.rs`" and "`src/capture/cdp.rs`"
- Decision: Fix
- Evidence or edit: rules 2, 3 and 4 of `parse_endpoint` have their messages; a table gives the text of every `CdpConnectFailed`; Task 8 has four cases for `authority`.

### 11. Two conversions in `src/error.rs` were not stated (Nit)

- Lens: readiness and codebase
- Where: todo.md Tasks 1 and 10
- Decision: Fix
- Evidence or edit: the plan says how `info()` builds candidates while they are still `Vec<WindowInfo>`, and names the two tests Task 10 changes.

### 12. A missing dependency and a wrong pointer (Nit)

- Lens: readiness
- Where: todo.md Task 4 "Dependencies: none"; Task 5 "Contracts decision 7"
- Decision: Fix
- Evidence or edit: Task 4 depends on Task 2. Task 5 points to Contracts under `src/capture/mod.rs`.

### 13. `CdpSource` is committed one task before anything runs it (Nit)

- Lens: readiness
- Where: todo.md Task 11
- Decision: Decline
- Evidence or edit: Task 12 is the next commit and records a real tab. Joining them would make one task of ten files. Task 11 says where it is first exercised.

### 14. A paced frame could still be discarded without being counted (Nit)

- Lens: codebase, round 2
- Where: plan decision 2
- Problem: the extra millisecond kept the recorder's rate check from rejecting a tab frame only while the thread was not preempted for more than 1 ms between two clock reads. A rejected frame is dropped silently, which the spec says cannot happen.
- Decision: Fix
- Evidence or edit: `CaptureSource` gets `paced()` with a default of false; `CdpSource` returns true and the recorder skips its check for a paced source. The interval is `1000 / fps` with no margin.

### 15. Imports orphaned by Task 3 (Nit)

- Lens: codebase, round 2
- Where: plan "`src/recorder.rs`" and "`src/capture/hook.rs`"
- Decision: Fix
- Evidence or edit: the contract lists `ResolvedKind` (removed in Task 3, back in Task 11), `use crate::model::Click;` and the comment that names `finish`.

### 16. The README task did not name the lines this module makes stale (Nit)

- Lens: codebase, round 2
- Where: todo.md Task 17
- Decision: Fix
- Evidence or edit: Task 17 names the Windows-only lines, `recdr.exe`, the target table and the `--no-click-highlight` sentence, says the stop-reason, error-code and test-command lists are new, and has an acceptance line for it.

### 17. Not every measurement is in the spec (Nit)

- Lens: codebase, round 2
- Where: plan "What was measured for this plan"
- Decision: Fix
- Evidence or edit: the section says six of the nine are spec facts 12 to 17 and three are recorded only in the plan.

### 18. A fallback for a failure that cannot happen (Nit)

- Lens: codebase, round 2
- Where: todo.md Task 5 acceptance
- Decision: Fix
- Evidence or edit: the sentence about `tests/desktop.rs` failing to type-check on Unix is removed; nothing in it is Windows-only at compile time.
