use corner_table_api::GameId;
use gpui::{
    AppContext, ClipboardItem, ExternalPaths, FileDropEvent, Modifiers, MouseButton, PinchEvent,
    Pixels, PlatformInput, Point, ScrollDelta, ScrollEvent, TestAppContext, TouchPhase,
    VisualTestContext, point, size,
};

use super::*;
use crate::games::GameEntry;

fn click(ui: &mut VisualTestContext, selector: &'static str) {
    let center = ui.debug_bounds(selector).expect(selector).center();
    ui.simulate_mouse_move(center, None, Modifiers::default());
    ui.simulate_click(center, Modifiers::default());
    ui.run_until_parked();
}

#[gpui::test]
fn assemble_result_keeps_undo_and_rounds_for_other_pictures(cx: &mut TestAppContext) {
    let launch = GameLaunch {
        game: GameId("jigsaw-assemble-result-test".into()),
        entry: GameEntry::Classic,
    };
    let viewport = size(px(760.), px(540.));
    let last = Size::Twelve.piece_count() - 1;
    let window = cx.open_window(viewport, |window, cx| {
        t::apply(t::Appearance::Light, window, cx);
        cx.set_reduce_motion(true);
        let mut view = GameView::new(&launch, cx);
        let art = artwork::cut_painting(&view.painting().source, view.key.size).unwrap();
        view.artwork.insert(view.key, Artwork::Ready(art));
        let round = view.round_mut();
        for piece in 0..last {
            round.assembly.place(piece, piece);
        }
        round.assembly_moves = last as u64;
        round.tray.selected = Some(last);
        round.tray.cursor = last;
        view.board_focus.focus(window, cx);
        view
    });
    cx.run_until_parked();
    let mut ui = VisualTestContext::from_window(window.into(), cx);
    assert!(ui.debug_bounds("jigsaw-play-again").is_none());

    ui.simulate_keystrokes("enter");
    let attempt = window
        .read_with(&ui, |view, _| {
            assert!(view.is_complete());
            assert_eq!(view.round().assembly_moves, (last + 1) as u64);
            assert!(view.settling_piece.is_none());
            view.round().assembly_attempt
        })
        .unwrap();
    assert!(ui.debug_bounds("jigsaw-play-again").unwrap().bottom() < viewport.height);
    window
        .update(&mut ui, |view, window, cx| {
            assert!(view.result_focus.is_focused(window));
            window.focus_next(cx);
            assert!(!view.result_focus.is_focused(window));
        })
        .unwrap();

    click(&mut ui, "jigsaw-next-picture");
    window
        .update(&mut ui, |view, window, cx| {
            assert_eq!(view.key.painting, 1);
            assert!(!view.is_complete());
            assert!(view.mode_focus.is_focused(window));
            view.choose(PuzzleKey {
                painting: 0,
                ..view.key
            });
            assert!(view.is_complete());
            cx.notify();
        })
        .unwrap();
    ui.run_until_parked();
    click(&mut ui, "jigsaw-undo");
    assert!(ui.debug_bounds("jigsaw-play-again").is_none());
    window
        .update(&mut ui, |view, window, cx| {
            assert_eq!(view.placed_count(), last);
            view.board_focus.focus(window, cx);
        })
        .unwrap();
    ui.simulate_keystrokes("enter");
    window
        .read_with(&ui, |view, _| {
            assert!(view.is_complete());
            assert!(view.round().assembly_attempt == attempt);
            assert_eq!(view.round().assembly_moves, (last + 2) as u64);
        })
        .unwrap();
    click(&mut ui, "jigsaw-play-again");
    window
        .read_with(&ui, |view, _| {
            assert_eq!(view.placed_count(), 0);
            assert!(view.round().assembly_attempt != attempt);
        })
        .unwrap();
    click(&mut ui, "jigsaw-swap");
    window
        .update(&mut ui, |view, window, _| {
            assert!(view.mode == Mode::Swap);
            assert!(view.mode_focus.is_focused(window));
        })
        .unwrap();
}

