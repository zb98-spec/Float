//! Schedule quality checks for TotalFloat.
//!
//! The DCMA 14-point assessment and further checks, run over a schedule model
//! and its engine results. Values are what the file stores, as a planner sees
//! them, falling back to the engine's. See docs/CHECKS.md.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use totalfloat_engine::{
    Calculated, ScheduleResult, out_of_sequence_with_projects, schedule_with_projects_reusing,
};
use totalfloat_model::{
    Activity, ActivityId, ActivityStatus, ActivityType, ConstraintType, Minutes, RelationshipType,
    Schedule,
};

/// Activities or relationships listed per check. `offenders` counts them all.
pub const MAX_LISTED: usize = 50;

/// Days added in the critical path test.
const TEST_DELAY_DAYS: i64 = 600;

/// Thresholds. Percentages are of each check's population.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Thresholds {
    /// Unfinished activities missing a predecessor or successor.
    pub logic_percent: f64,
    /// Relationships with a lead.
    pub leads_percent: f64,
    /// Relationships with a lag.
    pub lags_percent: f64,
    /// Finish-to-start relationships, at least.
    pub finish_to_start_percent: f64,
    /// Unfinished activities with a hard constraint.
    pub hard_constraints_percent: f64,
    /// Working days of total float that count as high.
    pub high_float_days: f64,
    /// Unfinished activities with high float.
    pub high_float_percent: f64,
    /// Unfinished activities with negative float.
    pub negative_float_percent: f64,
    /// Working days of duration that count as high.
    pub high_duration_days: f64,
    /// Unfinished activities with high duration.
    pub high_duration_percent: f64,
    /// Unfinished activities with work and no resource.
    pub resources_percent: f64,
    /// Critical path length index, at least.
    pub critical_path_length_index: f64,
    /// Unfinished activities whose float a calendar gap changes.
    pub calendar_gap_percent: f64,
    /// Working days of lag that count as long.
    pub long_lag_days: f64,
    /// Hours in a working day, where a calendar doesn't say.
    pub hours_per_day: f64,
    /// Predecessors above which an unfinished activity is a merge hotspot.
    pub merge_predecessors: usize,
    /// Unfinished activities that are merge hotspots.
    pub merge_hotspot_percent: f64,
}

impl Default for Thresholds {
    fn default() -> Self {
        Self {
            logic_percent: 5.0,
            leads_percent: 0.0,
            lags_percent: 5.0,
            finish_to_start_percent: 90.0,
            hard_constraints_percent: 5.0,
            high_float_days: 44.0,
            high_float_percent: 5.0,
            negative_float_percent: 0.0,
            calendar_gap_percent: 5.0,
            high_duration_days: 44.0,
            high_duration_percent: 5.0,
            resources_percent: 0.0,
            critical_path_length_index: 0.95,
            long_lag_days: 44.0,
            hours_per_day: 8.0,
            merge_predecessors: 10,
            merge_hotspot_percent: 5.0,
        }
    }
}

/// How a check came out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Outcome {
    /// Within its threshold.
    Pass,
    /// Outside its threshold.
    Fail,
    /// The file doesn't hold what the check needs.
    NotApplicable,
}

/// One check on one project.
#[derive(Debug, Clone, PartialEq, Serialize, schemars::JsonSchema)]
pub struct Check {
    /// Stable identifier, such as `dcma-01-logic`.
    pub id: &'static str,
    /// Short name.
    pub name: &'static str,
    /// How it came out.
    pub outcome: Outcome,
    /// What the measure is, in words.
    pub measure: String,
    /// Items outside the rule, counting all of them.
    pub offenders: usize,
    /// Items the check looked at.
    pub population: usize,
    /// The first [`MAX_LISTED`] activity codes or relationships outside the rule.
    pub listed: Vec<String>,
}

