//! Tapered evaluation using the PeSTO piece-square tables (Ronald Friederich),
//! plus a few cheap terms. Score is returned from the side to move's point of view.

use crate::bitboard::*;
use crate::board::*;

const MG_VALUE: [i32; 6] = [82, 337, 365, 477, 1025, 0];
const EG_VALUE: [i32; 6] = [94, 281, 297, 512, 936, 0];
const PHASE_INC: [i32; 6] = [0, 1, 1, 2, 4, 0];

// Tables are written from White's view with a8 at index 0 (as printed).
#[rustfmt::skip]
const MG_PAWN: [i32; 64] = [
      0,   0,   0,   0,   0,   0,  0,   0,
     98, 134,  61,  95,  68, 126, 34, -11,
     -6,   7,  26,  31,  65,  56, 25, -20,
    -14,  13,   6,  21,  23,  12, 17, -23,
    -27,  -2,  -5,  12,  17,   6, 10, -25,
    -26,  -4,  -4, -10,   3,   3, 33, -12,
    -35,  -1, -20, -23, -15,  24, 38, -22,
      0,   0,   0,   0,   0,   0,  0,   0,
];
#[rustfmt::skip]
const EG_PAWN: [i32; 64] = [
      0,   0,   0,   0,   0,   0,   0,   0,
    178, 173, 158, 134, 147, 132, 165, 187,
     94, 100,  85,  67,  56,  53,  82,  84,
     32,  24,  13,   5,  -2,   4,  17,  17,
     13,   9,  -3,  -7,  -7,  -8,   3,  -1,
      4,   7,  -6,   1,   0,  -5,  -1,  -8,
     13,   8,   8,  10,  13,   0,   2,  -7,
      0,   0,   0,   0,   0,   0,   0,   0,
];
#[rustfmt::skip]
const MG_KNIGHT: [i32; 64] = [
    -167, -89, -34, -49,  61, -97, -15, -107,
     -73, -41,  72,  36,  23,  62,   7,  -17,
     -47,  60,  37,  65,  84, 129,  73,   44,
      -9,  17,  19,  53,  37,  69,  18,   22,
     -13,   4,  16,  13,  28,  19,  21,   -8,
     -23,  -9,  12,  10,  19,  17,  25,  -16,
     -29, -53, -12,  -3,  -1,  18, -14,  -19,
    -105, -21, -58, -33, -17, -28, -19,  -23,
];
#[rustfmt::skip]
const EG_KNIGHT: [i32; 64] = [
    -58, -38, -13, -28, -31, -27, -63, -99,
    -25,  -8, -25,  -2,  -9, -25, -24, -52,
    -24, -20,  10,   9,  -1,  -9, -19, -41,
    -17,   3,  22,  22,  22,  11,   8, -18,
    -18,  -6,  16,  25,  16,  17,   4, -18,
    -23,  -3,  -1,  15,  10,  -3, -20, -22,
    -42, -20, -10,  -5,  -2, -20, -23, -44,
    -29, -51, -23, -15, -22, -18, -50, -64,
];
#[rustfmt::skip]
const MG_BISHOP: [i32; 64] = [
    -29,   4, -82, -37, -25, -42,   7,  -8,
    -26,  16, -18, -13,  30,  59,  18, -47,
    -16,  37,  43,  40,  35,  50,  37,  -2,
     -4,   5,  19,  50,  37,  37,   7,  -2,
     -6,  13,  13,  26,  34,  12,  10,   4,
      0,  15,  15,  15,  14,  27,  18,  10,
      4,  15,  16,   0,   7,  21,  33,   1,
    -33,  -3, -14, -21, -13, -12, -39, -21,
];
#[rustfmt::skip]
const EG_BISHOP: [i32; 64] = [
    -14, -21, -11,  -8, -7,  -9, -17, -24,
     -8,  -4,   7, -12, -3, -13,  -4, -14,
      2,  -8,   0,  -1, -2,   6,   0,   4,
     -3,   9,  12,   9, 14,  10,   3,   2,
     -6,   3,  13,  19,  7,  10,  -3,  -9,
    -12,  -3,   8,  10, 13,   3,  -7, -15,
    -14, -18,  -7,  -1,  4,  -9, -15, -27,
    -23,  -9, -23,  -5, -9, -16,  -5, -17,
];
#[rustfmt::skip]
const MG_ROOK: [i32; 64] = [
     32,  42,  32,  51, 63,  9,  31,  43,
     27,  32,  58,  62, 80, 67,  26,  44,
     -5,  19,  26,  36, 17, 45,  61,  16,
    -24, -11,   7,  26, 24, 35,  -8, -20,
    -36, -26, -12,  -1,  9, -7,   6, -23,
    -45, -25, -16, -17,  3,  0,  -5, -33,
    -44, -16, -20,  -9, -1, 11,  -6, -71,
    -19, -13,   1,  17, 16,  7, -37, -26,
];
#[rustfmt::skip]
const EG_ROOK: [i32; 64] = [
    13, 10, 18, 15, 12,  12,   8,   5,
    11, 13, 13, 11, -3,   3,   8,   3,
     7,  7,  7,  5,  4,  -3,  -5,  -3,
     4,  3, 13,  1,  2,   1,  -1,   2,
     3,  5,  8,  4, -5,  -6,  -8, -11,
    -4,  0, -5, -1, -7, -12,  -8, -16,
    -6, -6,  0,  2, -9,  -9, -11,  -3,
    -9,  2,  3, -1, -5, -13,   4, -20,
];
#[rustfmt::skip]
const MG_QUEEN: [i32; 64] = [
    -28,   0,  29,  12,  59,  44,  43,  45,
    -24, -39,  -5,   1, -16,  57,  28,  54,
    -13, -17,   7,   8,  29,  56,  47,  57,
    -27, -27, -16, -16,  -1,  17,  -2,   1,
     -9, -26,  -9, -10,  -2,  -4,   3,  -3,
    -14,   2, -11,  -2,  -5,   2,  14,   5,
    -35,  -8,  11,   2,   8,  15,  -3,   1,
     -1, -18,  -9,  10, -15, -25, -31, -50,
];
#[rustfmt::skip]
const EG_QUEEN: [i32; 64] = [
     -9,  22,  22,  27,  27,  19,  10,  20,
    -17,  20,  32,  41,  58,  25,  30,   0,
    -20,   6,   9,  49,  47,  35,  19,   9,
      3,  22,  24,  45,  57,  40,  57,  36,
    -18,  28,  19,  47,  31,  34,  39,  23,
    -16, -27,  15,   6,   9,  17,  10,   5,
    -22, -23, -30, -16, -16, -23, -36, -32,
    -33, -28, -22, -43,  -5, -32, -20, -41,
];
#[rustfmt::skip]
const MG_KING: [i32; 64] = [
    -65,  23,  16, -15, -56, -34,   2,  13,
     29,  -1, -20,  -7,  -8,  -4, -38, -29,
     -9,  24,   2, -16, -20,   6,  22, -22,
    -17, -20, -12, -27, -30, -25, -14, -36,
    -49,  -1, -27, -39, -46, -44, -33, -51,
    -14, -14, -22, -46, -44, -30, -15, -27,
      1,   7,  -8, -64, -43, -16,   9,   8,
    -15,  36,  12, -54,   8, -28,  24,  14,
];
#[rustfmt::skip]
const EG_KING: [i32; 64] = [
    -74, -35, -18, -18, -11,  15,   4, -17,
    -12,  17,  14,  17,  17,  38,  23,  11,
     10,  17,  23,  15,  20,  45,  44,  13,
     -8,  22,  24,  27,  26,  33,  26,   3,
    -18,  -4,  21,  24,  27,  23,   9, -11,
    -19,  -3,  11,  21,  23,  16,   7,  -9,
    -27, -11,   4,  13,  14,   4,  -5, -17,
    -53, -34, -21, -11, -28, -14, -24, -43,
];

