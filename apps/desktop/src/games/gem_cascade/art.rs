use corner_table_gem_cascade::Gem;
use gpui::{
    Bounds, Hsla, IntoElement, PathBuilder, Pixels, Point, Styled as _, Window, canvas, point, px,
    rgb,
};

#[derive(Clone, Copy)]
pub(super) enum Finish {
    Active,
    Settled,
    Ghost,
    Clearing(f32),
}

pub(super) fn name(gem: Gem) -> &'static str {
    match gem {
        Gem::Ruby => "Ruby",
        Gem::Amber => "Amber",
        Gem::Jade => "Jade",
        Gem::Amethyst => "Amethyst",
    }
}

pub(super) fn stone(gem: Gem, size: f32, finish: Finish) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds, (), window, _| draw(gem, bounds, finish, window),
    )
    .size(px(size))
    .flex_none()
}

type Vertex = (f32, f32);

struct Cut {
    outline: &'static [Vertex],
    facets: &'static [u32],
    center: Vertex,
    table: u32,
    edge: u32,
    highlight: u32,
}

// The silhouettes stay recognizable at the smallest board and preview sizes.
const RUBY: Cut = Cut {
    outline: &[
        (0.43, 0.07),
        (0.57, 0.07),
        (0.94, 0.44),
        (0.94, 0.56),
        (0.57, 0.93),
        (0.43, 0.93),
        (0.06, 0.56),
        (0.06, 0.44),
    ],
    facets: &[
        0xffc3cb, 0xff8195, 0xce3f63, 0x8b2246, 0xc22d51, 0xd94c70, 0xa81d42, 0xff6686,
    ],
    center: (0.5, 0.45),
    table: 0xe7476d,
    edge: 0xfe7894,
    highlight: 0xffd1d6,
};

const AMBER: Cut = Cut {
    outline: &[
        (0.31, 0.11),
        (0.69, 0.11),
        (0.91, 0.31),
        (0.91, 0.69),
        (0.69, 0.89),
        (0.31, 0.89),
        (0.09, 0.69),
        (0.09, 0.31),
    ],
    facets: &[
        0xffe7a4, 0xffc967, 0xd78521, 0x9c5014, 0xc87012, 0xe68f24, 0xd17616, 0xfebc44,
    ],
    center: (0.49, 0.46),
    table: 0xf9b738,
    edge: 0xffc76b,
    highlight: 0xfff1be,
};

const JADE: Cut = Cut {
    outline: &[
        (0.455, 0.11),
        (0.545, 0.11),
        (0.93, 0.76),
        (0.86, 0.87),
        (0.14, 0.87),
        (0.07, 0.76),
    ],
    facets: &[0xb4f8c7, 0x62d5a2, 0x2b976b, 0x217c61, 0x47b289, 0x7ae0ad],
    center: (0.49, 0.59),
    table: 0x49bf91,
    edge: 0x6adfa9,
    highlight: 0xc8ffdd,
};

const AMETHYST: Cut = Cut {
    outline: &[
        (0.27, 0.06),
        (0.73, 0.06),
        (0.83, 0.16),
        (0.83, 0.84),
        (0.73, 0.94),
        (0.27, 0.94),
        (0.17, 0.84),
        (0.17, 0.16),
    ],
    facets: &[
        0xdec2ff, 0xb992f3, 0x805ab9, 0x684597, 0x7042a9, 0xa573df, 0x8155b6, 0xc2a0f4,
    ],
    center: (0.48, 0.47),
    table: 0xac7ee6,
    edge: 0xc7a3f7,
    highlight: 0xe9d6ff,
};

