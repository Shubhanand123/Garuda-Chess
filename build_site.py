"""Builds site/index.html: the app template + embedded WASM engine + piece set, with the React UI bundle (ui.js / ui.css) linked."""
import base64, pathlib
root = pathlib.Path(__file__).parent
wasm = (root / 'target/wasm32-unknown-unknown/release/garuda.wasm').read_bytes()
t = (root / 'web/app.html').read_text()
t = t.replace('__WASM_B64__', base64.b64encode(wasm).decode()).replace('__PIECES__', (root / 'web/pieces.json').read_text())
(root / 'web/garuda-chess.html').write_text(t)
head = ('<!doctype html>\n<html lang="en">\n<head>\n<meta charset="utf-8">\n'
        '<meta name="viewport" content="width=device-width,initial-scale=1,viewport-fit=cover">\n'
        '<meta name="description" content="Play Garuda, a Rust chess engine running in your browser via WebAssembly.">\n'
        '<meta name="theme-color" content="#0d1015">\n'
        '<style>[hidden]{display:none!important}img{max-width:100%}:root{padding-top:env(safe-area-inset-top,0px);padding-bottom:env(safe-area-inset-bottom,0px)}</style>\n'
        '<link rel="stylesheet" href="ui.css">\n<script src="ui.js" defer></script>\n')
i = t.index('</style>') + len('</style>')
(root / 'site/index.html').write_text(head + t[:i] + '\n</head>\n<body>\n' + t[i:] + '\n</body>\n</html>\n')
print('site/index.html', (root / 'site/index.html').stat().st_size, 'bytes')
