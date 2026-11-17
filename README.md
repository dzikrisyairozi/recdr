# recdr

recdr records a window, a screen region or a monitor on Windows and leaves files an AI agent can read: a GIF or MP4 for people, and for the agent a contact sheet of the moments where the screen changed, a per-frame change log and the stored frames as PNG. Agents drive it through a CLI that prints JSON, or through an MCP server.

It exists because browser tools give an agent screenshots, not recordings. With recdr an agent can start a recording, run a test or click through a flow, stop, and look at what happened.

## Build

Requires Rust 1.98 or later and Windows 10 1903 or later (Windows 11 recommended).

```
cargo build --release
```

The binary is `target/release/recdr.exe`. MP4 output also needs `ffmpeg` on `PATH`.

## Commands

```
recdr windows                       List visible top-level windows
recdr monitors                      List monitors
recdr record <target> --duration 10s [options]     Record for a fixed time, then return
recdr record <target> [options] -- <command...>    Record while the command runs
recdr start  <target> [options]     Start a background recording, return its id at once
recdr stop   <id>                   Stop it, encode, return the result
recdr mark   <id> "<label>"         Add a labelled marker at the current time
recdr status [<id>]                 State of one recording, or of all in the output root
recdr frames <id> (--at 1200ms,2s | --every 500ms | --keyframes) [--max-width 1024]
recdr mcp                           Run the MCP server on stdio
```

Every command prints one JSON object on stdout and exits 0. A failure prints `{"error":{"code":"...","message":"..."}}` on stderr and exits 1, or 2 for a usage error.

`<target>` is exactly one of:

| Flag | Meaning |
|---|---|
| `--window-id <id>` | The `id` from `recdr windows`. Exact; prefer this. |
| `--window "<text>"` | The visible window whose title contains the text, ignoring case. |
| `--process <name>` | The visible window of a process with this executable name, such as `chrome.exe`. |
| `--region x,y,w,h` | A rectangle in physical pixels that lies inside one monitor. |
| `--monitor <index>` | A whole monitor, by the `index` from `recdr monitors`, starting at 0. |

A window target is resolved once, when recording starts. When `--window` or `--process` matches more than one window, recdr fails with `ambiguous_target` and lists the candidates instead of guessing. To record a window that the wrapped command will open, use `--monitor` or `--region`.

Options for `record` and `start`:

| Option | Default | Meaning |
|---|---|---|
| `--duration <time>` | none | `record` only. Stop after this long. |
| `--fps <n>` | 15 | Capture rate limit, 1 to 60. |
| `--max-duration <time>` | 5m | The recording stops itself at this length. |
| `--max-size <size>` | 2GB | The recording stops itself when its directory reaches this size. |
| `--format <list>` | gif | `gif`, `mp4`, or `gif,mp4`. |
| `--no-cursor` | off | Leave the mouse cursor out of the frames. |
| `--no-click-highlight` | off | Do not record clicks. No mouse hook is installed. |

`--out <dir>` (default `./.recdr`) sets the output root for every command that reads or writes recordings. A `<time>` is a whole number with `ms`, `s` or `m`; a `<size>` is a whole number with `MB` or `GB`.

### Examples

Record a window for five seconds:

```
recdr windows
recdr record --window-id 70628 --duration 5s
```

Record a monitor while a test suite runs:

```
recdr record --monitor 0 -- npm test
```

Start in the background, mark a moment, stop:

```
recdr start --window-id 70628
recdr mark 20261118-143502-1a2b "clicked submit"
recdr stop 20261118-143502-1a2b
```

## What a recording leaves

A recording lives in `<out>/<id>/`:

| File | Contents |
|---|---|
| `frames/000001.png`, ... | Each frame that differs from the one before it. |
| `manifest.json` | Size, fps, duration, each frame's time, marks, clicks, stop reason, dropped frames. |
| `diff.json` | For each frame: the share of pixels that changed, the box around them, and whether it is a keyframe. |
| `contact_sheet.png` | Up to 12 keyframes in a grid, each labelled with its time and any marks. |
| `recording.gif`, `recording.mp4` | The formats asked for, with a ring drawn for 400 ms after each click. |
| `result.json` | What `stop` and `record` print. |

`recdr` writes `<out>/.gitignore` containing `*`, so recordings are not committed by accident.

## MCP

Register the server with Claude Code:

```
claude mcp add recdr -- C:\path\to\recdr.exe mcp
```

Tools: `list_windows`, `list_monitors`, `start_recording`, `stop_recording`, `record`, `mark`, `get_status` and `get_frames`. Each returns the same JSON the CLI prints. `stop_recording` and `record` also return the contact sheet as an image, and `get_frames` returns up to 8 frames as images.

## Limits

- On Windows 10 the system draws a yellow border around the captured window or monitor, and it cannot be turned off there. Windows 11 turns it off.
- Clicks in windows of elevated (administrator) processes are not seen by the mouse hook, so they get no ring. Security software may flag a program that hooks the mouse and captures the screen; `--no-click-highlight` installs no hook.
- Protected video and some full-screen exclusive applications capture as black.
- When a recorded window is resized, the first frame at the new size is discarded by the capture library. A window that then never repaints keeps showing its old image in the recording. Frames of another size are scaled to fit the first frame's size, on black.
- A recording holds whatever was on screen, including secrets. recdr never uploads anything; recordings stay in the output root.
- Keyboard input is never recorded.

## License

MIT
