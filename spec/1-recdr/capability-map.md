# Capability map: recdr

recdr is a screen recorder written in Rust that an AI agent can drive. It is modelled on [ScreenToGif](https://github.com/NickeManarin/ScreenToGif): record one window, a screen region or a monitor, and get a GIF or video. The difference is that every action is available without a GUI, through a CLI and an MCP server, and that the agent can inspect the recording as still images.

Tracking issue: https://github.com/dzikrisyairozi/recdr/issues/1

| Module id | Responsibility | Depends on |
|---|---|---|
| `agent-recorder` | Windows headless recorder: window, region and monitor capture; PNG frame store; GIF and ffmpeg encoding; start/stop, fixed-duration and wrap-a-command control; agent view (keyframes, contact sheet, sampled frames, diff report); CLI and MCP server | none |
| `cdp-capture` | Record a browser tab through the Chrome DevTools Protocol screencast, attaching to a browser that is already running | `agent-recorder` |
| `macos-capture` | ScreenCaptureKit backend for window, region and monitor capture on macOS | `agent-recorder` |
| `native-mp4` | H.264 MP4 without ffmpeg: Media Foundation on Windows, VideoToolbox on macOS | `agent-recorder`, `macos-capture` |
| `gui-overlay` | egui desktop app: transparent always-on-top frame placed over the target, toolbar, and a trim and crop screen shown after recording | `agent-recorder` |

Build order: `agent-recorder` → `cdp-capture` → `macos-capture` → `native-mp4` → `gui-overlay`

## Why this order

`agent-recorder` alone solves the problem in the issue: an agent on Windows can record what happens on screen and look at the result. `native-mp4` comes late because ffmpeg already produces MP4 once `agent-recorder` ships. `gui-overlay` comes last because it is a second front end over a recorder that must already work.

## Decisions that apply to every module

- Language: Rust. License: MIT, so no GPL or AGPL dependencies (this rules out the `gifski` GIF encoder, which is AGPL-3.0-or-later).
- GUI toolkit: egui. Tauri was rejected because it draws the UI in a WebView and the owner wants no web layer.
- "Headless" means recdr shows no window of its own. A desktop session with visible windows still exists. Recording on a machine with no display is only in scope for `cdp-capture`.
- Each module gets its own spec in this directory, named `spec-<module-id>.md`, written when its turn comes.

## Specs

- [spec-agent-recorder.md](spec-agent-recorder.md)
