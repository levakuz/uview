use crate::domain::{EnvDependency, PyprojectDependency};
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
            shorten(
                &self
                    .groups
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>(),
            ),
            self.version_constraint.clone(),
        ]
    }

    fn key(&self) -> &str {
        &self.name
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
