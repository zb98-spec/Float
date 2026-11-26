//! `totalfloat mcp`: every command as an MCP tool over stdio (ARCHITECTURE.md, Bring your own agent).
//!
//! A client opens a file and gets a handle, so large files are never re-sent
//! or re-mapped.
//! Tools call the same reports as the CLI. Results are paged or summarized, so a
//! large schedule never lands whole in a model's context.

use std::collections::BTreeMap;
use std::fs::Permissions;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock};
use rmcp::transport::stdio;
use rmcp::{Peer, RoleServer, ServerHandler, ServiceExt, tool, tool_handler, tool_router};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use totalfloat_xer::RawXer;

use crate::error::{CliError, exit};
use crate::loaded::Loaded;
use crate::output::{create_new, permissions_of, refuse_existing, write_new};
use crate::{
    calendar, check, diff, edit, explain, float, import, inspect, new, page, portfolio, query,
    report, risk, schedule, variance,
};

/// Rows a single query can return, whatever the client asks for.
const MAX_ROWS: usize = 200;
/// Steps kept from each end of an explanation.
const CHAIN_ENDS: usize = 20;
/// Items kept in each list of a comparison.
const MAX_DIFF_ITEMS: usize = 100;

/// An open file. Never changed once made: `schedule` swaps in a new one, so a
/// call in progress keeps a consistent view without holding the lock.
struct Session {
    path: PathBuf,
    bytes: usize,
    /// Kept from `open`, so a moved source file doesn't strand calculated results.
    permissions: Permissions,
    loaded: Loaded,
    /// Held while calculating: each `schedule` starts from the last one's results.
    calculating: Arc<tokio::sync::Mutex<()>>,
    /// For a history store, where edits are recorded rather than kept on
    /// the handle.
    store: Option<StorePoint>,
}

/// A store handle's place in its history.
#[derive(Debug, Clone)]
struct StorePoint {
    /// The store file.
    path: PathBuf,
    /// The point opened (`plant.tfdb@what-if`), or `None` for the checked-out
    /// branch.
    at: Option<String>,
    /// The changeset the handle holds, so an edit can't record against a
    /// branch that moved since.
    head: String,
}

#[derive(Default)]
struct Sessions {
    next: u64,
    open: BTreeMap<String, Arc<Session>>,
}

