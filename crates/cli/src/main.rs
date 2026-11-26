//! The `totalfloat` command-line interface.
//!
//! Every capability is exposed here (and later over MCP from the `mcp`
//! subcommand) on top of the shared service crates. See
//! docs/ARCHITECTURE.md "CLI conventions" for the agent-first rules every
//! command follows: `--json` with a versioned schema, no prompts, exit codes
//! `0` ok / `1` error / `2` usage / `3` findings, and errors with a code,
//! message and hint.

#![forbid(unsafe_code)]

mod anonymize;
mod apply;
mod build;
mod calendar;
mod check;
mod diff;
mod edit;
mod error;
mod explain;
mod export;
mod fmt;
mod global;
mod history;
mod ics;
mod import;
mod inspect;
mod loaded;
mod location;
mod mcp;
mod new;
mod output;
mod page;
mod portfolio;
mod progress;
mod query;
mod report;
mod resheet;
mod risk;
mod roundtrip;
mod schedlog;
mod schedule;
mod serve;
mod sheets;
mod table;
mod textconv;
mod timesheet;
mod usage;
mod variance;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

/// Open-source, agent-native CPM scheduler for Primavera P6 XER files.
#[derive(Debug, Parser)]
#[command(name = "totalfloat", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

/// `totalfloat portfolio`'s actions.
#[derive(Debug, Subcommand)]
enum PortfolioAction {
    /// Write a portfolio file for schedules: their projects under the EPS
    /// nodes the files keep the IDs of. Refuses a path that exists.
    New {
        /// The XER files.
        #[arg(required = true)]
        files: Vec<PathBuf>,
        /// The new portfolio file, such as `plan.portfolio.toml`.
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Each resource's and role's demand across the portfolio's projects
    /// against what it can supply, by week or month, with over-allocations.
    Capacity {
        /// The portfolio file.
        file: PathBuf,
        /// The length of a period.
        #[arg(long, value_enum, default_value_t = usage::By::Week)]
        by: usage::By,
        /// Only this resource, by short name, with every period.
        #[arg(long)]
        resource: Option<String>,
        /// Only this role, by short name, with every period.
        #[arg(long)]
        role: Option<String>,
        /// Print JSON (schema `totalfloat.portfolio-capacity/1`).
        #[arg(long)]
        json: bool,
    },
    /// Show the EPS with each node's and project's dates, activities,
    /// critical work and costs.
    Show {
        /// The portfolio file.
        file: PathBuf,
        /// Print JSON (schema `totalfloat.portfolio-show/1`).
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Print an XER as stable, sorted text for git diffs. Set it up with
    /// `*.xer diff=totalfloat` in .gitattributes and
    /// `git config diff.totalfloat.textconv "totalfloat textconv"`
    /// (docs/guide/git.md).
    Textconv {
        /// The XER file.
        file: PathBuf,
    },
    /// Rewrite schedule text files (`.tfs`) in the canonical layout, or with
    /// `--check` list the ones that would change and exit 3
    /// (docs/guide/text.md).
    Fmt {
        /// The `.tfs` files.
        #[arg(required = true)]
        files: Vec<PathBuf>,
        /// Change nothing; exit 3 if any file isn't formatted.
        #[arg(long)]
        check: bool,
        /// Print JSON (schema `totalfloat.fmt/1`).
        #[arg(long)]
        json: bool,
    },
    /// Keep a schedule's history: who changed what, why and when, with every
    /// version of the file (docs/guide/history.md).
    History {
        #[command(subcommand)]
        action: history::Action,
    },
    /// Summarize an XER file: export version, projects, tables, calendars,
    /// schedule options and diagnostics.
    Inspect {
        /// The XER file.
        file: PathBuf,
        /// Print JSON (schema `totalfloat.inspect/1`).
        #[arg(long)]
        json: bool,
    },
    /// Read an XER and write it back in memory; report whether the bytes are
    /// identical. Exits 3 if they aren't.
    Roundtrip {
        /// The XER file.
        file: PathBuf,
        /// Print JSON (schema `totalfloat.roundtrip/1`).
        #[arg(long)]
        json: bool,
    },
    /// Calculate dates, float and the critical path, and report each project's
    /// finish and what changed. Writes results to a new file with `-o`; the
    /// source never changes.
    Schedule {
        /// The XER file.
        file: PathBuf,
        /// New output file; existing paths are refused.
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Only this project (`proj_id`); default is every project in the file.
        #[arg(long)]
        project: Option<u64>,
        /// Move the data date first, as `YYYY-MM-DD HH:MM`.
        #[arg(long)]
        data_date: Option<String>,
        /// Ignore relationships to and from other projects: each project is
        /// calculated on its own
        /// logic, without links to activities elsewhere or their external
        /// dates. For this run only; the file's own setting isn't changed.
        #[arg(long)]
        ignore_external: bool,
        /// Find up to this many float paths, numbered by TotalFloat's rule
        /// (docs/rules/TF-002-multiple-float-paths.md), whatever the file's own
        /// options say. For this run only; the file's options aren't changed.
        #[arg(long, value_name = "N")]
        float_paths: Option<u32>,
        /// The Activity ID the float paths end at. Default is the file's own
        /// end activity, or the project's finish.
        #[arg(long, value_name = "ACTIVITY", requires = "float_paths")]
        float_path_to: Option<String>,
        /// Number the float paths by total float or by free float. Default is
        /// the file's own setting.
        #[arg(long, value_enum, value_name = "MODE", requires = "float_paths")]
        float_paths_by: Option<PathsBy>,
        /// Also write a log of the run to this new file: the settings used,
        /// counts, every problem found, and the results.
        #[arg(long, value_name = "FILE")]
        log: Option<PathBuf>,
        /// Print JSON (schema `totalfloat.schedule/1`).
        #[arg(long)]
        json: bool,
    },
    /// Write a project as MS Project XML, schedule text (`.tfs`) or an
    /// iCalendar file (`.ics`), and say what didn't translate.
    Export {
        /// The XER file.
        file: PathBuf,
        /// The format to write: `msproject`, `tfs` for schedule text, or `ics`
        /// for the activities as an iCalendar file.
        #[arg(long, value_enum)]
        to: import::Format,
        /// New output file; existing paths are refused.
        #[arg(short, long)]
        output: PathBuf,
        /// Only this project (`proj_id`); needed if the file has several.
        #[arg(long)]
        project: Option<u64>,
        /// Print JSON (schema `totalfloat.export/1`).
        #[arg(long)]
        json: bool,
    },
    /// Bring in an MS Project XML file as a new XER, and say what didn't
    /// translate, or build one from a schedule text file (`.tfs`,
    /// docs/guide/text.md). Dates follow TotalFloat's own rules, so they can
    /// differ from Project's.
    Import {
        /// The MS Project XML file, schedule text file, or spreadsheet: an
        /// `.xlsx` workbook, or a folder of CSV files (docs/guide/import.md).
        file: PathBuf,
        /// New output file; existing paths are refused.
        #[arg(short, long)]
        output: PathBuf,
        /// Print JSON (schema `totalfloat.import/1`).
        #[arg(long)]
        json: bool,
    },
    /// Start a new schedule: a project, a root WBS node and a five-day calendar,
    /// built on by `--edits`. Refuses a path that exists.
    New {
        /// The project's code.
        #[arg(long)]
        code: String,
        /// The project's name.
        #[arg(long)]
        name: String,
        /// The data date, as `YYYY-MM-DD HH:MM`.
        #[arg(long)]
        data_date: String,
        /// The must-finish-by date, as `YYYY-MM-DD HH:MM`.
        #[arg(long)]
        must_finish: Option<String>,
        /// A JSON array of edits to build the schedule with.
        #[arg(long, value_name = "FILE")]
        edits: Option<PathBuf>,
        /// New output file.
        #[arg(short, long)]
        output: PathBuf,
        /// Print JSON (schema `totalfloat.new/1`).
        #[arg(long)]
        json: bool,
    },
    /// Change a schedule, recalculate, and report what the change did. Writes a
    /// new file with `-o`; the source never changes. On a history store, the
    /// change is recorded on its checked-out branch instead.
    Edit {
        /// The XER file, or a history store (`.tfdb`).
        file: PathBuf,
        #[command(flatten)]
        flags: Box<edit::Flags>,
        /// New output file; existing paths are refused.
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Only this project (`proj_id`); needed if an Activity ID is in several.
        #[arg(long)]
        project: Option<u64>,
        /// Move the data date too, as `YYYY-MM-DD HH:MM`.
        #[arg(long)]
        data_date: Option<String>,
        /// With a new data date (here or in --edits), record the progress the
        /// schedule planned before it: work due to start starts, and work due
        /// to finish finishes.
        #[arg(long)]
        as_planned: bool,
        /// Compare quality checks before and after. If one newly fails, nothing
        /// is written and the command exits 3.
        #[arg(long)]
        check: bool,
        /// Apply the edits that can be applied and skip the rest, with every
        /// edit that needs them. Lists what was skipped and exits 3.
        #[arg(long)]
        skip_failed: bool,
        /// Print JSON (schema `totalfloat.edit/1`).
        #[arg(long)]
        json: bool,
    },
    /// Run schedule quality checks: the DCMA 14-point assessment and more.
    /// Exits 3 if any check fails.
    Check {
        /// The XER file.
        file: PathBuf,
        /// Only this project (`proj_id`); default is every project in the file.
        #[arg(long)]
        project: Option<u64>,
        /// TOML file of thresholds (see docs/CHECKS.md).
        #[arg(long)]
        config: Option<PathBuf>,
        /// Print a Markdown report.
        #[arg(long, conflicts_with = "json")]
        markdown: bool,
        /// Write one self-contained HTML page.
        #[arg(long, conflicts_with_all = ["json", "markdown"])]
        html: bool,
        /// Print JSON (schema `totalfloat.check/1`).
        #[arg(long)]
        json: bool,
        /// New output file; existing paths are refused. Default: stdout.
        #[arg(short, long, conflicts_with = "json")]
        output: Option<PathBuf>,
    },
    /// Compare two versions of a schedule: activities, logic, progress, and
    /// changes worth a look. Exits 3 if any are flagged.
    Diff {
        /// The earlier XER file.
        from: PathBuf,
        /// The later XER file.
        to: PathBuf,
        /// Print a Markdown report.
        #[arg(long, conflicts_with = "json")]
        markdown: bool,
        /// Print JSON (schema `totalfloat.diff/1`).
        #[arg(long)]
        json: bool,
    },
    /// Measure a schedule against a baseline: finish and milestone variance,
    /// the largest slips and float erosion, and critical path changes.
    Variance {
        /// The current XER file.
        file: PathBuf,
        /// The baseline XER file: an earlier update or the approved plan.
        #[arg(long)]
        baseline: PathBuf,
        /// How many slips and float losses to list.
        #[arg(long, default_value_t = 10)]
        top: usize,
        /// Print a Markdown report.
        #[arg(long, conflicts_with = "json")]
        markdown: bool,
        /// Write one self-contained HTML page.
        #[arg(long, conflicts_with_all = ["json", "markdown"])]
        html: bool,
        /// Print JSON (schema `totalfloat.variance/1`).
        #[arg(long)]
        json: bool,
        /// New output file; existing paths are refused. Default: stdout.
        #[arg(short, long, conflicts_with = "json")]
        output: Option<PathBuf>,
    },
    /// Monte Carlo schedule risk: draw durations from three-point estimates,
    /// calculate each draw, and report the finish's P-values, the activities
    /// most often critical and the durations that move the finish most
    /// (docs/guide/risk.md).
    Risk {
        /// The XER file.
        file: PathBuf,
        /// A JSON file of estimates (schema `totalfloat.risk-inputs/1`).
        #[arg(long)]
        inputs: Option<PathBuf>,
        /// A triangular range for activities with no estimate, in percent of
        /// remaining duration, as `MIN,MAX`: `-10,25`.
        #[arg(long, allow_hyphen_values = true)]
        range: Option<String>,
        /// Only this project (`proj_id`); needed if the file has several.
        #[arg(long)]
        project: Option<u64>,
        /// Iterations, from 1 to 100,000.
        #[arg(long, default_value_t = 1000, value_parser = clap::value_parser!(u32).range(1..=100_000))]
        iterations: u32,
        /// The seed: the same seed gives the same result.
        #[arg(long, default_value_t = 1)]
        seed: u64,
        /// Give the chance of finishing by this date, as `YYYY-MM-DD HH:MM`.
        #[arg(long)]
        target: Option<String>,
        /// Print JSON (schema `totalfloat.risk-analysis/1`).
        #[arg(long)]
        json: bool,
    },
    /// Keep projects in an EPS and see them together: a portfolio file
    /// beside the schedules (docs/guide/portfolio.md).
    Portfolio {
        #[command(subcommand)]
        action: PortfolioAction,
    },
    /// Say why a date is what it is, step by step back to its origin.
    Explain {
        /// The XER file.
        file: PathBuf,
        /// The activity (`task_code` or `task_id`).
        activity: String,
        /// Which value to explain.
        #[arg(long, value_enum, default_value = "early_start")]
        field: explain::Field,
        /// Only look in this project (`proj_id`).
        #[arg(long)]
        project: Option<u64>,
        /// Print JSON (schema `totalfloat.explain/1`).
        #[arg(long)]
        json: bool,
    },
    /// Show what a calendar works: its week, hours and exceptions, and with
    /// `--from` and `--to`, each day in the range.
    Calendar {
        /// The XER file.
        file: PathBuf,
        /// The calendar's name or `clndr_id`.
        calendar: String,
        /// First date, as `YYYY-MM-DD`.
        #[arg(long)]
        from: Option<String>,
        /// Last date, as `YYYY-MM-DD`.
        #[arg(long)]
        to: Option<String>,
        /// Print JSON (schema `totalfloat.calendar/1`).
        #[arg(long)]
        json: bool,
    },
    /// Serve MCP over stdio, so an agent can use every command above.
    Mcp,
    /// Open the local app in a browser: serve a schedule on 127.0.0.1 until
    /// Ctrl-C (docs/guide/serve.md).
    Serve {
        /// A history store (`plant.tfdb`, `plant.tfdb@branch`) or an XER file,
        /// which is shown read-only.
        location: PathBuf,
        /// The port; default is any free one.
        #[arg(long)]
        port: Option<u16>,
        /// Open the link in the default browser.
        #[arg(long)]
        open: bool,
    },
    /// Find activities: filter them, follow relationships or trace what drives
    /// one. Paged, so a large schedule isn't printed whole.
    Query {
        /// The XER file.
        file: PathBuf,
        #[command(flatten)]
        query: Box<query::Query>,
        /// Print JSON (schema `totalfloat.query/1`).
        #[arg(long)]
        json: bool,
    },
    /// Timesheets for a period: one sheet per team member, listing their
    /// assignments on unfinished work in it, to fill in with hours and
    /// progress and apply with `edit --from-timesheet`.
    Timesheet {
        /// The XER file, or a history store (`.tfdb`).
        file: PathBuf,
        /// The period's first day, as `YYYY-MM-DD`.
        #[arg(long, value_name = "DATE")]
        from: String,
        /// The period's last day, as `YYYY-MM-DD`.
        #[arg(long, value_name = "DATE")]
        to: String,
        /// A new workbook with a sheet per resource.
        #[arg(
            long,
            value_name = "FILE",
            conflicts_with = "csv",
            required_unless_present = "csv"
        )]
        xlsx: Option<PathBuf>,
        /// A new folder with a CSV file per resource.
        #[arg(long, value_name = "DIR")]
        csv: Option<PathBuf>,
        /// Only this resource, by short name or name. Repeat for several.
        #[arg(long, value_name = "RESOURCE")]
        resource: Vec<String>,
        /// Only this project (`proj_id`); needed if the file has several.
        #[arg(long)]
        project: Option<u64>,
    },
    /// Write a report from a template: activities filtered, grouped and
    /// sorted, as Markdown, CSV or HTML. Built in: `look-ahead` and
    /// `critical-path` (docs/guide/report.md)
    Report {
        /// A built-in template's name, or a template file (TOML).
        template: String,
        /// The XER file, or a history store.
        file: PathBuf,
        /// How to write it.
        #[arg(long, value_enum, default_value = "markdown")]
        format: report::Format,
        /// Only this project (`proj_id`); its data date starts a window.
        #[arg(long)]
        project: Option<u64>,
        /// New output file; existing paths are refused. Default: stdout.
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Remove recognized identifying text, preserving scheduling data.
    /// Refuses unsupported content or failed field verification. IDs, GUIDs,
    /// dates and costs remain; review the result before sharing.
    Anonymize {
        /// Source XER file (never overwritten).
        file: PathBuf,
        /// New output file; existing paths are refused.
        #[arg(short, long)]
        output: PathBuf,
        /// Explicit deterministic seed (default: 0).
        #[arg(long, default_value_t = 0)]
        seed: u64,
        /// Include per-table/field scrubbed-cell counts, without source values.
        #[arg(long)]
        report: bool,
        /// Print JSON (schema `totalfloat.anonymize/1`).
        #[arg(long)]
        json: bool,
    },
}

/// The document `--markdown` or `--html` asks for, if either.
const fn document(markdown: bool, html: bool) -> Option<page::Format> {
    if html {
        Some(page::Format::Html)
    } else if markdown {
        Some(page::Format::Markdown)
    } else {
        None
    }
}

/// How float paths are numbered, for `schedule --float-paths-by`.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum PathsBy {
    /// By relative total float to the end activity.
    TotalFloat,
    /// By free float to the paths already found.
    FreeFloat,
}

