# wayscriber

[![许可证](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.98.1%2B-orange.svg)](https://www.rust-lang.org/)

[English](README.md) · **简体中文**

> 本页是英文 README 的机器辅助翻译。如中英文内容不一致，以[英文版](README.md)为准。非常欢迎中文流利的读者审阅并纠正翻译；您可以[提交问题](https://github.com/devmobasa/wayscriber/issues)或[提交拉取请求](https://github.com/devmobasa/wayscriber/pulls)。

对应英文 README.md：`5f3a257f`（2026-10-18；发布版本 `0.9.27`）。
<!-- translated-from: README.md @ 5f3a257f88fecc9f5e1884e094fa6dfae6b7bffc -->

一款用 Rust 编写、类似 ZoomIt 的 Linux/Wayland 实时屏幕标注工具。

在任意应用上绘图，演示时可放大或冻结屏幕，并在会话之间保留画板。轻量级后台服务只需一个快捷键即可显示或隐藏叠层。

[安装](#安装) · [快速上手](#首次启动) · [操作参考](#操作参考) · [配置](#配置) · [故障排查](#故障排查) · [完整文档](https://wayscriber.com/docs/)（英文）

![在编辑器上使用 wayscriber 进行代码审查：用方框和步骤标记标出循环，高亮一行代码，用便签箭头指向内容，圈出表达式，并画上手绘对勾](https://wayscriber.com/img/hero-annotate.webp)

<details>
<summary>演示视频</summary>

[观看演示 (wayscriber.com)](https://wayscriber.com/media/demo.webm)

https://github.com/user-attachments/assets/be0e1bea-594b-4f03-a24e-a934b61a4716

</details>

## 平台支持

| 平台 | 状态 | 说明 |
|----------|--------|-------|
| Wayland (layer-shell) | ✅ 支持 | Hyprland、Sway、River、Wayfire、Niri/Cosmic |
| Plasma/KWin | ✅ 支持 | 使用 layer-shell 叠层；冻结和缩放使用 `xdg-desktop-portal-kde` 截图门户 |
| GNOME | ⚠️ 部分支持 | 支持普通叠层，并在门户可用时支持冻结；不支持[轻量穿透](#轻量穿透模式) |
| X11 | ❌ | 不支持 |

在 Plasma/KWin 上，冻结和缩放需要可用的截图门户，并且可能需要您授权门户访问。

v0.9.23+ 的预编译 `wayscriber` 软件包需要 glibc 2.39 和 GTK 4.12。请参阅 [Debian 和 Ubuntu](#debian-和-ubuntu) 以及 [Fedora 和 RHEL](#fedora-和-rhel) 中的说明。AUR 软件包和 Nix 不受影响。

<details>
<summary>已测试的环境</summary>

- Ubuntu 25.10 GNOME（xdg 回退）
- Fedora 43 KDE（Plasma、layer-shell）
- Fedora 43 GNOME（xdg 回退）
- Debian 13.2 KDE（Plasma、layer-shell）
- Debian 13.2 GNOME（xdg 回退）
- CachyOS 2025-August KDE（Plasma、layer-shell）
- Arch 上的 Hyprland（layer-shell）
- Arch 上的 Niri（layer-shell）

</details>

## 功能

### 绘图

拖动即可用画笔绘图。按住 <kbd>Shift</kbd> 画直线，按住 <kbd>Ctrl</kbd> 画矩形，按住 <kbd>Tab</kbd> 画椭圆，按住 <kbd>Ctrl+Shift</kbd> 画箭头。

![径向菜单在带标注的代码审查画面上打开，工具位于中心，颜色位于外圈](https://wayscriber.com/img/radial-menu.webp)

*<kbd>Middle-click</kbd> 在光标处打开径向菜单。*

- 自由画笔、荧光笔，以及圆形／矩形橡皮擦
- 形状笔（<kbd>S</kbd>）：明确的笔画会转换为可编辑的直线、圆、椭圆、矩形或三角形；其他笔画保留手绘效果，撤销一次即可将识别出的形状恢复为原始笔迹
- 画笔平滑：在释放按键时清理已完成的画笔和马克笔笔画，因此实时线条不会落后于光标
- 形状：直线、矩形、椭圆和多边形，支持填充开关
- 四种箭头样式：标准、尖头、曲线和双端箭头；可选自动编号标签，并提供用于分步演示的步骤标记
- 模糊工具有四种样式：柔化、像素化、安全遮盖（将区域变成一种纯色）和涂黑
- 支持平滑显示的多行文本和便签，以及让标签在任意背景上保持可读的可选文本光晕
- 颜色选择器、带放大像素取样窗口的屏幕取色器、调色板，以及通过快捷键或滚轮调整大小
- 可浏览所有已安装字体的字体选择器，以及使用 <kbd>Shift+T</kbd> 循环切换的可配置字体列表
- 预设：将工具、颜色、大小以及填充／不透明度／文本设置保存到 3–5 个槽位；应用：<kbd>1</kbd>–<kbd>5</kbd>，保存：<kbd>Shift+1</kbd>–<kbd>Shift+5</kbd>
- 渲染颜色配置，用于打印、投影仪和浅色主题预览

<details>
<summary>绘图详情</summary>

- **形状笔：** 短暂显示的提示条（例如 "Circle · Ctrl+Z keeps ink"）会说明识别出的形状（`[drawing] shape_recognition_feedback`）。按 <kbd>S</kbd>，点击工具栏中画笔旁的按钮（简单模式下位于形状选择器中），或在命令面板中选择 **形状笔工具 (Shape Pen Tool)**。可以通过样式胶囊中的 **笔感 (Pen feel)** 面板（或根据 `[ui.toolbar] stroke_controls` 使用 **形状 (Shapes)** 刻度或 **识别 (Detect)** 步进控件）、`[drawing] shape_recognition_sensitivity`（0–4），或配置器的绘图页面调整识别灵敏度；该级别随会话保存。
- **画笔平滑：** 设置 `[drawing] pen_smoothing`（0–6），或使用样式胶囊的 **笔感** 提示条，打开带平滑刻度和实时预览的面板；`[ui.toolbar] stroke_controls` 可将其替换为内嵌 **平滑 (Smooth)** 刻度或步进控件。保留绘图板压力值，平滑级别随会话保存。
- **曲线箭头：** 拖动控制柄，让箭头绕开遮挡内容。
- **文本光晕：** 光晕根据标签下方的背景选择对比度，让文本在画板、填充形状或冻结屏幕上保持可读。通过 `[drawing] text_halo_enabled = false` 禁用光晕。实时透明画板没有可采样的像素，因此回退到文本颜色。
- **字体循环：** <kbd>Shift+T</kbd> 在 `[drawing] font_cycle` 中循环切换；选中文本时会修改其字体。
- **字体选择器：** 在命令面板中选择 **字体选择器 (Font Picker)**，或点击工具栏的当前字体按钮。支持搜索、等宽字体筛选、滚轮滚动和加速方向键连发，每行均使用对应字体显示。
- **径向菜单：** 快速选择工具／颜色、访问最近颜色，按下后轻扫并释放即可选择工具；还提供可拖动的外圈大小控制和滚轮调节。

[![代码行旁的三张便签](https://wayscriber.com/img/poster-sticky-notes.webp)](https://wayscriber.com/media/sticky-notes.mp4)

*便签的文本可以编辑：按 <kbd>N</kbd>，点击后输入。点击图片播放视频。*

</details>

### 选择与编辑

按住 <kbd>Alt</kbd> 拖动或按 <kbd>V</kbd> 进行选择。双击形状可打开其属性。

- 也可按 <kbd>Ctrl+Alt+P</kbd> 或右键 → **属性… (Properties…)** 打开属性面板；双击文本或便签则进入编辑
- 修改颜色、粗细、不透明度、填充和箭头头部；锁定、调整层级、复制或删除选区（[观看选择演示视频](https://wayscriber.com/media/context-menu-selection-boards-1080p.mp4)）
- 复制（<kbd>Ctrl+D</kbd>）、删除（<kbd>Delete</kbd>）、撤销／重做

<details>
<summary>属性面板详情</summary>

- 快捷颜色色板，以及用于打开完整颜色选择器的 **+**。不透明色板改变色相并保留各形状的不透明度。
- 粗细和不透明度滑块。一次拖动对应一次撤销。
- 矩形、椭圆和多边形的填充行。首个色板为“无填充”，其余色板用自己的颜色填充，与边框独立。
- −/+ 步进控件、开关、箭头起点／终点、图示箭头样式和锁定开关。
- 置底、后移一层、前移一层、置顶、复制和删除按钮。
- 预设行可将工具预设的样式应用到选区而保留当前工具；点击 **保存 (Save)** 再选槽位，即可将选区样式保存为该预设。
- 在行上滚动可逐步调整。悬停标题可查看形状 ID 和创建时间。

</details>

### 演示工具

按 <kbd>L</kbd> 使用激光笔，按 <kbd>Ctrl+Shift+F</kbd> 冻结屏幕，或用 <kbd>Ctrl</kbd> 加滚轮缩放。

[![放大查看用红圈标出的代码行，旁边写着 one query per item](https://wayscriber.com/img/poster-zoom.webp)](https://wayscriber.com/media/zoom.mp4)

*放大后仍可继续绘图。点击图片播放视频。*

- 激光笔：发光笔迹跟随指针，释放后短暂保留并逐渐消失。激光笔迹不会保存、撤销、选择、导出或捕获
- 使用 ZoomIt 风格的操作缩放
- 屏幕冻结：暂停显示内容，但应用继续运行
- 聚光灯：拖出椭圆，使其周围变暗；可叠加多个聚光灯来突出多个区域，并可选择 1×–4× 放大
- 编号标注：自动编号的箭头标签和步骤标记；使用 <kbd>Ctrl+Shift+R</kbd> 重置箭头标签
- 点击高亮，可配置颜色、半径和持续时间；点击高亮工具启用时持续显示圆环
- 演示模式（<kbd>Ctrl+Shift+M</kbd>）：隐藏界面并强制启用点击高亮
- 输入提示（<kbd>Ctrl+Shift+K</kbd>）：在屏幕上显示按键和点击提示，适合演示和录屏（[观看输入提示演示视频](https://wayscriber.com/media/input-hud.mp4)）
- 轻量穿透（layer-shell）：绘图时，输入可以穿透到下层应用。参阅[轻量穿透模式](#轻量穿透模式)

<details>
<summary>演示工具详情</summary>

- **激光笔：** 消失前绘制的笔画归为一组，并一起消失。颜色、宽度和时间可在 `[laser]` 或配置器的绘图页面中设置；在演示模式（即使锁定为高亮工具）和轻量穿透模式下仍可使用。参阅 [docs/CONFIG.md](docs/CONFIG.md#laser---laser-pointer)（英文）。
- **缩放：**
  - 放大／缩小：<kbd>Ctrl</kbd> 加滚轮（<kbd>Ctrl+Alt</kbd> 加滚轮同样有效），或 <kbd>Ctrl+Alt</kbd> 加 <kbd>+</kbd>/<kbd>-</kbd>
  - 重置：<kbd>Ctrl+Alt+0</kbd>；锁定视图：<kbd>Ctrl+Alt+L</kbd>
  - 平移：中键拖动或方向键
  - 右键菜单：**缩放 (Zoom)** → 放大 (Zoom In) / 缩小 (Zoom Out) / 重置缩放 (Reset Zoom)；缩放期间，右键仍可打开菜单，开头显示 放大、缩小、退出缩放 (Exit Zoom) 和 锁定视图 (Lock View)
- **屏幕冻结：** 优先使用合成器原生的 `wlr-screencopy` 或 `ext-image-copy-capture`，可用时回退到截图门户。
- **聚光灯：** 每个聚光灯可将开口放大 1×–4×；暗化强度和边缘柔和度仍由 `[spotlight]` 统一设置。放大功能使用纯色画板、冻结、缩放或捕获／导出背景中的完整像素；实时透明画板仅显示普通开口，并提示您冻结屏幕。
- **输入提示：** 可通过 `input-monitor` 构建特性启用系统范围捕获。参阅 [docs/CONFIG.md](docs/CONFIG.md#uiinput_hud---input-hud-keystrokes-and-clicks)（英文）。

</details>

### 画板

<kbd>Ctrl+W</kbd> 切换白板，<kbd>Ctrl+B</kbd> 切换黑板，<kbd>Ctrl+Shift+B</kbd> 打开画板选择器。

![白板图表上的画板选择器，列出五块画板及其快捷键，以及活动画板的页面](https://wayscriber.com/img/boards-and-pages.webp)

*画板选择器列出每块画板的快捷键和页面。*

- 命名画板，可使用透明叠层或自定义背景
- 可调间距的笛卡尔方格纸、等距网格线和等距点阵
- 每块画板有独立页面，画笔颜色自动适应背景对比度
- 使用 <kbd>Space</kbd> 加左键拖动平移纯色画板；从上下文菜单重置（[观看平移演示视频](https://wayscriber.com/media/wayscriber-pan-around.mp4)）
- 快速跳转槽位：<kbd>Ctrl+Shift+1..9</kbd>

<details>
<summary>更多画板截图</summary>

| | |
| :---: | :---: |
| ![黑板模式](https://wayscriber.com/img/blackboard.webp)<br>**黑板 (Blackboard)** | ![白板模式](https://wayscriber.com/img/whiteboard.webp)<br>**白板 (Whiteboard)** |
| ![带网格的 Blueprint 画板，上面画着构建、测试和部署流程](https://wayscriber.com/img/blueprint.webp)<br>**Blueprint（带网格纸的自定义画板）** | [![用圆圈、高亮、箭头和方框标注的代码](https://wayscriber.com/img/poster-pages-and-presets.webp)](https://wayscriber.com/media/pages-and-presets.mp4)<br>**页面与预设（视频）** |

</details>

### 捕获与截图

<kbd>Ctrl+Shift+C</kbd> 选择区域后，可复制、保存、两者兼做，或添加到画板。

- 保存全屏截图、截取活动窗口或捕获区域
- 复制到剪贴板或保存到文件
- 从屏幕复制文本（OCR）：按 <kbd>Ctrl+Shift+X</kbd> 并拖选屏幕区域，识别出的文本会复制到剪贴板。需要 `tesseract`（[观看 OCR 演示视频](https://wayscriber.com/media/ocr-copy-text.mp4)）
- 将活动画板或所有画板导出为多页 PDF

所有截图快捷键、文本识别、区域选择器和预览步骤见[截图与导出](#截图与导出)。

### 会话与持久化

画板、撤销／重做历史和工具状态在重启后仍会保留。会话持久化默认启用。

- 支持每个输出的默认会话，以及通过 `--session-file` 指定的命名会话文件
- 可通过叠层的会话面板、配置器会话标签页、托盘开关和 CLI 覆盖项控制
- 完整流程请参阅[会话管理器与持久化](#会话管理器与持久化)

### 工具栏与界面

<kbd>Ctrl+K</kbd> 可搜索所有动作，<kbd>F1</kbd> 列出快捷键。

- 界面支持英文和简体中文。默认跟随桌面区域设置；可在配置器中或通过 `--language` 切换。参阅[界面语言](#界面语言)
- 统一的浮动工具栏（<kbd>F9</kbd> 切换可见性；<kbd>F2</kbd> 将顶部工具条在完整 → 微型提示条 → 隐藏之间切换），提供预设槽位以及图标或文本显示模式
- 命令面板（也可按 <kbd>Ctrl+Shift+P</kbd>）：搜索、运行、编辑、取消绑定或重置动作快捷键
- 录制快捷键：按住 <kbd>Ctrl</kbd>+<kbd>Shift</kbd> 并点击可绑定的工具栏控件
- 快捷参考（<kbd>Shift+F1</kbd>）
- 带扩展调色板和屏幕取色器的颜色选择器（工具栏、弹出窗口或命令面板）
- 可分别配置各项目的状态栏

<details>
<summary>工具栏与界面详情</summary>

![代码编辑器上方的命令面板正在搜索 “board”，列出画板相关动作及其快捷键](https://wayscriber.com/img/command-palette.webp)

*命令面板按名称查找动作，并显示其快捷键。*

- **工具栏前端：** 支持 layer-shell 的合成器（Hyprland、KWin、Wayfire、River 等）使用 GTK4 工具栏；其他环境（GNOME xdg 回退、强制内嵌模式、未启用 `toolbar-gtk` 的构建）自动回退到内置 Cairo 工具栏。可通过 `ui.toolbar.backend = "auto" | "gtk" | "builtin"` 或 `WAYSCRIBER_TOOLBAR_BACKEND` 显式选择前端。
- **状态栏：** 可分别配置输出、选区、画板、页面、颜色、工具、大小、上下文、工具栏提示、帮助以及关于／版本项目；有两个或更多输出时显示输出项目，关于／版本需自行启用。
- **帮助叠层：** 未绑定的动作默认隐藏，按 <kbd>Tab</kbd> 或点击 **显示未绑定 (Show Unbound)** 即可显示；搜索始终可以找到它们。
- **快捷键录制：** 该 <kbd>Ctrl</kbd>+<kbd>Shift</kbd> 修饰键组合可以配置。Wayscriber 仅将该动作的设置写入 `config.toml`，并生成带时间戳的 `.bak` 备份。选中命令面板行后按 <kbd>Ctrl+Shift+E</kbd>，可在配置器中打开同一快捷键。

| | |
| :---: | :---: |
| ![简单工具栏](https://wayscriber.com/img/toolbar-simple.webp)<br>**简单工具栏** | ![高级工具栏](https://wayscriber.com/img/toolbar-advanced.webp)<br>**高级工具栏** |
| ![工具栏设置弹出菜单，包含布局模式以及预设、缩放、画板和页面开关](https://wayscriber.com/img/toolbar-settings.webp)<br>**工具栏设置** | ![选中便签旁的上下文菜单，包含编辑、复制、创建副本、排列、锁定、缩放和删除](https://wayscriber.com/img/context-menu.webp)<br>**上下文菜单** |

</details>

### 多显示器
- 在显示器之间移动叠层焦点：<kbd>Ctrl+Alt+Shift+←</kbd>/<kbd>Ctrl+Alt+Shift+→</kbd>
- 输出焦点变化时，工具栏和状态栏跟随活动输出
- 有两个或更多输出时，状态栏显示活动输出标记（`ui.active_output_badge`；`ui.active_output_badge_always` 可使其在单输出时仍然显示）
- `session.per_output = true` 时，按输出恢复会话
- GNOME 回退模式可通过 `ui.preferred_output` 或 `WAYSCRIBER_XDG_OUTPUT` 固定输出

### 与 ZoomIt 对比

| 功能 | ZoomIt (Windows) | wayscriber |
|---------|------------------|------------|
| 绘图工具 | ✅ | ✅ |
| 画板／背景 | ✅ | ✅ |
| 多行文本 | ❌ | ✅ |
| 自定义字体 | ❌ | ✅ |
| 配置文件 | ❌ | ✅ |
| 帮助叠层 | ❌ | ✅ |
| 缩放 | ✅ | ✅ |
| 休息计时器 | ✅ | ❌ |

## 安装

根据您的环境选择安装方式：

| 您的需求 | 安装方式 |
|----------|-----|
| 在 Debian/Ubuntu/Mint/Pop!_OS 上通过 CLI 快速安装并自动更新 | [Debian 和 Ubuntu](#debian-和-ubuntu) |
| 在 Fedora/Nobara 或 RHEL/Rocky/Alma 10+ 上通过 CLI 快速安装并自动更新 | [Fedora 和 RHEL](#fedora-和-rhel) |
| Arch、Manjaro、CachyOS 或其他 Arch 衍生发行版 | [AUR 或直接安装发布版](#arch-linux-aur)；AUR 可用时优先选择 `wayscriber-bin` |
| NixOS，或其他发行版上的 Nix 包管理器 | [NixOS 和 Nix](#nixos-和-nix)：常规安装使用 `nixpkgs`，最新发布版和 Configurator 使用项目 flake |
| 不添加软件源，通过浏览器或终端单次安装软件包 | [GitHub Releases](#github-releases单次安装) |
| 修改 wayscriber 或构建本地二进制文件 | [从源码构建](#从源码构建) |

对于 CLI 用户，发行版软件源、AUR 和声明式 NixOS 安装是合适的默认选择，因为它们遵循系统的常规更新流程。通过 `nix profile` 安装的软件包需要使用 `nix profile upgrade` 单独更新。从 GitHub 下载的 `.deb`/`.rpm` 和直接安装 Arch 发布版的脚本不提供自动更新；需要新版时，请重新运行安装程序或替换软件包。

请先安装主程序 `wayscriber`。[`wayscriber-configurator`](#配置器-gui) 是可选的 GUI 设置应用，不包含 `wayscriber` 二进制文件。

### Debian 和 Ubuntu

Linux Mint、Pop!_OS 和其他 Debian 衍生发行版同样使用此方式。

> **Wayscriber v0.9.23+ 需要 Ubuntu 24.04+、Debian 13 (trixie) 或更新版本。** 包括 Linux Mint 22+ 和 Pop!_OS 24.04+。发布包需要 glibc 2.39 和 GTK 4.12，并已内嵌固定版本的 GTK4 layer-shell 集成，因此无需单独安装 `libgtk4-layer-shell0`。Pop!_OS 22.04 的 GTK 版本仍不满足要求；请在该系统上使用[不含 GTK 的源码构建](#从源码构建)。

```bash
sudo apt update
sudo apt install curl gpg
sudo install -d -m 0755 /usr/share/keyrings
curl -fsSL https://wayscriber.com/apt/WAYSCRIBER-GPG-KEY.asc | gpg --dearmor | sudo tee /usr/share/keyrings/wayscriber.gpg > /dev/null
echo "deb [signed-by=/usr/share/keyrings/wayscriber.gpg] https://wayscriber.com/apt stable main" | sudo tee /etc/apt/sources.list.d/wayscriber.list
sudo apt update
sudo apt install wayscriber
```

可选的 GUI 配置器：

```bash
sudo apt install wayscriber-configurator
```


如需不添加软件源而单次安装 `.deb`，请参阅 [GitHub Releases](#github-releases单次安装)。

### Fedora 和 RHEL

支持的 Fedora 和 Nobara 版本，或 RHEL、Rocky Linux、AlmaLinux 10+，可使用此方式。

> **Wayscriber v0.9.23+ 的预编译 RPM 需要 glibc 2.39 和 GTK 4.12。** 适用于 RHEL/Rocky/Alma 10+ 和兼容的 Fedora/Nobara 版本。

```bash
cat <<'EOF' | sudo tee /etc/yum.repos.d/wayscriber.repo
[wayscriber]
name=Wayscriber Repo
baseurl=https://wayscriber.com/rpm
enabled=1
gpgcheck=1
repo_gpgcheck=1
gpgkey=https://wayscriber.com/rpm/RPM-GPG-KEY-wayscriber.asc
EOF
sudo dnf clean all
sudo dnf install wayscriber
```

可选的 GUI 配置器：

```bash
sudo dnf install wayscriber-configurator
```


如需不添加软件源而单次安装 `.rpm`，请参阅 [GitHub Releases](#github-releases单次安装)。

### Arch Linux (AUR)

Manjaro、CachyOS 和其他 Arch 衍生发行版同样使用此方式。推荐 AUR，因为 pacman 会跟踪安装的文件，AUR 助手会负责升级。示例使用 `yay`；如使用 `paru`，将命令中的 `yay` 换成 `paru` 即可。

预编译二进制包：

```bash
yay -S wayscriber-bin
```

或从源码构建：

```bash
yay -S wayscriber
```

可选的 GUI 配置器：

```bash
yay -S wayscriber-configurator
```


如果 AUR 不可用或尚未更新到最新发布版，可改用直接安装 x86_64 发布版的脚本。先下载脚本，以便在执行前查看内容：

```bash
curl -fsSL https://wayscriber.com/arch-install.sh -o arch-install.sh
sh arch-install.sh --dry-run
```

确认预演结果后再安装：

```bash
sh arch-install.sh
```

<details>
<summary>直接安装脚本的检查内容及更新方式</summary>

预演会验证最新稳定版、校验和、安装清单、所需 Arch 软件包和文件所属软件包，不安装任何文件。预演和正式安装都会拒绝检测到的其他 Wayscriber 副本：`/usr` 下不受包管理器管理的文件、`~/.local/bin/wayscriber`，或 `ExecStart` 不是 `/usr/local/bin/wayscriber` 的用户单元／覆盖配置。已经指向安装目标的符号链接不算冲突。在上述命令中添加 `--replace-other`，可先移除其他副本。如果 `wayscriber.service` 已在运行，安装程序会重启它，使后台叠层使用 `/usr/local/bin/wayscriber`；添加 `--no-restart` 可保持当前单元运行。`--version` 显示 crate 版本而非 Git 哈希，因此安装后请检查已安装路径和运行服务的 `ExecStart`。更新此类安装时，请重新下载脚本并再次运行。

</details>

<details>
<summary>从直接安装迁移到 AUR</summary>

迁移到 AUR 前，请先卸载直接安装的版本。否则，`/usr/local/bin` 和 `/usr/local/lib/systemd/user` 仍会优先于 `/usr` 下的软件包文件：

```bash
systemctl --user disable --now wayscriber.service
curl -fsSL https://wayscriber.com/arch-install.sh -o arch-install.sh
sh arch-install.sh --uninstall --dry-run
```

确认预演结果后，卸载并改用 AUR 软件包：

```bash
sh arch-install.sh --uninstall
yay -S wayscriber-bin
systemctl --user enable --now wayscriber.service
```

卸载程序仅删除直接安装清单中位于 `/usr/local` 下、未由包管理器管理的文件。它保留 pacman 管理的文件以及您的配置和数据；服务仍在运行或启用时，它会拒绝实际删除。`/usr` 下残留的不受管理的 Git 构建会保留；仅在确实也要移除该 `/usr` 副本时才添加 `--remove-unmanaged-usr`。默认卸载不会删除 `~/.local/bin/wayscriber` 或用户单元／覆盖配置；如果它们仍指向 `/usr/local`，脚本会发出警告。

</details>

### NixOS 和 Nix

Wayscriber 已打包到 [`nixpkgs`](https://search.nixos.org/packages?query=wayscriber)。在 NixOS 上请声明式安装；`nix profile` 主要适用于在其他 Linux 发行版上运行 Nix 的情况。

**您会获得哪个版本：**

| 来源 | 版本跟踪方式 |
|--------|--------|
| `nixpkgs-unstable` | 跟随 Wayscriber 发布，通常延迟数周 |
| 稳定通道（`nixos-25.11`、`nixos-26.05` 等） | 版本在发布分支建立时固定；不向后移植版本升级 |
| 本项目的 flake | 使用 `flake.lock` 中记录的标签或分支 |

如果您使用稳定版 NixOS 通道并希望获得最新发布版，请使用[项目 flake](#latest-release-and-configurator)，无需等待 `nixpkgs`。

**NixOS：通过 nixpkgs 安装。** 将 Wayscriber 添加到 `configuration.nix` 或其他 NixOS 模块：

```nix
{ pkgs, ... }:

{
  environment.systemPackages = with pkgs; [
    wayscriber

    # Optional screenshot and clipboard helpers on wlroots compositors:
    grim
    slurp
    wl-clipboard

    # Optional: copy text from screen (OCR)
    (tesseract.override { enableLanguages = [ "eng" ]; })
  ];
}
```


随后运行 `sudo nixos-rebuild switch` 重新构建；基于 flake 的配置使用 `sudo nixos-rebuild switch --flake .#myhost`。

**Home Manager：**

```nix
{ pkgs, ... }:

{
  home.packages = [ pkgs.wayscriber ];
}
```


随后运行 `home-manager switch`。

`nixpkgs` 构建安装 `wayscriber` 二进制文件。桌面入口、图标和 `wayscriber.service` systemd 用户单元由下文的 flake 软件包提供；它们也提供可选的 Configurator。

**不安装直接试用**（nixpkgs 版本）：

```bash
nix run nixpkgs#wayscriber -- --active
```

或使用当前上游开发分支：

```bash
nix run github:devmobasa/wayscriber -- --active
```


`nix run` 不会将 `wayscriber` 加入 `PATH`。

<a id="latest-release-and-configurator"></a>
<details>
<summary>NixOS 上的最新发布版和 Configurator</summary>

项目 flake 提供固定到发布版的软件包以及可选的 Configurator。将输入添加到现有 `flake.nix` 时，请把 `RELEASE_TAG` 替换为已发布的标签，例如[最新发布版](https://github.com/devmobasa/wayscriber/releases/latest)中显示的标签：

```nix
inputs.wayscriber.url = "github:devmobasa/wayscriber?ref=RELEASE_TAG";
```


将它加入 `outputs`：

```nix
outputs = { nixpkgs, wayscriber, ... }: {
  # Your existing outputs...
};
```


随后将下列模块加入 `nixosSystem` 的 `modules` 列表：

```nix
({ pkgs, ... }:

  let
    wayscriberPackages =
      wayscriber.packages.${pkgs.stdenv.hostPlatform.system};
  in
  {
    environment.systemPackages = [
      wayscriberPackages.wayscriber
      wayscriberPackages.wayscriber-configurator
    ];

    # Required before `systemctl --user enable --now wayscriber.service` works.
    systemd.packages = [ wayscriberPackages.wayscriber ];
  }
)
```


`flake.lock` 会固定准确的 Wayscriber 修订版本。升级时，先将 `flake.nix` 中的 `RELEASE_TAG` 改为新的已发布标签，然后仅更新该输入：

```bash
nix flake update wayscriber
sudo nixos-rebuild switch --flake .#myhost
```

</details>

**其他 Linux 发行版上的 Nix。** 在 Ubuntu、Fedora 或其他 Linux 发行版上使用 Nix 时，可以安装到用户配置环境：

```bash
nix profile install nixpkgs#wayscriber
```


如需当前上游开发分支：

```bash
nix profile install github:devmobasa/wayscriber#wayscriber
```

同一分支的可选 Configurator：

```bash
nix profile install github:devmobasa/wayscriber#wayscriber-configurator
```


通过 profile 安装的软件包不会随系统一起更新；请使用 `nix profile upgrade wayscriber`（以及 `nix profile upgrade wayscriber-configurator`）升级，或使用 `nix profile remove wayscriber` 移除。

**开发 shell**（用于贡献或本地构建，不用于安装）：

```bash
nix develop github:devmobasa/wayscriber
```


### GitHub Releases（单次安装）

如果不希望添加软件源，可以直接安装发布包。这些软件包不会自动更新。

在浏览器中：

1. 打开[最新发布版](https://github.com/devmobasa/wayscriber/releases/latest)。
2. 安装适合您发行版的主程序包：
   - [wayscriber-amd64.deb](https://github.com/devmobasa/wayscriber/releases/latest/download/wayscriber-amd64.deb)：v0.9.23+ 支持 Ubuntu 24.04+、Linux Mint 22+、Pop!_OS 24.04+、Debian 13+ 和兼容的更新 Debian 衍生发行版（参阅[版本要求](#debian-和-ubuntu)）
   - [wayscriber-x86_64.rpm](https://github.com/devmobasa/wayscriber/releases/latest/download/wayscriber-x86_64.rpm)：v0.9.23+ 支持兼容的 Fedora/Nobara 版本以及 RHEL/Rocky/Alma 10+（参阅[版本要求](#fedora-和-rhel)）
3. 可选：安装 wayscriber 后，再安装配置器软件包：
   - [wayscriber-configurator-amd64.deb](https://github.com/devmobasa/wayscriber/releases/latest/download/wayscriber-configurator-amd64.deb)
   - [wayscriber-configurator-x86_64.rpm](https://github.com/devmobasa/wayscriber/releases/latest/download/wayscriber-configurator-x86_64.rpm)
   - 注意：`wayscriber-configurator` 是独立应用，不会安装 `wayscriber`
4. 从应用菜单／启动器启动 **Wayscriber**。
5. 可选：以后按照[后台模式](#后台模式推荐)的步骤切换到后台模式（推荐）。

<details>
<summary>从终端进行相同的单次安装 (.deb / .rpm)</summary>

Debian/Ubuntu：

```bash
wget -O wayscriber-amd64.deb https://github.com/devmobasa/wayscriber/releases/latest/download/wayscriber-amd64.deb
sudo apt install ./wayscriber-amd64.deb
```


Fedora/Nobara 或 RHEL/Rocky/Alma 10+：

```bash
wget -O wayscriber-x86_64.rpm https://github.com/devmobasa/wayscriber/releases/latest/download/wayscriber-x86_64.rpm
sudo dnf install ./wayscriber-x86_64.rpm
```


Configurator .deb（可选）：

```bash
wget -O wayscriber-configurator-amd64.deb https://github.com/devmobasa/wayscriber/releases/latest/download/wayscriber-configurator-amd64.deb
sudo apt install ./wayscriber-configurator-amd64.deb
```


Configurator .rpm（可选）：

```bash
wget -O wayscriber-configurator-x86_64.rpm https://github.com/devmobasa/wayscriber/releases/latest/download/wayscriber-configurator-x86_64.rpm
sudo dnf install ./wayscriber-configurator-x86_64.rpm
```


</details>

### 从源码构建

需要 Rust 1.98.1 或更新版本。如果尚未安装 `rustup`，请先安装它（也会安装 `cargo`），随后在当前 shell 中加载 Cargo：

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```


克隆仓库，在仓库目录中运行依赖安装和构建命令：

```bash
git clone https://github.com/devmobasa/wayscriber.git
cd wayscriber
rustup toolchain install 1.98.1 --component clippy --component rustfmt
cargo --version
```


仓库的 `rust-toolchain.toml` 会在此检出目录中选择 Rust 1.98.1。如果此前为目录设置了旧版覆盖，请在该目录执行 `rustup override unset`，让工具链文件生效。

**依赖：**

Debian 13+ / Ubuntu 25.04+（包括 Ubuntu 26.04 LTS）：

```bash
sudo apt-get install build-essential pkg-config libcairo2-dev libwayland-dev libpango1.0-dev libgtk-4-dev libgtk4-layer-shell-dev
```

Fedora：

```bash
sudo dnf install gcc gcc-c++ make pkgconf-pkg-config cairo-devel wayland-devel pango-devel libxkbcommon-devel cairo-gobject-devel gtk4-devel gtk4-layer-shell-devel
```


**Ubuntu 24.04 LTS 和 Mint 22** 没有 `libgtk4-layer-shell-dev` 软件包，但 GTK 本身的版本满足要求。您可以从固定版本的源码构建这个小型 layer-shell 库（需要 `meson ninja-build wayland-protocols`，安装到 `/usr`）：

```bash
sudo apt-get install meson ninja-build wayland-protocols
bash tools/install-gtk4-layer-shell.sh   # builds + installs gtk4-layer-shell 1.3.0
```


也可以跳过它，使用下文不含 GTK 的构建方式。这会保留内置 Cairo 工具栏和其他所有默认特性。

**Pop!_OS 22.04 提供 GTK 4.6，低于默认构建所需的 GTK 4.12 API。** 因此，仅构建 gtk4-layer-shell 仍然不够；除非单独安装 GTK 4.12 或更新版本，否则请使用下文不含 GTK 的构建方式。

**构建：**

包含 GTK 工具栏前端（默认）：

```bash
cargo build --release
# Binary: target/release/wayscriber
```


不包含 GTK 工具栏前端：

```bash
cargo build --release --no-default-features --features tablet-input,portal,tray
# Binary: target/release/wayscriber
```


**可选安装脚本：**

```bash
./tools/install.sh
```


### 截图工具

deb/rpm/AUR 软件包会自动安装 `wl-clipboard`、`grim` 和 `slurp`。直接安装 Arch 发布版的脚本会检查这些软件是否已安装，缺失时打印所需的 `pacman` 命令。如果从源码构建或使用 tarball，请手动安装：

Debian/Ubuntu：

```bash
sudo apt-get install wl-clipboard grim slurp
```

Fedora：

```bash
sudo dnf install wl-clipboard grim slurp
```


### 从屏幕复制文本 (OCR)

[从屏幕复制文本](#截图与导出)需要 Tesseract 以及 `[capture].ocr_languages` 中配置的语言数据（默认 `eng`）：

Arch/Omarchy：

```bash
sudo pacman -S tesseract tesseract-data-eng
```

Debian/Ubuntu：

```bash
sudo apt-get install tesseract-ocr
```

Fedora：

```bash
sudo dnf install tesseract tesseract-langpack-eng
```

## 首次启动

安装后运行：

```bash
wayscriber --version
wayscriber --active
```


（如果您使用 `nix run` 而非安装，`wayscriber` 不会出现在 `PATH` 中。请继续使用[安装](#nixos-和-nix)中的 `nix run` 命令。）

叠层显示后：

- <kbd>F1</kbd> 或 <kbd>F10</kbd>：帮助叠层
- <kbd>Shift+F1</kbd>：快捷参考
- <kbd>Ctrl+K</kbd> / <kbd>Ctrl+Shift+P</kbd>：命令面板（搜索 `monitor` 或 `display` 可查找输出相关动作）
- <kbd>F11</kbd>：[配置器](#配置器-gui)
- <kbd>Escape</kbd>：隐藏或退出。Escape 关闭界面后，接下来的 400 ms 内会忽略进一步的 Escape 按键

界面默认跟随桌面区域设置。如需选择语言，请按 <kbd>F11</kbd>，打开 **UI → 语言 (Language)**，或参阅[界面语言](#界面语言)。

首次启动会显示简短的引导卡片：绘图与撤销、工具栏与退出方式、颜色和粗细、快捷菜单，以及查找命令；最后会提供后台模式设置入口。<kbd>Shift+Escape</kbd> 可跳过引导；命令面板或帮助叠层页脚中的 **重播引导 (Replay Tour)** 可重新播放卡片。

<details>
<summary>引导与提示的行为</summary>

卡片上的每个动作都是按钮（**知道了 (Got it)**、**设置 (Set up)**、**暂不 (Not now)**、**跳过导览 (Skip tour)**），旁边标有对应按键；点击卡片绝不会绘图。已经完成或跳过引导的用户配置文件不会再次自动显示。重播引导前会先退出缩放；在演示模式下，会等到您离开该模式后开始。

功能发现提示和快捷键教学提示提供 **知道了** 与 **提示设置… (Tip settings…)** 控件。**知道了** 仅永久确认这一条提示；**提示设置…** 除此之外还会在配置器中打开全局自动引导开关。使用提示介绍的功能，也会阻止该提示再次出现。点击消息正文只会在本次运行中关闭提示；无人操作的提示最多出现三次。工具栏隐藏时的恢复提示保留 **显示 (Show)** 作为主要操作，并提供相同的设置入口。关闭自动引导会在后续叠层启动时生效；手动重放引导和真实的功能或配置警告仍然可用。

</details>

日常使用建议设置[后台模式](#后台模式推荐)：切换更快，会话状态也会在多次激活之间保留。

### 获取帮助

- **关于窗口：** `wayscriber --about`
- **检查新版本：** `wayscriber --check-update`（Wayscriber 不会自行安装更新，只会提供适合您安装方式的操作说明）。使用 `[updates] check = false` 或 `WAYSCRIBER_DISABLE_UPDATE_CHECK=1` 可关闭后台检查。参阅 https://wayscriber.com/docs/getting-started/updating.html （英文）
- **完整文档：** https://wayscriber.com/docs/ （英文）

## 使用方法

### 后台模式（推荐）

让 wayscriber 在后台运行，通过快捷键切换显示。推荐用于日常使用：切换更快，工作流程更顺畅，并支持会话持久化。

**启用服务：**

```bash
systemctl --user enable --now wayscriber.service
wayscriber --daemon-toggle
```


更喜欢 GUI？打开 `wayscriber-configurator`，进入 **后台模式 (Background Mode)** 标签页，先点击 **安装服务 (Install Service)**（已安装时为 **更新服务 (Update Service)**），再点击 **启动服务 (Start Service)**。

**绑定切换快捷键：**

在合成器或桌面环境中，为 `wayscriber --daemon-toggle` 绑定全局快捷键。配置器的 **后台模式** 标签页也可完成此操作：设置快捷键并点击 **应用快捷键 (Apply Shortcut)**（GNOME：写入 GNOME 自定义快捷键；KDE/Plasma：写入 systemd 覆盖配置环境变量 `WAYSCRIBER_PORTAL_SHORTCUT`，用于门户全局快捷键）。

Hyprland Lua 配置（`~/.config/hypr/bindings.lua`）：

```lua
o.bind("SUPER + D", "Toggle Wayscriber", "wayscriber --daemon-toggle")
```


传统 Hyprland 配置（`~/.config/hypr/hyprland.conf`）：

```conf
bind = SUPER, D, exec, wayscriber --daemon-toggle
```


随后重新加载配置：

```bash
hyprctl reload
```


GNOME（Ubuntu/Debian/Fedora）：
1. 打开 `Settings -> Keyboard -> Keyboard Shortcuts`。
2. 滚动到 `Custom Shortcuts`，点击 `+`。
3. 名称：`Wayscriber Toggle`。
4. 命令：`wayscriber --daemon-toggle`。
5. 设置快捷键（Ubuntu GNOME 推荐 <kbd>Super+G</kbd>；<kbd>Super+D</kbd> 通常已经被占用）。

KDE Plasma：
1. 打开 `Settings -> Keyboard -> Shortcuts`。
2. 添加新的 `Command or Script`。
3. 名称：`Wayscriber Toggle`。
4. 命令：`wayscriber --daemon-toggle`。
5. 指定按键，例如 <kbd>Meta+Shift+D</kbd>。

其他桌面／窗口管理器：将 `wayscriber --daemon-toggle` 绑定到您喜欢的任意全局快捷键。

> 只保留一个切换绑定。重复条目（例如两条 `SUPER+D` 绑定，或同一按键上残留的单次运行绑定）可能触发两次，立即抵消切换效果。
> 如果合成器的快捷键环境无法从 `PATH` 找到 `wayscriber`，请使用 `command -v wayscriber` 给出的绝对路径。

**系统托盘：**

右键点击托盘图标查看选项。不常用的动作归入绘图模式、截图、设置与数据子菜单，以便在较矮或缩放后的显示器上使用：

- 切换叠层可见性
- 冻结／解冻当前叠层
- 捕获全屏／活动窗口／区域
- 切换帮助叠层
- 在配置器中打开会话持久化设置
- 清除已保存的会话数据
- 打开日志文件夹
- 打开配置器／打开配置文件／退出

<details>
<summary>托盘图标样式</summary>

支持的桌面使用随主题适配的符号托盘图标。无法可靠解析命名图标的托盘宿主（包括 Omarchy/Quickshell、Noctalia/Quickshell 和 COSMIC）会自动获得适应缩放的彩色像素图，包括 48px 的 HiDPI 版本。将 `[tray].icon_style` 设置为 `"auto"`（默认）、`"symbolic"` 或 `"colored"` 可选择主托盘图标样式；修改后请重启后台进程。如果没有系统托盘，请使用 `--no-tray` 或 `WAYSCRIBER_NO_TRAY=1`。若图标仍为空白，或菜单显示方框占位符，请使用 `WAYSCRIBER_TRAY_FORCE_PIXMAP=1` 启动后台进程；该环境变量优先于 TOML 设置。

</details>

**替代方式：使用合成器自动启动，而非 systemd：**

Hyprland Lua：在 `~/.config/hypr/autostart.lua` 中添加：

```lua
o.launch_on_start("wayscriber --daemon")
```

然后在 `~/.config/hypr/bindings.lua` 中添加：

```lua
o.bind("SUPER + D", "Toggle Wayscriber", "wayscriber --daemon-toggle")
```


传统 Hyprland 配置：

```conf
exec-once = wayscriber --daemon
bind = SUPER, D, exec, wayscriber --daemon-toggle
```


**服务命令：**

查看状态：

```bash
systemctl --user status wayscriber.service
```

重启服务：

```bash
systemctl --user restart wayscriber.service
```

跟踪日志：

```bash
journalctl --user -u wayscriber.service -f
```


### 单次运行模式（替代方式）

需要时启动 wayscriber，使用完后退出：

```bash
wayscriber --active
```

以白板启动：

```bash
wayscriber --active --mode whiteboard
```

以自定义画板启动：

```bash
wayscriber --active --mode blueprint
```

启动时冻结屏幕：

```bash
wayscriber --freeze
```


`whiteboard` 和 `blackboard` 是内置画板。`blueprint` 是在 `config.toml` 的 `[[boards.items]]` 下定义的自定义画板示例。参阅[配置](#配置)。

绑定到按键（Hyprland Lua 配置）：

```lua
o.bind("SUPER + D", "Open Wayscriber", "wayscriber --active")
```


传统 Hyprland 配置：

```conf
bind = SUPER, D, exec, wayscriber --active
```


叠层内的快捷键与[首次启动](#首次启动)中的说明相同；<kbd>Escape</kbd> 或 <kbd>Ctrl+Q</kbd> 退出。

### 轻量穿透模式

轻量穿透模式仅适用于 layer-shell 合成器。它让正常键盘和指针输入传递给下层应用，同时保持 wayscriber 可见以供绘图。

[![代码编辑器上方粘贴的图表和标注](https://wayscriber.com/img/poster-paste-and-passthroughmode.webp)](https://wayscriber.com/media/paste-and-passthroughmode.mp4)

*按 <kbd>Ctrl+Alt+V</kbd> 粘贴图片，再按 <kbd>F6</kbd>：点击和滚动会传递到下层应用，标注保持可见。点击图片播放视频。*

- <kbd>F6</kbd> 从已聚焦叠层进入穿透模式。它是 wayscriber 内部快捷键，不是操作系统／全局快捷键。穿透启用后，wayscriber 可能不再接收到该按键
- 如需可靠控制（包括退出穿透模式），请为 `wayscriber --light-toggle` 和 `wayscriber --light-draw-toggle` 绑定合成器／全局快捷键
- 如需按住时绘图的快捷键，请在按下时运行 `wayscriber --light-draw-on`，在释放时运行 `wayscriber --light-draw-off`
- Hyprland 和 KDE 的绑定示例见 [docs/SETUP.md](docs/SETUP.md#light-passthrough-controls-on-hyprland)（英文）；KDE 部分位于 Hyprland 绑定示例之后
- **原生 GNOME Wayland 不支持此模式。** 普通应用窗口无法提供所需的输入穿透 shell 叠层。冻结仍可能用于静态图像捕获，但不能代替实时穿透；真正的 shell 级穿透需要 GNOME Shell 扩展方案

### 截图与导出

| 快捷键 | 动作 |
|----------|--------|
| <kbd>Ctrl+Alt+F</kbd> | 捕获全屏（遵循 `capture.copy_to_clipboard`） |
| <kbd>Ctrl+Shift+O</kbd> | 捕获活动窗口（遵循 `capture.copy_to_clipboard`） |
| <kbd>Ctrl+Shift+I</kbd> | 捕获选区（遵循 `capture.copy_to_clipboard`） |
| <kbd>Ctrl+C</kbd> | 将全屏复制到剪贴板 |
| <kbd>Ctrl+S</kbd> | 将全屏保存为 PNG |
| <kbd>Ctrl+Shift+C</kbd> | 选择区域，再选择复制、保存、两者或画板（`capture_region_interactive`） |
| <kbd>Ctrl+Shift+S</kbd> | 选择区域 → 保存 PNG |
| <kbd>Ctrl+6</kbd> | 区域 → 剪贴板（明确指定） |
| <kbd>Ctrl+Alt+6</kbd> | 区域 → 保存 PNG（明确指定） |
| 未绑定 | 一步选择区域并复制到剪贴板（`capture_clipboard_selection`） |
| 未绑定 | 测量逻辑屏幕区域，不捕获图像（`measure_mode`） |
| <kbd>Ctrl+Alt+O</kbd> | 打开上次截图所在文件夹 |

标有“遵循 `capture.copy_to_clipboard`”的快捷键按 `config.toml` 的该设置决定输出到剪贴板还是文件；其他快捷键始终使用表中指定的目标。捕获需要[截图工具](#截图工具)，缺失时回退到 xdg-desktop-portal。

捕获后，简短通知会显示保存的文件名。保存到文件时，**打开文件夹 (Open folder)** 和 **复制路径 (Copy path)** 按钮可打开截图文件夹或复制完整路径。

全屏和区域捕获默认包含活动画板上已经提交的绘图。设置 `[capture].include_drawings = false` 可获取原始桌面像素。未完成的笔画、选择控制柄、预览、工具栏和其他 Wayscriber 界面不会包含在结果中。透明画板上的全屏捕获和旧版 `slurp` 捕获会保留标注后的桌面；纯色画板则保留画布背景。原生区域选择器始终把已提交的绘图合成到冻结的桌面裁剪上。

使用 `--exit-after-capture` / `--no-exit-after-capture` 可覆盖捕获后是否关闭叠层。

<details>
<summary>区域选择器</summary>

- 区域快捷键默认使用 Wayscriber 原生的冻结图像选择器。设置 `capture.region.picker = "slurp"` 可使用外部选择器；没有可用屏幕捕获后端时，原生选择也会回退到 `slurp`。
- 读数、快捷键图例和可选的放大像素取样窗口可在 `[capture.region]` 中配置。
- 即使常规截图格式设为 JPEG，区域截图仍以 PNG 编码并命名。显式设置 `picker = "slurp"` 则保留配置的旧版截图格式。
- 原生区域选择器会在冻结桌面上显示已提交的绘图，使选择时的预览与带标注导出一致。
- 在 Hyprland 和 Sway 上，原生选择器中的 <kbd>Space</kbd> 可在自由区域选择和当前工作区窗口选择之间切换。指向并点击窗口，或使用 <kbd>Super</kbd> 加方向键及 <kbd>Enter</kbd>。
- 合成器无法提供可靠几何信息时，不显示窗口控件；选择器复用旧的用户冻结或缩放图像时也不显示，仅为本次选择器新建的自动冻结图像提供窗口边界。
- 显示候选窗口之前，会验证提供者结果是否与该图像源的输出及布局标识一致；不一致时仅禁用窗口模式。Wayland 在冻结时刻没有可移植的工作区标识，因此候选窗口反映的是执行合成器查询时可见的工作区。

</details>

<details>
<summary>预览：复制、保存、两者、画板和剪切</summary>

- `capture_region_interactive` 默认占用 <kbd>Ctrl+Shift+C</kbd>，也可从命令面板访问。该动作始终使用原生选择器，确保外部选择器无法绕过预览。
- 这个组合键过去运行一步完成的 `capture_clipboard_selection`，后者现在默认未绑定；预览中的 **复制 (Copy)**（<kbd>Ctrl+C</kbd>）可得到相同的剪贴板结果，显式绑定 `capture_clipboard_selection` 可恢复一步复制。
- **两者 (Both)**（或 <kbd>Enter</kbd>）始终同时复制 PNG 并保存文件。
- **导出时包含绘图 (Include drawings in exports)**（或 <kbd>D</kbd>）以配置值为默认值，可仅针对这次捕获覆盖；复制、保存、两者和画板都会遵循该设置。关闭绘图会立即恢复原始桌面像素预览。
- 将带标注的裁剪图添加回其来源画板，会把这些标注的第二份扁平化副本写入图像；希望把原始裁剪放上画板时，请先关闭该开关。
- **剪切 (Cut)**（或 <kbd>X</kbd>）可删除捕获像素中的行或列：在选区上拖动；后续切割基于已经折叠的输出。**撤销 (Undo)**（<kbd>Ctrl+Z</kbd>）、**重做 (Redo)**（<kbd>Ctrl+Y</kbd> 或 <kbd>Ctrl+Shift+Z</kbd>）和 **重置 (Reset)** 仅影响本次预览，绝不更改画板绘图。
- 切割后，源裁剪区域会锁定，直到撤销所有切割或重置；切割工具待用或已有切割时，放大取样窗口隐藏。复制、保存、两者和画板均接收折叠后的结果；画板保留原来的左上角位置，并缩小到输出大小。

</details>

`measure_mode` 也以命令面板为主要入口，默认未绑定。它显示十字光标和逻辑像素尺寸，不冻结或捕获屏幕；按 <kbd>Esc</kbd> 或再次运行该动作即可退出。

**从屏幕复制文本 (OCR)：** 按 <kbd>Ctrl+Shift+X</kbd> 并拖选屏幕区域，或在选择器中按 <kbd>Ctrl+A</kbd> 读取整个屏幕。Tesseract 运行时，扫描带会扫过该区域，随后卡片显示结果。识别出的文本仅写入剪贴板。其工具栏按钮默认隐藏，需手动开启。OCR 选区生效期间，如果显示的屏幕图像变化，选择会被取消，确保选区坐标不会应用到替换后的冻结或缩放像素上。Tesseract 的安装方法见[从屏幕复制文本 (OCR)](#从屏幕复制文本-ocr)。

<details>
<summary>PDF 导出</summary>

命令面板和快捷键中提供画布导出动作。`export_board_pdf_file` 将活动画板保存为多页 PDF，`export_all_boards_pdf_file` 按画板顺序保存所有画板；两项 PDF 动作默认均未绑定。透明页面在 PDF 中默认留空，除非设置 `[export.pdf] transparent_background = "desktop"`，此时仅为透明页面捕获叠层后的实时桌面。含放大聚光灯的页面会栅格化，让放大窗口采样完整像素；页面标签仍作为矢量内容写在其上。

</details>

## 操作参考

按 <kbd>F1</kbd> 可查看完整的应用内操作速查表。

![Wayscriber 操作速查帮助叠层，快捷键按绘图、动作、画板以及画笔与文本分组](https://wayscriber.com/img/controls-cheatsheet.webp)

### 常用快捷键

| 动作 | 按键 |
|--------|-----|
| 自由画笔 | 拖动 |
| 箭头 | <kbd>Ctrl+Shift</kbd> 加拖动 |
| 矩形 | <kbd>Ctrl</kbd> 加拖动 |
| 文本模式 | <kbd>T</kbd> |
| 选择／移动 | <kbd>Alt</kbd> 加拖动，或 <kbd>V</kbd> |
| 撤销／重做 | <kbd>Ctrl+Z</kbd> / <kbd>Ctrl+Y</kbd> |
| 颜色键 | <kbd>R</kbd> <kbd>G</kbd> <kbd>B</kbd> <kbd>Y</kbd> <kbd>O</kbd> <kbd>P</kbd> <kbd>W</kbd> <kbd>K</kbd> |
| 大小 | <kbd>+</kbd> / <kbd>-</kbd> 或滚轮 |
| 帮助 | <kbd>F1</kbd> |
| 退出 | <kbd>Escape</kbd> |

<details>
<summary>全部绘图工具</summary>

| 动作 | 按键／鼠标 |
|--------|-----------|
| 自由画笔 | 按住鼠标左键拖动 |
| 直线 | <kbd>Shift</kbd> 加拖动 |
| 矩形 | <kbd>Ctrl</kbd> 加拖动 |
| 椭圆／圆 | <kbd>Tab</kbd> 加拖动 |
| 箭头 | <kbd>Ctrl+Shift</kbd> 加拖动 |
| 三角形／平行四边形／菱形／正多边形 | 顶部工具条的 **形状选择器 (Shape picker)**（可绑定） |
| 自由多边形 | 打开 **形状选择器**，然后点击各顶点；<kbd>Enter</kbd> 或双击完成 |
| 模糊 | **形状选择器**（可绑定）：拖选区域；通过 **切换模糊样式 (Cycle Blur Style)** 切换样式 |
| 聚光灯 | **形状选择器**（可绑定）：拖出椭圆，其他区域变暗；在样式胶囊中设置 1×–4× 放大，在放大窗口上滚动，或选中未锁定的放大窗口并拖动画布上的旋钮 |
| 步骤标记工具 | 工具栏（可绑定） |
| 高亮画刷 | <kbd>Ctrl+Alt+H</kbd> |
| 激光笔（渐隐笔迹） | <kbd>L</kbd>，然后拖动 |
| 文本模式 | <kbd>T</kbd>，<kbd>Click</kbd> 放置，输入文本，<kbd>Enter</kbd> 完成 |
| 便签 | <kbd>N</kbd>，<kbd>Click</kbd> 放置，输入文本，<kbd>Enter</kbd> 完成 |

![形状按钮下方展开的形状选择器](https://wayscriber.com/img/shape-picker.webp)

**形状选择器的位置。** 顶部工具条直接显示常用工具，其余工具归入单个 **形状选择器** 按钮。简单工具条直接显示选择、画笔、马克笔、步骤标记和橡皮擦。常规工具条还显示形状笔、激光笔、直线和箭头，选择器内提供矩形、椭圆、模糊、聚光灯和多边形工具。高级工具条还直接显示矩形、椭圆、模糊和聚光灯，因此选择器中只保留多边形工具。可从工具条最右端的布局按钮选择布局。

每种工具也都是独立的工具栏项目，可以通过设置弹出菜单（齿轮图标）或 `config.toml` 中的 `ui.toolbar.items` 显示、隐藏和排序。例如，撤销／重做旁的截图按钮默认可见，设置 `hidden = ["top.utility.screenshot"]` 可隐藏。

这些工具的默认快捷键有意留空；经常使用时可在 `[keybindings.tools]` 中绑定。拖动和鼠标按键映射也可配置。参阅[拖动工具映射](#拖动工具映射)。

</details>

<details>
<summary>文本编辑</summary>

[![放置在白板上的多行文本](https://wayscriber.com/img/poster-text-manipulation.webp)](https://wayscriber.com/media/text-manipulation.mp4)

*放置多行文本、移动它、用 <kbd>Shift+T</kbd> 切换字体，之后双击即可编辑。点击图片播放视频。*

编辑文本块或便签时，下列按键属于编辑器，不能配置。撤销、工具切换、画板导航和截图仍使用各自的常规绑定。

文本块放置在屏幕边缘，或输入过程中增长到边缘之外时，会向左或向上移动，确保完整可见。编辑原本已经超出边缘的文本块不会使其移动。

| 动作 | 按键／鼠标 |
|--------|-----------|
| 放置光标 | <kbd>Click</kbd> |
| 选择文本 | <kbd>Shift</kbd> 加 <kbd>Click</kbd>，或 <kbd>Shift</kbd> 加方向键 |
| 选择全部文本 | <kbd>Ctrl+A</kbd> |
| 移动整个文本块 | <kbd>Alt</kbd> 加拖动 |
| 按字符移动 | <kbd>←</kbd> / <kbd>→</kbd> |
| 按词移动 | <kbd>Ctrl</kbd> 加 <kbd>←</kbd>/<kbd>→</kbd> |
| 按行移动 | <kbd>↑</kbd> / <kbd>↓</kbd> |
| 行首／行尾 | <kbd>Home</kbd> / <kbd>End</kbd> |
| 文本起点／终点 | <kbd>Ctrl+Home</kbd> / <kbd>Ctrl+End</kbd> |
| 复制／剪切／粘贴 | <kbd>Ctrl+C</kbd> / <kbd>Ctrl+X</kbd> / <kbd>Ctrl+V</kbd> |
| 删除字符 | <kbd>Backspace</kbd> / <kbd>Delete</kbd> |
| 删除词 | <kbd>Ctrl+Backspace</kbd> / <kbd>Ctrl+Delete</kbd> |
| 换行 | <kbd>Shift+Enter</kbd> |
| 完成编辑 | <kbd>Enter</kbd> |
| 取消编辑 | <kbd>Escape</kbd> |

光标移动和选区遵循实际渲染布局，因此自动换行文本以及从右向左或混合方向文字都能正确操作。<kbd>Ctrl+C</kbd> 和 <kbd>Ctrl+X</kbd> 只作用于已选文本；没有选区时，会继续执行这两个组合键绑定的其他动作。复制、剪切和粘贴通过 `wl-copy`/`wl-paste` 使用系统剪贴板。

</details>

<details>
<summary>画板</summary>

| 动作 | 按键 |
|--------|-----|
| 切换白板 | <kbd>Ctrl+W</kbd> |
| 切换黑板 | <kbd>Ctrl+B</kbd> |
| 返回透明画板 | <kbd>Ctrl+Shift+T</kbd> |
| 切换画板槽位 | <kbd>Ctrl+Shift+1..9</kbd> |
| 上一个／下一个输出 | <kbd>Ctrl+Alt+Shift</kbd> 加 <kbd>←</kbd>/<kbd>→</kbd> |
| 上一块／下一块画板 | <kbd>Ctrl+Shift</kbd> 加 <kbd>←</kbd>/<kbd>→</kbd> |
| 新建画板 | <kbd>Ctrl+Shift+N</kbd> |
| 删除画板 | <kbd>Ctrl+Shift+Delete</kbd> |
| 画板选择器 | <kbd>Ctrl+Shift+B</kbd> |
| 平移纯色画板 | 按住 <kbd>Space</kbd> 加左键拖动 |
| 重置纯色画板平移 | <kbd>Right-click</kbd> → 重置画布位置 (Reset Canvas Position) |

</details>

<details>
<summary>颜色与大小</summary>

| 颜色 | 按键 |
|-------|-----|
| 红色 | <kbd>R</kbd> |
| 绿色 | <kbd>G</kbd> |
| 蓝色 | <kbd>B</kbd> |
| 黄色 | <kbd>Y</kbd> |
| 橙色 | <kbd>O</kbd> |
| 粉色 | <kbd>P</kbd> |
| 白色 | <kbd>W</kbd> |
| 黑色 | <kbd>K</kbd> |

前八个快捷颜色对应上表快捷键，可以自定义。参阅[快捷颜色](#快捷颜色)。

使用屏幕取色器可直接从显示的桌面选取颜色：按 <kbd>I</kbd>、使用工具栏或颜色选择器中的取色器按钮，或在命令面板（<kbd>Ctrl+K</kbd>）中搜索 **屏幕取色 (Pick screen color)**。如果偏好其他快捷键，可以重新绑定：

```toml
[keybindings.colors]
pick_screen_color = ["I"]
```


| 动作 | 按键 |
|--------|-----|
| 增加粗细 | <kbd>+</kbd> / <kbd>=</kbd> / 向下滚轮 |
| 减少粗细 | <kbd>-</kbd> / <kbd>_</kbd> / 向上滚轮 |
| 增大字体 | <kbd>Ctrl+Shift</kbd> 加 <kbd>+</kbd> / <kbd>Shift</kbd> 加向下滚轮 |
| 减小字体 | <kbd>Ctrl+Shift</kbd> 加 <kbd>-</kbd> / <kbd>Shift</kbd> 加向上滚轮 |
| 增大马克笔不透明度 | <kbd>Ctrl+Alt</kbd> 加 <kbd>↑</kbd> |
| 减小马克笔不透明度 | <kbd>Ctrl+Alt</kbd> 加 <kbd>↓</kbd> |

</details>

<details>
<summary>选择与排列</summary>

| 动作 | 按键 |
|--------|-----|
| 复制选区 | <kbd>Ctrl+D</kbd> |
| 将选区复制到剪贴板 | <kbd>Ctrl+Alt+C</kbd> |
| 粘贴选区或复制的 PNG/JPEG 图像 | <kbd>Ctrl+Alt+V</kbd> |
| 删除选区 | <kbd>Delete</kbd> |
| 置顶／置底 | <kbd>]</kbd> / <kbd>[</kbd> |
| 微移选区 | 方向键（大步移动：<kbd>PageUp</kbd>/<kbd>PageDown</kbd>） |
| 移到边缘 | <kbd>Home</kbd>/<kbd>End</kbd> / <kbd>Ctrl+Home</kbd>/<kbd>Ctrl+End</kbd> |
| 选择／移动形状 | 按住 <kbd>Alt</kbd> 加拖动 |
| 选择工具 | <kbd>V</kbd> |
| 添加到选区 | <kbd>Shift</kbd> 加点击 |
| 选区属性 | <kbd>Ctrl+Alt+P</kbd> |

</details>

<details>
<summary>页面</summary>

| 动作 | 按键 |
|--------|-----|
| 上一页／下一页 | <kbd>Ctrl+Alt</kbd> 加 <kbd>←</kbd>/<kbd>→</kbd>，或 <kbd>Ctrl+Alt</kbd> 加 <kbd>PageUp</kbd>/<kbd>PageDown</kbd> |
| 新建页面 | <kbd>Ctrl+Alt+N</kbd> |
| 复制页面 | <kbd>Ctrl+Alt+D</kbd> |
| 删除页面 | <kbd>Ctrl+Alt+Delete</kbd> |

</details>

<details>
<summary>编辑与界面</summary>

| 动作 | 按键 |
|--------|-----|
| 撤销 | <kbd>Ctrl+Z</kbd> |
| 重做 | <kbd>Ctrl+Shift+Z</kbd> / <kbd>Ctrl+Y</kbd> |
| 全选 | <kbd>Ctrl+A</kbd> |
| 橡皮擦 | <kbd>D</kbd> |
| 切换橡皮擦模式 | <kbd>Ctrl+Shift+E</kbd> |
| 清空全部 | <kbd>E</kbd> |
| 取消动作 | 绘图时 <kbd>Right-click</kbd>，或 <kbd>Escape</kbd> |
| 上下文菜单 | 空闲时 <kbd>Right-click</kbd> / <kbd>Shift+F10</kbd> / <kbd>Menu</kbd>，用 <kbd>Arrow keys</kbd> 加 <kbd>Enter</kbd>/<kbd>Space</kbd> 操作 |
| 编辑选中的文本／便签 | <kbd>Enter</kbd>（仅选中一个对象时） |
| 切换工具栏 | <kbd>F9</kbd> |
| 循环切换顶部工具条（完整 → 微型 → 隐藏） | <kbd>F2</kbd> |
| 帮助叠层 | <kbd>F1</kbd> / <kbd>F10</kbd> |
| 快捷参考 | <kbd>Shift+F1</kbd> |
| 配置器 | <kbd>F11</kbd> |
| 命令面板 | <kbd>Ctrl+K</kbd> / <kbd>Ctrl+Shift+P</kbd> |
| 屏幕取色器 | <kbd>I</kbd>、工具栏／颜色选择器，或 <kbd>Ctrl+K</kbd> → **屏幕取色** |
| 径向菜单 | 空闲时 <kbd>Middle-click</kbd> 打开／关闭；按住并向扇区轻扫可立即选择（在中心释放则取消）；<kbd>Left-click</kbd> 选择；<kbd>Right-click</kbd>/<kbd>Escape</kbd> 关闭；拖动外圈或滚轮可调整活动工具大小 |
| 状态栏 | <kbd>F4</kbd> / <kbd>F12</kbd> |
| 应用预设槽位 | <kbd>1</kbd> - <kbd>5</kbd> |
| 保存预设槽位 | <kbd>Shift+1</kbd> - <kbd>Shift+5</kbd> |
| 切换点击高亮 | <kbd>Ctrl+Shift+H</kbd> |
| 切换输入提示 | <kbd>Ctrl+Shift+K</kbd> |
| 切换轻量穿透（叠层内） | <kbd>F6</kbd>（参阅[轻量穿透模式](#轻量穿透模式)） |
| 重置箭头标签 | <kbd>Ctrl+Shift+R</kbd> |
| 切换冻结 | <kbd>Ctrl+Shift+F</kbd> |
| 放大／缩小 | <kbd>Ctrl</kbd> 加滚轮 / <kbd>Ctrl+Alt</kbd> 加 <kbd>+</kbd> / <kbd>Ctrl+Alt</kbd> 加 <kbd>-</kbd> |
| 重置缩放 | <kbd>Ctrl+Alt+0</kbd> |
| 切换缩放锁定 | <kbd>Ctrl+Alt+L</kbd> |
| 平移缩放视图 | <kbd>Middle drag</kbd> / <kbd>Arrow keys</kbd> |
| 退出 | <kbd>Escape</kbd> / <kbd>Ctrl+Q</kbd> |

</details>

说明：

- 在箭头工具栏中启用自动编号后，箭头标签会自动递增；<kbd>Ctrl+Shift+R</kbd> 可重置
- 箭头样式（标准、尖头、曲线、双端）可从箭头工具栏的图示样式菜单选择，或使用 **切换箭头样式 (Cycle Arrow Style)** 命令。存在选中箭头时，命令修改它们的样式；否则设置下一个箭头的样式。默认没有快捷键；可在 `config.toml` 绑定 `cycle_arrow_style`，并使用 `[arrow] style` 设定新箭头的初始样式。选中的曲线箭头在弧线中间显示圆形控制柄，可拖动调整曲线，按住 <kbd>Shift</kbd> 吸附
- 步骤标记自动递增，可从工具栏重置，或在 `config.toml` 绑定 `reset_step_markers`
- 预设槽位可从工具栏保存／清除；槽位立即更新，由后台任务写回 `config.toml`，文件写入完成后通知会确认。名称和高级字段可在配置器的预设标签页编辑
- 模糊工具默认没有快捷键；如需直接用键盘访问，可在 `config.toml` 绑定 `select_blur_tool`

## 配置

配置文件：`~/.config/wayscriber/config.toml`

**从示例创建：**

```bash
mkdir -p ~/.config/wayscriber
cp config.example.toml ~/.config/wayscriber/config.toml
```


**或使用 [GUI 配置器](#配置器-gui)：**

```bash
wayscriber-configurator   # or press F11
```


完整参考见 `docs/CONFIG.md`（英文）和 https://wayscriber.com/docs/ （英文）。

`config.toml` 保存您主动编写的默认设置，Wayscriber 不会自行更改它。有两类操作会写入它：配置器的 **保存**，以及叠层中的三个编辑器（快捷键编辑、预设槽位和快捷颜色面板）。每个叠层编辑器仅重写自己的键，并先将旧文件复制为带时间戳的 `.bak` 备份。

<details>
<summary>运行时偏好与已停用设置</summary>

偶尔切换的偏好只作用于当前运行，并提供前往负责该默认值的配置器页面的入口。

工具栏固定状态（固定按钮和工具栏显示／隐藏快捷键会更新它，使下次启动与当前屏幕状态一致）、拖动后的顶部工具条位置、顶部工具条显示形式及最小化状态、单项工具栏的可见性／顺序，以及在叠层中更改的画板固定状态，均属于直接交互状态，单独保存在自动生成的 `$XDG_DATA_HOME/wayscriber/runtime-ui.toml` 中。可从叠层设置弹出菜单查看、恢复或重置这些运行时偏好（**详情 (Details)** 显示路径，**重置工具栏更改 (Reset toolbar changes)** 恢复配置的默认值）；参阅[配置指南](docs/CONFIG.md#configured-defaults-and-runtime-ui-preferences)（英文）。

统一顶部工具栏是唯一布局。旧的面板键（例如 `side_layout`）仍可读取，保存时也会保留，但不再影响运行中的叠层；配置器会将其列为可删除的已停用设置。沿用历史名称的 `side.*` 项目 ID 仍用于自定义顶部工具栏控件，不得重命名。

</details>

### 配置器 (GUI)

`wayscriber-configurator` 是使用 Relm4 构建的原生 GTK4/libadwaita 应用，作为独立的可选软件包提供。可随时从叠层按 <kbd>F11</kbd> 打开，或运行 `wayscriber-configurator`。

它覆盖 `config.toml` 的大部分设置：绘图与箭头默认值、渲染配置和性能、界面语言、界面与工具栏布局、演示模式、历史、捕获与 PDF 导出、画板、会话、绘图板输入、预设和快捷键。一些设置仍需手动编辑：`[tray]`、`[updates]`、`[spotlight]`、`[tablet.stylus_button]` 的动作映射，以及 `[boards]` 中完整的多画板设置。配置器保存时会保留这些设置。

[![在 Wayland 桌面上用 wayscriber 配置器编辑设置](https://wayscriber.com/img/poster-configurator.webp)](https://wayscriber.com/#configurator)

- 编辑主动编写的默认设置，提供验证和自动备份；保存会保留注释、顺序以及此构建不认识的键
- 可搜索列表中约有 130 个可重新绑定的动作；在命令面板行上按 <kbd>Ctrl+Shift+E</kbd> 可直接打开相应快捷键
- 安装和管理后台服务，在 GNOME 和 KDE 上应用全局快捷键
- 管理会话：重命名、复制、移动或清除已保存的画板与工具状态
- 运行时界面偏好保存在独立文件中，绝不会被覆盖

<details>
<summary>更多配置器截图</summary>

| | |
| :---: | :---: |
| ![配置器颜色页面中的默认颜色和快捷颜色](https://wayscriber.com/img/configurator-colors.webp)<br>**颜色 (Colors)** | ![配置器渲染页面中的缓冲区和动画设置](https://wayscriber.com/img/configurator-rendering.webp)<br>**渲染 (Rendering)** |
| ![配置器偏好设置页面](https://wayscriber.com/img/configurator-preferences.webp)<br>**偏好设置** | ![配置器快捷键页面中的可搜索动作](https://wayscriber.com/img/configurator-keybindings.webp)<br>**快捷键 (Keybindings)** |

</details>

<details>
<summary>配置器演示视频</summary>

[观看演示 (wayscriber.com)](https://wayscriber.com/media/configurator.webm)

<!--
  如需在此嵌入播放器，请将网站仓库中的 media/configurator.mp4 拖入本仓库任意
  issue 或 PR 的评论框（不要提交评论），然后把生成的
  https://github.com/user-attachments/assets/<uuid> URL 单独粘贴在下方一行。
  GitHub 仅为自己的附件 URL 渲染视频。
-->

</details>

这是独立的可选软件包。请先安装主程序 `wayscriber`，再通过[安装](#安装)添加配置器。完整参考：https://wayscriber.com/docs/configuration/configurator.html （英文）

### 主要配置节

```toml
[drawing]
default_color = "red"
default_thickness = 3.0
polygon_sides = 5

[[drawing.quick_colors]]
label = "Blush"
color = "#FFB3BA"

[drawing.drag_tools.right]
drag_tool = "pen"
drag_color = "blue"

[presets]
# slot_count: 3-5
slot_count = 5

[presets.slot_1]
name = "Red pen"
tool = "pen"
color = "red"
size = 3.0

[performance]
# vsync and fps caps — see Performance tuning under Troubleshooting

[ui]
# status bar visibility and position

[boards]
# named boards + backgrounds
```


### 拖动工具映射

可通过 `[drawing]`（`drag_tool`、`shift_drag_tool`、`ctrl_drag_tool`、`ctrl_shift_drag_tool`、`tab_drag_tool`）或配置器绘图标签页配置拖动修饰键映射。按鼠标按钮区分的工作流程可使用 `[drawing.drag_tools.left]`、`[drawing.drag_tools.right]` 和 `[drawing.drag_tools.middle]`；每个绑定都可设置工具和可选颜色。多边形工具的默认快捷键有意留空，可从工具栏选择器选择，或自行绑定。自由多边形可以选择，但不能绑定到拖动操作。

### 快捷颜色

快捷调色板通过按顺序排列的 `[[drawing.quick_colors]]` 条目配置。前八个条目对应 <kbd>R</kbd>/<kbd>G</kbd>/<kbd>B</kbd>/<kbd>Y</kbd>/<kbd>O</kbd>/<kbd>P</kbd>/<kbd>W</kbd>/<kbd>K</kbd>；手动配置不足八项时，缺少的快捷键位置使用内置默认值。隐式默认工具栏调色板还将青色、紫色和灰色保留为扩展颜色，而径向菜单维持原来的前八色圆环。显式配置的第八项之后没有快捷键动作绑定，会将额外颜色加入密集调色界面，最多取前 24 色。顶部工具栏样式胶囊最多显示其中 12 色（宽度不足时更少），选区属性面板最多显示 17 色；颜色选择器可以访问任意颜色。

也可不打开编辑器直接修改调色板：**右键点击任意色板**，打开该槽位的颜色选择器。拖动时色板实时更新；确定 (OK) 将该条目的颜色写回 `config.toml`（保留其他设置和注释，生成带时间戳的 `.bak` 备份），取消 (Cancel) 则恢复原状。槽位保留原来的标签和快捷键，因此改成另一种红色后，<kbd>R</kbd> 仍然选择红色槽位。修改当前绘图颜色的色板时，实时颜色也会跟随变化。左键仍只选择色板，最左侧的提示条仍打开活动工具自身的颜色选择器。

改变主意了？重新着色选择器提供 **默认 (Default)** 按钮，可加载 wayscriber 为该槽位提供的默认颜色。它与其他颜色选择一样，先暂存并预览，最终仍由 确定/取消 决定。该按钮仅对 11 个内置槽位显示；您自己添加的额外颜色没有内置默认值可恢复。

### 会话管理器与持久化

会话持久化默认启用。可在配置器中管理（<kbd>F11</kbd> → 会话标签页，也可通过托盘的 **会话持久化设置… (Session persistence settings…)** 进入），或使用下列 CLI 参数仅针对本次运行覆盖。

强制恢复（持久化并恢复所有画板、历史和工具状态）：

```bash
wayscriber --resume-session
```

本次运行禁用恢复：

```bash
wayscriber --no-resume-session
```

查看已保存的会话：

```bash
wayscriber --session-info
```

删除已保存的画板：

```bash
wayscriber --clear-session
```

重置已保存的工具默认值，保留画板和历史：

```bash
wayscriber --clear-tool-state
```

命名会话文件：`--session-file` 可与上述任意参数组合，也可与 `--active`、`--freeze` 或 `--daemon` 组合。例如，在单次运行的叠层中打开讲座会话：

```bash
wayscriber --active --session-file ~/Documents/lecture-04.wayscriber-session
```

或通过后台进程打开会议会话：

```bash
wayscriber --daemon-toggle --session-file ~/Documents/meeting.wayscriber-session
```


完整的 CLI、叠层和配置器工作流程见[会话管理器示例](examples/session-manager.md)（英文）。

<details>
<summary>行为说明</summary>

- 全新启动将笔迹恢复到透明叠层画板时，会显示例如 “已从上次会话恢复 7 条标注” 的通知，并提供 **清空 (Clear)**（与清空画布一样可以撤销），因为恢复的笔迹现在覆盖在当前屏幕内容上。后台切换、没有内容的恢复和纯色画板不会显示该通知
- 配置值提供启动默认设置。启用 `restore_tool_state`（默认）时，会话中保存的上次工具设置（包括箭头头部位置和聚光灯初始放大倍率）会在启动时覆盖这些默认值。运行 `wayscriber --clear-tool-state` 可只删除保存的工具层，使下次启动使用配置默认值，画板和历史仍保留。运行中的叠层可使用 命令面板 (Command Palette) → 重置工具默认值 (Reset Tool Defaults)，清除保存层并立即把配置默认值应用到活动工具
- `--session-file` 精确使用指定文件，隐式为该次叠层运行启用持久化，拒绝目录／符号链接／特殊文件，并且不会创建缺失的父目录。运行中的后台进程可用命名目标启动隐藏的叠层；叠层已经可见时，必须先隐藏再切换命名会话。后台进程运行期间，其叠层在隐藏和显示之间保留上次打开或另存为的会话
- 叠层的会话控件位于顶部工具栏的会话弹出菜单（溢出菜单 → 会话… (Session...)）。可以打开已有命名会话、将当前叠层另存为其他命名会话、返回主会话（**返回 (Back to)** 启动时的会话文件，或 **默认会话 (Default session)**）、显示会话信息、清除活动会话、重新打开最近的命名会话，以及跳转到配置器。打开／另存为对话框使用 `zenity` 或 `kdialog`；没有扩展名时，另存为会添加 `.wayscriber-session`，替换已有会话文件前会先询问
- 配置器的会话标签页管理从 CLI、后台或叠层打开／保存命名目标时记录的最近命名会话。可重命名目录标签、显示文件位置，或仅遗忘元数据而不影响文件。清空工具状态 (Clear Tool State) 仅删除保存的工具层，清空已存数据 (Clear Saved Data) 删除会话文件。叠层、手动启动的后台进程或后台服务运行期间，创建副本 (Duplicate)、移动 (Move)、清空工具状态 和 清空 均禁用

</details>

### 绘图板与触笔支持

默认构建包含绘图板支持（`zwp_tablet_v2`），运行时也默认启用：

```toml
[tablet]
enabled = true
pressure_enabled = true
min_thickness = 1.0
max_thickness = 8.0
```


压力控制支持压力感应的自由画笔线宽。马克笔／Textmarker、步骤标记和形状工具在使用触笔时仍保持选定的大小。

默认构建可直接使用绘图板输入。可在 `config.toml` 中设置 `[tablet].enabled = false` 关闭。若要构建不含绘图板支持的版本，请仅去掉这一项特性（单独使用 `--no-default-features` 还会同时移除门户捕获、托盘和 GTK 工具栏）：`cargo build --release --no-default-features --features portal,tray,toolbar-gtk`。

### 界面语言

Wayscriber 提供英文和简体中文界面，默认跟随桌面区域设置，并在没有对应翻译时回退到英文。在配置器中选择语言（**UI → 语言 (Language)**，会立即预览，保存后保留），或在 `config.toml` 中设置：

```toml
[ui]
language = "zh-CN"   # "auto" (default), "en", or "zh-CN"
```


叠层和关于窗口在下次打开时使用已保存的语言，后台模式则在重启后使用。如果仅为本次运行选择语言，不修改 `config.toml`，或查看当前生效的语言：

列出可选语言，以及当前生效的语言及其原因：

```bash
wayscriber --list-languages
```

仅为本次叠层使用某种语言：

```bash
wayscriber --active --language zh-CN
```

为后台模式及其打开的所有叠层使用某种语言：

```bash
wayscriber --daemon --language en
```


`--language` 也适用于 `--freeze` 和 `--about`，其优先级高于 `[ui] language` 和 `WAYSCRIBER_LANGUAGE`。您输入的文本、画板名称以及其他自建内容不会翻译。翻译、本地翻译文件覆盖和添加语言的说明见 [docs/I18N.md](docs/I18N.md)（英文）。

## 故障排查

### 重启后后台进程未启动

已启用的用户服务在登录时启动。如果重启并登录后仍无后台进程，请参阅[服务无法启动](#服务无法启动)。只有希望在登录前启动，或注销后保持运行时，才需启用 lingering：

```bash
loginctl enable-linger $USER
```


也可以改用合成器自动启动。Hyprland Lua：在 `~/.config/hypr/autostart.lua` 中添加：

```lua
o.launch_on_start("wayscriber --daemon")
```


传统 Hyprland 配置：

```conf
exec-once = wayscriber --daemon
```


### 服务无法启动

查看状态：

```bash
systemctl --user status wayscriber.service
```

跟踪日志：

```bash
journalctl --user -u wayscriber.service -f
```

重启服务：

```bash
systemctl --user restart wayscriber.service
```


### KDE Plasma 上的叠层模糊或不透明

**原因：** "Better Blur DX" 或类似模糊特效可能会模糊 wayscriber 的透明叠层。

**解决方式一：配置 Better Blur DX：**
1. 打开 **系统设置 (System Settings)** → **窗口管理 (Window Management)** → **桌面特效 (Desktop Effects)**
2. 点击 "Better Blur DX" 旁的配置按钮
3. 进入 **Force Blur** 标签页
4. 将 `wayscriber` 加入窗口类列表
5. 确保选中 `Blur all except matching`
6. 点击 **应用 (Apply)**

**解决方式二：使用标准模糊：**
1. 在 **桌面特效 (Desktop Effects)** 中禁用 "Better Blur DX"
2. 改用标准 "Blur" 特效

如需查找 wayscriber 的窗口类，请运行下列命令，并在 2 秒等待结束前启动 wayscriber：

```bash
sleep 2; qdbus org.kde.KWin /KWin queryWindowInfo | grep resourceClass
```


### 叠层未出现

1. 检查 Wayland 会话：`echo $WAYLAND_DISPLAY`
2. 确认合成器支持 `wlr-layer-shell`
3. 启用日志运行：`RUST_LOG=info wayscriber --active`

### 配置问题

```bash
ls -la ~/.config/wayscriber/config.toml
RUST_LOG=info wayscriber --active   # watch for TOML errors
```


### 冻结与缩放捕获日志

`--active` 和 `--daemon` 将带日期的日志写入 `$XDG_DATA_HOME/wayscriber/logs/`，通常是 `~/.local/share/wayscriber/logs/`。后台启动的叠层也写入此处，不必转发其标准输出或标准错误。`WAYSCRIBER_LOG_FILE` 可覆盖日志目标。

在默认的 `info` 级别，`capture.output` 记录 surface 进入／离开事件以及先前和当前的输出 ID 与名称。`capture.preflight` 记录冻结／缩放、屏障 ID、保存的和当前的输出／布局令牌，以及重试决定。显示器元数据更新可在 `debug` 级别查看。

要获取完整输出元数据和变化前后的几何信息，请在后台进程的启动环境中设置 `RUST_LOG=info,wayscriber::capture_diagnostics=trace`。更改环境后重启后台进程，让新启动的叠层继承该过滤设置。

### 环境变量

常用开关：
- `WAYSCRIBER_TOOLBAR_DRAG_PREVIEW=0` 禁用内嵌工具栏拖动预览（默认开启）
- `WAYSCRIBER_TOOLBAR_POINTER_LOCK=1` 启用指针锁定拖动路径（默认开启）
- `WAYSCRIBER_TOOLBAR_DRAG_THROTTLE_MS=12` 限制工具栏拖动更新频率（默认 12；设为 0 禁用）
- `WAYSCRIBER_DEBUG_TOOLBAR_DRAG=1` 启用工具栏拖动日志（默认关闭）
- `WAYSCRIBER_DEBUG_TOOLBAR_COLOR=1` 启用工具栏颜色选择器日志（默认关闭）
- `WAYSCRIBER_FORCE_INLINE_TOOLBARS=1` 在 Wayland 上强制使用内嵌工具栏（默认关闭）
- `WAYSCRIBER_TOOLBAR_BACKEND=auto|gtk|builtin` 覆盖工具栏前端（默认 auto）
- `WAYSCRIBER_NO_TRAY=1` 禁用托盘图标（默认启用）
- `WAYSCRIBER_TRAY_FORCE_PIXMAP=1` 强制使用彩色托盘像素图，并覆盖 `[tray].icon_style`
- `WAYSCRIBER_LANGUAGE=zh-CN` 为本次运行选择界面语言（覆盖 `[ui] language`，`--language` 又可覆盖它）

完整列表见 `docs/CONFIG.md`（英文）。

### 性能调优

默认行为优先降低绘图延迟：禁用垂直同步，并限制无垂直同步时的渲染帧率：

```toml
[performance]
buffer_count = 3
enable_vsync = false
max_fps_no_vsync = 120
ui_animation_fps = 30
```


对常见系统，`120` 是较合适的低延迟上限。如果显示器匹配且机器能够承担额外渲染工作，可尝试 `144`、`165`、`240` 或更高。`max_fps_no_vsync = 0` 仅建议用于性能分析，因为无限帧率可能使 CPU/GPU 高负载运转。

如果无撕裂显示、较低功耗或安静运行比输入延迟更重要，请设置 `enable_vsync = true`。垂直同步通常会带来由帧节奏决定的延迟下限，尤其在 60 Hz 显示器上；禁用它能降低输入延迟，但可能出现撕裂，并增加 CPU/GPU 使用量。

## 参与贡献

贡献原则：wayscriber 以礼物交换的方式分享，而不是合同。欢迎提出请求，但不保证完成时间，也不承担支持义务。完整原则见 https://wayscriber.com/docs/ethos/gift-exchange.html （英文）。

开发环境设置、项目结构和工作流程说明见 [CONTRIBUTING.md](CONTRIBUTING.md)（英文）。

未来计划在 [GitHub issues](https://github.com/devmobasa/wayscriber/issues) 中跟踪。

## 许可证与致谢

**MIT 许可证**。参阅 [LICENSE](LICENSE)。

### 致谢

- 灵感来自 Mark Russinovich 的 [ZoomIt](https://learn.microsoft.com/en-us/sysinternals/downloads/zoomit)
- 为 Linux（使用 Wayland 的发行版）构建
- 从 [Gromit-MPX](https://github.com/bk138/gromit-mpx) 获得相近思路
- 使用 [Cairo](https://www.cairographics.org/) 和 [smithay-client-toolkit](https://github.com/Smithay/client-toolkit)

在 AI（ChatGPT、Codex、Claude Code）辅助下开发。