/// The MCP server. Clones share their open files.
#[derive(Clone)]
pub struct Server {
    sessions: Arc<Mutex<Sessions>>,
    /// Times a file has been mapped, which is the costly step.
    mappings: Arc<AtomicUsize>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct OpenParams {
    /// Path to a `.xer` file on this machine.
    path: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct HandleParams {
    /// The handle from `open`.
    handle: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CalendarParams {
    /// The handle from `open`.
    handle: String,
    /// The calendar's name or `clndr_id`, from `inspect`.
    calendar: String,
    /// First date, as `YYYY-MM-DD`.
    from: Option<String>,
    /// Last date, as `YYYY-MM-DD`.
    to: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct FloatParams {
    /// The handle from `open`.
    handle: String,
    /// Only this project (`proj_id`). Default is every project in the file.
    project: Option<u64>,
    /// Upper bounds of the bands above zero, in working days, ascending.
    /// Default `[10, 20, 44]`.
    bands: Option<Vec<f64>>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ScheduleParams {
    /// The handle from `open`.
    handle: String,
    /// Only this project (`proj_id`). Default is every project in the file.
    project: Option<u64>,
    /// Move the data date first, as `YYYY-MM-DD HH:MM`.
    data_date: Option<String>,
    /// Ignore relationships to and from other projects for this
    /// calculation. Default false.
    #[serde(default)]
    ignore_external: bool,
    /// Find up to this many float paths for this calculation, numbered by
    /// TotalFloat's rule (docs/rules/TF-002-multiple-float-paths.md), whatever the
    /// file's options say. Default is the file's own options.
    float_paths: Option<u32>,
    /// The Activity ID the float paths end at. Needs `float_paths`.
    float_path_to: Option<String>,
    /// `total-float` or `free-float`: how the paths are numbered. Needs
    /// `float_paths`; default is the file's own setting.
    float_paths_by: Option<PathsBy>,
}

/// How float paths are numbered.
#[derive(Deserialize, JsonSchema, Clone, Copy)]
#[serde(rename_all = "kebab-case")]
enum PathsBy {
    /// By relative total float to the end activity.
    TotalFloat,
    /// By free float to the paths already found.
    FreeFloat,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ImportParams {
    /// Path to an MS Project XML file on this machine.
    path: String,
    /// A new XER file to create. An existing file is never replaced.
    output: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct NewParams {
    /// A new file to create. An existing file is never replaced.
    path: String,
    /// The project's code. Needed unless `text` is given.
    code: Option<String>,
    /// The project's name. Needed unless `text` is given.
    name: Option<String>,
    /// The data date, as `YYYY-MM-DD HH:MM`. Needed unless `text` is given.
    data_date: Option<String>,
    /// The must-finish-by date, as `YYYY-MM-DD HH:MM`.
    must_finish: Option<String>,
    /// The whole schedule as schedule text (`totalfloat-schedule 1`, one
    /// activity per line; docs/guide/text.md), instead of the fields above.
    /// Every problem comes back at its line.
    text: Option<String>,
}

#[allow(
    clippy::struct_excessive_bools,
    reason = "independent switches, each one of the edit command's flags"
)]
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct EditParams {
    /// The handle from `open`.
    handle: String,
    /// The changes, applied in order. Every one applies, or none do, unless
    /// `skip_failed` is set.
    edits: Vec<totalfloat_edit::Edit>,
    /// Only this project (`proj_id`), if an Activity ID is in several.
    project: Option<u64>,
    /// Move the data date too, as `YYYY-MM-DD HH:MM`.
    data_date: Option<String>,
    /// With `data_date`, record the progress the schedule planned before it, then
    /// apply `edits` as corrections.
    #[serde(default)]
    as_planned: bool,
    /// Report the impact and leave the handle as it was.
    #[serde(default)]
    dry_run: bool,
    /// Compare quality checks before and after. If one newly fails, the handle
    /// stays as it was.
    #[serde(default)]
    check: bool,
    /// Apply the edits that can be applied, and skip the rest with every edit
    /// that needs them. The answer lists them under `skipped`.
    #[serde(default)]
    skip_failed: bool,
    /// On a history store: why. Required there, unless `dry_run`.
    reason: Option<String>,
    /// On a history store: record the change as a proposal for a person to
    /// accept or reject, leaving the branch where it is.
    #[serde(default)]
    propose: bool,
}

/// The filters a global change takes: `query`'s, without its columns,
/// paging, logic or baseline.
const GLOBAL_FILTERS: [&str; 15] = [
    "project",
    "wbs",
    "code",
    "resource",
    "status",
    "type",
    "float_min",
    "float_max",
    "critical",
    "constrained",
    "start_from",
    "start_to",
    "finish_from",
    "finish_to",
    "text",
];

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct UsageParams {
    /// The handle from `open`.
    handle: String,
    /// The resource's short name or name.
    resource: String,
    /// `week` (from Monday, the default) or `month`.
    by: Option<crate::usage::By>,
    /// Only this project (`proj_id`); default is every project in the file.
    project: Option<u64>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct GlobalChangeParams {
    /// The handle from `open`.
    handle: String,
    /// The activities to change, as `query`'s filters: `wbs`, `code`,
    /// `resource`, `status`, `type`, `float_min`, `float_max`, `critical`,
    /// `constrained`, `start_from`, `start_to`, `finish_from`, `finish_to`,
    /// `text`, `project`.
    filter: serde_json::Map<String, serde_json::Value>,
    /// The changes, each as `duration=5d`, `duration+=2d`, `duration-=1d`,
    /// `calendar=NAME`, `constraint=KIND@DATE`, `code=TYPE=VALUE`,
    /// `udf=FIELD=VALUE`, `prefix=TEXT` or `suffix=TEXT`; or `duration`,
    /// `remaining_duration`, `constraint_date` or `udf:FIELD` set to an
    /// expression over the activity's own fields, such as
    /// `remaining_duration=original_duration*0.5`.
    set: Vec<String>,
    /// Report the impact, listing every change, and leave the handle as it was.
    #[serde(default)]
    dry_run: bool,
    /// Compare quality checks before and after. If one newly fails, the handle
    /// stays as it was.
    #[serde(default)]
    check: bool,
    /// On a history store: why. Required there, unless `dry_run`.
    reason: Option<String>,
    /// On a history store: record the change as a proposal.
    #[serde(default)]
    propose: bool,
}

#[allow(
    clippy::struct_excessive_bools,
    reason = "independent switches, each one of the edit command's flags"
)]
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ApplyTextParams {
    /// The handle from `open`.
    handle: String,
    /// The schedule as schedule text (`totalfloat-schedule 1`), usually an
    /// `export` with format tfs, edited.
    text: String,
    /// What to do with activities and links the text leaves out: refuse (the
    /// default, listing them), keep, or delete.
    #[serde(default)]
    missing: crate::apply::Choice,
    /// Only this project (`proj_id`), if the file has several.
    project: Option<u64>,
    /// Report the impact and leave the handle as it was.
    #[serde(default)]
    dry_run: bool,
    /// Compare quality checks before and after.
    #[serde(default)]
    check: bool,
    /// Apply the edits that can be applied, and skip the rest.
    #[serde(default)]
    skip_failed: bool,
    /// On a history store: why. Required there, unless `dry_run`.
    reason: Option<String>,
    /// On a history store: record the change as a proposal.
    #[serde(default)]
    propose: bool,
}

/// An edit's settings, owned so they can go to the blocking thread.
#[derive(Debug, Clone)]
struct EditSettings {
    project: Option<u64>,
    data_date: Option<String>,
    as_planned: bool,
    check: bool,
    skip_failed: bool,
}

impl EditSettings {
    fn options(&self) -> edit::Options<'_> {
        edit::Options {
            project: self.project,
            data_date: self.data_date.as_deref(),
            as_planned: self.as_planned,
            check: self.check,
            skip_failed: self.skip_failed,
        }
    }
}

#[derive(Serialize)]
struct EditAnswer {
    #[serde(flatten)]
    report: edit::Report,
    /// True if the edits stayed on the handle.
    kept: bool,
}

#[derive(Deserialize, JsonSchema)]
struct QueryParams {
    /// The handle from `open`.
    handle: String,
    #[serde(flatten)]
    query: query::Query,
    /// Anything else. A misspelled filter must fail, not return everything.
    #[serde(flatten)]
    #[schemars(skip)]
    unknown: BTreeMap<String, serde_json::Value>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ExplainParams {
    /// The handle from `open`.
    handle: String,
    /// The activity's `task_code` or `task_id`.
    activity: String,
    /// Which value to explain. Default is `early_start`.
    field: Option<explain::Field>,
    /// Only look in this project (`proj_id`).
    project: Option<u64>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CheckParams {
    /// The handle from `open`.
    handle: String,
    /// Only this project (`proj_id`). Default is every project in the file.
    project: Option<u64>,
    /// `markdown` or `html` for the report as a document. Default is JSON.
    format: Option<page::Format>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ReportParams {
    /// The handle from `open`.
    handle: String,
    /// A built-in template: `look-ahead` or `critical-path`. Give this or
    /// `text`.
    template: Option<String>,
    /// A template's TOML, as a template file holds it. Give this or
    /// `template`.
    text: Option<String>,
    /// Only this project (`proj_id`). Default is every project in the file.
    project: Option<u64>,
    /// `markdown` (the default), `csv` or `html`.
    format: Option<report::Format>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct DiffParams {
    /// The handle of the earlier schedule.
    from: String,
    /// The handle of the later schedule.
    to: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct PortfolioParams {
    /// A portfolio file (`totalfloat.portfolio/1`), such as `plan.portfolio.toml`.
    path: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CapacityParams {
    /// A portfolio file (`totalfloat.portfolio/1`), such as `plan.portfolio.toml`.
    path: String,
    /// `week` (the default) or `month`.
    by: Option<crate::usage::By>,
    /// Only this resource, by short name.
    resource: Option<String>,
    /// Only this role, by short name.
    role: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct RiskParams {
    /// The handle from `open`.
    handle: String,
    /// Estimates, as a `totalfloat.risk-inputs/1` file holds them: `default`
    /// (a range in percent, such as {"min": -10, "max": 25}), `estimates` by
    /// Activity ID ({"min": "8d", "likely": "10d", "max": "15d",
    /// "distribution": "pert"}), `udfs` (activity fields holding days) and
    /// `correlations` (`[{"activities": ["A1", "A2"], "coefficient": 0.5}]`) and
    /// `risks` (`[{"code": "R-01", "probability": 0.3, "impacts": {"A1":
    /// {"min": "2d", "max": "10d"}}}]`).
    inputs: risk::InputsFile,
    /// Only this project (`proj_id`); needed if the file has several.
    project: Option<u64>,
    /// Iterations. Default 1,000, at most 10,000.
    iterations: Option<u32>,
    /// The seed. Default 1; the same seed gives the same result.
    seed: Option<u64>,
    /// Give the chance of finishing by this date, as `YYYY-MM-DD HH:MM`.
    target: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct VarianceParams {
    /// The handle of the current schedule.
    current: String,
    /// The handle of the baseline: an earlier update or the approved plan.
    baseline: String,
    /// How many slips and float losses to list. Default 10.
    top: Option<usize>,
    /// `markdown` or `html` for the report as a document. Default is JSON.
    format: Option<page::Format>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ExportParams {
    /// The handle from `open`.
    handle: String,
    /// A new file to create. An existing file is never replaced.
    path: String,
    /// Another format to write. Default is XER.
    format: Option<import::Format>,
    /// Only this project (`proj_id`), for another format.
    project: Option<u64>,
}

#[derive(Serialize)]
struct Opened {
    handle: String,
    file: String,
    projects: Vec<OpenedProject>,
}

#[derive(Serialize)]
struct OpenedProject {
    id: u64,
    code: String,
    activities: usize,
    data_date: Option<String>,
    scheduled_finish: Option<String>,
}

#[derive(Serialize)]
struct Exported {
    path: String,
}

#[derive(Serialize)]
struct Closed {
    closed: String,
}

#[derive(Serialize)]
struct ToolError<'a> {
    schema: &'static str,
    error: &'a CliError,
}

/// The client's name from MCP initialize, recorded as the agent's.
fn client_name(peer: &Peer<RoleServer>) -> String {
    peer.peer_info()
        .map(|info| info.client_info.name.clone())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| "mcp client".to_owned())
}

fn pretty(value: &impl Serialize) -> String {
    serde_json::to_string_pretty(value).unwrap_or_default()
}

fn answer<T: Serialize>(result: Result<T, CliError>) -> CallToolResult {
    match result {
        Ok(report) => CallToolResult::success(vec![ContentBlock::text(pretty(&report))]),
        Err(error) => CallToolResult::error(vec![ContentBlock::text(pretty(&ToolError {
            schema: "totalfloat.error/1",
            error: &error,
        }))]),
    }
}

/// A report as JSON, or as a document (Markdown or HTML) to return as it is.
enum Written<T> {
    Json(T),
    Document(String),
}

fn written<T: Serialize>(result: Result<Written<T>, CliError>) -> CallToolResult {
    match result {
        Ok(Written::Json(report)) => answer(Ok(report)),
        Ok(Written::Document(text)) => CallToolResult::success(vec![ContentBlock::text(text)]),
        Err(error) => answer::<()>(Err(error)),
    }
}

fn no_handle(handle: &str) -> CliError {
    CliError::new(
        "no-such-handle",
        format!("no open file has the handle {handle}"),
        "call `open` first; handles end with the session or `close`",
    )
}

impl Server {
    fn new() -> Self {
        Self {
            sessions: Arc::default(),
            mappings: Arc::default(),
        }
    }

    fn lock(&self) -> Result<MutexGuard<'_, Sessions>, CliError> {
        self.sessions.lock().map_err(|_| {
            CliError::new(
                "session-unavailable",
                "an earlier call failed while holding the open files",
                "restart the server",
            )
        })
    }

    fn session(&self, handle: &str) -> Result<Arc<Session>, CliError> {
        let sessions = self.lock()?;
        let session = sessions.open.get(handle);
        session.cloned().ok_or_else(|| no_handle(handle))
    }

    /// Does the work off the runtime and outside the lock, so a slow call
    /// doesn't block others, and a panic is an error instead of a silent client.
    async fn run<T: Send + 'static>(
        work: impl FnOnce() -> Result<T, CliError> + Send + 'static,
    ) -> Result<T, CliError> {
        tokio::task::spawn_blocking(work).await.unwrap_or_else(|_| {
            Err(CliError::new(
                "tool-failed",
                "the tool stopped unexpectedly",
                "this is a bug; open files are unaffected",
            ))
        })
    }

    async fn on_session<T: Send + 'static>(
        &self,
        handle: &str,
        work: impl FnOnce(&Session) -> Result<T, CliError> + Send + 'static,
    ) -> Result<T, CliError> {
        let session = self.session(handle)?;
        Self::run(move || work(&session)).await
    }

    fn load(mappings: &AtomicUsize, raw: RawXer) -> Loaded {
        mappings.fetch_add(1, Ordering::Relaxed);
        Loaded::new(raw)
    }

    fn insert(&self, session: Session) -> Result<String, CliError> {
        let mut sessions = self.lock()?;
        sessions.next += 1;
        let handle = format!("s{}", sessions.next);
        sessions.open.insert(handle.clone(), Arc::new(session));
        Ok(handle)
    }

    async fn open_file(&self, path: String) -> Result<Opened, CliError> {
        let mappings = Arc::clone(&self.mappings);
        let (session, projects) = Self::run(move || {
            let path = PathBuf::from(path);
            let store = match crate::location::locate(&path) {
                crate::location::Located::Store { path, at } => {
                    let opened = crate::history::open(&path)?;
                    let head = crate::history::resolve_point(&opened, &path, at.as_deref())?;
                    Some(StorePoint {
                        path,
                        at,
                        head: head.to_string(),
                    })
                }
                crate::location::Located::File(_) => None,
            };
            let raw = crate::location::read(&path)?;
            let bytes = raw.source().len();
            let loaded = Self::load(&mappings, raw);
            let projects = loaded
                .schedules
                .iter()
                .map(|s| OpenedProject {
                    id: s.project.id.0,
                    code: s.project.short_name.clone(),
                    activities: s.activities.len(),
                    data_date: s.project.data_date.map(|d| d.to_string()),
                    scheduled_finish: s.project.stored.scheduled_finish.map(|d| d.to_string()),
                })
                .collect();
            let permissions = permissions_of(&path)?;
            let session = Session {
                path,
                bytes,
                permissions,
                loaded,
                calculating: Arc::default(),
                store,
            };
            Ok((session, projects))
        })
        .await?;
        let file = session.path.display().to_string();
        Ok(Opened {
            handle: self.insert(session)?,
            file,
            projects,
        })
    }

    /// A session holding `raw`, with results in place.
    fn replaced(session: &Session, raw: RawXer, mappings: &AtomicUsize) -> Session {
        let mut written = Vec::with_capacity(session.bytes);
        let bytes = if raw.write(&mut written).is_ok() {
            written.len()
        } else {
            session.bytes
        };
        Session {
            path: session.path.clone(),
            bytes,
            permissions: session.permissions.clone(),
            loaded: Self::load(mappings, raw),
            calculating: Arc::clone(&session.calculating),
            store: session.store.clone(),
        }
    }

    /// Puts a new session on a handle. A handle closed meanwhile stays closed.
    fn swap(&self, handle: &str, replacement: Session) -> Result<(), CliError> {
        let mut sessions = self.lock()?;
        if let Some(slot) = sessions.open.get_mut(handle) {
            *slot = Arc::new(replacement);
        }
        Ok(())
    }

    async fn apply_text_edits(
        &self,
        params: ApplyTextParams,
        client: String,
    ) -> Result<EditAnswer, CliError> {
        let ApplyTextParams {
            handle,
            text,
            missing,
            project,
            dry_run,
            check,
            skip_failed,
            reason,
            propose,
        } = params;
        let session = self.session(&handle)?;
        let found = Self::run(move || {
            crate::apply::from_text(&session.loaded, project, ("text", &text), missing, true)
        })
        .await?;
        if found.edits.is_empty() && found.data_date.is_none() {
            return Err(CliError::new(
                "nothing-to-change",
                "the text matches the schedule; nothing to change",
                "edit the text first",
            ));
        }
        let places = found.places.clone();
        let warnings = found.warnings.clone();
        let params = EditParams {
            handle,
            edits: found.edits,
            project,
            data_date: found.data_date,
            as_planned: false,
            dry_run,
            check,
            skip_failed,
            reason,
            propose,
        };
        let mut answer = self
            .apply_edits(params, client)
            .await
            .map_err(|e| crate::apply::place_problems(e, &places))?;
        for warning in warnings {
            answer.report.warn(warning);
        }
        Ok(answer)
    }

    /// Expands a global change into edits and applies them as `edit` does.
    async fn change_globally(
        &self,
        params: GlobalChangeParams,
        client: String,
    ) -> Result<EditAnswer, CliError> {
        let GlobalChangeParams {
            handle,
            filter,
            set,
            dry_run,
            check,
            reason,
            propose,
        } = params;
        if let Some(name) = filter
            .keys()
            .find(|k| !GLOBAL_FILTERS.contains(&k.as_str()))
        {
            return Err(CliError::new(
                "unknown-argument",
                format!("the filter has no argument {name}"),
                format!("filters: {}", GLOBAL_FILTERS.join(", ")),
            ));
        }
        let query: query::Query = serde_json::from_value(serde_json::Value::Object(filter))
            .map_err(|error| {
                CliError::new(
                    "invalid-filter",
                    format!("the filter can't be read: {error}"),
                    "it takes query's filters, as query does",
                )
            })?;
        let changes = set
            .iter()
            .map(|text| crate::global::parse_set(text).map(|c| (text.clone(), c)))
            .collect::<Result<Vec<_>, _>>()?;
        let session = self.session(&handle)?;
        let expanded = Self::run(move || {
            let file = session.path.display().to_string();
            crate::global::expand(&file, &session.loaded, query, &changes)
        })
        .await?;
        let params = EditParams {
            handle,
            edits: expanded.edits,
            project: Some(expanded.project),
            data_date: None,
            as_planned: false,
            dry_run,
            check,
            skip_failed: false,
            reason,
            propose,
        };
        self.apply_edits(params, client).await
    }

    async fn apply_edits(
        &self,
        params: EditParams,
        client: String,
    ) -> Result<EditAnswer, CliError> {
        let calculating = Arc::clone(&self.session(&params.handle)?.calculating);
        let _turn = calculating.lock().await;
        let session = self.session(&params.handle)?;
        let mappings = Arc::clone(&self.mappings);
        let EditParams {
            handle,
            edits,
            project,
            data_date,
            as_planned,
            dry_run,
            check,
            skip_failed,
            reason,
            propose,
        } = params;
        let settings = EditSettings {
            project,
            data_date,
            as_planned,
            check,
            skip_failed,
        };
        if let Some(point) = session.store.clone() {
            let asked = (reason, propose, dry_run, client);
            let (report, replacement) = Self::run(move || {
                Self::record_edits(&session, &point, &edits, &settings, asked, &mappings)
            })
            .await?;
            let kept = replacement.is_some();
            if let Some(replacement) = replacement {
                self.swap(&handle, replacement)?;
            }
            return Ok(EditAnswer { report, kept });
        }
        if reason.is_some() || propose {
            return Err(CliError::new(
                "not-a-store",
                "reason and propose are kept with a change in a history store, and this handle is a file",
                "open a store (`history_init` makes one) to record changes",
            ));
        }
        let EditSettings {
            project,
            data_date,
            as_planned,
            check,
            skip_failed,
        } = settings;
        let (report, replacement) = Self::run(move || {
            let file = session.path.display().to_string();
            let options = edit::Options {
                project,
                data_date: data_date.as_deref(),
                as_planned,
                check,
                skip_failed,
            };
            let edited = edit::report(&file, &session.loaded, &edits, options)?;
            let keep = !dry_run && !edited.report.blocked();
            let replacement = keep.then(|| Self::replaced(&session, edited.raw, &mappings));
            Ok((edited.report, replacement))
        })
        .await?;
        if let Some(replacement) = replacement {
            self.swap(&handle, replacement)?;
        }
        let kept = !dry_run && !report.blocked();
        Ok(EditAnswer { report, kept })
    }

    /// Edits a store handle's branch and records the change as the client's,
    /// then moves the handle to what it recorded.
    fn record_edits(
        session: &Session,
        point: &StorePoint,
        edits: &[totalfloat_edit::Edit],
        settings: &EditSettings,
        (reason, propose, dry_run, client): (Option<String>, bool, bool, String),
        mappings: &AtomicUsize,
    ) -> Result<(edit::Report, Option<Session>), CliError> {
        let reason = if dry_run {
            reason
        } else {
            Some(stores::reason(reason)?)
        };
        let asked = crate::history::Asked {
            message: reason.as_deref(),
            author: None,
            propose,
        };
        let place = (
            point.path.as_path(),
            point.at.as_deref(),
            Some(point.head.as_str()),
        );
        let (report, raw) = crate::history::as_agent(&client, || {
            edit::edit_store(place, edits, (settings.options(), None), (asked, dry_run))
        })?;
        let replacement = match report.recorded() {
            Some(recorded) if recorded.proposal.is_none() => {
                let mut replaced = Self::replaced(session, raw, mappings);
                replaced.store = Some(StorePoint {
                    head: recorded.id.clone(),
                    ..point.clone()
                });
                Some(replaced)
            }
            _ => None,
        };
        Ok((report, replacement))
    }

    async fn calculate(&self, params: ScheduleParams) -> Result<schedule::Report, CliError> {
        let calculating = Arc::clone(&self.session(&params.handle)?.calculating);
        let _turn = calculating.lock().await;
        let session = self.session(&params.handle)?;
        let mappings = Arc::clone(&self.mappings);
        let overrides = schedule_overrides(&params)?;
        let handle = params.handle;
        let (project, data_date) = (params.project, params.data_date);
        let (report, replacement) = Self::run(move || {
            let file = session.path.display().to_string();
            let (report, scheduled) = schedule::report(
                &file,
                &session.loaded,
                (project, data_date.as_deref()),
                (true, &overrides),
            )?;
            let replacement = scheduled.map(|raw| Self::replaced(&session, raw, &mappings));
            Ok((report, replacement))
        })
        .await?;
        if let Some(replacement) = replacement {
            self.swap(&handle, replacement)?;
        }
        Ok(report)
    }
}

/// The run-only settings a `schedule` call asks for.
fn schedule_overrides(params: &ScheduleParams) -> Result<schedule::Overrides, CliError> {
    if params.float_paths.is_none()
        && (params.float_path_to.is_some() || params.float_paths_by.is_some())
    {
        return Err(CliError::new(
            "bad-float-paths",
            "float_path_to and float_paths_by need float_paths",
            "give float_paths, how many paths to find",
        ));
    }
    Ok(schedule::Overrides {
        ignore_external: params.ignore_external,
        float_paths: params.float_paths.map(|paths| schedule::FloatPathAsk {
            paths,
            to: params.float_path_to.clone(),
            by: params.float_paths_by.map(|by| match by {
                PathsBy::TotalFloat => totalfloat_model::FloatPathMode::TotalFloat,
                PathsBy::FreeFloat => totalfloat_model::FloatPathMode::FreeFloat,
            }),
        }),
    })
}

fn export(session: &Session, path: &Path) -> Result<Exported, CliError> {
    refuse_existing(path)?;
    write_new(&session.loaded.raw, session.permissions.clone(), path)?;
    Ok(Exported {
        path: path.display().to_string(),
    })
}

#[tool_router]
impl Server {
    #[tool(
        description = "Convert an MS Project XML file to a new XER, then `open` that. Reports what \
        didn't translate in `notes`, which the user should see. Dates follow TotalFloat's own \
        rules, so they can differ from MS Project's. .mpp files aren't read. Example: {\"path\": \
        \"/data/plan.xml\", \"output\": \"/data/plan.xer\"}."
    )]
    async fn import(&self, Parameters(params): Parameters<ImportParams>) -> CallToolResult {
        answer(
            Self::run(move || import::to_file(Path::new(&params.path), Path::new(&params.output)))
                .await,
        )
    }

    #[tool(
        name = "new",
        description = "Start a new schedule in a new XER file and get a handle for it: a project, a \
        root WBS node named for its code, and a five-day calendar called Standard 5-Day. path is the \
        empty starting file. Build on the handle with `edit`, then `export` to a different new \
        path, because edits stay on the handle and an existing path is refused. Or give the \
        whole schedule as `text` (schedule text, docs/guide/text.md) and path is built from it. \
        Example: {\"path\": \"/data/plant.xer\", \"code\": \"PLANT\", \"name\": \"New Plant\", \
        \"data_date\": \"2026-03-02 08:00\"}."
    )]
    async fn new_schedule(&self, Parameters(params): Parameters<NewParams>) -> CallToolResult {
        let path = params.path.clone();
        let created = Self::run(move || {
            let target = Path::new(&params.path);
            refuse_existing(target)?;
            if let Some(text) = &params.text {
                if params.code.is_some()
                    || params.name.is_some()
                    || params.data_date.is_some()
                    || params.must_finish.is_some()
                {
                    return Err(CliError::new(
                        "invalid-argument",
                        "give either text or code, name, data_date and must_finish, not both",
                        "the text's project line, data-date and must-finish give those",
                    ));
                }
                let compiled = crate::build::compile("text", text, true)?;
                let (raw, _, _) = crate::build::build("text", text, &compiled, true)?;
                return create_new(&raw, target);
            }
            let (Some(code), Some(name), Some(data_date)) =
                (&params.code, &params.name, &params.data_date)
            else {
                return Err(CliError::new(
                    "invalid-argument",
                    "a new schedule needs code, name and data_date, or text",
                    "give the project's code, name and data date, or the whole schedule as text",
                ));
            };
            let spec = new::Spec {
                code,
                name,
                data_date,
                must_finish: params.must_finish.as_deref(),
                calendar: None,
            };
            create_new(&new::start(&spec)?, target)
        })
        .await;
        match created {
            Ok(()) => answer(self.open_file(path).await),
            Err(error) => answer::<Opened>(Err(error)),
        }
    }

    #[tool(
        description = "Open an XER file, or a history store (plant.tfdb, or plant.tfdb@BRANCH for \
        a branch, baseline or changeset), and get a handle for the other tools, with each \
        project's activity count, data date and stored finish. On a store, `edit` records each \
        change in its history. Example: {\"path\": \"/data/plant.xer\"}."
    )]
    async fn open(&self, Parameters(params): Parameters<OpenParams>) -> CallToolResult {
        answer(self.open_file(params.path).await)
    }

    #[tool(
        description = "Summarize an open file: export version, encoding, projects with status \
        counts and finish, tables, calendars, activity code types with their values, resources, \
        and what couldn't be mapped."
    )]
    async fn inspect(&self, Parameters(params): Parameters<HandleParams>) -> CallToolResult {
        let report = self.on_session(&params.handle, |session| {
            let file = session.path.display().to_string();
            Ok(inspect::report(&file, session.bytes, &session.loaded))
        });
        answer(report.await)
    }

    #[tool(
        description = "Show what a calendar works: hours per day, the work week, and exception \
        dates such as holidays. With both from and to, also lists each day with its hours, so you \
        can see whether a date is worked. Example: {\"handle\": \"s1\", \"calendar\": \
        \"Standard\", \"from\": \"2026-12-20\", \"to\": \"2027-01-03\"}."
    )]
    async fn calendar(&self, Parameters(params): Parameters<CalendarParams>) -> CallToolResult {
        let CalendarParams {
            handle,
            calendar,
            from,
            to,
        } = params;
        let report = self.on_session(&handle, move |session| {
            let range = calendar::Range {
                from: from.as_deref(),
                to: to.as_deref(),
            };
            let file = session.path.display().to_string();
            calendar::report(&file, &session.loaded, &calendar, &range)
        });
        answer(report.await)
    }

    #[tool(
        description = "How unfinished work spreads across total-float bands, in working days: \
        below zero, zero, then up to each bound in bands. Counts each band, its share, and the \
        first 50 activity codes in it, lowest float first. Reads the stored float, so run \
        `schedule` first for fresh results. Example: {\"handle\": \"s1\", \"bands\": [5, 10, 20]}."
    )]
    async fn float(&self, Parameters(params): Parameters<FloatParams>) -> CallToolResult {
        let FloatParams {
            handle,
            project,
            bands,
        } = params;
        let report = self.on_session(&handle, move |session| {
            let bounds = match bands {
                Some(days) => float::bounds(
                    &days
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                )?,
                None => float::DEFAULT_BOUNDS.to_vec(),
            };
            let file = session.path.display().to_string();
            float::report(&file, &session.loaded, project, &bounds)
        });
        answer(report.await)
    }

    #[tool(
        description = "Calculate dates, float and the critical path, and keep the results on this handle, \
        so `query`, `explain` and `export` see them. Reports each project's finish, critical count, \
        what changed by field and activity, and `starts_at`, where differences begin. Stops before changing anything if a project can't be \
        calculated. Example: {\"handle\": \"s1\", \"data_date\": \"2026-02-02 08:00\"}."
    )]
    async fn schedule(&self, Parameters(params): Parameters<ScheduleParams>) -> CallToolResult {
        answer(self.calculate(params).await)
    }

    #[tool(
        description = "Change the schedule and recalculate: use this for any what-if. On a link \
        edit, name an activity in another project PROJECT:CODE. Each edit \
        has an op: set_duration (activity, original, remaining), set_constraint (activity, \
        constraint, date, secondary), clear_constraint, set_calendar, set_name, set_type, \
        set_expected_finish, add_activity (code, name, wbs, activity_type, calendar, duration), \
        delete_activity, set_progress (activity, actual_start, actual_finish, remaining, percent), \
        clear_progress, add_wbs (code, name, parent), rename_wbs (wbs, code, name), move_wbs (wbs, \
        parent), set_wbs (activity, wbs), set_udf (field, value, and activity, wbs, resource, or activity and resource), clear_udf, add_udf_type (field, for, data_type), add_step (activity, step, weight, percent), set_step (activity, step, name, weight, percent), complete_step, delete_step (activity, step), add_note (activity or wbs, topic, text), set_note, delete_note (activity or wbs, topic), add_obs (obs, parent, description), set_wbs_obs (wbs, obs), add_resource (resource, name, resource_type, \
        units_per_hour, calendar, max_units_per_hour), set_resource_limit (resource, max_units_per_hour: \
        a number or null to clear, from), add_role (role, name, parent, max_units_per_hour), set_role_limit (role, max_units_per_hour, from), assign_role (activity, role, units), unassign_role (activity, role), assign_resource (activity, resource), unassign_resource, set_assignment_units (activity, resource, actual_units, remaining_units), set_curve (activity, resource, curve: a curve's name, or none to spread evenly), set_spread (activity, resource, period: day|week|month, start, units: remaining units per period), set_code (activity, code_type, value), clear_code, \
        add_code_type (name, scope, max_length), add_code_value (code_type, \
        value, description), set_project_code (code_type, value), clear_project_code, \
        add_project_code_type (name, max_length), add_project_code_value (code_type, value, \
        description), add_calendar (name, copy_of), add_calendar_exception (calendar, date, shifts), remove_calendar_exception, \
        set_work_week (calendar, day, shifts), set_hours_per_day (calendar, hours), set_hours_per_period (calendar, period: week|month|year, hours), add_link (predecessor, successor, link_type, lag), remove_link, change_link. Spans read 120h, 15d, 2w, 1m (a month) or 1y, by the calendar's hours in each. Reports what each edit did, the finish before and \
        after, milestones moved, and what entered or left critical, against the file calculated \
        as it stands. Set dry_run to look without changing the handle, and check to compare quality checks before and after: if one newly fails, the handle stays as it was. Otherwise the change stays \
        on the handle, so query, explain, diff and export see it. The file on disk never changes. \
        On a history store's handle, the change is recorded in the store as yours, an agent's, \
        and needs a reason; set propose to leave it waiting for a person to accept or reject, \
        which is the right choice for any schedule someone else is answerable for. It starts \
        from the branch's recorded file, which must still be what the handle holds; if the \
        branch moved, open the store again. \
        Example: {\"handle\": \"s1\", \"edits\": [{\"op\": \"set_duration\", \"activity\": \
        \"A1000\", \"original\": \"15d\"}], \"dry_run\": true}."
    )]
    async fn edit(
        &self,
        Parameters(params): Parameters<EditParams>,
        peer: Peer<RoleServer>,
    ) -> CallToolResult {
        answer(self.apply_edits(params, client_name(&peer)).await)
    }

    #[tool(
        description = "A resource's usage profile: its units per week or month, actual and \
        remaining, against its limit (its maximum units per hour over its calendar's work \
        hours). Remaining units are spread over \
        each activity's stored remaining dates, so schedule first. Units are hours for labor \
        and nonlabor. over marks a period above the limit. Example: {\"handle\": \"s1\", \
        \"resource\": \"CREW\", \"by\": \"week\"}."
    )]
    async fn usage(&self, Parameters(params): Parameters<UsageParams>) -> CallToolResult {
        let UsageParams {
            handle,
            resource,
            by,
            project,
        } = params;
        let report = self.on_session(&handle, move |session| {
            let file = session.path.display().to_string();
            crate::usage::report(
                &file,
                &session.loaded,
                &resource,
                by.unwrap_or_default(),
                project,
            )
        });
        answer(report.await)
    }

    #[tool(
        description = "Global change: make one change to every activity a filter finds. \
        filter takes query's filters (wbs, code \"TYPE=VALUE\", \
        resource, status, type, float_min, float_max, critical, constrained, start_from, \
        start_to, finish_from, finish_to, text, project) and finds the same activities query \
        does. set lists the changes: duration=5d, duration+=2d or duration-=1d (days of each \
        activity's calendar; milestones, level of effort and WBS summaries are skipped), \
        calendar=NAME, constraint=KIND@DATE, code=TYPE=VALUE, udf=FIELD=VALUE, prefix=TEXT, \
        suffix=TEXT, or duration, remaining_duration, constraint_date or udf:FIELD set to an expression over \
        the activity's own fields, such as remaining_duration=original_duration*0.5 or \
        constraint_date=early_start+5d (a date plus a duration counts working time on its \
        calendar). They become ordinary edits, one an activity, that apply together or not \
        at all, and the answer is the edit tool's, listing each under applied. Use dry_run to \
        see every change first. Example: {\"handle\": \"s1\", \"filter\": {\"wbs\": \"CIV\", \
        \"status\": [\"not-started\"]}, \"set\": [\"duration+=2d\"], \"dry_run\": true}."
    )]
    async fn global_change(
        &self,
        Parameters(params): Parameters<GlobalChangeParams>,
        peer: Peer<RoleServer>,
    ) -> CallToolResult {
        answer(self.change_globally(params, client_name(&peer)).await)
    }

    #[tool(
        name = "apply_text",
        description = "Make an open schedule match schedule text: the difference becomes edits, \
        which then behave exactly as `edit`'s do (all or nothing, dry_run, check, skip_failed, \
        and on a history store reason and propose). Get the text with `export` format tfs, edit \
        it, and apply it. Activities and links the text leaves out are refused, listed, unless \
        missing is keep or delete. Problems in the text come back at their lines. \
        Example: {\"handle\": \"s1\", \"text\": \"totalfloat-schedule 1 ...\", \"dry_run\": true}."
    )]
    async fn apply_text(
        &self,
        Parameters(params): Parameters<ApplyTextParams>,
        peer: Peer<RoleServer>,
    ) -> CallToolResult {
        answer(self.apply_text_edits(params, client_name(&peer)).await)
    }

    #[tool(
        description = "Find activities without reading the schedule. Filters combine: project, \
        wbs, code (\"TYPE=VALUE\", from `inspect`), resource, status, type, float_min, float_max (hours), critical, \
        constrained, start_from, start_to, finish_from, finish_to, text. any takes groups of \
        those filters, each an object such as {\"critical\": true}: an activity has to meet at \
        least one whole group, besides the rest. Date filters leave out \
        finished work unless status asks for it. Or follow logic with preds, succs or \
        driving_path_to, which take a task_code or task_id. To compare with a baseline, give \
        baseline, the path of an earlier XER, and baseline_project if it has several projects: \
        it adds baseline_start, baseline_finish and finish_variance_hours (negative is late), and \
        counts what was added and deleted. Choose columns, or [\"all\"]. Returns \
        `total` and a page: limit defaults to 50 and is capped at 200, so use offset for more. \
        Example: {\"handle\": \"s1\", \"critical\": true, \"status\": [\"not-started\"], \"limit\": 10}."
    )]
    async fn query(&self, Parameters(params): Parameters<QueryParams>) -> CallToolResult {
        let QueryParams {
            handle,
            mut query,
            unknown,
        } = params;
        query.limit = query.limit.min(MAX_ROWS);
        let report = self.on_session(&handle, move |session| {
            if let Some(name) = unknown.keys().next() {
                return Err(CliError::new(
                    "unknown-argument",
                    format!("`query` has no argument {name}"),
                    "the tool's input schema lists the arguments",
                ));
            }
            let file = session.path.display().to_string();
            query::report(&file, &session.loaded, &query)
        });
        answer(report.await)
    }

    #[tool(
        description = "Say why a date is what it is, step by step back to its origin, such as the \
        data date or a constraint. field is early_start, early_finish, late_start, late_finish or \
        total_float. Example: {\"handle\": \"s1\", \"activity\": \"A1000\", \"field\": \"early_finish\"}."
    )]
    async fn explain(&self, Parameters(params): Parameters<ExplainParams>) -> CallToolResult {
        let ExplainParams {
            handle,
            activity,
            field,
            project,
        } = params;
        let report = self.on_session(&handle, move |session| {
            let file = session.path.display().to_string();
            let field = field.unwrap_or(explain::Field::EarlyStart);
            let (mut report, _) =
                explain::report(&file, &session.loaded, &activity, field, project)?;
            report.keep_ends(CHAIN_ENDS);
            Ok(report)
        });
        answer(report.await)
    }

    #[tool(
        description = "Run schedule quality checks: the DCMA 14-point assessment plus open ends, \
        out-of-sequence progress, constraints overriding logic, long lags and SF links. Each check \
        reports pass, fail or not-applicable, a measure, and up to 50 offenders. JSON unless \
        format is markdown or html, which return the report itself. Example: \
        {\"handle\": \"s1\"}."
    )]
    async fn check(&self, Parameters(params): Parameters<CheckParams>) -> CallToolResult {
        let CheckParams {
            handle,
            project,
            format,
        } = params;
        let report = self.on_session(&handle, move |session| {
            let file = session.path.display().to_string();
            let thresholds = totalfloat_check::Thresholds::default();
            let report = check::report(
                &file,
                &session.loaded,
                project,
                (&thresholds, &check::Own::default()),
            )?;
            Ok(match format {
                Some(format) => Written::Document(check::render(&report, &session.loaded, format)),
                None => Written::Json(report),
            })
        });
        written(report.await)
    }

    #[tool(
        description = "Write a report from a template: activities filtered, grouped, sorted and \
        totalled, as `report` prints it. Give template, a built-in name (look-ahead: unfinished \
        work starting within 21 days of the data date, by WBS; critical-path), or text, a \
        template's TOML: title, columns (query's), [filter] (query's filters), group, sort \
        ([{column, descending}]), totals (columns to add up per group and overall) and \
        days_from_data_date. Returns the report itself, Markdown unless format is csv or html. \
        Example: {\"handle\": \"s1\", \"template\": \"critical-path\"}."
    )]
    async fn report(&self, Parameters(params): Parameters<ReportParams>) -> CallToolResult {
        let ReportParams {
            handle,
            template,
            text,
            project,
            format,
        } = params;
        let written = self.on_session(&handle, move |session| {
            let template = match (template, text) {
                (Some(name), None) => report::Template::built_in(&name)?,
                (None, Some(text)) => report::Template::parse(&text, "text")?,
                _ => {
                    return Err(CliError::new(
                        "missing-argument",
                        "give one of template (a built-in name) or text (a template's TOML)",
                        "for example {\"handle\": \"s1\", \"template\": \"look-ahead\"}",
                    ));
                }
            };
            let file = session.path.display().to_string();
            report::render(
                &template,
                (&file, &session.loaded),
                project,
                format.unwrap_or(report::Format::Markdown),
            )
        });
        match written.await {
            Ok(text) => CallToolResult::success(vec![ContentBlock::text(text)]),
            Err(error) => answer::<()>(Err(error)),
        }
    }

    #[tool(
        description = "The EPS of a portfolio file and every project in it: each node's and \
        project's activities, earliest start, latest finish, data dates, critical and \
        negative-float counts, and budgeted, actual and remaining cost, rolled up the tree. \
        Dates are the results the schedules store. Example: {\"path\": \"plan.portfolio.toml\"}."
    )]
    async fn portfolio_show(
        &self,
        Parameters(params): Parameters<PortfolioParams>,
    ) -> CallToolResult {
        let path = PathBuf::from(params.path);
        answer(
            Self::run(move || {
                let read = portfolio::read(&path)?;
                Ok(portfolio::show(&path.display().to_string(), &read))
            })
            .await,
        )
    }

    #[tool(
        description = "Capacity across a portfolio file's projects: each resource's and role's \
        demand by week or month (actual and remaining units spread over the stored dates, as \
        query's usage spreads them) against what it can supply (its maximum units per hour over \
        its calendar's work hours), the periods over, and the worst one with each project's \
        share. Resources in different files are one when their short names match. Example: \
        {\"path\": \"plan.portfolio.toml\", \"by\": \"month\"}."
    )]
    async fn portfolio_capacity(
        &self,
        Parameters(params): Parameters<CapacityParams>,
    ) -> CallToolResult {
        let CapacityParams {
            path,
            by,
            resource,
            role,
        } = params;
        let path = PathBuf::from(path);
        answer(
            Self::run(move || {
                let read = portfolio::read_keeping(&path)?;
                portfolio::capacity::report(
                    &path.display().to_string(),
                    &read,
                    by.unwrap_or_default(),
                    (resource.as_deref(), role.as_deref()),
                )
            })
            .await,
        )
    }

    #[tool(
        description = "Monte Carlo schedule risk: draw remaining durations from three-point \
        estimates, calculate each draw with the engine, and report the finish's P5 to P95, the \
        chance of the deterministic finish and of a target, milestones' P50 and P80, the activities \
        most often critical, the durations that move the finish most, and each risk event's effect \
        on P50 and P80. Quote these numbers; \
        never estimate dates yourself. Example: {\"handle\": \"s1\", \"inputs\": {\"default\": \
        {\"min\": -10, \"max\": 25}}, \"target\": \"2026-09-30 17:00\"}."
    )]
    async fn risk(&self, Parameters(params): Parameters<RiskParams>) -> CallToolResult {
        let RiskParams {
            handle,
            inputs,
            project,
            iterations,
            seed,
            target,
        } = params;
        let run = risk::Run {
            project,
            iterations: iterations
                .unwrap_or(1_000)
                .clamp(1, risk::MAX_TOOL_ITERATIONS),
            seed: seed.unwrap_or(1),
            target,
        };
        let report = self.on_session(&handle, move |session| {
            let file = session.path.display().to_string();
            risk::report(&file, &session.loaded, &inputs, &run)
        });
        answer(report.await)
    }

    #[tool(
        description = "Measure an open schedule against an open baseline, for a narrative: the \
        project finish and its variance, milestones, the largest finish slips and float losses, \
        what joined or left the critical path, and counts of changed logic, durations and \
        constraints. Variances are baseline less current in working hours, so negative is late. \
        Write the narrative from these numbers only. JSON unless format is markdown or html, which \
        return the report itself. Example: {\"current\": \"s2\", \"baseline\": \"s1\"}."
    )]
    async fn variance(&self, Parameters(params): Parameters<VarianceParams>) -> CallToolResult {
        let sides = self
            .session(&params.current)
            .and_then(|now| Ok((now, self.session(&params.baseline)?)));
        let (now, was) = match sides {
            Ok(sides) => sides,
            Err(error) => return answer::<variance::Report>(Err(error)),
        };
        let (top, format) = (params.top.unwrap_or(10), params.format);
        written(
            Self::run(move || {
                let report = variance::report(
                    (&now.path.display().to_string(), &now.loaded),
                    (&was.path.display().to_string(), &was.loaded),
                    top,
                )?;
                Ok(match format {
                    Some(format) => Written::Document(variance::render(&report, format)),
                    None => Written::Json(report),
                })
            })
            .await,
        )
    }

    #[tool(
        description = "Compare two open schedules, usually consecutive updates: added, deleted and \
        changed activities, relationships, milestone and finish movement, critical path changes, \
        and changes worth a look such as a critical duration cut with no progress. `counts` is \
        complete; each list keeps 100 items. Example: {\"from\": \"s1\", \"to\": \"s2\"}."
    )]
    async fn diff(&self, Parameters(params): Parameters<DiffParams>) -> CallToolResult {
        let sides = self
            .session(&params.from)
            .and_then(|from| Ok((from, self.session(&params.to)?)));
        let (from, to) = match sides {
            Ok(sides) => sides,
            Err(error) => return answer::<diff::Report>(Err(error)),
        };
        answer(
            Self::run(move || {
                let mut report = diff::report(
                    &from.path.display().to_string(),
                    &from.loaded,
                    &to.path.display().to_string(),
                    &to.loaded,
                )?;
                report.limit(MAX_DIFF_ITEMS);
                Ok(report)
            })
            .await,
        )
    }

    #[tool(
        description = "Write this handle's file, with any edits and calculated results, to a new XER, \
        or with format msproject to MS Project XML, or tfs to schedule text (docs/guide/text.md), \
        reporting what didn't translate in notes. The \
        original file is never changed, and an existing path is refused. Only use a path the user \
        chose. Example: {\"handle\": \"s1\", \"path\": \"/data/plant-scheduled.xer\"}."
    )]
    async fn export(&self, Parameters(params): Parameters<ExportParams>) -> CallToolResult {
        let ExportParams {
            handle,
            path,
            format,
            project,
        } = params;
        if let Some(format) = format {
            let report = self.on_session(&handle, move |session| {
                let file = session.path.display().to_string();
                import::export_file(&file, &session.loaded, (format, project), Path::new(&path))
            });
            return answer(report.await);
        }
        let report = self.on_session(&handle, move |session| export(session, Path::new(&path)));
        answer(report.await)
    }

    /// Runs a history command as the client, with its JSON as the answer.
    async fn history(
        action: Result<crate::history::Action, CliError>,
        peer: &Peer<RoleServer>,
    ) -> CallToolResult {
        let client = client_name(peer);
        match Self::run(move || stores::answer_of(&action?, &client)).await {
            // A damaged store is a failure, with what's wrong as the answer.
            Ok((exit::ERROR, doc)) => CallToolResult::error(vec![ContentBlock::text(pretty(&doc))]),
            Ok((_, doc)) => answer::<serde_json::Value>(Ok(doc)),
            Err(error) => answer::<serde_json::Value>(Err(error)),
        }
    }

    #[tool(
        description = "Start a history for an XER file: a store beside it (plant.xer gives \
        plant.tfdb) on branch main. Open the store with `open` to query it, and `edit` it with a \
        reason: each change is recorded as yours, an agent's, with who, why and when. Example: \
        {\"file\": \"/data/plant.xer\", \"reason\": \"As submitted\"}."
    )]
    async fn history_init(
        &self,
        Parameters(params): Parameters<stores::InitParams>,
        peer: Peer<RoleServer>,
    ) -> CallToolResult {
        Self::history(params.action(), &peer).await
    }

    #[tool(
        description = "A store's history, newest first: who (agent or person), when, what and \
        why. `all` lists every changeset, proposals and rejected ones included. Example: \
        {\"store\": \"/data/plant.tfdb\", \"limit\": 10}."
    )]
    async fn history_log(
        &self,
        Parameters(params): Parameters<stores::LogParams>,
        peer: Peer<RoleServer>,
    ) -> CallToolResult {
        Self::history(Ok(params.action()), &peer).await
    }

    #[tool(
        description = "One changeset in full: its commands, impact, reviews and who made it. \
        Example: {\"store\": \"/data/plant.tfdb\", \"at\": \"main\"}."
    )]
    async fn history_show(
        &self,
        Parameters(params): Parameters<stores::ShowParams>,
        peer: Peer<RoleServer>,
    ) -> CallToolResult {
        Self::history(Ok(params.action()), &peer).await
    }

    #[tool(
        description = "What changed between two points (default the head and its parent), with \
        the changesets between them. Lists at most `limit` of each kind per project (default \
        25, 0 for every one); the counts are always complete. Example: {\"store\": \
        \"/data/plant.tfdb\", \"from\": \"contract\", \"to\": \"main\"}."
    )]
    async fn history_diff(
        &self,
        Parameters(params): Parameters<stores::DiffParams>,
        peer: Peer<RoleServer>,
    ) -> CallToolResult {
        Self::history(Ok(params.action()), &peer).await
    }

    #[tool(
        description = "Take a changeset back out of the checked-out branch, as a new changeset; \
        without `at`, undo the latest. Later edits are replayed without it; nothing is deleted. \
        Needs a reason. Example: {\"store\": \"/data/plant.tfdb\", \"reason\": \"Wrong activity\"}."
    )]
    async fn history_revert(
        &self,
        Parameters(params): Parameters<stores::RevertParams>,
        peer: Peer<RoleServer>,
    ) -> CallToolResult {
        Self::history(params.action(), &peer).await
    }

    #[tool(
        description = "Branches for what-ifs: create (at the head or `at` a point), switch, \
        list or delete. Edits record on the checked-out branch. Example: {\"store\": \
        \"/data/plant.tfdb\", \"action\": \"create\", \"name\": \"what-if/steel-late\"}."
    )]
    async fn history_branch(
        &self,
        Parameters(params): Parameters<stores::RefParams>,
        peer: Peer<RoleServer>,
    ) -> CallToolResult {
        Self::history(params.branch_action(), &peer).await
    }

    #[tool(
        description = "Baselines: named points that never move. create, list or delete; compare \
        with `query` or `variance` using baseline \"STORE@NAME\". Example: {\"store\": \
        \"/data/plant.tfdb\", \"action\": \"create\", \"name\": \"contract\"}."
    )]
    async fn history_baseline(
        &self,
        Parameters(params): Parameters<stores::RefParams>,
        peer: Peer<RoleServer>,
    ) -> CallToolResult {
        Self::history(params.baseline_action(), &peer).await
    }

    #[tool(
        description = "Merge a branch into the checked-out one by replaying its edits. Refused, \
        with every conflict listed, if both changed the same thing differently. Needs a reason \
        unless dry_run. Example: {\"store\": \"/data/plant.tfdb\", \"branch\": \
        \"what-if/steel-late\", \"dry_run\": true}."
    )]
    async fn history_merge(
        &self,
        Parameters(params): Parameters<stores::MergeParams>,
        peer: Peer<RoleServer>,
    ) -> CallToolResult {
        Self::history(params.action(), &peer).await
    }

    #[tool(
        description = "Proposals waiting for review, with who made each and how far each \
        project's finish would move. Make one with `edit` and `propose: true`. Example: \
        {\"store\": \"/data/plant.tfdb\"}."
    )]
    async fn history_proposals(
        &self,
        Parameters(params): Parameters<stores::StoreParams>,
        peer: Peer<RoleServer>,
    ) -> CallToolResult {
        Self::history(Ok(params.proposals_action()), &peer).await
    }

    #[tool(
        description = "Accept or reject a proposal. Only do this when the user has decided: a \
        proposal is waiting for a person. Rejecting needs a reason. Example: {\"store\": \
        \"/data/plant.tfdb\", \"proposal\": \"proposal/main/3f9a2c1d0e\", \"decision\": \
        \"accept\", \"reason\": \"Agreed by the planner\"}."
    )]
    async fn history_review(
        &self,
        Parameters(params): Parameters<stores::ReviewParams>,
        peer: Peer<RoleServer>,
    ) -> CallToolResult {
        Self::history(Ok(params.action()), &peer).await
    }

    #[tool(
        description = "Write a point in history (default the checked-out head) as a new XER \
        file, byte for byte. Example: {\"store\": \"/data/plant.tfdb\", \"at\": \
        \"contract\", \"path\": \"/data/contract.xer\"}."
    )]
    async fn history_export(
        &self,
        Parameters(params): Parameters<stores::ExportParams>,
        peer: Peer<RoleServer>,
    ) -> CallToolResult {
        Self::history(Ok(params.action()), &peer).await
    }

    #[tool(
        description = "Check a store against its hashes, and with replay whether each recorded \
        edit replays to the file recorded. Example: {\"store\": \"/data/plant.tfdb\", \
        \"replay\": true}."
    )]
    async fn history_verify(
        &self,
        Parameters(params): Parameters<stores::VerifyParams>,
        peer: Peer<RoleServer>,
    ) -> CallToolResult {
        Self::history(Ok(params.action()), &peer).await
    }

    #[tool(
        description = "Bring a new export from P6 into the history as the next point on the \
        checked-out branch, with what changed. Needs a reason. Example: {\"store\": \
        \"/data/plant.tfdb\", \"file\": \"/data/update-07.xer\", \"reason\": \"Update 07\"}."
    )]
    async fn history_import(
        &self,
        Parameters(params): Parameters<stores::ImportParams>,
        peer: Peer<RoleServer>,
    ) -> CallToolResult {
        Self::history(params.action(), &peer).await
    }

    #[tool(description = "Close a handle and free its file. Unexported results are lost.")]
    async fn close(&self, Parameters(params): Parameters<HandleParams>) -> CallToolResult {
        answer(self.lock().and_then(|mut sessions| {
            let session = sessions.open.remove(&params.handle);
            session
                .map(|_| Closed {
                    closed: params.handle.clone(),
                })
                .ok_or_else(|| no_handle(&params.handle))
        }))
    }
}