impl PathsBy {
    const fn mode(self) -> totalfloat_model::FloatPathMode {
        match self {
            Self::TotalFloat => totalfloat_model::FloatPathMode::TotalFloat,
            Self::FreeFloat => totalfloat_model::FloatPathMode::FreeFloat,
        }
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "one short arm per command, and each does its work in its own module"
)]
fn main() -> ExitCode {
    let cli = Cli::parse();
    let (result, json) = match cli.command {
        None => {
            // No subcommand: show help, like `--help`.
            use clap::CommandFactory;
            let _ = Cli::command().print_help();
            return ExitCode::from(error::exit::OK);
        }
        Some(Command::Anonymize {
            file,
            output,
            seed,
            report,
            json,
        }) => (anonymize::run(&file, &output, seed, report, json), json),
        Some(Command::Schedule {
            file,
            output,
            project,
            data_date,
            ignore_external,
            float_paths,
            float_path_to,
            float_paths_by,
            log,
            json,
        }) => (
            schedule::run(
                &file,
                (output.as_deref(), log.as_deref()),
                (project, data_date.as_deref()),
                json,
                &schedule::Overrides {
                    ignore_external,
                    float_paths: float_paths.map(|paths| schedule::FloatPathAsk {
                        paths,
                        to: float_path_to,
                        by: float_paths_by.map(PathsBy::mode),
                    }),
                },
            ),
            json,
        ),
        Some(Command::Export {
            file,
            to,
            output,
            project,
            json,
        }) => (
            import::run_export(&file, (to, project), &output, json),
            json,
        ),
        Some(Command::Import { file, output, json }) => {
            if sheets::is_sheets(&file) {
                (sheets::run(&file, &output, json), json)
            } else if build::is_text(&file) {
                (build::run(&file, &output, json), json)
            } else {
                (import::run(&file, &output, json), json)
            }
        }
        Some(Command::New {
            code,
            name,
            data_date,
            must_finish,
            edits,
            output,
            json,
        }) => {
            let spec = new::Spec {
                code: &code,
                name: &name,
                data_date: &data_date,
                must_finish: must_finish.as_deref(),
                calendar: None,
            };
            (new::run(&spec, edits.as_deref(), &output, json), json)
        }
        Some(Command::Edit {
            file,
            flags,
            output,
            project,
            data_date,
            as_planned,
            check,
            skip_failed,
            json,
        }) => {
            let options = edit::Options {
                project,
                data_date: data_date.as_deref(),
                as_planned,
                check,
                skip_failed,
            };
            (
                edit::run(&file, &flags, output.as_deref(), options, json),
                json,
            )
        }
        Some(Command::Check {
            file,
            project,
            config,
            markdown,
            html,
            json,
            output,
        }) => (
            check::run(
                &file,
                project,
                config.as_deref(),
                (document(markdown, html), json),
                output.as_deref(),
            ),
            json,
        ),
        Some(Command::Diff {
            from,
            to,
            markdown,
            json,
        }) => (diff::run(&from, &to, markdown, json), json),
        Some(Command::Variance {
            file,
            baseline,
            top,
            markdown,
            html,
            json,
            output,
        }) => (
            variance::run(
                (&file, &baseline),
                top,
                (document(markdown, html), json),
                output.as_deref(),
            ),
            json,
        ),
        Some(Command::Risk {
            file,
            inputs,
            range,
            project,
            iterations,
            seed,
            target,
            json,
        }) => (
            risk::run(
                &file,
                inputs.as_deref(),
                range.as_deref(),
                &risk::Run {
                    project,
                    iterations,
                    seed,
                    target,
                },
                json,
            ),
            json,
        ),
        Some(Command::Portfolio { action }) => match action {
            PortfolioAction::New { files, output } => (portfolio::run_new(&files, &output), false),
            PortfolioAction::Show { file, json } => (portfolio::run_show(&file, json), json),
            PortfolioAction::Capacity {
                file,
                by,
                resource,
                role,
                json,
            } => (
                portfolio::run_capacity(&file, by, (resource.as_deref(), role.as_deref()), json),
                json,
            ),
        },
        Some(Command::Mcp) => (mcp::serve(), false),
        Some(Command::Serve {
            location,
            port,
            open,
        }) => (serve::run(&location, port, open), false),
        Some(Command::Textconv { file }) => (textconv::run(&file), false),
        Some(Command::Fmt { files, check, json }) => (fmt::run(&files, check, json), json),
        Some(Command::History { action }) => (history::run(&action), action.json()),
        Some(Command::Calendar {
            file,
            calendar,
            from,
            to,
            json,
        }) => {
            let range = calendar::Range {
                from: from.as_deref(),
                to: to.as_deref(),
            };
            (calendar::run(&file, &calendar, &range, json), json)
        }
        Some(Command::Explain {
            file,
            activity,
            field,
            project,
            json,
        }) => (explain::run(&file, &activity, field, project, json), json),
        Some(Command::Query { file, query, json }) => (query::run(&file, &query, json), json),
        Some(Command::Timesheet {
            file,
            from,
            to,
            xlsx,
            csv,
            resource,
            project,
        }) => {
            let ask = timesheet::Ask {
                project,
                from: &from,
                to: &to,
                resources: &resource,
            };
            let out = match (&xlsx, &csv) {
                (Some(path), _) => Some(timesheet::Out::Xlsx(path)),
                (None, Some(dir)) => Some(timesheet::Out::Csv(dir)),
                (None, None) => None,
            };
            (timesheet::run(&file, &ask, out), false)
        }
        Some(Command::Report {
            template,
            file,
            format,
            project,
            output,
        }) => (
            report::run(&template, &file, (project, format), output.as_ref()),
            false,
        ),
        Some(Command::Inspect { file, json }) => (inspect::run(&file, json), json),
        Some(Command::Roundtrip { file, json }) => (roundtrip::run(&file, json), json),
    };
    match result {
        Ok(code) => ExitCode::from(code),
        Err(e) => {
            error::report(&e, json);
            ExitCode::from(error::exit::ERROR)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Cli;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }
}
