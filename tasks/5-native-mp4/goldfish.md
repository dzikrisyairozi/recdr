# Goldfish review: tasks/5-native-mp4/plan.md

| Reviewer | Status |
|---|---|
| readiness | completed, two rounds |
| codebase | completed, two rounds |

Round 1 ran both lenses on Opus. Both raised a Blocker, the fixes went into the plan, and both lenses ran a second time on the fixed plan. Findings 1 to 19 are round 1. Round 2 found no Blocker; its 2 Majors and 14 Nits are findings 20 to 35. Where both lenses raised the same thing it is one finding with both named, at the higher of the two severities.

Counts: 2 Blocker, 9 Major, 24 Nit. Decisions: 34 Fix, 1 Decline (finding 33), no Contest, no Escalate.

## Findings

### 1. The `#[path]` include pointed at a file that does not exist (Blocker)

- Lens: readiness and codebase
- Where: plan "`tests/common/mp4.rs`", the include for the unit tests
- Problem: the include was written inside `mod tests` of `src/encode/mod.rs`. There Rust reads the path from `src/encode/tests/`, so `../../tests/common/mp4.rs` is `src/tests/common/mp4.rs`.
- Why: the contract is implemented as written, so the unit tests would not compile.
- Decision: Fix
- Evidence or edit: the include is now at file level of `src/encode/mod.rs` under `#[cfg(test)]`, where the same path is read from `src/encode/` and reaches the file. The plan says why, and that tests call `super::mp4::facts`. Task 3 adds it.

### 2. Task 5 left a test failing on Windows (Blocker)

- Lens: readiness
- Where: todo.md Task 5 and Task 6
- Problem: from Task 5 Windows encodes without ffmpeg, but `mp4_without_ffmpeg_is_refused_before_anything_is_written` still expected `ffmpeg_not_found`, and its change was in Task 6.
- Why: Task 5 could not end with `cargo test` passing.
- Decision: Fix
- Evidence or edit: the change to `tests/record.rs` moved into Task 5, which now lists the file. Task 6 is only the CI step.

### 3. A writer that was dropped without `finish` kept the file open (Major)

- Lens: readiness and codebase
- Where: plan `write` and `mp4`; todo.md Task 3, the test of a failed encode
- Problem: when `video_frames` failed, `write` returned and the writer was dropped. A dropped `Child` is neither killed nor waited for, and `VideoEncoder` has a thread holding the file, so `mp4` removed a file that was still open. The test also made the first frame fail, before ffmpeg had created anything.
- Why: on Windows the removal fails, and the test could pass without testing it.
- Decision: Fix
- Evidence or edit: decision 10 and the `write` contract: `finish` always runs, and an error from the frames wins. The writers' contract says `finish` returns only when the file is closed. The test now adds a second manifest entry that names a missing file, so frames are pushed first. A second test, `finish_with_no_frame_returns`, covers the case of no frame at all with a 10 second limit, and the Risks table says what happens if it hangs.

### 4. The checkpoints assumed that pushing a branch runs CI (Major)

- Lens: readiness
- Where: Checkpoints B and C; Task 7
- Problem: the cdp plan's workflow runs on pull requests and on pushes to `main` only.
- Why: the plan needed CI results and did not say how to get a run.
- Decision: Fix
- Evidence or edit: decision 12, and Checkpoints B and C now ask for a draft pull request. "Before Task 1" checks the trigger.

### 5. Task 7's try-out program had no place (Major)

- Lens: readiness
- Where: todo.md Task 7
- Problem: no location for the program, no way to run it in CI, and no check of its output on a hosted runner.
- Why: without a Mac this task is the only macOS signal, and each of these was left to the builder.
- Decision: Fix
- Evidence or edit: decision 11 and Task 7: branch `spike/5-avf`, directory `spike/avf/`, never merged; the program checks its own file (headers, and two pixels of a frame decoded by ffmpeg) and exits 1 when they are wrong; the CI step is given.

### 6. The fallback for a Windows runner with no encoder named the wrong files (Major)

- Lens: readiness
- Where: Risks, second row
- Problem: it said the change was in Task 6's files, but the encoder tests are in `src/encode/mod.rs`, and the new test had no name or assertion.
- Why: the builder would design it at a red checkpoint.
- Decision: Fix
- Evidence or edit: new section "If the Windows runner has no encoder", with the helper `encoder_missing`, the test `a_missing_encoder_is_reported`, and the skip in `tests/record.rs`.

