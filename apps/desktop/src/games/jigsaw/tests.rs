use corner_table_api::GameId;
use gpui::{
    AppContext, ClipboardItem, ExternalPaths, FileDropEvent, Modifiers, PlatformInput,
    TestAppContext, VisualTestContext, size,
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
