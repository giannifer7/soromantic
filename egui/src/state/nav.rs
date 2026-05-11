//! Navigation state: view mode, page stack, search, focus, selection.

use super::{FocusScope, FooterAction, HeaderAction, ViewMode};
use parking_lot::Mutex;
use soromantic_core::db::{LibraryItem, PageData};
use std::collections::HashSet;
use std::sync::Arc;

/// State for navigation, view mode, selection, and browser/search.
#[derive(Debug)]
pub struct NavigationState {
    /// Current view mode (Library, Related(id), Models, Studios, etc.)
    pub view_mode: ViewMode,
    /// Which UI section has keyboard focus.
    pub focus_scope: FocusScope,
    /// The currently active related/performer page data.
    pub active_page: Option<PageData>,
    /// Back-navigation stack of previously visited pages.
    pub navigation_stack: Vec<PageData>,
    /// Current search query text.
    pub browser_search: String,
    /// Search results (pending or cached).
    pub browser_results: Vec<LibraryItem>,
    /// Currently focused item index in the grid or list.
    pub focused_index: Option<usize>,
    /// Last clicked item index (anchor for Shift+Click range selection).
    pub last_clicked_index: Option<usize>,
    /// Set of currently selected item IDs.
    pub selected_ids: HashSet<i64>,
    /// Which footer button has focus.
    pub footer_focus: FooterAction,
    /// Which header button has focus (Related view).
    pub header_focus: HeaderAction,
    /// Pending page data from async refresh.
    pub pending_page_data: Arc<Mutex<Option<PageData>>>,
    /// Pending search results from async search.
    pub pending_search_results: Arc<Mutex<Vec<LibraryItem>>>,
}