#[gpui::test]
fn swap_result_appears_after_the_final_tile_exchange(cx: &mut TestAppContext) {
    let launch = GameLaunch {
        game: GameId("jigsaw-swap-result-test".into()),
        entry: GameEntry::Classic,
    };
    let viewport = size(px(760.), px(540.));
    let window = cx.open_window(viewport, |window, cx| {
        t::apply(t::Appearance::Dark, window, cx);
        cx.set_reduce_motion(true);
        let mut view = GameView::new(&launch, cx);
        view.mode = Mode::Swap;
        let mut tiles: Vec<_> = (0..view.key.size.piece_count()).collect();
        tiles.swap(1, 2);
        view.round_mut().swap = Puzzle::restore(view.key.size, tiles, 10).unwrap();
        view
    });
    cx.run_until_parked();
    let mut ui = VisualTestContext::from_window(window.into(), cx);
    click(&mut ui, "jigsaw-tile-1");
    assert!(ui.debug_bounds("jigsaw-play-again").is_none());
    click(&mut ui, "jigsaw-tile-2");
    assert!(ui.debug_bounds("jigsaw-play-again").unwrap().bottom() < viewport.height);
    window
        .update(&mut ui, |view, window, _| {
            assert!(view.is_complete());
            assert_eq!(view.moves_text(), "11 moves");
            assert!(view.result_focus.is_focused(window));
        })
        .unwrap();
    click(&mut ui, "jigsaw-play-again");
    window
        .read_with(&ui, |view, _| {
            assert!(!view.is_complete());
            assert_eq!(view.round().swap.moves(), 0);
        })
        .unwrap();
}

#[gpui::test]
fn accepts_dropped_photos_and_copied_paths_without_the_picker(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("Pear on a blue plate.png");
    let photos = directory.path().join("gallery");
    image::RgbImage::from_pixel(24, 16, image::Rgb([76, 121, 160]))
        .save(&source)
        .unwrap();
    let launch = GameLaunch {
        game: GameId("jigsaw-picker-fallback-test".into()),
        entry: GameEntry::Classic,
    };
    let window = cx.open_window(size(px(760.), px(540.)), |_, cx| {
        let mut view = GameView::new(&launch, cx);
        view.gallery = Gallery::load(Some(photos.clone()));
        view.gallery.tab = gallery::Tab::Photos;
        view
    });
    cx.run_until_parked();
    let mut ui = VisualTestContext::from_window(window.into(), cx);
    click(&mut ui, "jigsaw-add-photo");
    assert!(cx.did_prompt_for_paths());
    cx.simulate_path_prompt_response(|_| None);
    cx.run_until_parked();

    let position = ui.debug_bounds("jigsaw-add-photo").unwrap().center();
    let paths = ExternalPaths([source.clone()].into_iter().collect());
    cx.update_window(window.into(), |_, window, cx| {
        window.dispatch_event(
            PlatformInput::FileDrop(FileDropEvent::Entered { position, paths }),
            cx,
        );
        window.dispatch_event(
            PlatformInput::FileDrop(FileDropEvent::Submit { position }),
            cx,
        );
    })
    .unwrap();
    ui.run_until_parked();
    window
        .read_with(&ui, |view, _| {
            assert_eq!(view.gallery.photos().len(), 1);
            assert_eq!(view.key.painting, GALLERY.len());
        })
        .unwrap();

    window
        .update(&mut ui, |_, _, cx| {
            cx.write_to_clipboard(ClipboardItem::new_string(format!(
                "\"{}\"",
                source.display()
            )));
        })
        .unwrap();
    click(&mut ui, "jigsaw-paste-photo");
    window
        .read_with(&ui, |view, _| {
            assert_eq!(view.gallery.photos().len(), 2);
            assert_eq!(view.key.painting, GALLERY.len() + 1);
        })
        .unwrap();
    assert_eq!(photos::stored(&photos).unwrap().len(), 2);
}

