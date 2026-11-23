use corner_table_solitaire::{DealMode, Game, Hint, Move, Place, Variant};
use serde::{Deserialize, Serialize};

use crate::{
    games::{GameEntry, GameLaunch},
    storage::{AttemptId, Outcome, ResultMetric, Session},
};

use super::mode_label;

// Increment when a deal generator or move encoding changes incompatibly.
const REPLAY_VERSION: u32 = 1;

#[derive(Deserialize, Serialize)]
struct SavedTable {
    seed: u64,
    mode: String,
}

pub(super) struct TableSession {
    session: Session,
    seed: u64,
}

impl TableSession {
    pub(super) fn open(launch: &GameLaunch, variant: Variant) -> (Self, Deal, Vec<Deal>) {
        let session = Session::new(launch);
        let saved = session.load::<SavedTable>();
        let seed = saved
            .as_ref()
            .map_or_else(|| launch.seed(), |saved| saved.seed);
        let modes = variant.deal_modes();
        let selected = saved
            .as_ref()
            .and_then(|saved| modes.iter().position(|&mode| mode_id(mode) == saved.mode))
            .unwrap_or(0);
        let mut deals: Vec<_> = modes
            .iter()
            .map(|&mode| Deal::open(launch, mode, seed))
            .collect();
        let deal = deals.swap_remove(selected);
        let table = Self { session, seed };
        if saved.is_none() {
            table.save_mode(deal.game.mode());
        }
        (table, deal, deals)
    }

    pub(super) fn save_mode(&self, mode: DealMode) {
        self.session.save(&SavedTable {
            seed: self.seed,
            mode: mode_id(mode).into(),
        });
    }
}

pub(super) struct Deal {
    session: Session,
    saved: SavedDeal,
    pub(super) game: Game,
    pub(super) selected: Option<Place>,
    pub(super) status: &'static str,
}

#[derive(Deserialize, Serialize)]
struct SavedDeal {
    version: u32,
    seed: u64,
    turns: Vec<Turn>,
    #[serde(default)]
    attempt: AttemptId,
    #[serde(default)]
    best_moves: Option<usize>,
    // An explicit Finish appends legal turns, but Undo treats it as one action.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    finish_start: Option<usize>,
}

impl SavedDeal {
    fn new(seed: u64) -> Self {
        Self {
            version: REPLAY_VERSION,
            seed,
            turns: Vec::new(),
            attempt: AttemptId::default(),
            best_moves: None,
            finish_start: None,
        }
    }

    fn replay(&self, mode: DealMode) -> Option<Game> {
        if self.version != REPLAY_VERSION {
            return None;
        }
        Game::replay(mode, self.seed, self.turns.iter().map(|turn| turn.0))
    }

    fn record_win(&mut self, game: &Game) {
        if game.won() && self.best_moves.is_none_or(|best| game.moves < best) {
            self.best_moves = Some(game.moves);
        }
    }
}

impl Deal {
    fn open(launch: &GameLaunch, mode: DealMode, seed: u64) -> Self {
        // A broken replay affects only its own deal style. Launch kind and ID
        // keep collection deals separate from challenge or classic entries.
        let entry = match &launch.entry {
            GameEntry::Classic => String::from("classic"),
            GameEntry::Mode(id) => format!("mode/{id}"),
            GameEntry::Challenge(id) => format!("challenge/{id}"),
        };
        let session = Session::new(&GameLaunch {
            game: launch.game.clone(),
            entry: GameEntry::Mode(format!("solitaire-deal/{entry}/{}", mode_id(mode))),
        });
        let (game, mut saved) = session
            .load::<SavedDeal>()
            .and_then(|saved| Some((saved.replay(mode)?, saved)))
            .unwrap_or_else(|| (Game::with_mode(mode, seed), SavedDeal::new(seed)));
        saved.best_moves = saved.best_moves.filter(|&moves| moves > 0);
        saved.finish_start = saved
            .finish_start
            .filter(|&start| game.won() && start < saved.turns.len());
        saved.record_win(&game);
        Self {
            session,
            saved,
            game,
            selected: None,
            status: "",
        }
    }

    pub(super) fn best_moves(&self) -> Option<usize> {
        self.saved.best_moves
    }

