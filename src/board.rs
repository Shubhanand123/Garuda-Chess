//! Board representation (bitboards + mailbox), Zobrist hashing,
//! legal move generation (copy-make), SEE and perft.

use crate::bitboard::*;

pub const WHITE: usize = 0;
pub const BLACK: usize = 1;

pub const PAWN: usize = 0;
pub const KNIGHT: usize = 1;
pub const BISHOP: usize = 2;
pub const ROOK: usize = 3;
pub const QUEEN: usize = 4;
pub const KING: usize = 5;
pub const NO_PIECE: usize = 6;

pub const EMPTY: u8 = 12;
pub const NO_SQ: u8 = 64;

pub const START_FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

// Move flags (upper 4 bits)
pub const QUIET: u16 = 0;
pub const DOUBLE_PUSH: u16 = 1;
pub const KING_CASTLE: u16 = 2;
pub const QUEEN_CASTLE: u16 = 3;
pub const CAPTURE: u16 = 4;
pub const EP_CAPTURE: u16 = 5;
pub const PROMO: u16 = 8; // + 0..3 => N, B, R, Q
pub const PROMO_CAPTURE: u16 = 12; // + 0..3

/// Piece values used by static exchange evaluation.
pub const SEE_VAL: [i32; 7] = [100, 320, 330, 500, 950, 20000, 0];

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub struct Move(pub u16);

impl Move {
    pub const NONE: Move = Move(0);

    #[inline(always)]
    pub fn new(from: usize, to: usize, flag: u16) -> Move {
        Move(from as u16 | (to as u16) << 6 | flag << 12)
    }
    #[inline(always)]
    pub fn from(self) -> usize {
        (self.0 & 63) as usize
    }
    #[inline(always)]
    pub fn to(self) -> usize {
        ((self.0 >> 6) & 63) as usize
    }
    #[inline(always)]
    pub fn flag(self) -> u16 {
        self.0 >> 12
    }
    #[inline(always)]
    pub fn is_capture(self) -> bool {
        (self.flag() & 4) != 0
    }
    #[inline(always)]
    pub fn is_promo(self) -> bool {
        (self.flag() & 8) != 0
    }
    #[inline(always)]
    pub fn promo_piece(self) -> usize {
        (self.flag() & 3) as usize + 1
    }
    pub fn to_uci(self) -> String {
        if self == Move::NONE {
            return "0000".to_string();
        }
        let mut s = format!("{}{}", sq_name(self.from()), sq_name(self.to()));
        if self.is_promo() {
            s.push(['n', 'b', 'r', 'q'][self.promo_piece() - 1]);
        }
        s
    }
}

pub struct MoveList {
    pub moves: [Move; 256],
    pub len: usize,
}

impl MoveList {
    #[inline(always)]
    pub fn new() -> Self {
        MoveList { moves: [Move::NONE; 256], len: 0 }
    }
    #[inline(always)]
    pub fn push(&mut self, m: Move) {
        self.moves[self.len] = m;
        self.len += 1;
    }
}

impl Default for MoveList {
    fn default() -> Self {
        Self::new()
    }
}

pub fn sq_name(sq: usize) -> String {
    format!("{}{}", (b'a' + (sq % 8) as u8) as char, (b'1' + (sq / 8) as u8) as char)
}

pub fn parse_sq(s: &str) -> Option<usize> {
    let b = s.as_bytes();
    if b.len() != 2 {
        return None;
    }
    let f = b[0].wrapping_sub(b'a');
    let r = b[1].wrapping_sub(b'1');
    if f < 8 && r < 8 {
        Some((r * 8 + f) as usize)
    } else {
        None
    }
}

// ---------------- Zobrist ----------------

pub struct Zobrist {
    pub psq: [[u64; 64]; 12],
    pub castling: [u64; 16],
    pub ep: [u64; 8],
    pub side: u64,
}

const fn xs(mut s: u64) -> u64 {
    s ^= s << 13;
    s ^= s >> 7;
    s ^= s << 17;
    s
}

