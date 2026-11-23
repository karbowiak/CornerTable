"""Regenerate tiles.webp with Python, NumPy, Pillow, and CairoSVG (with Cairo).

The first four rows are dots, bamboo, characters, then honors/bonuses. The
fifth row holds the ceramic bodies. Each cell uses a 100 × 114 drawing space.
"""

from io import BytesIO
from math import cos, pi, sin
from pathlib import Path

import cairosvg
import numpy as np
from PIL import Image

WIDTH, HEIGHT = 144, 164
GLAZE = "#f4e2b9"
COBALT, JADE, CINNABAR = "url(#cobalt)", "url(#jade)", "url(#cinnabar)"
DEFS = """
<defs>
  <linearGradient id="cobalt" x2="0.65" y2="1">
    <stop stop-color="#3b7990"/><stop offset=".54" stop-color="#244f6e"/>
    <stop offset="1" stop-color="#172e51"/>
  </linearGradient>
  <linearGradient id="jade" x2="0.7" y2="1">
    <stop stop-color="#4e805a"/><stop offset=".52" stop-color="#2d623f"/>
    <stop offset="1" stop-color="#193c33"/>
  </linearGradient>
  <linearGradient id="cinnabar" x2="0.7" y2="1">
    <stop stop-color="#b45d42"/><stop offset=".52" stop-color="#97392e"/>
    <stop offset="1" stop-color="#6c282a"/>
  </linearGradient>
</defs>"""


def paint(body):
    svg = f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 114">{DEFS}{body}</svg>'
    png = cairosvg.svg2png(
        bytestring=svg.encode(), output_width=WIDTH * 2, output_height=HEIGHT * 2
    )
    return Image.open(BytesIO(png)).convert("RGBA").resize((WIDTH, HEIGHT), Image.Resampling.LANCZOS)


def path(d, color, extra=""):
    return f'<path d="{d}" fill="{color}" {extra}/>'


def stroke(d, color, width, extra=""):
    return path(d, "none", f'stroke="{color}" stroke-width="{width}" '
                           f'stroke-linecap="round" stroke-linejoin="round" {extra}')


def circle(x, y, r, color, extra=""):
    return f'<circle cx="{x}" cy="{y}" r="{r}" fill="{color}" {extra}/>'


def at(x, y, scale, art):
    return f'<g transform="translate({x} {y}) scale({scale})">{art}</g>'


# The layouts show the actual pip count and give the diagonals, six and seven
# a different silhouette. Color accents follow the stamped pattern.
PIPS = {
    2: [(51, 43), (51, 78)],
    3: [(29, 49), (51, 65), (73, 81)],
    4: [(x, y) for y in (49, 80) for x in (33, 69)],
    5: [(x, y) for y in (49, 83) for x in (29, 73)] + [(51, 66)],
    6: [(x, y) for y in (48, 67, 85) for x in (34, 68)],
    7: [(29, 46), (51, 52), (73, 58)] + [(x, y) for y in (73, 89) for x in (29, 73)],
    8: [(x, y) for y in (45, 60, 75, 90) for x in (34, 68)],
    9: [(x, y) for y in (48, 67, 86) for x in (26, 51, 76)],
}


def dots(rank):
    if rank == 1:
        petal = path("M0-20 Q8-17 4-9 L0-6 Q-8-11 0-20Z", GLAZE, 'opacity=".92"')
        petals = "".join(f'<g transform="rotate({angle})">{petal}</g>' for angle in range(0, 360, 45))
        return at(51, 62, 1., (
            circle(0, 1.5, 29, "#17394c", 'opacity=".27"')
            + circle(0, 0, 29, COBALT)
            + circle(0, 0, 23.7, "none", f'stroke="{GLAZE}" stroke-width="2.1" opacity=".95"')
            + petals
            + circle(0, 0, 5.1, "#d98a5b")
            + circle(-1.2, -1.5, 1.8, "#f4d598")
            + stroke("M-23-14 Q-17-24-5-26", "#b7d3cb", 1.6, 'opacity=".58"')
        ))
    scale = (1.21, 1.06, 1.03, .98, .84, .69, .65, .82)[rank - 2]
    art = []
    for i, (x, y) in enumerate(PIPS[rank]):
        ink = (
            CINNABAR if (rank == 5 and i == 4) or (rank == 6 and i < 2)
            else JADE if (rank == 7 and i < 3) or (rank == 9 and 3 <= i < 6)
            else COBALT
        )
        ring = (
            circle(0, 0, 10.4, ink)
            + circle(0, 0, 5.8, "none", f'stroke="{GLAZE}" stroke-width="1.5" opacity=".88"')
            + circle(0, 0, 2.8, GLAZE, 'opacity=".91"')
            + stroke("M-8.4-3 Q-7.4-5.8-4-7.8", "#ffffff", 1.2, 'opacity=".37"')
        )
        art.append(at(x, y, scale, ring))
    return "".join(art)


