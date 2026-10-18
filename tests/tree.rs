//! The tree view, on rows of its own, and how a dependency reads in it.

use ratatui::style::{Color, Modifier};
use ratatui::text::Line;
use uview::domain::TreeNode;
use uview::ui::components::shared::tree_view::{TreeRow, TreeView};

/// A row named `key` at `depth`.
struct Row(usize, &'static str);

impl TreeRow for Row {
    fn depth(&self) -> usize {
        self.0
    }
    fn key(&self) -> &str {
        self.1
    }
    fn line(&self) -> Line<'static> {
        Line::raw(self.1)
    }
}

/// root ─ a ─ a1, a2; root ─ b ─ b1; root ─ c
fn rows() -> Vec<Row> {
    vec![
        Row(0, "root"),
        Row(1, "a"),
        Row(2, "a1"),
        Row(2, "a2"),
        Row(1, "b"),
        Row(2, "b1"),
        Row(1, "c"),
    ]
}

fn shown(view: &TreeView, rows: &[Row]) -> Vec<&'static str> {
    view.visible(rows).iter().map(|&i| rows[i].1).collect()
}

#[test]
fn a_tree_opens_unfolded_and_folds_one_row_or_all() {
    let rows = rows();
    let mut view = TreeView::default();
    assert_eq!(
        shown(&view, &rows),
        ["root", "a", "a1", "a2", "b", "b1", "c"]
    );
    // Down to a, fold it: its children go.
    view.down(&rows);
    view.collapse(&rows);
    assert_eq!(shown(&view, &rows), ["root", "a", "b", "b1", "c"]);
    // Folded already: ← goes to the parent.
    view.collapse(&rows);
    assert_eq!(view.selected(&rows), Some(0));
    // All folded below the roots, then all unfolded.
    view.collapse_all(&rows);
    assert_eq!(shown(&view, &rows), ["root", "a", "b", "c"]);
    view.expand_all();
    assert_eq!(shown(&view, &rows).len(), 7);
    // → unfolds the highlighted row again.
    view.down(&rows);
    view.collapse(&rows);
    view.expand(&rows);
    assert_eq!(shown(&view, &rows).len(), 7);
}

#[test]
fn the_highlight_moves_over_shown_rows_and_keeps_its_row_across_a_reload() {
    let rows = rows();
    let mut view = TreeView::default();
    // a2, a leaf: ← goes to its parent, a.
    for _ in 0..3 {
        view.down(&rows);
    }
    assert_eq!(view.selected(&rows), Some(3));
    view.collapse(&rows);
    assert_eq!(view.selected(&rows), Some(1));
    // At the ends it stays.
    view.up(&rows);
    view.up(&rows);
    assert_eq!(view.selected(&rows), Some(0));
    // A reload adds a row above: the highlight stays on b by its path.
    view.down(&rows);
    view.down(&rows);
    view.down(&rows);
    view.down(&rows);
    assert_eq!(rows[view.selected(&rows).unwrap_or(0)].1, "b");
    let reloaded = vec![
        Row(0, "root"),
        Row(1, "new"),
        Row(1, "a"),
        Row(2, "a1"),
        Row(1, "b"),
    ];
    assert_eq!(view.selected(&reloaded), Some(4));
    // Folding all hides a highlighted grandchild: the highlight goes to the shown row above.
    let mut view = TreeView::default();
    for _ in 0..5 {
        view.down(&rows);
    }
    view.collapse_all(&rows);
    assert!(
        view.visible(&rows)
            .contains(&view.selected(&rows).unwrap_or(99))
    );
}

#[test]
fn a_dependency_reads_as_uv_tree_writes_it_marked_for_what_matters() {
    let node = |depth: usize| TreeNode {
        depth,
        name: "httpx".into(),
        version: "0.27.0".into(),
        extras: vec!["http2".into()],
        notes: vec!["group: dev".into()],
        latest: Some("0.28.1".into()),
        repeated: true,
    };
    let line = node(1).line();
    let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(text, "httpx[http2] 0.27.0  group: dev  (*)  latest: 0.28.1");
    // A direct dependency is bold; deeper ones are not.
    assert!(line.spans[0].style.add_modifier.contains(Modifier::BOLD));
    assert!(
        !node(2).line().spans[0]
            .style
            .add_modifier
            .contains(Modifier::BOLD)
    );
    // The note and the repeat are dim, the newer version yellow.
    assert_eq!(line.spans[2].style.fg, Some(Color::DarkGray));
    assert_eq!(line.spans[3].style.fg, Some(Color::DarkGray));
    assert_eq!(line.spans[4].style.fg, Some(Color::Yellow));
}

#[test]
fn the_highlighted_row_has_no_colours_of_its_own_to_turn_into_blocks()
-> Result<(), Box<dyn std::error::Error>> {
    use ratatui::{Terminal, backend::TestBackend};
    let node = |depth: usize, name: &str, notes: &[&str]| TreeNode {
        depth,
        name: name.into(),
        version: "1.0".into(),
        notes: notes.iter().map(ToString::to_string).collect(),
        ..TreeNode::default()
    };
    let rows = vec![
        node(0, "acme", &[]),
        node(1, "mypy", &[]),
        node(2, "librt", &["group: dev"]),
    ];
    let mut view = TreeView::default();
    view.down(&rows);
    view.down(&rows);
    let mut terminal = Terminal::new(TestBackend::new(40, 5))?;
    terminal.draw(|frame| {
        view.render(
            frame,
            frame.area(),
            &rows,
            Line::raw("Tree"),
            Default::default(),
        );
    })?;
    let buffer = terminal.backend().buffer();
    // The highlighted row, librt: the marker before it and its dim note are reversed alike, with
    // no grey of their own.
    let row = 3;
    for x in 1..38 {
        assert_ne!(buffer[(x, row)].fg, Color::DarkGray, "x = {x}");
        assert_ne!(buffer[(x, row)].bg, Color::DarkGray, "x = {x}");
    }
    assert!(buffer[(5, row)].modifier.contains(Modifier::REVERSED));
    // Another row keeps its colours.
    assert_eq!(buffer[(3, 2)].fg, Color::DarkGray);
    Ok(())
}
