# lab（名字未定）

一个自绘界面框架。Rust + wgpu，同一份代码跑原生和网页（WebGPU，没有时用 WebGL2）。设计见 `docs/DESIGN.md`。

现在完成到第 2 步（骨架）：

- 界面是一棵由普通值组成的树（`src/el.rs`）。视图是 `fn(&状态) -> 树`，更新是 `fn(&mut 状态, 消息)`。
- 每次更新用 taffy 重新布局；文字由 parley 排版，字号任意，不要求等宽。每个节点得到绝对矩形和裁剪矩形。
- 运行时（`src/ui.rs`）不依赖显卡：命中、悬停、焦点（Tab / Shift-Tab 轮转，Enter 和空格触发）、滚轮滚动、每个实例的出生时间。悬停和焦点由四个角标框出，角标在目标之间平滑移动。
- 屏幕上的东西都是实例：字形、形状（填充或边框）、图片，画两遍。第一遍画进两张中间图（「这里有什么」和「离最近的墨有多近」，按真实轮廓）；第二遍由材质着色器决定最终像素。波、凝结、透镜的参数全部在 `Fx` 里，可以在界面上精确设置。
- 图片松开时先变成无缝的色块，再在每一块里画点或方块，块与块之间露出的是这块颜色的暗部，不是页面底色。
- 组件是普通函数（`src/widgets/`）：按钮、开关、数字控件。数字控件左右有步进按钮；获得焦点后可以直接键入数字、`.`、`-`，退格删除，回车确认，Esc 取消；上下方向键按步长改，按住 Shift 乘 10。
- 演示（`src/demo.rs`）全部用框架本身写成：计数卡片、一张程序生成的图、中英文混排的段落、一个可以滚动的列表、一组参数。窗口窄时参数面板排到内容下方，整页可以滚动。

## 运行

测试（不需要显卡）：

```bash
cargo test
```

离屏渲染一组画面到 PNG（需要一个 Vulkan 驱动，没有显卡时可用 lavapipe）。参数是输出目录、宽、高（设备像素）和缩放：

```bash
cargo run --release --bin shot -- shots 1280 900 1
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

输入框和输入法、无障碍镜像、其余组件和 gallery、标注层、其他材质、记号表（见设计文档 §1.6.1）。数字控件上横向拖动调值也还没做。

字体是 Noto Sans SC（SIL Open Font License）。
