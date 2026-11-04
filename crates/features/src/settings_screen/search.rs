//! Settings search (`Settings::Search`): every entry's title and keywords
//! split into words; a query word matches the start of one of them, and the
//! entries matching the most words come first.

use std::collections::HashSet;

use i18n::Key;

use super::PageId;

/// What a found entry opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Target {
    /// A section of its own (`addSectionButton`).
    Page(PageId),
    /// An item on its page, which opens that page.
    Item,
}

/// A searchable item (`Builder::SearchEntry`).
#[derive(Debug, Clone, Copy)]
pub(super) struct Entry {
    pub id: &'static str,
    pub title: Key,
    pub keywords: &'static [&'static str],
    /// The page it is on.
    pub page: PageId,
    pub target: Target,
    pub icon: Option<style::Icon>,
}

/// An entry with its translated title, ready to match.
#[derive(Debug, Clone)]
pub(super) struct Indexed {
    pub entry: Entry,
    pub title: String,
    terms: Vec<String>,
}

impl Indexed {
    pub(super) fn new(entry: Entry, title: String) -> Self {
        let combined = format!("{title} {}", entry.keywords.join(" "));
        Self {
            entry,
            terms: words(&combined),
            title,
        }
    }
}

/// Lowercase words, split at anything but letters and digits
/// (`TextUtilities::PrepareSearchWords`).
// shortcut: no accent folding (`RemoveAccents`); add it when a language pack's titles need it.
fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// Whether `query` has a word to search for.
pub(super) fn is_query(query: &str) -> bool {
    !words(query).is_empty()
}

/// The entries matching `query`, best first (`Search::rebuildResults`).
pub(super) fn find<'a>(index: &'a [Indexed], query: &str) -> Vec<&'a Indexed> {
    let query = words(query);
    // `_firstLetterIndex`: only the entries with a word starting as the
    // rarest first letter of the query; none when a letter starts nothing.
    let mut candidates: Option<HashSet<usize>> = None;
    for word in &query {
        let Some(first) = word.chars().next() else {
            continue;
        };
        let with_letter: HashSet<usize> = index
            .iter()
            .enumerate()
            .filter(|(_, indexed)| indexed.terms.iter().any(|term| term.starts_with(first)))
            .map(|(position, _)| position)
            .collect();
        if with_letter.is_empty() {
            return Vec::new();
        }
        if candidates
            .as_ref()
            .is_none_or(|known| with_letter.len() < known.len())
        {
            candidates = Some(with_letter);
        }
    }
    let Some(candidates) = candidates else {
        return Vec::new();
    };
    let mut found: Vec<(usize, &Indexed)> = candidates
        .into_iter()
        .filter_map(|position| {
            let indexed = &index[position];
            let matched = query
                .iter()
                .filter(|word| {
                    indexed
                        .terms
                        .iter()
                        .any(|term| term.starts_with(word.as_str()))
                })
                .count();
            (matched > 0).then_some((matched, indexed))
        })
        .collect();
    found.sort_by(|(matched_a, a), (matched_b, b)| {
        matched_b
            .cmp(matched_a)
            .then(a.entry.page.depth().cmp(&b.entry.page.depth()))
            .then_with(|| a.title.cmp(&b.title))
    });
    found.into_iter().map(|(_, indexed)| indexed).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn indexed(id: &'static str, title: &str, keywords: &'static [&'static str]) -> Indexed {
        Indexed::new(
            Entry {
                id,
                title: i18n::key::lng_settings_language,
                keywords,
                page: PageId::Main,
                target: Target::Item,
                icon: None,
            },
            title.to_owned(),
        )
    }

    fn ids(index: &[Indexed], query: &str) -> Vec<&'static str> {
        find(index, query)
            .into_iter()
            .map(|indexed| indexed.entry.id)
            .collect()
    }

    #[test]
    fn words_match_starts_and_more_matches_come_first() {
        let index = [
            indexed("notify", "Notifications and Sounds", &["alerts", "badge"]),
            indexed("privacy", "Privacy and Security", &["passcode", "password"]),
            indexed("language", "Language", &["translate"]),
        ];
        assert_eq!(ids(&index, "pass"), ["privacy"]);
        assert_eq!(ids(&index, "SOUND"), ["notify"]);
        assert_eq!(ids(&index, "and sec"), ["privacy", "notify"]);
        assert!(ids(&index, "zebra").is_empty());
        assert!(ids(&index, "  ").is_empty());
    }
}
