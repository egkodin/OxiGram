//! tdesktop's album mosaic (`Ui::LayoutMediaGroup`): where each photo or
//! video of an album goes, and which of the album's edges it touches.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// The album's edges a part touches (`RectParts`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct Sides {
    pub left: bool,
    pub top: bool,
    pub right: bool,
    pub bottom: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Placed {
    pub rect: Rect,
    pub sides: Sides,
}

const L: u8 = 1;
const T: u8 = 2;
const R: u8 = 4;
const B: u8 = 8;

fn placed(x: i32, y: i32, width: i32, height: i32, sides: u8) -> Placed {
    Placed {
        rect: Rect {
            x,
            y,
            width,
            height,
        },
        sides: Sides {
            left: sides & L != 0,
            top: sides & T != 0,
            right: sides & R != 0,
            bottom: sides & B != 0,
        },
    }
}

/// `base::SafeRound`: halves away from zero.
#[expect(clippy::cast_possible_truncation, reason = "pixel sizes")]
fn round(value: f64) -> i32 {
    value.round() as i32
}

struct Layouter {
    ratios: Vec<f64>,
    proportions: String,
    max_width: i32,
    max_height: i32,
    min_width: i32,
    spacing: i32,
    average_ratio: f64,
}

/// Places `sizes` (each part's width and height) in at most `max_width`.
pub(super) fn layout(
    sizes: &[(i32, i32)],
    max_width: i32,
    min_width: i32,
    spacing: i32,
) -> Vec<Placed> {
    let ratios: Vec<f64> = sizes
        .iter()
        .map(|&(width, height)| f64::from(width.max(1)) / f64::from(height.max(1)))
        .collect();
    let proportions = ratios
        .iter()
        .map(|&ratio| {
            if ratio > 1.2 {
                'w'
            } else if ratio < 0.8 {
                'n'
            } else {
                'q'
            }
        })
        .collect();
    #[expect(clippy::cast_precision_loss, reason = "a handful of parts")]
    let count = ratios.len() as f64;
    // tdesktop starts the sum at one; kept, as it picks the layouts.
    let average_ratio = ratios.iter().fold(1.0, |sum, ratio| sum + ratio) / count;
    let layouter = Layouter {
        ratios,
        proportions,
        max_width,
        // All apps use a square box first.
        max_height: max_width,
        min_width,
        spacing,
        average_ratio,
    };
    layouter.layout(sizes)
}

impl Layouter {
    fn layout(&self, sizes: &[(i32, i32)]) -> Vec<Placed> {
        let r = &self.ratios;
        match r.len() {
            0 => Vec::new(),
            1 => {
                let (width, height) = sizes[0];
                let height = height * self.max_width / width.max(1);
                vec![placed(0, 0, self.max_width, height, L | T | R | B)]
            }
            count if count >= 5 || r.iter().any(|&ratio| ratio > 2.0) => self.complex(),
            2 => self.two(),
            3 if self.proportions.starts_with('n') => self.three_left_and_other(),
            3 => self.three_top_and_other(),
            _ if self.proportions.starts_with('w') => self.four_top_and_other(),
            _ => self.four_left_and_other(),
        }
    }

    fn two(&self) -> Vec<Placed> {
        let (r, sp) = (&self.ratios, self.spacing);
        let (max_w, max_h) = (self.max_width, self.max_height);
        let is_wide = self.proportions == "ww";
        if is_wide && self.average_ratio > 1.4 && r[1] - r[0] < 0.2 {
            let width = max_w;
            let height = round(
                (f64::from(width) / r[0])
                    .min(f64::from(width) / r[1])
                    .min(f64::from(max_h - sp) / 2.0),
            );
            return vec![
                placed(0, 0, width, height, L | T | R),
                placed(0, height + sp, width, height, L | B | R),
            ];
        }
        if is_wide || self.proportions == "qq" {
            let width = (max_w - sp) / 2;
            let height = round(
                (f64::from(width) / r[0])
                    .min(f64::from(width) / r[1])
                    .min(f64::from(max_h)),
            );
            return vec![
                placed(0, 0, width, height, T | L | B),
                placed(width + sp, 0, width, height, T | R | B),
            ];
        }
        let minimal_width = round(f64::from(self.min_width) * 1.5);
        let free = f64::from(max_w - sp);
        let second_width = round((0.4 * free).max(free / r[0] / (1.0 / r[0] + 1.0 / r[1])))
            .min(max_w - sp - minimal_width);
        let first_width = max_w - second_width - sp;
        let height = max_h.min(round(
            (f64::from(first_width) / r[0]).min(f64::from(second_width) / r[1]),
        ));
        vec![
            placed(0, 0, first_width, height, T | L | B),
            placed(first_width + sp, 0, second_width, height, T | R | B),
        ]
    }

