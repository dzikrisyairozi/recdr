# Review remediation plan: macos-capture (PR #8)

PR: https://github.com/dzikrisyairozi/recdr/pull/8
Review: https://github.com/dzikrisyairozi/recdr/pull/8#pullrequestreview-5989267497
Reviewed head: `baec767` (`feat/4-macos-capture`), 2026-11-20. A fresh-context `code-reviewer` read the diff, the spec, the plan and `acceptance.md`; the main session triaged its findings. The review ran on Windows, so no macOS code was compiled or run for it. `tap.rs`, `macos.rs`, `capture/mod.rs` and `tests/desktop.rs` are the same at the stack top.

## Triage

| # | Finding | Severity | Decision |
|---|---|---|---|
| 1 | A stop sent before the tap thread's run loop starts is lost, and `Drop` never returns | Important | Fix |
| 2 | The CI skip removes the only macOS `--region` recording from CI | Suggestion | Fix |
| 3 | A failed display query becomes an empty list | Suggestion | Fix |
| 4 | Two desktop tests check less than their criteria say | Suggestion | Fix |
| 5 | `desktop_source`, `list_windows`, `list_monitors` have three `#[cfg]` copies each (over-engineering pass) | Suggestion | Fix, optional |
| 6 | `RawClick` alias is redundant next to `Press` (over-engineering pass) | Suggestion | Fix, optional |

### Notes on the triage

1. Confirmed by reading `src/capture/tap.rs:71-112`: `run` sends its run loop at line 111 and calls `CFRunLoop::run()` at line 112. `CFRunLoopStop` only sets the stopped flag when `_currentMode` is set, and each run resets the flag. The posted fix differs from the reviewer's: it calls `stop()` in a loop until `thread.is_finished()`, which needs no new objc2 call. Neither version was compiled.

2. The reviewer also offered a guess for the pale pixel on the runner: macOS 15 shows a repeating "bypass the private window picker" alert, and a translucent alert over `#00aa00` would give about (223, 250, 223). Not tested. A CI artifact of the monitor frame would settle it.

Licenses of the three new transitive crates the reviewer could not check (`apple-cf` 0.11.7, `apple-metal` 0.10.4, `doom-fish-utils` 0.4.3): all MIT OR Apache-2.0 on crates.io, checked 2026-11-20.

### Spec drift (owner decides; spec side unless noted)

- `objc2_core_foundation` is used in `macos.rs`; the spec's layer rule says `tap.rs` only. `tests/layers.rs:80-83` allows both.
- The test page titles are "moving box", "quiet page" and "green"; the spec's table says `anim`, `still`, `green`.
- Criterion 3 asks for a GIF between 2.5 and 3.5 s; `tests/desktop.rs:477` checks `duration_ms` and never reads the GIF. Change the test, or name `duration_ms` in the spec.
- The spec says CI runs the macOS desktop tests; the skip of the green region test is recorded only in `acceptance.md`.

## Phase 1: Important

### Task 1: Make the tap's stop impossible to lose

**Description:** `Drop for ClickTap` calls `self.run_loop.0.stop()` every millisecond until the tap thread has finished, then joins it.

**Acceptance criteria:**
- [ ] `start_then_drop_returns` passes 1,000 times in a row on a Mac.
- [ ] A recording of `--duration 100ms` with clicks on always finishes.

**Verification:**
- [ ] On macOS: `for i in $(seq 1000); do cargo test --lib start_then_drop_returns -- --exact || break; done`
- [ ] `cargo test --test desktop -- --ignored --test-threads=1` on macOS
- [ ] CI green on the macOS job

**Dependencies:** A Mac
**Files likely touched:** `src/capture/tap.rs`
**Estimated scope:** XS

### Checkpoint: Important
- [ ] macOS CI job green
- [ ] Windows and Linux jobs unchanged

## Phase 2: Suggestions

### Task 2: Keep a `--region` size check in macOS CI

**Description:** Move the 400x300 size assertion into its own desktop test and leave it in CI. Only the color check stays skipped on the runner.

**Acceptance criteria:**
- [ ] The macOS CI step runs a `--region` recording and asserts every frame is 400x300.
- [ ] The `--skip` covers only the color test.

**Verification:**
- [ ] macOS CI log shows the new test as run.

**Dependencies:** None
**Files likely touched:** `tests/desktop.rs`, `.github/workflows/ci.yml`, `spec/4-macos-capture/spec.md`
**Estimated scope:** Small

### Task 3: Report a failed display query

**Description:** `cg_displays` returns `Result<Vec<(u32, Display)>>` and its three callers use `?`.

**Acceptance criteria:**
- [ ] A non-zero status from `CGGetActiveDisplayList` gives `capture_failed`.

**Verification:**
- [ ] `cargo clippy --all-targets --target aarch64-apple-darwin -- -D warnings`
- [ ] macOS desktop tests

**Dependencies:** None
**Files likely touched:** `src/capture/macos.rs`
**Estimated scope:** XS

### Task 4: Make two desktop tests match their criteria

**Acceptance criteria:**
- [ ] The window-list test asserts `process` equals the started Chrome's application name.
- [ ] The monitor-size test takes its expected size from a source that does not share `display_scale`'s inputs.
- [ ] Criterion 3's test reads the GIF's length, or the spec names `duration_ms`.

**Verification:**
- [ ] macOS desktop tests

**Dependencies:** None
**Files likely touched:** `tests/desktop.rs`, `spec/4-macos-capture/spec.md`
**Estimated scope:** Small

### Task 5 (optional): One `desktop` alias per system

**Description:** `src/capture/mod.rs:142-209` picks the system's module with one `use windows as desktop` / `use macos as desktop` and one stub module, as `encode/mod.rs` does for `Writer`. About 40 lines less.

**Acceptance criteria:**
- [ ] `desktop_source`, `list_windows` and `list_monitors` each have one body.

**Verification:**
- [ ] clippy for Windows, `aarch64-apple-darwin` and `x86_64-unknown-linux-gnu`; `tests/layers.rs`

**Dependencies:** None
**Files likely touched:** `src/capture/mod.rs`, `src/capture/windows.rs`, `src/capture/macos.rs`
**Estimated scope:** Small

### Task 6 (optional): Have the Windows hook send `Press`

**Acceptance criteria:**
- [ ] `RawClick` is gone and `windows.rs` no longer converts each click.

**Verification:**
- [ ] Windows desktop tests

**Dependencies:** None
**Files likely touched:** `src/capture/hook.rs`, `src/capture/windows.rs`
**Estimated scope:** XS

### Checkpoint: Complete
- [ ] CI green on all three systems
- [ ] Spec drift items decided and the spec updated
