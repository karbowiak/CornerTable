use std::collections::HashSet;

use super::*;

fn tile(a: u8, b: u8) -> Tile {
    Tile::new(a, b).unwrap()
}

fn position(ends: [u8; 2], hands: [&[Tile]; 2], next_draws: &[Tile]) -> Round {
    let mut round = Round::new(0);
    let lead = tile(ends[0], ends[1]);
    round.hands = hands.map(<[Tile]>::to_vec);
    round.boneyard = next_draws.iter().rev().copied().collect();
    round.chain = VecDeque::from([Placed {
        tile: lead,
        faces: ends,
    }]);
    round.player = Player::One;
    round.last_placement = lead;
    round.last_event = Event::Opened {
        player: Player::Two,
        tile: lead,
    };
    round
}

#[test]
fn deals_a_unique_full_set_and_opens_with_the_highest_dealt_double() {
    for seed in 0..100 {
        let round = Round::new(seed);
        assert_eq!(round, Round::new(seed));
        let Event::Opened { player, tile: lead } = round.last_event() else {
            panic!("fresh deal did not open");
        };
        assert_eq!(round.hand(player).len(), 6);
        assert_eq!(round.hand(player.opponent()).len(), 7);
        assert_eq!(round.boneyard_count(), 14);
        assert_eq!(round.player(), player.opponent());
        assert_eq!(round.chain()[0].tile, lead);

        let dealt: Vec<_> = round
            .hands
            .iter()
            .flatten()
            .copied()
            .chain([lead])
            .collect();
        assert_eq!(
            dealt
                .iter()
                .copied()
                .max_by_key(|tile| (tile.is_double(), tile.total(), tile.high)),
            Some(lead),
        );
        let set: HashSet<_> = dealt
            .into_iter()
            .chain(round.boneyard.iter().copied())
            .collect();
        assert_eq!(set.len(), TILE_COUNT);
    }
    assert_ne!(Round::new(0), Round::new(1));
}

#[test]
fn opening_without_doubles_uses_larger_pip_to_break_a_total_tie() {
    let mut dealt = vec![tile(1, 6), tile(2, 5)];
    for high in 1..=6 {
        for low in 0..high {
            if low + high < 7 {
                dealt.push(tile(low, high));
            }
        }
    }
    let mut deck = vec![];
    for high in 0..=6 {
        for low in 0..=high {
            if !dealt[..14].contains(&tile(low, high)) {
                deck.push(tile(low, high));
            }
        }
    }
    deck.extend(dealt[..14].iter().rev().copied());
    let round = Round::deal(deck);
    assert_eq!(
        round.last_event(),
        Event::Opened {
            player: Player::One,
            tile: tile(1, 6)
        }
    );
    assert_eq!(round.player(), Player::Two);
}

#[test]
fn the_same_tile_can_fit_both_ends_with_different_exposed_results() {
    let choice = tile(2, 5);
    for (end, expected, faces) in [(End::Left, [5, 5], [5, 2]), (End::Right, [2, 2], [5, 2])] {
        let mut round = position([2, 5], [&[choice, tile(1, 1)], &[tile(2, 2)]], &[]);
        assert_eq!(round.fits(choice), [true, true]);
        assert_eq!(round.preview(choice, end).unwrap().faces, faces);
        assert!(round.act(Action::Play(choice, end)).is_some());
        assert_eq!(round.ends(), expected);
        assert_eq!(round.player(), Player::Two);
    }
}

#[test]
fn no_drawing_passing_or_playing_the_wrong_tile_or_end_when_a_match_exists() {
    let make = || position([2, 5], [&[tile(1, 2)], &[tile(5, 6)]], &[tile(4, 4)]);
    let mut round = make();
    assert_eq!(round.act(Action::Draw), None);
    assert_eq!(round.act(Action::Pass), None);
    assert_eq!(round.act(Action::Play(tile(1, 2), End::Right)), None);
    assert_eq!(round.act(Action::Play(tile(5, 6), End::Right)), None);
    assert_eq!(round, make());
}

