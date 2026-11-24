<div align="center">

<img src="https://raw.githubusercontent.com/dzikrisyairozi/recdr/main/assets/logo.svg" width="100" alt="recdr">

<h1>recdr</h1>

<a href="#install">安装</a> · <a href="#quick-start">快速开始</a> · <a href="#commands">文档</a>

<a href="README.md">English</a> · 简体中文

[![license](https://img.shields.io/badge/license-MIT-blue)](LICENSE)
[![downloads](https://img.shields.io/github/downloads/dzikrisyairozi/recdr/total)](https://github.com/dzikrisyairozi/recdr/releases)
[![stars](https://img.shields.io/github/stars/dzikrisyairozi/recdr)](https://github.com/dzikrisyairozi/recdr/stargazers)
[![release](https://img.shields.io/github/v/release/dzikrisyairozi/recdr)](https://github.com/dzikrisyairozi/recdr/releases/latest)
[![homebrew](https://img.shields.io/github/v/release/dzikrisyairozi/recdr?label=homebrew)](https://github.com/dzikrisyairozi/homebrew-tap)
[![crates.io](https://img.shields.io/crates/v/recdr)](https://crates.io/crates/recdr)

</div>

https://github.com/user-attachments/assets/b903fcf5-294d-473d-a2af-de5e2ef5af6f

recdr 可以录制一个窗口、一块区域、一台显示器或一个 Chrome 标签页。你得到 GIF 或 MP4。**你的 agent 得到画面发生变化的那些帧**，外加一张缩略图总览（contact sheet）和一份变化记录，通过 CLI 或 MCP 获取。

窗口、区域和显示器支持 Windows 和 macOS；浏览器标签页支持 Windows、macOS 和 Linux。

做这个工具的原因是：浏览器工具只能给 agent 截图，给不了录像。有了 recdr，agent 可以开始录制，运行测试或点击走完一个流程，停止录制，然后查看发生了什么。

<a id="install"></a>
## 安装

macOS 和 Linux：

```
curl -fsSL https://recdr.dev/install.sh | sh
```

Windows（PowerShell）：

```
irm https://recdr.dev/install.ps1 | iex
```

脚本会从最新的 GitHub release 下载适合你系统的构建，校验其 SHA-256，然后把 `recdr` 放到 `~/.local/bin`；在 Windows 上则把 `recdr.exe` 放到 `%LOCALAPPDATA%\Programs\recdr`，并把该目录加入你的用户 `Path`。脚本最后会打印注册 MCP 服务器的命令。

在 macOS 或 Linux 上用 Homebrew 安装，或者用 cargo 安装：

```
brew install dzikrisyairozi/tap/recdr
cargo install recdr
```

`recdr update` 使用同一个脚本，把最新版本安装到当前运行的二进制文件所在位置，覆盖旧版本。`recdr update --check` 只报告是否有新版本。由 Homebrew 或 cargo 安装的副本会被拒绝并返回 `managed_install`；请用 `brew upgrade recdr` 或 `cargo install recdr` 更新。

两个脚本都会读取三个环境变量：

| 变量 | 默认值 | 含义 |
|---|---|---|
| `RECDR_VERSION` | 最新版本 | 安装指定版本，写成 `0.1.0` 或 `v0.1.0`。 |
| `RECDR_INSTALL_DIR` | 见上文 | 安装到这个文件夹。在 Windows 上，这里设置的文件夹不会被加入 `Path`。 |
| `RECDR_RELEASE_BASE` | `https://github.com/dzikrisyairozi/recdr/releases` | 从另一个目录结构相同的地址读取 release。`recdr update` 也会读取它。 |

```
curl -fsSL https://recdr.dev/install.sh | RECDR_VERSION=0.1.0 sh
$env:RECDR_VERSION = '0.1.0'; irm https://recdr.dev/install.ps1 | iex
```

卸载时删除二进制文件即可。在 Windows 上还要把 `%LOCALAPPDATA%\Programs\recdr` 从用户 `Path` 中移除。

Windows 二进制文件没有签名，因此 Defender 或 SmartScreen 可能会发出警告。recdr 在录制期间会安装一个低级鼠标钩子，用来绘制点击圆环；使用 `--no-click-highlight` 则不安装任何钩子。

<a id="quick-start"></a>
## 快速开始

在 Claude Code 中注册 MCP 服务器：

```
claude mcp add recdr -- recdr mcp
```

然后让 agent 录点东西，例如：“录制打开结账页面的那个 Chrome 标签页，同时点击购买，然后给我看缩略图总览”。在命令行中做同样的录制，时长 10 秒：

```
recdr record --cdp chrome --tab checkout --duration 10s
```

`--cdp chrome` 需要先在 `chrome://inspect/#remote-debugging` 打开远程调试；参见[录制浏览器标签页](#record-a-browser-tab)。

<a id="build"></a>
## 构建

供贡献者使用，或者用于从源码检出目录运行 recdr。

需要 Rust 1.98 或更高版本。桌面目标（窗口、区域、显示器）需要 Windows 10 1903 或更高版本（推荐 Windows 11），或 macOS 14 或更高版本。在 macOS 上构建需要 Xcode 命令行工具（`xcode-select --install`），不需要完整的 Xcode。标签页目标可在 Windows、Linux 和 macOS 上运行，需要一个打开了远程调试的 Chromium 内核浏览器。

```
cargo build --release
```

二进制文件是 `target/release/recdr`（在 Windows 上是 `recdr.exe`）。

MP4 输出在 Windows（Media Foundation）和 macOS（AVFoundation）上使用系统自带的 H.264 编码器，在 Linux 上使用 `PATH` 中的 `ffmpeg`。Windows "N" 版本在安装 Media Feature Pack 之前没有 Media Foundation 编码器。在 Windows 上，每个 MP4 至少为 160 x 120（更小的录制会补上黑色的列和行），并带有一条静音 AAC 音轨，这两点都来自 `windows-capture` 编码器。

在 Linux 上，`windows`、`monitors` 和桌面目标会以 `unsupported_platform` 失败。在 Linux 上，`--` 之后的命令不经过 shell 运行，需要 shell 时请写成 `-- sh -c "npm test && echo done"`。

<a id="macos-permissions"></a>
### macOS 权限

macOS 会请求两项权限，并把权限授予启动 recdr 的程序：也就是你的终端（Terminal、iTerm2）或编辑器，而不是 recdr 本身。用 `start` 启动的录制，以及由 MCP 服务器启动的录制，都算作同一个程序。

- **屏幕录制**（系统设置 > 隐私与安全性 > 屏幕与系统录音），用于 `windows`、`monitors` 和所有桌面目标。没有这项权限时，recdr 会请 macOS 弹出授权对话框，并以 `permission_denied` 失败。在设置中打开该程序的开关，然后重启该程序。
- **输入监控**（系统设置 > 隐私与安全性 > 输入监控），用于点击圆环。没有这项权限时录制仍会进行，只是不记录点击，并在 stderr 上打印 `clicks are not recorded: ...`（对于 `start`，打印在 `recorder.log` 中）。使用 `--no-click-highlight` 则不请求任何权限。

录制进行时，macOS 会在菜单栏显示屏幕录制指示图标。在 macOS 15 及更高版本上，系统还可能定期弹出提示，说明该程序正在不经系统窗口选择器读取屏幕。

在 macOS 上，`recdr windows` 中的 `minimized` 对任何不在屏幕上的窗口都为 true：已最小化、随应用一起隐藏，或位于另一个空间（Space）。这样的窗口会被拒绝并返回 `target_minimized`。坐标以像素为单位：每台显示器 `bounds` 的原点是它以点（point）为单位的位置乘以最大缩放系数，因此一台 Retina 笔记本外接一台 1x 显示器时，各显示器的 `bounds` 之间会有空隙；而从帧上读到的位置加上 `bounds` 原点，仍然是有效的 `--region` 位置。

<a id="commands"></a>
## 命令

```
recdr windows                       List visible top-level windows
recdr monitors                      List monitors
recdr tabs --cdp <endpoint>         List the tabs of a browser
recdr record <target> --duration 10s [options]     Record for a fixed time, then return
recdr record <target> [options] -- <command...>    Record while the command runs
recdr start  <target> [options]     Start a background recording, return its id at once
recdr stop   <id>                   Stop it, encode, return the result
recdr mark   <id> "<label>"         Add a labelled marker at the current time
recdr status [<id>]                 State of one recording, or of all in the output root
recdr frames <id> (--at 1200ms,2s | --every 500ms | --keyframes) [--max-width 1024]
recdr export <id> --file <out.gif|out.mp4> [--from 2s] [--until 5s] [--crop x,y,w,h]
recdr gui                           Open the desktop app (Windows and macOS)
recdr mcp                           Run the MCP server on stdio
recdr update [--check]              Install the newest release over this binary
```

每个命令都在 stdout 上打印一个 JSON 对象并以 0 退出。失败时在 stderr 上打印 `{"error":{"code":"...","message":"..."}}` 并以 1 退出，用法错误则以 2 退出。

`<target>` 必须是以下之一，且只能是一个：

| 参数 | 含义 |
|---|---|
| `--window-id <id>` | `recdr windows` 给出的 `id`。精确匹配；优先使用。 |
| `--window "<text>"` | 标题包含该文本的可见窗口，不区分大小写。 |
| `--process <name>` | 可执行文件名为该名称的进程的可见窗口，例如 `chrome.exe`。 |
| `--region x,y,w,h` | 位于同一台显示器内、以物理像素表示的矩形。 |
| `--monitor <index>` | 整台显示器，用 `recdr monitors` 给出的 `index` 指定，从 0 开始。 |
| `--tab-id <id>` with `--cdp <endpoint>` | `recdr tabs` 给出的 `id`。精确匹配；优先使用。 |
| `--tab "<text>"` with `--cdp <endpoint>` | 标题或 URL 包含该文本的标签页，不区分大小写。 |

窗口目标只在录制开始时解析一次。标签页在录制器连接到浏览器时查找。当 `--window`、`--process` 或 `--tab` 匹配到多个窗口或标签页时，recdr 不会猜测，而是以 `ambiguous_target` 失败并列出候选项。如果要录制由被包装命令打开的窗口，请使用 `--monitor` 或 `--region`。

`record` 和 `start` 的选项：

| 选项 | 默认值 | 含义 |
|---|---|---|
| `--duration <time>` | 无 | 仅用于 `record`。录制这么长时间后停止。 |
| `--fps <n>` | 15 | 采集帧率上限，1 到 60。 |
| `--max-duration <time>` | 5m | 录制达到这个时长时自动停止。 |
| `--max-size <size>` | 2GB | 录制目录达到这个大小时自动停止。 |
| `--format <list>` | gif | `gif`、`mp4`、`gif,mp4`，或 `none`：只保留帧和分析结果，之后再导出。 |
| `--no-cursor` | 关 | 帧中不包含鼠标指针。标签页录制从不显示指针。 |
| `--no-click-highlight` | 关 | 不记录点击。不安装鼠标钩子，也不向页面添加任何内容。 |

`--out <dir>`（默认 `./.recdr`）为所有读写录制的命令设置输出根目录。`<time>` 是一个带 `ms`、`s` 或 `m` 的整数；`<size>` 是一个带 `MB` 或 `GB` 的整数。

<a id="examples"></a>
### 示例

录制一个窗口五秒：

```
recdr windows
recdr record --window-id 70628 --duration 5s
```

在测试套件运行期间录制一台显示器：

```
recdr record --monitor 0 -- npm test
```

在后台开始录制，标记一个时刻，然后停止：

```
recdr start --window-id 70628
recdr mark 20261118-143502-1a2b "clicked submit"
recdr stop 20261118-143502-1a2b
```

<a id="export-part-of-a-recording"></a>
## 导出录制的一部分

`recdr export` 把一段已完成的录制裁剪时间、裁切画面后写成 GIF 或 MP4，不改动录制本身的文件：

```
recdr export 20261118-143502-1a2b --file cut.gif --from 2s --until 5s --crop 0,0,640,360
```

| 参数 | 含义 |
|---|---|
| `--file <path>` | 必填。`.gif` 或 `.mp4`；由扩展名决定格式。已存在的文件只有在导出成功时才会被替换。 |
| `--from <time>` | 保留部分的起点。默认：开头。 |
| `--until <time>` | 保留部分的终点。默认：结尾。 |
| `--crop x,y,w,h` | 以录制像素表示的矩形，至少 2 x 2，且在画面之内。 |

它会打印 `{"path", "width", "height", "duration_ms", "frames"}`；对于 MP4，尺寸是文件中画面的尺寸，即经过“构建”一节所述填充之后的尺寸。如果录制保留了点击，会绘制点击圆环。仍在进行中的录制会返回 `recording_not_found`；时间不合法、裁切区域超出画面或扩展名不是这两种时返回 `usage`；文件无法写入时返回 `encode_failed`。

<a id="the-app"></a>
## 桌面应用

`recdr gui` 会在其他窗口之上打开一个取景框和一条工具栏，支持 Windows 和 macOS。拖动工具栏可以移动取景框，拖动边缘和角可以调整大小，也可以在目标菜单中选择一个窗口，让取景框套住它。录制和停止按钮录制的是取景框内的内容；录制时取景框变成红色。停止后会打开一个编辑器：它可以播放录制，用时间轴上的两个手柄裁剪时间，在画面上拖出矩形来裁切画面，并通过 `recdr export` 保存为 GIF 或 MP4。“新录制”会回到取景框。

应用以 `--format none` 把录制写入输出根目录，默认是 `./.recdr`，因此应用的快捷方式应当传入 `--out`。它生成的每一段录制都是普通录制：`recdr status`、`frames` 和 `export` 都可以用于它，在终端中执行 `recdr stop <id>` 会结束录制并打开编辑器。

<a id="record-a-browser-tab"></a>
## 录制浏览器标签页

recdr 通过 Chrome DevTools Protocol 连接到一个正在运行的 Chromium 内核浏览器，按浏览器绘制的样子录制一个标签页的页面，无论该标签页是否被选中、被遮挡，或位于已最小化的窗口中。屏幕上的任何内容都不会改变。

先列出标签页，再录制其中一个：

```
recdr tabs --cdp 9222
recdr record --cdp 9222 --tab-id 6A1B5C... --duration 5s
recdr record --cdp 9222 --tab "checkout" --duration 5s
```

`--cdp` 接受四种形式：

| 形式 | 示例 | 适用场景 |
|---|---|---|
| 端口 | `9222` | 以 `--remote-debugging-port=9222 --user-data-dir=<dir>` 启动的浏览器，例如 Playwright MCP 用 `--cdp-endpoint` 连接的浏览器，或者脚本、CI 任务中的无头 Chrome。 |
| `http://` URL | `http://127.0.0.1:9222` | 同上，但在另一个地址上。recdr 会向 `<url>/json/version` 请求 WebSocket URL。 |
| `ws://` URL | `ws://127.0.0.1:9222/devtools/browser/4f0c...` | 原样使用。agent-browser 用 `get cdp-url` 可以打印出这样的 URL。 |
| `chrome` | `chrome` | 你日常使用的 Chrome 144 或更高版本，前提是已在 `chrome://inspect/#remote-debugging` 打开远程调试。Chrome 会要求你允许每一次连接，因此 `tabs` 需要点一次，`record` 或 `start` 还要再点一次。 |

`https://` 和 `wss://` 端点会被拒绝。

对于 Chrome Beta、Canary、Edge 或位于其他目录的配置文件，请根据该浏览器用户数据目录中的 `DevToolsActivePort` 文件拼出 `ws://` URL。浏览器在 `chrome://inspect/#remote-debugging` 打开远程调试时，或以 `--remote-debugging-port=0` 启动时，会写入这个文件（测试中，指定了固定端口的无头 Chrome 没有写入它）。文件第一行是端口，第二行是路径：

```
$ cat "<user data dir>/DevToolsActivePort"
53122
/devtools/browser/0e2d5f44-...
$ recdr tabs --cdp ws://127.0.0.1:53122/devtools/browser/0e2d5f44-...
```

从 Chrome 136 起，默认配置文件会忽略 `--remote-debugging-port`，因此用这种方式启动的浏览器需要自己的 `--user-data-dir`。

标签页中的点击来自页面本身：recdr 在顶层文档上添加一个监听鼠标按下的监听器，因此自动化工具通过 CDP 发送的点击也会有圆环。manifest 以帧像素保存这些点击。

<a id="limits-of-tab-recording"></a>
### 标签页录制的限制

- 录制期间，页面认为自己处于可见且获得焦点的状态，因此隐藏时会暂停的页面会继续运行。
- 页面上会添加 `window.__recdrClick` 和 `window.__recdrListening`，因此页面可以知道自己正在被录制。`--no-click-highlight` 不添加任何内容。
- iframe 内的点击不会被报告。视口尺寸改变后的点击可能画在错误的位置。
- 在录制即将停止前的按下，或者在页面导航与监听器重新添加之间那一瞬间的按下，可能会漏掉。
- 一帧最多会在浏览器发送它之后一个帧间隔（15 fps 时约 67 ms）才被保存。
- 如果录制器被强行结束，它的监听器会留在打开的文档中；请重新加载该标签页。

<a id="stop-reasons"></a>
## 停止原因

结果中的 `stop_reason` 是以下之一：`stopped`（`recdr stop`）、`duration_reached`（`--duration`）、`cap_reached`（`--max-duration`）、`size_cap_reached`（`--max-size`）、`command_exited`（被包装的命令已结束；`command_exit_code` 保存其退出码）、`window_closed`（被录制的窗口已关闭）和 `tab_closed`（被录制的标签页已关闭或浏览器已断开）。

<a id="error-codes"></a>
## 错误码

| 错误码 | 含义 |
|---|---|
| `usage` | 参数或选项有误。退出码为 2。 |
| `target_not_found` | 没有匹配的窗口、显示器或标签页。 |
| `ambiguous_target` | 匹配到多个；`candidates` 列出了它们。 |
| `target_minimized` | 窗口已最小化，没有可采集的内容。 |
| `region_out_of_bounds` | `--region` 不在同一台显示器内。 |
| `ffmpeg_not_found` | 在 Linux 上使用 `--format mp4`，但 `PATH` 中没有 `ffmpeg`。 |
| `recorder_start_failed` | 后台录制器退出且未说明原因，或没有及时报告第一帧。 |
| `recording_not_found` | 输出根目录下没有该 id 的录制。 |
| `recording_not_active` | 对已结束的录制执行 `stop` 或 `mark`。 |
| `recorder_crashed` | 后台录制器在完成前退出。 |
| `too_many_frames` | `get_frames` 将返回超过 8 帧。 |
| `capture_failed` | 采集源失败。 |
| `encode_failed` | 写入 GIF 或 MP4 失败，或者在录制前发现系统没有可用的 H.264 编码器。 |
| `unsupported_platform` | 在 Linux 上使用 `windows`、`monitors` 或桌面目标。 |
| `permission_denied` | macOS 没有授予启动 recdr 的程序屏幕录制权限。 |
| `cdp_connect_failed` | 无法连接到浏览器，浏览器不支持 CDP，或 30 秒内没有响应。 |
| `managed_install` | 对 Homebrew 或 cargo 安装的副本执行 `recdr update`；消息中会给出应当运行的命令。 |
| `update_failed` | `recdr update` 无法读取最新版本、无法下载安装脚本，或者脚本执行失败；消息中会说明是哪一种。 |
| `io_error` | 文件或进程操作失败。 |

<a id="tests"></a>
## 测试

```
cargo test
cargo test --test desktop -- --ignored --test-threads=1      # Windows and macOS; records the real screen
cargo test --test browser -- --ignored --test-threads=1      # starts headless Chrome
```

浏览器测试会在默认安装路径查找 Chrome；设置 `RECDR_TEST_CHROME` 可以使用其他二进制文件。

<a id="what-a-recording-leaves"></a>
## 录制留下的文件

一段录制位于 `<out>/<id>/`：

| 文件 | 内容 |
|---|---|
| `frames/000001.png`, ... | 与上一帧不同的每一帧。 |
| `manifest.json` | 尺寸、fps、时长、每一帧的时间、标记、点击、停止原因、丢弃的帧。 |
| `diff.json` | 每一帧的变化像素占比、包围这些像素的框，以及该帧是否为关键帧。 |
| `contact_sheet.png` | 最多 12 个关键帧排成的网格，每个都标注了时间和标记。每条边最多 2000 px。 |
| `recording.gif`, `recording.mp4` | 所请求的格式，每次点击后会绘制一个持续 400 ms 的圆环。 |
| `result.json` | `stop` 和 `record` 打印的内容。 |

`recdr` 会写入内容为 `*` 的 `<out>/.gitignore`，避免录制被意外提交。

<a id="mcp"></a>
## MCP

在 Claude Code 中注册服务器：

```
claude mcp add recdr -- /path/to/recdr mcp
```

工具：`list_windows`、`list_monitors`、`list_tabs`、`start_recording`、`stop_recording`、`record`、`mark`、`get_status`、`get_frames` 和 `export`。每个工具返回的 JSON 与 CLI 打印的相同。`stop_recording` 和 `record` 还会以图片形式返回缩略图总览，`get_frames` 以图片形式返回最多 8 帧。每张图片每条边最多 2000 px；`get_frames` 会把帧缩小到 `max_width`（默认 1024），如果仍有一条边超过 2000 px，会再缩小。

<a id="limits"></a>
## 限制

- 在 Windows 10 上，系统会在被采集的窗口或显示器周围画一圈黄色边框，并且在该系统上无法关闭。Windows 11 会关闭它。
- 鼠标钩子看不到提权（管理员）进程窗口中的点击，因此这些点击没有圆环。安全软件可能会标记同时挂钩鼠标并采集屏幕的程序；`--no-click-highlight` 不安装钩子。
- 受保护的视频和部分全屏独占应用会被采集为黑屏。
- 在 macOS 上，窗口在最小化期间或位于另一个空间时，那段时间不会保存任何帧；录制会继续进行。调整过大小的窗口会由 macOS 自身缩放以适应起始尺寸，并用黑色填充。
- 被录制的窗口调整大小后，采集库会丢弃新尺寸下的第一帧。如果窗口之后不再重绘，录制中会一直显示它的旧画面。其他尺寸的帧会缩放到第一帧的尺寸，放在黑色背景上。
- 录制会包含屏幕上的所有内容，包括机密信息。recdr 从不上传任何内容；录制只保存在输出根目录中。
- 从不记录键盘输入。
- 尚未在未安装 Media Feature Pack 的 Windows "N" 版本上试过 MP4 路径。recdr 在录制前会检查是否有可用的编码器，因此预期结果是在采集任何内容之前返回 `encode_failed`。
- 测试通过文件头以及用 ffmpeg 解码帧来检查 MP4 文件。Windows 生成的 MP4 可以在 Chrome 中播放，也能在 Windows 媒体组件中打开；QuickTime Player 和 Safari 尚未检查。
- 桌面应用需要 Windows 或 macOS；在 Linux 上 `recdr gui` 会以 `unsupported_platform` 失败。它录制区域或窗口，不能录制显示器或标签页。套在窗口上的取景框在窗口移动时会留在原位。
- 在 Windows 11 上，应用的四个边框窗口各有一圈由系统绘制的细灰色轮廓和圆角端点，egui 无法关闭它们。
- 在 macOS 上，尚未试过取景框在全屏应用（它有自己的空间）之上的表现。

<a id="license"></a>
## 许可证

MIT
