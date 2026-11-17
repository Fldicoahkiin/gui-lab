# 更新记录 · Changelog

新的在上面，各部分的验收见 DESIGN.md §7。Newest first; acceptance for each part is in DESIGN.md §7.

## 场图和尾迹、线、小字、Loose Thread · The field map and trails, the line, small type, Loose Thread

- 形体这一页只用英文，不论界面是什么语言；语言标记写 `LANGUAGE  EN / ZH`，按下换的是 gallery 其余页的语言。Body is English only.
- 场图（`src/fieldmap.rs`）：整窗一张粗网格，每个结点有能量和流向，固定 1/60 秒一步，摊开、按寿命衰减、蜿蜒（`meander`）、沿着墨爬（`creep`），有限时间里正好回到 0，静止时不要帧。The field map: energy and flow on a coarse grid, back to exactly zero within a bound.
- 尾迹：`El::stirr` 的元素动了就沿走过的路搅一下，`Cmd::Stir` 可以搅一点、一个元素或一条路径的一段；浮动窗口、底部面板、卡片叠默认带尾迹，强弱由预设定（Normcore 没有）。Trails: `El::stirr` and `Cmd::Stir`, on by default for the floating window, the sheet and the card stack.
- 读场图：第一遍里它是一份松开度（照样受可读上限和响应管），会被推的实例顺着流向漂开（图片和三维元素在原地松开）；dot 的底点和 glitch 的底读它；`Ui::looseness` 用同一个值。留尾迹而有底色的元素画在上面一层，经过的松开的装饰不会透过它。Reading the map in both passes and on the CPU.
- 页面 `#stirr`（概念一组），十二个数都是数字控件；F2 读数加一行；`check` 第 19 条（原来的第 19 条“效果”改为第 17 条）、`bench … map`、`craft <目录> stirr`。The `#stirr` page, check 19, a bench row and samples.
- 线的材质 `scribblr`（`src/strand.rs`）：静止时和 plain 逐字节相同；松开时填充淡下去，墨变成一根连续的细线，先沿字的真实轮廓走、用桥连成一笔（干净的空心字），再松就一步步去掉顶点，合成跨几个字的长折线、各自游走开，能量最高的一到三处缠成星形的大结，两根长线头一根从范围的边进来、一根走到另一条边，收回时倒着来。要读的字和控件不出线、不淡、不动。八个数在材质参数里（`reach` 40、`knot_r` 48、`tail` 480、`angular` 0.9），`Look::hold_ms` 让线一拍一换姿势。The `scribblr` material: loose ink becomes one wandering polyline with a few big knots and two long tails, and writes itself back; text to read holds.
- 长路径 `El::trace`：按进度画出的长折线，可以是虚线（`dash`、`gap`，按弧长从起点算），被流向推歪，头经过处搅动场图；`#stirr` 页的“Draw a line”。The long-path element with dashes.
- 线从第一遍画出来的轮廓起头（加上场图推开实例的那一段）；顶点按一遍 Ramer–Douglas–Peucker 排好，滑到弦上再去掉，跟着弦走，收回时没有跳点；结是穿过中心的长弦；线头出范围时切在边上，不进要读的字。`strand::figure` 按固定种子画一个线团给页面当内容。The thread starts on the outline as drawn, loses vertices onto chords without jumps; knots are stars of chords; tails stop at the scope's edge and before text to read; `strand::figure`.
- 页面 `#material-scribblr`；`check` 第 20 条、`bench … strand`、`craft <目录> scribblr`、`tests/strand.rs`。The page, check 20, a bench row, samples and tests.
- 字距 `El::tracking`；小声的字 `El::whisper()`：小写、字距 0.6 em、衬线体（Source Serif 4 的 8 pt 光学尺寸，只留西文，82 KB，OFL）。Tracking and `El::whisper()` in a small serif.
- 标注层的三种新读数：边注 `EDGE`（范围右边的细线、刻度、指针记号，竖排写着范围大小、指针坐标、材质和主题、最松的元素、场图的结点数）、波形 `OSC`（一行结点的能量，一个设备像素粗的折线）、电平条 `LEVEL`（平均能量）；标注层认实例的变换，只用来把边注转 90 度；导出的树多了 `field: { level, nonzero }`。Edge notes, a waveform and a level bar read the runtime and the field map.
- 第十个预设 Loose Thread：黑底白线（或纸白），一种强调色，`scribblr`，悬停时只有边注和角标，场图蜿蜒、沿墨爬、两秒的寿命（指针扫过的标题松到 0.8 以上，慢慢收回，三秒内回到 0），线一拍二，700 ms 的过渡先松成线再画轮廓；风格页只用英文：小写的衬线标题、一张图、一句正文、页脚一行小字的三个控件，静止时有一个画好的线团。The tenth preset, Loose Thread, and its page: English only, a lowercase serif title, controls as a line of small type, a drawn tangle at rest.
- 收尾的一遍加了“只用黑白”（`binary`：黑白调色板、不抖动、线宽取整；字、线和标注按覆盖率取阈值，不看颜色多亮，图片是两级的有序抖动）和“翻转”（`invert`：最后 `1 − 颜色`）两个开关。Black and white only (thresholds by coverage, pictures dithered), and invert, in the finishing pass.
- 标注的角标在元素矩形外 6 像素，不压元素自己的字。Corner marks sit 6 px outside the rect, off the ink.
- 修了 `THEME_IDS` 和风格页说明里删掉的主题、风格留下的错位。Fixed theme ids and style descriptions that had drifted out of line.
- `check` 第 7.2、7.3 条（边注、波形、电平条、1080p 的边注）、第 20.6、20.7 条（只用黑白、翻转），`tests/gaze.rs` 的三个测试，`tests/i18n.rs` 的 Loose Thread 页，`craft <目录> loose-thread`、`craft <目录> thread-ref`。Checks, tests and samples.