const MG_TABLES: [[i32; 64]; 6] = [MG_PAWN, MG_KNIGHT, MG_BISHOP, MG_ROOK, MG_QUEEN, MG_KING];
const EG_TABLES: [[i32; 64]; 6] = [EG_PAWN, EG_KNIGHT, EG_BISHOP, EG_ROOK, EG_QUEEN, EG_KING];

/// Builds [color][piece][square] tables indexed with a1 = 0.
const fn build(values: [i32; 6], tables: [[i32; 64]; 6]) -> [[[i32; 64]; 6]; 2] {
    let mut t = [[[0i32; 64]; 6]; 2];
    let mut p = 0;
    while p < 6 {
        let mut sq = 0;
        while sq < 64 {
            t[WHITE][p][sq] = values[p] + tables[p][sq ^ 56];
            t[BLACK][p][sq] = values[p] + tables[p][sq];
            sq += 1;
        }
        p += 1;
    }
    t
}

static MG: [[[i32; 64]; 6]; 2] = build(MG_VALUE, MG_TABLES);
static EG: [[[i32; 64]; 6]; 2] = build(EG_VALUE, EG_TABLES);

const TEMPO: i32 = 12;
// Passed pawn bonus by relative rank (0..7), endgame-weighted
const PASSED_MG: [i32; 8] = [0, 0, 5, 10, 20, 35, 60, 0];
const PASSED_EG: [i32; 8] = [0, 5, 10, 20, 40, 70, 110, 0];