const fn gen_zobrist() -> Zobrist {
    let mut s: u64 = 0x2545_F491_4F6C_DD1D;
    let mut psq = [[0u64; 64]; 12];
    let mut i = 0;
    while i < 12 {
        let mut j = 0;
        while j < 64 {
            s = xs(s);
            psq[i][j] = s;
            j += 1;
        }
        i += 1;
    }
    let mut castling = [0u64; 16];
    i = 0;
    while i < 16 {
        s = xs(s);
        castling[i] = s;
        i += 1;
    }
    let mut ep = [0u64; 8];
    i = 0;
    while i < 8 {
        s = xs(s);
        ep[i] = s;
        i += 1;
    }
    s = xs(s);
    Zobrist { psq, castling, ep, side: s }
}

pub static ZOB: Zobrist = gen_zobrist();

// Castling rights: 1 = white king side, 2 = white queen side, 4 = black king side, 8 = black queen side
const fn gen_castle_mask() -> [u8; 64] {
    let mut m = [15u8; 64];
    m[0] = 13; // a1
    m[7] = 14; // h1
    m[4] = 12; // e1
    m[56] = 7; // a8
    m[63] = 11; // h8
    m[60] = 3; // e8
    m
}
static CASTLE_MASK: [u8; 64] = gen_castle_mask();

// ---------------- Board ----------------

#[derive(Clone, Copy)]
pub struct Board {
    pub pieces: [u64; 6],
    pub colors: [u64; 2],
    pub mailbox: [u8; 64],
    pub side: usize,
    pub castling: u8,
    pub ep: u8,
    pub halfmove: u16,
    pub fullmove: u16,
    pub hash: u64,
}

impl Board {
    pub fn empty() -> Board {
        Board {
            pieces: [0; 6],
            colors: [0; 2],
            mailbox: [EMPTY; 64],
            side: WHITE,
            castling: 0,
            ep: NO_SQ,
            halfmove: 0,
            fullmove: 1,
            hash: 0,
        }
    }

    pub fn startpos() -> Board {
        Board::from_fen(START_FEN).unwrap()
    }

    #[inline(always)]
    pub fn occ(&self) -> u64 {
        self.colors[0] | self.colors[1]
    }

    #[inline(always)]
    fn put(&mut self, sq: usize, c: usize, p: usize) {
        let b = 1u64 << sq;
        self.pieces[p] |= b;
        self.colors[c] |= b;
        self.mailbox[sq] = (c * 6 + p) as u8;
        self.hash ^= ZOB.psq[c * 6 + p][sq];
    }

    #[inline(always)]
    fn remove(&mut self, sq: usize) {
        let code = self.mailbox[sq];
        if code == EMPTY {
            return;
        }
        let c = code as usize / 6;
        let p = code as usize % 6;
        let b = !(1u64 << sq);
        self.pieces[p] &= b;
        self.colors[c] &= b;
        self.mailbox[sq] = EMPTY;
        self.hash ^= ZOB.psq[code as usize][sq];
    }

    #[inline(always)]
    pub fn piece_type_at(&self, sq: usize) -> usize {
        let m = self.mailbox[sq];
        if m == EMPTY {
            NO_PIECE
        } else {
            m as usize % 6
        }
    }

    #[inline(always)]
    pub fn king_sq(&self, c: usize) -> usize {
        (self.pieces[KING] & self.colors[c]).trailing_zeros() as usize
    }

    #[inline(always)]
    pub fn attackers_to(&self, sq: usize, occ: u64) -> u64 {
        (PAWN_ATT[BLACK][sq] & self.pieces[PAWN] & self.colors[WHITE])
            | (PAWN_ATT[WHITE][sq] & self.pieces[PAWN] & self.colors[BLACK])
            | (KNIGHT_ATT[sq] & self.pieces[KNIGHT])
            | (KING_ATT[sq] & self.pieces[KING])
            | (bishop_attacks(sq, occ) & (self.pieces[BISHOP] | self.pieces[QUEEN]))
            | (rook_attacks(sq, occ) & (self.pieces[ROOK] | self.pieces[QUEEN]))
    }

