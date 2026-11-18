# Goldfish review: spec/5-native-mp4/spec.md

| Reviewer | Status |
|---|---|
| comprehension | completed (two rounds) |
| zoom-out | completed |

Run on Opus. The first launch of both reviewers failed on a usage limit before they produced anything; round 1 below is from the second launch. The comprehension reviewer raised a Blocker in round 1, it was fixed, and that lens ran a second time on the fixed spec.

In round 1 the comprehension reviewer reported 1 Blocker, 6 Major and 4 Nit, and the zoom-out reviewer 2 Major and 5 Nit. Both raised the check before recording, which is one finding here, so round 1 has 17.

## Paraphrase (comprehension)

Written about the first draft.

What this proposes: on Windows and macOS, `--format mp4` stops using ffmpeg and encodes H.264 with the encoder that ships with the system. On Windows that is the encoder wrapped by `windows-capture`; on macOS it is `AVAssetWriter`, reached through new `objc2` crates. Linux keeps ffmpeg. The loop that sets frame timing and the padding to an even size move into shared code in `encode/mod.rs`. There is no fallback to ffmpeg: a failure is `encode_failed`. Tests read the MP4 file's boxes directly.

How the existing system works, per this document: recdr stores a PNG frame only when the screen changes, and records timing and clicks in a manifest. At encode time it expands the frames to a constant rate and draws click rings. Today the MP4 is written by piping frames to ffmpeg, which pads odd sizes. A check before recording returns `ffmpeg_not_found`. GIF encoding is separate.

## Findings, round 1

### 1. Two criteria could not work with the synthetic source (Blocker)

- Lens: comprehension
- Where: success criteria 5 and 9, "Testing strategy"
- Problem: the pixel check was to catch reversed rows and swapped red and blue on a recording of the synthetic source. That source is 320 by 240, a white square on black at rows 100 to 139, with no clicks: it looks the same upside down and with red and blue swapped, it has no odd size, and it reports no click. `src/capture/fake.rs` was not on the file list.
- Why: the two mistakes the measurement found possible would pass every test.
- Decision: Fix
- Evidence or edit: the encoder tests and the pixel check run inside the crate on a test recording built with the existing helper `one_frame` in `src/encode/mod.rs`, with a picture the test chooses: red top-left, blue bottom-right, at 320 by 240 and 321 by 241, with one click. Criteria 5 and 9 name the pixels. `src/capture/fake.rs` does not change.

### 2. The check before recording on Windows and macOS (Major)

- Lens: comprehension and zoom-out (both raised it)
- Where: "Errors", `src/control.rs` in the file list
- Problem: the spec said `control` "asks encode whether MP4 can be written here" and did not say what that does where ffmpeg is not used. Read with the rest, the check was gone on Windows and macOS, so a machine with no H.264 encoder (a Windows N edition, some hosted runners) would fail only after recording, and nothing can encode the recording again until `recdr export` exists.
- Why: a recording made and then lost is what the existing check prevents.
- Decision: Fix
- Evidence or edit: on Windows and macOS the check starts the system encoder on a small temporary file and discards it. A failure is `encode_failed` before anything is recorded. Criterion 11, a Risks row about the added delay, and an item under "Not checked" follow from it.

### 3. Two statements about a partly written file contradicted each other (Major)

- Lens: comprehension
- Where: "Errors", Boundaries, success criterion 10
- Problem: "what stays on disk after a failed encode is what stays today" and "remove a partly written `recording.mp4`" cannot both hold: the ffmpeg path does not delete the file today.
- Why: the plan would have had to pick one.
- Decision: Fix
- Evidence or edit: the frames, the manifest and an earlier GIF stay; a partly written `recording.mp4` is removed, and the spec says that is new on every system.

### 4. Criterion 10 had no way to be set up (Major)

- Lens: comprehension
- Where: success criterion 10
- Problem: "given a path it cannot write to" is not reachable from the command line, because the MP4 path inside a recording directory is fixed.
- Why: an untestable criterion.
- Decision: Fix
- Evidence or edit: an encoder test puts a directory named `recording.mp4` where the file should go and expects `encode_failed`.

