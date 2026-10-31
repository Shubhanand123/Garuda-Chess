//! Principal Variation Search with iterative deepening, aspiration windows,
//! transposition table, null-move pruning, late move reductions, reverse futility,
//! razoring, futility / late-move / SEE pruning, killers, history and quiescence search.

use crate::board::*;
use crate::eval::evaluate;
use crate::timer::now_ms;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub const INF: i32 = 32000;
pub const MATE: i32 = 31000;
pub const MATE_BOUND: i32 = MATE - 512;
pub const MAX_PLY: usize = 120;

const EXACT: u8 = 1;
const LOWER: u8 = 2;
const UPPER: u8 = 3;

// ---------------- Transposition table ----------------

#[derive(Clone, Copy, Default)]
struct TTEntry {
    key: u64,
    mv: u16,
    score: i16,
    eval: i16,
    depth: i8,
    flag: u8,
}

pub struct TT {
    table: Vec<TTEntry>,
}

impl TT {
    pub fn new(mb: usize) -> TT {
        let n = (mb * 1024 * 1024 / std::mem::size_of::<TTEntry>()).max(1024);
        TT { table: vec![TTEntry::default(); n] }
    }
    #[inline(always)]
    fn idx(&self, key: u64) -> usize {
        ((key as u128 * self.table.len() as u128) >> 64) as usize
    }
    pub fn clear(&mut self) {
        self.table.iter_mut().for_each(|e| *e = TTEntry::default());
    }
    #[inline(always)]
    fn probe(&self, key: u64) -> Option<TTEntry> {
        let e = self.table[self.idx(key)];
        if e.key == key && e.flag != 0 {
            Some(e)
        } else {
            None
        }
    }
    #[inline(always)]
    fn store(&mut self, key: u64, mv: Move, score: i16, eval: i32, depth: i32, flag: u8) {
        let i = self.idx(key);
        let e = &mut self.table[i];
        if e.key != key || flag == EXACT || depth + 3 >= e.depth as i32 {
            let mv = if mv == Move::NONE && e.key == key { e.mv } else { mv.0 };
            *e = TTEntry { key, mv, score, eval: eval.clamp(-INF, INF) as i16, depth: depth.clamp(-1, 127) as i8, flag };
        }
    }
    pub fn hashfull(&self) -> usize {
        let n = self.table.len().min(1000);
        self.table[..n].iter().filter(|e| e.flag != 0).count() * 1000 / n
    }
}

#[inline(always)]
fn score_to_tt(s: i32, ply: usize) -> i16 {
    (if s >= MATE_BOUND {
        s + ply as i32
    } else if s <= -MATE_BOUND {
        s - ply as i32
    } else {
        s
    }) as i16
}

#[inline(always)]
fn score_from_tt(s: i32, ply: usize) -> i32 {
    if s >= MATE_BOUND {
        s - ply as i32
    } else if s <= -MATE_BOUND {
        s + ply as i32
    } else {
        s
    }
}

// ---------------- Limits / results ----------------

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub depth: i32,
    pub nodes: u64,
    pub soft_ms: f64,
    pub hard_ms: f64,
}

impl Default for Limits {
    fn default() -> Self {
        Limits { depth: (MAX_PLY - 10) as i32, nodes: u64::MAX, soft_ms: f64::INFINITY, hard_ms: f64::INFINITY }
    }
}

#[derive(Clone, Debug)]
pub struct SearchResult {
    pub best: Move,
    pub score: i32,
    pub depth: i32,
    pub seldepth: usize,
    pub nodes: u64,
    pub time_ms: f64,
    pub pv: Vec<Move>,
}

pub fn score_string(s: i32) -> String {
    if s >= MATE_BOUND {
        format!("mate {}", (MATE - s + 1) / 2)
    } else if s <= -MATE_BOUND {
        format!("mate -{}", (MATE + s) / 2)
    } else {
        format!("cp {}", s)
    }
}

// ---------------- Searcher ----------------

