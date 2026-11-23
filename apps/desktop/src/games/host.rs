use gpui::{
    AnyView, Context, FocusHandle, IntoElement, Render, WeakFocusHandle, Window, div, prelude::*,
};

pub struct GameHost {
    view: AnyView,
    scope: FocusHandle,
    last_focus: Option<WeakFocusHandle>,
    restore_from: Option<FocusHandle>,
}

impl GameHost {
    pub fn new(view: AnyView, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let scope = cx.focus_handle();
        cx.on_focus_out(&scope, window, |game, event, _, _| {
            game.last_focus = Some(event.blurred);
        })
        .detach();
        Self {
            view,
            scope,
            last_focus: None,
            restore_from: None,
        }
    }

    pub fn restore_focus(&mut self, current: &FocusHandle, cx: &mut Context<Self>) {
        self.restore_from = Some(current.clone());
        cx.notify();
    }
}

impl Render for GameHost {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(current) = self.restore_from.take() {
            cx.on_next_frame(window, move |game, window, cx| {
                if current.is_focused(window)
                    && let Some(focus) = game.last_focus.as_ref().and_then(WeakFocusHandle::upgrade)
                    && game.scope.contains(&focus, window)
                {
                    focus.focus(window, cx);
                }
            });
        }
        div()
            .w_full()
            .track_focus(&self.scope)
            .child(self.view.clone())
    }
}

#[cfg(test)]
mod tests {
    use gpui::{Entity, TestAppContext};

    use super::*;

    struct Board(FocusHandle);

    impl Render for Board {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().id("test-board").track_focus(&self.0)
        }
    }

    struct Page {
        focus: FocusHandle,
        board_focus: FocusHandle,
        game: Entity<GameHost>,
        playing: bool,
    }

    impl Render for Page {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .id("test-page")
                .track_focus(&self.focus)
                .children(self.playing.then(|| self.game.clone()))
        }
    }

    #[gpui::test]
    fn returning_to_a_game_reuses_its_board_focus(cx: &mut TestAppContext) {
        let app = cx.add_window(|window, cx| {
            let board = cx.new(|cx| Board(cx.focus_handle().tab_stop(true)));
            let board_focus = board.read(cx).0.clone();
            let game = cx.new(|cx| GameHost::new(board.into(), window, cx));
            Page {
                focus: cx.focus_handle(),
                board_focus,
                game,
                playing: true,
            }
        });
        app.update(cx, |_, window, _| window.activate_window())
            .unwrap();
        cx.run_until_parked();
        let draw = |cx: &mut TestAppContext| {
            cx.update_window(app.into(), |_, window, cx| window.draw(cx).clear(cx))
                .unwrap();
        };
        draw(cx);

        app.update(cx, |page, window, cx| {
            page.board_focus.focus(window, cx);
        })
        .unwrap();
        draw(cx);
        app.update(cx, |page, window, cx| {
            page.focus.focus(window, cx);
            page.playing = false;
            cx.notify();
        })
        .unwrap();
        draw(cx);

        app.update(cx, |page, _, cx| {
            page.playing = true;
            page.game
                .update(cx, |game, cx| game.restore_focus(&page.focus, cx));
            cx.notify();
        })
        .unwrap();
        draw(cx);
        app.update(cx, |page, window, cx| {
            window.simulate_next_frame(cx);
            assert!(page.board_focus.is_focused(window));
        })
        .unwrap();
    }
}
