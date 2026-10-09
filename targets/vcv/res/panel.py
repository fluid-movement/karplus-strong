#!/usr/bin/env python3
"""Generate KarplusStrong.svg, the Rack panel.

Rack renders panels with nanosvg, which ignores `<text>`, so labels are
stroked paths from a small built-in font. The font and the approach are
Fathom's (targets/vcv/res/panel.py there), copied rather than shared: there
is no package for them to live in yet (docs/fathom-reuse.md).

    python3 targets/vcv/res/panel.py

Coordinates are millimetres, matching `mm2px()` in KarplusStrong.cpp. This
file is the source of the numbers; the two must agree.
"""

import pathlib

# --- Geometry ------------------------------------------------------------

HP = 5.08
WIDTH = 16 * HP
HEIGHT = 128.5
CX = WIDTH / 2

PANEL = "#efece6"
EDGE = "#c8c4bc"
INK = "#1a1a1a"
FAINT = "#6a665e"
PLATE = "#15171a"
PLATE_INK = "#f2f0ec"

# --- A stroke font -------------------------------------------------------
#
# Uppercase only, drawn in a 5-wide by 7-tall box with y increasing downward,
# the baseline at y=7. Round caps and joins, so a stroked path reads as a
# letterform rather than as wireframe. Advance is per-glyph so that I and .
# do not sit in a five-wide hole.

GLYPHS = {
    "A": ("M0,7 L2.5,0 L5,7 M0.85,5.1 L4.15,5.1", 5.6),
    "B": ("M0,0 L0,7 M0,0 L3,0 A1.75,1.75 0 0 1 3,3.5 L0,3.5"
          " M3,3.5 A1.75,1.75 0 0 1 3,7 L0,7", 5.4),
    "C": ("M4.9,1.7 A3.0,3.4 0 1 0 4.9,5.3", 5.5),
    "D": ("M0,0 L0,7 M0,0 L2.1,0 A3.4,3.5 0 0 1 2.1,7 L0,7", 5.6),
    "E": ("M4.8,0 L0,0 L0,7 L4.8,7 M0,3.4 L3.7,3.4", 5.2),
    "F": ("M4.8,0 L0,0 L0,7 M0,3.4 L3.6,3.4", 5.2),
    "G": ("M4.9,1.7 A3.0,3.4 0 1 0 4.9,5.3 L4.9,3.9 L2.9,3.9", 5.7),
    "H": ("M0,0 L0,7 M5,0 L5,7 M0,3.5 L5,3.5", 5.6),
    "I": ("M2.5,0 L2.5,7 M1.45,0 L3.55,0 M1.45,7 L3.55,7", 4.3),
    "J": ("M4.1,0 L4.1,5.2 A1.95,1.8 0 0 1 0.2,5.2", 5.0),
    "K": ("M0,0 L0,7 M4.7,0 L0.5,3.8 M1.5,2.9 L4.9,7", 5.4),
    "L": ("M0,0 L0,7 L4.5,7", 5.0),
    "M": ("M0,7 L0,0 L2.5,3.7 L5,0 L5,7", 6.0),
    "N": ("M0,7 L0,0 L5,7 L5,0", 5.7),
    "O": ("M2.5,0 A2.9,3.5 0 0 1 2.5,7 A2.9,3.5 0 0 1 2.5,0", 6.0),
    "P": ("M0,7 L0,0 L3,0 A1.9,1.9 0 0 1 3,3.8 L0,3.8", 5.4),
    "Q": ("M2.5,0 A2.9,3.5 0 0 1 2.5,7 A2.9,3.5 0 0 1 2.5,0"
          " M3.3,5.3 L5.3,7.5", 6.0),
    "R": ("M0,7 L0,0 L3,0 A1.9,1.9 0 0 1 3,3.8 L0,3.8 M2.7,3.8 L5,7", 5.5),
    "S": ("M4.8,1.3 C4.8,-0.5 0.2,-0.5 0.2,2.0"
          " C0.2,3.7 4.8,3.3 4.8,5.0 C4.8,7.5 0.2,7.5 0.2,5.6", 5.4),
    "T": ("M2.5,0 L2.5,7 M0,0 L5,0", 5.2),
    "U": ("M0,0 L0,4.9 A2.5,2.1 0 0 0 5,4.9 L5,0", 5.7),
    "V": ("M0,0 L2.5,7 L5,0", 5.6),
    "W": ("M0,0 L1.2,7 L2.5,2.7 L3.8,7 L5,0", 6.2),
    "X": ("M0,0 L5,7 M5,0 L0,7", 5.5),
    "Y": ("M0,0 L2.5,3.7 L5,0 M2.5,3.7 L2.5,7", 5.5),
    "Z": ("M0,0 L5,0 L0,7 L5,7", 5.4),
    "/": ("M4.4,-0.4 L0.6,7.4", 4.6),
    "-": ("M0.6,3.5 L4.4,3.5", 5.0),
    ".": ("M2.4,7 L2.5,7", 2.8),
    " ": ("", 2.6),
}

LETTER_GAP = 0.7


def _text_width(s, scale):
    total = sum(GLYPHS[c][1] for c in s) + LETTER_GAP * max(len(s) - 1, 0)
    return total * scale