pub struct Searcher {
    pub tt: TT,
    pub nodes: u64,
    pub seldepth: usize,
    pub silent: bool,
    pub stop_flag: Option<Arc<AtomicBool>>,
    history: Box<[[[i32; 64]; 64]; 2]>,
    killers: [[Move; 2]; MAX_PLY + 4],
    eval_stack: [i32; MAX_PLY + 4],
    pv: Box<[[Move; MAX_PLY + 4]; MAX_PLY + 4]>,
    pv_len: [usize; MAX_PLY + 4],
    lmr: [[i32; 64]; 64],
    hashes: Vec<u64>,
    limits: Limits,
    start: f64,
    stop: bool,
}

const MVV: [i32; 7] = [100, 320, 330, 500, 950, 0, 0];

impl Searcher {
    pub fn new(tt_mb: usize) -> Searcher {
        let mut lmr = [[0i32; 64]; 64];
        for (d, row) in lmr.iter_mut().enumerate().skip(1) {
            for (m, v) in row.iter_mut().enumerate().skip(1) {
                *v = (0.75 + (d as f64).ln() * (m as f64).ln() / 2.25) as i32;
            }
        }
        Searcher {
            tt: TT::new(tt_mb),
            nodes: 0,
            seldepth: 0,
            silent: false,
            stop_flag: None,
            history: Box::new([[[0; 64]; 64]; 2]),
            killers: [[Move::NONE; 2]; MAX_PLY + 4],
            eval_stack: [0; MAX_PLY + 4],
            pv: Box::new([[Move::NONE; MAX_PLY + 4]; MAX_PLY + 4]),
            pv_len: [0; MAX_PLY + 4],
            lmr,
            hashes: Vec::with_capacity(1024),
            limits: Limits::default(),
            start: 0.0,
            stop: false,
        }
    }

    pub fn resize_tt(&mut self, mb: usize) {
        self.tt = TT::new(mb);
    }

    pub fn new_game(&mut self) {
        self.tt.clear();
        *self.history = [[[0; 64]; 64]; 2];
    }

    #[inline(always)]
    fn should_stop(&mut self) -> bool {
        if self.stop {
            return true;
        }
        if self.nodes >= self.limits.nodes {
            self.stop = true;
        } else if (self.nodes & 2047) == 0 {
            if now_ms() - self.start >= self.limits.hard_ms {
                self.stop = true;
            }
            if let Some(f) = &self.stop_flag {
                if f.load(Ordering::Relaxed) {
                    self.stop = true;
                }
            }
        }
        self.stop
    }

    fn is_repetition(&self, b: &Board) -> bool {
        let n = self.hashes.len();
        let limit = (b.halfmove as usize).min(n.saturating_sub(1));
        let mut i = 4;
        while i <= limit {
            if self.hashes[n - 1 - i] == b.hash {
                return true;
            }
            i += 2;
        }
        false
    }

    #[inline(always)]
    fn update_history(&mut self, side: usize, m: Move, bonus: i32) {
        let h = &mut self.history[side][m.from()][m.to()];
        *h += bonus - *h * bonus.abs() / 16384;
    }

    fn score_moves(&self, b: &Board, list: &MoveList, scores: &mut [i32; 256], tt_move: Move, ply: usize) {
        for i in 0..list.len {
            let m = list.moves[i];
            scores[i] = if m == tt_move {
                1 << 30
            } else if m.is_capture() || m.is_promo() {
                let victim = if m.flag() == EP_CAPTURE { PAWN } else { b.piece_type_at(m.to()) };
                let attacker = b.piece_type_at(m.from()) as i32;
                let base = MVV[victim] * 10 - attacker;
                if m.is_promo() && m.promo_piece() != QUEEN {
                    -(1 << 29) + base
                } else if m.is_promo() || b.see_ge(m, 0) {
                    (1 << 28) + base + if m.is_promo() { 5000 } else { 0 }
                } else {
                    -(1 << 28) + base
                }
            } else if m == self.killers[ply][0] {
                (1 << 27) + 1
            } else if m == self.killers[ply][1] {
                1 << 27
            } else {
                self.history[b.side][m.from()][m.to()]
            };
        }
    }