fn draw(gem: Gem, bounds: Bounds<Pixels>, finish: Finish, window: &mut Window) {
    let cut = match gem {
        Gem::Ruby => &RUBY,
        Gem::Amber => &AMBER,
        Gem::Jade => &JADE,
        Gem::Amethyst => &AMETHYST,
    };
    let mut table_vertices = [(0., 0.); 8];
    let table = &mut table_vertices[..cut.outline.len()];
    for (inside, &(x, y)) in table.iter_mut().zip(cut.outline) {
        *inside = (
            cut.center.0 + (x - cut.center.0) * 0.56,
            cut.center.1 + (y - cut.center.1) * 0.56,
        );
    }
    if matches!(finish, Finish::Ghost) {
        polygon(window, bounds, cut.outline, tint(cut.edge, 0.08), None);
        polygon(
            window,
            bounds,
            cut.outline,
            tint(cut.edge, 0.75),
            Some(1.05),
        );
        return;
    }

    let (bounds, alpha) = if let Finish::Clearing(progress) = finish {
        let fade = ((progress - 0.25) / 0.5).clamp(0., 1.);
        let mut smaller = bounds;
        let inset = bounds.size.width * (fade * 0.20);
        smaller.origin.x += inset;
        smaller.origin.y += inset;
        smaller.size.width -= inset * 2.;
        smaller.size.height -= inset * 2.;
        (smaller, 1. - fade)
    } else {
        (bounds, 1.)
    };

    if alpha > 0. {
        let mut shadow = bounds;
        shadow.origin.y += px(1.4);
        polygon(
            window,
            shadow,
            cut.outline,
            tint(0x080d0c, 0.68 * alpha),
            None,
        );
        polygon(window, bounds, cut.outline, tint(cut.table, alpha), None);
        for (index, &color) in cut.facets.iter().enumerate() {
            let next = (index + 1) % cut.outline.len();
            let face = [
                cut.outline[index],
                cut.outline[next],
                table[next],
                table[index],
            ];
            polygon(window, bounds, &face, tint(color, alpha), None);
        }
        polygon(
            window,
            bounds,
            table,
            tint(cut.highlight, 0.37 * alpha),
            Some(0.55),
        );
        polygon(
            window,
            bounds,
            cut.outline,
            tint(
                if matches!(finish, Finish::Active) {
                    0xfff9e4
                } else {
                    cut.edge
                },
                0.78 * alpha,
            ),
            Some(0.8),
        );
        let glint = if gem == Gem::Jade {
            [(0.36, 0.48), (0.49, 0.35), (0.46, 0.53)]
        } else {
            [(0.29, 0.31), (0.46, 0.24), (0.39, 0.39)]
        };
        polygon(
            window,
            bounds,
            &glint,
            tint(cut.highlight, 0.68 * alpha),
            None,
        );
    }

    if let Finish::Clearing(progress) = finish {
        let radius = 0.11 + 0.41 * progress;
        let glow = 1. - (2. * progress - 0.45).abs().min(1.);
        let spark = [
            (0.50, 0.49 - radius),
            (0.57, 0.42),
            (0.50 + radius, 0.49),
            (0.57, 0.56),
            (0.50, 0.49 + radius),
            (0.43, 0.56),
            (0.50 - radius, 0.49),
            (0.43, 0.42),
        ];
        polygon(window, bounds, &spark, tint(0xfff3bd, glow), None);
    }
}

fn tint(color: u32, alpha: f32) -> Hsla {
    Hsla {
        a: alpha,
        ..rgb(color).into()
    }
}

fn polygon(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    vertices: &[Vertex],
    color: Hsla,
    stroke: Option<f32>,
) {
    let vertex = |(x, y)| -> Point<Pixels> {
        bounds.origin + point(bounds.size.width * x, bounds.size.height * y)
    };
    let mut path = stroke.map_or_else(PathBuilder::fill, |width| PathBuilder::stroke(px(width)));
    path.move_to(vertex(vertices[0]));
    for &point in &vertices[1..] {
        path.line_to(vertex(point));
    }
    path.close();
    if let Ok(path) = path.build() {
        window.paint_path(path, color);
    }
}
