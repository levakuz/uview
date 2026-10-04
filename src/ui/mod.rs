pub mod components;
pub mod keymap;
pub mod pages;

use ratatui::Frame;
use ratatui::layout::Rect;

use crate::message::{Action, Command};
use crate::state::AppState;

/// Common interface for all UI components (pages, widgets). A component reads what the app
/// knows from `AppState` and keeps only its own view state. `&mut self` is for that view state,
/// which ratatui's `TableState` needs mutable during rendering.
pub trait Component {
    fn render(&mut self, frame: &mut Frame, area: Rect, state: &AppState);
    /// Applies `action` and returns the effects it calls for, without performing them. `state`
    /// is read only: the app changes it, from the runtime's events.
    fn update(&mut self, action: &Action, state: &AppState) -> Vec<Command>;
}
