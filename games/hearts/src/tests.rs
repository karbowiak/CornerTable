use super::*;

fn card(suit: Suit, rank: u8) -> Card {
    Card(suit as u8 * 13 + rank - 2)
}

fn played(seat: Seat, suit: Suit, rank: u8) -> PlayedCard {
    PlayedCard {
        seat,
        card: card(suit, rank),
    }
}

#[test]
fn passing_is_simultaneous_and_rotates_through_keep() {
    let mut game = Game::new(621);
    let directions = [
        Pass::Left,
        Pass::Right,
        Pass::Across,
        Pass::Keep,
        Pass::Left,
    ];
    for direction in directions {
        assert_eq!(game.pass_direction(), direction);
        let before = game.clone();
        let sent = [before.hands[0][0], before.hands[0][1], before.hands[0][2]];
        if direction == Pass::Keep {
            assert!(!game.apply(Action::Pass(sent)));
            assert_eq!(game, before);
        } else {
            assert!(!game.apply(Action::Pass([sent[0]; 3])));
            assert_eq!(game, before);
            assert!(game.apply(Action::Pass(sent)));
            let to = direction.recipient(Seat::You);
            let from = direction.sender(Seat::You);
            assert!(sent.iter().all(|card| game.hand(to).contains(card)));
            let received = game.received().unwrap();
            assert!(received.iter().all(|card| before.hand(from).contains(card)));
            assert!(
                received
                    .iter()
                    .all(|card| game.hand(Seat::You).contains(card))
            );
            assert_eq!(game.hands.each_ref().map(Vec::len), [13; 4]);
        }
        let Phase::Playing(opener) = game.phase() else {
            panic!("passing must yield the lead");
        };
        assert!(game.hand(opener).contains(&Card::TWO_OF_CLUBS));
        assert_eq!(game.legal_plays(), [Card::TWO_OF_CLUBS]);
        finish_hand(&mut game);
        assert!(game.apply(Action::Deal));
    }
}

#[test]
fn first_trick_requires_clubs_and_blocks_points_if_a_clean_discard_exists() {
    let mut game = Game::new(0);
    game.phase = Phase::Playing(Seat::You);
    game.trick.push(played(Seat::East, Suit::Clubs, 2));
    let club = card(Suit::Clubs, 13);
    let diamond = card(Suit::Diamonds, 2);
    let queen = Card::QUEEN_OF_SPADES;
    let heart = card(Suit::Hearts, 3);
    game.hands[0] = vec![club, diamond, queen, heart];
    assert_eq!(game.legal_plays(), [club]);
    game.hands[0].remove(0);
    assert_eq!(game.legal_plays(), [diamond]);
    let before = game.clone();
    assert!(!game.apply(Action::Play(queen)));
    assert_eq!(game, before);
    game.hands[0].remove(0);
    assert_eq!(game.legal_plays(), [queen, heart]);
    assert!(game.apply(Action::Play(heart)));
    assert!(game.hearts_broken());
}

#[test]
fn only_a_heart_breaks_hearts_and_an_all_hearts_hand_can_lead() {
    let mut game = Game::new(0);
    game.phase = Phase::Playing(Seat::You);
    game.taken = vec![
        played(Seat::You, Suit::Clubs, 2),
        played(Seat::West, Suit::Clubs, 3),
        played(Seat::North, Suit::Clubs, 4),
        played(Seat::East, Suit::Clubs, 5),
    ];
    let heart = card(Suit::Hearts, 2);
    game.hands[0] = vec![Card::QUEEN_OF_SPADES, heart];
    assert_eq!(game.legal_plays(), [Card::QUEEN_OF_SPADES]);
    assert!(!game.apply(Action::Play(heart)));
    assert!(game.apply(Action::Play(Card::QUEEN_OF_SPADES)));
    assert!(!game.hearts_broken());

    game.phase = Phase::Playing(Seat::You);
    game.trick.clear();
    assert_eq!(game.legal_plays(), [heart]);
    assert!(game.apply(Action::Play(heart)));
    assert!(game.hearts_broken());
    game.phase = Phase::Playing(Seat::You);
    game.trick.clear();
    game.hands[0] = vec![card(Suit::Clubs, 7), card(Suit::Hearts, 8)];
    assert_eq!(game.legal_plays(), game.hands[0]);
}

