use crate::{
    domain::{
        EnvDependency, Membership, PyprojectDependency, Source, TreeNode, bare_name, normalize_name,
    },
    message::{Action, Command, DepsAction, NavAction},
    state::{DepsState, Loadable},
    ui::components::{
        features::{
            add_from_source::AddFromSource,
            bump_popup::BumpPopup,
            groups_popup::GroupsPopup,
            indexes_popup::IndexesPopup,
            input_popup::InputPopup,
            package_control_popup::{PackageControlOption, PackageControlPopup},
            search_popup::{SearchPopup, Searched},
            version_picker::{First, Picked, VersionPicker},
        },
        shared::{list_popup::ListPopup, table::SharedTable, tree_view::TreeView},
    },
    ui::keymap::Mode,
};
use ratatui::{
    Frame,
    layout::Rect,
    style::Stylize,
    text::{Line, Text},
};
#[derive(PartialEq)]
enum ActiveTable {
    Pyproject,
    Environment,
    Tree,
}

/// The two dependency tables behind tabs, and the popups acting on them. It reads the rows from
/// `DepsState` and keeps only which tab, row and popup are showing.
pub struct DependenciesTablesTabs {
    pyproject_table: SharedTable<PyprojectDependency>,
    env_table: SharedTable<EnvDependency>,
    active_table: ActiveTable,
    package_control_popup: PackageControlPopup,
    show_package_control_popup: bool,
    /// The package the actions list was opened for. Its actions use this, not the selection
    /// when they run, which a reload can move to another row.
    target: Option<String>,
    /// The popups with an input are shown exactly while they are being typed into.
    package_add_popup: InputPopup,
    pin_version_popup: InputPopup,
    new_group_popup: InputPopup,
    /// The questions `P` asks before its group is chosen.
    add_from_source: AddFromSource,
    /// The project's indexes, on `i`.
    indexes_popup: IndexesPopup,
    /// `g`: the project's dependency groups.
    groups_popup: GroupsPopup,
    /// `/`: a package looked up by name.
    search_popup: SearchPopup,
    /// `b`'s bumps of a declared package.
    bump_popup: BumpPopup,
    /// The Tree tab's view of the dependency tree, and of what pulls a package in.
    tree_view: TreeView,
    why_view: TreeView,
    /// The package whose "why" the Tree tab shows in place of the tree, when it does.
    why_of: Option<String>,
    /// The list of versions, for Pin to version or for a package being added.
    version_picker: VersionPicker,
    /// The requirement typed into the add input, while its version is chosen; `None` when the
    /// list is pinning.
    adding_version_of: Option<String>,
    /// What waits on a group to be chosen.
    pending: Option<Pending>,
    /// The groups `pending` can be done to; past them, a new group.
    group_popup: Option<(ListPopup, Vec<Membership>)>,
    /// The groups a removal or move was sent from. Once a reload shows one empty, its deletion
    /// is offered, once: a group that was empty before is never asked about.
    emptied: Vec<String>,
    /// Whether to delete the emptied group it names.
    delete_question: Option<(ListPopup, String)>,
}

/// An action that runs once its group is chosen.
enum Pending {
    /// Add the requirement: typed into the add input, or a declared package's own.
    Add(String),
    /// Add a package from somewhere other than the default index.
    AddFrom(Source),
    /// Add the package's requirement to the chosen group, then remove it from `from`.
    Move {
        name: String,
        requirement: String,
        from: Membership,
    },
    /// Remove the package from the chosen group only.
    RemoveFrom(String),
}