    #[inline(always)]
    fn pick(list: &mut MoveList, scores: &mut [i32; 256], i: usize) -> (Move, i32) {
        let mut best = i;
        for j in (i + 1)..list.len {
            if scores[j] > scores[best] {
                best = j;
            }
        }
        list.moves.swap(i, best);
        scores.swap(i, best);
        (list.moves[i], scores[i])
    }

    // ---------------- Iterative deepening ----------------

    /// `game_hashes`: Zobrist keys of all positions in the game so far, ending with `root`.
    pub fn search(&mut self, root: &Board, limits: Limits, game_hashes: &[u64]) -> SearchResult {
        self.limits = limits;
        self.start = now_ms();
        self.nodes = 0;
        self.seldepth = 0;
        self.stop = false;
        self.killers = [[Move::NONE; 2]; MAX_PLY + 4];
        for side in self.history.iter_mut() {
            for row in side.iter_mut() {
                for h in row.iter_mut() {
                    *h /= 2;
                }
            }
        }
        self.hashes.clear();
        self.hashes.extend_from_slice(game_hashes);
        if self.hashes.last() != Some(&root.hash) {
            self.hashes.push(root.hash);
        }

        let legal = root.legal_moves();
        let mut best = legal.first().copied().unwrap_or(Move::NONE);
        let mut best_pv: Vec<Move> = vec![best];
        let mut score = 0;
        let mut completed = 0;
        let mut seldepth = 0;

        if legal.len() <= 1 {
            // forced (or no) move: still search a little for a score, but cap effort
            self.limits.depth = self.limits.depth.min(6);
        }

        for depth in 1..=self.limits.depth {
            self.seldepth = 0;
            let mut delta = 20;
            let (mut alpha, mut beta) =
                if depth >= 5 { ((score - delta).max(-INF), (score + delta).min(INF)) } else { (-INF, INF) };
            let mut result = 0;
            loop {
                let v = self.negamax(root, alpha, beta, depth, 0, false);
                if self.stop {
                    break;
                }
                if v <= alpha {
                    beta = (alpha + beta) / 2;
                    alpha = (v - delta).max(-INF);
                } else if v >= beta {
                    beta = (v + delta).min(INF);
                } else {
                    result = v;
                    break;
                }
                delta += delta / 2 + 10;
                if delta > 1500 {
                    alpha = -INF;
                    beta = INF;
                }
            }
            if self.stop {
                break;
            }
            score = result;
            completed = depth;
            seldepth = self.seldepth;
            if self.pv_len[0] > 0 && self.pv[0][0] != Move::NONE {
                best = self.pv[0][0];
                best_pv = self.pv[0][..self.pv_len[0]].to_vec();
            }
            let elapsed = now_ms() - self.start;
            if !self.silent {
                let nps = if elapsed > 0.0 { (self.nodes as f64 * 1000.0 / elapsed) as u64 } else { 0 };
                let pv: Vec<String> = best_pv.iter().map(|m| m.to_uci()).collect();
                println!(
                    "info depth {} seldepth {} score {} nodes {} nps {} hashfull {} time {} pv {}",
                    depth,
                    seldepth,
                    score_string(score),
                    self.nodes,
                    nps,
                    self.tt.hashfull(),
                    elapsed as u64,
                    pv.join(" ")
                );
            }
            if elapsed >= self.limits.soft_ms || self.nodes >= self.limits.nodes {
                break;
            }
        }

        SearchResult {
            best,
            score,
            depth: completed,
            seldepth,
            nodes: self.nodes,
            time_ms: now_ms() - self.start,
            pv: best_pv,
        }
    }

