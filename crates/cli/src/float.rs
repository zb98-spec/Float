//! `float`: how a project's unfinished work spreads across total-float bands,
//! from negative float through critical to plenty, in working days.
//!
//! Float is what the file stores, as `query` and `check` read it, converted to
//! days with each activity's calendar, or 8 hours where a calendar doesn't say.
//! Level of effort and WBS summaries are left out, as `check` leaves them out.

use std::path::Path;

use serde::Serialize;
use totalfloat_model::{Activity, ActivityStatus, ActivityType, Schedule};

use crate::error::{CliError, exit};
use crate::loaded::Loaded;
use crate::output::{print, print_line};
use crate::query::critical;

/// Activity codes listed per band. `activities` counts them all.
pub const MAX_LISTED: usize = 50;

/// The upper bounds of the bands above zero, in working days, by default.
pub const DEFAULT_BOUNDS: [f64; 4] = [10.0, 20.0, 44.0, f64::INFINITY];

/// Hours in a working day, where a calendar doesn't say.
const HOURS_PER_DAY: f64 = 8.0;

/// `totalfloat.float/1`.
#[derive(Debug, Serialize)]
pub struct Report {
    schema: &'static str,
    file: String,
    projects: Vec<ProjectBands>,
}

/// One project's bands.
#[derive(Debug, Serialize)]
pub struct ProjectBands {
    /// `PROJECT.proj_id`.
    project: u64,
    /// `PROJECT.proj_short_name`.
    code: String,
    /// Unfinished activities counted, leaving out level of effort and WBS
    /// summaries.
    unfinished: usize,
    /// Of those, critical by the project's own definition.
    critical: usize,
    /// Of those, the file stores no total float for.
    without_float: usize,
    /// In order, from negative float up.
    bands: Vec<Band>,
}

/// One band.
#[derive(Debug, Serialize)]
pub struct Band {
    /// `negative`, `zero`, or `positive` for the bands above zero.
    kind: &'static str,
    /// In words, such as `over 10 to 20 days`.
    label: String,
    /// Of a positive band, its lower bound in working days, exclusive.
    above_days: Option<f64>,
    /// Of a positive band, its upper bound in working days, inclusive, or
    /// none for the highest band.
    to_days: Option<f64>,
    /// Activities in the band.
    activities: usize,
    /// Of the unfinished activities with float, to one decimal place.
    share_percent: f64,
    /// The first [`MAX_LISTED`] activity codes, lowest float first.
    listed: Vec<String>,
}

/// Parses `--bands`: upper bounds in working days above zero, ascending.
///
/// # Errors
///
/// A bound that isn't a number above zero, or bounds out of order.
pub fn bounds(text: &str) -> Result<Vec<f64>, CliError> {
    let bad = |why: String| {
        CliError::new(
            "bad-bands",
            why,
            "give working days above zero, ascending, such as --bands 5,10,20",
        )
    };
    let mut bounds = Vec::new();
    for part in text.split(',') {
        let part = part.trim();
        let days: f64 = part
            .parse()
            .map_err(|_| bad(format!("{part:?} isn't a number of days")))?;
        if !days.is_finite() || days <= 0.0 {
            return Err(bad(format!("{part} isn't above zero")));
        }
        if bounds.last().is_some_and(|&last| days <= last) {
            return Err(bad(format!("{part} isn't above the bound before it")));
        }
        bounds.push(days);
    }
    bounds.push(f64::INFINITY);
    Ok(bounds)
}

const fn is_span(activity: &Activity) -> bool {
    matches!(
        activity.activity_type,
        ActivityType::LevelOfEffort | ActivityType::WbsSummary
    )
}

/// An activity's total float in working days of its calendar.
fn float_days(schedule: &Schedule, activity: &Activity) -> Option<f64> {
    let float = activity.stored.total_float?;
    let day = schedule
        .calendars
        .get(&activity.calendar)
        .and_then(|c| c.hours_per_period.day)
        .filter(|day| day.get() > 0);
    #[allow(
        clippy::cast_precision_loss,
        reason = "minutes of float and of a day are far below 2^52"
    )]
    let days = float.get() as f64 / day.map_or(HOURS_PER_DAY * 60.0, |d| d.get() as f64);
    Some(days)
}

