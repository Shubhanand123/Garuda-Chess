# Garuda — Rust chess engine

## Build (native, fastest)
    cargo build --release
    RUSTFLAGS="-C target-cpu=native" cargo build --release   # a bit faster on your own PC

## Commands
    ./target/release/garuda perftsuite        # verify move generation (6 standard positions)
    ./target/release/garuda bench 10000000    # depth reached per position with a 10M node budget
    ./target/release/garuda tactics 2000000   # tactical test positions
    ./target/release/garuda                   # UCI mode: add it as an engine in Arena, Cute Chess, En Croissant, BanksiaGUI

In UCI mode you can also type:  position startpos moves e2e4  /  go nodes 5000000  /  d  /  eval  /  perft 5

## Browser build
    rustup target add wasm32-unknown-unknown
    cargo build --release --lib --target wasm32-unknown-unknown
    # web/template.html + embed target/wasm32-unknown-unknown/release/garuda.wasm as base64 (see web/build note)

## Layout
    src/bitboard.rs  attack tables, sliding attacks
    src/board.rs     board, Zobrist hashing, legal move generation, SEE, perft
    src/eval.rs      PeSTO tapered eval + passed pawns, rook files, bishop pair, mobility
    src/search.rs    PVS, iterative deepening, aspiration windows, TT, null move, LMR,
                     reverse futility, razoring, futility/LMP/SEE pruning, killers, history, qsearch
    src/wasm.rs      WebAssembly interface (no wasm-bindgen)
    src/main.rs      UCI loop, bench, perft and tactics tests

## Website (site/)
    cd ui && npm install && npm run build   # React components -> site/ui.js + site/ui.css
    cargo build --release --lib --target wasm32-unknown-unknown
    python3 build_site.py                    # web/app.html + engine + pieces -> site/index.html
Vercel serves the `site/` folder. Components in `ui/src/components`: glass reveal intro, ContainerScroll hero,
clothing-rail bot picker (chessboard jerseys), halftone wordmark footer.