    /// Weakened play for lower bot levels: each root move gets a shallow search,
    /// then random noise is added to its score; with `blunder_pct`% chance a random
    /// move from the reasonable half of the list is played instead.
    /// Returns (move, real score of that move from the side to move's view).
    pub fn skill_move(&mut self, root: &Board, depth: i32, noise: i32, blunder_pct: u32, seed: u64, game_hashes: &[u64]) -> (Move, i32) {
        self.limits = Limits::default();
        self.start = now_ms();
        self.stop = false;
        self.nodes = 0;
        self.hashes.clear();
        self.hashes.extend_from_slice(game_hashes);
        if self.hashes.last() != Some(&root.hash) {
            self.hashes.push(root.hash);
        }
        let mut rng = seed | 1;
        let mut next = move || {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            rng
        };
        let mut scored: Vec<(Move, i32)> = Vec::new();
        for m in root.legal_moves() {
            let mut nb = *root;
            nb.make_move(m);
            self.hashes.push(nb.hash);
            let s = -self.negamax(&nb, -INF, INF, depth - 1, 1, true);
            self.hashes.pop();
            scored.push((m, s));
        }
        if scored.is_empty() {
            return (Move::NONE, 0);
        }
        scored.sort_by(|a, b| b.1.cmp(&a.1));
        // never miss a mate in one, even at low levels
        if scored[0].1 >= MATE - 2 {
            return scored[0];
        }
        if blunder_pct > 0 && (next() % 100) < blunder_pct as u64 {
            let half = (scored.len() + 1) / 2;
            return scored[(next() % half as u64) as usize];
        }
        let mut best = scored[0];
        let mut best_noisy = i32::MIN;
        for &(m, s) in &scored {
            let noisy = s + if noise > 0 { (next() % noise as u64) as i32 } else { 0 };
            if noisy > best_noisy {
                best_noisy = noisy;
                best = (m, s);
            }
        }
        best
    }

    // ---------------- Main search ----------------

