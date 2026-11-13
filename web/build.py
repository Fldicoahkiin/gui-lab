# Assemble one self-contained page: the wasm module, a subset of the font, and the glue script, inlined.
import base64, pathlib, re, subprocess, sys
root = pathlib.Path(__file__).resolve().parent.parent
text = (root / 'src/app.rs').read_text()
chars = set(''.join(re.findall(r'"([^"\\]*)"', text))) | {chr(c) for c in range(32, 127)}
sub = root / 'dist/font-subset.otf'
subprocess.run([sys.executable, '-m', 'fontTools.subset', str(root / 'assets/NotoSansSC-Regular.otf'), '--text=' + ''.join(sorted(chars)), '--output-file=' + str(sub)], check=True)
wasm = root / 'dist/pkg/lab_opt.wasm'
if not wasm.exists(): wasm = root / 'dist/pkg/lab_bg.wasm'
b = lambda p: base64.b64encode(p.read_bytes()).decode()
glue = (root / 'dist/pkg/lab.js').read_text().replace('</script', '<\\/script')
page = (root / 'web/page.html').read_text().replace('__GLUE__', glue).replace('__WASM__', b(wasm)).replace('__FONT__', b(sub))
(root / 'dist/artifact.html').write_text(page)
(root / 'dist/index.html').write_text('<!doctype html><html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1,viewport-fit=cover">' + page.replace('<div id="stage">', '</head><body style="margin:0"><div id="stage">', 1) + '</body></html>')
print('wasm', wasm.name, wasm.stat().st_size // 1024, 'KB; font', sub.stat().st_size // 1024, 'KB; page', len(page) // 1024, 'KB')
