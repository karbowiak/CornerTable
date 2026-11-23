use super::*;
use std::sync::atomic::AtomicBool;

fn play(game: &mut Game, text: &str) -> Played {
    game.play(text.parse().unwrap())
        .unwrap_or_else(|| panic!("illegal move {text}"))
        .clone()
}

fn fixture(fen: &str) -> Game {
    let board: Board = fen.parse().unwrap();
    let halfmoves = board.halfmove_clock();
    let mut game = Game {
        position: Position { board, halfmoves },
        ..Game::default()
    };
    game.ending = game.automatic_ending();
    game
}

#[test]
fn opening_has_twenty_moves_and_rejects_wrong_turns() {
    let mut game = Game::default();
    assert_eq!(game.legal_moves().len(), 20);
    assert!(game.play("e7e5".parse().unwrap()).is_none());
    assert!(game.play("e2e5".parse().unwrap()).is_none());
    assert_eq!(play(&mut game, "e2e4").san, "e4");
    assert_eq!(game.side_to_move(), Color::Black);
}

#[test]
fn check_blocks_unrelated_moves_and_mate_finishes_the_game() {
    let mut game = Game::default();
    for movement in ["f2f3", "e7e5", "g2g4"] {
        play(&mut game, movement);
    }
    assert_eq!(play(&mut game, "d8h4").san, "Qh4#");
    assert_eq!(game.checked_king(), Some(Square::E1));
    assert_eq!(
        game.ending(),
        Some(Ending::Checkmate {
            winner: Color::Black
        })
    );
    assert!(game.legal_moves().is_empty());
    assert!(game.undo(None));
    assert!(game.ending().is_none());

    let mut checked = fixture("1k2r3/8/8/8/8/8/8/R3K3 w - - 0 1");
    assert_eq!(checked.checked_king(), Some(Square::E1));
    assert!(checked.play("a1a8".parse().unwrap()).is_none());
    assert!(checked.play("e1d1".parse().unwrap()).is_some());
}

#[test]
fn castling_uses_the_kings_landing_square_and_moves_both_pieces() {
    let mut game = fixture("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1");
    assert!(game.play("e1h1".parse().unwrap()).is_none());
    let white = play(&mut game, "e1g1");
    assert_eq!(white.san, "O-O");
    assert_eq!(white.captured, None);
    assert_eq!(game.piece(Square::G1), Some((Color::White, Piece::King)));
    assert_eq!(game.piece(Square::F1), Some((Color::White, Piece::Rook)));
    assert_eq!(game.piece(Square::H1), None);
    assert_eq!(play(&mut game, "e8c8").san, "O-O-O");
    assert_eq!(game.piece(Square::C8), Some((Color::Black, Piece::King)));
    assert_eq!(game.piece(Square::D8), Some((Color::Black, Piece::Rook)));
    assert!(game.undo(None));
    assert_eq!(game.piece(Square::A8), Some((Color::Black, Piece::Rook)));
}

#[test]
fn castling_is_illegal_in_check_through_check_and_after_the_rook_moves() {
    let mut game = fixture("r3k2r/8/8/8/2b5/8/8/R3K2R w KQkq - 0 1");
    assert!(game.play("e1g1".parse().unwrap()).is_none());
    assert!(game.legal_moves().contains(&"e1c1".parse().unwrap()));

    let mut game = fixture("r3k2r/8/8/8/8/8/4q3/R3K2R w KQkq - 0 1");
    assert!(game.play("e1g1".parse().unwrap()).is_none());
    assert!(game.play("e1c1".parse().unwrap()).is_none());

    let mut game = fixture("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1");
    for movement in ["h1h2", "h8h7", "h2h1", "h7h8"] {
        play(&mut game, movement);
    }
    assert!(game.play("e1g1".parse().unwrap()).is_none());
}

#[test]
fn en_passant_takes_the_adjacent_pawn_and_expires_after_one_turn() {
    let mut game = Game::default();
    for movement in ["e2e4", "a7a6", "e4e5", "d7d5"] {
        play(&mut game, movement);
    }
    let capture = play(&mut game, "e5d6");
    assert_eq!(capture.san, "exd6");
    assert_eq!(capture.captured, Some(Piece::Pawn));
    assert_eq!(game.piece(Square::D5), None);
    assert_eq!(game.piece(Square::D6), Some((Color::White, Piece::Pawn)));
    assert_eq!(game.position.halfmoves, 0);
    assert!(game.undo(None));
    play(&mut game, "g1f3");
    play(&mut game, "g8f6");
    assert!(game.play("e5d6".parse().unwrap()).is_none());
}

