pub mod components;
pub mod pages;

use ratatui::Frame;
use ratatui::layout::Rect;

use crate::events::Action;

/// Common interface for all UI components (pages, widgets).
/// Uses `&mut self` because ratatui's TableState requires mutable access during render.
pub trait Component {
    fn render(&mut self, frame: &mut Frame, area: Rect);
    fn handle_event(&mut self, action: &Action) -> Option<Action>;
}