### 7. Where the `#[should_panic]` test lives, and which task adds the include (Major)

- Lens: readiness
- Where: todo.md Task 1 and Task 4
- Problem: the test's file was not named, and "if Task 3 did not already need it" left the include's task open.
- Why: the location decides which task owns the include.
- Decision: Fix
- Evidence or edit: the test is `facts_refuses_a_file_that_is_not_an_mp4` in `tests/record.rs`, with its ten bytes given. Task 3 adds the include.

### 8. "Before Task 1" checked a different list than the plan set up (Major)

- Lens: codebase (readiness rated it Nit)
- Where: plan "When this plan was written"; todo.md "Before Task 1"
- Problem: the plan's list had `src/lib.rs`, which nothing uses, and the todo checked other things. Five files listed as "exists today" are changed by the cdp plan.
- Why: the safeguard against stale facts did not match the task list.
- Decision: Fix
- Evidence or edit: three lists now: not touched by `cdp-capture`; changed by it, with the named things this plan uses in each; and taken from the cdp plan. "Before Task 1" checks the second and third.

### 9. `FrameWriter::push` stores nothing for a repeated frame (Major)

- Lens: codebase
- Where: todo.md Task 2, the test with stored frames at 0 and 250 ms
- Problem: the test was to build two frames "as `one_frame` does", which pushes solid black. `push` returns `Ok(false)` for a frame equal to the last, so there would be one entry.
- Why: the assertion could not pass.
- Decision: Fix
- Evidence or edit: Task 2 says the test pushes a black frame and then a white one, and why.

### 10. Task 2 did not say what the ffmpeg closure does on a write error (Nit)

- Lens: readiness
- Decision: Fix
- Evidence or edit: Task 2: a local `broken` flag, no write after the first error, always `Ok(())`, then ffmpeg's exit status and stderr as today.

### 11. The skips in the encoder tests had no shape, and the criterion 11 test no name (Nit)

- Lens: readiness
- Decision: Fix
- Evidence or edit: `ffmpeg_runs` and `NEEDS_FFMPEG` in the test module, with the value for each task; the test is `check_mp4_leaves_no_file`.

### 12. The skip of `mp4_is_h264_of_the_recording_length` was ahead of macOS (Nit)

- Lens: readiness
- Decision: Fix
- Evidence or edit: `NEEDS_FFMPEG` in `tests/record.rs` is `cfg!(not(windows))` from Task 5 and is widened to macOS in Task 8.

### 13. Task 5 wrote `acceptance.md` without listing it (Nit)

- Lens: readiness
- Decision: Fix
- Evidence or edit: Task 5 lists it; the File boundary says Task 7 edits the plan.

### 14. Two contracts contradicted "every failure is `EncodeFailed`" (Nit)

- Lens: readiness
- Decision: Fix
- Evidence or edit: writers map I/O errors to `EncodeFailed`, the ffmpeg `finish` says so for `wait`, and `check_mp4` says a writer returns no other variant.

### 15. Task 6's duration assertion was not in the contract (Nit)

- Lens: readiness
- Decision: Fix
- Evidence or edit: the contract for `mp4_with_no_ffmpeg_on_path` has the 67 ms limit.

### 16. Criterion 12 had no way to time only the encode (Nit)

- Lens: readiness
- Decision: Fix
- Evidence or edit: the `#[ignore]` test `encodes_90_large_frames_in_time`, run by hand in release mode.

### 17. "Until Task 5 it is" described the file wrongly for Tasks 1 and 2 (Nit)

- Lens: codebase
- Decision: Fix
- Evidence or edit: the sentence gives the module lines for Tasks 1 and 2, then for Tasks 3 to 5.

### 18. Task 1 called the skip "still `!tool_runs("ffmpeg")`" (Nit)

- Lens: codebase
- Decision: Fix
- Evidence or edit: the contract quotes today's skip and note (`ffmpeg or ffprobe`) and the new ones.

### 19. `prepare` has a three-line `if`, not "two lines" (Nit)

- Lens: codebase
- Decision: Fix
- Evidence or edit: reworded.

## Round 2

### 20. `check_mp4` did not say whether a failed `push` is followed by `finish` (Major)

- Lens: readiness
- Where: plan "`src/encode/mod.rs`", `check_mp4` on Windows and macOS
- Problem: the steps were `create`, `push`, `finish`, then remove. On a failed `push` an early return drops the writer, which keeps the file open, and the removal then fails on Windows.
- Why: the check fails mostly on machines with no encoder, and there it would leave its temporary file behind, against the spec's "Always" rule.
- Decision: Fix
- Evidence or edit: the contract is `pushed.and(finished)` for a created writer, then `let _ = std::fs::remove_file(&path);` whatever the trial returned.