#[test]
fn en_passant_cannot_expose_the_king_and_does_not_change_repetition_if_illegal() {
    let fen = "k3r3/8/8/3pP3/8/8/8/4K3 w - ";
    let mut game = fixture(&format!("{fen}d6 0 1"));
    let without = fixture(&format!("{fen}- 0 1"));
    assert!(game.position.board.same_position(&without.position.board));
    assert!(game.play("e5d6".parse().unwrap()).is_none());
}

#[test]
fn all_four_promotions_are_legal_and_a_choice_is_required() {
    let game = fixture("7k/P7/8/8/8/8/8/K7 w - - 0 1");
    assert_eq!(
        game.legal_moves()
            .iter()
            .filter(|m| m.from == Square::A7)
            .count(),
        4
    );
    assert!(game.clone().play("a7a8".parse().unwrap()).is_none());
    for (suffix, piece) in [
        ('q', Piece::Queen),
        ('r', Piece::Rook),
        ('b', Piece::Bishop),
        ('n', Piece::Knight),
    ] {
        let mut game = game.clone();
        let played = play(&mut game, &format!("a7a8{suffix}"));
        assert!(played.san.starts_with("a8="));
        assert_eq!(game.piece(Square::A8), Some((Color::White, piece)));
    }
}

#[test]
fn stalemate_and_insufficient_material_are_automatic() {
    let mut game = fixture("7k/5K2/8/6Q1/8/8/8/8 w - - 0 1");
    play(&mut game, "g5g6");
    assert_eq!(game.ending(), Some(Ending::Stalemate));
    assert_eq!(game.checked_king(), None);

    for fen in [
        "7k/8/8/8/8/8/8/K7 w - - 0 1",
        "7k/8/8/8/8/8/8/KN6 w - - 0 1",
        "5b1k/8/8/8/8/8/8/K1B5 w - - 0 1",
    ] {
        assert_eq!(fixture(fen).ending(), Some(Ending::InsufficientMaterial));
    }
    assert_eq!(fixture("7k/8/8/8/8/NN6/8/K7 w - - 0 1").ending(), None);
    assert_eq!(fixture("4b2k/8/8/8/8/8/8/K1B5 w - - 0 1").ending(), None);
}

#[test]
fn repetition_can_be_claimed_with_an_announced_move_or_after_three_positions() {
    let cycle = ["g1f3", "g8f6", "f3g1", "f6g8"];
    let mut game = Game::default();
    for movement in cycle.into_iter().cycle().take(7) {
        play(&mut game, movement);
    }
    assert_eq!(game.claim(None), None);
    let movement = "f6g8".parse().unwrap();
    let intended = Some(movement);
    assert_eq!(game.claim(intended), Some(DrawClaim::ThreefoldRepetition));
    assert_eq!(game.claim_moves(), vec![movement]);
    assert!(game.decide(Decision::ClaimDraw { intended }));
    assert_eq!(game.moves().len(), 7);
    assert_eq!(game.piece(Square::F6), Some((Color::Black, Piece::Knight)));
    assert_eq!(
        game.ending(),
        Some(Ending::ClaimedDraw(DrawClaim::ThreefoldRepetition))
    );
    assert!(game.undo(None));
    assert_eq!(game.moves().len(), 7);
    play(&mut game, "f6g8");
    assert_eq!(game.claim(None), Some(DrawClaim::ThreefoldRepetition));
    assert_eq!(game.ending(), None);
    for movement in cycle.into_iter().cycle().take(8) {
        play(&mut game, movement);
    }
    assert_eq!(game.ending(), Some(Ending::FivefoldRepetition));
    assert!(game.undo(None));
    assert_eq!(game.ending(), None);
}

#[test]
fn a_repetition_must_preserve_castling_rights() {
    let mut game = fixture("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1");
    let cycle = ["h1h2", "h8h7", "h2h1", "h7h8"];
    for movement in cycle.into_iter().cycle().take(8) {
        play(&mut game, movement);
    }
    assert_eq!(game.claim(None), None);
    for movement in cycle {
        play(&mut game, movement);
    }
    assert_eq!(game.claim(None), Some(DrawClaim::ThreefoldRepetition));
}

