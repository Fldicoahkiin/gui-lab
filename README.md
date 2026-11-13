# lab（名字未定）

一个自绘界面框架。Rust + wgpu，同一份代码跑原生和网页（WebGPU，没有时用 WebGL2）。设计见 `docs/DESIGN.md`。

现在完成到第 2 步（骨架），加上图片的记号、像素字和作为数据的主题与材质：

- 界面是一棵由普通值组成的树（`src/el.rs`）。视图是 `fn(&状态) -> 树`，更新是 `fn(&mut 状态, 消息)`。
- 每次更新用 taffy 重新布局；文字由 parley 排版，字号任意，不要求等宽。每个节点得到绝对矩形和裁剪矩形。
- 运行时（`src/ui.rs`）不依赖显卡：命中、悬停、焦点（Tab / Shift-Tab 轮转，Enter 和空格触发）、滚轮滚动、每个实例的出生时间。悬停和焦点由四个角标框出，角标在目标之间平滑移动。
- 屏幕上的东西都是实例：字形、形状（填充或边框）、图片，画两遍。第一遍画进两张中间图（「这里有什么」和「离最近的墨有多近」，按真实轮廓）；第二遍由材质着色器决定最终像素。波、凝结、透镜的参数全部在 `Fx` 里，可以在界面上精确设置。
- 图片松开时是图片自己变成记号（设计文档 §1.6.1）：先变成无缝的色块，再在每一格里按亮度换记号。记号表有十二张（字符、抖动、像素画、马赛克、积木、圆点、十字、斜线、菱形、横线、光点、笔触），启动时在 CPU 上生成；字符按实际量出的墨量排序。可以限色（六种调色板，含两色和三色）、浮雕、分区、让记号顺着图像的形体转、按指针做视差、发光。每张图片选八组参数里的一组，每个参数都是精确的数字。记号之间露出的是图片自己的暗部或黑，从来不是页面底色。
- 对比擦除：图片上一条可以拖动的竖线，一侧是原图，一侧是完全松开的样子。
- 像素字：Fusion Pixel 12 像素（中文）和 Departure Mono 11 像素（西文），只按整数倍字号、整像素位置、不抗锯齿地画，边缘只有墨色和底色。任何字体也可以通过材质印成像素（`.print(点距)`）。
- 主题是数据（颜色、三种字体角色、字号阶、线宽），有暗、明、像素三个；材质有 `plain` 和 `dot` 两个，运行时切换。
- 组件是普通函数（`src/widgets/`）：按钮、开关、数字控件。数字控件左右有步进按钮；获得焦点后可以直接键入数字、`.`、`-`，退格删除，回车确认，Esc 取消；上下方向键按步长改，按住 Shift 乘 10。
- 演示（`src/demo.rs`）全部用框架本身写成：计数卡片、程序生成的图（球、天空、地形、胸像）、中英文混排的段落、一个可以滚动的列表、图片的记号（带对比线和全部参数）、像素字和普通字体的对照、一组参数和主题、材质的切换。窗口窄时参数面板排到内容下方，整页可以滚动。

## 运行

测试（不需要显卡）：

```bash
cargo test
```

离屏渲染一组画面到 PNG（需要一个 Vulkan 驱动，没有显卡时可用 lavapipe）。参数是输出目录、宽、高（设备像素）和缩放：

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

打开 `dist/index.html`；`dist/artifact.html` 是同样内容、不带外壳标签的单文件。网页只有一个 canvas，负责把指针、滚轮、键盘和尺寸变化转给框架。

## 还没有

输入框和输入法、无障碍镜像、其余组件和 gallery、标注层、`plain` 和 `dot` 以外的材质、风格预设。数字控件上横向拖动调值也还没做。

## 字体

都是 SIL Open Font License 1.1，许可证在 `assets/licenses/`：

- Noto Sans SC。
- Departure Mono 1.500（Helena Zhang），取自它的发布包。
- Fusion Pixel 12px 比例宽度简体中文版（TakWolf，上游版本 2026.08.11），由 npm 包 `@vp-tw/cjk-web-fonts-fusion-pixel-font` 里按 Unicode 区块切开的 WOFF2 合并回一个 OTF（这里连不上 GitHub 的发布下载）。它包含的上游字体 Ark Pixel、Cubic 11、Galmuri 的许可证也一并收了。
