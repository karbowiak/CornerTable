use cozy_chess::{Board, File, Move, Piece};

pub(super) fn san(
    before: &Board,
    after: &Board,
    raw: Move,
    movement: Move,
    capture: bool,
) -> String {
    let mut text = String::with_capacity(7);
    if raw.to != movement.to {
        text.push_str(if movement.to.file() == File::G {
            "O-O"
        } else {
            "O-O-O"
        });
    } else {
        let piece = before.piece_on(raw.from).unwrap();
        if piece == Piece::Pawn {
            if capture {
                text.push(file_letter(raw));
            }
        } else {
            text.push(piece_letter(piece));
            disambiguate(before, raw, piece, &mut text);
        }
        if capture {
            text.push('x');
        }
        text.push_str(&movement.to.to_string());
        if let Some(promoted) = movement.promotion {
            text.push('=');
            text.push(piece_letter(promoted));
        }
    }
    if !after.checkers().is_empty() {
        text.push(if after.generate_moves(|_| true) {
            '+'
        } else {
            '#'
        });
    }
    text
}

fn disambiguate(board: &Board, raw: Move, piece: Piece, text: &mut String) {
    let others = board.colored_pieces(board.side_to_move(), piece) - raw.from.bitboard();
    let mut ambiguous = false;
    let mut same_file = false;
    let mut same_rank = false;
    board.generate_moves_for(others, |moves| {
        if moves.to.has(raw.to) {
            ambiguous = true;
            same_file |= moves.from.file() == raw.from.file();
            same_rank |= moves.from.rank() == raw.from.rank();
        }
        false
    });
    if ambiguous {
        if !same_file || same_rank {
            text.push(file_letter(raw));
        }
        if same_file {
            text.push(char::from(b'1' + raw.from.rank() as u8));
        }
    }
}

fn file_letter(movement: Move) -> char {
    char::from(b'a' + movement.from.file() as u8)
}

fn piece_letter(piece: Piece) -> char {
    match piece {
        Piece::Pawn => 'P',
        Piece::Knight => 'N',
        Piece::Bishop => 'B',
        Piece::Rook => 'R',
        Piece::Queen => 'Q',
        Piece::King => 'K',
    }
}