impl DependenciesTablesTabs {
    pub fn new() -> Self {
        let package_control_popup = PackageControlPopup::new(0);
        let package_add_popup = InputPopup::new("Package (e.g. requests, flask[dotenv]>=3.0)", 50);
        let pin_version_popup = InputPopup::new("Version (e.g. ==1.2.3, >=2.0, <3.0)", 50);
        let new_group_popup = InputPopup::new("New group (e.g. dev, test)", 50);
        // Only the active table is drawn, so both are styled as focused. The tabs are the title
        // of its border.
        let mut pyproject_table = SharedTable::new();
        pyproject_table.set_focused(true);
        let mut env_table = SharedTable::new();
        env_table.set_focused(true);

        Self {
            pyproject_table,
            env_table,
            active_table: ActiveTable::Pyproject,
            package_control_popup,
            show_package_control_popup: false,
            target: None,
            package_add_popup,
            pin_version_popup,
            new_group_popup,
            add_from_source: AddFromSource::default(),
            indexes_popup: IndexesPopup::default(),
            version_picker: VersionPicker::default(),
            tree_view: TreeView::default(),
            why_view: TreeView::default(),
            why_of: None,
            bump_popup: BumpPopup::default(),
            search_popup: SearchPopup::default(),
            groups_popup: GroupsPopup::default(),
            adding_version_of: None,
            pending: None,
            group_popup: None,
            emptied: Vec::new(),
            delete_question: None,
        }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect, deps: &DepsState) {
        self.render_content(frame, area, deps);
        if self.show_package_control_popup {
            self.package_control_popup.render(frame, area);
        }
        if self.package_add_popup.is_editing() {
            self.package_add_popup.render(frame, area);
        }
        self.version_picker.render(frame, area, deps);
        self.bump_popup.render(frame, area, deps);
        self.search_popup.render(frame, area, deps);
        self.groups_popup.render(frame, area, deps);
        if self.pin_version_popup.is_editing() {
            self.pin_version_popup.render(frame, area);
        }
        self.add_from_source.render(frame, area);
        self.indexes_popup.render(frame, area);
        if let Some((popup, _)) = &mut self.group_popup {
            popup.render(frame, area);
        }
        if self.new_group_popup.is_editing() {
            self.new_group_popup.render(frame, area);
        }
        self.ask_to_delete(deps);
        // Opened only over the bare table, and it takes every key while open.
        if let Some((popup, _)) = &mut self.delete_question {
            popup.render(frame, area);
        }
    }

    /// Asks whether to delete a group a removal or move left empty, once nothing else is open.
    fn ask_to_delete(&mut self, deps: &DepsState) {
        if self.delete_question.is_some()
            || self.mode() != Mode::Table
            || self.show_package_control_popup
        {
            return;
        }
        let Some(i) = self
            .emptied
            .iter()
            .position(|group| deps.deletable_groups.contains(group))
        else {
            return;
        };
        let group = self.emptied.remove(i);
        let options = vec![format!("Delete group {group}"), "Keep it".into()];
        let title = format!("Group {group} is now empty");
        self.delete_question = Some((ListPopup::new(&title, options), group));
    }

    /// Remembers the groups `commands` remove a package from.
    fn watch(&mut self, commands: &[Command]) {
        for command in commands {
            let from = match command {
                Command::RemoveFromProject { groups, .. } => groups.as_slice(),
                Command::Move { from, .. } => std::slice::from_ref(from),
                _ => &[],
            };
            for membership in from {
                if let Membership::Group(name) = membership
                    && !self.emptied.contains(name)
                {
                    self.emptied.push(name.clone());
                }
            }
        }
        // A group the same commands go on to delete, as `g` deletes or renames one, is not
        // asked about.
        self.emptied.retain(|group| {
            !commands
                .iter()
                .any(|c| matches!(c, Command::DeleteGroup { name } if name == group))
        });
    }

    /// Where the focus is: an input, the actions list, or the table.
    pub fn mode(&self) -> Mode {
        if let Some(mode) = self
            .add_from_source
            .mode()
            .or(self.indexes_popup.mode())
            .or(self.search_popup.mode())
            .or(self.groups_popup.mode())
        {
            mode
        } else if self.package_add_popup.is_editing()
            || self.pin_version_popup.is_editing()
            || self.new_group_popup.is_editing()
        {
            Mode::Input
        } else if self.show_package_control_popup
            || self.group_popup.is_some()
            || self.delete_question.is_some()
            || self.version_picker.is_open()
            || self.bump_popup.is_open()
        {
            Mode::Actions
        } else if self.active_table == ActiveTable::Tree {
            Mode::Tree
        } else {
            Mode::Table
        }
    }

