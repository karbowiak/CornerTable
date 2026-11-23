use super::{Board, Difficulty};
use std::fmt::Write;

const PREFIX: &str = "minesweeper:v1:";

impl Board {
    /// Share the fixed opening and field, independently of player progress or generator changes.
    pub fn deal_code(&self) -> Option<String> {
        let opening = self.opening?;
        let mut code = format!(
            "{PREFIX}{}:{:016x}:{opening}:",
            self.difficulty.id(),
            self.seed
        );
        // Each pair of hex digits encodes eight row-major cells, least significant bit first.
        for chunk in self.cells.chunks(8) {
            let byte = chunk
                .iter()
                .enumerate()
                .fold(0u8, |bits, (bit, cell)| bits | (u8::from(cell.mine) << bit));
            write!(code, "{byte:02x}").unwrap();
        }
        Some(code)
    }

    /// Reconstruct a covered practice field. Only its original first reveal can start play.
    pub fn from_deal_code(text: &str) -> Option<Self> {
        if text.len() > 256 {
            return None;
        }
        let mut parts = text.trim().strip_prefix(PREFIX)?.split(':');
        let id = parts.next()?;
        let difficulty = Difficulty::ALL.into_iter().find(|d| d.id() == id)?;
        let seed = parts.next()?;
        if seed.len() != 16 || !seed.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let seed = u64::from_str_radix(seed, 16).ok()?;
        let opening = parts.next()?;
        if !opening.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let opening = opening.parse().ok()?;
        let mask = parts.next()?;
        let (width, height) = difficulty.dimensions();
        let size = width * height;
        if parts.next().is_some()
            || mask.len() != 2 * size.div_ceil(8)
            || !mask.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return None;
        }
        let mut mines = Vec::with_capacity(difficulty.mines());
        for offset in (0..mask.len()).step_by(2) {
            let byte = u8::from_str_radix(&mask[offset..offset + 2], 16).ok()?;
            for bit in 0..8 {
                if byte & (1 << bit) != 0 {
                    mines.push(offset / 2 * 8 + bit);
                }
            }
        }
        let mut board = Self::new(difficulty, seed);
        board.install_layout(opening, &mines)?;
        Some(board)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Status;

    #[test]
    fn every_size_shares_only_the_layout_and_required_opening() {
        for difficulty in Difficulty::ALL {
            let mut source = Board::new(difficulty, u64::MAX);
            assert!(source.deal_code().is_none());
            let (w, h) = difficulty.dimensions();
            assert!(source.reveal(w - 1, h - 1));
            let first = source.checkpoint();
            let code = source.deal_code().unwrap();
            let mine = first.mines[0];
            assert!(source.toggle_flag(first.mines[1] % w, first.mines[1] / w));
            assert!(source.reveal(mine % w, mine / w));
            let lost = source.checkpoint();
            assert_eq!(source.deal_code().unwrap(), code);
            let mut imported = Board::from_deal_code(&format!(" {code}\n")).unwrap();
            assert_eq!(imported.status(), Status::Ready);
            assert_eq!(imported.flags(), 0);
            assert!(!imported.reveal(0, 0));
            assert!(imported.reveal(w - 1, h - 1));
            assert_eq!(imported.checkpoint(), first);
            assert_eq!(source.checkpoint(), lost);
        }
    }

    #[test]
    fn v1_literal_preserves_an_explicit_layout_and_rejects_malformed_inputs() {
        // Mines at cells 16 through 25. The layout is independent of seed zero.
        let sample = "minesweeper:v1:meadow:0000000000000000:0:0000ff0300000000000000";
        let board = Board::from_deal_code(sample).unwrap();
        assert_eq!(board.deal_code().unwrap(), sample);
        assert_eq!(board.checkpoint().mines, (16..26).collect::<Vec<_>>());
        for bad in [
            sample.replace(":v1:", ":v2:"),
            sample.replace("meadow", "custom"),
            sample.replace(":0:", ":81:"),
            sample.replace(":0:", ":16:"),
            sample.replace(":0:", ":7:"), // opening next to a mine
            sample.replace("ff03", "ff01"),
            sample.replace("ff03", "ffzz"),
            sample.replace("ff03", "ffø"),
            sample.replace("ff03", "ff0"),
            sample.replace("0000000000000000:", "+000000000000000:"),
            format!("{}80", &sample[..sample.len() - 2]).replace("ff03", "ff01"),
            format!("{sample}:"),
            " ".repeat(257),
        ] {
            assert!(Board::from_deal_code(&bad).is_none(), "{bad}");
        }
    }
}
