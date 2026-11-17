---
kind: reference
---

# Fonts

The fonts in `assets/` are all under the SIL Open Font License 1.1. Their licenses are in
`assets/licenses/`. `text::load_bundled(text::ASSETS)` loads them.

| File | Face | Source | Used as |
| --- | --- | --- | --- |
| `NotoSansSC-Regular.otf` | Noto Sans SC | Google Noto | The outline face for Latin, Han and kana |
| `DepartureMono-Regular.otf` | Departure Mono 1.500 (Helena Zhang) | Its release package | The Latin pixel face, 11 px |
| `FusionPixel-12px-Proportional-zh_hans.otf` | Fusion Pixel 12px, proportional, Simplified Chinese (TakWolf, upstream 2026.08.11) | See below | The CJK pixel face, 12 px |
| `GeistPixel-{Square,Grid,Circle,Triangle,Line}.otf` | Geist Pixel (Vercel and basement.studio) | npm package `geist` 1.9.2 | Optional Latin pixel faces (`text::GEIST_PIXEL`) |
| `SourceSerif4-8pt-subset.ttf` | Source Serif 4 (Frank Grießhammer, Adobe) | Google Fonts repository | `Font::Whisper`, small lowercase type and margin notes (`text::SERIF`) |

Pixel faces are drawn only at integer multiples of their size, at whole-pixel positions and without
anti-aliasing, so their edges hold only ink and ground. Any face can also be printed as pixels through
the material (`.print(pitch)`).

## How each file was made

- **Fusion Pixel** was merged back into one OTF from the WOFF2 files split by Unicode block in the npm
  package `@vp-tw/cjk-web-fonts-fusion-pixel-font`. The licenses of the upstream fonts it includes
  (Ark Pixel, Cubic 11, Galmuri) are kept in `assets/licenses/fusion-pixel/`.
- **Geist Pixel** was converted from WOFF2 to an OTF container with fontTools, with no change to the
  glyphs or names. Its license field reads "SIL OPEN FONT LICENSE"; the text is in
  `assets/licenses/geist-pixel/OFL.txt`. It has Latin only.
- **Source Serif 4** was instanced from the variable font `SourceSerif4[opsz,wght].ttf` with fontTools at
  the 8 pt optical size and weight 400, the cut drawn for small sizes, and subset to Basic Latin,
  Latin-1 Supplement and common punctuation (82 KB). The glyphs are unchanged. The license is in
  `assets/licenses/source-serif/OFL.txt`; it has no reserved font name.

## Coverage

Scripts outside these faces are not drawn: Arabic, Hebrew, Thai, Devanagari and emoji, and Hangul in
the outline face (Fusion Pixel has Hangul). The text system does not load system fonts
(`text.rs`, `system_fonts: false`).
