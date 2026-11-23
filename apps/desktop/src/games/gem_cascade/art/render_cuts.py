#!/usr/bin/env python3
"""Paint Gem Cascade's original sprites and velvet lining with Pillow and NumPy."""

from pathlib import Path

import numpy as np
from PIL import Image, ImageColor, ImageDraw, ImageFilter

OUTPUT = Path(__file__).resolve().parents[4] / "assets/games/gem-cascade"
SIZE = 320
PIXEL_Y, PIXEL_X = (np.indices((SIZE, SIZE)) + 0.5) / SIZE


def pixels(vertices):
    return [(round(x * SIZE), round(y * SIZE)) for x, y in vertices]


def facet(image, vertices, light, dark):
    """Ramp across a single plane with a shared light above and to its left."""
    mask = Image.new("L", (SIZE, SIZE))
    ImageDraw.Draw(mask).polygon(pixels(vertices), fill=255)
    corners = [0.48 * x + 0.87 * y for x, y in vertices]
    lit = np.array(ImageColor.getrgb(light))
    shade = np.array(ImageColor.getrgb(dark))
    ramp = (0.48 * PIXEL_X + 0.87 * PIXEL_Y - min(corners)) / (max(corners) - min(corners))
    colors = lit + np.clip(ramp[..., None], 0, 1) * (shade - lit)
    plane = Image.fromarray(colors.round().astype("uint8"))
    image.paste(plane, mask=mask)


def bevel(image, outer, inner, colors):
    for i, light in enumerate(colors):
        j = (i + 1) % len(outer)
        shade = tuple(round(channel * 0.74) for channel in ImageColor.getrgb(light))
        facet(image, [outer[i], outer[j], inner[j], inner[i]], light, "#%02x%02x%02x" % shade)


def cut(name, rim, crown, colors, table, reflections, ghost_color, inner_step=None):
    mask = Image.new("L", (SIZE, SIZE))
    ImageDraw.Draw(mask).polygon(pixels([(x, y + .034) for x, y in rim]), fill=175)
    image = Image.new("RGBA", mask.size, "#09090d")
    image.putalpha(mask.filter(ImageFilter.GaussianBlur(4)))
    facets = Image.new("RGBA", mask.size)
    # Lay down the table color under the rim, then facet the separate planes.
    facet(facets, rim, *table)
    bevel(facets, rim, crown, colors)
    glass = crown
    if inner_step:
        glass, step_colors = inner_step
        bevel(facets, crown, glass, step_colors)
    facet(facets, glass, *table)
    for vertices, light, dark in reflections:
        facet(facets, vertices, light, dark)
    draw = ImageDraw.Draw(facets)
    draw.line(pixels(rim + rim[:1]), fill=(*ImageColor.getrgb(ghost_color), 210), width=2, joint="curve")
    # One deliberate catchlight follows the top-left shoulder on every cut.
    draw.line(pixels(rim[-1:] + rim[:2]), fill=(255, 245, 234), width=3, joint="curve")
    image.alpha_composite(facets)
    image.resize((160, 160), Image.Resampling.LANCZOS).save(OUTPUT / f"{name}.png", optimize=True)

    outline = Image.new("RGBA", mask.size)
    draw = ImageDraw.Draw(outline)
    color = ImageColor.getrgb(ghost_color)
    draw.polygon(pixels(rim), fill=(*color, 24))
    draw.line(pixels(rim + rim[:1]), fill=(*color, 187), width=10, joint="curve")
    outline.resize((160, 160), Image.Resampling.LANCZOS).save(OUTPUT / f"{name}-outline.png", optimize=True)


def lining():
    rng = np.random.default_rng(91)
    width, height = 192, 384
    y, x = (np.indices((height, width)) + 0.5) / np.array([height, width])[:, None, None]
    fibers = rng.normal(0, 1.05, (height, width))
    fibers[:, 1:] += 0.44 * fibers[:, :-1]
    light = 5.2 - 5.7 * y - 4.0 * (x - 0.34)**2
    pattern = np.array([29, 31, 39]) + (fibers + light)[..., None]
    Image.fromarray(np.clip(pattern, 0, 255).round().astype("uint8")).save(OUTPUT / "lining.png", optimize=True)