    /// The tabs, as the active table's title: its name green and bold, the other white.
    fn tabs(&self) -> Line<'static> {
        let tab = |name: String, active: bool| {
            if active {
                name.green().bold()
            } else {
                name.white()
            }
        };
        let active = |table| self.active_table == table;
        Line::from(vec![
            " ".into(),
            tab("pyproject.toml".into(), active(ActiveTable::Pyproject)),
            " • ".white(),
            tab("Environment".into(), active(ActiveTable::Environment)),
            " • ".white(),
            // A "why" names its package in place of the tree.
            tab(
                self.why_of
                    .as_ref()
                    .map_or("Tree".into(), |name| format!("Why {name}")),
                active(ActiveTable::Tree),
            ),
            " ".into(),
        ])
    }

    fn render_content(&mut self, frame: &mut Frame, area: Rect, deps: &DepsState) {
        let tabs = self.tabs();
        match self.active_table {
            ActiveTable::Pyproject => {
                let placeholder = placeholder(
                    &deps.pyproject,
                    "No dependencies yet. Press p to add one.",
                    "Could not read pyproject.toml",
                );
                let rows = deps.pyproject.rows();
                self.pyproject_table.set_title(tabs);
                self.pyproject_table.render(frame, area, rows, placeholder);
            }
            ActiveTable::Environment => {
                let placeholder = placeholder(
                    &deps.env,
                    "Nothing installed yet. Press s to sync.",
                    "Could not list the environment",
                );
                self.env_table.set_title(tabs);
                self.env_table
                    .render(frame, area, deps.env.rows(), placeholder);
            }
            ActiveTable::Tree => match &self.why_of {
                // What pulls a package in, in place of the tree until Esc.
                Some(name) => {
                    let (rows, placeholder) = match &deps.why {
                        Some((of, loaded)) if of == name => (
                            loaded.rows(),
                            placeholder(loaded, "Nothing pulls it in", "Could not tell"),
                        ),
                        _ => (&[][..], Text::from("Loading…").dark_gray()),
                    };
                    self.why_view.render(frame, area, rows, tabs, placeholder);
                }
                None => {
                    let placeholder = match &deps.tree {
                        // uv's words for a project never locked.
                        Loadable::Failed(reason) if reason.contains("lockfile") => {
                            Text::from("No uv.lock yet. Press l to lock.").dark_gray()
                        }
                        tree => placeholder(tree, "No dependencies", "Could not read the tree"),
                    };
                    self.tree_view
                        .render(frame, area, deps.tree.rows(), tabs, placeholder);
                }
            },
        }
    }

    /// The highlighted package of the Tree tab, in the tree or in a "why".
    fn tree_selected<'a>(&self, deps: &'a DepsState) -> Option<&'a TreeNode> {
        match &self.why_of {
            Some(name) => match &deps.why {
                Some((of, loaded)) if of == name => {
                    let rows = loaded.rows();
                    self.why_view.selected(rows).and_then(|i| rows.get(i))
                }
                _ => None,
            },
            None => {
                let rows = deps.tree.rows();
                self.tree_view.selected(rows).and_then(|i| rows.get(i))
            }
        }
    }

    fn selected_name(&self, deps: &DepsState) -> Option<String> {
        match self.active_table {
            ActiveTable::Pyproject => self
                .pyproject_table
                .selected(deps.pyproject.rows())
                .map(|d| d.name.clone()),
            ActiveTable::Environment => self
                .env_table
                .selected(deps.env.rows())
                .map(|d| d.name.clone()),
            ActiveTable::Tree => self.tree_selected(deps).map(|node| node.name.clone()),
        }
    }

    /// Moves to the next tab, or the one before, round.
    fn switch_table(&mut self, forward: bool) {
        use ActiveTable::*;
        self.active_table = match (&self.active_table, forward) {
            (Pyproject, true) | (Tree, false) => Environment,
            (Environment, true) | (Pyproject, false) => Tree,
            (Tree, true) | (Environment, false) => Pyproject,
        };
    }

    /// Runs `step` on the Tree tab's view, of the "why" when one shows, with its rows.
    fn on_tree(&mut self, deps: &DepsState, step: impl FnOnce(&mut TreeView, &[TreeNode])) {
        match &self.why_of {
            Some(name) => {
                if let Some((of, loaded)) = &deps.why
                    && of == name
                {
                    step(&mut self.why_view, loaded.rows());
                }
            }
            None => step(&mut self.tree_view, deps.tree.rows()),
        }
    }

    pub fn update(&mut self, action: &Action, deps: &DepsState) -> Vec<Command> {
        self.ask_to_delete(deps);
        if let Some((popup, group)) = &mut self.delete_question {
            match action {
                Action::Nav(NavAction::Up) => popup.up(),
                Action::Nav(NavAction::Down) => popup.down(),
                Action::Nav(NavAction::Select) => {
                    let delete = popup.selected() == Some(0);
                    let name = group.clone();
                    self.delete_question = None;
                    if delete {
                        return vec![Command::DeleteGroup { name }];
                    }
                }
                Action::Nav(NavAction::Back) => self.delete_question = None,
                _ => {}
            }
            return Vec::new();
        }
        let commands = self.respond(action, deps);
        self.watch(&commands);
        commands
    }

    fn respond(&mut self, action: &Action, deps: &DepsState) -> Vec<Command> {
        if self.groups_popup.mode().is_some() {
            return self.groups_popup.update(action, deps);
        }
        if self.search_popup.mode().is_some() {
            return match self.search_popup.update(action, deps) {
                Some(Searched::Find(name)) => vec![Command::ListVersions { name }],
                // On into the add, as from `p`; the versions are already in.
                Some(Searched::Add { name, requirement }) => {
                    self.adding_version_of = Some(requirement);
                    let title = format!("Add {name}");
                    self.version_picker.open(&name, &title, First::Latest, deps);
                    Vec::new()
                }
                None => Vec::new(),
            };
        }
        if self.bump_popup.is_open() {
            return self.bump_popup.update(action, deps).into_iter().collect();
        }
        if self.indexes_popup.mode().is_some() {
            return self.indexes_popup.update(action).into_iter().collect();
        }
        if self.add_from_source.mode().is_some() {
            if let Some(source) = self.add_from_source.update(action) {
                self.pending = Some(Pending::AddFrom(source));
                self.group_popup = Some(group_choices(deps, &[], "Add to group"));
            }
            return Vec::new();
        }
        // An open input takes every key.
        if self.package_add_popup.is_editing() {
            if let Action::Nav(nav) = action
                && let Some(requirement) = self.package_add_popup.update(nav)
            {
                // A name alone asks which version; one typed with a version, a marker or a URL
                // goes on to its group as it is.
                if let Some((name, with_extras)) = bare_name(&requirement) {
                    self.adding_version_of = Some(with_extras);
                    let title = format!("Add {name}");
                    self.version_picker.open(&name, &title, First::Latest, deps);
                    return vec![Command::ListVersions { name }];
                }
                self.pending = Some(Pending::Add(requirement));
                self.group_popup = Some(group_choices(deps, &[], "Add to group"));
            }
            return Vec::new();
        }
        if self.new_group_popup.is_editing() {
            if let Action::Nav(nav) = action
                && let Some(group) = self.new_group_popup.update(nav)
            {
                return self.finish(Membership::Group(group.trim().to_string()));
            }
            // Esc gives up on what was waiting.
            if !self.new_group_popup.is_editing() {
                self.pending = None;
            }
            return Vec::new();
        }
        if let Some((popup, groups)) = &mut self.group_popup {
            match action {
                Action::Nav(NavAction::Up) => popup.up(),
                Action::Nav(NavAction::Down) => popup.down(),
                Action::Nav(NavAction::Select) => {
                    let chosen = popup.selected().and_then(|i| groups.get(i).cloned());
                    self.group_popup = None;
                    // Past the project's groups is the new one, which is typed in.
                    return match chosen {
                        Some(membership) => self.finish(membership),
                        None => {
                            self.new_group_popup.open();
                            Vec::new()
                        }
                    };
                }
                Action::Nav(NavAction::Back) => {
                    self.group_popup = None;
                    self.pending = None;
                }
                _ => {}
            }
            return Vec::new();
        }
        if self.version_picker.is_open() {
            let picked = self.version_picker.update(action, deps);
            match (picked, self.adding_version_of.clone()) {
                (Some(Picked::First), None) => self.pin_version_popup.open(),
                (Some(Picked::Constraint(constraint)), None) => {
                    if let Some(name) = self.target.clone() {
                        return vec![Command::Pin { name, constraint }];
                    }
                }
                // Adding: "Latest" leaves the requirement as typed, for uv to choose; a version
                // joins it. Then its group.
                (Some(picked), Some(requirement)) => {
                    let requirement = match picked {
                        Picked::First => requirement,
                        Picked::Constraint(constraint) => format!("{requirement}{constraint}"),
                    };
                    self.adding_version_of = None;
                    self.pending = Some(Pending::Add(requirement));
                    self.group_popup = Some(group_choices(deps, &[], "Add to group"));
                }
                (None, _) => {}
            }
            // Esc gave up on the add too.
            if !self.version_picker.is_open() && self.group_popup.is_none() {
                self.adding_version_of = None;
            }
            return Vec::new();
        }
        if self.pin_version_popup.is_editing() {
            if let Action::Nav(nav) = action
                && let Some(constraint) = self.pin_version_popup.update(nav)
                && let Some(name) = self.target.clone()
            {
                return vec![Command::Pin { name, constraint }];
            }
            return Vec::new();
        }
        match action {
            Action::Nav(NavAction::Up) => {
                if self.show_package_control_popup {
                    self.package_control_popup.up();
                } else {
                    match self.active_table {
                        ActiveTable::Pyproject => {
                            self.pyproject_table.previous(deps.pyproject.rows());
                        }
                        ActiveTable::Environment => self.env_table.previous(deps.env.rows()),
                        ActiveTable::Tree => self.on_tree(deps, |view, rows| view.up(rows)),
                    }
                }
            }
            Action::Nav(NavAction::Down) => {
                if self.show_package_control_popup {
                    self.package_control_popup.down();
                } else {
                    match self.active_table {
                        ActiveTable::Pyproject => self.pyproject_table.next(deps.pyproject.rows()),
                        ActiveTable::Environment => self.env_table.next(deps.env.rows()),
                        ActiveTable::Tree => self.on_tree(deps, |view, rows| view.down(rows)),
                    }
                }
            }
            // The tree's own keys, bound on its tab alone.
            Action::Deps(DepsAction::Expand) => self.on_tree(deps, |view, rows| view.expand(rows)),
            Action::Deps(DepsAction::Collapse) => {
                self.on_tree(deps, |view, rows| view.collapse(rows));
            }
            Action::Deps(DepsAction::ExpandAll) => self.on_tree(deps, |view, _| view.expand_all()),
            Action::Deps(DepsAction::CollapseAll) => {
                self.on_tree(deps, |view, rows| view.collapse_all(rows));
            }
            // Shown or hidden: what is behind takes the index to tell.
            Action::Deps(DepsAction::Outdated) => {
                return vec![Command::LoadTree {
                    outdated: !deps.tree_outdated,
                }];
            }
            Action::Deps(DepsAction::Why) => {
                if let Some(node) = self.tree_selected(deps) {
                    let name = node.name.clone();
                    self.why_of = Some(name.clone());
                    self.why_view = TreeView::default();
                    return vec![Command::WhyInstalled { name }];
                }
            }
            Action::Nav(NavAction::Select) => {
                if self.show_package_control_popup {
                    return self.choose(deps);
                }
                // Without a row the actions would have nothing to act on; on the tree, a
                // package pyproject.toml does not declare has none.
                let Some(name) = self.selected_name(deps) else {
                    return Vec::new();
                };
                if self.active_table == ActiveTable::Tree && self.declared(&name, deps).is_none() {
                    return Vec::new();
                }
                // A fresh popup, so it never opens with Remove still selected. Group actions are
                // for a declared package.
                let groups = self.declared(&name, deps).map_or(0, |d| d.groups.len());
                self.target = Some(name);
                self.package_control_popup = PackageControlPopup::new(groups);
                self.show_package_control_popup = true;
            }
            // Esc closes the actions, or goes back from a "why" to the tree.
            Action::Nav(NavAction::Back) => {
                if self.show_package_control_popup {
                    self.show_package_control_popup = false;
                } else {
                    self.why_of = None;
                }
            }
            Action::Deps(DepsAction::SwitchTable | DepsAction::SwitchTableBack) => {
                // The control popup acts on the row it was opened for.
                if !self.show_package_control_popup {
                    self.switch_table(matches!(action, Action::Deps(DepsAction::SwitchTable)));
                }
            }
            Action::Deps(DepsAction::OpenAdd) => self.package_add_popup.open(),
            Action::Deps(DepsAction::OpenAddFromSource) => self.add_from_source.open(),
            Action::Deps(DepsAction::OpenIndexes) => self.indexes_popup.open(&deps.indexes),
            Action::Deps(DepsAction::OpenSearch) => self.search_popup.open(),
            Action::Deps(DepsAction::OpenGroups) => self.groups_popup.open(deps),
            // The page's: the version is the project's, not the tables'.
            Action::Deps(
                DepsAction::OpenVersion | DepsAction::OpenEnvironment | DepsAction::Publish,
            ) => {}
            // Keys of the list of groups, which handles them itself.
            Action::Deps(DepsAction::Rename | DepsAction::New) => {}
            // The keys of the actions list's Update and Remove, on the selected row; d in the list
            // itself is for the list of indexes.
            Action::Deps(DepsAction::Update | DepsAction::Remove)
                if self.show_package_control_popup => {}
            Action::Deps(DepsAction::Bump) => {
                if let Some(name) = self.selected_name(deps) {
                    return self.bump(&name, deps);
                }
            }
            Action::Deps(DepsAction::Update) => {
                if let Some(name) = self.selected_name(deps) {
                    return vec![Command::Upgrade { name }];
                }
            }
            Action::Deps(DepsAction::Remove) => {
                if let Some(name) = self.selected_name(deps) {
                    return self.remove(&name, deps).into_iter().collect();
                }
            }
            Action::Deps(DepsAction::Sync) => return vec![Command::Sync],
            Action::Deps(DepsAction::Build) => return vec![Command::Build],
            Action::Deps(DepsAction::Lock { upgrade }) => {
                return vec![Command::Lock { upgrade: *upgrade }];
            }
            Action::Deps(DepsAction::Refresh) => return vec![Command::Reload],
            // Confirm and Type only mean something in an input; Quit, Cancel and Help belong to
            // the app.
            Action::Nav(NavAction::Confirm | NavAction::Type(_))
            | Action::Quit
            | Action::Cancel
            | Action::Help => {}
        }
        Vec::new()
    }

    /// Does what waited on a group to `membership`.
    fn finish(&mut self, membership: Membership) -> Vec<Command> {
        let command = match self.pending.take() {
            Some(Pending::Add(requirement)) => Command::Add {
                requirement,
                membership,
            },
            Some(Pending::AddFrom(source)) => Command::AddFrom { source, membership },
            Some(Pending::Move {
                name,
                requirement,
                from,
            }) => Command::Move {
                name,
                requirement,
                from,
                to: membership,
            },
            Some(Pending::RemoveFrom(name)) => Command::RemoveFromProject {
                name,
                groups: vec![membership],
            },
            None => return Vec::new(),
        };
        vec![command]
    }

    /// The pyproject row of `name`, when the pyproject table or the tree is the one showing.
    /// The tree names packages as uv does, so it is matched normalized.
    fn declared<'a>(&self, name: &str, deps: &'a DepsState) -> Option<&'a PyprojectDependency> {
        let rows = deps.pyproject.rows().iter();
        match self.active_table {
            ActiveTable::Pyproject => rows.into_iter().find(|d| d.name == name),
            ActiveTable::Tree => {
                let wanted = normalize_name(name);
                rows.into_iter().find(|d| normalize_name(&d.name) == wanted)
            }
            ActiveTable::Environment => None,
        }
    }

    /// Opens the group list for a group action on the declared package `name`.
    fn choose_group(&mut self, option: PackageControlOption, name: String, deps: &DepsState) {
        let Some(dep) = self.declared(&name, deps) else {
            return;
        };
        let requirement = dep.requirement();
        let (pending, list) = match option {
            PackageControlOption::MoveToGroup => (
                Pending::Move {
                    name,
                    requirement,
                    // Offered for a package in exactly one group.
                    from: dep.groups.first().cloned().unwrap_or(Membership::Project),
                },
                group_choices(deps, &dep.groups, "Move to group"),
            ),
            PackageControlOption::AddToGroup => (
                Pending::Add(requirement),
                group_choices(deps, &dep.groups, "Add to group"),
            ),
            PackageControlOption::RemoveFromGroup => {
                let labels = dep.groups.iter().map(ToString::to_string).collect();
                (
                    Pending::RemoveFrom(name),
                    (
                        ListPopup::new("Remove from group", labels),
                        dep.groups.clone(),
                    ),
                )
            }
            _ => return,
        };
        self.pending = Some(pending);
        self.group_popup = Some(list);
    }

    /// Runs the option highlighted in the actions list on the package it was opened for.
    fn choose(&mut self, deps: &DepsState) -> Vec<Command> {
        self.show_package_control_popup = false;
        let Some(name) = self.target.clone() else {
            return Vec::new();
        };
        match self.package_control_popup.selected() {
            // The versions are asked for as the list opens, and fill it when they arrive.
            Some(PackageControlOption::PinToVersion) => {
                let title = format!("Pin {name}");
                self.version_picker
                    .open(&name, &title, First::TypeConstraint, deps);
                vec![Command::ListVersions { name }]
            }
            Some(PackageControlOption::Bump) => self.bump(&name, deps),
            Some(PackageControlOption::Update) => vec![Command::Upgrade { name }],
            Some(PackageControlOption::Remove) => self.remove(&name, deps).into_iter().collect(),
            Some(
                option @ (PackageControlOption::MoveToGroup
                | PackageControlOption::AddToGroup
                | PackageControlOption::RemoveFromGroup),
            ) => {
                self.choose_group(option, name, deps);
                Vec::new()
            }
            Some(PackageControlOption::Cancel) | None => Vec::new(),
        }
    }

    /// Opens the bumps of `name`, if pyproject.toml declares it, and asks for its versions. One
    /// only installed is not bumped: that would declare a dependency of a dependency.
    fn bump(&mut self, name: &str, deps: &DepsState) -> Vec<Command> {
        let wanted = normalize_name(name);
        deps.pyproject
            .rows()
            .iter()
            .find(|d| normalize_name(&d.name) == wanted)
            .map(|dep| self.bump_popup.open(dep, deps))
            .into_iter()
            .collect()
    }

    /// Removes `name` from wherever the active table lists it, if it still does.
    fn remove(&self, name: &str, deps: &DepsState) -> Option<Command> {
        match self.active_table {
            ActiveTable::Pyproject => {
                deps.pyproject
                    .rows()
                    .iter()
                    .find(|d| d.name == name)
                    .map(|d| Command::RemoveFromProject {
                        name: d.name.clone(),
                        groups: d.groups.clone(),
                    })
            }
            // On the tree, as from pyproject.toml, for a package it declares.
            ActiveTable::Tree => self
                .declared(name, deps)
                .map(|d| Command::RemoveFromProject {
                    name: d.name.clone(),
                    groups: d.groups.clone(),
                }),
            // Environment only: pyproject.toml is left alone.
            ActiveTable::Environment => {
                deps.env
                    .rows()
                    .iter()
                    .find(|d| d.name == name)
                    .map(|d| Command::Uninstall {
                        name: d.name.clone(),
                    })
            }
        }
    }
}

