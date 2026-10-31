//! Precomputed attack tables and sliding-piece attack generation.
//! Squares: a1 = 0, b1 = 1, ..., h8 = 63.

pub const FILE_A: u64 = 0x0101_0101_0101_0101;
pub const FILE_H: u64 = FILE_A << 7;
pub const RANK_1: u64 = 0xFF;
pub const RANK_2: u64 = RANK_1 << 8;
pub const RANK_7: u64 = RANK_1 << 48;
pub const RANK_8: u64 = RANK_1 << 56;

const fn gen_leaper(deltas: &[(i32, i32)]) -> [u64; 64] {
    let mut t = [0u64; 64];
    let mut sq = 0;
    while sq < 64 {
        let r = (sq / 8) as i32;
        let f = (sq % 8) as i32;
        let mut i = 0;
        while i < deltas.len() {
            let nr = r + deltas[i].0;
            let nf = f + deltas[i].1;
            if nr >= 0 && nr < 8 && nf >= 0 && nf < 8 {
                t[sq] |= 1u64 << (nr * 8 + nf);
            }
            i += 1;
        }
        sq += 1;
    }
    t
}

pub static KNIGHT_ATT: [u64; 64] =
    gen_leaper(&[(1, 2), (2, 1), (2, -1), (1, -2), (-1, -2), (-2, -1), (-2, 1), (-1, 2)]);
pub static KING_ATT: [u64; 64] =
    gen_leaper(&[(1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1), (0, -1), (1, -1)]);
/// PAWN_ATT[color][sq] = squares attacked by a pawn of `color` standing on `sq`.
pub static PAWN_ATT: [[u64; 64]; 2] = [gen_leaper(&[(1, -1), (1, 1)]), gen_leaper(&[(-1, -1), (-1, 1)])];

// Ray directions: 0 N, 1 S, 2 E, 3 W, 4 NE, 5 NW, 6 SE, 7 SW
const DIRS: [(i32, i32); 8] = [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)];

const fn gen_rays() -> [[u64; 64]; 8] {
    let mut t = [[0u64; 64]; 8];
    let mut d = 0;
    while d < 8 {
        let mut sq = 0;
        while sq < 64 {
            let mut r = (sq / 8) as i32 + DIRS[d].0;
            let mut f = (sq % 8) as i32 + DIRS[d].1;
            while r >= 0 && r < 8 && f >= 0 && f < 8 {
                t[d][sq] |= 1u64 << (r * 8 + f);
                r += DIRS[d].0;
                f += DIRS[d].1;
            }
            sq += 1;
        }
        d += 1;
    }
    t
}

pub static RAYS: [[u64; 64]; 8] = gen_rays();

#[inline(always)]
fn pos_ray(d: usize, sq: usize, occ: u64) -> u64 {
    let a = RAYS[d][sq];
    let b = a & occ;
    if b != 0 {
        a ^ RAYS[d][b.trailing_zeros() as usize]
    } else {
        a
    }
}

#[inline(always)]
fn neg_ray(d: usize, sq: usize, occ: u64) -> u64 {
    let a = RAYS[d][sq];
    let b = a & occ;
    if b != 0 {
        a ^ RAYS[d][63 - b.leading_zeros() as usize]
    } else {
        a
    }
}

#[inline(always)]
pub fn rook_attacks(sq: usize, occ: u64) -> u64 {
    pos_ray(0, sq, occ) | neg_ray(1, sq, occ) | pos_ray(2, sq, occ) | neg_ray(3, sq, occ)
}

#[inline(always)]
pub fn bishop_attacks(sq: usize, occ: u64) -> u64 {
    pos_ray(4, sq, occ) | pos_ray(5, sq, occ) | neg_ray(6, sq, occ) | neg_ray(7, sq, occ)
}

#[inline(always)]
pub fn queen_attacks(sq: usize, occ: u64) -> u64 {
    rook_attacks(sq, occ) | bishop_attacks(sq, occ)
}
