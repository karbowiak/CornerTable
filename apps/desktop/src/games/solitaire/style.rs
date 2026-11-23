use corner_table_api::GameId;
use gpui::{
    App, BoxShadow, Context, Div, Global, ObjectFit, Render, Role, Stateful, Toggled, Window, div,
    img, prelude::*, px, rgb, rgba,
};
use serde::{Deserialize, Serialize};

use crate::{
    games::{GameEntry, GameLaunch},
    storage::Session,
    theme as t,
};

use super::GameView;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum TableStyle {
    #[default]
    GreenBaize,
    IndigoLinen,
    OxbloodLeather,
}

impl Global for TableStyle {}

impl TableStyle {
    const ALL: [Self; 3] = [Self::GreenBaize, Self::IndigoLinen, Self::OxbloodLeather];

    pub(super) fn init(cx: &mut Context<GameView>) {
        if !cx.has_global::<Self>() {
            cx.set_global(Self::session().load::<Self>().unwrap_or_default());
        }
        cx.observe_global::<Self>(|_, cx| cx.notify()).detach();
    }

    pub(super) fn active(cx: &App) -> Self {
        cx.try_global::<Self>().copied().unwrap_or_default()
    }

    fn select(self, cx: &mut App) {
        if self != Self::active(cx) {
            Self::session().save(&self);
            cx.set_global(self);
        }
    }

    fn session() -> Session {
        // All Solitaire launches share this preference. Deal replays and
        // selected rule modes are stored under their own launch keys.
        Session::new(&GameLaunch {
            game: GameId("solitaire".into()),
            entry: GameEntry::Mode("solitaire-table-style".into()),
        })
    }

    fn label(self) -> &'static str {
        match self {
            Self::GreenBaize => "Green Baize",
            Self::IndigoLinen => "Indigo Linen",
            Self::OxbloodLeather => "Oxblood Leather",
        }
    }

    pub(super) fn palette(self) -> &'static TablePalette {
        match self {
            Self::GreenBaize => &GREEN_BAIZE,
            Self::IndigoLinen => &INDIGO_LINEN,
            Self::OxbloodLeather => &OXBLOOD_LEATHER,
        }
    }
}

pub(super) struct TablePalette {
    pub surface: u32,
    pub surface_art: &'static str,
    pub frame: u32,
    pub rule: u32,
    pub text: u32,
    pub accent: u32,
    pub meter: u32,
    pub meter_fill: u32,
    pub counter: u32,
    pub slot: u32,
    pub slot_border: u32,
    pub slot_mark: u32,
    pub drop_slot: u32,
    pub back: CardBack,
}

pub(super) struct CardBack {
    pub art: &'static str,
    pub field: u32,
    pub paper: u32,
    pub edge: u32,
}

const GREEN_BAIZE: TablePalette = TablePalette {
    surface: 0x153b30,
    surface_art: "games/solitaire/baize.webp",
    frame: 0x0d2d24,
    rule: 0xc7d7b33b,
    text: 0xdae7d6,
    accent: 0xf5dfaa,
    meter: 0x426052,
    meter_fill: 0xd9ba75,
    counter: 0x305747,
    slot: 0x15392f,
    slot_border: 0x769986,
    slot_mark: 0xaac2aa,
    drop_slot: 0x44745a,
    back: CardBack {
        art: "games/solitaire/card-back.webp",
        field: 0x214b52,
        paper: 0xe7d4ac,
        edge: 0xccb38a,
    },
};

const INDIGO_LINEN: TablePalette = TablePalette {
    surface: 0x25324a,
    surface_art: "games/solitaire/indigo-linen.webp",
    frame: 0x172032,
    rule: 0xb3c6e455,
    text: 0xdce5f5,
    accent: 0xc4ddff,
    meter: 0x485974,
    meter_fill: 0x9ebfe9,
    counter: 0x394d6c,
    slot: 0x1c273b,
    slot_border: 0x849bbb,
    slot_mark: 0xb8cbea,
    drop_slot: 0x425d80,
    back: CardBack {
        art: "games/solitaire/indigo-back.webp",
        field: 0x283e68,
        paper: 0xd4e3ed,
        edge: 0xa5bdcf,
    },
};