#[gpui::test]
fn assembly_worktable_keeps_loose_pieces_and_only_snaps_matching_cells(cx: &mut TestAppContext) {
    let launch = GameLaunch {
        game: GameId("jigsaw-worktable-test".into()),
        entry: GameEntry::Classic,
    };
    let window = cx.open_window(size(px(1040.), px(740.)), |window, cx| {
        t::apply(t::Appearance::Dark, window, cx);
        cx.set_reduce_motion(true);
        let mut view = GameView::new(&launch, cx);
        let seed = (0..)
            .find(|&seed| {
                Assembly::new(Size::Twelve, seed)
                    .tray(TrayFilter::All)
                    .next()
                    == Some(5)
            })
            .unwrap();
        *view.round_mut() = Round::new(Size::Twelve, seed);
        let art = artwork::cut_painting(&view.painting().source, view.key.size).unwrap();
        view.artwork.insert(view.key, Artwork::Ready(art));
        view
    });
    cx.run_until_parked();
    let mut ui = VisualTestContext::from_window(window.into(), cx);
    let (piece, (columns, rows)) = window
        .read_with(&ui, |view, _| {
            (
                view.round().assembly.tray(TrayFilter::All).next().unwrap(),
                view.key.size.dimensions(),
            )
        })
        .unwrap();
    click(&mut ui, "jigsaw-grid");
    window
        .read_with(&ui, |view, _| assert!(!view.show_grid))
        .unwrap();
    let drag = |ui: &mut VisualTestContext, from: Point<Pixels>, to: Point<Pixels>| {
        let modifiers = Modifiers::default();
        ui.simulate_mouse_move(from, None, modifiers);
        ui.simulate_mouse_down(from, MouseButton::Left, modifiers);
        ui.simulate_mouse_move(
            from + point(px(9.), px(2.)),
            Some(MouseButton::Left),
            modifiers,
        );
        ui.simulate_mouse_move(to, Some(MouseButton::Left), modifiers);
        ui.simulate_mouse_up(to, MouseButton::Left, modifiers);
        ui.run_until_parked();
    };
    let center = |ui: &mut VisualTestContext, index: usize| {
        let board = ui.debug_bounds("jigsaw-assembly-board").unwrap();
        board.origin
            + point(
                board.size.width * ((index % columns) as f32 + 0.5) / columns as f32,
                board.size.height * ((index / columns) as f32 + 0.5) / rows as f32,
            )
    };
    let pile: &'static str = Box::leak(format!("jigsaw-tray-piece-{piece}").into_boxed_str());
    let loose: &'static str = Box::leak(format!("jigsaw-loose-piece-{piece}").into_boxed_str());
    let start = ui.debug_bounds(pile).unwrap().center();
    let wrong = center(&mut ui, (piece + 1) % (columns * rows));
    drag(&mut ui, start, wrong);
    let first = window
        .read_with(&ui, |view, _| {
            assert_eq!(view.placed_count(), 0);
            assert_eq!(view.round().loose.len(), 1);
            view.round().loose[0]
        })
        .unwrap();
    assert!(ui.debug_bounds(pile).is_none());
    click(&mut ui, "jigsaw-zoom-in");
    let start = ui.debug_bounds(loose).unwrap().center();
    let wrong = center(&mut ui, (piece + 2) % (columns * rows));
    drag(&mut ui, start, wrong);
    let moved = window
        .read_with(&ui, |view, _| {
            assert_eq!(view.placed_count(), 0);
            assert_ne!(view.round().loose[0].position, first.position);
            view.round().loose[0]
        })
        .unwrap();
    let start = ui.debug_bounds(loose).unwrap().center();
    let correct = center(&mut ui, piece);
    drag(&mut ui, start, correct);
    window
        .read_with(&ui, |view, _| {
            assert_eq!(view.round().assembly.placements(), &[piece])
        })
        .unwrap();
    assert!(ui.debug_bounds(loose).is_none());
    click(&mut ui, "jigsaw-undo");
    assert!(ui.debug_bounds(loose).is_some());
    click(&mut ui, "jigsaw-fit");
    window
        .read_with(&ui, |view, _| {
            assert_eq!(view.placed_count(), 0);
            assert_eq!(view.round().loose, [moved]);
        })
        .unwrap();
}