    #[inline(always)]
    pub fn is_attacked(&self, sq: usize, by: usize) -> bool {
        let them = self.colors[by];
        if (PAWN_ATT[by ^ 1][sq] & self.pieces[PAWN] & them) != 0 {
            return true;
        }
        if (KNIGHT_ATT[sq] & self.pieces[KNIGHT] & them) != 0 {
            return true;
        }
        if (KING_ATT[sq] & self.pieces[KING] & them) != 0 {
            return true;
        }
        let occ = self.occ();
        let bq = (self.pieces[BISHOP] | self.pieces[QUEEN]) & them;
        if bq != 0 && (bishop_attacks(sq, occ) & bq) != 0 {
            return true;
        }
        let rq = (self.pieces[ROOK] | self.pieces[QUEEN]) & them;
        rq != 0 && (rook_attacks(sq, occ) & rq) != 0
    }

    #[inline(always)]
    pub fn in_check(&self) -> bool {
        self.is_attacked(self.king_sq(self.side), self.side ^ 1)
    }

    #[inline(always)]
    pub fn has_non_pawn(&self, c: usize) -> bool {
        ((self.pieces[KNIGHT] | self.pieces[BISHOP] | self.pieces[ROOK] | self.pieces[QUEEN]) & self.colors[c]) != 0
    }

    pub fn insufficient_material(&self) -> bool {
        if (self.pieces[PAWN] | self.pieces[ROOK] | self.pieces[QUEEN]) != 0 {
            return false;
        }
        (self.pieces[KNIGHT] | self.pieces[BISHOP]).count_ones() <= 1
    }

    // ---------------- FEN ----------------

    pub fn from_fen(fen: &str) -> Result<Board, String> {
        let parts: Vec<&str> = fen.split_whitespace().collect();
        if parts.len() < 4 {
            return Err("FEN needs at least 4 fields".into());
        }
        let mut b = Board::empty();
        let mut rank: i32 = 7;
        let mut file: i32 = 0;
        for ch in parts[0].chars() {
            match ch {
                '/' => {
                    rank -= 1;
                    file = 0;
                }
                '1'..='8' => file += ch as i32 - '0' as i32,
                _ => {
                    let (c, p) = match ch {
                        'P' => (WHITE, PAWN),
                        'N' => (WHITE, KNIGHT),
                        'B' => (WHITE, BISHOP),
                        'R' => (WHITE, ROOK),
                        'Q' => (WHITE, QUEEN),
                        'K' => (WHITE, KING),
                        'p' => (BLACK, PAWN),
                        'n' => (BLACK, KNIGHT),
                        'b' => (BLACK, BISHOP),
                        'r' => (BLACK, ROOK),
                        'q' => (BLACK, QUEEN),
                        'k' => (BLACK, KING),
                        _ => return Err(format!("bad piece char '{}'", ch)),
                    };
                    if !(0..8).contains(&rank) || !(0..8).contains(&file) {
                        return Err("board overflow".into());
                    }
                    b.put((rank * 8 + file) as usize, c, p);
                    file += 1;
                }
            }
        }
        if (b.pieces[KING] & b.colors[WHITE]).count_ones() != 1 || (b.pieces[KING] & b.colors[BLACK]).count_ones() != 1 {
            return Err("each side needs exactly one king".into());
        }
        b.side = match parts[1] {
            "w" => WHITE,
            "b" => BLACK,
            _ => return Err("bad side to move".into()),
        };
        for ch in parts[2].chars() {
            match ch {
                'K' if b.mailbox[4] == 5 && b.mailbox[7] == 3 => b.castling |= 1,
                'Q' if b.mailbox[4] == 5 && b.mailbox[0] == 3 => b.castling |= 2,
                'k' if b.mailbox[60] == 11 && b.mailbox[63] == 9 => b.castling |= 4,
                'q' if b.mailbox[60] == 11 && b.mailbox[56] == 9 => b.castling |= 8,
                _ => {}
            }
        }
        if parts[3] != "-" {
            if let Some(sq) = parse_sq(parts[3]) {
                // only record e.p. square if a capture is actually possible (keeps hashes canonical)
                if (PAWN_ATT[b.side ^ 1][sq] & b.pieces[PAWN] & b.colors[b.side]) != 0 {
                    b.ep = sq as u8;
                }
            }
        }
        b.halfmove = parts.get(4).and_then(|s| s.parse().ok()).unwrap_or(0);
        b.fullmove = parts.get(5).and_then(|s| s.parse().ok()).unwrap_or(1);
        b.hash ^= ZOB.castling[b.castling as usize];
        if b.ep != NO_SQ {
            b.hash ^= ZOB.ep[(b.ep & 7) as usize];
        }
        if b.side == BLACK {
            b.hash ^= ZOB.side;
        }
        if b.is_attacked(b.king_sq(b.side ^ 1), b.side) {
            return Err("side not to move is in check".into());
        }
        Ok(b)
    }

