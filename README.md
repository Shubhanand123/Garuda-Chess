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
