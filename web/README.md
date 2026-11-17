# amenui web shell

The page side of an amenui application on the web. The Rust side is `amenui::web_app!`, which
generates the module's exports; this file forwards the browser's events to them.

```js
import init, * as app from './pkg/my_app.js';   // wasm-bindgen --target web output
import { css, stage, mount } from 'amenui';

await init();
document.head.append(Object.assign(document.createElement('style'), { textContent: css }));
const s = stage();
document.body.append(s);
const view = await mount(s, {
  module: app,
  fonts: ['assets/NotoSansSC-Regular.otf', 'assets/FusionPixel-12px-Proportional-zh_hans.otf', 'assets/DepartureMono-Regular.otf'],
  hash: location.hash,
  ownsHash: true,
});
view.look('dark', 'dot');
```

`mount` returns `{ route(hash), look(theme, material), locale(tag), focus() }`. The fonts are the three
main bundled faces from the crate's `assets/` (see `docs/reference/fonts.md`); the others are compiled
into the module. One view per document: the module holds one interface.