def sparrow():
    return (
        stroke("M22 83 Q45 80 72 69", "#31583d", 4.5)
        + path("M29 81 Q24 66 15 67 Q15 78 29 81Z M61 74 Q67 61 79 62 Q76 71 61 74Z", JADE)
        + stroke("M49 69 L46 78 M55 69 L57 76", "#754638", 2.6)
        + path("M37 56 L19 65 Q16 69 22 70 L43 66Z", "#244f57")
        + path("M65 44 Q69 36 82 37 L72 45 L64 47Z", "#a64b32")
        + path("M26 60 Q32 52 44 46 Q45 30 58 28 Q70 27 73 36 Q77 45 66 51 "
               "Q69 68 53 72 Q37 75 26 65 Q24 63 26 60Z", JADE)
        + path("M47 45 Q51 34 63 34 Q70 35 72 40 Q70 46 65 48 Q63 62 54 66 "
               "Q43 68 36 63 Q43 62 48 58 Q57 51 47 45Z", "#d29756")
        + path("M33 60 Q42 46 57 49 Q56 60 46 64 Q38 65 33 60Z", "#234e5c")
        + stroke("M37 58 Q44 52 52 52 M42 61 Q47 58 51 57", "#8eb7a4", 1.65)
        + path("M49 37 Q55 30 63 32 Q69 34 70 39 Q58 35 49 37Z", COBALT)
        + circle(63, 40, 2.65, "#202b2b")
        + circle(62.6, 39.4, .85, "#fdf2d0")
    )


# Columns of notched bamboo. Three has a high stalk and two low ones.
BAMBOO = {
    2: (1, 1),
    3: (1, 2),
    4: (2, 2),
    5: (2, 1, 2),
    6: (3, 3),
    7: (1, 3, 3),
    8: (4, 4),
    9: (3, 3, 3),
}


def bamboo(rank):
    if rank == 1:
        return sparrow()
    columns = BAMBOO[rank]
    scale = .61 if rank == 8 else .78 if rank >= 5 else .99
    spacing = (0, 0, 27, 21, 15)
    art = []
    for column, count in enumerate(columns):
        x = 51 + (column - (len(columns) - 1) / 2) * (27 if len(columns) == 3 else 32)
        for row in range(count):
            y = (68 if count >= 3 else 65) + (row - (count - 1) / 2) * spacing[count]
            if rank == 3:
                y += -12 if column == 0 else 3
            ink = CINNABAR if (rank in (5, 7) and count == 1) or (rank == 9 and column == 1) else JADE
            joint = path(
                "M-6-8 Q-7-10-4-11 Q0-12 5-11 Q7-10 6-8 Q4-6 4-3 "
                "L4 4 Q4 7 6 8 Q7 11 4 11 Q0 12-5 11 Q-7 10-6 8 "
                "Q-4 6-4 3 L-4-3 Q-4-6-6-8Z",
                ink,
            )
            vein = stroke("M-2-7 Q-1-3-1 5 M-3-9 L2-9", "#c4cf92", 1.45, 'opacity=".75"')
            stalk = joint + vein
            if rank == 2:
                stalk = f'<g transform="scale(1.22 1.86)">{stalk}</g>'
            art.append(at(x, y, scale, stalk))
    return "".join(art)