    fn three_left_and_other(&self) -> Vec<Placed> {
        let (r, sp) = (&self.ratios, self.spacing);
        let (max_w, max_h) = (self.max_width, self.max_height);
        let first_height = max_h;
        let third_height =
            round((f64::from(max_h - sp) / 2.0).min(r[1] * f64::from(max_w - sp) / (r[2] + r[1])));
        let second_height = first_height - third_height - sp;
        let right_width = self.min_width.max(round(
            (f64::from(max_w - sp) / 2.0)
                .min(f64::from(third_height) * r[2])
                .min(f64::from(second_height) * r[1]),
        ));
        let left_width = round(f64::from(first_height) * r[0]).min(max_w - sp - right_width);
        let right = left_width + sp;
        vec![
            placed(0, 0, left_width, first_height, T | L | B),
            placed(right, 0, right_width, second_height, T | R),
            placed(right, second_height + sp, right_width, third_height, B | R),
        ]
    }

    fn three_top_and_other(&self) -> Vec<Placed> {
        let (r, sp) = (&self.ratios, self.spacing);
        let (max_w, max_h) = (self.max_width, self.max_height);
        let first_width = max_w;
        let first_height = round((f64::from(first_width) / r[0]).min(f64::from(max_h - sp) * 0.66));
        let second_width = (max_w - sp) / 2;
        let second_height = (max_h - first_height - sp).min(round(
            (f64::from(second_width) / r[1]).min(f64::from(second_width) / r[2]),
        ));
        let third_width = first_width - second_width - sp;
        let top = first_height + sp;
        vec![
            placed(0, 0, first_width, first_height, L | T | R),
            placed(0, top, second_width, second_height, B | L),
            placed(second_width + sp, top, third_width, second_height, B | R),
        ]
    }

    fn four_top_and_other(&self) -> Vec<Placed> {
        let (r, sp) = (&self.ratios, self.spacing);
        let (max_w, max_h) = (self.max_width, self.max_height);
        let w = max_w;
        let h0 = round((f64::from(w) / r[0]).min(f64::from(max_h - sp) * 0.66));
        let free = f64::from(max_w - 2 * sp);
        let h = round(free / (r[1] + r[2] + r[3]));
        let w0 = self
            .min_width
            .max(round((free * 0.4).min(f64::from(h) * r[1])));
        let w2 = round(
            f64::from(self.min_width)
                .max(free * 0.33)
                .max(f64::from(h) * r[3]),
        );
        let w1 = w - w0 - w2 - 2 * sp;
        let h1 = (max_h - h0 - sp).min(h);
        let top = h0 + sp;
        vec![
            placed(0, 0, w, h0, L | T | R),
            placed(0, top, w0, h1, B | L),
            placed(w0 + sp, top, w1, h1, B),
            placed(w0 + sp + w1 + sp, top, w2, h1, R | B),
        ]
    }