    fn negamax(&mut self, b: &Board, mut alpha: i32, mut beta: i32, mut depth: i32, ply: usize, allow_null: bool) -> i32 {
        self.pv_len[ply] = ply;
        if self.should_stop() {
            return 0;
        }
        let in_check = b.in_check();
        if in_check {
            depth += 1;
        }
        if depth <= 0 {
            return self.qsearch(b, alpha, beta, ply);
        }
        self.nodes += 1;
        let root = ply == 0;
        let pv_node = beta - alpha > 1;
        if ply > self.seldepth {
            self.seldepth = ply;
        }

        if !root {
            if b.halfmove >= 100 || self.is_repetition(b) || b.insufficient_material() {
                return 0;
            }
            if ply >= MAX_PLY - 2 {
                return evaluate(b);
            }
            // mate distance pruning
            alpha = alpha.max(-MATE + ply as i32);
            beta = beta.min(MATE - ply as i32 - 1);
            if alpha >= beta {
                return alpha;
            }
        }

        // --- transposition table ---
        let tt_entry = self.tt.probe(b.hash);
        let mut tt_move = Move::NONE;
        let mut tt_score = 0;
        let mut tt_flag = 0;
        if let Some(e) = tt_entry {
            tt_move = Move(e.mv);
            tt_score = score_from_tt(e.score as i32, ply);
            tt_flag = e.flag;
            if !pv_node && e.depth as i32 >= depth {
                if tt_flag == EXACT
                    || (tt_flag == LOWER && tt_score >= beta)
                    || (tt_flag == UPPER && tt_score <= alpha)
                {
                    return tt_score;
                }
            }
        }

        // --- static evaluation ---
        let static_eval;
        let mut eval;
        if in_check {
            static_eval = -INF;
            eval = -INF;
        } else {
            static_eval = match tt_entry {
                Some(e) => e.eval as i32,
                None => evaluate(b),
            };
            eval = static_eval;
            if tt_entry.is_some()
                && (tt_flag == EXACT
                    || (tt_flag == LOWER && tt_score > eval)
                    || (tt_flag == UPPER && tt_score < eval))
            {
                eval = tt_score;
            }
        }
        self.eval_stack[ply] = static_eval;
        let improving = !in_check && ply >= 2 && static_eval > self.eval_stack[ply - 2];
        self.killers[ply + 1] = [Move::NONE; 2];

        // --- node-level pruning ---
        if !pv_node && !in_check {
            // Reverse futility pruning
            if depth <= 8
                && eval.abs() < MATE_BOUND
                && eval - (80 * depth - if improving { 60 } else { 0 }) >= beta
            {
                return eval;
            }
            // Razoring
            if depth <= 3 && eval + 200 + 200 * depth <= alpha {
                let v = self.qsearch(b, alpha, alpha + 1, ply);
                if v <= alpha {
                    return v;
                }
            }
            // Null move pruning
            if allow_null && depth >= 3 && eval >= beta && static_eval >= beta && b.has_non_pawn(b.side) {
                let r = 3 + depth / 3 + ((eval - beta) / 200).min(3);
                let mut nb = *b;
                nb.make_null();
                self.hashes.push(nb.hash);
                let v = -self.negamax(&nb, -beta, -beta + 1, depth - r, ply + 1, false);
                self.hashes.pop();
                if self.stop {
                    return 0;
                }
                if v >= beta {
                    return if v >= MATE_BOUND { beta } else { v };
                }
            }
        }

        // Internal iterative reduction: no hash move => likely a less important node
        if depth >= 4 && tt_move == Move::NONE {
            depth -= 1;
        }

        // --- move loop ---
        let mut list = MoveList::new();
        b.gen_moves(&mut list, false);
        let mut scores = [0i32; 256];
        self.score_moves(b, &list, &mut scores, tt_move, ply);

        let orig_alpha = alpha;
        let mut best = -INF;
        let mut best_move = Move::NONE;
        let mut legal = 0usize;
        let mut quiets_seen = 0;
        let mut quiets_tried = [Move::NONE; 64];
        let mut nq = 0;

        for i in 0..list.len {
            let (m, mscore) = Self::pick(&mut list, &mut scores, i);
            let is_quiet = !m.is_capture() && !m.is_promo();

            let mut nb = *b;
            if !nb.make_move(m) {
                continue;
            }
            let gives_check = nb.in_check();
            // pawn pushes to the 6th/7th rank are never "unnecessary" moves
            let pawn_advance = b.piece_type_at(m.from()) == PAWN && {
                let rank = m.to() / 8;
                if b.side == WHITE { rank >= 5 } else { rank <= 2 }
            };

            // move-level pruning (only after we have a real score)
            if !root && best > -MATE_BOUND {
                if is_quiet {
                    if !pv_node && !in_check && !gives_check && !pawn_advance {
                        let lmp_limit = (3 + depth * depth) / if improving { 1 } else { 2 };
                        if depth <= 8 && quiets_seen >= lmp_limit {
                            continue;
                        }
                        if depth <= 6 && static_eval + 100 + 100 * depth <= alpha {
                            continue;
                        }
                    }
                    if depth <= 7 && !gives_check && !b.see_ge(m, -50 * depth) {
                        continue;
                    }
                } else if depth <= 6 && !gives_check && !b.see_ge(m, -100 * depth) {
                    continue;
                }
            }

            legal += 1;
            if is_quiet {
                quiets_seen += 1;
            }
            self.hashes.push(nb.hash);
            let new_depth = depth - 1;
            let mut score;

            if legal == 1 {
                score = -self.negamax(&nb, -beta, -alpha, new_depth, ply + 1, true);
            } else {
                // Late move reductions
                let mut r = 0;
                if depth >= 3 && (is_quiet || mscore < 0) {
                    r = self.lmr[(depth as usize).min(63)][legal.min(63)];
                    if pv_node {
                        r -= 1;
                    }
                    if !improving {
                        r += 1;
                    }
                    if gives_check {
                        r -= 1;
                    }
                    if is_quiet {
                        if mscore >= (1 << 27) {
                            r -= 1; // killer
                        }
                        r -= self.history[b.side][m.from()][m.to()] / 6000;
                    }
                    r = r.max(0).min(new_depth - 1);
                }
                score = -self.negamax(&nb, -alpha - 1, -alpha, new_depth - r, ply + 1, true);
                if score > alpha && r > 0 {
                    score = -self.negamax(&nb, -alpha - 1, -alpha, new_depth, ply + 1, true);
                }
                if score > alpha && score < beta {
                    score = -self.negamax(&nb, -beta, -alpha, new_depth, ply + 1, true);
                }
            }
            self.hashes.pop();
            if self.stop {
                return 0;
            }

            if score > best {
                best = score;
                if score > alpha {
                    alpha = score;
                    best_move = m;
                    self.pv[ply][ply] = m;
                    let next_len = self.pv_len[ply + 1].max(ply + 1);
                    for j in (ply + 1)..next_len {
                        let v = self.pv[ply + 1][j];
                        self.pv[ply][j] = v;
                    }
                    self.pv_len[ply] = next_len;

                    if score >= beta {
                        if is_quiet {
                            if self.killers[ply][0] != m {
                                self.killers[ply][1] = self.killers[ply][0];
                                self.killers[ply][0] = m;
                            }
                            let bonus = (depth * depth * 16 + 32).min(1600);
                            self.update_history(b.side, m, bonus);
                            for &q in quiets_tried.iter().take(nq) {
                                self.update_history(b.side, q, -bonus);
                            }
                        }
                        break;
                    }
                }
            }
            if is_quiet && nq < 64 {
                quiets_tried[nq] = m;
                nq += 1;
            }
        }

        if legal == 0 {
            return if in_check { -MATE + ply as i32 } else { 0 };
        }

        let flag = if best >= beta {
            LOWER
        } else if alpha > orig_alpha {
            EXACT
        } else {
            UPPER
        };
        let store_move = if best_move != Move::NONE { best_move } else { tt_move };
        self.tt.store(b.hash, store_move, score_to_tt(best, ply), static_eval, depth, flag);
        best
    }