    pub fn to_fen(&self) -> String {
        let chars = ['P', 'N', 'B', 'R', 'Q', 'K', 'p', 'n', 'b', 'r', 'q', 'k'];
        let mut s = String::new();
        for rank in (0..8).rev() {
            let mut empty = 0;
            for file in 0..8 {
                let m = self.mailbox[rank * 8 + file];
                if m == EMPTY {
                    empty += 1;
                } else {
                    if empty > 0 {
                        s.push_str(&empty.to_string());
                        empty = 0;
                    }
                    s.push(chars[m as usize]);
                }
            }
            if empty > 0 {
                s.push_str(&empty.to_string());
            }
            if rank > 0 {
                s.push('/');
            }
        }
        s.push_str(if self.side == WHITE { " w " } else { " b " });
        if self.castling == 0 {
            s.push('-');
        } else {
            if self.castling & 1 != 0 {
                s.push('K');
            }
            if self.castling & 2 != 0 {
                s.push('Q');
            }
            if self.castling & 4 != 0 {
                s.push('k');
            }
            if self.castling & 8 != 0 {
                s.push('q');
            }
        }
        s.push(' ');
        if self.ep == NO_SQ {
            s.push('-');
        } else {
            s.push_str(&sq_name(self.ep as usize));
        }
        s.push_str(&format!(" {} {}", self.halfmove, self.fullmove));
        s
    }

    pub fn pretty(&self) -> String {
        let chars = ['P', 'N', 'B', 'R', 'Q', 'K', 'p', 'n', 'b', 'r', 'q', 'k'];
        let mut s = String::new();
        for rank in (0..8).rev() {
            s.push_str(&format!(" {} ", rank + 1));
            for file in 0..8 {
                let m = self.mailbox[rank * 8 + file];
                s.push(' ');
                s.push(if m == EMPTY { '.' } else { chars[m as usize] });
            }
            s.push('\n');
        }
        s.push_str("    a b c d e f g h\n");
        s.push_str(&format!("FEN: {}\nKey: {:016x}\n", self.to_fen(), self.hash));
        s
    }

    // ---------------- Make move ----------------

