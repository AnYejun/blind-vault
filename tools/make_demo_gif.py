#!/usr/bin/env python3
"""Render assets/demo.gif — the day-one flow: ls, blind Gemini call, hands-free login.

usage: python3 tools/make_demo_gif.py <logo.png>
  logo.png: the lock-with-closed-eye mark rendered on #0D1117 (any size).
"""
import os
import sys

from PIL import Image, ImageDraw, ImageFont

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = os.path.join(ROOT, "assets", "demo.gif")
LOGO = sys.argv[1] if len(sys.argv) > 1 else None

W, H = 960, 600
BG = "#0D1117"; TITLEBAR = "#161B22"; HAIR = "#21262D"
TXT = "#E6EDF3"; OUTC = "#8B949E"; DIM = "#58606A"; GREEN = "#27DBA2"

MONO = ImageFont.truetype("/System/Library/Fonts/Menlo.ttc", 17, index=0)
SANS = ImageFont.truetype("/System/Library/Fonts/Helvetica.ttc", 15, index=0)
BIG = ImageFont.truetype("/System/Library/Fonts/Menlo.ttc", 34, index=1)
MID = ImageFont.truetype("/System/Library/Fonts/Menlo.ttc", 18, index=0)
PAD_X, TOP_Y, LH = 30, 78, 30


def chrome(d):
    d.rectangle([0, 0, W, H], fill=BG)
    d.rectangle([0, 0, W, 46], fill=TITLEBAR)
    d.line([0, 46, W, 46], fill=HAIR)
    for i, c in enumerate(["#FF5F57", "#FEBC2E", "#28C840"]):
        d.ellipse([22 + i * 22, 16, 36 + i * 22, 30], fill=c)
    t = "blind-vault — day one, for real"
    d.text(((W - d.textlength(t, font=SANS)) / 2, 14), t, font=SANS, fill=OUTC)


def frame(lines, typed=None, cursor=True):
    img = Image.new("RGB", (W, H), BG)
    d = ImageDraw.Draw(img)
    chrome(d)
    y = TOP_Y
    for segs in lines:
        x = PAD_X
        for text, color in segs:
            d.text((x, y), text, font=MONO, fill=color)
            x += d.textlength(text, font=MONO)
        y += LH
    if typed is not None:
        d.text((PAD_X, y), "$ ", font=MONO, fill=GREEN)
        x = PAD_X + d.textlength("$ ", font=MONO)
        d.text((x, y), typed, font=MONO, fill=TXT)
        x += d.textlength(typed, font=MONO)
        if cursor:
            d.rectangle([x + 2, y + 1, x + 12, y + 21], fill=GREEN)
    return img


frames, durs = [], []


def emit(img, ms):
    frames.append(img)
    durs.append(ms)


def type_cmd(L, cmd, step=3, ms=52):
    for i in range(step, len(cmd) + step, step):
        emit(frame(L, typed=cmd[:i]), ms)
    emit(frame(L, typed=cmd), 380)
    L.append([("$ ", GREEN), (cmd, TXT)])


L = []
emit(frame(L, typed=""), 700)

# 1. what the agent sees
type_cmd(L, "vault ls", step=2, ms=70)
L.append([("NAME            ENV              ALLOWED FOR                        LAST USED", DIM)])
L.append([("gemini-api-key  GEMINI_API_KEY   generativelanguage.googleapis.com  never", OUTC)])
L.append([("github-login    GITHUB_LOGIN     github.com", OUTC), ("  id: you@example.com", DIM)])
L.append([("fly-api-token   FLY_API_TOKEN    fly                                2026-07-11", OUTC)])
L.append([("# pointers only. there is no value column — by design.", GREEN)])
L.append([])
emit(frame(L, typed=""), 2100)

# 2. blind API call (really happened)
type_cmd(L, "vault use gemini-api-key -- curl https://generativelanguage.googleapis.com/…")
L.append([('Gemini: "Claude just called me, *blindly* using my API key.', OUTC)])
L.append([('         Congrats on that slick new blind-vault setup!"', OUTC)])
L.append([("# the key rode an env var from Keychain into curl. the agent never saw it.", GREEN)])
L.append([])
emit(frame(L, typed=""), 2500)

# 3. hands-free login
type_cmd(L, "vault type github-login --account --enter")
L.append([("typing in 4s — click the target field now (a browser must be frontmost)...", DIM)])
emit(frame(L, cursor=False), 1100)
L.append([("typed into 'Google Chrome'. the value never appeared on screen", OUTC)])
L.append([("or in this conversation.", OUTC)])
L.append([("# ID → Tab → password → Enter, as raw OS keystrokes. hands never touched keys.", GREEN)])
emit(frame(L, cursor=False), 2600)

# end card with the lock-with-closed-eye mark
img = Image.new("RGB", (W, H), BG)
d = ImageDraw.Draw(img)
chrome(d)
top = 200
if LOGO:
    mark = Image.open(LOGO).convert("RGB")
    mark = mark.resize((int(mark.width * 0.62), int(mark.height * 0.62)), Image.LANCZOS)
    y0 = (H + 46 - (mark.height + 18 + 130)) // 2
    img.paste(mark, ((W - mark.width) // 2, y0))
    top = y0 + mark.height + 18
t1, t2, t3 = "blind-vault", "secrets your agent can use — but never see", "github.com/AnYejun/blind-vault"
d.text(((W - d.textlength(t1, font=BIG)) / 2, top), t1, font=BIG, fill=TXT)
d.text(((W - d.textlength(t2, font=MID)) / 2, top + 62), t2, font=MID, fill=GREEN)
d.text(((W - d.textlength(t3, font=MONO)) / 2, top + 110), t3, font=MONO, fill=DIM)
emit(img, 3000)

frames[0].save(OUT, save_all=True, append_images=frames[1:], duration=durs, loop=0, optimize=True)
print(f"{OUT}: {len(frames)} frames, {sum(durs) / 1000:.1f}s, {os.path.getsize(OUT) / 1024:.0f} KB")
