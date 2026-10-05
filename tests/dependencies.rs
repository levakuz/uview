use std::collections::VecDeque;
use std::error::Error;
use std::path::PathBuf;
use std::process::ExitStatus;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};
use tokio_util::sync::CancellationToken;
use uview::controllers::dependencies::{
    add_dep, normalize, parse_dep_string, remove_dep_from_pyproject,
};
use uview::domain::{Membership, PyprojectDependency};
use uview::message::{Command, Event, JobId};
use uview::runtime::jobs::JobRunner;
use uview::state::{DepsState, JobsState, LOG_LINES, LogLine};
use uview::uv::args::{membership_args, pin_requirement, pip_list_args, uninstall_args};
use uview::uv::{RealUvClient, UvClient, UvError};

/// Stands in for `uv`: answers each call with the next canned result, or empty stdout once
/// they run out, and records the argv it received. A call whose verb is `hang` never returns,
/// like a slow resolve.
#[derive(Default)]
struct FakeUvClient {
    results: Mutex<VecDeque<Result<String, UvError>>>,
    calls: Mutex<Vec<Vec<String>>>,
    hang: Option<&'static str>,
    /// Hanging calls dropped unfinished, as `run_streaming` drops a cancelled call.
    dropped: AtomicUsize,
    /// What every streamed call writes to stderr before it returns.
    stderr: Vec<&'static str>,
}

impl FakeUvClient {
    fn returning(results: impl IntoIterator<Item = Result<String, UvError>>) -> Self {
        Self {
            results: Mutex::new(results.into_iter().collect()),
            ..Self::default()
        }
    }

    fn hanging_on(verb: &'static str) -> Self {
        Self {
            hang: Some(verb),
            ..Self::default()
        }
    }

