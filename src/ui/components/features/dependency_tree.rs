use ratatui::{
    style::Stylize,
    text::{Line, Span},
};

use crate::domain::TreeNode;
use crate::ui::components::shared::tree_view::TreeRow;

impl TreeRow for TreeNode {
    fn depth(&self) -> usize {
        self.depth
    }

    fn key(&self) -> &str {
        &self.name
    }

    /// The name, bold for the project and its direct dependencies, with its extras and version;
    /// then why it is here, dim; then, yellow, the newer version when there is one.
    fn line(&self) -> Line<'static> {
        let mut name = self.name.clone();
        if !self.extras.is_empty() {
            name += &format!("[{}]", self.extras.join(","));
        }
        let mut spans = vec![
            if self.depth <= 1 {
                Span::raw(name).bold()
            } else {
                Span::raw(name)
            },
            format!(" {}", self.version).into(),
        ];
        spans.extend(
            self.notes
                .iter()
                .map(|note| format!("  {note}").dark_gray()),
        );
        if self.repeated {
            spans.push("  (*)".dark_gray());
        }
        if let Some(latest) = &self.latest {
            spans.push(format!("  latest: {latest}").yellow());
        }
        Line::from(spans)
    }
}