    // ---------------- Quiescence ----------------

    fn qsearch(&mut self, b: &Board, mut alpha: i32, beta: i32, ply: usize) -> i32 {
        self.pv_len[ply] = ply;
        if self.should_stop() {
            return 0;
        }
        self.nodes += 1;
        if ply > self.seldepth {
            self.seldepth = ply;
        }
        if ply >= MAX_PLY - 2 {
            return evaluate(b);
        }
        let in_check = b.in_check();

        let tt_entry = self.tt.probe(b.hash);
        let mut tt_move = Move::NONE;
        if let Some(e) = tt_entry {
            tt_move = Move(e.mv);
            let s = score_from_tt(e.score as i32, ply);
            if e.flag == EXACT || (e.flag == LOWER && s >= beta) || (e.flag == UPPER && s <= alpha) {
                return s;
            }
        }

        let static_eval;
        let mut best;
        if in_check {
            static_eval = -INF;
            best = -INF;
        } else {
            static_eval = match tt_entry {
                Some(e) => e.eval as i32,
                None => evaluate(b),
            };
            best = static_eval;
            if best >= beta {
                return best;
            }
            if best > alpha {
                alpha = best;
            }
        }

        let mut list = MoveList::new();
        b.gen_moves(&mut list, !in_check);
        let mut scores = [0i32; 256];
        self.score_moves(b, &list, &mut scores, tt_move, MAX_PLY);

        let orig_alpha = alpha;
        let mut best_move = Move::NONE;
        let mut legal = 0;
        for i in 0..list.len {
            let (m, mscore) = Self::pick(&mut list, &mut scores, i);
            if !in_check {
                if mscore < 0 {
                    break; // remaining captures all lose material by SEE
                }
                if !m.is_promo() {
                    let victim = if m.flag() == EP_CAPTURE { PAWN } else { b.piece_type_at(m.to()) };
                    if static_eval + SEE_VAL[victim] + 150 <= alpha {
                        continue; // delta pruning
                    }
                }
            }
            let mut nb = *b;
            if !nb.make_move(m) {
                continue;
            }
            legal += 1;
            let score = -self.qsearch(&nb, -beta, -alpha, ply + 1);
            if self.stop {
                return 0;
            }
            if score > best {
                best = score;
                if score > alpha {
                    alpha = score;
                    best_move = m;
                    if score >= beta {
                        break;
                    }
                }
            }
        }
        if in_check && legal == 0 {
            return -MATE + ply as i32;
        }
        let flag = if best >= beta {
            LOWER
        } else if alpha > orig_alpha {
            EXACT
        } else {
            UPPER
        };
        self.tt.store(b.hash, best_move, score_to_tt(best, ply), static_eval, 0, flag);
        best
    }
}
