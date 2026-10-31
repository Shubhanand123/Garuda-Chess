//! Garuda — UCI chess engine.
//! Usage:
//!   garuda                 UCI mode (plug into any chess GUI)
//!   garuda bench [nodes]   search 10 test positions with a node budget and report depth reached
//!   garuda perftsuite      verify move generation against known perft counts

use garuda::board::*;
use garuda::eval::evaluate;
use garuda::search::*;
use std::io::{self, BufRead};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::Instant;

const BENCH_FENS: [&str; 10] = [
    "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
    "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 10",
    "4rrk1/pp1n3p/3q2pQ/2p1pb2/2PP4/2P3N1/P2B2PP/4RRK1 b - - 7 19",
    "rq3rk1/ppp2ppp/1bnpb3/3N2B1/3NP3/7P/PPPQ1PP1/2KR3R w - - 7 14",
    "r1bq1r1k/1pp1n1pp/1p1p4/4p2Q/4Pp2/1BNP4/PPP2PPP/3R1RK1 w - - 2 14",
    "r3r1k1/2p2ppp/p1p1bn2/8/1q2P3/2NPQN2/PPP3PP/R4RK1 b - - 2 15",
    "3rr1k1/pp3pp1/1qn2np1/8/3p4/PP1R1P2/2P1NQPP/R1B3K1 b - - 0 1",
    "2r1nrk1/p2q1ppp/bp1p4/n1pPp3/P1P1P3/2PBB1N1/4QPPP/R4RK1 w - - 0 1",
    "8/2p5/3p4/KP5r/1R3p2/4P3/6k1/8 w - - 0 11",
    "6k1/6p1/6Pp/ppp5/3pn2P/1P3K2/1PP2P2/3N4 b - - 0 1",
];

struct Engine {
    board: Board,
    hashes: Vec<u64>,
    searcher: Searcher,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        match args[1].as_str() {
            "bench" => {
                let nodes = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(10_000_000);
                bench(nodes);
                return;
            }
            "perftsuite" => {
                perft_suite();
                return;
            }
            "tactics" => {
                let nodes = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(2_000_000);
                tactics(nodes);
                return;
            }
            _ => {}
        }
    }
    uci_loop();
}

fn uci_loop() {
    let stop = Arc::new(AtomicBool::new(false));
    let (tx, rx) = mpsc::channel::<String>();
    {
        let stop = stop.clone();
        thread::spawn(move || {
            let stdin = io::stdin();
            for line in stdin.lock().lines() {
                let Ok(line) = line else { break };
                let t = line.trim().to_string();
                if t == "stop" || t == "quit" {
                    stop.store(true, Ordering::Relaxed);
                }
                if tx.send(t).is_err() {
                    break;
                }
            }
            let _ = tx.send("quit".to_string());
        });
    }

    let mut eng = Engine { board: Board::startpos(), hashes: vec![], searcher: Searcher::new(64) };
    eng.hashes.push(eng.board.hash);

    for line in rx {
        let tokens: Vec<&str> = line.split_whitespace().collect();
        if tokens.is_empty() {
            continue;
        }
        match tokens[0] {
            "uci" => {
                println!("id name Garuda 0.1");
                println!("id author Shubhanand Chowdhary");
                println!("option name Hash type spin default 64 min 1 max 4096");
                println!("uciok");
            }
            "isready" => println!("readyok"),
            "ucinewgame" => eng.searcher.new_game(),
            "setoption" => {
                // setoption name Hash value N
                if let (Some(n), Some(v)) = (tokens.iter().position(|&t| t == "name"), tokens.iter().position(|&t| t == "value")) {
                    if tokens.get(n + 1).map(|s| s.eq_ignore_ascii_case("hash")) == Some(true) {
                        if let Some(mb) = tokens.get(v + 1).and_then(|s| s.parse().ok()) {
                            eng.searcher.resize_tt(mb);
                        }
                    }
                }
            }
            "position" => handle_position(&mut eng, &tokens),
            "go" => handle_go(&mut eng, &tokens, &stop),
            "perft" => {
                let d = tokens.get(1).and_then(|s| s.parse().ok()).unwrap_or(5);
                perft_divide(&eng.board, d);
            }
            "bench" => {
                let nodes = tokens.get(1).and_then(|s| s.parse().ok()).unwrap_or(10_000_000);
                bench(nodes);
            }
            "d" => print!("{}", eng.board.pretty()),
            "eval" => println!("static eval (side to move): {} cp", evaluate(&eng.board)),
            "quit" => break,
            "stop" => {}
            _ => println!("info string unknown command: {}", line),
        }
    }
}

