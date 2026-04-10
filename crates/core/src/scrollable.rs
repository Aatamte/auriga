use std::ops::Range;

use orchestrator_types::ScrollDirection;

#[derive(Debug)]
pub struct Scrollable {
    pub offset: usize,
    pub selected: Option<usize>,
    item_count: usize,
    visible_height: usize,
}

impl Scrollable {
    pub fn new() -> Self {
        Self {
            offset: 0,
            selected: None,
            item_count: 0,
            visible_height: 0,
        }
    }

    pub fn set_item_count(&mut self, count: usize) {
        self.item_count = count;
        if let Some(sel) = self.selected {
            if sel >= count && count > 0 {
                self.selected = Some(count - 1);
            } else if count == 0 {
                self.selected = None;
            }
        }
    }

    pub fn set_visible_height(&mut self, height: usize) {
        self.visible_height = height;
    }

    pub fn scroll(&mut self, direction: ScrollDirection) {
        match direction {
            ScrollDirection::Up => {
                self.offset = self.offset.saturating_sub(1);
            }
            ScrollDirection::Down => {
                let max_offset = self.item_count.saturating_sub(self.visible_height);
                if self.offset < max_offset {
                    self.offset += 1;
                }
            }
        }
    }

    pub fn select(&mut self, idx: usize) {
        if idx < self.item_count {
            self.selected = Some(idx);
            self.ensure_visible();
        }
    }

    pub fn select_next(&mut self) {
        let next = match self.selected {
            Some(s) if s + 1 < self.item_count => s + 1,
            None if self.item_count > 0 => 0,
            _ => return,
        };
        self.selected = Some(next);
        self.ensure_visible();
    }

    pub fn select_prev(&mut self) {
        let prev = match self.selected {
            Some(0) | None => return,
            Some(s) => s - 1,
        };
        self.selected = Some(prev);
        self.ensure_visible();
    }

    pub fn ensure_visible(&mut self) {
        if let Some(sel) = self.selected {
            if sel < self.offset {
                self.offset = sel;
            } else if self.visible_height > 0 && sel >= self.offset + self.visible_height {
                self.offset = sel - self.visible_height + 1;
            }
        }
    }

    pub fn visible_range(&self) -> Range<usize> {
        let end = (self.offset + self.visible_height).min(self.item_count);
        self.offset..end
    }

    pub fn can_scroll_up(&self) -> bool {
        self.offset > 0
    }

    pub fn can_scroll_down(&self) -> bool {
        self.offset + self.visible_height < self.item_count
    }
}

impl Default for Scrollable {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scroll_down_advances_offset() {
        let mut s = Scrollable::new();
        s.set_item_count(20);
        s.set_visible_height(5);
        s.scroll(ScrollDirection::Down);
        assert_eq!(s.offset, 1);
    }

    #[test]
    fn scroll_up_at_zero_stays_zero() {
        let mut s = Scrollable::new();
        s.set_item_count(20);
        s.set_visible_height(5);
        s.scroll(ScrollDirection::Up);
        assert_eq!(s.offset, 0);
    }

    #[test]
    fn scroll_down_stops_at_max() {
        let mut s = Scrollable::new();
        s.set_item_count(5);
        s.set_visible_height(5);
        s.scroll(ScrollDirection::Down);
        assert_eq!(s.offset, 0);
    }

    #[test]
    fn select_next_wraps_correctly() {
        let mut s = Scrollable::new();
        s.set_item_count(3);
        s.set_visible_height(3);
        s.select_next();
        assert_eq!(s.selected, Some(0));
        s.select_next();
        assert_eq!(s.selected, Some(1));
        s.select_next();
        assert_eq!(s.selected, Some(2));
        s.select_next();
        assert_eq!(s.selected, Some(2));
    }

    #[test]
    fn select_prev_stops_at_zero() {
        let mut s = Scrollable::new();
        s.set_item_count(3);
        s.set_visible_height(3);
        s.select(1);
        s.select_prev();
        assert_eq!(s.selected, Some(0));
        s.select_prev();
        assert_eq!(s.selected, Some(0));
    }

    #[test]
    fn visible_range_correct() {
        let mut s = Scrollable::new();
        s.set_item_count(20);
        s.set_visible_height(5);
        s.offset = 3;
        assert_eq!(s.visible_range(), 3..8);
    }

    #[test]
    fn set_item_count_clamps_selection() {
        let mut s = Scrollable::new();
        s.set_item_count(5);
        s.select(4);
        s.set_item_count(2);
        assert_eq!(s.selected, Some(1));
    }

    #[test]
    fn set_item_count_to_zero_clears_selection() {
        let mut s = Scrollable::new();
        s.set_item_count(5);
        s.select(2);
        s.set_item_count(0);
        assert_eq!(s.selected, None);
    }
}