/// All checks on one project, DCMA first.
#[derive(Debug, Clone, PartialEq, Serialize, schemars::JsonSchema)]
pub struct Assessment {
    /// Internal project ID.
    pub project: u64,
    /// `PROJECT.proj_short_name`.
    pub code: String,
    /// In a fixed order.
    pub checks: Vec<Check>,
}

impl Assessment {
    /// Checks that failed.
    #[must_use]
    pub fn failed(&self) -> usize {
        self.checks
            .iter()
            .filter(|check| check.outcome == Outcome::Fail)
            .count()
    }
}

fn percent(part: usize, whole: usize) -> f64 {
    #[allow(
        clippy::cast_precision_loss,
        reason = "display rates; counts are far below 2^52"
    )]
    let rate = part as f64 / whole as f64 * 100.0;
    rate
}

/// Items outside a rule: how many, and the first [`MAX_LISTED`] written out.
/// Only those are written, so a rule many items break costs no more to list
/// than one few do.
struct Offenders {
    count: usize,
    listed: Vec<String>,
}

impl Offenders {
    fn of<T>(items: impl Iterator<Item = T>, label: impl Fn(T) -> String) -> Self {
        let mut listed = Vec::new();
        let mut count = 0;
        for item in items {
            if count < MAX_LISTED {
                listed.push(label(item));
            }
            count += 1;
        }
        Self { count, listed }
    }

    /// Ones already written out, all counted and the first [`MAX_LISTED`]
    /// kept.
    fn written(mut listed: Vec<String>) -> Self {
        let count = listed.len();
        listed.truncate(MAX_LISTED);
        Self { count, listed }
    }
}

/// `share_of`, with the offenders written out.
#[cfg(test)]
fn share(
    head: (&'static str, &'static str),
    offenders: Vec<String>,
    population: usize,
    limit: f64,
) -> Check {
    share_of(head, Offenders::written(offenders), population, limit)
}

/// A check that passes when offenders stay at or under a share of its population.
fn share_of(
    head: (&'static str, &'static str),
    offenders: Offenders,
    population: usize,
    limit: f64,
) -> Check {
    let count = offenders.count;
    let (outcome, measure) = if population == 0 {
        (Outcome::NotApplicable, "nothing to check".to_owned())
    } else {
        let rate = percent(count, population);
        // On the counts, so a rounded rate can't push an exact limit either way.
        #[allow(
            clippy::cast_precision_loss,
            reason = "counts of activities are far below 2^52"
        )]
        let within = count as f64 * 100.0 <= limit * population as f64;
        let outcome = if within { Outcome::Pass } else { Outcome::Fail };
        (outcome, format!("{rate:.1}%, limit {limit}%"))
    };
    finish(head, outcome, measure, offenders, population)
}

fn finish(
    (id, name): (&'static str, &'static str),
    outcome: Outcome,
    measure: String,
    offenders: Offenders,
    population: usize,
) -> Check {
    Check {
        id,
        name,
        outcome,
        measure,
        offenders: offenders.count,
        population,
        listed: offenders.listed,
    }
}

fn not_applicable(head: (&'static str, &'static str), why: &str) -> Check {
    let none = Offenders::written(Vec::new());
    finish(head, Outcome::NotApplicable, why.to_owned(), none, 0)
}

/// What the checks read, once.
struct Facts<'a> {
    schedule: &'a Schedule,
    result: Option<&'a ScheduleResult>,
    /// The calculation `result` comes from, for the critical path test to
    /// take from.
    calculated: Option<&'a Calculated>,
    thresholds: &'a Thresholds,
    /// Unfinished work, leaving out what only measures other work.
    unfinished: Vec<&'a Activity>,
    /// Activities with a predecessor, sorted.
    has_pred: Vec<ActivityId>,
    /// Activities with a successor, sorted.
    has_succ: Vec<ActivityId>,
    /// Activities with a resource assignment, sorted.
    assigned: Vec<ActivityId>,
    /// The file's other projects, for links to them, or none when the
    /// project ignores relationships to other projects.
    others: &'a [Schedule],
}

