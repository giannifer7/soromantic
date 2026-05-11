use crate::state::FocusScope;
use crate::ui::grid::GridAction;

pub enum Action {
    Grid(GridAction),
    Search(String),
    Back,
    SetFocusScope(FocusScope),
    NavTo(i64),
    HeaderButton(crate::state::HeaderAction), // 0=Back, 1=Play
}