## 基础的收尾、实验室的墙、形体 · Foundation leftovers, the Lab as a wall, Body

- 取样看变换：`morph::cover_at` 把点按实例自己的变换反算回盒子里，`cells`、`ink_bounds` 用转过的外接框，摆了姿态、转过的字从它画在的地方松开，遮罩和解码也在那里。Sampling honours `xf` and glyph poses.
- 角标在指针到的那一帧就落在目标上（原来每帧追过去，总落后一段滑行），只有 70 ms 的收拢和淡入。Hover and focus marks land in the pointer's frame.
- 透镜按第一性原理只碰可以松开、没有被指针操作的图片和三维元素，像波前一样在第一遍按实例加；松开度只属于实例自己的墨（墨压过别人的近度圈），不再漏到旁边的装饰上。The lens touches only pictures and 3D elements that may loosen and are not being operated; looseness no longer spills onto neighbours.
- 浮动窗口拖动、甩出、滑到角上加进了 `check` 的部分重画对比（每种材质，两种 2 倍缩放）。The floating box in the partial-redraw check.
- 解码按轮廓的覆盖算、不看颜色：用底色填的对话框里面在最细一档也被盖住。Ground-toned fills count as coverage in decode.
- DESIGN.md 里删掉了已经删除的材质（金属、液体、拼贴）的旧说法。Old mentions of removed materials cleaned up.
- 实验室：舞台占格子的七成以上、作品填满，公式一行，数字收进 `[ params ]`；看得见的格子一直走、滚出去停（减少动态效果时照旧静止）；每格都重做到填满舞台、指针下出现那种蓝；解码的分层视图改成斜看的平面、各带引线和标签。The Lab as a wall of living pieces.
- 删掉了影片（序章和各章、场记板、时间线、前后对比、各章的公式注）、透视材质（xray）和它的预设（Transparent Electronics），以及只为它们存在的测试、检查、文档和文案；拆开场景不再用 xray。The film, the xray material and its preset are gone.
- 形体（`#body`）是新的落地页，铺满窗口：一页能读的文字先出现，再自己聚成由它自己的字（皮，字数守恒）和一万多个体积记号组成、慢慢转动的发光形体，一盏主光、边光、雾、辉光、地上的倒影和光晕、三层灰尘、视差和镜头的呼吸；画面里的小标记（形、种子、聚拢 / 散开、语言、实验室、组件库）；指针处开口读到原文。Body is the landing page, full-bleed, as a picture.
- 辉光的采样按像素转一个噪声角度，不再出现文字的重影；`Pose::hue` 让一个字换强调色。Bloom without ghost copies; a glyph's hue in its pose.

## 场、实验室、解码过渡、范围缩小、帧预算 · Fields, the Lab, decode, a narrower scope, the frame budget

- 实例自己的变换（`Instance::xf`：转、缩放、斜切，绕自己的中心），字形的姿态（`El::pose`），所以字可以沿路径、包在球上、被光照亮，轮廓不变。Per-instance transforms and glyph poses.
- 图（`el::canvas(Drawing)`：点和笔画都是有真实轮廓的形状），`El::on_point`（移动、按下拖动、离开一个处理函数，画布上是画布的坐标）。Drawings and one pointer handler.
- 场（`amenui::field`：噪声、鼓包、元球、旋度、网格、marching squares 等值线、矢量流、点群平流、有遮罩的 Gray–Scott）。Fields as one abstraction.
- `El::on_visible`（看得见才走）和 `El::on_outline`（把真实轮廓作为遮罩交给状态）。Visibility and true outlines as messages.
- 帧预算（`amenui::budget`）：变形格子、掉落、流场、点群、图的记号都从一份里取，超时自动缩、有余量时恢复；读数里写出。One frame budget for every small mark.
- 实验室（`#lab`）：十格实验（光照文字、等值线、丝、长成形状、水银、沿路径的文字、球面上的文字、轨道、弹簧、卡片叠），每格一个示例文件，各有一页文档。The Lab: ten experiments, one file each, with their pages.
- 解码过渡（`El::decode`）：按真实轮廓由粗到细解开，倒放消失；文档页有分层的三维视图和时间轴。The decode transition with an exploded view and a timeline.
- 删掉了金属、液体、拼贴三种材质，Neo-Y2K 和 Mixed media 两个预设，影片第 11、12 章（连同终章）。Removed chrome, liquid and collage, two presets and two film chapters.