#[test]
fn draws_through_non_matches_then_keeps_the_turn_and_stops_at_first_fit() {
    let mut round = position(
        [2, 5],
        [&[tile(3, 4)], &[tile(5, 5)]],
        &[tile(0, 0), tile(4, 4), tile(0, 5), tile(1, 2)],
    );
    assert_eq!(round.phase(), Phase::Draw);
    assert_eq!(round.act(Action::Pass), None);
    assert_eq!(
        round.act(Action::Draw),
        Some(Event::Drew {
            player: Player::One,
            count: 3
        })
    );
    assert_eq!(round.player(), Player::One);
    assert_eq!(round.boneyard_count(), 1);
    assert_eq!(
        round.hand(Player::One),
        [tile(3, 4), tile(0, 0), tile(4, 4), tile(0, 5)]
    );
    assert_eq!(round.phase(), Phase::Play);
    assert!(round.act(Action::Play(tile(0, 5), End::Right)).is_some());
    assert_eq!(round.ends(), [2, 0]);
}

#[test]
fn passes_after_exhausting_the_boneyard_and_scores_a_block_by_difference() {
    let mut round = position([2, 5], [&[tile(1, 3)], &[tile(4, 6)]], &[tile(0, 4)]);
    assert_eq!(round.phase(), Phase::Draw);
    round.act(Action::Draw).unwrap();
    assert_eq!(round.phase(), Phase::Pass);
    assert_eq!(
        round.act(Action::Pass),
        Some(Event::Passed {
            player: Player::One
        })
    );
    assert!(round.outcome().is_none());
    round.act(Action::Pass).unwrap();
    assert_eq!(
        round.outcome(),
        Some(Outcome {
            finish: Finish::Blocked,
            winner: Some(Player::One),
            pips: [8, 10],
            points: 2,
        })
    );
    assert_eq!(round.phase(), Phase::Finished);
}

#[test]
fn a_play_breaks_the_pass_streak_and_a_tied_block_awards_nothing() {
    let mut round = position(
        [2, 5],
        [&[tile(4, 4)], &[tile(5, 6), tile(1, 4), tile(0, 3)]],
        &[],
    );
    round.act(Action::Pass).unwrap();
    round.act(Action::Play(tile(5, 6), End::Right)).unwrap();
    round.act(Action::Pass).unwrap();
    assert!(round.outcome().is_none());
    round.act(Action::Pass).unwrap();
    assert_eq!(
        round.outcome(),
        Some(Outcome {
            finish: Finish::Blocked,
            winner: None,
            pips: [8, 8],
            points: 0,
        })
    );
}

#[test]
fn going_out_wins_even_when_the_other_player_has_only_a_blank() {
    for (remaining, points) in [(tile(0, 0), 0), (tile(4, 4), 8)] {
        let mut round = position([2, 5], [&[tile(2, 3)], &[remaining]], &[]);
        round.act(Action::Play(tile(2, 3), End::Left)).unwrap();
        assert_eq!(
            round.outcome(),
            Some(Outcome {
                finish: Finish::WentOut,
                winner: Some(Player::One),
                pips: [0, points],
                points,
            })
        );
    }
}

#[test]
fn computer_finishes_many_deals_with_an_intact_chain_and_tile_set() {
    for seed in 0..512 {
        let mut round = Round::new(seed);
        for _ in 0..70 {
            let Some(action) = computer_action(&round) else {
                break;
            };
            round.act(action).expect("computer must act legally");
            assert!(
                round
                    .chain
                    .iter()
                    .zip(round.chain.iter().skip(1))
                    .all(|(a, b)| a.faces[1] == b.faces[0])
            );
            let all_tiles: HashSet<_> = round
                .chain
                .iter()
                .map(|p| p.tile)
                .chain(round.hands.iter().flatten().copied())
                .chain(round.boneyard.iter().copied())
                .collect();
            assert_eq!(all_tiles.len(), TILE_COUNT);
        }
        assert!(round.outcome().is_some(), "seed {seed} did not end");
        assert_eq!(computer_action(&round), None);
    }
}