    fn calls(&self) -> Vec<Vec<String>> {
        self.calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl UvClient for FakeUvClient {
    async fn run(&self, args: &[&str]) -> Result<String, UvError> {
        self.calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(args.iter().map(ToString::to_string).collect());
        if self.hang.is_some_and(|verb| args.first() == Some(&verb)) {
            let _counted = CountsDrop(&self.dropped);
            std::future::pending::<()>().await;
        }
        self.results
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pop_front()
            .unwrap_or_else(|| Ok(String::new()))
    }

    async fn run_streaming(
        &self,
        args: &[&str],
        cancel: &CancellationToken,
        on_line: &(dyn Fn(&str) + Sync),
    ) -> Result<String, UvError> {
        for line in &self.stderr {
            on_line(line);
        }
        tokio::select! {
            biased;
            () = cancel.cancelled() => Err(UvError::Cancelled),
            result = self.run(args) => result,
        }
    }
}

struct CountsDrop<'a>(&'a AtomicUsize);

impl Drop for CountsDrop<'_> {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

/// An environment that exists, so a reload asks `uv` to list it.
fn synced() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Receives events up to the first that `last` matches, and returns them all. Gives up after a
/// few seconds rather than hang the test run.
async fn events_until(
    received: &mut UnboundedReceiver<Event>,
    last: impl Fn(&Event) -> bool,
) -> Result<Vec<Event>, Box<dyn Error>> {
    let mut events = Vec::new();
    loop {
        let event = tokio::time::timeout(Duration::from_secs(5), received.recv())
            .await?
            .ok_or("the runner stopped")?;
        let done = last(&event);
        events.push(event);
        if done {
            return Ok(events);
        }
    }
}

fn finished(id: u64) -> impl Fn(&Event) -> bool {
    move |event| matches!(event, Event::JobFinished(JobId(n)) if *n == id)
}

fn started(id: u64) -> impl Fn(&Event) -> bool {
    move |event| matches!(event, Event::JobStarted(JobId(n)) if *n == id)
}

fn notice(text: &str) -> impl Fn(&Event) -> bool {
    move |event| matches!(event, Event::Notice(m) if m == text)
}

#[test]
fn test_parse_dep_string() {
    let parsed = |name: &str, extras: &[&str], constraint: &str| {
        let extras = extras.iter().map(ToString::to_string).collect();
        (name.to_string(), extras, constraint.to_string())
    };
    assert_eq!(
        parse_dep_string("httpx>=0.28.1"),
        parsed("httpx", &[], ">=0.28.1")
    );
    assert_eq!(parse_dep_string("requests"), parsed("requests", &[], ""));
    assert_eq!(
        parse_dep_string("numpy==1.26.0"),
        parsed("numpy", &[], "==1.26.0")
    );
    // Operators inside a marker or a URL are not the version's.
    assert_eq!(
        parse_dep_string("tomli; python_version < '3.11'"),
        parsed("tomli", &[], "; python_version < '3.11'")
    );
    assert_eq!(
        parse_dep_string("pkg @ https://example.com/pkg-1.0.tar.gz"),
        parsed("pkg", &[], "@ https://example.com/pkg-1.0.tar.gz")
    );
    // Extras are not part of the name, with or without spaces around them.
    assert_eq!(
        parse_dep_string("flask[dotenv]>=3.0"),
        parsed("flask", &["dotenv"], ">=3.0")
    );
    assert_eq!(
        parse_dep_string("uvicorn [standard, socks] ~= 0.30"),
        parsed("uvicorn", &["standard", "socks"], "~= 0.30")
    );
    assert_eq!(
        parse_dep_string("requests[socks]; sys_platform == 'win32'"),
        parsed("requests", &["socks"], "; sys_platform == 'win32'")
    );
    assert_eq!(
        parse_dep_string("pkg[cli] @ git+https://example.com/pkg"),
        parsed("pkg", &["cli"], "@ git+https://example.com/pkg")
    );
}

/// What pinning rebuilds from a parsed declaration is the declaration: extras and the marker
/// survive, so `uv add` rewrites the entry in place instead of adding a second one.
#[test]
fn a_parsed_requirement_rebuilds_as_it_was() {
    for requirement in [
        "flask[dotenv]>=3.0",
        "uvicorn[socks,standard]==0.30.1",
        "tomli<2; python_version < '3.11'",
        "requests[socks]>=2.31; sys_platform == 'win32'",
    ] {
        let (name, extras, constraint) = parse_dep_string(requirement);
        let (version, marker) = match constraint.split_once(';') {
            Some((version, marker)) => (version, Some(marker.trim())),
            None => (constraint.as_str(), None),
        };
        assert_eq!(
            pin_requirement(&name, &extras, version, marker),
            requirement
        );
    }
}

#[test]
fn pin_requirement_normalizes_constraints() {
    assert_eq!(pin_requirement("httpx", &[], "1.2.3", None), "httpx==1.2.3");
    assert_eq!(pin_requirement("httpx", &[], " >=2.0 ", None), "httpx>=2.0");
    assert_eq!(pin_requirement("flask", &[], "~=3.0", None), "flask~=3.0");
    assert_eq!(pin_requirement("httpx", &[], "", None), "httpx");
    assert_eq!(
        pin_requirement("tomli", &[], "2.0.1", Some("python_version < '3.11'")),
        "tomli==2.0.1; python_version < '3.11'"
    );
}

#[tokio::test]
async fn failed_uv_call_returns_error_instead_of_panicking() {
    // `uv` exits 1 for an unknown package; without `uv` on PATH this is a spawn error.
    assert!(
        RealUvClient
            .run(&["pip", "show", "uview-no-such-package"])
            .await
            .is_err()
    );
}

#[tokio::test]
async fn successful_uv_call_returns_stdout() {
    match RealUvClient.run(&["--version"]).await {
        Err(UvError::Spawn(_)) => {} // `uv` is not installed here; nothing to check.
        other => assert!(other.is_ok_and(|out| out.starts_with("uv "))),
    }
}

/// `uv add` reports its progress on stderr as it runs, and writes pyproject.toml before it
/// resolves. Cancelled, it must be stopped the way Ctrl+C stops it, which puts the file back, and
/// the call must return only once it has.
#[cfg(unix)]
#[tokio::test]
async fn uv_add_streams_its_lines_and_a_cancel_puts_pyproject_back() -> Result<(), Box<dyn Error>> {
    // An index that takes connections and never answers, so the resolve hangs.
    let index = std::net::TcpListener::bind("127.0.0.1:0")?;
    let url = format!("http://{}/simple", index.local_addr()?);
    std::thread::spawn(move || index.incoming().collect::<Vec<_>>());
    let dir = std::env::temp_dir().join(format!("uview-cancel-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    let pyproject = dir.join("pyproject.toml");
    std::fs::write(
        &pyproject,
        "[project]\nname = \"p\"\nversion = \"0.1.0\"\ndependencies = []\n",
    )?;
    let lists_requests = || -> std::io::Result<bool> {
        Ok(std::fs::read_to_string(&pyproject)?.contains("requests"))
    };

    let dir_arg = dir.to_string_lossy();
    let args = [
        "add",
        "requests",
        "--directory",
        &dir_arg,
        "--default-index",
        &url,
    ];
    let lines = Mutex::new(Vec::new());
    let on_line = |line: &str| {
        let mut lines = lines.lock().unwrap_or_else(PoisonError::into_inner);
        lines.push(line.to_owned());
    };
    let streamed = || {
        !lines
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .is_empty()
    };
    let cancel = CancellationToken::new();
    let mut add = Box::pin(RealUvClient.run_streaming(&args, &cancel, &on_line));
    let mut waited = 0;
    // Both while uv still runs, since it hangs until cancelled.
    while !(lists_requests()? && streamed()) {
        // Finished already: no `uv` or no Python here, so nothing to cancel.
        if tokio::time::timeout(Duration::from_millis(50), &mut add)
            .await
            .is_ok()
        {
            return Ok(());
        }
        waited += 1;
        if waited > 200 {
            return Err("uv wrote neither pyproject.toml nor a line".into());
        }
    }
    // uv sets its Ctrl+C handler only just after it writes the file. No user is that quick, but
    // a test watching for the write can be.
    tokio::time::sleep(Duration::from_millis(200)).await;
    cancel.cancel();
    assert!(matches!(add.await, Err(UvError::Cancelled)));
    assert!(!lists_requests()?, "uv left requests in pyproject.toml");
    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

#[test]
fn membership_args_pick_the_flag_for_each_membership() {
    let main_group = Membership::Group("main".into());
    assert_eq!(
        membership_args("remove", "httpx", &Membership::Project),
        ["remove", "httpx"]
    );
    // A dependency group that happens to be named `main` is still a group.
    assert_eq!(
        membership_args("remove", "pytest", &main_group),
        ["remove", "pytest", "--group", "main"]
    );
    assert_eq!(
        membership_args("add", "flask==3.0", &Membership::Extra("web".into())),
        ["add", "flask==3.0", "--optional", "web"]
    );
}

#[test]
fn the_environment_is_listed_quietly() {
    // A workspace member's environment is not the `.venv` beside it, and uv would say so.
    assert_eq!(
        pip_list_args("/ws/.venv"),
        [
            "pip",
            "list",
            "--quiet",
            "--format",
            "json",
            "--python",
            "/ws/.venv"
        ]
    );
}

#[test]
fn uninstall_targets_the_given_environment_explicitly() {
    assert_eq!(
        uninstall_args("six", ".venv"),
        ["pip", "uninstall", "--python", ".venv", "six"]
    );
}

#[test]
fn normalize_ignores_case_separators_and_extras() {
    assert_eq!(normalize("Flask_SQLAlchemy"), normalize("flask-sqlalchemy"));
    assert_eq!(normalize("zope.interface"), normalize("zope-interface"));
    assert_eq!(normalize("flask[dotenv]"), normalize("Flask"));
    assert_ne!(normalize("flask"), normalize("flask-cors"));
}

#[tokio::test]
async fn remove_runs_uv_remove_per_membership() -> Result<(), UvError> {
    let uv = FakeUvClient::default();
    let memberships = vec![Membership::Project, Membership::Group("dev".into())];
    remove_dep_from_pyproject(&uv, "httpx", memberships).await?;
    assert_eq!(
        uv.calls(),
        [
            vec!["remove", "httpx"],
            vec!["remove", "httpx", "--group", "dev"],
        ]
    );
    Ok(())
}

#[tokio::test]
async fn add_runs_uv_add_alone() -> Result<(), UvError> {
    let uv = FakeUvClient::default();
    add_dep(&uv, "httpx>=0.27", &Membership::Project).await?;
    add_dep(&uv, "pytest", &Membership::Group("dev".into())).await?;
    assert_eq!(
        uv.calls(),
        [
            vec!["add", "httpx>=0.27"],
            vec!["add", "pytest", "--group", "dev"]
        ]
    );
    Ok(())
}

#[tokio::test]
async fn an_add_to_a_group_says_which() -> Result<(), Box<dyn Error>> {
    let uv = Arc::new(FakeUvClient::default());
    let (events, mut received) = unbounded_channel();
    let mut runner = JobRunner::new(Arc::clone(&uv), synced(), events);
    runner.execute(Command::Add {
        requirement: "pytest".into(),
        membership: Membership::Group("dev".into()),
    });
    let events = events_until(&mut received, finished(1)).await?;
    assert!(events.iter().any(|e| matches!(
        e,
        Event::JobQueued { label, .. } if label == "Adding pytest to dev"
    )));
    assert!(events.iter().any(notice("Added pytest to dev")));
    Ok(())
}

fn move_six() -> Command {
    Command::Move {
        name: "six".into(),
        requirement: "six>=1.16".into(),
        from: Membership::Project,
        to: Membership::Group("dev".into()),
    }
}

#[tokio::test]
async fn a_move_adds_to_the_new_group_then_removes_from_the_old() -> Result<(), Box<dyn Error>> {
    let uv = Arc::new(FakeUvClient::default());
    let (events, mut received) = unbounded_channel();
    let mut runner = JobRunner::new(Arc::clone(&uv), synced(), events);
    runner.execute(move_six());
    let events = events_until(&mut received, finished(1)).await?;
    assert_eq!(
        uv.calls(),
        [
            vec!["add", "six>=1.16", "--group", "dev"],
            vec!["remove", "six"]
        ]
    );
    assert!(events.iter().any(notice("Moved six from main to dev")));
    Ok(())
}

#[tokio::test]
async fn a_move_whose_add_fails_removes_nothing() -> Result<(), Box<dyn Error>> {
    let uv = Arc::new(FakeUvClient::returning([Err(UvError::Parse("no".into()))]));
    let (events, mut received) = unbounded_channel();
    let mut runner = JobRunner::new(Arc::clone(&uv), synced(), events);
    runner.execute(move_six());
    events_until(&mut received, finished(1)).await?;
    assert_eq!(uv.calls().len(), 1);
    Ok(())
}

#[tokio::test]
async fn a_move_whose_remove_fails_says_where_the_package_is() -> Result<(), Box<dyn Error>> {
    let uv = Arc::new(FakeUvClient::returning([
        Ok(String::new()),
        Err(UvError::Parse("no".into())),
    ]));
    let (events, mut received) = unbounded_channel();
    let mut runner = JobRunner::new(Arc::clone(&uv), synced(), events);
    runner.execute(move_six());
    let events = events_until(&mut received, finished(1)).await?;
    assert!(events.iter().any(|e| matches!(
        e,
        Event::Error(r) if r.starts_with("six was added to dev but is still in main:")
    )));
    Ok(())
}

#[tokio::test]
async fn every_mutation_reports_the_project_changed_however_it_ends() -> Result<(), Box<dyn Error>>
{
    let uv = Arc::new(FakeUvClient::returning([
        Ok(String::new()),
        Err(UvError::Parse("no".into())),
    ]));
    let (events, mut received) = unbounded_channel();
    let mut runner = JobRunner::new(Arc::clone(&uv), synced(), events);
    runner.execute(Command::Upgrade { name: "six".into() });
    runner.execute(Command::Add {
        requirement: "httpx".into(),
        membership: Membership::Project,
    });
    let events = events_until(&mut received, finished(2)).await?;
    assert!(events.iter().any(notice("Updated six")));
    // Done and failed alike: a failed `uv add` can have written pyproject.toml.
    let mutated = events
        .iter()
        .filter(|event| matches!(event, Event::ProjectMutated))
        .count();
    assert_eq!(mutated, 2);
    // The runner reads nothing itself: the app reloads, in answer to ProjectMutated.
    let commands: Vec<String> = uv.calls().iter().map(|call| call[..2].join(" ")).collect();
    assert_eq!(commands, ["sync --upgrade-package", "add httpx"]);
    Ok(())
}

#[tokio::test]
async fn a_reload_carries_how_many_mutations_had_ended() -> Result<(), Box<dyn Error>> {
    let uv = Arc::new(FakeUvClient::default());
    let (events, mut received) = unbounded_channel();
    let mut runner = JobRunner::new(Arc::clone(&uv), synced(), events);
    let generation = |events: &[Event]| {
        events.iter().find_map(|event| match event {
            Event::PyprojectLoaded { generation, .. } => Some(*generation),
            _ => None,
        })
    };
    runner.execute(Command::Reload);
    let before = events_until(&mut received, finished(1)).await?;
    runner.execute(Command::Upgrade { name: "six".into() });
    events_until(&mut received, finished(2)).await?;
    runner.execute(Command::Reload);
    let after = events_until(&mut received, finished(3)).await?;
    assert_eq!(generation(&before), Some(0));
    assert_eq!(generation(&after), Some(1));
    Ok(())
}

#[tokio::test]
async fn sync_runs_plain_uv_sync() -> Result<(), Box<dyn Error>> {
    let uv = Arc::new(FakeUvClient::default());
    let (events, mut received) = unbounded_channel();
    let mut runner = JobRunner::new(Arc::clone(&uv), synced(), events);
    runner.execute(Command::Sync);
    let events = events_until(&mut received, finished(1)).await?;
    assert!(events.iter().any(notice("Synced the environment")));
    assert!(
        events
            .iter()
            .any(|event| matches!(event, Event::ProjectMutated))
    );
    assert_eq!(uv.calls(), [["sync"]]);
    Ok(())
}

#[tokio::test]
async fn a_failed_load_reaches_its_table_and_the_status_line() -> Result<(), Box<dyn Error>> {
    let uv = Arc::new(FakeUvClient::returning([Err(UvError::Parse(
        "bad json".into(),
    ))]));
    let (events, mut received) = unbounded_channel();
    let mut runner = JobRunner::new(Arc::clone(&uv), synced(), events);
    runner.execute(Command::Reload);
    let events = events_until(&mut received, finished(1)).await?;
    let reason = "could not read `uv` output: bad json";
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::EnvLoaded { loaded: Err(r), .. } if r == reason))
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::Error(r) if r == reason))
    );
    Ok(())
}