const OXBLOOD_LEATHER: TablePalette = TablePalette {
    surface: 0x45272f,
    surface_art: "games/solitaire/oxblood-leather.webp",
    frame: 0x2c1720,
    rule: 0xdcb6a24f,
    text: 0xf2ddd5,
    accent: 0xffd5a8,
    meter: 0x714d51,
    meter_fill: 0xe9ac85,
    counter: 0x633f44,
    slot: 0x351e28,
    slot_border: 0xb78d88,
    slot_mark: 0xe2bab1,
    drop_slot: 0x7c494d,
    back: CardBack {
        art: "games/solitaire/oxblood-back.webp",
        field: 0x632c37,
        paper: 0xf4d5ad,
        edge: 0xdcab80,
    },
};

impl TablePalette {
    pub(super) fn surface_image(&self) -> gpui::Img {
        img(self.surface_art).object_fit(ObjectFit::Fill)
    }
}

impl CardBack {
    pub(super) fn image(&self) -> gpui::Img {
        img(self.art).object_fit(ObjectFit::Fill)
    }
}

pub(super) fn picker(active: TableStyle) -> Stateful<Div> {
    let palette = active.palette();
    div()
        .id("solitaire-table-style")
        .role(Role::Group)
        .aria_label("Table style")
        .flex()
        .flex_shrink_0()
        .items_center()
        .gap(px(4.))
        .child(div().mr_1().text_color(rgb(palette.text)).child("Table"))
        .children(TableStyle::ALL.into_iter().map(|choice| {
            let preview = choice.palette();
            let selected = choice == active;
            let label = choice.label();
            div()
                .id(label)
                .role(Role::Button)
                .aria_label(label)
                .aria_toggled(if selected {
                    Toggled::True
                } else {
                    Toggled::False
                })
                .tab_index(0)
                .relative()
                .flex()
                .items_center()
                .justify_center()
                .w(px(30.))
                .h(px(26.))
                .border_2()
                .border_color(rgb(if selected {
                    palette.accent
                } else {
                    palette.slot_border
                }))
                .rounded_md()
                .bg(rgb(preview.surface))
                .cursor_pointer()
                .when(!selected, |button| {
                    button.hover(|style| style.border_color(rgb(palette.text)))
                })
                .focus_visible(|style| {
                    style.border_color(rgb(t::SURFACE)).shadow(vec![
                        BoxShadow::new(px(0.), px(0.), rgb(palette.accent).into())
                            .spread_radius(px(2.)),
                    ])
                })
                .child(
                    preview
                        .surface_image()
                        .absolute()
                        .inset_0()
                        .size_full()
                        .rounded(px(3.)),
                )
                .child(
                    div()
                        .relative()
                        .w(px(12.))
                        .h(px(17.))
                        .border_1()
                        .border_color(rgb(preview.back.edge))
                        .bg(rgb(preview.back.field))
                        .rounded(px(2.))
                        .child(preview.back.image().size_full().rounded(px(1.))),
                )
                .tooltip(move |_, cx| cx.new(|_| StyleTooltip(label)).into())
                .on_click(move |_, _, cx| choice.select(cx))
        }))
}

struct StyleTooltip(&'static str);

impl Render for StyleTooltip {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = t::colors(cx);
        div()
            .bg(rgb(palette.surface))
            .border_1()
            .border_color(rgb(palette.border))
            .rounded_md()
            .px_2()
            .py_1()
            .text_size(px(12.))
            .text_color(rgb(palette.ink))
            .shadow(vec![
                BoxShadow::new(px(0.), px(2.), rgba(0x0e182040).into()).blur_radius(px(6.)),
            ])
            .child(self.0)
    }
}
