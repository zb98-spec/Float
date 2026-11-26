//! `totalfloat import`: bring another tool's schedule in as an XER.
//!
//! The result is calculated by TotalFloat's own rules (docs/rules), so its
//! dates can differ from the other tool's, and the report says what didn't translate (ARCHITECTURE.md, File interop).

use std::path::Path;

use serde::Serialize;
use totalfloat_interop::{Note, msproject, msproject_write};
use totalfloat_xer::RawXer;
use totalfloat_xer::fresh::{FreshOptions, seed_of, write};

use crate::error::{CliError, exit};
use crate::loaded::Loaded;
use crate::output::{create_new, create_new_text, print_line, refuse_existing};
use crate::schedule;

/// `totalfloat.import/1`.
#[derive(Debug, Serialize)]
pub struct Report {
    schema: &'static str,
    source: String,
    output: String,
    code: String,
    activities: usize,
    relationships: usize,
    wbs_nodes: usize,
    calendars: usize,
    resources: usize,
    /// False if the engine refused it, so the file holds no dates yet.
    calculated: bool,
    finish: Option<String>,
    /// What didn't translate exactly.
    notes: Vec<Note>,
    /// Why it couldn't be calculated, if it couldn't.
    #[serde(skip_serializing_if = "Option::is_none")]
    not_calculated: Option<String>,
}

fn refused(error: impl std::fmt::Display) -> CliError {
    CliError::new(
        "import-refused",
        format!("can't import that file: {error}"),
        "save it from MS Project as XML; .mpp files aren't read",
    )
}

fn read_text(path: &Path) -> Result<String, CliError> {
    let bytes = std::fs::read(path).map_err(|error| {
        CliError::new(
            "file-not-readable",
            format!("can't read {}: {error}", path.display()),
            "check the path and that the file is readable",
        )
    })?;
    String::from_utf8(bytes).map_err(refused)
}

/// Converts a file's text, and calculates it if the engine can.
pub fn convert(source: &str, output: &str, text: &str) -> Result<(Report, RawXer), CliError> {
    let imported = msproject::read(text).map_err(refused)?;
    let schedule = &imported.schedule;
    let date = schedule
        .project
        .data_date
        .map(totalfloat_model::DateTime::date)
        .ok_or_else(|| refused("it has no data date"))?;
    let options = FreshOptions {
        seed: seed_of(text.as_bytes()),
        ..FreshOptions::new(date)
    };
    let raw = write(schedule, options).map_err(refused)?;
    let loaded = Loaded::new(raw);
    let (raw, finish, not_calculated) = match schedule::fresh(&loaded, None, None) {
        Ok(done) => {
            let finish = done
                .schedules
                .first()
                .and_then(|s| s.project.stored.scheduled_finish);
            (done.raw, finish.map(|d| d.to_string()), None)
        }
        Err(error) => (loaded.raw.clone(), None, Some(error.message)),
    };
    let report = Report {
        schema: "totalfloat.import/1",
        source: source.to_owned(),
        output: output.to_owned(),
        code: schedule.project.short_name.clone(),
        activities: schedule.activities.len(),
        relationships: schedule.relationships.len(),
        wbs_nodes: schedule.wbs.len(),
        calendars: schedule.calendars.len(),
        resources: schedule.resources.len(),
        calculated: not_calculated.is_none(),
        finish,
        notes: imported.notes,
        not_calculated,
    };
    Ok((report, raw))
}

fn note_lines(notes: &[Note]) -> Vec<String> {
    let heading = (!notes.is_empty()).then(|| "didn't translate exactly:".to_owned());
    let lines = notes.iter().map(|note| {
        let shown = note.examples.join(", ");
        format!("  {} x {}: {shown}", note.count, note.message)
    });
    heading.into_iter().chain(lines).collect()
}

fn human(r: &Report) -> String {
    let mut lines = vec![
        format!(
            "Project {}: {} activities, {} relationships, {} WBS nodes, {} calendars, {} resources",
            r.code, r.activities, r.relationships, r.wbs_nodes, r.calendars, r.resources
        ),
        r.finish.as_ref().map_or_else(
            || "not calculated".to_owned(),
            |finish| format!("finish: {finish}"),
        ),
    ];
    if let Some(why) = &r.not_calculated {
        lines.push(format!("warning: {why}"));
    }
    lines.extend(note_lines(&r.notes));
    lines.push(
        "Dates follow TotalFloat's own rules, so they can differ from MS Project's".to_owned(),
    );
    lines.push(format!("Wrote {}", r.output));
    lines.join("\n")
}

