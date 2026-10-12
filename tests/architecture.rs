//! Architecture fitness tests: the layering rules from `AGENTS.md` and
//! `docs/ARCHITECTURE.md`, checked on every `cargo test`.
//!
//! ponytail: matching is textual, so a path hidden inside a macro or behind a glob import
//! can slip through. Parse with `syn` if that ever matters.

use std::fs;
use std::path::{Path, PathBuf};

struct Rule {
    name: &'static str,
    /// This `.rs` file, or every one under this directory, is checked. It must exist, so a
    /// module move cannot silently turn a rule into a no-op.
    scope: &'static str,
    forbidden: &'static [&'static str],
    source: &'static str,
}

const RULES: &[Rule] = &[
    Rule {
        name: "controllers-no-ratatui",
        scope: "src/controllers",
        forbidden: &["ratatui"],
        source: "ARCHITECTURE.md §2",
    },
    Rule {
        name: "controllers-no-process",
        scope: "src/controllers",
        forbidden: &["process::Command"],
        source: "ADR-0002",
    },
    Rule {
        name: "uv-bottom-layer",
        scope: "src/uv",
        forbidden: &[
            "ratatui",
            "controllers",
            "crate::ui",
            "crate::runtime",
            "crate::message",
        ],
        source: "ARCHITECTURE.md §2, ADR-0002",
    },
    Rule {
        name: "runtime-no-ui",
        scope: "src/runtime",
        forbidden: &["ratatui", "crate::ui"],
        source: "ARCHITECTURE.md §2, ADR-0001",
    },
    Rule {
        name: "state-data-only",
        scope: "src/state.rs",
        forbidden: &["ratatui", "crate::ui", "crate::runtime", "controllers"],
        source: "ARCHITECTURE.md §2, ADR-0004",
    },
    Rule {
        name: "shared-no-domain",
        scope: "src/ui/components/shared",
        forbidden: &["PyprojectDependency", "EnvDependency", "controllers"],
        source: "ARCHITECTURE.md §6",
    },
    Rule {
        name: "ui-no-spawn",
        scope: "src/ui",
        forbidden: &[
            "tokio::spawn",
            ".spawn(",
            "controllers::",
            "uv::",
            "process::Command",
        ],
        source: "ARCHITECTURE.md §2, ADR-0001, ADR-0002",
    },
    Rule {
        name: "no-working-directory",
        scope: "src",
        forbidden: &["set_current_dir", "\"./"],
        source: "ADR-0011",
    },
    Rule {
        name: "no-tests-in-src",
        scope: "src",
        forbidden: &["#[test]", "#[cfg(test)]"],
        source: "AGENTS.md, Testing",
    },
];

/// Known violations as `(file, rule, issue that removes it)`. The list can only shrink:
/// an entry that no longer matches a violation fails the test until it is deleted.
const ALLOWED: &[(&str, &str, &str)] = &[];

#[test]
fn architecture_rules_hold() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut failures = Vec::new();
    let mut used = vec![false; ALLOWED.len()];

    for rule in RULES {
        let scope = root.join(rule.scope);
        let files = if scope.is_file() {
            vec![scope]
        } else if scope.is_dir() {
            rust_files(&scope)
        } else {
            failures.push(format!(
                "rule `{}`: scope `{}` does not exist, update RULES",
                rule.name, rule.scope
            ));
            continue;
        };
        for file in files {
            let path = file
                .strip_prefix(root)
                .unwrap_or(&file)
                .to_string_lossy()
                .replace('\\', "/");
            let text = match fs::read_to_string(&file) {
                Ok(text) => text,
                Err(e) => {
                    failures.push(format!("{path}: {e}"));
                    continue;
                }
            };
            let hits: Vec<String> = text
                .lines()
                .enumerate()
                .filter(|(_, l)| !l.trim_start().starts_with("//"))
                .filter(|(_, l)| rule.forbidden.iter().any(|f| l.contains(f)))
                .map(|(i, l)| format!("  {path}:{}: {}", i + 1, l.trim()))
                .collect();
            if hits.is_empty() {
                continue;
            }
            match ALLOWED
                .iter()
                .position(|(p, r, _)| *p == path && *r == rule.name)
            {
                Some(i) => used[i] = true,
                None => failures.push(format!(
                    "rule `{}` ({}) broken:\n{}",
                    rule.name,
                    rule.source,
                    hits.join("\n")
                )),
            }
        }
    }

    for ((path, rule, issue), used) in ALLOWED.iter().zip(used) {
        if !used {
            failures.push(format!(
                "{path} no longer breaks `{rule}` ({issue}): delete its ALLOWED entry"
            ));
        }
    }

    assert!(failures.is_empty(), "\n{}\n", failures.join("\n\n"));
}

fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}