const fn gen_passed_masks() -> [[u64; 64]; 2] {
    let mut m = [[0u64; 64]; 2];
    let mut sq = 0;
    while sq < 64 {
        let f = sq % 8;
        let r = sq / 8;
        let mut files = 0x0101_0101_0101_0101u64 << f;
        if f > 0 {
            files |= 0x0101_0101_0101_0101u64 << (f - 1);
        }
        if f < 7 {
            files |= 0x0101_0101_0101_0101u64 << (f + 1);
        }
        // white: ranks above r
        let above = if r == 7 { 0 } else { !0u64 << ((r + 1) * 8) };
        let below = if r == 0 { 0 } else { !0u64 >> ((8 - r) * 8) };
        m[WHITE][sq] = files & above;
        m[BLACK][sq] = files & below;
        sq += 1;
    }
    m
}
static PASSED_MASK: [[u64; 64]; 2] = gen_passed_masks();

#[inline(always)]
fn pawn_attacks(c: usize, pawns: u64) -> u64 {
    if c == WHITE {
        ((pawns << 7) & !FILE_H) | ((pawns << 9) & !FILE_A)
    } else {
        ((pawns >> 9) & !FILE_H) | ((pawns >> 7) & !FILE_A)
    }
}

pub fn evaluate(b: &Board) -> i32 {
    let mut mg = [0i32; 2];
    let mut eg = [0i32; 2];
    let mut phase = 0;
    let occ = b.occ();

    for c in 0..2 {
        for p in 0..6 {
            let mut bb = b.pieces[p] & b.colors[c];
            while bb != 0 {
                let sq = bb.trailing_zeros() as usize;
                bb &= bb - 1;
                mg[c] += MG[c][p][sq];
                eg[c] += EG[c][p][sq];
                phase += PHASE_INC[p];
            }
        }
        // bishop pair
        if (b.pieces[BISHOP] & b.colors[c]).count_ones() >= 2 {
            mg[c] += 30;
            eg[c] += 50;
        }
        // passed pawns
        let own_pawns = b.pieces[PAWN] & b.colors[c];
        let their_pawns = b.pieces[PAWN] & b.colors[c ^ 1];
        let mut bb = own_pawns;
        while bb != 0 {
            let sq = bb.trailing_zeros() as usize;
            bb &= bb - 1;
            if (PASSED_MASK[c][sq] & their_pawns) == 0 {
                let rel_rank = if c == WHITE { sq / 8 } else { 7 - sq / 8 };
                mg[c] += PASSED_MG[rel_rank];
                eg[c] += PASSED_EG[rel_rank];
            }
        }
        // rooks on open / semi-open files
        let mut rooks = b.pieces[ROOK] & b.colors[c];
        while rooks != 0 {
            let sq = rooks.trailing_zeros() as usize;
            rooks &= rooks - 1;
            let file = FILE_A << (sq % 8);
            if (file & own_pawns) == 0 {
                if (file & their_pawns) == 0 {
                    mg[c] += 25;
                    eg[c] += 10;
                } else {
                    mg[c] += 12;
                    eg[c] += 5;
                }
            }
        }
        // pawn structure: doubled and isolated pawns
        for f in 0..8 {
            let file = FILE_A << f;
            let n = (own_pawns & file).count_ones() as i32;
            if n == 0 {
                continue;
            }
            if n > 1 {
                mg[c] -= 10 * (n - 1);
                eg[c] -= 20 * (n - 1);
            }
            let adj = (if f > 0 { FILE_A << (f - 1) } else { 0 }) | (if f < 7 { FILE_A << (f + 1) } else { 0 });
            if (own_pawns & adj) == 0 {
                mg[c] -= 12 * n;
                eg[c] -= 16 * n;
            }
        }

        // mobility (squares not occupied by own pieces and not attacked by enemy pawns)
        // and attacks on the enemy king zone
        let enemy_pawn_att = pawn_attacks(c ^ 1, their_pawns);
        let safe = !b.colors[c] & !enemy_pawn_att;
        let their_king = b.king_sq(c ^ 1);
        let zone = KING_ATT[their_king] | (1u64 << their_king);
        let mut attack_units = 0;
        let mut attackers = 0;
        for (p, w_mg, w_eg, centre, unit) in [
            (KNIGHT, 4, 4, 4, 2),
            (BISHOP, 5, 5, 7, 2),
            (ROOK, 2, 4, 7, 3),
            (QUEEN, 1, 2, 14, 5),
        ] {
            let mut bb = b.pieces[p] & b.colors[c];
            while bb != 0 {
                let sq = bb.trailing_zeros() as usize;
                bb &= bb - 1;
                let att = match p {
                    KNIGHT => KNIGHT_ATT[sq],
                    BISHOP => bishop_attacks(sq, occ),
                    ROOK => rook_attacks(sq, occ),
                    _ => queen_attacks(sq, occ),
                };
                let n = (att & safe).count_ones() as i32;
                mg[c] += w_mg * (n - centre);
                eg[c] += w_eg * (n - centre);
                let z = (att & zone).count_ones() as i32;
                if z > 0 {
                    attackers += 1;
                    attack_units += unit * z;
                }
            }
        }
        // king attack only counts when the enemy queen side is real attacking material
        if attackers >= 2 && (b.pieces[QUEEN] & b.colors[c]) != 0 {
            mg[c] += (attack_units * attack_units / 2).min(400);
        }

        // pawn shield in front of our own castled king (middlegame only)
        let ksq = b.king_sq(c);
        let kf = ksq % 8;
        let kr = if c == WHITE { ksq / 8 } else { 7 - ksq / 8 };
        if kr <= 1 && kf != 3 && kf != 4 {
            for f in kf.saturating_sub(1)..=(kf + 1).min(7) {
                let file_pawns = own_pawns & (FILE_A << f);
                let one = if c == WHITE { 1u64 << ((kr + 1) * 8 + f) } else { 1u64 << ((6 - kr) * 8 + f) };
                let two = if c == WHITE { 1u64 << ((kr + 2) * 8 + f) } else { 1u64 << ((5 - kr) * 8 + f) };
                if (file_pawns & one) != 0 {
                } else if (file_pawns & two) != 0 {
                    mg[c] -= 10;
                } else {
                    mg[c] -= 25;
                }
            }
        }
    }

    let us = b.side;
    let them = us ^ 1;
    let mg_score = mg[us] - mg[them];
    let eg_score = eg[us] - eg[them];
    let phase = phase.min(24);
    let mut score = (mg_score * phase + eg_score * (24 - phase)) / 24 + TEMPO;

    // Scale down drawish endgames with no pawns where the stronger side has little extra material
    let strong = if score > 0 { us } else { them };
    if (b.pieces[PAWN] & b.colors[strong]) == 0 {
        let non_pawn = |c: usize| -> i32 {
            (b.pieces[KNIGHT] & b.colors[c]).count_ones() as i32 * 3
                + (b.pieces[BISHOP] & b.colors[c]).count_ones() as i32 * 3
                + (b.pieces[ROOK] & b.colors[c]).count_ones() as i32 * 5
                + (b.pieces[QUEEN] & b.colors[c]).count_ones() as i32 * 9
        };
        if non_pawn(strong) - non_pawn(strong ^ 1) <= 3 {
            score /= 8;
        }
    }
    score
}