/// The list a group is chosen from: main, then the project's dependency groups, those with rows
/// and the empty ones, leaving out those in `except`, then a new group.
fn group_choices(
    deps: &DepsState,
    except: &[Membership],
    title: &str,
) -> (ListPopup, Vec<Membership>) {
    let mut groups: Vec<Membership> = deps
        .pyproject
        .rows()
        .iter()
        .flat_map(|d| &d.groups)
        .filter(|m| matches!(m, Membership::Group(_)))
        .cloned()
        .chain(deps.empty_groups.iter().cloned().map(Membership::Group))
        .collect();
    groups.sort_by_key(ToString::to_string);
    groups.dedup();
    groups.insert(0, Membership::Project);
    groups.retain(|m| !except.contains(m));
    let mut labels: Vec<String> = groups.iter().map(ToString::to_string).collect();
    labels.push("New group…".into());
    (ListPopup::new(title, labels), groups)
}

/// What a table shows in place of rows: how its load is going, that it loaded nothing, or why
/// it failed.
fn placeholder<T>(loadable: &Loadable<Vec<T>>, empty: &'static str, failed: &str) -> Text<'static> {
    match loadable {
        Loadable::NotLoaded => Text::default(),
        Loadable::Loading { since } => match since.elapsed().as_secs() {
            // A slow load says how long it has taken.
            waited @ 2.. => Text::from(format!("Loading… {waited}s")).dark_gray(),
            _ => Text::from("Loading…").dark_gray(),
        },
        Loadable::Loaded(_) => Text::from(empty).dark_gray(),
        Loadable::Failed(reason) => Text::from(format!("{failed}: {reason}")).red(),
    }
}