### 5. Two existing tests were not accounted for (Major)

- Lens: comprehension
- Where: "Testing strategy", "Not checked"
- Problem: `mp4_is_h264_of_the_recording_length` in `tests/record.rs` needs `ffprobe`, and `mp4_without_ffmpeg_is_refused_before_anything_is_written` runs on every system and would fail on Windows and macOS after this module. "Which hosted runners have `ffprobe`" was listed as unknown although the new tests were not to use it.
- Why: the build would break existing tests with no stated plan for them.
- Decision: Fix
- Evidence or edit: a subsection "Existing tests" says what happens to each. The "Not checked" item now asks which runners have `ffmpeg`, which the pixel check uses.

### 6. Building the Windows half early conflicted with what the spec assumes (Major)

- Lens: comprehension
- Where: Open question 1, "Project structure"
- Problem: the question said the Windows half needs only `agent-recorder`, while the file list and the layer rules assume the state after `cdp-capture`.
- Why: an early build would have had no three CI jobs and no `cfg` rule to extend.
- Decision: Fix
- Evidence or edit: the introduction says both halves need `cdp-capture` and what they take from it. The question now asks only whether the Windows half is built right after `cdp-capture`. The capability map's "Depends on" cell for this module reads "`agent-recorder`, `cdp-capture`".

### 7. The `objc2` layer rule could not be checked as written (Major)

- Lens: comprehension
- Where: layer rules
- Problem: "the `objc2` crates appear only in `encode/avf.rs`" fails against `tests/layers.rs`, which matches plain text, because `macos-capture` uses `objc2_core_graphics` in its own files.
- Why: the layer test would fail on a correct build.
- Decision: Fix
- Evidence or edit: the rule names `objc2_av_foundation`, `objc2_core_video` and `objc2_core_media`.

### 8. The frame loop was presented as new code (Major)

- Lens: zoom-out
- Where: "What the video is", "Project structure"
- Problem: `encode::timeline`, `encode::render` and the constant-rate loop in `encode::ffmpeg::encode` exist. The spec described the loop as something to write, under a name that collides with the existing type `encode::OutFrame`.
- Why: a second loop that could drift from the one ffmpeg uses.
- Decision: Fix
- Evidence or edit: the loop "moves out of the ffmpeg file so the three encoders share it; it is not written again". The term is now "video frame".

### 9. "Output frame" clashed with `OutFrame` (Nit)

- Lens: comprehension
- Where: "Words used here"
- Problem: the spec's term and the code's type mean different things.
- Why: a reader of the code would confuse them.
- Decision: Fix
- Evidence or edit: the same rename as finding 8, with a sentence on what `OutFrame` is.

### 10. "Changes only" introduced rules that are new (Nit)

- Lens: comprehension
- Where: layer rules
- Problem: the two rules that confine `windows_capture` and the `objc2` crates do not exist today.
- Why: the heading misdescribes them.
- Decision: Fix
- Evidence or edit: both are marked "New:".

### 11. "The three CI jobs" were not named (Nit)

- Lens: comprehension
- Where: Boundaries, success criteria
- Problem: the jobs come from `cdp-capture` and were not named here.
- Why: a reader new to the project cannot tell which jobs.
- Decision: Fix
- Evidence or edit: the introduction names the three runners.

### 12. "Stored frame", "manifest" and "click rings" were not defined or linked (Nit)

- Lens: comprehension
- Where: throughout
- Problem: three terms of the first spec were used without a definition.
- Why: the same as finding 11.
- Decision: Fix
- Evidence or edit: "Words used here" defines all three.

### 13. No reason for the constant frame rate (Nit)

- Lens: zoom-out
- Where: "What the video is"
- Problem: both system encoders take a time per frame, so stored frames could be handed over as they are. The spec kept a constant rate without saying why.
- Why: the next reader will ask the same question.
- Decision: Fix
- Evidence or edit: the bullet gives the reasons: it is what ffmpeg produces today, and it makes frame count and length the same on every system.