#[tokio::test]
async fn a_missing_environment_loads_empty_without_asking_uv() -> Result<(), Box<dyn Error>> {
    let uv = Arc::new(FakeUvClient::default());
    let (events, mut received) = unbounded_channel();
    let missing = synced().join("no-such-venv");
    let mut runner = JobRunner::new(Arc::clone(&uv), missing, events);
    runner.execute(Command::Reload);
    let events = events_until(&mut received, finished(1)).await?;
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::EnvLoaded { loaded: Ok(rows), .. } if rows.is_empty()))
    );
    assert!(!uv.calls().iter().any(|call| call[0] == "pip"));
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::Error(r) if r.contains("environment")))
    );
    Ok(())
}

#[tokio::test]
async fn a_reload_reads_the_projects_name_and_version() -> Result<(), Box<dyn Error>> {
    let uv = Arc::new(FakeUvClient::returning([
        Ok("[]".into()),
        Ok(r#"{"package_name": "acme-api", "version": "0.4.2", "commit_info": null}"#.into()),
    ]));
    let (events, mut received) = unbounded_channel();
    let mut runner = JobRunner::new(Arc::clone(&uv), synced(), events);
    runner.execute(Command::Reload);
    let events = events_until(&mut received, finished(1)).await?;
    assert!(events.iter().any(|e| matches!(
        e,
        Event::ProjectLoaded(Ok(info)) if info.name == "acme-api" && info.version == "0.4.2"
    )));
    assert_eq!(uv.calls()[1], ["version", "--output-format", "json"]);
    Ok(())
}

#[tokio::test]
async fn a_version_uv_cannot_read_is_told_to_the_project_tile_alone() -> Result<(), Box<dyn Error>>
{
    let said = "error: We cannot get or set dynamic project versions in: pyproject.toml";
    let uv = Arc::new(FakeUvClient::returning([
        Ok("[]".into()),
        Err(UvError::Failed {
            command: "version --output-format json".into(),
            status: ExitStatus::default(),
            stderr: said.into(),
        }),
    ]));
    let (events, mut received) = unbounded_channel();
    let mut runner = JobRunner::new(Arc::clone(&uv), synced(), events);
    runner.execute(Command::Reload);
    let events = events_until(&mut received, finished(1)).await?;
    // What uv said, without the command around it, and not on the status line.
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::ProjectLoaded(Err(r)) if r == said))
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::Error(r) if r.contains("dynamic")))
    );
    Ok(())
}