## 交互的手艺 · Interaction craft

- 手势的物理：弹簧（解析步，松手速度交接）、橡皮筋、投射、速度（`amenui::gesture`）。Gesture physics: a spring with velocity hand-off, a rubber band, projection, velocity.
- 底部面板（几档、按投射收起、橡皮筋、背板、滚动交接）和浮动窗口（按投射选角）。A bottom sheet and a floating box.
- 公式注：演示下面印出真实参数的一行，参数可改。The formula caption.
- `el::layers`：叠在同一个盒子里的孩子。Children layered in one box.
- 英文绕排：断词（Liang，en-US）和至多 0.08 em 的字距，不对齐的段降到 15% 以下。English wrap: hyphenation and a little letter spacing.
- 点阵图表：点的大小和亮度是曲线下的面积，扫过的竖线，可拖的控制点。A dot chart.
- 第 6 章的墨粉减半、避开文字（`Shed.clear`），一行公式注。The zine's toner halved and kept off the text.
- 卡片叠：angle0 × falloff^n、delay_ms × n、三种样子随材质。A fanning stack of cards in three looks.
- 骨架：`El::skeleton()` 用真实的树画骨架（条是量出来的行宽），`Loading` 的两个数，零位移；四种加载方式的比较。Skeletons from the real tree, the loading rules, zero layout shift.
- 多级菜单：安全区三角形、两个延迟、键盘、可画出的三角形（`El::on_move`、`El::guide`）。Nested menus with a safe-zone triangle.

## 序章、展示而不是标语、引导、文字绕排、读数、配方 · The prologue, display not slogans, guidance, wrap, readout, recipes

- 影片的序章：同一个小界面左边原样、右边一种材质，分界线能拖，材质名一排。The prologue: one small interface, as it is and in a material, a divider and chips.
- 不再有标语：章名是一个元素，换章时一个字一个字地重组；第 9、10、11 章重做。No slogan: the chapter's title recomposes across cuts; chapters 9, 10, 11 redone.
- 底栏的引导：上一章 / 下一章、自动播放的进度、只到第一次输入为止的提示、括号提示、看不见的边。Guidance in the bottom bar and on the stage.
- 文字绕排按真实的距离、不留碎片、拖动时不跳；正文不会一个字一个字地凝聚；330 px 宽时没有文字被截掉。Wrap by true distance, no slivers, no hopping; text condenses as a whole; nothing cut at 330 px.
- F2 读数：一行和全部两种，带火花线，空闲时不要帧。The F2 readout: one line or every number, a sparkline, no frames at rest.
- `look:v1:` 配方（网址、PNG 的文本块）；八种新记号，按家族分组；Geist Pixel 的五种字面；`<amenui-view>` 自定义元素。Recipes; eight new mark sets; Geist Pixel; the `<amenui-view>` element.

## 滚动、标志与机件、多语言、组件与文档 · Scrolling, the sign and the mechanism, languages, components and docs

- 慢速滚动是整像素移动的图，快速滚动每种材质有自己的痕迹（§1.8.3，`check` 第 18 条）。Slow scrolling is a picture moved by whole pixels; fast, each material leaves its own trace.
- 影片的第三个演员是平面的标志；第 13 章是真正三维的机件（齿轮、凸轮、从动杆）。The film's third actor is a flat sign; chapter 13 is a real 3D mechanism.
- 消息目录、`t!`、复数、回退链、从右往左的行（§1.20）。Message catalogs, `t!`, plurals, a fallback chain, right-to-left rows.
- 十八个新组件、`amenui add`、按 shadcn/ui 的样子组织的文档站。Eighteen new components, `amenui add`, a documentation site laid out after shadcn/ui.

## 修正和影片 · Fixes and the film
重组有界、网格表面发射粒子、局部的时钟、十二章的影片。Bounded recomposition, particles from mesh surfaces, local clocks, the twelve-chapter film.

## 三维、GPU 粒子、只画变了的部分、预设和过渡 · 3D, GPU particles, dirty boxes, presets and transitions

## 涟漪规则、内容重组、给模型用的一面 · Ripple rules, recomposition, the model-facing surface

## 交互和性能 · Interaction and performance
只在有变化时画、帧时间读数、文字输入、增量布局和长列表、触摸、无障碍镜像。

## 标注层、全部材质、十二个预设、展示场景 · Annotation, every material, twelve presets, scenes

## 变换：粒子和流动 · Transforms: particles and flows

## 组件和 gallery · Components and the gallery

## 图片的记号、像素字、主题和材质作为数据 · Marks, pixel type, themes and materials as data

## 骨架 · The skeleton
界面树、taffy、parley、滚动、点击、焦点、数字控件。

## 竖线 · The vertical slice
两遍绘制、按轮廓走的波、原生和网页同一份代码。