fn handle_position(eng: &mut Engine, tokens: &[&str]) {
    let mut idx;
    let mut board: Board;
    match tokens.get(1) {
        Some(&"startpos") => {
            board = Board::startpos();
            idx = 2;
        }
        Some(&"fen") => {
            let end = tokens.iter().position(|&t| t == "moves").unwrap_or(tokens.len());
            let fen = tokens[2..end].join(" ");
            match Board::from_fen(&fen) {
                Ok(b) => board = b,
                Err(e) => {
                    println!("info string bad fen: {}", e);
                    return;
                }
            }
            idx = end;
        }
        _ => return,
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
                None => {
                    println!("info string illegal move {}", tokens[idx]);
                    break;
                }
            }
            idx += 1;
        }
    }
    eng.board = board;
    eng.hashes = hashes;
}

fn handle_go(eng: &mut Engine, tokens: &[&str], stop: &Arc<AtomicBool>) {
    let mut limits = Limits::default();
    let num = |i: usize| tokens.get(i).and_then(|s| s.parse::<f64>().ok());
    let (mut wtime, mut btime, mut winc, mut binc, mut mtg, mut movetime) = (None, None, 0.0, 0.0, None, None);
    let mut i = 1;
    while i < tokens.len() {
        match tokens[i] {
            "depth" => limits.depth = num(i + 1).unwrap_or(64.0) as i32,
            "nodes" => limits.nodes = num(i + 1).unwrap_or(1e18) as u64,
            "movetime" => movetime = num(i + 1),
            "wtime" => wtime = num(i + 1),
            "btime" => btime = num(i + 1),
            "winc" => winc = num(i + 1).unwrap_or(0.0),
            "binc" => binc = num(i + 1).unwrap_or(0.0),
            "movestogo" => mtg = num(i + 1),
            _ => {}
        }
        i += 1;
    }
    if let Some(mt) = movetime {
        limits.soft_ms = (mt - 20.0).max(1.0);
        limits.hard_ms = limits.soft_ms;
    } else {
        let (time, inc) = if eng.board.side == WHITE { (wtime, winc) } else { (btime, binc) };
        if let Some(t) = time {
            let t = (t - 50.0).max(10.0);
            let moves = mtg.unwrap_or(30.0).max(1.0);
            let soft = t / moves + inc * 0.75;
            let hard = (soft * 4.0).min(t * 0.4);
            limits.soft_ms = soft.min(hard);
            limits.hard_ms = hard;
        }
    }
    stop.store(false, Ordering::Relaxed);
    eng.searcher.stop_flag = Some(stop.clone());
    eng.searcher.silent = false;
    let res = eng.searcher.search(&eng.board, limits, &eng.hashes);
    println!("bestmove {}", res.best.to_uci());
}

fn perft_divide(b: &Board, depth: u32) {
    let t = Instant::now();
    let mut total = 0;
    for m in b.legal_moves() {
        let mut nb = *b;
        nb.make_move(m);
        let n = if depth <= 1 { 1 } else { nb.perft(depth - 1) };
        println!("{}: {}", m.to_uci(), n);
        total += n;
    }
    let s = t.elapsed().as_secs_f64();
    println!("\nNodes: {}  time {:.2}s  nps {:.0}", total, s, total as f64 / s.max(1e-9));
}

fn perft_suite() {
    let tests: [(&str, u32, u64); 6] = [
        (START_FEN, 5, 4_865_609),
        ("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1", 4, 4_085_603),
        ("8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1", 6, 11_030_083),
        ("r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1", 4, 422_333),
        ("rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8", 4, 2_103_487),
        ("r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10", 4, 3_894_594),
    ];
    let mut all_ok = true;
    let t0 = Instant::now();
    let mut total = 0;
    for (fen, d, expected) in tests {
        let b = Board::from_fen(fen).unwrap();
        let t = Instant::now();
        let n = b.perft(d);
        total += n;
        let ok = n == expected;
        all_ok &= ok;
        println!(
            "{}  depth {}  got {:>10}  expected {:>10}  {:.2}s   {}",
            if ok { "PASS" } else { "FAIL" },
            d,
            n,
            expected,
            t.elapsed().as_secs_f64(),
            fen
        );
    }
    let s = t0.elapsed().as_secs_f64();
    println!("\n{}  total {} nodes in {:.2}s ({:.1} M nodes/s)", if all_ok { "ALL PASSED" } else { "SOME FAILED" }, total, s, total as f64 / s / 1e6);
}