    fn four_left_and_other(&self) -> Vec<Placed> {
        let (r, sp) = (&self.ratios, self.spacing);
        let (max_w, max_h) = (self.max_width, self.max_height);
        let h = max_h;
        let w0 = round((f64::from(h) * r[0]).min(f64::from(max_w - sp) * 0.6));
        let w = round(f64::from(max_h - 2 * sp) / (1.0 / r[1] + 1.0 / r[2] + 1.0 / r[3]));
        let h0 = round(f64::from(w) / r[1]);
        let h1 = round(f64::from(w) / r[2]);
        let h2 = h - h0 - h1 - 2 * sp;
        let w1 = self.min_width.max((max_w - w0 - sp).min(w));
        let left = w0 + sp;
        vec![
            placed(0, 0, w0, h, T | L | B),
            placed(left, 0, w1, h0, T | R),
            placed(left, h0 + sp, w1, h1, R),
            placed(left, h0 + h1 + 2 * sp, w1, h2, B | R),
        ]
    }

    /// Rows of one to three (four in tall albums), the split whose height
    /// comes closest to 4:3 of the width (`ComplexLayouter`).
    fn complex(&self) -> Vec<Placed> {
        let ratios: Vec<f64> = self
            .ratios
            .iter()
            .map(|&ratio| {
                if self.average_ratio > 1.1 {
                    ratio.clamp(1.0, 2.75)
                } else {
                    ratio.clamp(0.6667, 1.0)
                }
            })
            .collect();
        let max_width = self.max_width;
        let max_height = max_width * 4 / 3;
        let sp = self.spacing;
        let middle_max = if self.average_ratio < 0.85 { 4 } else { 3 };
        let best = splits(ratios.len(), middle_max)
            .into_iter()
            .map(|counts| {
                let heights = row_heights(&ratios, &counts, max_width, sp);
                (counts, heights)
            })
            .min_by(|a, b| {
                let score = |(counts, heights): &(Vec<usize>, Vec<f64>)| {
                    split_score(counts, heights, max_height, self.min_width, sp)
                };
                score(a).total_cmp(&score(b))
            });
        let Some((counts, heights)) = best else {
            return Vec::new();
        };
        let mut result = Vec::with_capacity(ratios.len());
        let mut index = 0;
        let mut y = 0;
        let rows = counts.len();
        for (row, (&columns, &line_height)) in counts.iter().zip(&heights).enumerate() {
            let height = round(line_height);
            let mut x = 0;
            for column in 0..columns {
                let is_last = column + 1 == columns;
                let width = if is_last {
                    max_width - x
                } else {
                    round(ratios[index] * line_height)
                };
                let sides = if row == 0 { T } else { 0 }
                    | if row + 1 == rows { B } else { 0 }
                    | if column == 0 { L } else { 0 }
                    | if is_last { R } else { 0 };
                result.push(placed(x, y, width, height, sides));
                x += width + sp;
                index += 1;
            }
            y += height + sp;
        }
        result
    }
}

/// The ways to cut `count` parts into two, three or four rows, in
/// tdesktop's order, each row at most three (`middle_max` for the middle of
/// three rows).
fn splits(count: usize, middle_max: usize) -> Vec<Vec<usize>> {
    let mut result = Vec::new();
    for first in 1..count {
        let second = count - first;
        if first <= 3 && second <= 3 {
            result.push(vec![first, second]);
        }
    }
    for first in 1..count.saturating_sub(1) {
        for second in 1..count - first {
            let third = count - first - second;
            if first <= 3 && second <= middle_max && third <= 3 {
                result.push(vec![first, second, third]);
            }
        }
    }
    for first in 1..count.saturating_sub(1) {
        for second in 1..count - first {
            for third in 1..count - first - second {
                let fourth = count - first - second - third;
                if [first, second, third, fourth].iter().all(|&n| n <= 3) {
                    result.push(vec![first, second, third, fourth]);
                }
            }
        }
    }
    result
}

/// Each row's height when its parts fill the width side by side.
fn row_heights(ratios: &[f64], counts: &[usize], max_width: i32, spacing: i32) -> Vec<f64> {
    let mut offset = 0;
    counts
        .iter()
        .map(|&count| {
            let sum: f64 = ratios[offset..offset + count].iter().sum();
            offset += count;
            let gaps = i32::try_from(count - 1).unwrap_or(0) * spacing;
            f64::from(max_width - gaps) / sum
        })
        .collect()
}

