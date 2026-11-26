//! `export --to dot`: a project's activity network as a Graphviz file.
//!
//! One node per activity, one edge per link within the project, with the
//! critical path drawn in red. Dates and float are the ones the file stores,
//! as `query` shows them, so schedule the file first for fresh ones. Output is
//! deterministic: activities in ID order, links in ID order.

use std::fmt::Write as _;

use totalfloat_interop::Note;
use totalfloat_model::{Activity, ActivityStatus, Schedule};

use crate::output::hours_of;
use crate::query::{critical, link_label};

/// Examples listed per note.
const MAX_EXAMPLES: usize = 10;

/// Text inside a double-quoted DOT string. Graphviz reads `\n` in a label as
/// a line break, so only the escapes are kept literal.
fn quoted(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' | '\r' => out.push(' '),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A node's label lines: code, name, dates and float, joined by DOT's `\n`.
fn label(activity: &Activity) -> String {
    let lines = [
        quoted(&activity.code),
        quoted(&activity.name),
        match (activity.stored.early_start, activity.stored.early_finish) {
            (Some(start), Some(finish)) => quoted(&format!("{start} to {finish}")),
            _ => String::new(),
        },
        activity
            .stored
            .total_float
            .map(|f| quoted(&format!("TF {:.1}h", hours_of(f))))
            .unwrap_or_default(),
    ];
    // Each line was quoted on its own so it is escaped; join their insides.
    let inside: Vec<&str> = lines
        .iter()
        .filter(|line| !line.is_empty())
        .map(|line| &line[1..line.len() - 1])
        .collect();
    format!("\"{}\"", inside.join("\\n"))
}

fn node_id(activity: &Activity) -> String {
    format!("a{}", activity.id.0)
}

/// Writes an edge per link within the project, and notes those to other
/// projects, which are left out.
fn links(schedule: &Schedule, others: &[Schedule], out: &mut String) -> Vec<Note> {
    let is_critical = |a: &Activity| critical(schedule, a);
    let code = |id: totalfloat_model::ActivityId| {
        std::iter::once(schedule)
            .chain(others)
            .find_map(|s| s.activities.get(&id))
            .map_or_else(|| id.0.to_string(), |a| a.code.clone())
    };
    let mut external = Vec::new();
    let mut external_count = 0;
    for link in schedule.relationships.values() {
        let (Some(from), Some(to)) = (
            schedule.activities.get(&link.predecessor),
            schedule.activities.get(&link.successor),
        ) else {
            if external.len() < MAX_EXAMPLES {
                external.push(format!(
                    "{} to {}",
                    code(link.predecessor),
                    code(link.successor)
                ));
            }
            external_count += 1;
            continue;
        };
        let mut attributes = Vec::new();
        let kind = link_label(link.kind);
        let lag = link.lag.get();
        if lag != 0 {
            let hours = hours_of(link.lag);
            attributes.push(format!("label={}", quoted(&format!("{kind} {hours:+.1}h"))));
        } else if kind != "FS" {
            attributes.push(format!("label={}", quoted(kind)));
        }
        if is_critical(from) && is_critical(to) {
            attributes.push("color=\"#c0392b\"".to_owned());
            attributes.push("penwidth=2".to_owned());
        }
        let attributes = if attributes.is_empty() {
            String::new()
        } else {
            format!(" [{}]", attributes.join(", "))
        };
        let _ = writeln!(out, "  {} -> {}{attributes};", node_id(from), node_id(to));
    }
    let mut notes = Vec::new();
    if external_count > 0 {
        notes.push(Note {
            kind: "external-links",
            message: "links to activities in other projects are left out",
            count: external_count,
            examples: external,
        });
    }
    notes
}

/// The network as DOT text, and notes on what it leaves out. `others` are
/// the file's other projects, to name the ends of links to them.
#[must_use]
pub fn write(schedule: &Schedule, others: &[Schedule]) -> (String, Vec<Note>) {
    let mut out = String::new();
    let project = &schedule.project;
    let title = project.name.as_ref().map_or_else(
        || project.short_name.clone(),
        |name| format!("{}: {name}", project.short_name),
    );
    let _ = writeln!(out, "digraph {} {{", quoted(&project.short_name));
    let _ = writeln!(
        out,
        "  graph [rankdir=LR, labelloc=t, label={}, fontname=\"Helvetica\"];",
        quoted(&title)
    );
    out.push_str(
        "  node [shape=box, style=\"rounded,filled\", fillcolor=\"#ffffff\", \
         fontname=\"Helvetica\", fontsize=10];\n",
    );
    out.push_str("  edge [fontname=\"Helvetica\", fontsize=9];\n");
    let is_critical = |a: &Activity| critical(schedule, a);
    for activity in schedule.activities.values() {
        let mut attributes = vec![format!("label={}", label(activity))];
        if activity.activity_type.is_milestone() {
            attributes.push("shape=diamond".to_owned());
            attributes.push("style=filled".to_owned());
        }
        if activity.status == ActivityStatus::Completed {
            attributes.push("fillcolor=\"#e5e7eb\"".to_owned());
        } else if is_critical(activity) {
            attributes.push("fillcolor=\"#fde2e1\"".to_owned());
            attributes.push("color=\"#c0392b\"".to_owned());
            attributes.push("penwidth=2".to_owned());
        }
        let _ = writeln!(out, "  {} [{}];", node_id(activity), attributes.join(", "));
    }
    let mut notes = links(schedule, others, &mut out);
    out.push_str("}\n");
    let undated: Vec<&Activity> = schedule
        .activities
        .values()
        .filter(|a| a.stored.early_start.is_none() || a.stored.early_finish.is_none())
        .collect();
    if !undated.is_empty() {
        notes.push(Note {
            kind: "no-dates",
            message: "activities the file holds no early dates for are drawn without them; \
                      schedule the file first",
            count: undated.len(),
            examples: undated
                .iter()
                .take(MAX_EXAMPLES)
                .map(|a| a.code.clone())
                .collect(),
        });
    }
    (out, notes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoted_escapes_quotes_backslashes_and_line_breaks() {
        assert_eq!(quoted(r#"a "b" \c"#), r#""a \"b\" \\c""#);
        assert_eq!(quoted("one\ntwo"), "\"one two\"");
    }
}
