use super::*;

#[test]
fn all_layout_symmetries_are_solvable_without_guessing() {
    for room in Room::ALL {
        for variant in 0..16 {
            let mut run = Expedition::new(0);
            run.current = room;
            run.rooms[room as usize] = RoomBoard::new(room, variant);
            assert_eq!(run.diamonds(room).0, 0, "{room:?} variant {variant}");
            solve(&mut run, room == Room::DeepVault);
        }
    }
}

#[test]
fn either_drift_can_open_the_floodgate_and_the_other_remains_optional() {
    for (chosen, detour) in [
        (Room::AmberDrift, Room::SlateDrift),
        (Room::SlateDrift, Room::AmberDrift),
    ] {
        let mut run = Expedition::new(42);
        assert_eq!(run, Expedition::new(42));
        assert_ne!(run, Expedition::new(43));
        assert!(!run.travel(chosen));
        solve(&mut run, false);
        assert!(run.travel(chosen));
        assert!(run.can_travel(detour));
        assert!(!run.travel(Room::Floodgate));
        solve(&mut run, false);
        assert!(run.travel(Room::Floodgate));
        assert!(!run.travel(Room::DeepVault));
        solve(&mut run, false);
        assert!(run.travel(Room::DeepVault));
        solve(&mut run, true);
        assert!(run.is_complete());
        assert_eq!(run.surveyed_count(), 4);

        assert!(run.travel(detour));
        solve(&mut run, false);
        assert!(run.travel(Room::DeepVault));
        assert_eq!(run.surveyed_count(), 5);
        assert_eq!(run.cave_ins(), 0);
    }
}

#[test]
fn cave_in_shows_mines_and_wrong_flags_and_retry_keeps_other_chambers() {
    let mut run = Expedition::new(42);
    solve(&mut run, false);
    assert!(run.travel(Room::AmberDrift));
    let starting_room = run.board().clone();
    let mine = (0..MAX_CELLS)
        .map(|cell| starting_room.position(cell))
        .find(|&position| {
            run.can_dig(position)
                && starting_room.features[starting_room.index(position).unwrap()] == b'M'
        })
        .expect("amber frontier has a mine");
    let wrong_flag = starting_room
        .features
        .iter()
        .zip(starting_room.cells)
        .position(|(&feature, cell)| feature == b'.' && cell == Cell::Covered)
        .map(|cell| starting_room.position(cell))
        .expect("covered bare slate");
    assert!(run.toggle_flag(wrong_flag));
    assert!(run.toggle_flag(mine));
    assert!(!run.dig(mine));
    assert!(run.toggle_flag(mine));
    assert!(run.dig(mine));
    assert_eq!(run.status(Room::AmberDrift), RoomStatus::Collapsed);
    assert_eq!(run.tile(mine), Some(Tile::Mine { hit: true }));
    assert_eq!(run.tile(wrong_flag), Some(Tile::FalseFlag));
    assert_eq!(
        (0..run.side() * run.side())
            .filter(|&cell| matches!(
                run.tile(starting_room.position(cell)),
                Some(Tile::Mine { .. })
            ))
            .count(),
        run.mine_count()
    );
    assert!(!run.dig(mine));
    assert_eq!(run.cave_ins(), 1);
    assert!(run.retry_room());
    assert!(!run.retry_room());
    assert_eq!(*run.board(), starting_room);
    assert_eq!(run.cave_ins(), 1);
    assert_eq!(run.status(Room::Entrance), RoomStatus::Surveyed);
}

#[test]
fn bedrock_and_corners_cannot_be_dug_from_the_entry() {
    let mut run = Expedition::new(0);
    run.current = Room::SlateDrift;
    let mut board = RoomBoard::new(Room::SlateDrift, 0);
    board.cells.fill(Cell::Covered);
    board.cells[board.index((1, 1)).unwrap()] = Cell::Open;
    run.rooms[Room::SlateDrift as usize] = board;
    assert!(run.can_dig((2, 1)));
    assert!(!run.can_dig((0, 2)));
    assert!(!run.dig((2, 2)));
    assert!(!run.toggle_flag((2, 2)));
    assert_eq!(run.tile((2, 2)), Some(Tile::Bedrock));
    assert_eq!(run.tile((usize::MAX, usize::MAX)), None);
}

#[test]
fn sweep_uses_flags_and_can_set_off_a_mine_if_they_are_wrong() {
    let mut run = Expedition::new(0);
    let mut board = RoomBoard::new(Room::Entrance, 0);
    board.cells.fill(Cell::Covered);
    board.cells[1] = Cell::Open;
    run.rooms[Room::Entrance as usize] = board;

    assert!(!run.sweep((1, 0)));
    assert!(run.toggle_flag((0, 0)));
    assert!(run.can_sweep((1, 0)));
    let mut wrong = run.clone();
    assert!(run.sweep((1, 0)));
    assert!(matches!(run.tile((2, 0)), Some(Tile::Open { .. })));
    assert_eq!(run.status(Room::Entrance), RoomStatus::Exploring);
    assert!(wrong.toggle_flag((0, 0)));
    assert!(wrong.toggle_flag((2, 0)));
    assert!(wrong.sweep((1, 0)));
    assert_eq!(wrong.tile((0, 0)), Some(Tile::Mine { hit: true }));
    assert_eq!(wrong.cave_ins(), 1);
}

/// Read the same tile and flag information the player sees. The Vault may
/// require comparing two clues whose unknown cells form a subset.
fn solve(run: &mut Expedition, compare_clues: bool) {
    use Tile::*;
    let room = run.current_room();
    while run.status(room) == RoomStatus::Exploring {
        let side = run.side();
        let positions: Vec<_> = (0..side)
            .flat_map(|y| (0..side).map(move |x| (x, y)))
            .collect();
        let mut clues = Vec::new();
        let mut safe = 0;
        let mut mines = 0;
        for (i, &position) in positions.iter().enumerate() {
            match run.tile(position).unwrap() {
                Covered { diamond: true } => safe |= 1_u64 << i,
                Open { clue, .. } => {
                    let unknowns = run
                        .board()
                        .neighbors(i, true)
                        .filter(|&n| {
                            matches!(run.tile(positions[n]), Some(Covered { diamond: false }))
                        })
                        .fold(0_u64, |mask, n| mask | 1_u64 << n);
                    let remaining = u32::from(clue) - run.adjacent_flags(position) as u32;
                    clues.push((unknowns, remaining));
                }
                _ => (),
            }
        }
        let mut deduce = |unknowns: u64, remaining| {
            if remaining == 0 {
                safe |= unknowns;
            }
            if unknowns.count_ones() == remaining {
                mines |= unknowns;
            }
        };
        for &(unknowns, remaining) in &clues {
            deduce(unknowns, remaining);
            if compare_clues {
                for &(other, other_count) in &clues {
                    if unknowns != other && unknowns & other == unknowns {
                        deduce(other & !unknowns, other_count - remaining);
                    }
                }
            }
        }
        assert_eq!(safe & mines, 0);
        let mut changed = false;
        for (i, position) in positions.into_iter().enumerate() {
            if mines & (1_u64 << i) != 0 {
                changed |= run.toggle_flag(position);
            } else if safe & (1_u64 << i) != 0 {
                changed |= run.dig(position);
            }
        }
        assert!(changed, "{room:?} has no clue-based move at {run:?}");
    }
    assert_eq!(run.status(room), RoomStatus::Surveyed, "{room:?}");
}
