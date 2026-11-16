# 更新记录 · Changelog

按 DESIGN.md §7 的步骤记。Each step of DESIGN.md §7.

## 14 — 交互的手艺 · Interaction craft

- 手势的物理：弹簧（解析步，松手速度交接）、橡皮筋、投射、速度（`lab::gesture`）。Gesture physics: a spring with velocity hand-off, a rubber band, projection, velocity.
- 底部面板（几档、按投射收起、橡皮筋、背板、滚动交接）和浮动窗口（按投射选角）。A bottom sheet and a floating box.
- 公式注：演示下面印出真实参数的一行，参数可改。The formula caption.
- `el::layers`：叠在同一个盒子里的孩子。Children layered in one box.
- 骨架：`El::skeleton()` 用真实的树画骨架（条是量出来的行宽），`Loading` 的两个数，零位移；四种加载方式的比较。Skeletons from the real tree, the loading rules, zero layout shift.
- 多级菜单：安全区三角形、两个延迟、键盘、可画出的三角形（`El::on_move`、`El::guide`）。Nested menus with a safe-zone triangle.

## 11 — 序章、展示而不是标语、引导、文字绕排、读数、配方 · The prologue, display not slogans, guidance, wrap, readout, recipes

- 影片的序章：同一个小界面左边原样、右边一种材质，分界线能拖，材质名一排。The prologue: one small interface, as it is and in a material, a divider and chips.
- 不再有标语：章名是一个元素，换章时一个字一个字地重组；第 9、10、11 章重做。No slogan: the chapter's title recomposes across cuts; chapters 9, 10, 11 redone.
- 底栏的引导：上一章 / 下一章、自动播放的进度、只到第一次输入为止的提示、括号提示、看不见的边。Guidance in the bottom bar and on the stage.
- 文字绕排按真实的距离、不留碎片、拖动时不跳；正文不会一个字一个字地凝聚；330 px 宽时没有文字被截掉。Wrap by true distance, no slivers, no hopping; text condenses as a whole; nothing cut at 330 px.
- F2 读数：一行和全部两种，带火花线，空闲时不要帧。The F2 readout: one line or every number, a sparkline, no frames at rest.
- `look:v1:` 配方（网址、PNG 的文本块）；八种新记号，按家族分组；Geist Pixel 的五种字面；`<lab-view>` 自定义元素。Recipes; eight new mark sets; Geist Pixel; the `<lab-view>` element.

## 10 — 滚动、标志与机件、多语言、组件与文档 · Scrolling, the sign and the mechanism, languages, components and docs

- 慢速滚动是整像素移动的图，快速滚动每种材质有自己的痕迹（§1.8.3，`check` 第 18 条）。Slow scrolling is a picture moved by whole pixels; fast, each material leaves its own trace.
- 影片的第三个演员是平面的标志；第 13 章是真正三维的机件（齿轮、凸轮、从动杆）。The film's third actor is a flat sign; chapter 13 is a real 3D mechanism.
- 消息目录、`t!`、复数、回退链、从右往左的行（§1.20）。Message catalogs, `t!`, plurals, a fallback chain, right-to-left rows.
- 十八个新组件、`lab add`、按 shadcn/ui 的样子组织的文档站。Eighteen new components, `lab add`, a documentation site laid out after shadcn/ui.

## 9 — 修正和影片 · Fixes and the film
重组有界、网格表面发射粒子、局部的时钟、十二章的影片。Bounded recomposition, particles from mesh surfaces, local clocks, the twelve-chapter film.

## 8 — 三维、GPU 粒子、只画变了的部分、预设和过渡 · 3D, GPU particles, dirty boxes, presets and transitions

## 7 — 涟漪规则、内容重组、给模型用的一面 · Ripple rules, recomposition, the model-facing surface

## 6 — 交互和性能 · Interaction and performance
只在有变化时画、帧时间读数、文字输入、增量布局和长列表、触摸、无障碍镜像。

## 5 — 标注层、全部材质、十二个预设、展示场景 · Annotation, every material, twelve presets, scenes

## 4 — 变换：粒子和流动 · Transforms: particles and flows

## 3 — 组件和 gallery · Components and the gallery

## 2.5 — 图片的记号、像素字、主题和材质作为数据 · Marks, pixel type, themes and materials as data

## 2 — 骨架 · The skeleton
界面树、taffy、parley、滚动、点击、焦点、数字控件。

## 1 — 竖线 · The vertical slice
两遍绘制、按轮廓走的波、原生和网页同一份代码。
