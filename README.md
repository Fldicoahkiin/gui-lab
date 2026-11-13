# lab（名字未定）

一个自绘界面框架的第一条竖线。Rust + wgpu，同一份代码跑原生和网页。

现在只证明一件事：字、图和背景之间没有方框。

- 文字是非等宽的，由 cosmic-text 排版；排完后每个字形是一个独立的实例。
- 画两遍。第一遍把所有实例原样画进两张中间图：一张是「这里有什么」，一张是「这里离最近的笔画有多近」（按字形的真实轮廓）。第二遍由一个材质着色器读这两张图和波，决定最终的像素。
- 波扫过时，字和图松开成点阵上的点或方块，再合回来；每个字形也会被波推着动一下。
- 指针是图片上的一面透镜，不影响文字。
- 背景的点按笔画的真实形状避开字。
- 点距、不规则程度、错位强度都是参数。

## 运行

离屏渲染几帧到 PNG（需要一个 Vulkan 驱动，没有显卡时可用 lavapipe）：

```bash
cargo run --bin shot -- shots 1200 900 1
```

网页版（需要 `wasm32-unknown-unknown` 目标、同版本的 `wasm-bindgen-cli`、Python 的 fonttools）：

```bash
cargo build --release --lib --target wasm32-unknown-unknown
```

```bash
wasm-bindgen --target no-modules --no-typescript --out-dir dist/pkg target/wasm32-unknown-unknown/release/lab.wasm
```

```bash
python3 web/build.py
```

打开 `dist/index.html`。

## 还不是框架

没有界面树、布局引擎、组件、滚动、输入框、无障碍。页面内容写死在 `src/app.rs` 里，网页版底部的滑块是普通网页控件，临时用。

字体是 Noto Sans SC（SIL Open Font License）。
