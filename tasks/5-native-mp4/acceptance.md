# Acceptance: 5-native-mp4

Results for the checkpoints and success criteria of `spec/5-native-mp4/spec.md`, filled in as the build reaches them. Runs are on Windows 11 with ffmpeg 7.0.2 on PATH unless a line says otherwise.

## Checkpoint A

- The three commands pass after Task 4 (`evidence/T4-build.log`, `evidence/T4-test.log`).
- The MP4 of `recdr record --monitor 0 --duration 3s --format mp4` (release build): exit 0, `duration_ms` 3050. `ffprobe -count_frames` reads `h264`, 1920 by 1080, 46 frames, 3.067 s, and `ffmpeg -i <mp4> -f null -` decodes the whole file with exit 0. Decoded by ffmpeg, not watched in a player; the owner confirms by playing it.
- Kept for plan decision 13: `recdr record --monitor 0 --duration 10s --format mp4 --out <scratchpad>\mp4-kept`, recording id `20261118-160103-1aa7`, `duration_ms` 10012, 136 frames stored, 0 dropped. Motion came from an `ffplay` window showing `testsrc2` at 1280 by 720 and 30 fps, not from a window moved by hand (the build sends no mouse or keyboard input). ffmpeg's MP4 was copied to `<scratchpad>\mp4-kept\ffmpeg-kept.mp4`: 2,097,805 bytes. `<scratchpad>` is the build session's scratchpad directory on the development machine; the recording shows the owner's desktop and is not committed.