/// Win-At-Chess style tactical positions: (fen, expected best move in UCI).
const TACTICS: [(&str, &str); 12] = [
    ("2rr3k/pp3pp1/1nnqbN1p/3pN3/2pP4/2P3Q1/PPB4P/R4RK1 w - - 0 1", "g3g6"),
    ("8/7p/5k2/5p2/p1p2P2/Pr1pPK2/1P1R3P/8 b - - 0 1", "b3b2"),
    ("5rk1/1ppb3p/p1pb4/6q1/3P1p1r/2P1R2P/PP1BQ1P1/5RKN w - - 0 1", "e3g3"),
    ("r1bq2rk/pp3pbp/2p1p1pQ/7P/3P4/2PB1N2/PP3PPR/2KR4 w - - 0 1", "h6h7"),
    ("5k2/6pp/p1qN4/1p1p4/3P4/2PKP2Q/PP3r2/3R4 b - - 0 1", "c6c4"),
    ("7k/p7/1R5K/6r1/6p1/6P1/8/8 w - - 0 1", "b6b7"),
    ("rnbqkb1r/pppp1ppp/8/4P3/6n1/7P/PPPNPPP1/R1BQKBNR b KQkq - 0 1", "g4e3"),
    ("r4q1k/p2bR1rp/2p2Q1N/5p2/5p2/2P5/PP3PPP/R5K1 w - - 0 1", "e7f7"),
    ("3q1rk1/p4pp1/2pb3p/3p4/6Pr/1PNQ4/P1PB1PP1/4RRK1 b - - 0 1", "d6h2"),
    ("2br2k1/2q3rn/p2NppQ1/2p1P3/Pp5R/4P3/1P3PPP/3R2K1 w - - 0 1", "h4h7"),
    ("r1bqkb1r/pppp1ppp/2n2n2/4p2Q/2B1P3/8/PPPP1PPP/RNB1K1NR w KQkq - 4 4", "h5f7"),
    ("6k1/5ppp/8/8/8/8/5PPP/3R2K1 w - - 0 1", "d1d8"),
];

fn tactics(nodes: u64) {
    let mut s = Searcher::new(64);
    s.silent = true;
    let mut solved = 0;
    println!("Tactical test, node budget {}\n", nodes);
    for (i, (fen, expected)) in TACTICS.iter().enumerate() {
        s.new_game();
        let b = Board::from_fen(fen).unwrap();
        let r = s.search(&b, Limits { nodes, ..Limits::default() }, &[b.hash]);
        let ok = r.best.to_uci() == *expected;
        if ok {
            solved += 1;
        }
        println!(
            "{:>2}. {}  played {}  expected {}  depth {:>2}  {:>9}  {:>6.0} ms",
            i + 1,
            if ok { "OK  " } else { "MISS" },
            r.best.to_uci(),
            expected,
            r.depth,
            score_string(r.score),
            r.time_ms
        );
    }
    println!("\nSolved {}/{}", solved, TACTICS.len());
}

fn bench(nodes: u64) {
    let mut s = Searcher::new(64);
    s.silent = true;
    let mut total_nodes = 0u64;
    let mut total_ms = 0.0;
    let mut depths = vec![];
    println!("Node budget per position: {}\n", nodes);
    for (i, fen) in BENCH_FENS.iter().enumerate() {
        s.new_game();
        let b = Board::from_fen(fen).unwrap();
        let limits = Limits { nodes, ..Limits::default() };
        let r = s.search(&b, limits, &[b.hash]);
        total_nodes += r.nodes;
        total_ms += r.time_ms;
        depths.push(r.depth);
        println!(
            "{:>2}. depth {:>2}  seldepth {:>2}  nodes {:>9}  {:>6.0} ms  {:>11}  best {}",
            i + 1,
            r.depth,
            r.seldepth,
            r.nodes,
            r.time_ms,
            score_string(r.score),
            r.best.to_uci()
        );
    }
    let avg = depths.iter().sum::<i32>() as f64 / depths.len() as f64;
    println!(
        "\nAverage depth {:.1} (min {}, max {})  |  {} nodes  {:.1}s  {:.2} M nodes/s",
        avg,
        depths.iter().min().unwrap(),
        depths.iter().max().unwrap(),
        total_nodes,
        total_ms / 1000.0,
        total_nodes as f64 / (total_ms / 1000.0) / 1e6
    );
}
