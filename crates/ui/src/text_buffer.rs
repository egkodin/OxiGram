//! The text, selection and composition a field edits, with the cursor
//! moves that need no layout: by grapheme, by word, by paragraph.

use std::ops::Range;

use unicode_segmentation::UnicodeSegmentation as _;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct TextBuffer {
    pub(crate) text: String,
    /// Byte offsets into `text`, on char boundaries.
    pub(crate) selected: Range<usize>,
    /// The cursor is at the selection's start.
    pub(crate) reversed: bool,
    /// The IME composition, if one is under way.
    pub(crate) marked: Option<Range<usize>>,
}

impl TextBuffer {
    pub(crate) fn cursor(&self) -> usize {
        if self.reversed {
            self.selected.start
        } else {
            self.selected.end
        }
    }

    pub(crate) fn move_to(&mut self, offset: usize) {
        let offset = offset.min(self.text.len());
        self.selected = offset..offset;
        self.reversed = false;
    }

    /// Moves the cursor end of the selection, keeping the other end.
    pub(crate) fn select_to(&mut self, offset: usize) {
        let offset = offset.min(self.text.len());
        if self.reversed {
            self.selected.start = offset;
        } else {
            self.selected.end = offset;
        }
        if self.selected.end < self.selected.start {
            self.reversed = !self.reversed;
            self.selected = self.selected.end..self.selected.start;
        }
    }

    pub(crate) fn select_all(&mut self) {
        self.selected = 0..self.text.len();
        self.reversed = false;
    }

    pub(crate) fn selected_text(&self) -> &str {
        &self.text[self.selected.clone()]
    }

    /// Replaces `range` and puts the cursor after the new text.
    pub(crate) fn replace(&mut self, range: Range<usize>, new_text: &str) {
        self.text.replace_range(range.clone(), new_text);
        self.move_to(range.start + new_text.len());
        self.marked = None;
    }

    /// What an edit replaces: the composition, else the selection.
    pub(crate) fn edited_range(&self) -> Range<usize> {
        self.marked.clone().unwrap_or(self.selected.clone())
    }

    pub(crate) fn previous_boundary(&self, offset: usize) -> usize {
        self.text
            .grapheme_indices(true)
            .rev()
            .find_map(|(index, _)| (index < offset).then_some(index))
            .unwrap_or(0)
    }

    pub(crate) fn next_boundary(&self, offset: usize) -> usize {
        self.text
            .grapheme_indices(true)
            .find_map(|(index, _)| (index > offset).then_some(index))
            .unwrap_or(self.text.len())
    }

    pub(crate) fn previous_word(&self, offset: usize) -> usize {
        self.text
            .split_word_bound_indices()
            .rev()
            .find(|(index, word)| *index < offset && word.chars().any(char::is_alphanumeric))
            .map_or(0, |(index, _)| index)
    }

    pub(crate) fn next_word(&self, offset: usize) -> usize {
        self.text
            .split_word_bound_indices()
            .find(|(index, word)| {
                index + word.len() > offset && word.chars().any(char::is_alphanumeric)
            })
            .map_or(self.text.len(), |(index, word)| index + word.len())
    }

    /// The word around `offset`, for a double click.
    pub(crate) fn word_at(&self, offset: usize) -> Range<usize> {
        self.text
            .split_word_bound_indices()
            .find(|(index, word)| index + word.len() > offset)
            .map_or(offset..offset, |(index, word)| index..index + word.len())
    }

    /// The paragraph (text between newlines) around `offset`.
    pub(crate) fn paragraph_at(&self, offset: usize) -> Range<usize> {
        let start = self.text[..offset].rfind('\n').map_or(0, |index| index + 1);
        let end = self.text[offset..]
            .find('\n')
            .map_or(self.text.len(), |index| offset + index);
        start..end
    }

    pub(crate) fn offset_from_utf16(&self, offset: usize) -> usize {
        let mut utf8 = 0;
        let mut utf16 = 0;
        for c in self.text.chars() {
            if utf16 >= offset {
                break;
            }
            utf16 += c.len_utf16();
            utf8 += c.len_utf8();
        }
        utf8
    }

    pub(crate) fn offset_to_utf16(&self, offset: usize) -> usize {
        self.text[..offset.min(self.text.len())]
            .encode_utf16()
            .count()
    }

    pub(crate) fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    pub(crate) fn range_from_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range.start)..self.offset_from_utf16(range.end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buffer(text: &str, cursor: usize) -> TextBuffer {
        let mut buffer = TextBuffer {
            text: text.to_owned(),
            ..TextBuffer::default()
        };
        buffer.move_to(cursor);
        buffer
    }

    #[test]
    fn selecting_back_past_the_anchor_reverses() {
        let mut buffer = buffer("hello", 2);
        buffer.select_to(4);
        assert_eq!((buffer.selected.clone(), buffer.reversed), (2..4, false));
        buffer.select_to(0);
        assert_eq!((buffer.selected.clone(), buffer.reversed), (0..2, true));
        assert_eq!(buffer.cursor(), 0);
    }

    #[test]
    fn moves_by_grapheme_and_word() {
        let buffer = buffer("ab 👍🏽 cd", 0);
        assert_eq!(buffer.next_boundary(3), 3 + "👍🏽".len());
        assert_eq!(buffer.previous_boundary(3 + "👍🏽".len()), 3);
        assert_eq!(buffer.next_word(0), 2);
        assert_eq!(
            buffer.previous_word(buffer.text.len()),
            buffer.text.len() - 2
        );
    }

    #[test]
    fn finds_the_word_and_paragraph_around_an_offset() {
        let buffer = buffer("one two\nthree", 0);
        assert_eq!(buffer.word_at(5), 4..7);
        assert_eq!(buffer.paragraph_at(5), 0..7);
        assert_eq!(buffer.paragraph_at(9), 8..13);
        assert_eq!(buffer.paragraph_at(8), 8..13);
    }

    #[test]
    fn replacing_puts_the_cursor_after_the_new_text() {
        let mut buffer = buffer("hello", 0);
        buffer.selected = 1..4;
        buffer.replace(buffer.edited_range(), "ipp");
        assert_eq!(buffer.text, "hippo");
        assert_eq!(buffer.selected, 4..4);
    }

    #[test]
    fn converts_offsets_to_and_from_utf16() {
        let buffer = buffer("a😀b", 0);
        assert_eq!(buffer.offset_to_utf16(5), 3);
        assert_eq!(buffer.offset_from_utf16(3), 5);
        assert_eq!(buffer.range_from_utf16(&(1..3)), 1..5);
    }
}