# Hand-drawn forms in normalized calligraphy spaces. The cross and hooks
# are intentionally substantial enough to survive scaling down to a tile.
NUMERALS = (
    "M8 19 Q25 20 56 17",
    "M16 11 Q29 12 48 9 M8 29 Q29 31 56 27",
    "M13 8 L51 6 M19 18 L46 16 M8 30 Q29 31 56 27",
    "M10 8 Q11 20 12 31 M11 9 Q29 7 54 7 L51 30 L12 30 "
    "M24 10 Q24 20 18 23 M38 9 L38 20 Q41 23 50 20",
    "M15 7 Q32 7 51 5 M31 8 L22 29 M16 17 L44 15 L42 28 M8 30 L55 28",
    "M29 4 L35 11 M9 16 Q31 15 55 13 M24 21 Q18 28 12 30 M40 21 Q49 25 52 29",
    "M10 17 Q31 15 54 10 M29 5 L27 25 Q27 33 44 31 Q50 31 51 25",
    "M24 8 Q23 21 10 31 M38 7 Q43 24 54 30",
    "M12 13 L46 9 Q43 22 44 26 Q48 32 55 28 L57 23 "
    "M33 4 Q32 23 12 32",
)
TEN_THOUSAND = (
    "M7 9 L55 7 M20 4 L21 13 M41 3 L39 13 "
    "M13 18 L49 17 L47 36 L14 37Z M14 27 L47 26 M31 18 L31 52 "
    "M8 56 L8 44 L57 43 L55 56 Q54 61 48 58 M20 52 L40 50 M39 47 L44 54"
)


def characters(rank):
    return at(32, 30, .69, stroke(NUMERALS[rank - 1], "#414035", 6.8)) + at(
        27, 52, .74, stroke(TEN_THOUSAND, CINNABAR, 5.5)
    )


WINDS = {
    # 東 南 西 北
    "E": (
        "M8 13 Q31 13 56 10 M32 4 L31 62 M15 24 L50 22 L47 43 L16 43Z M16 33 L49 32 "
        "M28 43 Q21 53 6 60 M35 43 Q43 53 58 59"
    ),
    "S": (
        "M11 12 L53 10 M34 4 L31 22 M9 62 L9 24 L55 22 L53 57 Q53 64 44 60 "
        "M20 30 L25 36 M43 29 L38 36 M18 39 L45 38 M18 48 L45 47 M32 39 L31 59"
    ),
    "W": (
        "M6 12 Q29 12 59 10 M12 25 L54 24 L51 55 L13 56Z "
        "M25 14 Q26 36 17 41 M39 13 L38 34 Q40 42 50 37"
    ),
    "N": (
        "M27 6 L26 61 M10 24 L25 22 M7 48 L25 39 "
        "M38 7 L37 50 Q37 57 46 57 L51 57 Q58 56 59 46 M39 31 Q47 25 55 18"
    ),
}


def wind(direction):
    return at(27, 33, .83, stroke(WINDS[direction], COBALT, 6.5))


def red_dragon():
    stamp = path(
        "M9 24 Q8 18 14 18 L49 17 Q56 17 55 23 L54 45 Q54 50 49 50 "
        "L15 51 Q11 51 11 47Z M17 26 L19 43 L46 42 L47 25Z",
        CINNABAR, 'fill-rule="evenodd"',
    ) + path("M29 7 Q29 3 34 4 Q37 4 37 9 L35 62 Q35 66 31 65 Q27 64 27 60Z", CINNABAR)
    return at(21, 24, .96, stamp + stroke("M14 22 L24 21 M32 10 L31 18", "#e9ae81", 1.7))


def green_dragon():
    # 發 has the two mirrored upper strokes, with the bow and hand underneath.
    form = (
        "M8 8 L29 7 Q23 21 6 30 M11 16 L19 21 M34 6 Q42 21 59 29 "
        "M45 7 L40 14 M53 13 L47 20 "
        "M11 34 L26 33 L25 42 L13 42 L11 48 L26 48 L24 58 Q22 64 16 60 "
        "M34 43 L37 32 L47 31 L47 39 L54 39 "
        "M35 47 L53 46 Q48 56 32 62 M38 48 Q43 57 56 62"
    )
    return at(25, 34, .86, stroke(form, JADE, 6.))