#[test]
fn highest_led_suit_takes_the_points_and_the_next_lead() {
    let mut game = Game::new(0);
    game.taken = vec![
        played(Seat::You, Suit::Clubs, 2),
        played(Seat::West, Suit::Clubs, 3),
        played(Seat::North, Suit::Clubs, 4),
        played(Seat::East, Suit::Clubs, 5),
    ];
    game.phase = Phase::Playing(Seat::West);
    let plays = [
        played(Seat::West, Suit::Diamonds, 12),
        played(Seat::North, Suit::Hearts, 14),
        played(Seat::East, Suit::Spades, 12),
        played(Seat::You, Suit::Diamonds, 13),
    ];
    for play in plays {
        game.hands[play.seat.index()] = vec![play.card];
        assert!(game.apply(Action::Play(play.card)));
    }
    let result = TrickResult {
        winner: Seat::You,
        points: 14,
    };
    assert_eq!(game.phase(), Phase::TrickComplete(result));
    assert_eq!(game.trick(), plays);
    assert!(game.apply(Action::Collect));
    assert_eq!(game.phase(), Phase::Playing(Seat::You));
    assert_eq!(game.last_trick(), Some(result));
    assert_eq!(game.hand_points(), [14, 0, 0, 0]);
    assert!(!game.apply(Action::Collect));
}

#[test]
fn scoring_accounts_for_the_moon_match_limit_and_tied_low_score() {
    let mut scores = [10, 80, 2, 3];
    let moon = score_hand([0, 0, 0, 26], &mut scores);
    assert_eq!(moon.points, [26, 26, 26, 0]);
    assert_eq!(moon.moon, Some(Seat::East));
    assert_eq!(scores, [36, 106, 28, 3]);
    assert_eq!(moon.winner, Some(Seat::East));

    let mut scores = [94, 12, 11, 30];
    let tie = score_hand([13, 0, 1, 12], &mut scores);
    assert!(tie.tied_at_limit);
    assert_eq!(tie.winner, None);
    assert_eq!(tie.points, [13, 0, 1, 12]);
    assert_eq!(scores, [107, 12, 12, 42]);
    let untied = score_hand([0, 1, 0, 25], &mut scores);
    assert!(!untied.tied_at_limit);
    assert_eq!(untied.winner, Some(Seat::North));
}

#[test]
fn deterministic_computer_games_reach_a_terminal_match() {
    for seed in 0..100 {
        let mut game = Game::new(seed);
        assert_eq!(game, Game::new(seed));
        loop {
            let result = finish_hand(&mut game);
            assert_eq!(game.hand_points().iter().sum::<u8>(), 26);
            assert_eq!(game.hands.each_ref().map(Vec::len), [0; 4]);
            let mut cards: Vec<_> = game.taken.iter().map(|play| play.card).collect();
            cards.sort_unstable();
            assert_eq!(cards, (0..52).map(Card).collect::<Vec<_>>());
            if let Some(winner) = result.winner {
                assert!(game.scores().iter().any(|&score| score >= MATCH_LIMIT));
                assert_eq!(
                    Some(&game.scores()[winner.index()]),
                    game.scores().iter().min()
                );
                assert!(!game.apply(Action::Deal));
                break;
            }
            assert!(game.hand_number() < 100);
            assert!(game.apply(Action::Deal));
        }
    }
}

#[test]
fn opponent_levels_choose_legal_and_distinct_plays() {
    let mut game = Game::new(42);
    let mut easy_differs = false;
    let mut hard_differs = false;
    loop {
        let action = match game.phase() {
            Phase::Passing => Action::Pass(computer::pass(game.hand(Seat::You))),
            Phase::Playing(_) => {
                let legal = game.legal_plays();
                let easy = game.suggested_play_at(Difficulty::Easy).unwrap();
                let medium = game.suggested_play().unwrap();
                let hard = game.suggested_play_at(Difficulty::Hard).unwrap();
                assert!(legal.contains(&easy) && legal.contains(&hard));
                easy_differs |= easy != medium;
                hard_differs |= hard != medium;
                Action::Play(medium)
            }
            Phase::TrickComplete(_) => Action::Collect,
            Phase::HandComplete(_) => break,
        };
        assert!(game.apply(action));
    }
    assert!(easy_differs && hard_differs);
}

fn finish_hand(game: &mut Game) -> HandResult {
    loop {
        let action = match game.phase() {
            Phase::Passing => Action::Pass(computer::pass(game.hand(Seat::You))),
            Phase::Playing(_) => Action::Play(game.suggested_play().unwrap()),
            Phase::TrickComplete(_) => Action::Collect,
            Phase::HandComplete(result) => return result,
        };
        assert!(game.apply(action));
    }
}