#[tokio::test]
async fn a_cancelled_load_is_not_left_loading() -> Result<(), Box<dyn Error>> {
    let uv = Arc::new(FakeUvClient::hanging_on("pip"));
    let (events, mut received) = unbounded_channel();
    let mut runner = JobRunner::new(Arc::clone(&uv), synced(), events);
    runner.execute(Command::Reload);
    events_until(&mut received, started(1)).await?;
    runner.cancel();
    let events = events_until(&mut received, finished(1)).await?;
    // The table says why it has no rows; the status line says the job was cancelled, not that
    // something failed.
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::EnvLoaded { loaded: Err(r), .. } if r == "cancelled"))
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::Error(r) if r == "cancelled"))
    );
    assert!(events.iter().any(notice("Cancelled: Loading dependencies")));
    Ok(())
}

#[tokio::test]
async fn a_mutation_waits_for_the_one_running_and_a_read_does_not() -> Result<(), Box<dyn Error>> {
    let uv = Arc::new(FakeUvClient::hanging_on("add"));
    let (events, mut received) = unbounded_channel();
    let mut runner = JobRunner::new(Arc::clone(&uv), synced(), events);
    runner.execute(Command::Add {
        requirement: "httpx".into(),
        membership: Membership::Project,
    });
    events_until(&mut received, started(1)).await?;
    runner.execute(Command::Upgrade { name: "six".into() });
    runner.execute(Command::Reload);
    // The reload runs to its end while the add still runs, and the upgrade waits behind it.
    let events = events_until(&mut received, finished(3)).await?;
    assert!(!events.iter().any(started(2)));
    assert!(!uv.calls().iter().any(|call| call[0] == "sync"));
    Ok(())
}