    pub(super) fn hint(&self) -> Option<Hint> {
        const RECENT_POSITIONS: usize = 8;
        let turns = &self.saved.turns;
        let (prefix, recent_turns) = turns.split_at(turns.len().saturating_sub(RECENT_POSITIONS));
        let mut board = Game::replay(
            self.game.mode(),
            self.saved.seed,
            prefix.iter().map(|t| t.0),
        )
        .expect("accepted moves replay from their deal seed");
        let recent: Vec<_> = recent_turns
            .iter()
            .map(|turn| {
                let before = board.clone();
                board.play(turn.0);
                before
            })
            .collect();
        self.game.hint(&recent)
    }

    pub(super) fn restart(&mut self, new_seed: Option<u64>) {
        let mode = self.game.mode();
        let seed = match (new_seed, mode) {
            // Walk the catalog from the opening seed so New deal shows a
            // different shuffle on each visit until the catalog wraps.
            (Some(_), DealMode::KlondikeDrawOneVerified) => self.saved.seed.wrapping_add(1),
            (seed, _) => seed.unwrap_or(self.saved.seed),
        };
        self.game = Game::with_mode(mode, seed);
        self.saved.seed = seed;
        self.saved.turns.clear();
        self.saved.finish_start = None;
        self.saved.attempt = AttemptId::default();
        self.save();
    }

    pub(super) fn apply(&mut self, action: Move) -> bool {
        if !self.game.play(action) {
            return false;
        }
        self.saved.turns.push(Turn(action));
        self.record_progress();
        true
    }

    pub(super) fn finish(&mut self) -> bool {
        let Some(actions) = self.game.collection_moves() else {
            return false;
        };
        let mut completed = self.game.clone();
        if !actions.iter().all(|&action| completed.play(action)) || !completed.won() {
            return false;
        }
        self.saved.finish_start = Some(self.saved.turns.len());
        self.saved.turns.extend(actions.into_iter().map(Turn));
        self.game = completed;
        self.record_progress();
        true
    }

    fn record_progress(&mut self) {
        self.saved.record_win(&self.game);
        if self.game.won() {
            let mode = self.game.mode();
            let variant = self.game.variant;
            let label = if variant.deal_modes().len() == 1 {
                variant.title().to_owned()
            } else {
                format!("{} · {}", variant.title(), mode_label(mode))
            };
            self.session.record_result(
                self.saved.attempt,
                label,
                Outcome::Won,
                ResultMetric::Moves(self.game.moves as u64),
            );
        }
        self.save();
    }

    pub(super) fn can_undo(&self) -> bool {
        !self.saved.turns.is_empty()
    }

    pub(super) fn undo(&mut self) -> bool {
        if !self.can_undo() {
            return false;
        }
        let keep = self
            .saved
            .finish_start
            .take()
            .unwrap_or(self.saved.turns.len() - 1);
        self.saved.turns.truncate(keep);
        // These moves were accepted by the same rules, either during play or on load.
        self.game = self
            .saved
            .replay(self.game.mode())
            .expect("accepted moves replay from their deal seed");
        self.save();
        true
    }

    fn save(&mut self) {
        self.selected = None;
        self.status = "";
        self.session.save(&self.saved);
    }
}

fn mode_id(mode: DealMode) -> &'static str {
    match mode {
        DealMode::KlondikeDrawOne => "draw-one",
        DealMode::KlondikeDrawThree => "draw-three",
        DealMode::KlondikeDrawOneVerified => "draw-one-verified",
        DealMode::SpiderOneSuit => "one-suit",
        DealMode::SpiderTwoSuits => "two-suits",
        DealMode::SpiderFourSuits => "four-suits",
        DealMode::FreeCell => "freecell",
        DealMode::Pyramid => "pyramid",
        DealMode::TriPeaks => "tripeaks",
        DealMode::TriPeaksSolvable => "tripeaks-solvable",
    }
}

