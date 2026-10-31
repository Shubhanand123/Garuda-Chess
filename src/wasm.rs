//! Minimal WebAssembly interface (no wasm-bindgen needed).
//! JS writes a UTF-8 command into the input buffer, calls `command(len)`,
//! then reads `out_len` bytes of JSON / text from `out_ptr()`.
//!
//! Commands:
//!   position startpos [moves e2e4 ...] | position fen <fen> [moves ...]
//!   go [nodes N] [movetime MS] [depth D]
//!   state            -> JSON with fen, side, legal moves, check and game status
//!   newgame

use crate::board::*;
use crate::search::*;
use std::cell::RefCell;

struct State {
    board: Board,
    hashes: Vec<u64>,
    searcher: Searcher,
    inbuf: Vec<u8>,
    out: Vec<u8>,
}

impl State {
    fn new() -> State {
        let board = Board::startpos();
        let mut searcher = Searcher::new(32);
        searcher.silent = true;
        State { hashes: vec![board.hash], board, searcher, inbuf: vec![0; 16384], out: Vec::new() }
    }
}

thread_local! {
    static ST: RefCell<Option<State>> = const { RefCell::new(None) };
}

fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    ST.with(|s| {
        let mut s = s.borrow_mut();
        if s.is_none() {
            *s = Some(State::new());
        }
        f(s.as_mut().unwrap())
    })
}

#[no_mangle]
pub extern "C" fn in_ptr() -> *mut u8 {
    with(|s| s.inbuf.as_mut_ptr())
}

#[no_mangle]
pub extern "C" fn out_ptr() -> *const u8 {
    with(|s| s.out.as_ptr())
}

#[no_mangle]
pub extern "C" fn command(len: usize) -> usize {
    with(|s| {
        let len = len.min(s.inbuf.len());
        let cmd = String::from_utf8_lossy(&s.inbuf[..len]).to_string();
        let reply = run(s, &cmd);
        s.out = reply.into_bytes();
        s.out.len()
    })
}

fn run(s: &mut State, cmd: &str) -> String {
    let tokens: Vec<&str> = cmd.split_whitespace().collect();
    match tokens.first().copied() {
        Some("position") => position(s, &tokens),
        Some("go") => go(s, &tokens),
        Some("state") => state_json(s),
        Some("skill") => skill(s, &tokens),
        Some("newgame") => {
            s.searcher.new_game();
            "ok".into()
        }
        _ => format!("error unknown command '{}'", cmd),
    }
}

fn position(s: &mut State, tokens: &[&str]) -> String {
    let mut idx;
    let mut board: Board;
    match tokens.get(1) {
        Some(&"startpos") => {
            board = Board::startpos();
            idx = 2;
        }
        Some(&"fen") => {
            let end = tokens.iter().position(|&t| t == "moves").unwrap_or(tokens.len());
            match Board::from_fen(&tokens[2..end].join(" ")) {
                Ok(b) => board = b,
                Err(e) => return format!("error {}", e),
            }
            idx = end;
        }
        _ => return "error expected startpos or fen".into(),
    }
    let mut hashes = vec![board.hash];
    if tokens.get(idx) == Some(&"moves") {
        idx += 1;
        while idx < tokens.len() {
            match board.parse_move(tokens[idx]) {
                Some(m) => {
                    board.make_move(m);
                    hashes.push(board.hash);
                }
                None => return format!("error illegal move {}", tokens[idx]),
            }
            idx += 1;
        }
    }
    s.board = board;
    s.hashes = hashes;
    "ok".into()
}

fn go(s: &mut State, tokens: &[&str]) -> String {
    let mut limits = Limits::default();
    let num = |i: usize| tokens.get(i).and_then(|t| t.parse::<f64>().ok());
    for i in 1..tokens.len() {
        match tokens[i] {
            "nodes" => limits.nodes = num(i + 1).unwrap_or(1e6) as u64,
            "movetime" => {
                let ms = num(i + 1).unwrap_or(5000.0);
                limits.soft_ms = ms;
                limits.hard_ms = ms;
            }
            "depth" => limits.depth = num(i + 1).unwrap_or(20.0) as i32,
            _ => {}
        }
    }
    let r = s.searcher.search(&s.board, limits, &s.hashes);
    let pv: Vec<String> = r.pv.iter().map(|m| m.to_uci()).collect();
    // principal variation in SAN, validated move by move
    let mut b = s.board;
    let mut pv_san = Vec::new();
    for m in &r.pv {
        if !b.legal_moves().contains(m) {
            break;
        }
        pv_san.push(b.san(*m));
        b.make_move(*m);
    }
    format!(
        "{{\"best\":\"{}\",\"score\":\"{}\",\"depth\":{},\"seldepth\":{},\"nodes\":{},\"time\":{},\"pv\":\"{}\",\"pvsan\":\"{}\"}}",
        r.best.to_uci(),
        score_string(r.score),
        r.depth,
        r.seldepth,
        r.nodes,
        r.time_ms as u64,
        pv.join(" "),
        pv_san.join(" ")
    )
}

fn skill(s: &mut State, tokens: &[&str]) -> String {
    let num = |k: &str, d: f64| {
        tokens.iter().position(|&t| t == k).and_then(|i| tokens.get(i + 1)).and_then(|t| t.parse::<f64>().ok()).unwrap_or(d)
    };
    let (m, score) = s.searcher.skill_move(
        &s.board,
        num("depth", 2.0) as i32,
        num("noise", 100.0) as i32,
        num("blunder", 0.0) as u32,
        num("seed", 12345.0) as u64,
        &s.hashes,
    );
    format!("{{\"best\":\"{}\",\"score\":\"{}\",\"depth\":{},\"nodes\":{},\"time\":0,\"pv\":\"\",\"pvsan\":\"\"}}",
        m.to_uci(), score_string(score), num("depth", 2.0) as i32, s.searcher.nodes)
}

fn state_json(s: &State) -> String {
    let b = &s.board;
    let legal: Vec<String> = b.legal_moves().iter().map(|m| format!("[\"{}\",\"{}\"]", m.to_uci(), b.san(*m))).collect();
    let check = b.in_check();
    let reps = s.hashes.iter().filter(|&&h| h == b.hash).count();
    let status = if legal.is_empty() {
        if check {
            "checkmate"
        } else {
            "stalemate"
        }
    } else if b.halfmove >= 100 {
        "fifty"
    } else if reps >= 3 {
        "repetition"
    } else if b.insufficient_material() {
        "insufficient"
    } else {
        "playing"
    };
    format!(
        "{{\"fen\":\"{}\",\"turn\":\"{}\",\"check\":{},\"status\":\"{}\",\"legal\":[{}]}}",
        b.to_fen(),
        if b.side == WHITE { "w" } else { "b" },
        check,
        status,
        legal.join(",")
    )
}