if __name__ == "__main__":
    OUTPUT.mkdir(parents=True, exist_ok=True)
    cut(
        "ruby",
        [(0.47, 0.052), (0.54, 0.058), (0.94, 0.448), (0.944, 0.515),
         (0.54, 0.924), (0.47, 0.928), (0.059, 0.527), (0.061, 0.463)],
        [(0.465, 0.213), (0.518, 0.208), (0.763, 0.459), (0.763, 0.499),
         (0.520, 0.740), (0.472, 0.734), (0.218, 0.508), (0.223, 0.455)],
        ["#ffdbdc", "#ff9ca8", "#fa5575", "#9e1b45", "#d7324d", "#ff6e80", "#bc2950", "#ffc2c3"],
        ("#f04d67", "#961a45"),
        [([(0.223, .455), (.465, .213), (.518, .208), (.488, .427)], "#ff9caa", "#f64561"),
         ([(.488, .427), (.763, .499), (.520, .740), (.472, .734)], "#f95a72", "#ca294d"),
         ([(.488, .427), (.472, .734), (.218, .508)], "#ba244b", "#e84059"),
         ([(.318, .365), (.474, .267), (.453, .344)], "#ffe4dc", "#ffa1a6")],
        "#ff8294",
    )
    cut(
        "amber",
        [(0.216, 0.128), (0.784, 0.128), (0.932, 0.276), (0.932, 0.686),
         (0.784, 0.846), (0.216, 0.846), (0.068, 0.686), (0.068, 0.276)],
        [(0.259, 0.242), (0.739, 0.242), (0.817, 0.326), (0.817, 0.633),
         (0.737, 0.722), (0.259, 0.722), (0.183, 0.633), (0.183, 0.326)],
        ["#ffecac", "#ffcb62", "#d67c22", "#8b461a", "#efac37", "#ffd772", "#efab35", "#fff2c2"],
        ("#ffe188", "#e3932a"),
        [([(0.299, 0.338), (.693, .337), (.599, .473), (.279, .427)], "#fff5c8", "#ffdb6c"),
         ([(.279, .427), (.599, .473), (.718, .582), (.685, .622), (.306, .622)], "#e8a43f", "#ffcc66")],
        "#ffd37c",
        ([(0.298, 0.330), (0.697, 0.330), (0.727, 0.370), (0.727, 0.587),
          (0.686, 0.626), (0.310, 0.626), (0.272, 0.587), (0.272, 0.364)],
         ["#fbb449", "#fff0a6", "#ffdb72", "#eaa131", "#9f541d", "#b16a20", "#ffd674", "#ffc957"]),
    )
    cut(
        "jade",
        [(0.454, 0.070), (0.533, 0.073), (0.938, 0.753),
         (0.872, 0.873), (0.126, 0.873), (0.061, 0.753)],
        [(0.475, 0.255), (0.517, 0.254), (0.760, 0.696),
         (0.713, 0.743), (0.277, 0.743), (0.235, 0.692)],
        ["#dbfff2", "#5cebc0", "#23868b", "#2acb97", "#69f3c9", "#afffe9"],
        ("#59eeb7", "#087e75"),
        [([(.475, .255), (.517, .254), (.494, .58), (.235, .692)], "#b0ffe0", "#2cdca6"),
         ([(.517, .254), (.760, .696), (.494, .58)], "#1a9b83", "#24bb9b"),
         ([(.494, .58), (.760, .696), (.713, .743), (.277, .743), (.235, .692)], "#a4ffde", "#42d6ae"),
         ([(.402, .440), (.476, .301), (.469, .416)], "#eafff2", "#8effd3")],
        "#75f3cf",
    )
    cut(
        "amethyst",
        [(0.309, 0.053), (0.691, 0.053), (0.824, 0.184), (0.824, 0.786),
         (0.691, 0.927), (0.309, 0.927), (0.176, 0.786), (0.176, 0.184)],
        [(0.348, 0.168), (0.653, 0.168), (0.718, 0.229), (0.718, 0.734),
         (0.653, 0.809), (0.348, 0.809), (0.280, 0.734), (0.280, 0.229)],
        ["#e7daff", "#c8acff", "#6a60bc", "#584295", "#8b71db", "#806cf4", "#c6adff", "#f3e4ff"],
        ("#a990f7", "#6258ba"),
        [([(.391, .264), (.606, .264), (.548, .453), (.375, .476)], "#d0b9ff", "#a68cf5"),
         ([(.548, .453), (.623, .534), (.622, .679), (.600, .712), (.394, .712)], "#6f58c9", "#ac98fc"),
         ([(.374, .296), (.459, .264), (.438, .327), (.376, .374)], "#efe4ff", "#cbb5ff")],
        "#bbaaef",
        ([(0.391, 0.263), (0.608, 0.263), (0.632, 0.297), (0.632, 0.676),
          (0.604, 0.716), (0.391, 0.716), (0.365, 0.671), (0.365, 0.297)],
         ["#8770d4", "#aa91fb", "#baa7ff", "#9180e8", "#5946a0", "#6652b9", "#a28cf7", "#9f89ec"]),
    )
    lining()
