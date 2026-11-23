use super::*;

fn hole(row: u8, col: u8) -> Hole {
    Hole::new(row, col).unwrap()
}

#[test]
fn peg_solitaire_opening_has_four_jumps_into_the_center() {
    let position = Position::default();
    assert_eq!(Hole::all().count(), 33);
    assert_eq!(position.peg_count(), 32);
    assert!(!position.occupied(Hole::CENTER));
    let jumps: Vec<_> = position.jumps().collect();
    assert_eq!(jumps.len(), 4);
    assert!(jumps.iter().all(|jump| jump.to == Hole::CENTER));
    assert_eq!(Hole::new(0, 0), None);
}

#[test]
fn peg_solitaire_validates_and_restores_jumps() {
    let mut round = Round::default();
    let start = round.position();
    let center = Hole::CENTER;
    let diagonal = Jump {
        from: hole(2, 2),
        to: center,
    };
    let occupied = Jump {
        from: hole(0, 3),
        to: hole(2, 3),
    };
    assert!(round.play(diagonal).is_none());
    assert!(round.play(occupied).is_none());

    let from = hole(1, 3);
    let jump = Jump { from, to: center };
    assert_eq!(round.play(jump), Some(()));
    assert!(round.position().occupied(center));
    assert!(!round.position().occupied(hole(2, 3)));
    assert!(!round.position().occupied(from));
    assert_eq!(round.position().peg_count(), 31);
    assert!(round.play(jump).is_none());

    let pair = [from.index(), center.index()];
    let mut restored = Round::restore(&[pair]).unwrap();
    assert_eq!(restored.position(), round.position());
    assert_eq!(restored.undo(), Some(jump));
    assert_eq!(restored.position(), start);
    assert!(Round::restore(&[pair, pair]).is_none());
    assert!(Round::restore(&[[255, center.index()]]).is_none());
}

#[test]
fn peg_solitaire_distinguishes_wins_and_stuck_boards() {
    let center = Hole::CENTER;
    let other = hole(0, 3);
    let edge = hole(6, 3);
    assert_eq!(
        Position(center.mask()).status(),
        Status::Won { center: true }
    );
    assert_eq!(
        Position(other.mask()).status(),
        Status::Won { center: false }
    );
    assert_eq!(Position(other.mask() | edge.mask()).status(), Status::Stuck);
}
