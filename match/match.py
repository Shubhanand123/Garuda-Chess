"""Garuda vs Stockfish match runner. Writes games incrementally to games.json."""
import chess, chess.engine, json, sys, threading, time
from concurrent.futures import ThreadPoolExecutor

GARUDA = "/home/claude/garuda/target/release/garuda"
SF = "/home/claude/official-stockfish/stockfish/src/stockfish"
OUT = sys.argv[1] if len(sys.argv) > 1 else "games.json"
MOVETIME = 1.0

OPENINGS = [
    ("Ruy Lopez / Open game", ["e2e4", "e7e5", "g1f3", "b8c6"]),
    ("Queen's Gambit Declined", ["d2d4", "d7d5", "c2c4", "e7e6"]),
    ("Sicilian Defence", ["e2e4", "c7c5", "g1f3", "d7d6"]),
    ("King's Indian Defence", ["d2d4", "g8f6", "c2c4", "g7g6"]),
]

# (label, sf_elo or None for full strength, list of (opening index, garuda_is_white))
SCHEDULE = [
    ("Full strength", None, [(0, True), (0, False)]),
    ("2600", 2600, [(1, True), (1, False), (2, True), (2, False)]),
    ("2200", 2200, [(3, True), (3, False), (0, True), (0, False)]),
    ("1800", 1800, [(1, True), (1, False), (2, True), (2, False)]),
]

lock = threading.Lock()
games = []


def save():
    with open(OUT, "w") as f:
        json.dump(games, f)


def cp_white(info, board_turn_white):
    s = info.get("score")
    if s is None:
        return None
    w = s.white()
    if w.is_mate():
        m = w.mate()
        return {"mate": m}
    return {"cp": w.score()}


def play_game(level, elo, opening_idx, garuda_white):
    name, opening = OPENINGS[opening_idx]
    g = chess.engine.SimpleEngine.popen_uci(GARUDA)
    s = chess.engine.SimpleEngine.popen_uci(SF)
    s.configure({"Threads": 1, "Hash": 64})
    g.configure({"Hash": 64})
    if elo:
        s.configure({"UCI_LimitStrength": True, "UCI_Elo": elo})
    board = chess.Board()
    rec = {
        "level": level, "elo": elo, "opening": name,
        "white": "Garuda" if garuda_white else "Stockfish",
        "black": "Stockfish" if garuda_white else "Garuda",
        "plies": [], "result": "*", "reason": "", "status": "playing",
    }
    for u in opening:
        mv = chess.Move.from_uci(u)
        rec["plies"].append({"uci": u, "san": board.san(mv), "book": True})
        board.push(mv)
    with lock:
        games.append(rec)
        save()
    win_streak = {"w": 0, "b": 0}
    draw_streak = 0
    try:
        while not board.is_game_over(claim_draw=True):
            garuda_to_move = (board.turn == chess.WHITE) == garuda_white
            eng = g if garuda_to_move else s
            t0 = time.time()
            r = eng.play(board, chess.engine.Limit(time=MOVETIME), info=chess.engine.INFO_ALL, game=rec["opening"] + str(id(rec)))
            dt = time.time() - t0
            info = r.info
            sc = cp_white(info, board.turn)
            ply = {
                "uci": r.move.uci(), "san": board.san(r.move),
                "by": "Garuda" if garuda_to_move else "Stockfish",
                "depth": info.get("depth"), "seldepth": info.get("seldepth"),
                "nodes": info.get("nodes"), "time": round(dt, 2), "score": sc,
            }
            board.push(r.move)
            with lock:
                rec["plies"].append(ply)
                save()
            # adjudication (only trusted from Stockfish's evaluation)
            if not garuda_to_move and sc is not None:
                v = 100000 if "mate" in sc and sc["mate"] > 0 else -100000 if "mate" in sc else sc["cp"]
                win_streak["w"] = win_streak["w"] + 1 if v >= 1000 else 0
                win_streak["b"] = win_streak["b"] + 1 if v <= -1000 else 0
                draw_streak = draw_streak + 1 if abs(v) <= 15 and len(board.move_stack) >= 80 else 0
                if elo is None and win_streak["w"] >= 3:
                    rec["result"], rec["reason"] = "1-0", "adjudicated: Stockfish eval over +10 for 3 moves"
                    break
                if elo is None and win_streak["b"] >= 3:
                    rec["result"], rec["reason"] = "0-1", "adjudicated: Stockfish eval under -10 for 3 moves"
                    break
                if draw_streak >= 10:
                    rec["result"], rec["reason"] = "1/2-1/2", "adjudicated draw: dead-equal for 10 moves"
                    break
            if len(board.move_stack) >= 300:
                rec["result"], rec["reason"] = "1/2-1/2", "adjudicated draw: 150 moves"
                break
        if rec["result"] == "*":
            out = board.outcome(claim_draw=True)
            rec["result"] = out.result()
            rec["reason"] = out.termination.name.replace("_", " ").lower()
    finally:
        rec["status"] = "done"
        with lock:
            save()
        g.quit()
        s.quit()
    return rec


jobs = [(lvl, elo, o, gw) for lvl, elo, lst in SCHEDULE for o, gw in lst]
with ThreadPoolExecutor(max_workers=2) as ex:
    for r in ex.map(lambda j: play_game(*j), jobs):
        g_score = {"1-0": 1, "0-1": 0, "1/2-1/2": 0.5}[r["result"]]
        if r["white"] != "Garuda":
            g_score = 1 - g_score
        print(f"{r['level']:>13} | {r['white']:>9} vs {r['black']:<9} | {r['result']:<7} | Garuda {g_score} | {r['reason']}", flush=True)
print("MATCH COMPLETE", flush=True)
