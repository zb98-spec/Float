//! `export --to ics`: a project's activities as an iCalendar file (RFC 5545),
//! for Outlook, Google Calendar and phones.
//!
//! One event per activity with dates. Work not started uses the early dates
//! the file stores, work under way its actual start and early finish, and
//! completed work its actual dates. Times
//! are written as floating local times, as TotalFloat's dates are civil
//! (time-zone free), so a calendar app shows them at the same clock time in
//! any zone. Output is deterministic: `DTSTAMP` is the project's data date,
//! not the time of the export, and each event's `UID` is stable, so importing
//! a later export updates events instead of adding copies.

use totalfloat_interop::Note;
use totalfloat_model::{Activity, ActivityStatus, DateTime, Schedule};

use crate::output::hours_of;
use crate::query::critical;

/// Examples listed per note.
const MAX_EXAMPLES: usize = 10;

/// The longest content line, in octets, before it is folded (RFC 5545 §3.1).
const LINE_OCTETS: usize = 75;

/// A date as an iCalendar floating local time, `YYYYMMDDTHHMMSS`.
fn local(date: DateTime) -> String {
    let (year, month, day) = date.date().ymd();
    let time = date.time();
    format!(
        "{year:04}{month:02}{day:02}T{:02}{:02}00",
        time.hour(),
        time.minute()
    )
}

/// Text as an iCalendar TEXT value: backslash, semicolon, comma and line
/// breaks escaped (RFC 5545 §3.3.11).
fn text(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => out.push_str("\\\\"),
            ';' => out.push_str("\\;"),
            ',' => out.push_str("\\,"),
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                out.push_str("\\n");
            }
            '\n' => out.push_str("\\n"),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}

/// Appends one content line, folded at [`LINE_OCTETS`] without splitting a
/// character, and ended with CRLF.
fn line(out: &mut String, content: &str) {
    let mut width = 0;
    for c in content.chars() {
        let size = c.len_utf8();
        // A continuation line starts with a space, which counts.
        if width + size > LINE_OCTETS {
            out.push_str("\r\n ");
            width = 1;
        }
        out.push(c);
        width += size;
    }
    out.push_str("\r\n");
}

/// The activity's dates to show: actuals for completed work, the actual
/// start and early finish for work under way, early dates otherwise.
fn span(activity: &Activity) -> Option<(DateTime, DateTime)> {
    match activity.status {
        ActivityStatus::Completed => Some((activity.actual_start?, activity.actual_finish?)),
        ActivityStatus::InProgress => Some((
            activity.actual_start.or(activity.stored.early_start)?,
            activity.stored.early_finish?,
        )),
        ActivityStatus::NotStarted => {
            Some((activity.stored.early_start?, activity.stored.early_finish?))
        }
    }
}

/// A part of a UID: letters, digits, `-` and `.` kept, the rest as `_`.
fn uid_part(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// The activities as iCalendar text, and notes on what it leaves out.
#[must_use]
pub fn write(schedule: &Schedule) -> (String, Vec<Note>) {
    let project = &schedule.project;
    let stamp = project
        .data_date
        .or(project.planned_start)
        .map_or_else(|| "19700101T000000Z".to_owned(), |d| local(d) + "Z");
    let domain = uid_part(&project.short_name);
    let mut out = String::new();
    line(&mut out, "BEGIN:VCALENDAR");
    line(&mut out, "VERSION:2.0");
    line(&mut out, "PRODID:-//TotalFloat//TotalFloat//EN");
    line(&mut out, "CALSCALE:GREGORIAN");
    let title = project.name.as_ref().map_or_else(
        || project.short_name.clone(),
        |name| format!("{}: {name}", project.short_name),
    );
    line(&mut out, &format!("X-WR-CALNAME:{}", text(&title)));
    let mut undated = Vec::new();
    let mut undated_count = 0;
    for activity in schedule.activities.values() {
        let Some((start, finish)) = span(activity) else {
            if undated.len() < MAX_EXAMPLES {
                undated.push(activity.code.clone());
            }
            undated_count += 1;
            continue;
        };
        line(&mut out, "BEGIN:VEVENT");
        line(
            &mut out,
            &format!("UID:{}-{}@{domain}.totalfloat", project.id.0, activity.id.0),
        );
        line(&mut out, &format!("DTSTAMP:{stamp}"));
        line(&mut out, &format!("DTSTART:{}", local(start)));
        // A milestone, or any zero-length span, is an instant: no DTEND.
        if finish > start {
            line(&mut out, &format!("DTEND:{}", local(finish)));
        }
        line(
            &mut out,
            &format!(
                "SUMMARY:{}",
                text(&format!("{} {}", activity.code, activity.name))
            ),
        );
        let is_critical = critical(schedule, activity);
        let mut description = vec![format!("Status: {}", status(activity.status))];
        if let Some(float) = activity.stored.total_float {
            description.push(format!("Total float: {:.1}h", hours_of(float)));
        }
        if is_critical {
            description.push("Critical".to_owned());
        }
        line(
            &mut out,
            &format!("DESCRIPTION:{}", text(&description.join("\n"))),
        );
        let mut categories = Vec::new();
        if activity.activity_type.is_milestone() {
            categories.push("MILESTONE");
        }
        if is_critical {
            categories.push("CRITICAL");
        }
        if !categories.is_empty() {
            line(&mut out, &format!("CATEGORIES:{}", categories.join(",")));
        }
        line(&mut out, "TRANSP:TRANSPARENT");
        line(&mut out, "END:VEVENT");
    }
    line(&mut out, "END:VCALENDAR");
    let mut notes = Vec::new();
    if undated_count > 0 {
        notes.push(Note {
            kind: "no-dates",
            message: "activities the file holds no dates for are left out; schedule the file first",
            count: undated_count,
            examples: undated,
        });
    }
    (out, notes)
}

const fn status(status: ActivityStatus) -> &'static str {
    match status {
        ActivityStatus::NotStarted => "not started",
        ActivityStatus::InProgress => "in progress",
        ActivityStatus::Completed => "completed",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_escapes_what_rfc_5545_reserves() {
        assert_eq!(text("a;b,c\\d"), "a\\;b\\,c\\\\d");
        assert_eq!(text("one\r\ntwo\nthree"), "one\\ntwo\\nthree");
        assert_eq!(text("bell\u{7}"), "bell");
    }

    #[test]
    fn long_lines_fold_at_75_octets_without_splitting_a_character() {
        let mut out = String::new();
        let long = format!("SUMMARY:{}", "é".repeat(60));
        line(&mut out, &long);
        for folded in out.split("\r\n").filter(|l| !l.is_empty()) {
            assert!(folded.len() <= LINE_OCTETS, "{folded:?}");
        }
        assert_eq!(out.replace("\r\n ", ""), format!("{long}\r\n"));
    }

    #[test]
    fn dates_are_floating_local_times() {
        let date: DateTime = "2026-01-05 08:30".parse().unwrap();
        assert_eq!(local(date), "20260105T083000");
    }
}
