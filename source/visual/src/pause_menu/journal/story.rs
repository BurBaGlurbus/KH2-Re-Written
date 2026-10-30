//! Story (state 3, from Notes; FUN_1403291c0 / FUN_140328b80): the
//! world's reached story entries as one continuous column of text, scrolled
//! by line. The entries are the `stor` rows of the world whose story flag
//! is set (show-all does not widen the filter); the newest entry is at the
//! bottom and the page opens on it.
use super::*;

pub(super) const STATE: u8 = 3;
/// The panel (ctx (88, 100), pad 26) and its scroll track.
const PANEL: usize = 96;
const TRACK: usize = 100;
const BLANK: usize = 2;
/// NEW labels: pulsing, then static once the entry has reached the top.
const MARK: [usize; 2] = [57, 58];
const PRIO_PANEL: u32 = 0x755c;
const PRIO_NEW: u32 = 0x755e;
const LINES: usize = 10;
const PITCH: f32 = 26.;
/// The extra line gap: pad (26) − font 3 spacing − size (18).
const LINE_EXTRA: f32 = 6.;
const HELP: u16 = 0x48d7;

/// One listed entry: its `stor` row and its lines.
struct Entry {
    row: usize,
    lines: Vec<Vec<u8>>,
}

#[derive(Default)]
pub(super) struct Page {
    entries: Vec<Entry>,
    marks: Vec<Id>,
    text: Option<Id>,
    /// The `stor` rows seen so far and when the journal opened (save+0x4c38
    /// + 0xac).
    pub seen: HashSet<u16>,
    pub seen_at_open: HashSet<u16>,
}

/// FUN_1403ae5c0: the lines of a message (a final newline before the end
/// does not add one).
fn lines_of(text: &[u8]) -> Vec<Vec<u8>> {
    let mut lines: Vec<Vec<u8>> = text.split(|&b| b == 0x02).map(<[u8]>::to_vec).collect();
    if lines.len() > 1 && lines.last().is_some_and(Vec::is_empty) {
        lines.pop();
    }
    lines
}

impl Journal {
    /// The visible lines from the scroll line, and each entry's first line.
    fn story_window(&self) -> (Vec<u8>, Vec<(usize, usize)>) {
        let scroll = self.list.cursor;
        let mut flat: Vec<&[u8]> = Vec::new();
        let mut starts = Vec::new();
        for (k, e) in self.story.entries.iter().enumerate() {
            starts.push((k, flat.len()));
            flat.extend(e.lines.iter().map(Vec::as_slice));
        }
        let end = (scroll + LINES).min(flat.len());
        let shown = flat.get(scroll..end).unwrap_or(&[]).join(&0x02u8);
        (shown, starts)
    }

    /// FUN_140328d60 and FUN_140329790: the text window and the marks.
    fn story_view(&mut self, seq: &Sequence) {
        let scroll = self.list.cursor;
        self.bar_to(seq, scroll);
        let (shown, starts) = self.story_window();
        self.set_text(self.story.text, shown);
        let marks = self.story.marks.clone();
        for &m in &marks {
            self.sprites.set(seq, m, BLANK, None);
        }
        for (k, first) in starts {
            let e = &self.story.entries[k];
            let (row, len) = (e.row as u16, e.lines.len());
            // The entry whose first line is the top row is seen.
            if first == scroll {
                self.story.seen.insert(row);
            }
            let off = first as i64 - scroll as i64;
            if off >= LINES as i64 || off + len as i64 <= 0 || off < 0 {
                continue;
            }
            if !self.story.seen_at_open.contains(&row)
                && let Some(&m) = marks.get(off as usize)
            {
                let seen = self.story.seen.contains(&row);
                self.sprites.set(seq, m, MARK[seen as usize], None);
            }
        }
    }

    /// FUN_1403291c0.
    pub(super) fn build_story(&mut self, layout_: &Layout) {
        let seq = &layout_.sequences[SEQUENCE];
        self.reset_list();
        self.no_cursor = true;
        self.cat.detail = false;
        self.category_frame(seq);
        let c = self.container;
        let panel = self.page_sprite(seq, PRIO_PANEL, PANEL, c, None);
        let track = self.page_sprite(seq, PRIO_PANEL, TRACK, c, None);
        let mut t = Self::text(Vec::new(), Self::size(seq, PANEL));
        t.group_position = false;
        t.keep_rgb = false;
        t.extra = Some(LINE_EXTRA);
        let text = self.page_sprite(seq, BASE, BLANK, panel, Some(t));
        self.story.text = Some(text);
        self.story.marks.clear();
        for i in 0..LINES {
            let m = self.page_sprite(seq, PRIO_NEW, BLANK, panel, None);
            self.sprites.get_mut(m).offset = [0., PITCH * i as f32];
            self.story.marks.push(m);
        }
        // The `stor` rows of the world whose flag is set.
        self.story.entries = self
            .data
            .stories
            .iter()
            .enumerate()
            .filter(|(_, s)| s.world as i32 == self.world && self.flag(s.flag))
            .map(|(k, s)| Entry {
                row: k,
                lines: lines_of(&self.msg(s.entry)),
            })
            .collect();
        let n = self.story.entries.len();
        let (before, last) = match self.story.entries.split_last() {
            Some((last, rest)) => (
                rest.iter().map(|e| e.lines.len()).sum::<usize>(),
                last.lines.len(),
            ),
            None => (0, 0),
        };
        let total = if n == 0 {
            1
        } else {
            before + if last > LINES { last - LINES + 1 } else { 1 }
        };
        // FUN_1402fa930: total L0 + max(10, the newest entry's lines).
        self.bar_in(seq, track, before + last.max(LINES), LINES);
        // The page opens on the first line of the newest entry.
        self.list = MenuList::new(vec![panel], before.min(total - 1));
        self.list.total = total;
        self.list.mask = 0x63;
        if total > 1 {
            self.list.page = Some(LINES);
            self.list.mask |= 0x3000;
        }
        self.story_view(seq);
        self.show_help(seq, HELP);
    }

    /// FUN_140328b80 (not leaving).
    pub(super) fn story_page(
        &mut self,
        layout_: &Layout,
        pad: &Pad,
        sounds: &mut Vec<u32>,
    ) -> bool {
        let seq = &layout_.sequences[SEQUENCE];
        let (input, moved) = self.input(pad);
        if self.substate == 2 {
            self.leave(layout_, input, moved, false);
            return false;
        }
        self.settle(seq, input, moved, true);
        if let Some(s) = sound(input, moved, true) {
            sounds.push(s);
        }
        if moved && self.substate == 1 {
            self.story_view(seq);
        }
        match input {
            In::Start => self.exit = true,
            In::Cancel => {
                self.pill_out(seq);
                self.hide_help(seq);
                self.slide(seq, 1, 1);
                self.pending = Some(NOTES_PAGE);
                self.substate = 2;
            }
            _ => {}
        }
        moved
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_trailing_newline_adds_no_line() {
        assert_eq!(lines_of(b"a\x02b").len(), 2);
        assert_eq!(lines_of(b"a\x02b\x02").len(), 2);
        assert_eq!(lines_of(b"a\x02\x02").len(), 2);
        assert_eq!(lines_of(b"").len(), 1);
    }
}
