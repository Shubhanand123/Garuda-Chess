"""Engine A vs engine B match. Usage: selfplay.py A B movetime_ms [rounds]
Each opening is played twice with colors swapped. Prints score and Elo estimate."""
import chess, chess.engine, math, sys
from concurrent.futures import ThreadPoolExecutor

A, B = sys.argv[1], sys.argv[2]
MT = float(sys.argv[3]) / 1000
ROUNDS = int(sys.argv[4]) if len(sys.argv) > 4 else 1

OPENINGS = [l.split() for l in """e2e4 e7e5 g1f3 b8c6 f1b5 a7a6
e2e4 e7e5 g1f3 b8c6 f1c4 f8c5
e2e4 c7c5 g1f3 d7d6 d2d4 c5d4
e2e4 c7c5 b1c3 b8c6 g2g3 g7g6
e2e4 e7e6 d2d4 d7d5 b1c3 g8f6
e2e4 c7c6 d2d4 d7d5 e4e5 c8f5
e2e4 d7d5 e4d5 d8d5 b1c3 d5a5
e2e4 g8f6 e4e5 f6d5 d2d4 d7d6
e2e4 d7d6 d2d4 g8f6 b1c3 g7g6
d2d4 d7d5 c2c4 e7e6 b1c3 g8f6
d2d4 d7d5 c2c4 c7c6 g1f3 g8f6
d2d4 d7d5 c2c4 d5c4 g1f3 g8f6
d2d4 g8f6 c2c4 g7g6 b1c3 f8g7
d2d4 g8f6 c2c4 e7e6 b1c3 f8b4
d2d4 g8f6 c2c4 e7e6 g1f3 b7b6
d2d4 g8f6 c2c4 c7c5 d4d5 b7b5
d2d4 f7f5 g2g3 g8f6 f1g2 g7g6
c2c4 e7e5 b1c3 g8f6 g1f3 b8c6
c2c4 c7c5 g1f3 g8f6 b1c3 b8c6
g1f3 d7d5 g2g3 g8f6 f1g2 c7c6
e2e4 e7e5 f2f4 e5f4 g1f3 g7g5
e2e4 e7e5 d2d4 e5d4 g1f3 b8c6
e2e4 c7c5 c2c3 g8f6 e4e5 f6d5
d2d4 d7d5 c1f4 g8f6 e2e3 c7c5""".splitlines()]


def game(opening, a_white):
    ea = chess.engine.SimpleEngine.popen_uci(A)
    eb = chess.engine.SimpleEngine.popen_uci(B)
    b = chess.Board()
    for u in opening:
        b.push_uci(u)
    try:
        while not b.is_game_over(claim_draw=True) and len(b.move_stack) < 300:
            eng = ea if (b.turn == chess.WHITE) == a_white else eb
            b.push(eng.play(b, chess.engine.Limit(time=MT)).move)
        res = b.result(claim_draw=True) if b.is_game_over(claim_draw=True) else "1/2-1/2"
    finally:
        ea.quit()
        eb.quit()
    s = {"1-0": 1.0, "0-1": 0.0}.get(res, 0.5)
    return s if a_white else 1 - s


jobs = [(o, w) for _ in range(ROUNDS) for o in OPENINGS for w in (True, False)]
w = d = l = 0
with ThreadPoolExecutor(max_workers=2) as ex:
    for s in ex.map(lambda j: game(*j), jobs):
        if s == 1:
            w += 1
        elif s == 0:
            l += 1
        else:
            d += 1
n = w + d + l
score = (w + d / 2) / n
elo = -400 * math.log10(1 / min(max(score, 0.001), 0.999) - 1)
# standard error of the score -> Elo error (95%)
var = (w * (1 - score) ** 2 + d * (0.5 - score) ** 2 + l * score ** 2) / n
se = math.sqrt(var / n)
lo = -400 * math.log10(1 / min(max(score - 1.96 * se, 0.001), 0.999) - 1)
hi = -400 * math.log10(1 / min(max(score + 1.96 * se, 0.001), 0.999) - 1)
print(f"A vs B: +{w} ={d} -{l}  ({n} games)  score {score:.3f}  Elo {elo:+.0f}  [{lo:+.0f}, {hi:+.0f}]", flush=True)
