# gui-lab（工作名）

一个自绘界面框架。Rust + wgpu，同一份代码跑原生和网页（WebGPU，没有时用 WebGL2）。设计见 `docs/DESIGN.md`。

现在完成到第 3 步（组件和 gallery）：

- 界面是一棵由普通值组成的树（`src/el.rs`）。视图是 `fn(&状态) -> 树`，更新是 `fn(&mut 状态, 消息)`。
- 每次更新用 taffy 重新布局；文字由 parley 排版，字号任意，不要求等宽。每个节点得到绝对矩形和裁剪矩形。
- 运行时（`src/ui.rs`）不依赖显卡：命中、悬停、焦点（Tab / Shift-Tab 轮转，Enter 和空格触发）、滚轮滚动、每个实例的出生时间。悬停和焦点由四个角标框出，角标在目标之间平滑移动。
- 屏幕上的东西都是实例：字形、形状（填充或边框）、图片，画两遍。第一遍画进两张中间图（「这里有什么」和「离最近的墨有多近」，按真实轮廓）；第二遍由材质着色器决定最终像素。波、凝结、透镜的参数全部在 `Fx` 里，可以在界面上精确设置。
- 图片松开时是图片自己变成记号（设计文档 §1.6.1）：先变成无缝的色块，再在每一格里按亮度换记号。记号表有十二张（字符、抖动、像素画、马赛克、积木、圆点、十字、斜线、菱形、横线、光点、笔触），启动时在 CPU 上生成；字符按实际量出的墨量排序。可以限色（六种调色板，含两色和三色）、浮雕、分区、让记号顺着图像的形体转、按指针做视差、发光。每张图片选八组参数里的一组，每个参数都是精确的数字。记号之间露出的是图片自己的暗部或黑，从来不是页面底色。
- 对比擦除：图片上一条可以拖动的竖线，一侧是原图，一侧是完全松开的样子。
- 像素字：Fusion Pixel 12 像素（中文）和 Departure Mono 11 像素（西文），只按整数倍字号、整像素位置、不抗锯齿地画，边缘只有墨色和底色。任何字体也可以通过材质印成像素（`.print(点距)`）。
- 主题是数据（颜色、三种字体角色、字号阶、线宽），有暗、明、像素三个；材质有 `plain` 和 `dot` 两个，运行时切换。
- 字符记号按显示的点距绘制：每个设备点距（4 到 40 像素）各切一套字符块，直接按像素读，点距 8 到 10 时字符仍然清楚；每个点距按量出的墨量挑 16 个字符，暗处落到黑底。
- 组件是普通函数，每个一个文件（`src/widgets/`），以源码分发，`registry.json` 列出文件和依赖：按钮（默认、安静、强调，以及不可用）、数字控件、输入框、开关、滑块、标签页、选项（`choice` 和分段的 `segmented`）、列表、卡片、对话框、图片、分隔线、滚动区。
  - 数字控件：获得焦点后直接键入，回车确认，Esc 取消；上下方向键按步长改，Shift 乘 10；在数值上横向拖动可以连续调（每 6 像素一步）。
  - 输入框：插入可打印字符，退格和 Delete 删除，左右、Home、End 移动插入点。没有选区和输入法。
  - 对话框：打开时只有它响应操作，焦点留在里面，下面的页面保持部分松开；Esc 或关闭按钮关闭，焦点回到打开它的按钮。
- 运行时加了：拖动时捕获指针、文字插入点、对话框那种层、不可用状态、横向滚动和滚动位置细条、按键后把选中的行滚进视野、提交（切主题和材质时保留旧画面，新旧两种状态以波前为界）。
- Gallery（`src/gallery/`）是主应用：顶栏切主题、材质、恢复默认；左栏分组件、记号、排版三组（窄窗口时变成一行可以横向滚动的标签）；每个组件一页，有预览、全部变体、它自己的参数和源码；每张记号表一页；一页字号与字体。第 2 步的演示（`src/demo.rs`）是其中一页。网页版的每一页有自己的地址（`#button`、`#marks-chars`…）。

## 运行

测试（不需要显卡）：

```bash
cargo test
```

Gallery 的每一页在每个主题、每种材质下各出一张 PNG（文件名是 `页名-主题-材质.png`），另有对话框打开、切主题的中途、换页、输入、窄窗口几种状态（需要一个 Vulkan 驱动，没有显卡时可用 lavapipe）。参数是输出目录、宽、高（设备像素）和缩放：

```bash
cargo run --release --bin pages -- pages 1280 1800 1
```

第 2 步演示的一组画面：

```bash
cargo run --release --bin shot -- shots 1280 900 1
```

读回像素检查设计文档 §1.6.1 和 §1.6.2 的验收（浅色主题下完全松开的图片里没有页面底色；点距 1 时等于原图；像素字边缘只有两种颜色），失败时退出码不为 0：

```bash
cargo run --release --bin check
```

每张记号表、每种调色板、浮雕、分区、方向跟随、辉光、视差和对比擦除各出一张样张，另有记号图集本身和原图：

```bash
cargo run --release --bin sheet -- sheets
```

原生窗口（winit；macOS 上用 Metal，其他系统用 Vulkan）。可以带一个页名，直接打开那一页。在这里的 Linux 容器里只编译过、没有显示器可以运行；macOS 上只做了类型检查（`cargo check --target aarch64-apple-darwin`），没有实际运行过：

```bash
cargo run --release --bin window -- dialog
```

网页版（需要 `wasm32-unknown-unknown` 目标、同版本的 `wasm-bindgen-cli`、Python 的 fonttools）：

```bash
cargo build --release --lib --target wasm32-unknown-unknown
```

```bash
wasm-bindgen --target no-modules --no-typescript --remove-name-section --out-dir dist/pkg target/wasm32-unknown-unknown/release/lab.wasm
```

```bash
python3 web/build.py
```

打开 `dist/index.html`（可以带页名，比如 `dist/index.html#dialog`）；`dist/artifact.html` 是同样内容、不带外壳标签的单文件。网页只有一个 canvas，负责把指针、滚轮、键盘、尺寸变化和地址里的页名转给框架。

## 还没有

输入法和输入框里的选区、无障碍镜像、换页时共有部分的流动、标注层、`plain` 和 `dot` 以外的材质、风格预设、展示场景。和设计文档不一样的地方记在 `docs/DESIGN.md` §9。

## 字体

都是 SIL Open Font License 1.1，许可证在 `assets/licenses/`：

- Noto Sans SC。
- Departure Mono 1.500（Helena Zhang），取自它的发布包。
- Fusion Pixel 12px 比例宽度简体中文版（TakWolf，上游版本 2026.08.11），由 npm 包 `@vp-tw/cjk-web-fonts-fusion-pixel-font` 里按 Unicode 区块切开的 WOFF2 合并回一个 OTF（这里连不上 GitHub 的发布下载）。它包含的上游字体 Ark Pixel、Cubic 11、Galmuri 的许可证也一并收了。
