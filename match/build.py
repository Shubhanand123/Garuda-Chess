import json, chess, datetime, sys
games = json.load(open('games.json'))
for g in games:
    b = chess.Board()
    for p in g['plies']:
        b.push_uci(p['uci']); p['fen'] = b.fen()
data = {"games": games, "startFen": chess.STARTING_FEN, "sfVersion": "dev 28 Oct 2026",
        "updated": (datetime.datetime.utcnow() + datetime.timedelta(hours=5, minutes=30)).strftime('%H:%M IST')}
t = open('template.html').read().replace('__DATA__', json.dumps(data, separators=(',', ':')))
open('garuda-vs-stockfish.html', 'w').write(t)
print(len(games), 'games,', sum(1 for g in games if g['status']=='done'), 'done')