/// How far a split's height is from the box, worse for rows thinner than
/// the minimum and for rows longer than the next.
fn split_score(
    counts: &[usize],
    heights: &[f64],
    max_height: i32,
    min_width: i32,
    spacing: i32,
) -> f64 {
    let gaps = i32::try_from(counts.len() - 1).unwrap_or(0) * spacing;
    let total = heights.iter().sum::<f64>() + f64::from(gaps);
    let lowest = heights.iter().copied().fold(f64::INFINITY, f64::min);
    let thin = if lowest < f64::from(min_width) {
        1.5
    } else {
        1.0
    };
    let shrinking = if counts.windows(2).any(|pair| pair[0] > pair[1]) {
        1.5
    } else {
        1.0
    };
    (total - f64::from(max_height)).abs() * thin * shrinking
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rects(sizes: &[(i32, i32)]) -> Vec<(i32, i32, i32, i32)> {
        layout(sizes, 430, 100, 4)
            .into_iter()
            .map(|placed| {
                let Rect {
                    x,
                    y,
                    width,
                    height,
                } = placed.rect;
                (x, y, width, height)
            })
            .collect()
    }

    #[test]
    fn two_squares_share_the_row() {
        assert_eq!(
            rects(&[(1000, 1000), (1000, 1000)]),
            [(0, 0, 213, 213), (217, 0, 213, 213)]
        );
    }

    #[test]
    fn two_landscapes_stack() {
        // tdesktop's average starts at one, so 3:2 pairs count as wide.
        assert_eq!(
            rects(&[(1280, 853), (1280, 853)]),
            [(0, 0, 430, 213), (0, 217, 430, 213)]
        );
    }

    #[test]
    fn two_wide_panoramas_stack() {
        assert_eq!(
            rects(&[(1600, 900), (1600, 900)]),
            [(0, 0, 430, 213), (0, 217, 430, 213)]
        );
    }

    #[test]
    fn a_portrait_leads_three() {
        let placed = layout(&[(720, 1280), (1280, 853), (1280, 853)], 430, 100, 4);
        // The right column keeps half the width, so the portrait is cropped.
        assert_eq!(
            placed[0].rect,
            Rect {
                x: 0,
                y: 0,
                width: 213,
                height: 430
            }
        );
        assert!(placed[0].sides.left && placed[0].sides.top && placed[0].sides.bottom);
        assert!(!placed[0].sides.right);
        assert_eq!(placed[1].rect.x, 217);
        assert_eq!(placed[1].rect.width + placed[1].rect.x, 430);
        assert_eq!(placed[2].rect.y, placed[1].rect.height + 4);
    }

    #[test]
    fn a_landscape_tops_four() {
        let placed = rects(&[(1280, 720), (1000, 1000), (1000, 1000), (1000, 1000)]);
        assert_eq!(placed[0], (0, 0, 430, 242));
        // The three below fill the width with the gaps.
        let row = &placed[1..];
        assert!(row.iter().all(|&(_, y, _, _)| y == 246));
        assert_eq!(row[2].0 + row[2].2, 430);
    }

    #[test]
    fn many_parts_fill_every_row_to_the_edge() {
        let sizes = [(1000, 1000); 7];
        let placed = layout(&sizes, 430, 100, 4);
        assert_eq!(placed.len(), 7);
        for part in placed.iter().filter(|part| part.sides.right) {
            assert_eq!(part.rect.x + part.rect.width, 430);
        }
        assert!(placed[0].sides.top && placed[0].sides.left);
        assert!(placed[6].sides.bottom && placed[6].sides.right);
    }

    #[test]
    fn splits_follow_tdesktops_order() {
        assert_eq!(splits(4, 3)[..3], [vec![1, 3], vec![2, 2], vec![3, 1]]);
        assert!(
            splits(7, 3)
                .iter()
                .all(|split| split.iter().sum::<usize>() == 7)
        );
    }
}
