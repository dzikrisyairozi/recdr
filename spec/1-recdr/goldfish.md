# Goldfish review: spec/1-recdr/spec-agent-recorder.md

| Reviewer | Status |
|---|---|
| comprehension | completed |
| zoom-out | completed |

One round. No Blockers were raised, so no second round ran.

## Paraphrase (comprehension)

What this proposes: A Windows command-line program, `recdr`, written in Rust, that records a window, a screen region or a monitor through the Windows Graphics Capture API without showing a window of its own. It stores each changed frame as a PNG with a timestamp manifest, then encodes a GIF itself, or MP4/WebM through an `ffmpeg` executable on `PATH`. A recording can run for a fixed duration, for the lifetime of a wrapped command, or in a detached background process that later `stop`, `mark` and `status` commands control through files in the recording directory. When a recording finishes it writes a contact sheet of up to 12 keyframes and a per-frame `diff.json`, so an agent that cannot watch video can inspect the result as still images. The same functions are exposed as MCP tools over stdio.

How the existing system works, per this document: There is no existing system. The repository holds only `LICENSE` (MIT) and `README.md`, and everything in the spec is new. `capability-map.md` places this as the first of five modules, with `cdp-capture`, `macos-capture`, `native-mp4` and `gui-overlay` built on it later.

## Findings

### 1. Wrap-a-command example cannot work as written (Major)

- Lens: zoom-out
- Where: User story 3, target table, Non-goals
- Problem: The example wrapped `npx playwright test`, which runs headless by default, and a window target cannot name a window the wrapped command has not opened yet. No wait-for-window behavior was defined.
- Why: The headline example of the feature fails or records the wrong thing.
- Decision: Fix
- Evidence or edit: Story 3 now uses `--monitor 0 -- npx playwright test --headed`. Commands states that a window target is resolved once before recording and must exist, and points wrap users to `--monitor` or `--region`. "Waiting for a window that does not exist yet" added to Non-goals.

### 2. Click capture has no user story (Major)

- Lens: zoom-out
- Where: Cursor, clicks and markers; `src/input.rs`; Risks; criterion 12
- Problem: No story asks for click capture, and it brings a mouse hook, a second `unsafe` file and the security-software risk.
- Why: It is the highest-risk piece of the module.
- Decision: Escalate
- Evidence or edit: The owner chose click highlight for this module when asked in chat. Recorded as open question 4.

### 3. `--no-cursor` could force replacing the capture crate (Major)

- Lens: zoom-out
- Where: Tech stack, unknown (b)
- Problem: A flag with no story could trigger the fallback of hand-written WGC code.
- Why: Largest possible cost increase for the smallest feature.
- Decision: Fix
- Evidence or edit: Tech stack now says: if (b) is missing, `--no-cursor` is dropped and the crate stays. Only (a) triggers the fallback.

### 4. Surfaces with no user story (Major)

- Lens: zoom-out
- Where: Commands, options, MCP tools
- Problem: `--format webm`, `recdr export` and its MCP tool, `--process`, `--foreground`, `frames --every`, and the statement that several recordings can run at once.
- Why: Each becomes a frozen interface after the first release.
- Decision: Escalate
- Evidence or edit: `export` (command and tool) and the concurrent-recordings sentence were the author's additions and are removed; the tool count is now eight and re-encoding is a Non-goal. WebM, `--process`, `--foreground` and `--every` were chosen by the owner in chat. Recorded as open question 5.

### 5. Repository root is ambiguous (Major)

- Lens: comprehension
- Where: Objective, Project structure
- Problem: `D:\recdr` and `D:\recdr\recdr` each hold a `.git`, `LICENSE` and `README.md`.
- Why: The Cargo package could be created in the wrong checkout.
- Decision: Fix
- Evidence or edit: The spec's opening now defines the repository root as the directory holding `LICENSE` and `spec/`. The nested clone is a stray duplicate the owner asked to have deleted; removal is pending on the owner.

### 6. `start` can hang when no first frame arrives (Major)

- Lens: comprehension
- Where: Control model
- Problem: `start` waited for a first frame, but a minimized window delivers none, and a recorder that dies early was not covered.
- Why: `start` could block forever.
- Decision: Fix
- Evidence or edit: `start` now waits up to 5 seconds for `status: recording` in `state.json`, else `recorder_start_failed`. A minimized target fails up front with `target_minimized`.

### 7. `stop` has no crash path (Major)

- Lens: comprehension
- Where: Control model
- Problem: `stop` waited for `result.json`, which never appears after a crash.
- Why: An agent would block with no result.
- Decision: Fix
- Evidence or edit: `stop` checks recorder liveness while waiting and fails with `recorder_crashed`; unknown id is `recording_not_found`; `stop` on a finished recording prints the existing result.

### 8. How an id resolves to a directory (Major)

- Lens: comprehension
- Where: Commands
- Problem: Not stated whether id-taking commands accept `--out`, or what the MCP server's output root is.
- Why: `stop <id>` could not find a recording started elsewhere.
- Decision: Fix
- Evidence or edit: `--out` is now listed for every command that touches recordings, with the rule that it must match. The MCP server's root is `.recdr` under its start directory unless `out` is passed. `start` returns an absolute `dir`.