#[tokio::test]
async fn cancel_stops_the_running_job_and_drops_the_waiting() -> Result<(), Box<dyn Error>> {
    let uv = Arc::new(FakeUvClient::hanging_on("add"));
    let (events, mut received) = unbounded_channel();
    let mut runner = JobRunner::new(Arc::clone(&uv), synced(), events);
    runner.execute(Command::Add {
        requirement: "httpx".into(),
        membership: Membership::Project,
    });
    runner.execute(Command::Upgrade { name: "six".into() });
    events_until(&mut received, started(1)).await?;
    runner.cancel();
    let events = events_until(&mut received, finished(2)).await?;
    assert!(events.iter().any(notice("Cancelled: Adding httpx")));
    assert!(!events.iter().any(started(2)));
    // The running `uv add` was stopped, and the app is told that the project may have changed,
    // in case it had written pyproject.toml. The upgrade never ran.
    assert_eq!(uv.dropped.load(Ordering::SeqCst), 1);
    let commands: Vec<String> = uv.calls().iter().map(|call| call[..2].join(" ")).collect();
    assert_eq!(commands, ["add httpx"]);
    assert!(
        events
            .iter()
            .any(|event| matches!(event, Event::ProjectMutated))
    );
    // A job accepted after the cancel runs as usual.
    runner.execute(Command::Upgrade { name: "six".into() });
    let events = events_until(&mut received, finished(3)).await?;
    assert!(events.iter().any(notice("Updated six")));
    Ok(())
}