#[allow(
    clippy::unused_async_trait_impl,
    reason = "generated by the rmcp macro, which needs the trait's async signature"
)]
#[tool_handler(
    name = "totalfloat",
    instructions = "Schedule Primavera P6 XER files. Call `open` with a file path, or `new` to start \
    a schedule, to get a handle, then pass it to the other tools. All dates and floats come from the deterministic engine: use \
    `query` and `explain` to find them, and `edit` for any what-if. Never work out or estimate a \
    date, float or day count yourself, even labeled as an estimate. If a tool can't do something, \
    say so and give no number. To see what drives the \
    project finish, find the activity with `query` (`critical: true`), then call `explain` on it \
    with `field: \"early_finish\"`, or `query` with `driving_path_to`."
)]
impl ServerHandler for Server {}

/// Serves until the client disconnects.
pub fn serve() -> Result<u8, CliError> {
    let failed = |error: &dyn std::fmt::Display| {
        CliError::new(
            "mcp-failed",
            format!("the MCP server stopped: {error}"),
            "check that the client speaks MCP over stdio",
        )
    };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| failed(&e))?;
    runtime.block_on(async {
        let service = Server::new().serve(stdio()).await.map_err(|e| failed(&e))?;
        service.waiting().await.map_err(|e| failed(&e))?;
        Ok(exit::OK)
    })
}

pub(crate) mod stores;

#[cfg(test)]
mod tests;