### 14. Padding and row reversal exist in a dependency (Nit)

- Lens: zoom-out
- Where: "Project structure"
- Problem: the `image` crate, already a dependency, has `imageops` functions for both.
- Why: hand-written pixel loops where none are needed.
- Decision: Fix
- Evidence or edit: the spec says both use `image::imageops`, and that swapping red and blue for Windows is the only new pixel code.

### 15. The "PATH with no ffmpeg" test already exists (Nit)

- Lens: zoom-out
- Where: "Testing strategy"
- Problem: the spec described a new test, while `mp4_without_ffmpeg_is_refused_before_anything_is_written` already builds that set-up. The reviewer also pointed at the helper `tool_runs` in the same file.
- Why: a duplicate test.
- Decision: Fix
- Evidence or edit: "Existing tests" says the existing test is split by system. The helper is not named in the spec; which helper a test calls is for the plan.

### 16. Linux did change, although the spec said it does not (Nit)

- Lens: zoom-out
- Where: "Changes to existing behavior"
- Problem: padding in recdr and removing a partial file both touch the ffmpeg path.
- Why: "exactly as today" was not true.
- Decision: Fix
- Evidence or edit: the two Linux changes are listed under "Changes to existing behavior".

### 17. MP4 box names and a line count were plan detail (Nit)

- Lens: zoom-out
- Where: "Testing strategy", success criterion 1
- Problem: the spec named `ftyp`, `mvhd` and `avc1` and said the reader is "about thirty lines".
- Why: detail the plan owns.
- Decision: Fix
- Evidence or edit: the spec says the tests read the codec, the size and the length from the file's own headers.

## Findings, round 2

The comprehension lens ran again on the fixed spec. Its new paraphrase matched the spec, including the test recording that fixed the Blocker. It raised no Blocker, 6 Major and 7 Nit. The protocol stops after this round, so these fixes were not reviewed again.

### 18. The trial of the system encoder was not described, and might never fail (Major)

- Lens: comprehension
- Where: "Errors"
- Problem: "starts it on a temporary file, discards it" did not say how far the trial goes or where the file is. The reviewer read `windows-capture` 2.0.1 and found that `VideoEncoder::new` does not read whether the system can transcode; a failure shows only later, on the transcode thread.
- Why: a check that builds the encoder and drops it could pass on a machine with no H.264 encoder.
- Decision: Fix
- Evidence or edit: the check writes a whole one-frame MP4 in the system's temporary directory and removes it. "Not checked" says it is unknown whether a missing encoder shows when that file is finished, and the plan's first task looks for a machine to try it on.

### 19. The MP4 reader had no stated form and an unreachable home (Major)

- Lens: comprehension
- Where: "Testing strategy", "Project structure"
- Problem: the reader was placed in `tests/common/mod.rs`, which the encoder tests inside the crate cannot use, and the spec did not say whether it is hand-written or a crate.
- Why: four criteria depend on it.
- Decision: Fix
- Evidence or edit: the reader is hand-written, with no new crate, in `tests/common/mp4.rs`. The integration tests reach it through `tests/common/mod.rs` and the encoder tests include the same file with `#[path]`.

### 20. The pixel check had no home on Windows and macOS (Major)

- Lens: comprehension
- Where: "Testing strategy", success criterion 8
- Problem: the check runs `ffmpeg` on every system, while `encode/ffmpeg.rs` is Linux-only and the word is forbidden in `encode/mf.rs` and `encode/avf.rs`.
- Why: no file was left for the check.
- Decision: Fix
- Evidence or edit: the check is in the test module of `encode/mod.rs` and runs `ffmpeg` as a command itself.

### 21. Criterion 9 left its numbers open (Major)

- Lens: comprehension
- Where: success criterion 9
- Problem: the click's position and time, the frame to decode, the ring's radius and color and a tolerance for lossy H.264 were all missing. A ring shows for 400 ms only.
- Why: the test could not be written without inventing them.
- Decision: Fix
- Evidence or edit: the test recording is 1 second at 10 frames per second with one click at (240, 60) at 0 ms. The first video frame is decoded. The ring's values are from `src/draw.rs` (`ring`) and `RING_MS` in `src/encode/mod.rs`. Criterion 9 names four pixels and limits for each.