const fn is_span(activity: &Activity) -> bool {
    matches!(
        activity.activity_type,
        ActivityType::LevelOfEffort | ActivityType::WbsSummary
    )
}

fn codes<'a>(activities: impl Iterator<Item = &'a Activity>) -> Offenders {
    Offenders::of(activities, |a| a.code.clone())
}

/// `ids`, sorted, once each, to search.
fn sorted(ids: impl Iterator<Item = ActivityId>) -> Vec<ActivityId> {
    let mut ids: Vec<ActivityId> = ids.collect();
    ids.sort_unstable();
    ids.dedup();
    ids
}

impl Facts<'_> {
    fn links(&self, activity: ActivityId) -> (bool, bool) {
        (
            self.has_pred.binary_search(&activity).is_ok(),
            self.has_succ.binary_search(&activity).is_ok(),
        )
    }

    fn day(&self, activity: &Activity) -> f64 {
        let calendar = self.schedule.calendars.get(&activity.calendar);
        let day = calendar.and_then(|c| c.hours_per_period.day);
        #[allow(
            clippy::cast_precision_loss,
            reason = "minutes in a day are far below 2^52"
        )]
        day.filter(|day| day.get() > 0)
            .map_or(self.thresholds.hours_per_day * 60.0, |day| day.get() as f64)
    }

    fn days(&self, activity: &Activity, span: Minutes) -> f64 {
        #[allow(clippy::cast_precision_loss, reason = "minutes are far below 2^52")]
        let minutes = span.get() as f64;
        minutes / self.day(activity)
    }

    fn total_float(&self, activity: &Activity) -> Option<Minutes> {
        activity.stored.total_float.or_else(|| {
            self.result?
                .activities
                .get(&activity.id)?
                .values
                .total_float
        })
    }

    fn logic(&self) -> Check {
        let missing = self.unfinished.iter().copied().filter(|a| {
            let (has_pred, has_succ) = self.links(a.id);
            !has_pred || !has_succ
        });
        share_of(
            ("dcma-01-logic", "Logic"),
            codes(missing),
            self.unfinished.len(),
            self.thresholds.logic_percent,
        )
    }

    fn link_label(&self, predecessor: ActivityId, successor: ActivityId) -> String {
        let code = |id: ActivityId| {
            self.schedule
                .activities
                .get(&id)
                .map_or_else(|| id.0.to_string(), |a| a.code.clone())
        };
        format!("{} to {}", code(predecessor), code(successor))
    }

    /// [`Self::link_label`], with an end in another project named by its code
    /// there.
    fn link_label_across(&self, predecessor: ActivityId, successor: ActivityId) -> String {
        let code = |id: ActivityId| {
            std::iter::once(self.schedule)
                .chain(self.others)
                .find_map(|s| s.activities.get(&id))
                .map_or_else(|| id.0.to_string(), |a| a.code.clone())
        };
        format!("{} to {}", code(predecessor), code(successor))
    }

    fn relationships_where(
        &self,
        rule: impl Fn(&totalfloat_model::Relationship) -> bool,
    ) -> Offenders {
        let links = self
            .schedule
            .relationships
            .values()
            .filter(|link| rule(link));
        Offenders::of(links, |link| {
            self.link_label(link.predecessor, link.successor)
        })
    }

    fn leads(&self) -> Check {
        share_of(
            ("dcma-02-leads", "Leads"),
            self.relationships_where(|link| link.lag.get() < 0),
            self.schedule.relationships.len(),
            self.thresholds.leads_percent,
        )
    }

    fn lags(&self) -> Check {
        share_of(
            ("dcma-03-lags", "Lags"),
            self.relationships_where(|link| link.lag.get() > 0),
            self.schedule.relationships.len(),
            self.thresholds.lags_percent,
        )
    }

    fn relationship_types(&self) -> Check {
        share_of(
            ("dcma-04-relationship-types", "Relationship types"),
            self.relationships_where(|link| link.kind != RelationshipType::FinishToStart),
            self.schedule.relationships.len(),
            100.0 - self.thresholds.finish_to_start_percent,
        )
    }

    fn hard_constraints(&self) -> Check {
        let hard = |a: &&Activity| {
            [a.primary_constraint, a.secondary_constraint]
                .into_iter()
                .flatten()
                .any(|c| {
                    matches!(
                        c.kind,
                        ConstraintType::StartOn
                            | ConstraintType::StartOnOrBefore
                            | ConstraintType::FinishOn
                            | ConstraintType::FinishOnOrBefore
                            | ConstraintType::MandatoryStart
                            | ConstraintType::MandatoryFinish
                    )
                })
        };
        share_of(
            ("dcma-05-hard-constraints", "Hard constraints"),
            codes(self.unfinished.iter().copied().filter(hard)),
            self.unfinished.len(),
            self.thresholds.hard_constraints_percent,
        )
    }

    fn with_float(&self) -> Vec<(&Activity, Minutes)> {
        self.unfinished
            .iter()
            .copied()
            .filter_map(|a| Some((a, self.total_float(a)?)))
            .collect()
    }

    fn high_float(&self) -> Check {
        let floats = self.with_float();
        let limit = self.thresholds.high_float_days;
        let high = floats
            .iter()
            .filter(|(a, float)| self.days(a, *float) > limit)
            .map(|(a, _)| *a);
        share_of(
            ("dcma-06-high-float", "High float"),
            codes(high),
            floats.len(),
            self.thresholds.high_float_percent,
        )
    }

    fn negative_float(&self) -> Check {
        let floats = self.with_float();
        let negative = floats
            .iter()
            .filter(|(_, float)| float.get() < 0)
            .map(|(a, _)| *a);
        share_of(
            ("dcma-07-negative-float", "Negative float"),
            codes(negative),
            floats.len(),
            self.thresholds.negative_float_percent,
        )
    }

    fn high_duration(&self) -> Check {
        let limit = self.thresholds.high_duration_days;
        let high = self
            .unfinished
            .iter()
            .copied()
            .filter(|a| self.days(a, a.original_duration) > limit);
        share_of(
            ("dcma-08-high-duration", "High duration"),
            codes(high),
            self.unfinished.len(),
            self.thresholds.high_duration_percent,
        )
    }

    fn invalid_dates(&self) -> Check {
        let head = ("dcma-09-invalid-dates", "Invalid dates");
        let Some(data_date) = self.schedule.project.data_date else {
            return not_applicable(head, "the project has no data date");
        };
        let activities = self.schedule.activities.values();
        let invalid = activities.clone().filter(|a| {
            let actual_ahead = [a.actual_start, a.actual_finish]
                .into_iter()
                .flatten()
                .any(|date| date > data_date);
            // Work under way started in the past, but can't still finish there.
            let start_behind = a.status == ActivityStatus::NotStarted
                && a.stored.early_start.is_some_and(|date| date < data_date);
            let finish_behind = a.status != ActivityStatus::Completed
                && a.stored.early_finish.is_some_and(|date| date < data_date);
            let forecast_behind = start_behind || finish_behind;
            actual_ahead || forecast_behind
        });
        share_of(head, codes(invalid), activities.count(), 0.0)
    }

    fn resources(&self) -> Check {
        let worked =
            |a: &&&Activity| !a.activity_type.is_milestone() && a.remaining_duration.get() > 0;
        let population: Vec<&Activity> = self
            .unfinished
            .iter()
            .filter(worked)
            .map(|a| &**a)
            .collect();
        share_of(
            ("dcma-10-resources", "Resources"),
            codes(
                population
                    .iter()
                    .copied()
                    .filter(|a| self.assigned.binary_search(&a.id).is_err()),
            ),
            population.len(),
            self.thresholds.resources_percent,
        )
    }

    fn critical_path_test(&self, projects: &[Schedule]) -> Check {
        let head = ("dcma-12-critical-path-test", "Critical path test");
        let Some(result) = self.result else {
            return not_applicable(head, "the schedule can't be calculated");
        };
        let target = self.schedule.activities.values().find(|a| {
            a.status != ActivityStatus::Completed
                && !a.activity_type.is_milestone()
                && !is_span(a)
                && result
                    .activities
                    .get(&a.id)
                    .is_some_and(|r| r.values.on_longest_path == Some(true))
        });
        // The project's own finish: with float from the opened projects'
        // finish, the scheduled finish can be another project's.
        let (Some(target), Some(finish)) = (target, result.project.network_finish) else {
            return not_applicable(head, "no unfinished work on the longest path");
        };
        let delay = minutes_of_days(TEST_DELAY_DAYS, self.day(target));
        let mut delayed = self.schedule.clone();
        if let Some(a) = delayed.activities.get_mut(&target.id) {
            a.remaining_duration = Minutes(a.remaining_duration.get() + delay);
        }
        let options = &delayed.project.options;
        // Only the target and what follows it move forward, so the forward
        // pass takes the rest from the project's own calculation.
        let moved = schedule_with_projects_reusing(&delayed, options, projects, self.calculated)
            .ok()
            .and_then(|r| r.result().project.network_finish);
        let Some(moved) = moved else {
            return not_applicable(head, "the delayed schedule can't be calculated");
        };
        let outcome = if moved > finish {
            Outcome::Pass
        } else {
            Outcome::Fail
        };
        let measure = format!(
            "{} delayed {TEST_DELAY_DAYS} days: finish {finish} to {moved}",
            target.code
        );
        let offenders = if outcome == Outcome::Fail {
            vec![target.code.clone()]
        } else {
            Vec::new()
        };
        finish_test(head, outcome, measure, Offenders::written(offenders))
    }

    /// (critical path length + float to the required finish) / length.
    fn critical_path_length_index(&self) -> Check {
        let head = ("dcma-13-cpli", "Critical path length index");
        let project = &self.schedule.project;
        let Some(required) = project.must_finish_by else {
            return not_applicable(head, "the project has no required finish date");
        };
        let finish = project
            .stored
            .scheduled_finish
            .or_else(|| self.result?.project.scheduled_finish);
        let (Some(finish), Some(data_date)) = (finish, project.data_date) else {
            return not_applicable(head, "the project has no finish or data date");
        };
        let length = finish.minutes_since(data_date).get();
        if length <= 0 {
            return not_applicable(head, "no critical path remains");
        }
        let float = required.minutes_since(finish).get();
        #[allow(clippy::cast_precision_loss, reason = "minutes are far below 2^52")]
        let index = (length + float) as f64 / length as f64;
        let limit = self.thresholds.critical_path_length_index;
        let outcome = if index >= limit {
            Outcome::Pass
        } else {
            Outcome::Fail
        };
        finish_test(
            head,
            outcome,
            format!("{index:.2}, limit {limit}, in elapsed time"),
            Offenders::written(Vec::new()),
        )
    }

    fn open_ends(&self) -> Check {
        let without = |side: fn((bool, bool)) -> bool| {
            self.unfinished
                .iter()
                .copied()
                .filter(move |a| !side(self.links(a.id)))
        };
        // One activity normally starts the project and one ends it, so the
        // first to start and the last to finish are no finding. Once work has
        // finished the start is behind us, and nothing open can be it.
        let begun = self
            .schedule
            .activities
            .values()
            .any(|a| a.status == ActivityStatus::Completed);
        let first = without(|links| links.0)
            .min_by_key(|a| (a.stored.early_start.is_none(), a.stored.early_start))
            .filter(|_| !begun);
        let last = without(|links| links.1).max_by_key(|a| a.stored.early_finish);
        let open = self.unfinished.iter().copied().filter(|a| {
            let (has_pred, has_succ) = self.links(a.id);
            let is = |excused: Option<&Activity>| excused.is_some_and(|e| e.id == a.id);
            (!has_pred && !is(first)) || (!has_succ && !is(last))
        });
        share_of(
            ("open-ends", "Open ends"),
            codes(open),
            self.unfinished.len(),
            0.0,
        )
    }

    fn out_of_sequence(&self) -> Check {
        let broken = out_of_sequence_with_projects(self.schedule, self.others);
        let links = broken
            .iter()
            .filter_map(|id| self.schedule.relationships.get(id));
        let labels = Offenders::of(links, |link| {
            self.link_label_across(link.predecessor, link.successor)
        });
        share_of(
            ("out-of-sequence", "Out-of-sequence progress"),
            labels,
            self.schedule.relationships.len(),
            0.0,
        )
    }

    /// Mandatory dates win over logic, so one on an activity with a predecessor
    /// can override it.
    fn constraints_overriding_logic(&self) -> Check {
        let overriding = self.unfinished.iter().copied().filter(|a| {
            let mandatory = [a.primary_constraint, a.secondary_constraint]
                .into_iter()
                .flatten()
                .any(|c| {
                    matches!(
                        c.kind,
                        ConstraintType::MandatoryStart | ConstraintType::MandatoryFinish
                    )
                });
            mandatory && self.links(a.id).0
        });
        share_of(
            (
                "constraints-overriding-logic",
                "Constraints overriding logic",
            ),
            codes(overriding),
            self.unfinished.len(),
            0.0,
        )
    }

    fn long_lags(&self) -> Check {
        let limit = self.thresholds.long_lag_days;
        let long = |link: &totalfloat_model::Relationship| {
            let lag = Minutes(link.lag.get().abs());
            let activities = &self.schedule.activities;
            activities
                .get(&link.predecessor)
                .or_else(|| activities.get(&link.successor))
                .is_some_and(|a| self.days(a, lag) > limit)
        };
        share_of(
            ("long-lags", "Long lags"),
            self.relationships_where(long),
            self.schedule.relationships.len(),
            0.0,
        )
    }

    /// Activities where float differs from the truth because of a calendar gap.
    fn calendar_gap_float(&self) -> Check {
        let head = ("calendar-gap-float", "Float lost to a calendar gap");
        let Some(result) = self.result else {
            return not_applicable(head, "the schedule can't be calculated");
        };
        #[allow(clippy::cast_precision_loss, reason = "minutes are far below 2^52")]
        let hours = |float: Minutes| float.get() as f64 / 60.0;
        let gaps = self.unfinished.iter().filter_map(|a| {
            let found = result.activities.get(&a.id)?;
            Some((a, found.values.total_float?, found.true_total_float?))
        });
        let listed = Offenders::of(gaps, |(a, shown, truth)| {
            format!(
                "{}: {:.1}h total float, {:.1}h true",
                a.code,
                hours(shown),
                hours(truth)
            )
        });
        let limit = self.thresholds.calendar_gap_percent;
        share_of(head, listed, self.unfinished.len(), limit)
    }

    /// Activities where many paths merge. Each path's chance of running late
    /// adds to the merge point's, so its forecast is optimistic: merge bias.
    fn merge_hotspots(&self) -> Check {
        let mut predecessors: std::collections::BTreeMap<ActivityId, Vec<ActivityId>> =
            std::collections::BTreeMap::new();
        for link in self.schedule.relationships.values() {
            predecessors
                .entry(link.successor)
                .or_default()
                .push(link.predecessor);
        }
        for list in predecessors.values_mut() {
            list.sort_unstable();
            list.dedup();
        }
        let limit = self.thresholds.merge_predecessors;
        let hotspots = self.unfinished.iter().filter_map(|a| {
            let count = predecessors.get(&a.id).map_or(0, Vec::len);
            (count > limit).then_some((a, count))
        });
        share_of(
            ("merge-hotspots", "Merge hotspots"),
            Offenders::of(hotspots, |(a, count)| {
                format!("{}: {count} predecessors", a.code)
            }),
            self.unfinished.len(),
            self.thresholds.merge_hotspot_percent,
        )
    }

    fn start_to_finish(&self) -> Check {
        share_of(
            ("start-to-finish", "Start-to-finish relationships"),
            self.relationships_where(|link| link.kind == RelationshipType::StartToFinish),
            self.schedule.relationships.len(),
            0.0,
        )
    }
}