def white_dragon():
    frame = (
        "M35 29 Q51 32 67 28 Q67 37 76 39 L76 78 Q68 80 67 88 "
        "Q51 85 35 89 Q33 80 26 79 L26 39 Q33 37 35 29Z"
    )
    return at(7, 17, .86, (
        stroke(frame, COBALT, 6.3)
        + stroke("M37 39 L65 38 L66 78 L36 79Z", "#6b8d94", 1.9)
        + stroke("M29 49 L29 41 Q36 40 38 33 M52 31 L63 31", "#d4d9b5", 1.5)
    ))


def flower():
    leaves = (
        stroke("M31 89 Q47 81 53 49", "#2c5238", 4.7)
        + path("M43 79 Q27 76 23 64 Q43 65 43 79Z M46 75 Q53 62 73 65 Q67 79 46 75Z", JADE)
        + stroke("M29 68 L41 76 M50 73 L65 68", "#abbb83", 1.65)
    )
    petal = path(
        "M0 3 Q-14-3-14-13 Q-15-20-8-22 Q-4-23 0-20 "
        "Q7-25 13-19 Q22-10 0 3Z",
        "#dc9878", 'stroke="#943f3d" stroke-width="2.3"',
    ) + stroke("M-1-4 Q-7-11-7-17", "#f3bf94", 1.8)
    petals = "".join(f'<g transform="rotate({angle})">{petal}</g>' for angle in (0, 72, 144, 216, 288))
    bloom = at(54, 46, .90, petals + circle(0, 0, 6.8, "#84392d") + circle(0, 0, 4.2, "#e6b253"))
    return leaves + bloom


def ginkgo():
    fan = path(
        "M0 0 Q-15-2-24-17 Q-26-21-20-24 Q-17-30-9-28 "
        "Q-6-32 0-29 Q6-32 9-28 Q17-29 20-24 Q26-22 23-16 Q13-2 0 0Z",
        "#d8a754", 'stroke="#815126" stroke-width="2.8"',
    ) + stroke(
        "M0-2 Q-6-11-18-21 M0-2 Q-3-19-9-25 M0-2 L0-26 "
        "M0-2 Q4-19 9-25 M0-2 Q8-12 18-21",
        "#956630", 1.5,
    )
    return (
        stroke("M32 90 Q56 67 59 43 M47 71 Q36 68 34 63 M50 67 Q62 64 67 66", "#66502e", 4.6)
        + f'<g transform="translate(59 44) rotate(18)">{fan}</g>'
        + f'<g transform="translate(34 64) rotate(-46) scale(.69)">{fan}</g>'
        + f'<g transform="translate(67 66) rotate(65) scale(.68)">{fan}</g>'
    )


def smooth(value):
    t = np.clip(value, 0., 1.)
    return t * t * (3. - 2. * t)


def blend(a, b, amount):
    return np.asarray(a) + (np.asarray(b) - a) * np.asarray(amount)[..., None]


def rounded(x, y, left, top, right, bottom, radius):
    """Signed distance from the rounded ceramic edge, negative inside."""
    u = np.abs(x - (left + right) / 2) - (right - left) / 2 + radius
    v = np.abs(y - (top + bottom) / 2) - (bottom - top) / 2 + radius
    return np.hypot(np.maximum(u, 0), np.maximum(v, 0)) + np.minimum(np.maximum(u, v), 0) - radius


def color(value):
    return np.asarray(tuple(bytes.fromhex(value)))