// Keep the on-disk encoding in the desktop, independent of the rules crate.
#[derive(Deserialize, Serialize)]
#[serde(transparent)]
struct Turn(#[serde(with = "SavedMove")] Move);

#[derive(Deserialize, Serialize)]
#[serde(remote = "Move", rename_all = "kebab-case")]
enum SavedMove {
    Draw,
    Transfer {
        #[serde(with = "SavedPlace")]
        from: Place,
        #[serde(with = "SavedPlace")]
        to: Place,
    },
    Clear(#[serde(with = "SavedPlace")] Place),
}

#[derive(Deserialize, Serialize)]
#[serde(remote = "Place", rename_all = "kebab-case")]
enum SavedPlace {
    Stock,
    Waste,
    Tableau(usize, usize),
    Foundation(usize),
    Cell(usize),
    Layout(usize),
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(debug_assertions)]
    use crate::{
        games::solitaire::{GameView, Suggestion, TableStyle},
        storage,
    };
    #[cfg(debug_assertions)]
    use corner_table_api::GameId;
    #[cfg(debug_assertions)]
    use gpui::{Modifiers, MouseButton, TestAppContext, VisualTestContext, point, px, size};

    #[cfg(debug_assertions)]
    #[gpui::test]
    fn hint_button_and_shortcut_mark_cards_without_changing_the_saved_round(
        cx: &mut TestAppContext,
    ) {
        let launch = GameLaunch {
            game: GameId(format!(
                "solitaire-hint-controls-test-{}",
                std::process::id()
            )),
            entry: GameEntry::Mode("klondike".into()),
        };
        let (seed, hint) = (0..32)
            .find_map(|seed| {
                let hint = Game::new(Variant::Klondike, seed).hint(&[])?;
                matches!(hint.action, Move::Transfer { .. }).then_some((seed, hint))
            })
            .unwrap();
        let window = cx.open_window(size(px(760.), px(540.)), |_, cx| {
            let mut view = GameView::new(&launch, cx);
            view.restart(Some(seed));
            view
        });
        let mut visual = VisualTestContext::from_window(window.into(), cx);
        draw(&mut visual);
        visual.update(|window, cx| window.simulate_next_frame(cx));
        let saved = |view: &GameView| serde_json::to_vec(&view.deal.saved).unwrap();
        let read_preferences = |view: &GameView| {
            serde_json::to_vec(&view.session.session.load::<SavedTable>().unwrap()).unwrap()
        };
        let before = window
            .read_with(cx, |view, cx| {
                (
                    view.deal.game.clone(),
                    saved(view),
                    read_preferences(view),
                    view.revision,
                    TableStyle::active(cx),
                )
            })
            .unwrap();

        visual.simulate_keystrokes("alt-h");
        assert!(
            window
                .read_with(cx, |view, _| view.suggestion.is_none())
                .unwrap()
        );
        visual.simulate_keystrokes("h");
        draw(&mut visual);
        assert!(window.read_with(cx, |view, _| matches!(view.suggestion, Some(Suggestion::Move(shown)) if shown == hint)).unwrap());
        let source = visual.debug_bounds("solitaire-hint-source").unwrap();
        let target = visual.debug_bounds("solitaire-hint-target").unwrap();
        assert_ne!(source.origin, target.origin);
        assert!(target.right() <= px(760.));

        window
            .update(cx, |view, _, cx| {
                view.suggestion = None;
                cx.notify();
            })
            .unwrap();
        draw(&mut visual);
        let hint_button = visual.debug_bounds("solitaire-hint-button").unwrap();
        assert!(hint_button.right() <= px(760.));
        visual.simulate_click(hint_button.center(), Modifiers::none());
        draw(&mut visual);
        window
            .read_with(cx, |view, cx| {
                assert!(matches!(view.suggestion, Some(Suggestion::Move(shown)) if shown == hint));
                assert_eq!(view.deal.game, before.0);
                assert_eq!(saved(view), before.1);
                assert_eq!(read_preferences(view), before.2);
                assert_eq!(view.revision, before.3);
                assert_eq!(TableStyle::active(cx), before.4);
                assert!(!view.deal.can_undo());
            })
            .unwrap();
        assert!(
            storage::recent_results(usize::MAX)
                .iter()
                .all(|r| r.game != launch.game)
        );

        visual.simulate_click(source.center(), Modifiers::none());
        draw(&mut visual);
        assert_eq!(visual.debug_bounds("solitaire-hint-target"), Some(target));
        visual.simulate_click(target.center(), Modifiers::none());
        draw(&mut visual);
        window
            .read_with(cx, |view, _| {
                let mut expected = before.0.clone();
                assert!(expected.play(hint.action));
                assert_eq!(view.deal.game, expected);
                assert!(view.suggestion.is_none());
            })
            .unwrap();
        let undo = visual.debug_bounds("solitaire-undo-button").unwrap();
        visual.simulate_click(undo.center(), Modifiers::none());
        window
            .read_with(cx, |view, _| assert_eq!(view.deal.game, before.0))
            .unwrap();
    }

    #[cfg(debug_assertions)]
    #[gpui::test]
    fn h_works_on_entry_through_the_desktop_game_host(cx: &mut TestAppContext) {
        use crate::games::GameHost;
        use gpui::{Context, Entity, FocusHandle, IntoElement, Render, Window, div, prelude::*};

        struct GamePage {
            focus: FocusHandle,
            host: Entity<GameHost>,
        }

        impl Render for GamePage {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                div().track_focus(&self.focus).child(self.host.clone())
            }
        }

        let launch = GameLaunch {
            game: GameId("solitaire-hint-host-test".into()),
            entry: GameEntry::Challenge(String::new()),
        };
        let window = cx.open_window(size(px(1080.), px(820.)), |window, cx| {
            let view = cx.new(|cx| GameView::new(&launch, cx));
            let host = cx.new(|cx| GameHost::new(view.into(), window, cx));
            let focus = cx.focus_handle();
            focus.focus(window, cx);
            GamePage { focus, host }
        });
        let mut visual = VisualTestContext::from_window(window.into(), cx);
        draw(&mut visual);
        visual.update(|window, cx| window.simulate_next_frame(cx));
        visual.simulate_keystrokes("h");
        draw(&mut visual);
        assert!(visual.debug_bounds("solitaire-hint-source").is_some());
    }

    #[cfg(debug_assertions)]
    #[gpui::test]
    fn a_hinted_stack_can_still_be_dragged(cx: &mut TestAppContext) {
        let (launch, turns) = verified_prefix("drag", |board, recent| {
            matches!(board.hint(recent).map(|hint| hint.action),
                Some(Move::Transfer { from: Place::Tableau(column, index), to: Place::Tableau(..) })
                    if board.tableau[column].len() - index > 1)
        });
        let window = cx.open_window(size(px(1080.), px(820.)), |_, cx| {
            let mut view = GameView::new(&launch, cx);
            view.switch_mode(DealMode::KlondikeDrawOneVerified);
            for turn in turns {
                assert!(view.deal.apply(turn));
            }
            view
        });
        let mut visual = VisualTestContext::from_window(window.into(), cx);
        draw(&mut visual);
        let button = visual.debug_bounds("solitaire-hint-button").unwrap();
        visual.simulate_click(button.center(), Modifiers::none());
        draw(&mut visual);
        let source = visual
            .debug_bounds("solitaire-hint-source")
            .unwrap()
            .center();
        let target = visual
            .debug_bounds("solitaire-hint-target")
            .unwrap()
            .center();
        let expected = window
            .read_with(cx, |view, _| {
                let Some(Suggestion::Move(hint)) = view.suggestion else {
                    panic!("expected a suggested stack transfer");
                };
                let mut expected = view.deal.game.clone();
                assert!(expected.play(hint.action));
                expected
            })
            .unwrap();

        visual.simulate_mouse_down(source, MouseButton::Left, Modifiers::none());
        visual.simulate_mouse_move(
            source - point(px(8.), px(0.)),
            MouseButton::Left,
            Modifiers::none(),
        );
        visual.simulate_mouse_move(target, MouseButton::Left, Modifiers::none());
        visual.simulate_mouse_up(target, MouseButton::Left, Modifiers::none());
        window
            .read_with(cx, |view, _| {
                assert_eq!(view.deal.game, expected);
                assert!(view.suggestion.is_none());
            })
            .unwrap();
    }

    #[cfg(debug_assertions)]
    #[gpui::test]
    fn finish_control_records_a_win_and_undo_restores_the_entire_collection(
        cx: &mut TestAppContext,
    ) {
        let mode = DealMode::KlondikeDrawOneVerified;
        let (launch, turns) = verified_prefix("finish", |board, _| {
            board
                .collection_moves()
                .is_some_and(|moves| moves.len() > 1)
        });
        let window = cx.open_window(size(px(760.), px(540.)), |_, cx| {
            let mut view = GameView::new(&launch, cx);
            view.switch_mode(mode);
            for &turn in &turns {
                assert!(view.deal.apply(turn));
            }
            view
        });
        let mut visual = VisualTestContext::from_window(window.into(), cx);
        draw(&mut visual);
        let before = window
            .read_with(cx, |view, _| view.deal.game.clone())
            .unwrap();
        let number_collected = before.collection_moves().unwrap().len();
        let finish = visual.debug_bounds("solitaire-finish-button").unwrap();
        assert!(finish.right() <= px(760.));
        visual.simulate_click(finish.center(), Modifiers::none());
        draw(&mut visual);
        assert!(visual.debug_bounds("solitaire-finish-button").is_none());
        let restored = Deal::open(&launch, mode, launch.seed());
        assert!(restored.game.won());
        let won_in = before.moves + number_collected;
        assert_eq!(restored.game.moves, won_in);
        assert_eq!(restored.best_moves(), Some(won_in));
        assert_eq!(restored.saved.finish_start, Some(before.moves));
        let results: Vec<_> = storage::recent_results(usize::MAX)
            .into_iter()
            .filter(|r| r.game == launch.game)
            .collect();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].metric, ResultMetric::Moves(won_in as u64));

        // Reopening the saved win still gives Undo the same group boundary.
        window
            .update(cx, |view, _, _| view.deal = restored)
            .unwrap();
        let undo = visual.debug_bounds("solitaire-undo-button").unwrap();
        visual.simulate_click(undo.center(), Modifiers::none());
        draw(&mut visual);
        window
            .read_with(cx, |view, _| assert_eq!(view.deal.game, before))
            .unwrap();
        assert_eq!(Deal::open(&launch, mode, launch.seed()).game, before);
        assert!(visual.debug_bounds("solitaire-finish-button").is_some());
    }

    #[cfg(debug_assertions)]
    fn verified_prefix(
        test: &str,
        ready: impl Fn(&Game, &[Game]) -> bool,
    ) -> (GameLaunch, Vec<Move>) {
        (b'a'..=b'z')
            .find_map(|seed| {
                let launch = GameLaunch {
                    game: GameId(format!("solitaire-{test}-test-{}", std::process::id())),
                    entry: GameEntry::Challenge(char::from(seed).to_string()),
                };
                let mut board = Game::with_mode(DealMode::KlondikeDrawOneVerified, launch.seed());
                let mut turns = Vec::new();
                let mut history = Vec::new();
                for turn in Game::verified_klondike_solution(launch.seed()) {
                    if ready(&board, &history[history.len().saturating_sub(8)..]) {
                        return Some((launch, turns));
                    }
                    history.push(board.clone());
                    assert!(board.play(turn));
                    turns.push(turn);
                }
                None
            })
            .expect("an eligible position in the verified deals")
    }

    #[cfg(debug_assertions)]
    fn draw(visual: &mut VisualTestContext) {
        visual.update(|window, cx| window.draw(cx).clear(cx));
        visual.run_until_parked();
    }

    #[test]
    fn all_styles_restore_the_board_and_its_undo_prefix() {
        let seed = u64::MAX;
        for variant in [
            Variant::Klondike,
            Variant::Spider,
            Variant::FreeCell,
            Variant::Pyramid,
            Variant::TriPeaks,
        ] {
            for &mode in variant.deal_modes() {
                let turns = if variant == Variant::FreeCell {
                    [6, 5].map(|index| Move::Transfer {
                        from: Place::Tableau(0, index),
                        to: Place::Cell(6 - index),
                    })
                } else {
                    [Move::Draw; 2]
                };
                let mut game = Game::with_mode(mode, seed);
                assert!(game.play(turns[0]));
                let after_one = game.clone();
                assert!(game.play(turns[1]));

                let mut saved = SavedDeal::new(seed);
                saved.turns.extend(turns.map(Turn));
                let data = serde_json::to_vec(&saved).unwrap();
                let mut restored: SavedDeal = serde_json::from_slice(&data).unwrap();
                assert_eq!(restored.replay(mode).unwrap(), game);
                restored.turns.pop();
                assert_eq!(restored.replay(mode).unwrap(), after_one);
                restored.turns.pop();
                assert_eq!(restored.replay(mode).unwrap(), Game::with_mode(mode, seed));
            }
        }
    }

    #[test]
    fn corrupt_and_unsupported_replays_are_rejected() {
        let mode = DealMode::TriPeaks;
        let mut saved = SavedDeal::new(18);
        saved.version -= 1;
        assert!(saved.replay(mode).is_none());
        saved.version = REPLAY_VERSION;
        saved.turns.push(Turn(Move::Draw));
        saved
            .turns
            .push(Turn(Move::Clear(Place::Layout(usize::MAX))));
        assert!(saved.replay(mode).is_none());
        assert!(
            serde_json::from_str::<SavedDeal>(r#"{"version":1,"seed":18,"turns":["unknown"]}"#)
                .is_err()
        );
        assert!(
            serde_json::from_str::<SavedDeal>(r#"{"version":1,"seed":18,"turns":[]}"#)
                .unwrap()
                .best_moves
                .is_none()
        );
    }
}
