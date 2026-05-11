use super::MyApp;
use soromantic_core::db::LibraryItem;

impl MyApp {
    pub fn toggle_selection(&mut self, id: i64) {
        if self.nav.selected_ids.contains(&id) {
            self.nav.selected_ids.remove(&id);
        } else {
            self.nav.selected_ids.insert(id);
        }
    }

    pub fn select_range(&mut self, start_idx: usize, end_idx: usize, items: &[LibraryItem]) {
        self.nav.selected_ids.clear();
        let start = start_idx.min(end_idx);
        let end = start_idx.max(end_idx);
        for i in start..=end {
            if let Some(item) = items.get(i) {
                self.nav.selected_ids.insert(item.id);
            }
        }
    }

    pub fn select_all(&mut self, items: &[LibraryItem]) {
        self.nav.selected_ids.clear();
        for item in items {
            self.nav.selected_ids.insert(item.id);
        }
    }

    pub fn invert_selection(&mut self, items: &[LibraryItem]) {
        for item in items {
            if self.nav.selected_ids.contains(&item.id) {
                self.nav.selected_ids.remove(&item.id);
            } else {
                self.nav.selected_ids.insert(item.id);
            }
        }
    }
}
