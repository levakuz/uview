pub mod components;
pub mod pages;

use ratatui::Frame;
use ratatui::layout::Rect;

use crate::message::{Action, Command};

/// Common interface for all UI components (pages, widgets).
/// Uses `&mut self` because ratatui's TableState requires mutable access during render.
pub trait Component {
    fn render(&mut self, frame: &mut Frame, area: Rect);
    /// Applies `action` and returns the effects it calls for, without performing them.
    fn update(&mut self, action: &Action) -> Vec<Command>;
}
