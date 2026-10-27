//! Lists of strings ranked by use, most used first, as tdesktop keeps its
//! recent hashtags and emoji (`incrementRecentHashtag`,
//! `Settings::incrementRecentEmoji`).

/// An item and how often it was used, most used first.
pub(crate) type Ranked = Vec<(String, u32)>;

/// How long a list grows and when its counts halve.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Limits {
    pub length: usize,
    /// Past this count every count halves, so new items can catch up.
    pub ceiling: u32,
}

/// Counts one more use of `item`; a new item drops the least used once
/// the list is full.
pub(crate) fn increment(ranked: &mut Ranked, item: &str, limits: Limits) {
    let index = match ranked.iter().position(|(known, _)| known == item) {
        Some(index) => {
            ranked[index].1 += 1;
            if ranked[index].1 > limits.ceiling {
                for (_, count) in ranked.iter_mut() {
                    *count = (*count / 2).max(1);
                }
            }
            index
        }
        None => {
            ranked.truncate(limits.length.saturating_sub(1));
            ranked.push((item.to_owned(), 1));
            ranked.len() - 1
        }
    };
    // Ties go to the newest use.
    let mut index = index;
    while index > 0 && ranked[index - 1].1 <= ranked[index].1 {
        ranked.swap(index - 1, index);
        index -= 1;
    }
}