### 21. No task compared the bit rate rule with ffmpeg's file (Major)

- Lens: readiness
- Where: task list; spec, Risks, the bit rate row
- Problem: the spec says the plan compares a real recording with ffmpeg's file of the same recording, by eye and by size, before the rule is final. Nothing did.
- Why: `bit_rate` would be final with the spec's condition unchecked.
- Decision: Fix
- Evidence or edit: decision 13. Checkpoint A keeps a 10 second recording and ffmpeg's file; Task 5 has the `#[ignore]` test `encodes_a_kept_recording`, which encodes the same recording with the Windows writer; the decision gives the limit (three times the size, or visible damage) past which the build stops.

### 22. The two-frame test of Task 2 cannot use the `recording` helper (Nit)

- Lens: readiness
- Decision: Fix
- Evidence or edit: Task 2 says the test makes its own directory and manifest with one frame writer, and why.

### 23. Five tests had no names (Nit)

- Lens: readiness
- Decision: Fix
- Evidence or edit: `one_call_per_video_frame`, `a_call_gets_the_picture_of_its_time`, `even_adds_a_black_column_and_row`, `bit_rate_is_clamped`, `no_picture_fails_before_the_encoder`.

### 24. The MP4 reader could not reach a second track, and its panic had no path (Nit)

- Lens: readiness
- Decision: Fix
- Evidence or edit: `boxes` returns every box with a name, `parse` returns `Option` and reads through `slice::get`, and `facts` panics with the path when `parse` is `None`.

### 25. Task 5 read as correcting a choice Task 4 never stated (Nit)

- Lens: readiness
- Decision: Fix
- Evidence or edit: Task 4 says `first_frame` asks `ffmpeg_runs()`; the clause in Task 5 is gone.

### 26. Where `HINT` lives was open (Nit)

- Lens: readiness
- Decision: Fix
- Evidence or edit: a local `hint` chosen with `cfg!(windows)` inside the Windows and macOS definition of `check_mp4`. No constant.

### 27. Two gaps in the `avf.rs` list (Nit)

- Lens: readiness
- Decision: Fix
- Evidence or edit: the text of the 10 second timeout error, and `path.is_file()` before the removal so that a directory is left alone.

### 28. A Risks row named Task 6 for a test that is in Task 5 (Nit)

- Lens: readiness
- Decision: Fix
- Evidence or edit: the row names `mp4_with_no_ffmpeg_on_path` in Task 5.

### 29. The test's duration was 1 second where criterion 1 says 2 (Nit)

- Lens: readiness
- Decision: Fix
- Evidence or edit: the test's command changes to `--duration 2s`.

### 30. Whether the runner skip covers `check_mp4_leaves_no_file` (Nit)

- Lens: readiness
- Decision: Fix
- Evidence or edit: the section names it and `finish_with_no_frame_returns`.

### 31. The added frame entry repeated index 1 (Nit)

- Lens: readiness
- Decision: Fix
- Evidence or edit: `index: 2`.

### 32. Two checks had no form (Nit)

- Lens: readiness
- Problem: Task 5 wrote a time into `acceptance.md` with no format, and Task 6 asked for "valid YAML" with no tool.
- Decision: Fix
- Evidence or edit: the two lines of `acceptance.md` are given; Task 6 says the Checkpoint B run shows that GitHub accepts the file.

### 33. `tasks/5-native-mp4/plan.md` is in Task 7's Files and not in the File boundary (Nit)

- Lens: readiness
- Decision: Decline
- Evidence or edit: the boundary says files under `spec/` and `tasks/` may always be edited and that Task 7 edits the plan, and `/build` never checks those paths against it. Listing a plan under its own Modify list adds nothing.

### 34. `src/store.rs` was listed as not touched by `cdp-capture` (Nit)

- Lens: codebase
- Decision: Fix
- Evidence or edit: it is in the second list with the functions this plan uses, and "Before Task 1" checks them.

### 35. Two descriptions were loose (Nit)

- Lens: codebase
- Problem: the sixth layer test was described without its `cfg(test)` exception, which Task 3's include depends on; and "its four callers" of `one_frame` is three after Task 3.
- Decision: Fix
- Evidence or edit: the exception is stated with what it allows; the callers are named by file.