    /// Applies a pseudo-legal move. Returns false if it leaves our king in check
    /// (the board is then garbage and must be discarded — we use copy-make).
    #[inline]
    pub fn make_move(&mut self, m: Move) -> bool {
        let from = m.from();
        let to = m.to();
        let flag = m.flag();
        let us = self.side;
        let them = us ^ 1;
        let pc = self.mailbox[from] as usize % 6;

        if self.ep != NO_SQ {
            self.hash ^= ZOB.ep[(self.ep & 7) as usize];
            self.ep = NO_SQ;
        }
        self.hash ^= ZOB.castling[self.castling as usize];
        self.halfmove += 1;
        if pc == PAWN {
            self.halfmove = 0;
        }

        if flag == EP_CAPTURE {
            let cap = if us == WHITE { to - 8 } else { to + 8 };
            self.remove(cap);
        } else if m.is_capture() {
            self.remove(to);
            self.halfmove = 0;
        }
        self.remove(from);
        let placed = if m.is_promo() { m.promo_piece() } else { pc };
        self.put(to, us, placed);

        match flag {
            DOUBLE_PUSH => {
                let ep = if us == WHITE { from + 8 } else { from - 8 };
                if (PAWN_ATT[us][ep] & self.pieces[PAWN] & self.colors[them]) != 0 {
                    self.ep = ep as u8;
                    self.hash ^= ZOB.ep[ep & 7];
                }
            }
            KING_CASTLE => {
                self.remove(to + 1);
                self.put(to - 1, us, ROOK);
            }
            QUEEN_CASTLE => {
                self.remove(to - 2);
                self.put(to + 1, us, ROOK);
            }
            _ => {}
        }

        self.castling &= CASTLE_MASK[from] & CASTLE_MASK[to];
        self.hash ^= ZOB.castling[self.castling as usize];
        self.side = them;
        self.hash ^= ZOB.side;
        if us == BLACK {
            self.fullmove += 1;
        }
        !self.is_attacked(self.king_sq(us), them)
    }

    #[inline]
    pub fn make_null(&mut self) {
        if self.ep != NO_SQ {
            self.hash ^= ZOB.ep[(self.ep & 7) as usize];
            self.ep = NO_SQ;
        }
        self.side ^= 1;
        self.hash ^= ZOB.side;
        self.halfmove = 0; // stops repetition look-back across the null move
    }

    // ---------------- Move generation ----------------

