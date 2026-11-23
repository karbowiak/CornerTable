use corner_table_block_puzzle::Wood;
use gpui::{
    Background, BoxShadow, Div, ObjectFit, div, img, linear_color_stop, linear_gradient, point,
    prelude::*, px, rgb, rgba,
};

pub(super) fn board_frame() -> Div {
    div()
        .absolute()
        .inset_0()
        .child(
            img("games/block-puzzle/walnut-board.jpg")
                .size_full()
                .object_fit(ObjectFit::Fill)
                .rounded(px(11.)),
        )
        .child(
            div()
                .absolute()
                .inset_0()
                .rounded(px(11.))
                .bg(linear_gradient(
                    150.,
                    linear_color_stop(rgba(0xf9ca9242), 0.),
                    linear_color_stop(rgba(0x27140ec2), 1.),
                )),
        )
        .child(
            div()
                .absolute()
                .inset(px(5.))
                .rounded(px(7.))
                .border_1()
                .border_color(rgba(0x180c07be)),
        )
}

pub(super) fn recess() -> Background {
    linear_gradient(
        165.,
        linear_color_stop(rgba(0x21120ced), 0.),
        linear_color_stop(rgba(0x472e21af), 1.),
    )
}

pub(super) fn wood_tile(wood: Wood, cell: f32) -> Div {
    let (texture, face, lit_edge, dark_edge) = match wood {
        Wood::Amber => ("games/block-puzzle/amber.jpg", 0xd6a456, 0xf3ce87, 0x795025),
        Wood::Cherry => (
            "games/block-puzzle/cherry.jpg",
            0xd38374,
            0xeabb9b,
            0x793d33,
        ),
        Wood::Juniper => (
            "games/block-puzzle/juniper.jpg",
            0xa3ae70,
            0xcbd29a,
            0x505837,
        ),
        Wood::Plum => ("games/block-puzzle/plum.jpg", 0xb096b8, 0xd5bbda, 0x5e4369),
        Wood::Lagoon => (
            "games/block-puzzle/lagoon.jpg",
            0x76adb2,
            0xa8dad6,
            0x2d5c63,
        ),
    };
    let bevel = (cell * 0.055).clamp(1., 2.5);
    let depth = (cell * 0.11).clamp(1.5, 4.5);
    let radius = px((cell * 0.115).clamp(2., 5.));
    let face_radius = radius - px(bevel * 0.5);

    div()
        .relative()
        .size(px(cell))
        .rounded(radius)
        .bg(linear_gradient(
            155.,
            linear_color_stop(rgb(lit_edge), 0.),
            linear_color_stop(rgb(dark_edge), 0.93),
        ))
        .shadow(vec![BoxShadow {
            inset: false,
            color: rgba(0x160d0866).into(),
            offset: point(px(0.), px(0.7)),
            blur_radius: px(1.4),
            spread_radius: px(0.),
        }])
        .child(
            div()
                .absolute()
                .top(px(bevel))
                .left(px(bevel))
                .right(px(bevel))
                .bottom(px(depth))
                .rounded(face_radius)
                .bg(rgb(face))
                .child(
                    img(texture)
                        .size_full()
                        .rounded(face_radius)
                        .object_fit(ObjectFit::Fill),
                )
                .child(
                    div()
                        .absolute()
                        .inset_0()
                        .rounded(face_radius)
                        .bg(linear_gradient(
                            145.,
                            linear_color_stop(rgba(0xfff0d84a), 0.),
                            linear_color_stop(rgba(0x2a14232b), 1.),
                        )),
                ),
        )
}
