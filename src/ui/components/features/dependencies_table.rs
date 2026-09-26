use crate::events::{EnvDependency, PyprojectDependency};
use crate::ui::components::shared::table::TableRow;

impl TableRow for PyprojectDependency {
    fn columns() -> Vec<&'static str> {
        vec!["Name", "Group", "Version Constraint"]
    }

    fn cells(&self) -> Vec<String> {
        vec![
            self.name.clone(),
            format_groups(&self.groups),
            self.version_constraint.clone(),
        ]
    }
}

fn format_groups(groups: &[String]) -> String {
    const MAX_LEN: usize = 20;
    let mut result = String::new();
    for (i, g) in groups.iter().enumerate() {
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
}