    /// Generates pseudo-legal moves. With `captures_only`, generates captures and queen promotions.
    pub fn gen_moves(&self, list: &mut MoveList, captures_only: bool) {
        let us = self.side;
        let them = us ^ 1;
        let own = self.colors[us];
        let enemy = self.colors[them];
        let occ = own | enemy;
        let empty = !occ;
        let targets = if captures_only { enemy } else { !own };

        // Pawns
        let pawns = self.pieces[PAWN] & own;
        let (up, promo_rank, start_rank): (i32, u64, u64) =
            if us == WHITE { (8, RANK_8, RANK_2) } else { (-8, RANK_1, RANK_7) };
        let mut bb = pawns;
        while bb != 0 {
            let from = bb.trailing_zeros() as usize;
            bb &= bb - 1;
            let to = (from as i32 + up) as usize;
            if ((1u64 << to) & empty) != 0 {
                if ((1u64 << to) & promo_rank) != 0 {
                    list.push(Move::new(from, to, PROMO + 3));
                    if !captures_only {
                        list.push(Move::new(from, to, PROMO));
                        list.push(Move::new(from, to, PROMO + 1));
                        list.push(Move::new(from, to, PROMO + 2));
                    }
                } else if !captures_only {
                    list.push(Move::new(from, to, QUIET));
                    if ((1u64 << from) & start_rank) != 0 {
                        let to2 = (to as i32 + up) as usize;
                        if ((1u64 << to2) & empty) != 0 {
                            list.push(Move::new(from, to2, DOUBLE_PUSH));
                        }
                    }
                }
            }
            let mut caps = PAWN_ATT[us][from] & enemy;
            while caps != 0 {
                let t = caps.trailing_zeros() as usize;
                caps &= caps - 1;
                if ((1u64 << t) & promo_rank) != 0 {
                    list.push(Move::new(from, t, PROMO_CAPTURE + 3));
                    list.push(Move::new(from, t, PROMO_CAPTURE));
                    list.push(Move::new(from, t, PROMO_CAPTURE + 1));
                    list.push(Move::new(from, t, PROMO_CAPTURE + 2));
                } else {
                    list.push(Move::new(from, t, CAPTURE));
                }
            }
        }
        if self.ep != NO_SQ {
            let ep = self.ep as usize;
            let mut att = PAWN_ATT[them][ep] & pawns;
            while att != 0 {
                let from = att.trailing_zeros() as usize;
                att &= att - 1;
                list.push(Move::new(from, ep, EP_CAPTURE));
            }
        }

        // Knights
        let mut bb = self.pieces[KNIGHT] & own;
        while bb != 0 {
            let from = bb.trailing_zeros() as usize;
            bb &= bb - 1;
            push_targets(list, from, KNIGHT_ATT[from] & targets, enemy);
        }
        // Bishops
        let mut bb = self.pieces[BISHOP] & own;
        while bb != 0 {
            let from = bb.trailing_zeros() as usize;
            bb &= bb - 1;
            push_targets(list, from, bishop_attacks(from, occ) & targets, enemy);
        }
        // Rooks
        let mut bb = self.pieces[ROOK] & own;
        while bb != 0 {
            let from = bb.trailing_zeros() as usize;
            bb &= bb - 1;
            push_targets(list, from, rook_attacks(from, occ) & targets, enemy);
        }
        // Queens
        let mut bb = self.pieces[QUEEN] & own;
        while bb != 0 {
            let from = bb.trailing_zeros() as usize;
            bb &= bb - 1;
            push_targets(list, from, queen_attacks(from, occ) & targets, enemy);
        }
        // King
        let ksq = self.king_sq(us);
        push_targets(list, ksq, KING_ATT[ksq] & targets, enemy);

        // Castling
        if !captures_only && self.castling != 0 {
            if us == WHITE {
                if (self.castling & 1) != 0
                    && (occ & 0x60) == 0
                    && !self.is_attacked(4, them)
                    && !self.is_attacked(5, them)
                    && !self.is_attacked(6, them)
                {
                    list.push(Move::new(4, 6, KING_CASTLE));
                }
                if (self.castling & 2) != 0
                    && (occ & 0x0E) == 0
                    && !self.is_attacked(4, them)
                    && !self.is_attacked(3, them)
                    && !self.is_attacked(2, them)
                {
                    list.push(Move::new(4, 2, QUEEN_CASTLE));
                }
            } else {
                if (self.castling & 4) != 0
                    && (occ & (0x60u64 << 56)) == 0
                    && !self.is_attacked(60, them)
                    && !self.is_attacked(61, them)
                    && !self.is_attacked(62, them)
                {
                    list.push(Move::new(60, 62, KING_CASTLE));
                }
                if (self.castling & 8) != 0
                    && (occ & (0x0Eu64 << 56)) == 0
                    && !self.is_attacked(60, them)
                    && !self.is_attacked(59, them)
                    && !self.is_attacked(58, them)
                {
                    list.push(Move::new(60, 58, QUEEN_CASTLE));
                }
            }
        }
    }

    pub fn legal_moves(&self) -> Vec<Move> {
        let mut list = MoveList::new();
        self.gen_moves(&mut list, false);
        let mut out = Vec::with_capacity(list.len);
        for i in 0..list.len {
            let mut b = *self;
            if b.make_move(list.moves[i]) {
                out.push(list.moves[i]);
            }
        }
        out
    }