def text(s, x, y, cap=2.4, fill=INK, anchor="middle", weight=0.17):
    """A string as stroked paths. `y` is the baseline, `x` the anchor point."""
    s = s.upper()
    for c in s:
        if c not in GLYPHS:
            raise KeyError("no glyph for %r; add one to GLYPHS" % c)
    scale = cap / 7.0
    width = _text_width(s, scale)
    if anchor == "middle":
        pen = x - width / 2
    elif anchor == "end":
        pen = x - width
    else:
        pen = x
    top = y - cap

    out = []
    for c in s:
        d, advance = GLYPHS[c]
        if d:
            out.append(
                '<path d="%s" transform="translate(%.3f %.3f) scale(%.4f)" '
                'fill="none" stroke="%s" stroke-width="%.3f" '
                'stroke-linecap="round" stroke-linejoin="round"/>'
                % (d, pen, top, scale, fill, weight * cap / scale)
            )
        pen += (advance + LETTER_GAP) * scale
    return out


# --- Layout --------------------------------------------------------------
#
# Rows top to bottom, in the order a player meets them: what the string is
# (with CV), how it is plucked, how it is played, and the jacks.

COLUMNS = [CX + (i - 1.5) * 18.5 for i in range(4)]

# The four that take CV, each with an attenuverter and a jack beneath.
MAIN_LABEL_Y = 17.0
MAIN_Y = 25.0
MAIN = ["DECAY", "BRIGHT", "PICK", "DRIVE"]
ATTEN_Y = 37.0
CV_Y = 45.5

PLUCK_LABEL_Y = 58.5
PLUCK_Y = 65.0
PLUCK_ROW = ["PITCH", "TONE", "LENGTH", "DAMP"]

FEEL_LABEL_Y = 77.0
FEEL_Y = 82.0
FEEL = ["HUMAN", "KEY", "HARM"]
EXCITER_X = COLUMNS[3] - 4.0
EXCITER_NAMES = ["SINE", "DUST", "NOISE"]   # top to bottom: values 2, 1, 0

VELOCITY_HEADER_Y = 90.0
VELOCITY_LABEL_Y = 94.0
VELOCITY_Y = 98.5
VELOCITY = ["BRIGHT", "DECAY", "LENGTH"]
BUTTON_LABEL = "PLUCK"

JACK_LABEL_Y = 110.5
JACK_Y = 116.5
JACKS = [(9.5, "V/OCT"), (24.0, "GATE"), (38.5, "VEL"), (53.0, "IN")]
OUT_X = WIDTH - 11.0
OUT_PLATE = (OUT_X - 8.0, 106.5, 16.0, 16.0)

DIVIDERS = [52.0, 72.0, 104.0]


def build():
    o = []
    o.append(
        '<svg xmlns="http://www.w3.org/2000/svg" width="%.3fmm" height="%.3fmm" '
        'viewBox="0 0 %.3f %.3f">' % (WIDTH, HEIGHT, WIDTH, HEIGHT)
    )
    o.append('<rect width="%.3f" height="%.3f" fill="%s"/>' % (WIDTH, HEIGHT, PANEL))
    o.append(
        '<rect x="0" y="0" width="%.3f" height="%.3f" fill="none" stroke="%s" '
        'stroke-width="0.3"/>' % (WIDTH, HEIGHT, EDGE)
    )

    o += text("KARPLUS-STRONG", CX, 8.6, cap=2.8, weight=0.16)

    for y in DIVIDERS:
        o.append(
            '<line x1="4" y1="%.3f" x2="%.3f" y2="%.3f" stroke="%s" '
            'stroke-width="0.3"/>' % (y, WIDTH - 4, y, EDGE)
        )

    for cx, label in zip(COLUMNS, MAIN):
        o += text(label, cx, MAIN_LABEL_Y, cap=2.5)
    o += text("CV", COLUMNS[0] - 8.2, CV_Y + 1.0, cap=1.9, fill=FAINT)

    for cx, label in zip(COLUMNS, PLUCK_ROW):
        o += text(label, cx, PLUCK_LABEL_Y, cap=2.4)

    for cx, label in zip(COLUMNS, FEEL):
        o += text(label, cx, FEEL_LABEL_Y, cap=2.2)
    for i, name in enumerate(EXCITER_NAMES):
        o += text(name, EXCITER_X + 4.2, FEEL_Y - 2.6 + i * 3.3, cap=1.8, anchor="start")

    o += text("VELOCITY", (COLUMNS[0] + COLUMNS[2]) / 2, VELOCITY_HEADER_Y, cap=1.9,
              fill=FAINT)
    for cx, label in zip(COLUMNS, VELOCITY):
        o += text(label, cx, VELOCITY_LABEL_Y, cap=2.0)
    o += text(BUTTON_LABEL, COLUMNS[3], VELOCITY_LABEL_Y, cap=2.2)

    x, y, w, h = OUT_PLATE
    o.append(
        '<rect x="%.3f" y="%.3f" width="%.3f" height="%.3f" rx="1.4" fill="%s"/>'
        % (x, y, w, h, PLATE)
    )
    o += text("OUT", OUT_X, JACK_LABEL_Y, cap=2.4, fill=PLATE_INK)
    for cx, label in JACKS:
        o += text(label, cx, JACK_LABEL_Y, cap=2.4)

    o.append("</svg>")
    return "\n".join(o) + "\n"


if __name__ == "__main__":
    out = pathlib.Path(__file__).with_name("KarplusStrong.svg")
    out.write_text(build())
    print("wrote %s (%.2f x %.2f mm, %d HP)" % (out, WIDTH, HEIGHT, WIDTH / HP))