/// A format `export` can write.
#[derive(Debug, Clone, Copy, clap::ValueEnum, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    /// MS Project XML.
    Msproject,
    /// Schedule text (`.tfs`, ARCHITECTURE.md, Schedule as text).
    Tfs,
    /// The activity network as a Graphviz file (`.dot`).
    Dot,
}

/// `totalfloat.export/1`.
#[derive(Debug, Serialize)]
pub struct Exported {
    schema: &'static str,
    source: String,
    output: String,
    format: &'static str,
    /// What didn't translate exactly.
    notes: Vec<Note>,
}

/// Writes one project of a file as MS Project XML, schedule text or a
/// Graphviz network. The source is never changed.
pub fn export_file(
    source: &str,
    loaded: &Loaded,
    (format, project): (Format, Option<u64>),
    output: &Path,
) -> Result<Exported, CliError> {
    refuse_existing(output)?;
    let reach = loaded
        .schedules
        .iter()
        .filter(|s| project.is_none_or(|id| s.project.id.0 == id));
    let schedule = match reach.collect::<Vec<_>>().as_slice() {
        [one] => *one,
        [] => {
            let message = project.map_or_else(
                || "the file holds no projects".to_owned(),
                |id| format!("the file has no project {id}"),
            );
            return Err(CliError::new(
                "no-such-project",
                message,
                "`inspect` lists the projects in the file",
            ));
        }
        many => {
            return Err(CliError::new(
                "needs-project",
                format!("the file has {} projects", many.len()),
                "name the project: --project, or `project` over MCP",
            ));
        }
    };
    loaded.refuse_incomplete(schedule)?;
    if let Format::Tfs = format {
        let rendered = totalfloat_text::render(schedule);
        create_new_text(&totalfloat_text::format(&rendered.document), output)?;
        return Ok(Exported {
            schema: "totalfloat.export/1",
            source: source.to_owned(),
            output: output.display().to_string(),
            format: "tfs",
            notes: rendered
                .left_out
                .into_iter()
                .map(|l| Note {
                    kind: l.kind,
                    message: l.message,
                    count: l.count,
                    examples: l.examples,
                })
                .collect(),
        });
    }
    if let Format::Dot = format {
        let others: Vec<_> = loaded
            .schedules
            .iter()
            .filter(|s| s.project.id != schedule.project.id)
            .cloned()
            .collect();
        let (text, notes) = crate::dot::write(schedule, &others);
        create_new_text(&text, output)?;
        return Ok(Exported {
            schema: "totalfloat.export/1",
            source: source.to_owned(),
            output: output.display().to_string(),
            format: "dot",
            notes,
        });
    }
    if schedule.calendars.is_empty() {
        return Err(CliError::new(
            "no-calendars",
            "the project has no calendars, so its durations can't be read in Project",
            "add a calendar with `edit`, or export again from P6 with its calendars",
        ));
    }
    let written = msproject_write::write(schedule);
    create_new_text(&written.text, output)?;
    Ok(Exported {
        schema: "totalfloat.export/1",
        source: source.to_owned(),
        output: output.display().to_string(),
        format: "msproject",
        notes: written.notes,
    })
}

/// Runs `export`.
pub fn run_export(
    input: &Path,
    (format, project): (Format, Option<u64>),
    output: &Path,
    json: bool,
) -> Result<u8, CliError> {
    let loaded = Loaded::new(crate::location::read(input)?);
    let report = export_file(
        &input.display().to_string(),
        &loaded,
        (format, project),
        output,
    )?;
    if json {
        print_line(&serde_json::to_string_pretty(&report).unwrap_or_default());
    } else {
        let mut lines = note_lines(&report.notes);
        lines.push(format!("Wrote {}", report.output));
        print_line(&lines.join("\n"));
    }
    Ok(exit::OK)
}

/// Converts `input` into a new XER at `output`. The source is never changed.
pub fn to_file(input: &Path, output: &Path) -> Result<Report, CliError> {
    refuse_existing(output)?;
    let text = read_text(input)?;
    let (report, raw) = convert(
        &input.display().to_string(),
        &output.display().to_string(),
        &text,
    )?;
    create_new(&raw, output)?;
    Ok(report)
}

/// Runs the command.
pub fn run(input: &Path, output: &Path, json: bool) -> Result<u8, CliError> {
    let report = to_file(input, output)?;
    if json {
        print_line(&serde_json::to_string_pretty(&report).unwrap_or_default());
    } else {
        print_line(&human(&report));
    }
    Ok(exit::OK)
}