    /// Standard algebraic notation for a legal move (e.g. Nbd7, exd5, O-O, e8=Q+, Qxf7#).
    pub fn san(&self, m: Move) -> String {
        let from = m.from();
        let to = m.to();
        let flag = m.flag();
        let mut s = String::new();
        if flag == KING_CASTLE {
            s.push_str("O-O");
        } else if flag == QUEEN_CASTLE {
            s.push_str("O-O-O");
        } else {
            let pt = self.piece_type_at(from);
            if pt == PAWN {
                if m.is_capture() {
                    s.push((b'a' + (from % 8) as u8) as char);
                    s.push('x');
                }
                s.push_str(&sq_name(to));
                if m.is_promo() {
                    s.push('=');
                    s.push(['N', 'B', 'R', 'Q'][m.promo_piece() - 1]);
                }
            } else {
                s.push(['P', 'N', 'B', 'R', 'Q', 'K'][pt]);
                let others: Vec<usize> = self
                    .legal_moves()
                    .into_iter()
                    .filter(|o| o.to() == to && o.from() != from && self.piece_type_at(o.from()) == pt)
                    .map(|o| o.from())
                    .collect();
                if !others.is_empty() {
                    let same_file = others.iter().any(|&f| f % 8 == from % 8);
                    let same_rank = others.iter().any(|&f| f / 8 == from / 8);
                    if !same_file {
                        s.push((b'a' + (from % 8) as u8) as char);
                    } else if !same_rank {
                        s.push((b'1' + (from / 8) as u8) as char);
                    } else {
                        s.push_str(&sq_name(from));
                    }
                }
                if m.is_capture() {
                    s.push('x');
                }
                s.push_str(&sq_name(to));
            }
        }
        let mut nb = *self;
        nb.make_move(m);
        if nb.in_check() {
            s.push(if nb.legal_moves().is_empty() { '#' } else { '+' });
        }
        s
    }

    pub fn parse_move(&self, s: &str) -> Option<Move> {
        self.legal_moves().into_iter().find(|m| m.to_uci() == s)
    }

    // ---------------- SEE ----------------

    /// Static exchange evaluation: is the material outcome of `m` >= threshold?
    pub fn see_ge(&self, m: Move, threshold: i32) -> bool {
        let flag = m.flag();
        if flag == KING_CASTLE || flag == QUEEN_CASTLE || flag == EP_CAPTURE || m.is_promo() {
            return 0 >= threshold;
        }
        let from = m.from();
        let to = m.to();
        let mut swap = SEE_VAL[self.piece_type_at(to)] - threshold;
        if swap < 0 {
            return false;
        }
        swap = SEE_VAL[self.piece_type_at(from)] - swap;
        if swap <= 0 {
            return true;
        }
        let mut occ = self.occ() ^ (1u64 << from) ^ (1u64 << to);
        let mut stm = self.side;
        let mut attackers = self.attackers_to(to, occ);
        let bq = self.pieces[BISHOP] | self.pieces[QUEEN];
        let rq = self.pieces[ROOK] | self.pieces[QUEEN];
        let mut res: i32 = 1;
        loop {
            stm ^= 1;
            attackers &= occ;
            let stm_att = attackers & self.colors[stm];
            if stm_att == 0 {
                break;
            }
            res ^= 1;
            let mut pt = PAWN;
            let mut bb = 0u64;
            while pt <= KING {
                bb = stm_att & self.pieces[pt];
                if bb != 0 {
                    break;
                }
                pt += 1;
            }
            if pt == KING {
                return if (attackers & !self.colors[stm]) != 0 { (res ^ 1) != 0 } else { res != 0 };
            }
            swap = SEE_VAL[pt] - swap;
            if swap < res {
                break;
            }
            occ ^= bb & bb.wrapping_neg();
            if pt == PAWN || pt == BISHOP || pt == QUEEN {
                attackers |= bishop_attacks(to, occ) & bq;
            }
            if pt == ROOK || pt == QUEEN {
                attackers |= rook_attacks(to, occ) & rq;
            }
        }
        res != 0
    }

    // ---------------- Perft ----------------

    pub fn perft(&self, depth: u32) -> u64 {
        if depth == 0 {
            return 1;
        }
        let mut list = MoveList::new();
        self.gen_moves(&mut list, false);
        let mut n = 0;
        for i in 0..list.len {
            let mut b = *self;
            if b.make_move(list.moves[i]) {
                n += if depth == 1 { 1 } else { b.perft(depth - 1) };
            }
        }
        n
    }
}

#[inline(always)]
fn push_targets(list: &mut MoveList, from: usize, mut t: u64, enemy: u64) {
    while t != 0 {
        let to = t.trailing_zeros() as usize;
        t &= t - 1;
        let flag = if ((enemy >> to) & 1) != 0 { CAPTURE } else { QUIET };
        list.push(Move::new(from, to, flag));
    }
}