/// Days as the labels write them: whole numbers without a fraction.
fn days_text(days: f64) -> String {
    let text = format!("{days:.2}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// A band's kind, bounds and label.
type Edge = (&'static str, Option<f64>, Option<f64>, String);

/// The bands' edges: negative, zero, then each bound.
fn edges(bounds: &[f64]) -> Vec<Edge> {
    let mut edges = vec![
        ("negative", None, None, "below 0 days".to_owned()),
        ("zero", None, None, "0 days".to_owned()),
    ];
    let mut above = 0.0;
    for &to in bounds {
        if to.is_finite() {
            edges.push((
                "positive",
                Some(above),
                Some(to),
                format!("over {} to {} days", days_text(above), days_text(to)),
            ));
            above = to;
        } else {
            edges.push((
                "positive",
                Some(above),
                None,
                format!("over {} days", days_text(above)),
            ));
        }
    }
    edges
}

/// Which of [`edges`] float falls in.
fn band_of(days: f64, bounds: &[f64]) -> usize {
    if days < 0.0 {
        0
    } else if days == 0.0 {
        1
    } else {
        2 + bounds
            .iter()
            .position(|&to| days <= to)
            .unwrap_or(bounds.len() - 1)
    }
}

fn percent(part: usize, whole: usize) -> f64 {
    if whole == 0 {
        return 0.0;
    }
    #[allow(
        clippy::cast_precision_loss,
        reason = "display rates; counts are far below 2^52"
    )]
    let rate = part as f64 / whole as f64 * 100.0;
    (rate * 10.0).round() / 10.0
}

/// One project's bands, with `bounds` from [`bounds`].
#[must_use]
pub fn project_bands(schedule: &Schedule, bounds: &[f64]) -> ProjectBands {
    let unfinished: Vec<&Activity> = schedule
        .activities
        .values()
        .filter(|a| a.status != ActivityStatus::Completed && !is_span(a))
        .collect();
    let mut with_float: Vec<(f64, &Activity)> = unfinished
        .iter()
        .filter_map(|a| Some((float_days(schedule, a)?, *a)))
        .collect();
    // Lowest float first, then by code, so the lists read in order of urgency.
    with_float.sort_by(|(x, a), (y, b)| x.total_cmp(y).then_with(|| a.code.cmp(&b.code)));
    let mut bands: Vec<Band> = edges(bounds)
        .into_iter()
        .map(|(kind, above_days, to_days, label)| Band {
            kind,
            label,
            above_days,
            to_days,
            activities: 0,
            share_percent: 0.0,
            listed: Vec::new(),
        })
        .collect();
    for (days, activity) in &with_float {
        let band = &mut bands[band_of(*days, bounds)];
        band.activities += 1;
        if band.listed.len() < MAX_LISTED {
            band.listed.push(activity.code.clone());
        }
    }
    for band in &mut bands {
        band.share_percent = percent(band.activities, with_float.len());
    }
    ProjectBands {
        project: schedule.project.id.0,
        code: schedule.project.short_name.clone(),
        unfinished: unfinished.len(),
        critical: unfinished.iter().filter(|a| critical(schedule, a)).count(),
        without_float: unfinished.len() - with_float.len(),
        bands,
    }
}

/// The report for a loaded file, every project or one.
///
/// # Errors
///
/// A project the file doesn't hold.
pub fn report(
    file: &str,
    loaded: &Loaded,
    project: Option<u64>,
    bounds: &[f64],
) -> Result<Report, CliError> {
    let projects: Vec<ProjectBands> = loaded
        .schedules
        .iter()
        .filter(|s| project.is_none_or(|id| s.project.id.0 == id))
        .map(|s| project_bands(s, bounds))
        .collect();
    if projects.is_empty() {
        return Err(CliError::new(
            "no-such-project",
            project.map_or_else(
                || "the file holds no projects".to_owned(),
                |id| format!("the file has no project {id}"),
            ),
            "`inspect` lists the projects in the file",
        ));
    }
    Ok(Report {
        schema: "totalfloat.float/1",
        file: file.to_owned(),
        projects,
    })
}

fn human(report: &Report) -> String {
    let mut lines = Vec::new();
    for project in &report.projects {
        lines.push(format!(
            "Project {} ({}): {} unfinished, {} critical",
            project.code, project.project, project.unfinished, project.critical
        ));
        let width = project
            .bands
            .iter()
            .map(|b| b.label.len())
            .max()
            .unwrap_or(0);
        for band in &project.bands {
            lines.push(format!(
                "  {:<width$}  {:>6}  {:>5.1}%",
                band.label, band.activities, band.share_percent
            ));
        }
        if project.without_float > 0 {
            lines.push(format!(
                "  {} with no stored float; schedule the file first",
                project.without_float
            ));
        }
    }
    lines.join("\n") + "\n"
}

/// Runs `float`.
///
/// # Errors
///
/// An unreadable file, bad bands, or a project the file doesn't hold.
pub fn run(
    file: &Path,
    project: Option<u64>,
    bands: Option<&str>,
    json: bool,
) -> Result<u8, CliError> {
    let bounds = match bands {
        Some(text) => bounds(text)?,
        None => DEFAULT_BOUNDS.to_vec(),
    };
    let loaded = Loaded::new(crate::location::read(file)?);
    let report = report(&file.display().to_string(), &loaded, project, &bounds)?;
    if json {
        print_line(&serde_json::to_string_pretty(&report).unwrap_or_default());
    } else {
        print(&human(&report));
    }
    Ok(exit::OK)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bands_parse_ascending_days_above_zero() {
        assert_eq!(
            bounds("5, 10,20").unwrap(),
            [5.0, 10.0, 20.0, f64::INFINITY]
        );
        for bad in ["", "0", "-5", "10,5", "5,5", "x", "inf", "NaN"] {
            assert!(bounds(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn each_float_falls_in_one_band() {
        let bounds = DEFAULT_BOUNDS;
        assert_eq!(band_of(-0.5, &bounds), 0);
        assert_eq!(band_of(0.0, &bounds), 1);
        assert_eq!(band_of(0.1, &bounds), 2);
        assert_eq!(band_of(10.0, &bounds), 2);
        assert_eq!(band_of(10.1, &bounds), 3);
        assert_eq!(band_of(44.0, &bounds), 4);
        assert_eq!(band_of(44.5, &bounds), 5);
        assert_eq!(edges(&bounds).len(), 6);
        assert_eq!(edges(&bounds)[5].3, "over 44 days");
        assert_eq!(edges(&[2.5, f64::INFINITY])[2].3, "over 0 to 2.5 days");
    }
}
