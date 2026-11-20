# Goldfish review: spec/4-macos-capture/spec.md

| Reviewer | Status |
|---|---|
| comprehension | completed |
| zoom-out | completed |

One round, run on Opus. No finding was a Blocker, so no second round was run. The first launch of both reviewers failed on a usage limit before they produced anything; the reports below are from the second launch.

The comprehension reviewer reported 6 Major and 6 Nit, the zoom-out reviewer 3 Major and 2 Nit. Both raised the click filter, which is one finding here, so the list has 16.

## Paraphrase (comprehension)

Written about the first draft.

What this proposes: `recdr windows`, `recdr monitors` and the five desktop target flags fail on macOS with `unsupported_platform`. This module makes them work through ScreenCaptureKit on macOS 14 and later, with the same JSON, files, error codes and MCP tools as on Windows. All positions are in pixels, converted from points with each display's scale factor. `capture/macos.rs` does listing, target resolution and the stream, and `capture/tap.rs` is a listen-only mouse event tap. A missing Screen Recording permission fails with a new code, `permission_denied`. A missing Input Monitoring permission only drops clicks, with a warning.

How the existing system works, per this document: after `cdp-capture`, recdr builds on all three systems; tab targets record everywhere and desktop targets only on Windows. Capture sits behind a `CaptureSource` trait with three implementations, each of which reports clicks as `CaptureEvent::Click`. The Windows source owns a mouse hook and a click filter. Everything after capture does not depend on the source. `tests/layers.rs` limits where `unsafe`, crates and `cfg` may appear.

## Findings

### 1. The test pages and the test window were not defined (Major)

- Lens: comprehension
- Where: "Testing strategy", success criterion 5
- Problem: "the animation page", "the still page" and "the red box" were named and not described, and the spec did not say how a Chrome started with `--app=<url>` gets a URL. Criterion 5 asked for the red box in a region, but the box sits 400 px down the page and the window's content is shorter than 600 points, so the region could miss it.
- Why: the desktop tests could not be written from the spec, and criterion 5 could fail on a correct build.
- Decision: Fix
- Evidence or edit: "Testing strategy" now has a table of the three pages (`/anim`, `/still`, `/green`) and says the page server and CDP client move from `tests/browser.rs` to `tests/common/mod.rs`. Criterion 5 uses `/green`, a region centered on the window, and a check on the center pixel that allows for display color profiles.

### 2. The click filter lived in Windows-only code (Major)

- Lens: comprehension and zoom-out (both raised it)
- Where: "Clicks", "Project structure"
- Problem: the spec said a press is filtered "by the same filter the Windows source uses". After `cdp-capture` that filter is in `capture/windows.rs`, which is not compiled on macOS, and the file list did not move it.
- Why: the plan would have had to copy the filter or break a layer rule.
- Decision: Fix
- Evidence or edit: the function moves to `capture/mod.rs`, gains a scale factor argument (1 on Windows) and is called by both desktop sources. `capture/windows.rs` is on the file list. Criterion 19 checks a Windows click by hand after the move.

### 3. Behavior on an older macOS could not be promised (Major)

- Lens: comprehension
- Where: "Tech stack"
- Problem: the spec said desktop targets fail with `unsupported_platform` before macOS 14. It did not say how recdr finds the version, or whether a recdr built with the `macos_14_0` feature starts on macOS 13 at all.
- Why: a promise that may be impossible to keep.
- Decision: Fix
- Evidence or edit: the promise is removed. "Not checked" gains the question, and the README names macOS 14 as the smallest version if such a build does not start on 13.

### 4. Which process asks for the permission was not stated (Major)

- Lens: comprehension
- Where: "The Screen Recording permission"
- Problem: for `recdr start` the stream is opened by the detached recorder, which runs in its own session. The spec did not say which process runs the check, or whether macOS counts the same responsible program for the recorder.
- Why: a recording could pass the check and then be refused, with no stated error.
- Decision: Fix
- Evidence or edit: the check runs in the command the person ran, for `record` and `start` alike. A refusal inside the detached recorder is `permission_denied` through the start-up error path of `cdp-capture`. The attribution question is on the "Not checked" list.

### 5. No way to close or minimize the test window (Major)

- Lens: comprehension
- Where: success criteria 8 and 9
- Problem: the criteria needed a closed and a minimized Chrome `--app` window, and the spec gave no way to do either from a test.
- Why: sending keys or clicks would need a third permission.
- Decision: Fix
- Evidence or edit: the test Chrome starts with `--remote-debugging-port=0`. The test minimizes the window with the CDP command `Browser.setWindowBounds` and closes it by ending the process.

### 6. How a resized window is fitted was not stated (Major)