#[tokio::test]
async fn a_jobs_uv_lines_arrive_in_order_as_its_output() -> Result<(), Box<dyn Error>> {
    let uv = Arc::new(FakeUvClient {
        stderr: vec!["Resolved 1 package in 2ms", "+ six==1.17.0"],
        ..FakeUvClient::default()
    });
    let (events, mut received) = unbounded_channel();
    let mut runner = JobRunner::new(Arc::clone(&uv), synced(), events);
    runner.execute(Command::Upgrade { name: "six".into() });
    let events = events_until(&mut received, finished(1)).await?;
    let output: Vec<&str> = events
        .iter()
        .filter_map(|event| match event {
            Event::JobOutput(JobId(1), line) => Some(line.as_str()),
            _ => None,
        })
        .collect();
    // The update's own call.
    assert_eq!(output, ["Resolved 1 package in 2ms", "+ six==1.17.0"]);
    Ok(())
}

#[tokio::test]
async fn a_reload_writes_nothing_to_the_log() -> Result<(), Box<dyn Error>> {
    // What a read writes is an error, shown where it reads into. A dynamic version's would come
    // back after every mutation.
    let uv = Arc::new(FakeUvClient {
        stderr: vec!["error: We cannot get or set dynamic project versions in: pyproject.toml"],
        ..FakeUvClient::default()
    });
    let (events, mut received) = unbounded_channel();
    let mut runner = JobRunner::new(Arc::clone(&uv), synced(), events);
    runner.execute(Command::Reload);
    let events = events_until(&mut received, finished(1)).await?;
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, Event::JobOutput(..)))
    );
    Ok(())
}