fn finish_test(
    head: (&'static str, &'static str),
    outcome: Outcome,
    measure: String,
    offenders: Offenders,
) -> Check {
    finish(head, outcome, measure, offenders, 1)
}

fn minutes_of_days(days: i64, day_minutes: f64) -> i64 {
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        reason = "a test delay of hundreds of days, in whole minutes"
    )]
    let minutes = (days as f64 * day_minutes).round() as i64;
    minutes
}

/// Runs every check on one project. `projects` holds any others in the file,
/// for links between them.
#[must_use]
pub fn assess(schedule: &Schedule, projects: &[Schedule], thresholds: &Thresholds) -> Assessment {
    let options = &schedule.project.options;
    let calculated = schedule_with_projects_reusing(schedule, options, projects, None).ok();
    assessed(schedule, projects, thresholds, calculated.as_ref())
}

/// [`assess`], with `calculated` standing for the calculation it makes:
/// `schedule` calculated among `projects` by
/// [`totalfloat_engine::schedule_with_projects`] with its own options. An
/// edit preview has just made it. Given any other calculation, the
/// checks that read results are wrong.
#[must_use]
pub fn assess_calculated(
    schedule: &Schedule,
    projects: &[Schedule],
    thresholds: &Thresholds,
    calculated: &Calculated,
) -> Assessment {
    assessed(schedule, projects, thresholds, Some(calculated))
}