### 22. Nothing was said about pixels on macOS (Major)

- Lens: comprehension
- Where: "Tech stack", "Project structure"
- Problem: "the only new pixel code is swapping red and blue for Windows" implied macOS takes RGBA as it is, with no support.
- Why: whether macOS needs a conversion is a code question the spec answered by accident.
- Decision: Fix
- Evidence or edit: the design for macOS is stated (32BGRA buffers, rows top to bottom, frame `k` at `k / fps` seconds) under "Not checked", as a design that was not run. Both system encoders get the red and blue swap; only Windows gets reversed rows.

### 23. The layer rules depended on a build order the spec leaves open (Major)

- Lens: comprehension
- Where: layer rules, Open question 1
- Problem: the baseline was "the rules in the `cdp-capture` spec", while the capability map builds `macos-capture` first, and that spec confines `objc2_core_foundation` to `capture/tap.rs`. `encode/avf.rs` may need that crate.
- Why: the rule list was incomplete under one of the two orders.
- Decision: Fix
- Evidence or edit: the baseline is the rules `tests/layers.rs` holds when the module is built. `objc2-core-foundation` is in the Tech stack table, and whichever module is built second widens its rule to both files.

### 24. A layer rule named in a criterion was missing from the rule list (Nit)

- Lens: comprehension
- Where: success criterion 8, layer rules
- Problem: "`ffmpeg` does not appear in `encode/mf.rs` or `encode/avf.rs`" was called a layer test and was not among the rule changes.
- Why: two lists of the same rules disagreed.
- Decision: Fix
- Evidence or edit: the rule is on the list, marked "New:".

### 25. The clean-up rule could delete the test's directory (Nit)

- Lens: comprehension
- Where: "Errors", success criterion 10
- Problem: "a partly written `recording.mp4` is removed" did not say it removes only a file the encoder made.
- Why: criterion 10 puts a directory of that name in the way.
- Decision: Fix
- Evidence or edit: an encoder that could not create the file removes nothing, and criterion 10 checks the directory is still there.

### 26. The scope of the "N editions" ending was unclear (Nit)

- Lens: comprehension
- Where: "Errors"
- Problem: the ending could apply to the check only or to every Windows `encode_failed`.
- Why: two readings.
- Decision: Fix
- Evidence or edit: only the message of a failed trial has it.

### 27. The check function had no name (Nit)

- Lens: comprehension
- Where: "Project structure"
- Problem: `encode::mp4` was named and the check was not.
- Why: the file list referred to it by description.
- Decision: Fix
- Evidence or edit: it is `encode::check_mp4`.

### 28. "The first spec" and "the owner" were not defined (Nit)

- Lens: comprehension
- Where: "Words used here", Objective
- Problem: neither term was tied to anything.
- Why: a reader new to the project has to guess.
- Decision: Fix
- Evidence or edit: both are the first entry under "Words used here".

### 29. How to choose the synthetic source was not stated (Nit)

- Lens: comprehension
- Where: success criterion 1
- Problem: the criterion said "on the synthetic source" without the set-up.
- Why: the reader has to open the first spec.
- Decision: Fix
- Evidence or edit: "Testing strategy" gives `RECDR_FAKE_SOURCE=moving` with `--monitor 0`, as `tests/record.rs` uses today.

### 30. The time sent with video frame `k` on Windows was not stated (Nit)

- Lens: comprehension
- Where: fact 5
- Problem: the unit was given and the value was not.
- Why: a small gap between the fact and its use.
- Decision: Fix
- Evidence or edit: fact 5 gives `k × 10,000,000 / fps`.

## Counts

30 findings over two rounds: 1 Blocker, 13 Major, 16 Nit. Decisions: 30 Fix, 0 Contest, 0 Escalate, 0 Decline.