#[test]
fn the_log_heads_each_jobs_lines_with_its_label() {
    let mut jobs = JobsState::default();
    let queued = |id, label: &str| Event::JobQueued {
        id: JobId(id),
        label: label.into(),
    };
    let line = |id, text: &str| Event::JobOutput(JobId(id), text.into());
    jobs.apply(&queued(1, "Loading dependencies"));
    jobs.apply(&Event::JobStarted(JobId(1)));
    jobs.apply(&queued(2, "Updating six"));
    jobs.apply(&Event::JobStarted(JobId(2)));
    jobs.apply(&line(2, "Resolved 2 packages"));
    jobs.apply(&line(2, "Audited 1 package"));
    jobs.apply(&line(1, "Late line"));
    let log: Vec<String> = jobs
        .log()
        .iter()
        .map(|line| match line {
            LogLine::Job(label) => format!("# {label}"),
            LogLine::Output(text) => text.clone(),
        })
        .collect();
    // A job is headed by its first line, and again when its lines resume after another's. The
    // load had written nothing until then, so it had no header.
    assert_eq!(
        log,
        [
            "# Updating six",
            "Resolved 2 packages",
            "Audited 1 package",
            "# Loading dependencies",
            "Late line",
        ]
    );
    for n in 0..LOG_LINES {
        jobs.apply(&line(2, &n.to_string()));
    }
    assert_eq!(jobs.log().len(), LOG_LINES);
    let last = (LOG_LINES - 1).to_string();
    assert!(matches!(jobs.log().back(), Some(LogLine::Output(text)) if *text == last));
}

#[test]
fn the_indicator_names_the_oldest_job_and_counts_those_waiting() {
    let mut jobs = JobsState::default();
    let queued = |id, label: &str| Event::JobQueued {
        id: JobId(id),
        label: label.into(),
    };
    jobs.apply(&queued(1, "Removing six"));
    jobs.apply(&Event::JobStarted(JobId(1)));
    jobs.apply(&queued(2, "Adding httpx"));
    // A read runs beside the mutation; it is not waiting.
    jobs.apply(&queued(3, "Loading dependencies"));
    jobs.apply(&Event::JobStarted(JobId(3)));
    assert_eq!(jobs.summary().as_deref(), Some("Removing six · 1 queued"));
    jobs.apply(&Event::JobFinished(JobId(3)));
    jobs.apply(&Event::JobFinished(JobId(1)));
    jobs.apply(&Event::JobStarted(JobId(2)));
    assert_eq!(jobs.summary().as_deref(), Some("Adding httpx"));
    // The runner sends JobFinished however a job ends: done, failed or cancelled.
    jobs.apply(&Event::JobFinished(JobId(2)));
    assert_eq!(jobs.summary(), None);
}

#[test]
fn a_reload_from_before_a_mutation_is_discarded() {
    let six = |version: &str| PyprojectDependency {
        name: "six".into(),
        groups: vec![Membership::Project],
        extras: Vec::new(),
        version_constraint: version.into(),
    };
    let loaded = |generation, version: &str| Event::PyprojectLoaded {
        empty_groups: Vec::new(),
        deletable_groups: Vec::new(),
        generation,
        loaded: Ok(vec![six(version)]),
    };
    let mut deps = DepsState::default();
    // Reload A starts, a mutation ends, and reload B starts after it. B lands first.
    deps.apply(loaded(1, "==1.16.0"));
    deps.apply(loaded(0, ">=1.17"));
    let shown: Vec<&str> = deps
        .pyproject
        .rows()
        .iter()
        .map(|dep| dep.version_constraint.as_str())
        .collect();
    assert_eq!(shown, ["==1.16.0"]);
    // A stale failure is discarded too, and a load as new as what is shown is not.
    deps.apply(Event::PyprojectLoaded {
        empty_groups: Vec::new(),
        deletable_groups: Vec::new(),
        generation: 0,
        loaded: Err("gone".into()),
    });
    deps.apply(loaded(1, "==1.15.0"));
    assert_eq!(deps.pyproject.rows()[0].version_constraint, "==1.15.0");
}