fn assessed(
    schedule: &Schedule,
    projects: &[Schedule],
    thresholds: &Thresholds,
    calculated: Option<&Calculated>,
) -> Assessment {
    let options = &schedule.project.options;
    let result = calculated.map(Calculated::result);
    let unfinished = schedule
        .activities
        .values()
        .filter(|a| a.status != ActivityStatus::Completed && !is_span(a))
        .collect();
    let links = schedule.relationships.values();
    let facts = Facts {
        schedule,
        result,
        calculated,
        thresholds,
        unfinished,
        has_pred: sorted(links.clone().map(|link| link.successor)),
        has_succ: sorted(links.map(|link| link.predecessor)),
        assigned: sorted(schedule.assignments.values().map(|a| a.activity)),
        others: if options.ignore_external_relationships {
            &[]
        } else {
            projects
        },
    };
    let no_baseline = "the file holds no baseline";
    let checks = vec![
        facts.logic(),
        facts.leads(),
        facts.lags(),
        facts.relationship_types(),
        facts.hard_constraints(),
        facts.high_float(),
        facts.negative_float(),
        facts.high_duration(),
        facts.invalid_dates(),
        facts.resources(),
        not_applicable(("dcma-11-missed-tasks", "Missed tasks"), no_baseline),
        facts.critical_path_test(projects),
        facts.critical_path_length_index(),
        not_applicable(("dcma-14-bei", "Baseline execution index"), no_baseline),
        facts.open_ends(),
        facts.out_of_sequence(),
        facts.constraints_overriding_logic(),
        facts.long_lags(),
        facts.start_to_finish(),
        facts.calendar_gap_float(),
        facts.merge_hotspots(),
    ];
    Assessment {
        project: schedule.project.id.0,
        code: schedule.project.short_name.clone(),
        checks,
    }
}

#[cfg(test)]
mod tests;