#[gpui::test]
fn worktable_view_pans_and_resumes_without_moving_loose_pieces(cx: &mut TestAppContext) {
    let launch = GameLaunch {
        game: GameId("jigsaw-camera-test".into()),
        entry: GameEntry::Classic,
    };
    let viewport = size(px(760.), px(540.));
    let loose = LoosePiece {
        piece: 6,
        position: [0.25, 0.6],
    };
    let window = cx.open_window(viewport, |window, cx| {
        t::apply(t::Appearance::Light, window, cx);
        let mut view = GameView::new(&launch, cx);
        view.round_mut().loose.push(loose);
        view
    });
    cx.run_until_parked();
    let mut ui = VisualTestContext::from_window(window.into(), cx);
    let fitted = ui.debug_bounds("jigsaw-assembly-board").unwrap();
    click(&mut ui, "jigsaw-zoom-in");
    let zoomed = ui.debug_bounds("jigsaw-assembly-board").unwrap();
    assert_eq!(zoomed.size.width, fitted.size.width * 1.25);
    assert!(zoomed.origin.x < fitted.origin.x);

    click(&mut ui, "jigsaw-pan");
    let position = ui.debug_bounds("jigsaw-worktable").unwrap().center();
    let end = position + point(px(38.), px(5.));
    let modifiers = Modifiers::default();
    ui.simulate_mouse_move(position, None, modifiers);
    ui.simulate_mouse_down(position, MouseButton::Left, modifiers);
    ui.simulate_mouse_move(end, Some(MouseButton::Left), modifiers);
    ui.simulate_mouse_up(end, MouseButton::Left, modifiers);
    ui.run_until_parked();
    let panned = ui.debug_bounds("jigsaw-assembly-board").unwrap();
    assert_eq!(panned.origin.x - zoomed.origin.x, px(38.));
    ui.simulate_keystrokes("down escape");
    window
        .read_with(&ui, |view, _| {
            assert!(!view.pan_mode);
            assert!(view.round().camera.offset()[1] < -0.125);
        })
        .unwrap();

    cx.update_window(window.into(), |_, window, cx| {
        window.dispatch_event(
            PlatformInput::Scroll(ScrollEvent {
                position,
                delta: ScrollDelta::Lines(point(0., 1.)),
                modifiers: Modifiers {
                    control: true,
                    ..Modifiers::default()
                },
                ..ScrollEvent::default()
            }),
            cx,
        );
    })
    .unwrap();
    let before_pinch = window
        .read_with(&ui, |view, _| {
            assert!(view.round().camera.zoom() > 1.25);
            view.round().camera
        })
        .unwrap();
    cx.update_window(window.into(), |_, window, cx| {
        for (phase, delta) in [
            (TouchPhase::Started, 0.),
            (TouchPhase::Moved, 0.2),
            (TouchPhase::Cancelled, 0.),
        ] {
            window.dispatch_event(
                PlatformInput::Pinch(PinchEvent {
                    position,
                    phase,
                    delta,
                    ..PinchEvent::default()
                }),
                cx,
            );
        }
    })
    .unwrap();
    window
        .read_with(&ui, |view, _| {
            assert_eq!(view.round().camera, before_pinch);
            assert_eq!(view.round().loose, [loose]);
        })
        .unwrap();

    let restored = cx.open_window(viewport, |_, cx| GameView::new(&launch, cx));
    cx.run_until_parked();
    let mut ui = VisualTestContext::from_window(restored.into(), cx);
    restored
        .read_with(&ui, |view, _| {
            assert_eq!(view.round().camera, before_pinch);
            assert_eq!(view.round().loose, [loose]);
        })
        .unwrap();
    click(&mut ui, "jigsaw-fit");
    assert_eq!(ui.debug_bounds("jigsaw-assembly-board").unwrap(), fitted);
}