def ceramic(state):
    """Light the lip, poured glaze and undercut jade foot as separate surfaces."""
    w, h = WIDTH * 2, HEIGHT * 2
    y, x = np.mgrid[:h, :w].astype(float)
    x, y = (x + .5) * 100 / w, (y + .5) * 114 / h
    body = rounded(x, y, .6, .6, 99.1, 113.5, 11.6)
    lip = rounded(x, y, 1.3, .2, 99.6, 107.5, 11.)
    plate = rounded(x, y, 5.2, .2, 99.4, 101., 10.3)
    depth = -plate
    dy, dx = np.gradient(depth)
    light = np.clip(.50 + dx * .82 + dy * .93, 0., 1.)
    wash = smooth((x * .54 + y * .79) / 125)
    gleam = np.exp(-((x - 38) / 37)**2 - ((y - 17 - .095 * (x - 38)) / 12)**2)
    blocked = state == "blocked"

    foot = blend(color("172d29"), color("377c62"), smooth((114.1 - y) / 6) * .74 + .10)
    foot = blend(foot, color("161f20"), smooth((body + 2.5) / 2.5) * .57)
    rim = blend(color("a08055"), color("e4c898"), smooth((110 - y) / 15) * .72 + .13)
    rim = blend(rim, color("6f583e"), smooth((lip + 2.1) / 2.1) * .37)
    if blocked:
        rim = blend(rim, color("786e54"), .40)
        foot = blend(foot, color("252f2a"), .36)
    base = blend(foot, rim, smooth(.55 - lip))

    porcelain = blend(
        color("d2cab1" if blocked else "fff6de"),
        color("b6ac90" if blocked else "e8d9b2"), wash,
    )
    porcelain = blend(porcelain, color("fffdee"), gleam * (.09 if blocked else .54))
    bevel = blend(color("928569" if blocked else "a6895c"),
                  color("e5ddc0" if blocked else "fff9e4"), light)
    glaze = blend(bevel, porcelain, smooth((depth + .6) / 6.8))
    crest = np.exp(-((depth - 2.15) / 1.15)**2)
    glaze = blend(glaze, color("fffdef"), crest * light * (.15 if blocked else .48))
    glaze = blend(glaze, color("fff1c1"), crest * (1 - light) * (.13 if blocked else .26))

    rng = np.random.default_rng(318)
    freckles = np.zeros_like(x)
    for sx, sy, r, opacity in zip(
        rng.uniform(4, 99, 125), rng.uniform(2, 106, 125),
        rng.uniform(.22, .68, 125), rng.uniform(.012, .13, 125),
    ):
        freckles += opacity * np.exp(-((x - sx)**2 + (y - sy)**2) / (r * r))
    glaze = blend(glaze, color("926944"), freckles)

    if state == "selected":
        stripe = smooth((depth - 1.5) / 1.2) * smooth((7.8 - depth) / 1.2)
        glaze = blend(glaze, blend(color("164354"), color("437b89"), light), stripe)
    elif state == "hinted":
        # Four painted corner brackets show a hint without relying on hue.
        corners = smooth((24 - np.minimum(x - 5, 99 - x)) / 8)
        corners *= smooth((26 - np.minimum(y, 101 - y)) / 8)
        stripe = smooth((depth - 1.7) / 1.2) * smooth((8.7 - depth) / 1.2)
        glaze = blend(glaze, color("95571c"), corners * stripe)
    elif state == "matched":
        glaze = blend(glaze, color("ffdd87"), (1 - smooth((depth - 1) / 16)) * .79)
        glaze = blend(glaze, color("fffdf1"), gleam * .78)

    tile = blend(base, glaze, smooth(.6 - plate))
    alpha = smooth(.50 - np.minimum(body, lip)) * 255
    rgba = np.dstack((tile, alpha)).clip(0, 255).astype("uint8")
    return Image.fromarray(rgba).resize((WIDTH, HEIGHT), Image.Resampling.LANCZOS)


def main():
    faces = [dots(rank) for rank in range(1, 10)]
    faces += [bamboo(rank) for rank in range(1, 10)]
    faces += [characters(rank) for rank in range(1, 10)]
    faces += [wind(direction) for direction in "ESWN"]
    faces += [red_dragon(), green_dragon(), white_dragon(), flower(), ginkgo()]
    pieces = [paint(face) for face in faces]
    pieces += [ceramic(state) for state in ("free", "blocked", "selected", "hinted", "matched")]
    sheet = Image.new("RGBA", (9 * WIDTH, 5 * HEIGHT))
    for index, piece in enumerate(pieces):
        sheet.paste(piece, (index % 9 * WIDTH, index // 9 * HEIGHT))
    sheet.save(Path(__file__).with_name("tiles.webp"), lossless=True, method=6)


if __name__ == "__main__":
    main()
