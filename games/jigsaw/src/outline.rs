use super::Size;

/// A point measured in cell widths and heights from the painting's top-left corner.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OutlinePoint {
    pub x: f32,
    pub y: f32,
}

impl OutlinePoint {
    fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    fn lerp(self, to: Self, t: f32) -> Self {
        Self::new(self.x + (to.x - self.x) * t, self.y + (to.y - self.y) * t)
    }
}

/// The open polygon traced clockwise around a piece. Consecutive pieces share
/// exactly the same seam, which each traverses in the opposite direction.
pub fn piece_outline(size: Size, piece: usize) -> Option<Vec<OutlinePoint>> {
    let (columns, rows) = size.dimensions();
    let (column, row) = (piece % columns, piece / columns);
    if row >= rows {
        return None;
    }

    let mut outline = Vec::new();
    for (axis, x, y, outer, reverse) in [
        (Axis::Horizontal, column, row, row == 0, false),
        (
            Axis::Vertical,
            column + 1,
            row,
            column + 1 == columns,
            false,
        ),
        (Axis::Horizontal, column, row + 1, row + 1 == rows, true),
        (Axis::Vertical, column, row, column == 0, true),
    ] {
        let mut seam = seam(axis, x, y, outer);
        if reverse {
            seam.reverse();
        }
        seam.pop();
        outline.extend(seam);
    }
    Some(outline)
}

#[derive(Clone, Copy)]
enum Axis {
    Horizontal,
    Vertical,
}

fn seam(axis: Axis, x: usize, y: usize, flat: bool) -> Vec<OutlinePoint> {
    let from_edge = |along: f32, perpendicular: f32| match axis {
        Axis::Horizontal => OutlinePoint::new(x as f32 + along, y as f32 + perpendicular),
        Axis::Vertical => OutlinePoint::new(x as f32 + perpendicular, y as f32 + along),
    };
    let start = from_edge(0., 0.);
    let end = from_edge(1., 0.);
    if flat {
        return vec![start, end];
    }

    // The seam's address determines its direction, neck width, and offset.
    // Both neighbors use that same address to cut the artwork and paint the board.
    let address = (x as u32 + 1) * 719 + (y as u32 + 1) * 1303 + axis as u32 * 193;
    let hash = (address ^ (address >> 16)).wrapping_mul(0x85eb_ca6b);
    let hash = (hash ^ (hash >> 13)).wrapping_mul(0xc2b2_ae35);
    let hash = hash ^ (hash >> 16);
    let sign = if hash & 1 == 0 { 1. } else { -1. };
    let center = 0.44 + ((hash >> 1) % 13) as f32 * 0.01;
    let width = 0.87 + ((hash >> 5) % 23) as f32 * 0.01;
    let depth = 0.87 + ((hash >> 10) % 23) as f32 * 0.01;
    let tab = |u: f32, v: f32| from_edge(center + u * width, v * depth * sign);

    let mut points = vec![start, tab(-0.19, 0.)];
    for (a, b, target) in [
        (tab(-0.10, 0.), tab(-0.052, -0.022), tab(-0.058, 0.038)),
        (tab(-0.058, 0.067), tab(-0.129, 0.065), tab(-0.130, 0.137)),
        (tab(-0.135, 0.251), tab(0.135, 0.251), tab(0.130, 0.137)),
        (tab(0.129, 0.065), tab(0.058, 0.067), tab(0.058, 0.038)),
        (tab(0.052, -0.022), tab(0.10, 0.), tab(0.19, 0.)),
    ] {
        let previous = *points.last().unwrap();
        // Sample each cubic uniformly so image masks and board outlines use the same geometry.
        points.extend((1..=8).map(|step| {
            let t = step as f32 / 8.;
            let left = previous.lerp(a, t).lerp(a.lerp(b, t), t);
            let right = a.lerp(b, t).lerp(b.lerp(target, t), t);
            left.lerp(right, t)
        }));
    }
    points.push(end);
    points
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adjacent_pieces_share_their_full_curved_edge() {
        for size in Size::ALL {
            let (columns, rows) = size.dimensions();
            for row in 0..rows {
                for column in 0..columns {
                    let x = column as f32;
                    let y = row as f32;
                    let piece = piece_outline(size, row * columns + column).unwrap();
                    if column + 1 < columns {
                        let other = piece_outline(size, row * columns + column + 1).unwrap();
                        assert_shared_seam(&piece, &other, (x + 1., y), (x + 1., y + 1.));
                    }
                    if row + 1 < rows {
                        let other = piece_outline(size, (row + 1) * columns + column).unwrap();
                        assert_shared_seam(&piece, &other, (x + 1., y + 1.), (x, y + 1.));
                    }
                    assert!(piece.iter().all(|p| p.x >= 0.
                        && p.y >= 0.
                        && p.x <= columns as f32
                        && p.y <= rows as f32));
                }
            }
            assert!(piece_outline(size, size.piece_count()).is_none());
        }
    }

    fn assert_shared_seam(
        first: &[OutlinePoint],
        second: &[OutlinePoint],
        start: (f32, f32),
        end: (f32, f32),
    ) {
        fn between(
            outline: &[OutlinePoint],
            from: (f32, f32),
            to: (f32, f32),
        ) -> Vec<OutlinePoint> {
            outline
                .iter()
                .copied()
                .cycle()
                .skip_while(|p| (p.x, p.y) != from)
                .take_while(|p| (p.x, p.y) != to)
                .skip(1)
                .collect()
        }

        let seam = between(first, start, end);
        assert!(seam.len() > 2);
        assert!(
            seam.into_iter()
                .eq(between(second, end, start).into_iter().rev())
        );
    }
}
