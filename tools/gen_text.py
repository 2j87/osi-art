#!/usr/bin/env python3
"""Osiloskop XY icin buyuk-harf vektor yazi tablosu uretir.

Bir kelimeyi (yalnizca su an tanimli harfler) tek surekli kalem yoluna cevirir:
harflere yogun, harf-arasi gecislere seyrek nokta dusurur; boylece osiloskopta
harfler parlak, gecis (retrace) cizgileri soluk cizilir.

Kullanim:
    python3 tools/gen_text.py ELSAN            # -> src/text_elsan.rs (ELSAN sabiti)
    python3 tools/gen_text.py MERHABA -o src/text_merhaba.rs -n MERHABA

Yeni harf eklemek: FONT sozlugune birim hucre [0,1]x[0,1] icinde polyline
listesi ekle (her polyline surekli cizilir, ayri polyline = kalem-yukari).
"""
import argparse
import math
import os

# Buyuk harf govdesi (basit tek-cizgi font). Genisletmek serbest.
FONT = {
    "E": [[(1, 1), (0, 1), (0, 0), (1, 0)], [(0, 0.5), (0.72, 0.5)]],
    "L": [[(0, 1), (0, 0), (1, 0)]],
    "S": [[(1, 1), (0, 1), (0, 0.5), (1, 0.5), (1, 0), (0, 0)]],
    "A": [[(0, 0), (0.5, 1), (1, 0)], [(0.2, 0.4), (0.8, 0.4)]],
    "N": [[(0, 0), (0, 1), (1, 0), (1, 1)]],
    "M": [[(0, 0), (0, 1), (0.5, 0.45), (1, 1), (1, 0)]],
    "R": [[(0, 0), (0, 1), (0.8, 1), (0.8, 0.5), (0, 0.5), (0.8, 0)]],
    "H": [[(0, 0), (0, 1)], [(0, 0.5), (1, 0.5)], [(1, 1), (1, 0)]],
    "B": [[(0, 0), (0, 1), (0.75, 1), (0.75, 0.5), (0, 0.5), (0.75, 0.5),
           (0.75, 0), (0, 0)]],
    "T": [[(0, 1), (1, 1)], [(0.5, 1), (0.5, 0)]],
    "O": [[(0, 0), (0, 1), (1, 1), (1, 0), (0, 0)]],
    "I": [[(0.5, 1), (0.5, 0)]],
}

MOVE_PTS = 4  # gecis segmenti basina nokta (az = soluk)


def seglen(pts):
    return sum(math.hypot(pts[j + 1][0] - pts[j][0], pts[j + 1][1] - pts[j][1])
               for j in range(len(pts) - 1))


def resample(pts, m):
    m = max(1, m)
    acc = [0.0]
    for j in range(len(pts) - 1):
        acc.append(acc[-1] + math.hypot(pts[j + 1][0] - pts[j][0],
                                        pts[j + 1][1] - pts[j][1]))
    tot = acc[-1] or 1.0
    out = []
    for s in range(m):
        d = tot * s / m
        j = 0
        while j < len(acc) - 2 and acc[j + 1] < d:
            j += 1
        seg = acc[j + 1] - acc[j]
        f = 0.0 if seg == 0 else (d - acc[j]) / seg
        out.append((pts[j][0] + (pts[j + 1][0] - pts[j][0]) * f,
                    pts[j][1] + (pts[j + 1][1] - pts[j][1]) * f))
    return out


def build(word, n):
    for ch in word:
        if ch not in FONT:
            raise SystemExit(f"'{ch}' harfi FONT'ta tanimli degil; ekle veya cikar.")
    cell_w = 0.30
    lw = 0.22  # harf ic genislik
    x0 = -len(word) * cell_w / 2.0
    polys = []
    for k, ch in enumerate(word):
        ox = x0 + k * cell_w + (cell_w - lw) / 2
        for stroke in FONT[ch]:
            polys.append([(ox + px * lw, (py - 0.5) * 0.9) for px, py in stroke])

    segs = []
    for i, p in enumerate(polys):
        if i > 0:
            segs.append(("move", [polys[i - 1][-1], p[0]]))
        segs.append(("draw", p))
    segs.append(("move", [polys[-1][-1], polys[0][0]]))  # dongu kapansin

    draw_len = sum(seglen(p) for t, p in segs if t == "draw")
    n_moves = sum(1 for t, _ in segs if t == "move")
    budget = n - n_moves * MOVE_PTS

    pts = []
    for t, p in segs:
        if t == "move":
            pts += resample(p, MOVE_PTS)
        else:
            pts += resample(p, max(2, round(budget * seglen(p) / draw_len)))
    while len(pts) < n:
        pts.append(pts[-1])
    return pts[:n]


def to_rust(pts, name):
    per = 6
    rows = []
    for i in range(0, len(pts), per):
        rows.append("    " + "".join(f"({x:.4f},{y:.4f})," for x, y in pts[i:i + per]))
    return (f"//! \"{name}\" yazisi -- host'ta uretilmis XY nokta tablosu.\n"
            f"//! Uretim: tools/gen_text.py {name}\n\n"
            f"pub const {name}: [(f32, f32); {len(pts)}] = [\n"
            + "\n".join(rows) + "\n];\n")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("word")
    ap.add_argument("-n", "--name", help="Rust sabit adi (varsayilan: kelime)")
    ap.add_argument("-o", "--out", help="cikti .rs yolu")
    ap.add_argument("-p", "--points", type=int, default=2048, help="POINTS ile ayni olmali")
    a = ap.parse_args()
    word = a.word.upper()
    name = (a.name or word).upper()
    out = a.out or os.path.join("src", f"text_{word.lower()}.rs")
    pts = build(word, a.points)
    with open(out, "w") as f:
        f.write(to_rust(pts, name))
    print(f"yazildi: {out}  ({len(pts)} nokta, sabit adi {name})")


if __name__ == "__main__":
    main()