#[test]
fn fifty_moves_are_claimable_and_seventy_five_are_automatic() {
    let mut game = fixture("7k/8/8/8/8/8/RP6/K7 w - - 99 1");
    assert_eq!(game.claim(None), None);
    assert_eq!(game.claim(Some("b2b3".parse().unwrap())), None);
    assert_eq!(
        game.claim(Some("a2a3".parse().unwrap())),
        Some(DrawClaim::FiftyMoveRule)
    );
    play(&mut game, "a2a3");
    assert_eq!(game.claim(None), Some(DrawClaim::FiftyMoveRule));
    assert_eq!(game.ending(), None);

    let mut game = fixture("7k/8/8/8/8/8/R7/K7 w - - 100 1");
    game.position.halfmoves = 149;
    play(&mut game, "a2a3");
    assert_eq!(game.ending(), Some(Ending::SeventyFiveMoveRule));
    assert!(game.undo(None));
    assert_eq!(game.claim(None), Some(DrawClaim::FiftyMoveRule));

    let mut mate = fixture("7k/5K2/6Q1/8/8/8/8/8 w - - 100 1");
    mate.position.halfmoves = 149;
    play(&mut mate, "g6g7");
    assert_eq!(
        mate.ending(),
        Some(Ending::Checkmate {
            winner: Color::White
        })
    );
}

#[test]
fn solo_undo_returns_to_the_white_decision_before_the_computer_reply() {
    let mut game = Game::default();
    play(&mut game, "e2e4");
    assert!(game.undo(Some(Color::White)));
    play(&mut game, "e2e4");
    play(&mut game, "e7e5");
    assert!(game.undo(Some(Color::White)));
    assert_eq!(game.moves().len(), 0);
    assert_eq!(game.side_to_move(), Color::White);
}

#[test]
fn agreement_and_resignation_finish_and_are_undoable() {
    let mut game = Game::default();
    assert!(!game.decide(Decision::AgreeDraw));
    play(&mut game, "e2e4");
    play(&mut game, "e7e5");
    assert!(game.decide(Decision::AgreeDraw));
    assert_eq!(game.ending(), Some(Ending::AgreedDraw));
    assert!(game.undo(None));
    assert!(game.decide(Decision::Resign));
    assert_eq!(
        game.ending(),
        Some(Ending::Resignation {
            winner: Color::Black
        })
    );
    assert!(game.undo(None));
    assert_eq!(game.moves().len(), 2);

    let mut game = fixture("7k/8/8/8/8/8/R7/K7 w - - 0 1");
    assert!(game.decide(Decision::Resign));
    assert_eq!(game.ending(), Some(Ending::ResignationDraw));
}

#[test]
fn notation_disambiguates_pieces_and_ignores_a_pinned_alternative() {
    let mut game = fixture("7k/8/8/8/8/8/8/KN3N2 w - - 0 1");
    assert_eq!(play(&mut game, "b1d2").san, "Nbd2");
    let mut game = fixture("1r5k/8/8/8/8/8/1N6/1K3N2 w - - 0 1");
    assert_eq!(play(&mut game, "f1d2").san, "Nd2");
}

#[test]
fn computer_finds_mate_and_cancellable_search_returns_legal_replies() {
    let mut game = fixture("7k/5K2/6Q1/8/8/8/8/8 w - - 0 1");
    let live = AtomicBool::new(false);
    let Some(ComputerTurn::Play(movement)) = choose_turn(&game, &live) else {
        panic!("missing computer move");
    };
    assert!(game.play(movement).is_some());
    assert_eq!(
        game.ending(),
        Some(Ending::Checkmate {
            winner: Color::White
        })
    );
    assert_eq!(choose_turn(&game, &live), None);

    let mut game = Game::default();
    for _ in 0..4 {
        let Some(ComputerTurn::Play(movement)) = choose_turn(&game, &live) else {
            panic!("missing opening move");
        };
        assert!(game.play(movement).is_some());
    }
    assert_eq!(choose_turn(&game, &AtomicBool::new(true)), None);
}
