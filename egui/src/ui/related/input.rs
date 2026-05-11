use super::actions::Action;
use crate::state::{FocusScope, MyApp};
use eframe::egui;
use soromantic_core::db::LibraryItem;

pub fn handle_global_shortcuts(
    ctx: &egui::Context,
    app: &MyApp,
    actions: &mut Vec<Action>,
) -> bool {
    let typing = ctx.wants_keyboard_input();
    if !typing && ctx.input(|i| i.key_pressed(egui::Key::Tab)) {
        let next_scope = match app.nav.focus_scope {
            FocusScope::Grid => FocusScope::NavList,
            FocusScope::NavList => FocusScope::Search,
            FocusScope::Search => FocusScope::HeaderButtons,
            FocusScope::HeaderButtons => FocusScope::Footer,
            FocusScope::Footer => FocusScope::Grid,
        };
        actions.push(Action::SetFocusScope(next_scope));
        return true;
    }
    false
}

pub fn handle_nav_list_input(
    ctx: &egui::Context,
    app: &MyApp,
    nav_items: &[LibraryItem],
    current_nav_id: i64,
    actions: &mut Vec<Action>,
) {
    let typing = ctx.wants_keyboard_input();
    if !typing && app.nav.focus_scope == FocusScope::NavList && !nav_items.is_empty() {
        let current_idx = nav_items.iter().position(|i| i.id == current_nav_id);
        let move_delta = if ctx
            .input(|i| i.key_pressed(egui::Key::ArrowDown) || i.key_pressed(egui::Key::J))
        {
            Some(1isize)
        } else if ctx.input(|i| i.key_pressed(egui::Key::ArrowUp) || i.key_pressed(egui::Key::K)) {
            Some(-1isize)
        } else {
            None
        };

        if let Some(delta) = move_delta {
            let new_idx = current_idx.map_or(0, |idx| {
                idx.saturating_add_signed(delta)
                    .min(nav_items.len().saturating_sub(1))
            });
            if let Some(item) = nav_items.get(new_idx) {
                actions.push(Action::NavTo(item.id));
            }
        }
    }
}

pub fn handle_header_buttons_input(
    ctx: &egui::Context,
    app: &mut MyApp,
    actions: &mut Vec<Action>,
) {
    // HeaderButtons: Back, Play
    let typing = ctx.wants_keyboard_input();
    if !typing && app.nav.focus_scope == FocusScope::HeaderButtons {
        if ctx.input(|i| i.key_pressed(egui::Key::ArrowRight) || i.key_pressed(egui::Key::L)) {
            app.nav.header_focus = app.nav.header_focus.next();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::ArrowLeft) || i.key_pressed(egui::Key::H)) {
            app.nav.header_focus = app.nav.header_focus.prev();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
            actions.push(Action::HeaderButton(app.nav.header_focus));
        }
    }
}