- Lens: comprehension
- Where: "Recording"
- Problem: "scaled to fit the starting size and padded with black, as on Windows" did not say whether recdr or ScreenCaptureKit does it.
- Why: ScreenCaptureKit's own scaling may stretch, which would break the first spec's rule.
- Decision: Fix
- Evidence or edit: the stream has a fixed output size. ScreenCaptureKit's scaling is used if it pads with black; otherwise recdr fits the frames. The check is on the "Not checked" list, and criterion 15 is a by-hand test of the result.

### 7. The spec described target resolution that already exists (Major)

- Lens: zoom-out
- Where: "Positions and sizes", "Project structure"
- Problem: choosing the monitor for a region, matching a window by title or process and refusing a minimized window were written as new macOS code. `resolve` in `src/capture/mod.rs` does all three from the lists of windows and monitors.
- Why: a second implementation that could drift from the Windows one.
- Decision: Fix
- Evidence or edit: the macOS code supplies the two lists in pixels and `resolve` is used unchanged. The only macOS step is dividing by the scale factor when the stream opens. "Target resolution" and "monitor choice" are gone from the file list and the test table.

### 8. Monitors with different scale factors were documented and left broken (Major)

- Lens: zoom-out
- Where: "Positions and sizes", Risks
- Problem: the draft multiplied each monitor's position by its own scale factor, so the `bounds` of a Retina laptop and a 1x external display could overlap, and said so in the README. That pair is the common two-monitor Mac.
- Why: a region on the second monitor could resolve to the first.
- Decision: Fix
- Evidence or edit: a monitor's origin is its position in points times the largest scale factor among the monitors, and sizes use the monitor's own scale factor. Two `bounds` cannot overlap. Criterion 16 has a unit test with a 2x and a 1x monitor.

### 9. The window filter had no name where it was defined (Nit)

- Lens: comprehension
- Where: "Listing"
- Problem: later sections said "the window filter" and "Listing" never used the words.
- Why: a reader has to guess which rule is meant.
- Decision: Fix
- Evidence or edit: the rule is named under "Listing".

### 10. The layer rule and the Risks table disagreed, and the permission check had no file (Nit)

- Lens: comprehension
- Where: "Project structure", Risks
- Problem: the rule allowed the crates in two files while Risks said "named in one file only". The Screen Recording check was assigned to neither file.
- Why: two readings of the same rule.
- Decision: Fix
- Evidence or edit: the rule lists each crate with its files, the check is in `capture/macos.rs`, and the Risks row says `screencapturekit` is named in one file.

### 11. "The recorder's log" was unclear for `record` (Nit)

- Lens: comprehension
- Where: "Clicks"
- Problem: `record` has no detached recorder, so it has no log file.
- Why: the warning's destination was ambiguous.
- Decision: Fix
- Evidence or edit: the process that records writes the line to its stderr, which for a detached recorder is `recorder.log`.

### 12. "Shows its dialog once" was ambiguous (Nit)

- Lens: comprehension
- Where: "The Screen Recording permission", success criterion 11
- Problem: "once" could mean once per run or once ever, and criterion 11 had no starting state.
- Why: the by-hand check could not be repeated.
- Decision: Fix
- Evidence or edit: macOS shows the dialog the first time a program asks. Criterion 11 starts from `tccutil reset ScreenCapture`.

### 13. Criterion 2 named `Google Chrome` (Nit)

- Lens: comprehension
- Where: success criteria 2 and 4
- Problem: `RECDR_TEST_CHROME` may point at Chromium.
- Why: the test would fail on a correct build.
- Decision: Fix
- Evidence or edit: both criteria use the application name of the Chrome the test started.

### 14. "System picker" and "the three CI jobs" were not defined (Nit)

- Lens: comprehension
- Where: "What was checked", Boundaries
- Problem: both terms were used without a definition.
- Why: a reader new to the project cannot follow them.
- Decision: Fix
- Evidence or edit: fact 6 says what the system picker is, and Boundaries names the three runners.

### 15. A macOS-only message for `target_minimized` (Nit)

- Lens: zoom-out
- Where: "Recording"
- Problem: the error has one `#[error]` message in `src/error.rs`. A second message for macOS would need a `cfg` there, which the layer rules forbid.
- Why: a rule broken for wording.
- Decision: Fix
- Evidence or edit: the message stays as it is. The README says what `minimized` covers on macOS.

### 16. A criterion fixed the shape of the code (Nit)

- Lens: zoom-out
- Where: success criterion 16 of the first draft
- Problem: "the tap's event list, as a constant" described an implementation.
- Why: the criterion would fail on a correct build that stores the list another way.
- Decision: Fix
- Evidence or edit: criterion 17 now says the tap asks for the three mouse-button presses and for no other event type.

## Counts

16 findings: 0 Blocker, 8 Major, 8 Nit. Decisions: 16 Fix, 0 Contest, 0 Escalate, 0 Decline.
