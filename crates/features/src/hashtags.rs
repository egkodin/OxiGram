//! Recently searched hashtags (`cRecentSearchHashtags`): found in the
//! search query, ranked by use, offered while typing a hashtag.

/// A tag without its `#` and how often it was used, most used first.
pub(crate) type Recent = Vec<(String, u32)>;

/// tdesktop keeps this many (`incrementRecentHashtag`).
const RECENT_LIMIT: usize = 64;
/// Past this count every count halves, so new tags can catch up.
const COUNT_CEILING: u32 = 0x4000;
/// The search shows this many (`kHashtagResultsLimit`).
const SHOWN_LIMIT: usize = 5;
/// A hashtag's length without `#` (`ExpressionHashtag`).
const TAG_LENGTH: std::ops::RangeInclusive<usize> = 2..=64;

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// The hashtags in `text`, without `#`: 2 to 64 word characters after a
/// `#` that starts the text or follows a separator, not all digits
/// (`ExpressionHashtag`, `ExpressionHashtagExclude`).
pub(crate) fn tags_in(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut tags = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let starts = chars[i] == '#' && (i == 0 || !is_word(chars[i - 1]) && chars[i - 1] != '#');
        if !starts {
            i += 1;
            continue;
        }
        let end = (i + 1..chars.len())
            .find(|&j| !is_word(chars[j]))
            .unwrap_or(chars.len());
        let tag: String = chars[i + 1..end].iter().collect();
        let length = end - i - 1;
        if TAG_LENGTH.contains(&length) && !tag.chars().all(|c| c.is_ascii_digit()) {
            tags.push(tag);
        }
        i = end.max(i + 1);
    }
    tags
}

/// Counts one more use of `tag`, keeping the most used first
/// (`incrementRecentHashtag`).
pub(crate) fn increment(recent: &mut Recent, tag: &str) {
    let index = match recent.iter().position(|(known, _)| known == tag) {
        Some(index) => {
            recent[index].1 += 1;
            if recent[index].1 > COUNT_CEILING {
                for (_, count) in recent.iter_mut() {
                    *count = (*count / 2).max(1);
                }
            }
            index
        }
        None => {
            recent.truncate(RECENT_LIMIT - 1);
            recent.push((tag.to_owned(), 1));
            recent.len() - 1
        }
    };
    // Ties go to the newest use.
    let mut index = index;
    while index > 0 && recent[index - 1].1 <= recent[index].1 {
        recent.swap(index - 1, index);
        index -= 1;
    }
}

/// The recent tags that continue `query`, a `#` and a prefix; the tag the
/// query already spells out is left out (`onHashtagFilterUpdate`).
pub(crate) fn matching(recent: &Recent, query: &str) -> Vec<String> {
    let Some(prefix) = query.strip_prefix('#') else {
        return Vec::new();
    };
    let prefix = prefix.to_lowercase();
    recent
        .iter()
        .map(|(tag, _)| tag)
        .filter(|tag| {
            tag.to_lowercase().starts_with(&prefix) && tag.chars().count() != prefix.chars().count()
        })
        .take(SHOWN_LIMIT)
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_hashtags_after_separators_only() {
        assert_eq!(tags_in("#rust and (#gpui)"), ["rust", "gpui"]);
        assert_eq!(tags_in("mail#tag ##double #1234 #a #ok_2"), ["ok_2"]);
        assert_eq!(tags_in("#привет"), ["привет"]);
        assert!(tags_in(&format!("#{}", "x".repeat(65))).is_empty());
    }

    #[test]
    fn ranks_by_use_with_ties_to_the_newest() {
        let mut recent = Recent::new();
        increment(&mut recent, "a");
        increment(&mut recent, "b");
        assert_eq!(recent, [("b".to_owned(), 1), ("a".to_owned(), 1)]);
        increment(&mut recent, "a");
        increment(&mut recent, "c");
        assert_eq!(
            recent,
            [
                ("a".to_owned(), 2),
                ("c".to_owned(), 1),
                ("b".to_owned(), 1)
            ]
        );
    }

    #[test]
    fn keeps_the_limit_and_halves_past_the_ceiling() {
        let mut recent: Recent = (0..RECENT_LIMIT).map(|i| (i.to_string(), 1)).collect();
        increment(&mut recent, "new");
        assert_eq!(recent.len(), RECENT_LIMIT);
        assert_eq!(recent[0].0, "new");
        let mut recent: Recent = vec![("a".to_owned(), COUNT_CEILING), ("b".to_owned(), 3)];
        increment(&mut recent, "a");
        assert_eq!(recent, [("a".to_owned(), 0x2000), ("b".to_owned(), 1)]);
    }

    #[test]
    fn offers_continuations_of_the_typed_tag() {
        let recent: Recent = vec![("Rust".to_owned(), 3), ("rustlang".to_owned(), 1)];
        assert_eq!(matching(&recent, "#ru"), ["Rust", "rustlang"]);
        assert_eq!(matching(&recent, "#rust"), ["rustlang"]);
        assert_eq!(matching(&recent, "#"), ["Rust", "rustlang"]);
        assert!(matching(&recent, "rust").is_empty());
    }
}
