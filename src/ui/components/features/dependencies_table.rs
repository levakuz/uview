use crate::domain::{EnvDependency, Membership, PyprojectDependency};
use crate::ui::components::shared::table::TableRow;

impl TableRow for PyprojectDependency {
    fn columns() -> Vec<&'static str> {
        vec!["Name", "Group", "Constraint"]
    }

    fn cells(&self) -> Vec<String> {
        // Extras follow the name as they would in the requirement, e.g. `flask[dotenv]`.
        let name = if self.extras.is_empty() {
            self.name.clone()
        } else {
            format!("{}[{}]", self.name, shorten(&self.extras))
        };
        vec![
            name,
            format_groups(&self.groups),
            self.version_constraint.clone(),
        ]
    }

    fn key(&self) -> &str {
        &self.name
    }

    /// The groups, however many, so the columns do not shift as packages join groups.
    fn fixed_width(column: usize) -> Option<u16> {
        (column == 1).then_some(GROUP_WIDTH as u16)
    }
}

const GROUP_WIDTH: usize = 20;

/// The first two groups, then `...` for the rest, cut to the column's width.
fn format_groups(groups: &[Membership]) -> String {
    let mut shown: Vec<String> = groups.iter().take(2).map(ToString::to_string).collect();
    if groups.len() > 2 {
        shown.push("...".into());
    }
    let shown = shown.join(", ");
    if shown.chars().count() <= GROUP_WIDTH {
        shown
    } else {
        format!(
            "{}...",
            shown.chars().take(GROUP_WIDTH - 3).collect::<String>()
        )
    }
}

/// `items` joined by commas, cut short with `...` past 20 characters.
fn shorten(items: &[String]) -> String {
    const MAX_LEN: usize = 20;
    let mut result = String::new();
    for (i, g) in items.iter().enumerate() {
        let candidate = if i == 0 {
            g.clone()
        } else {
            format!("{}, {}", result, g)
        };
        if candidate.len() > MAX_LEN {
            return if result.is_empty() {
                format!("{}...", g.chars().take(MAX_LEN - 3).collect::<String>())
            } else {
                format!("{}, ...", result)
            };
        }
        result = candidate;
    }
    result
}

impl TableRow for EnvDependency {
    fn columns() -> Vec<&'static str> {
        vec!["Name", "Version"]
    }

    fn cells(&self) -> Vec<String> {
        vec![self.name.clone(), self.version.clone()]
    }

    fn key(&self) -> &str {
        &self.name
    }
}