### 9. Wrapped command's stdout breaks the one-JSON-object rule (Major)

- Lens: comprehension
- Where: Output of every command
- Problem: The child's output was not addressed.
- Why: Either the JSON contract breaks or test output disappears.
- Decision: Fix
- Evidence or edit: The child's stdout and stderr both go to recdr's stderr. Criterion 4 now checks this.

### 10. Error enum does not cover the failures described (Major)

- Lens: comprehension
- Where: Code style
- Problem: Four variants shown, more failures implied, nothing said the list was partial.
- Why: Error codes are a stable contract.
- Decision: Fix
- Evidence or edit: New "Error codes" section with the full list of 14. The code sample is labelled as a style sample. Criterion 6 covers the table.

### 11. Frame difference undefined; criteria 8 and 9 conflict (Major)

- Lens: comprehension
- Where: Recording directory, Agent view, criteria 8 and 9
- Problem: "Differs" had no definition, and a static screen could not yield two keyframes.
- Why: Two criteria could not both pass.
- Decision: Fix
- Evidence or edit: Storing uses byte-for-byte equality. A pixel counts as changed for keyframes when a channel differs by more than 16. A one-frame recording has one keyframe; criterion 8 now requires at least 1.

### 12. Cursor is in stored frames while rings are kept out (Major)

- Lens: comprehension
- Where: Cursor, clicks and markers
- Problem: Not stated whether a moving cursor affecting stored frames and `diff.json` is intended.
- Why: It changes what criterion 9 measures.
- Decision: Fix
- Evidence or edit: The section now says the cursor is part of the picture on purpose, that it creates stored frames but not keyframes, and that rings are left out because they are annotations. Criterion 9 adds "with the mouse not moving".

### 13. JSON output shapes not specified (Major)

- Lens: comprehension
- Where: whole document
- Problem: Field names appeared only in passing.
- Why: The plan would have to invent a stable interface.
- Decision: Fix
- Evidence or edit: New "JSON output" section with fields for every command and the result object, and the three `status` values.

### 14. `diff.json` has no user story (Nit)

- Lens: zoom-out
- Where: Agent view, criterion 8
- Problem: No story reads it.
- Why: A second output format to keep stable.
- Decision: Decline
- Evidence or edit: The owner chose the frame diff report in chat; the changed-fraction data is computed for keyframes anyway.

### 15. Write mechanism fixed before measurement (Nit)

- Lens: zoom-out
- Where: Risks
- Problem: Thread and bounded queue named before any measurement.
- Why: Criterion 15 is the measurement.
- Decision: Fix
- Evidence or edit: The risk row now keeps the counter and the criterion and leaves the mechanism to the plan.

### 16. Choices justified by unspecified future modules (Nit)

- Lens: zoom-out
- Where: Alternatives considered, Project structure
- Problem: "Works unchanged on macOS" and "gains two more implementations".
- Why: Invites shaping the trait for backends nobody has designed.
- Decision: Fix
- Evidence or edit: Both phrases removed. The trait is justified by the WGC backend and the synthetic test source.

### 17. Plan-level detail in the spec (Nit)

- Lens: zoom-out
- Where: Control model, Code style, Project structure
- Problem: 50 ms poll interval, full error enum, per-file layout.
- Why: Changing them should not need a spec edit.
- Decision: Fix
- Evidence or edit: The interval is gone; criterion 3 now bounds stop latency at 500 ms. The enum is labelled a sample. Project structure says the plan may split or merge files; the layout itself stays because the spec template asks for it.

### 18. `--duration` and time grammar (Nit)

- Lens: comprehension
- Where: Commands
- Decision: Fix
- Evidence or edit: `--duration` added to the options table; `<time>` grammar defined; `--max-duration` stated to apply in every mode.

### 19. `--no-click-highlight` described two ways (Nit)

- Lens: comprehension
- Where: Commands, Risks
- Decision: Fix
- Evidence or edit: The option now says no hook, no clicks in the manifest, no rings.

### 20. How the recorder process is invoked (Nit)

- Lens: comprehension
- Where: Control model
- Decision: Fix
- Evidence or edit: "run through a hidden internal subcommand".

### 21. How frame hold times reach ffmpeg (Nit)

- Lens: comprehension
- Where: Encoding
- Decision: Fix
- Evidence or edit: Constant frame rate with repeated frames; criterion 7 now checks duration.

### 22. `frames` edge cases (Nit)

- Lens: comprehension
- Where: Agent view, MCP tools
- Decision: Fix
- Evidence or edit: `--every` defined by example; more than 8 frames in `get_frames` is the error `too_many_frames`.

### 23. Loosely used terms (Nit)

- Lens: comprehension
- Where: several
- Decision: Fix
- Evidence or edit: "caps" replaced by "time limits"; "the plan" and `1-recdr` defined at the top; window `id` defined as the window handle in decimal; `--fps` limiting explained.
