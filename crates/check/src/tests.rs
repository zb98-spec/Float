//! One authored schedule per check.

use std::collections::BTreeMap;

use totalfloat_model::{
    Activity, ActivityId, ActivityStatus, ActivityType, Assignment, AssignmentId, Calendar,
    CalendarId, CalendarType, Constraint, ConstraintType, DateTime, HoursPerPeriod, Minutes,
    Project, ProjectId, Relationship, RelationshipId, RelationshipType, ResourceId, Schedule,
    TimeOfDay, WbsId, WorkInterval, WorkWeek,
};

use super::{Outcome, Thresholds, assess};

const DAY: i64 = 480;

fn at(text: &str) -> DateTime {
    text.parse().unwrap()
}

/// A chain of `count` one-day tasks, each with a resource and stored float zero.
fn chain(count: u64) -> Schedule {
    let mut project = Project::new(ProjectId(1), "AUTHORED");
    project.planned_start = Some(at("2026-01-05 08:00"));
    project.data_date = project.planned_start;
    project.default_calendar = Some(CalendarId(1));
    let mut s = Schedule::new(project);
    let shift = WorkInterval::new(
        TimeOfDay::from_minutes(480).unwrap(),
        TimeOfDay::from_minutes(960).unwrap(),
    )
    .unwrap();
    s.calendars.insert(
        CalendarId(1),
        Calendar {
            id: CalendarId(1),
            name: "Authored".to_owned(),
            kind: CalendarType::Global,
            project: None,
            base: None,
            is_default: true,
            hours_per_period: HoursPerPeriod::default(),
            work_week: WorkWeek(std::array::from_fn(|day| {
                if (1..=5).contains(&day) {
                    vec![shift]
                } else {
                    vec![]
                }
            })),
            exceptions: BTreeMap::new(),
            source: None,
        },
    );
    for n in 1..=count {
        let mut a = Activity::new(
            ActivityId(n),
            ProjectId(1),
            WbsId(1),
            CalendarId(1),
            format!("A{n}"),
            "Authored",
        );
        a.original_duration = Minutes(DAY);
        a.remaining_duration = Minutes(DAY);
        a.stored.total_float = Some(Minutes::ZERO);
        s.activities.insert(a.id, a);
        s.assignments.insert(AssignmentId(n), assignment(n));
        if n > 1 {
            link(&mut s, n, n - 1, n, RelationshipType::FinishToStart, 0);
        }
    }
    s
}

fn assignment(n: u64) -> Assignment {
    Assignment {
        id: AssignmentId(n),
        activity: ActivityId(n),
        resource: Some(ResourceId(1)),
        role: None,
        cost_account: None,
        project: ProjectId(1),
        curve: None,
        typed_spread: None,
        amounts: totalfloat_model::Amounts::default(),
        udfs: BTreeMap::new(),
        source: None,
    }
}

fn link(s: &mut Schedule, id: u64, from: u64, to: u64, kind: RelationshipType, lag: i64) {
    s.relationships.insert(
        RelationshipId(id),
        Relationship {
            id: RelationshipId(id),
            predecessor: ActivityId(from),
            successor: ActivityId(to),
            predecessor_project: ProjectId(1),
            successor_project: ProjectId(1),
            kind,
            lag: Minutes(lag),
            source: None,
        },
    );
}

fn check(s: &Schedule, id: &str) -> super::Check {
    assess(s, &[], &Thresholds::default())
        .checks
        .into_iter()
        .find(|check| check.id == id)
        .unwrap()
}

fn activity(s: &mut Schedule, n: u64) -> &mut Activity {
    s.activities.get_mut(&ActivityId(n)).unwrap()
}

#[test]
fn a_clean_chain_passes_everything_it_can_measure() {
    let assessment = assess(&chain(60), &[], &Thresholds::default());
    assert_eq!(assessment.failed(), 0, "{:#?}", assessment.checks);
    assert_eq!(assessment.checks.len(), 21);
}

#[test]
fn logic_counts_unfinished_activities_missing_a_link() {
    let mut s = chain(10);
    s.relationships.remove(&RelationshipId(5));
    let result = check(&s, "dcma-01-logic");
    assert_eq!(result.outcome, Outcome::Fail);
    assert_eq!(result.offenders, 4);
    assert_eq!(result.population, 10);
}

#[test]
fn leads_allow_none() {
    let mut s = chain(30);
    s.relationships.get_mut(&RelationshipId(2)).unwrap().lag = Minutes(-60);
    let result = check(&s, "dcma-02-leads");
    assert_eq!(result.outcome, Outcome::Fail);
    assert_eq!(result.listed, ["A1 to A2"]);
}

#[test]
fn lags_allow_a_share() {
    let mut s = chain(30);
    s.relationships.get_mut(&RelationshipId(2)).unwrap().lag = Minutes(60);
    assert_eq!(check(&s, "dcma-03-lags").outcome, Outcome::Pass);
    for id in 2..=6 {
        s.relationships.get_mut(&RelationshipId(id)).unwrap().lag = Minutes(60);
    }
    assert_eq!(check(&s, "dcma-03-lags").outcome, Outcome::Fail);
}

#[test]
fn relationship_types_want_finish_to_start() {
    let mut s = chain(30);
    for id in 2..=5 {
        s.relationships.get_mut(&RelationshipId(id)).unwrap().kind = RelationshipType::StartToStart;
    }
    let result = check(&s, "dcma-04-relationship-types");
    assert_eq!(result.outcome, Outcome::Fail);
    assert_eq!(result.offenders, 4);
}

#[test]
fn hard_constraints_leave_out_soft_ones() {
    let mut s = chain(10);
    activity(&mut s, 2).primary_constraint = Some(Constraint {
        kind: ConstraintType::StartOnOrAfter,
        date: Some(at("2026-01-05 08:00")),
    });
    assert_eq!(check(&s, "dcma-05-hard-constraints").outcome, Outcome::Pass);
    activity(&mut s, 3).primary_constraint = Some(Constraint {
        kind: ConstraintType::MandatoryFinish,
        date: Some(at("2026-01-09 16:00")),
    });
    let result = check(&s, "dcma-05-hard-constraints");
    assert_eq!(result.outcome, Outcome::Fail);
    assert_eq!(result.listed, ["A3"]);
}

#[test]
fn high_float_is_more_than_44_days() {
    let mut s = chain(10);
    activity(&mut s, 2).stored.total_float = Some(Minutes(44 * DAY));
    assert_eq!(check(&s, "dcma-06-high-float").offenders, 0);
    activity(&mut s, 2).stored.total_float = Some(Minutes(45 * DAY));
    let result = check(&s, "dcma-06-high-float");
    assert_eq!(result.outcome, Outcome::Fail);
    assert_eq!(result.listed, ["A2"]);
}

#[test]
fn negative_float_allows_none() {
    let mut s = chain(30);
    activity(&mut s, 2).stored.total_float = Some(Minutes(-1));
    assert_eq!(check(&s, "dcma-07-negative-float").outcome, Outcome::Fail);
}

#[test]
fn high_duration_is_more_than_44_days() {
    let mut s = chain(10);
    activity(&mut s, 4).original_duration = Minutes(45 * DAY);
    assert_eq!(check(&s, "dcma-08-high-duration").outcome, Outcome::Fail);
}

#[test]
fn invalid_dates_catch_actuals_ahead_and_forecasts_behind() {
    let mut s = chain(10);
    activity(&mut s, 2).actual_start = Some(at("2026-02-01 08:00"));
    activity(&mut s, 3).stored.early_start = Some(at("2026-01-01 08:00"));
    let result = check(&s, "dcma-09-invalid-dates");
    assert_eq!(result.outcome, Outcome::Fail);
    assert_eq!(result.listed, ["A2", "A3"]);

    s.project.data_date = None;
    s.project.planned_start = Some(at("2026-01-05 08:00"));
    assert_eq!(
        check(&s, "dcma-09-invalid-dates").outcome,
        Outcome::NotApplicable
    );
}

#[test]
fn invalid_dates_catch_work_under_way_forecast_to_finish_in_the_past() {
    let mut s = chain(3);
    s.project.data_date = Some(at("2026-01-12 08:00"));
    let a = activity(&mut s, 2);
    a.status = ActivityStatus::InProgress;
    a.actual_start = Some(at("2026-01-05 08:00"));
    a.stored.early_finish = Some(at("2026-01-09 16:00"));
    let check = check(&s, "dcma-09-invalid-dates");
    assert_eq!(check.listed, ["A2"]);
}

#[test]
fn resources_leave_out_milestones() {
    let mut s = chain(10);
    s.assignments.remove(&AssignmentId(2));
    assert_eq!(check(&s, "dcma-10-resources").outcome, Outcome::Fail);
    let a = activity(&mut s, 2);
    a.activity_type = ActivityType::FinishMilestone;
    a.original_duration = Minutes::ZERO;
    a.remaining_duration = Minutes::ZERO;
    assert_eq!(check(&s, "dcma-10-resources").outcome, Outcome::Pass);
}

#[test]
fn baseline_checks_are_not_applicable() {
    let s = chain(5);
    assert_eq!(
        check(&s, "dcma-11-missed-tasks").outcome,
        Outcome::NotApplicable
    );
    assert_eq!(check(&s, "dcma-14-bei").outcome, Outcome::NotApplicable);
}

#[test]
fn the_critical_path_test_moves_the_finish() {
    let s = chain(5);
    let result = check(&s, "dcma-12-critical-path-test");
    assert_eq!(result.outcome, Outcome::Pass);
    assert!(
        result.measure.starts_with("A1 delayed 600 days"),
        "{result:?}"
    );
}

#[test]
fn the_critical_path_test_moves_the_project_s_own_finish() {
    // Float from the opened projects' finish, and another project finishing
    // years later: the scheduled finish is that project's, but a delay still
    // moves this project's own finish.
    let mut s = chain(5);
    s.project.options.float_from_project_finish = false;
    let mut other = Schedule::new(Project::new(ProjectId(2), "OTHER"));
    let mut late = Activity::new(
        ActivityId(99),
        ProjectId(2),
        WbsId(1),
        CalendarId(1),
        "Z1",
        "Authored",
    );
    late.stored.early_finish = Some(at("2029-01-01 16:00"));
    other.activities.insert(late.id, late);
    let result = assess(&s, &[s.clone(), other], &Thresholds::default())
        .checks
        .into_iter()
        .find(|check| check.id == "dcma-12-critical-path-test")
        .unwrap();
    assert_eq!(result.outcome, Outcome::Pass, "{result:?}");
}

#[test]
fn the_critical_path_test_needs_a_calculable_schedule() {
    let mut s = chain(5);
    activity(&mut s, 1).status = ActivityStatus::InProgress;
    assert_eq!(
        check(&s, "dcma-12-critical-path-test").outcome,
        Outcome::NotApplicable
    );
}

#[test]
fn the_critical_path_length_index_needs_a_required_finish() {
    let mut s = chain(10);
    assert_eq!(check(&s, "dcma-13-cpli").outcome, Outcome::NotApplicable);
    s.project.must_finish_by = Some(at("2026-01-06 16:00"));
    let result = check(&s, "dcma-13-cpli");
    assert_eq!(result.outcome, Outcome::Fail, "{result:?}");
    s.project.must_finish_by = Some(at("2026-03-01 16:00"));
    assert_eq!(check(&s, "dcma-13-cpli").outcome, Outcome::Pass);
}

#[test]
fn open_ends_excuse_one_finish() {
    let mut s = chain(10);
    assert_eq!(check(&s, "open-ends").outcome, Outcome::Pass);
    s.relationships.remove(&RelationshipId(5));
    assert_eq!(check(&s, "open-ends").listed, ["A4", "A5"]);
}

#[test]
fn an_open_start_is_listed_once_work_has_finished() {
    let mut s = chain(10);
    s.relationships.remove(&RelationshipId(5));
    assert_eq!(check(&s, "open-ends").listed, ["A4", "A5"]);
    activity(&mut s, 1).status = ActivityStatus::Completed;
    assert_eq!(check(&s, "open-ends").listed, ["A4", "A5"]);
    s.relationships.remove(&RelationshipId(2));
    activity(&mut s, 2).status = ActivityStatus::InProgress;
    assert_eq!(check(&s, "open-ends").listed, ["A2", "A4", "A5"]);
}

#[test]
fn open_ends_excuse_the_activity_that_finishes_last() {
    let mut s = chain(10);
    s.relationships.remove(&RelationshipId(5));
    activity(&mut s, 4).stored.early_finish = Some(at("2026-01-08 16:00"));
    activity(&mut s, 10).stored.early_finish = Some(at("2026-01-16 16:00"));
    assert_eq!(check(&s, "open-ends").listed, ["A4", "A5"]);
    activity(&mut s, 4).stored.early_finish = Some(at("2026-02-16 16:00"));
    assert_eq!(check(&s, "open-ends").listed, ["A5", "A10"]);
}

#[test]
fn an_open_start_with_no_dates_is_not_taken_for_the_project_start() {
    let mut s = chain(10);
    s.relationships.remove(&RelationshipId(5));
    activity(&mut s, 5).stored.early_start = Some(at("2026-01-05 08:00"));
    assert_eq!(check(&s, "open-ends").listed, ["A1", "A4"]);
}

#[test]
fn out_of_sequence_progress_is_listed() {
    let mut s = chain(10);
    let a = activity(&mut s, 3);
    a.status = ActivityStatus::InProgress;
    a.actual_start = Some(at("2026-01-05 08:00"));
    let result = check(&s, "out-of-sequence");
    assert_eq!(result.outcome, Outcome::Fail);
    assert_eq!(result.listed, ["A2 to A3"]);
}

#[test]
fn out_of_sequence_progress_across_projects_is_listed_unless_ignored() {
    // Project 1's unfinished A2 leads to project 2's B1, which has started.
    let one = chain(2);
    let mut two = chain(0);
    two.project.id = ProjectId(2);
    let mut b1 = Activity::new(
        ActivityId(11),
        ProjectId(2),
        WbsId(1),
        CalendarId(1),
        "B1",
        "Authored",
    );
    b1.status = ActivityStatus::InProgress;
    b1.actual_start = Some(at("2026-01-05 08:00"));
    b1.remaining_duration = Minutes(DAY);
    two.activities.insert(b1.id, b1);
    two.relationships.insert(
        RelationshipId(20),
        Relationship {
            id: RelationshipId(20),
            predecessor: ActivityId(2),
            successor: ActivityId(11),
            predecessor_project: ProjectId(1),
            successor_project: ProjectId(2),
            kind: RelationshipType::FinishToStart,
            lag: Minutes(0),
            source: None,
        },
    );
    let listed = |two: &Schedule| {
        assess(two, &[one.clone(), two.clone()], &Thresholds::default())
            .checks
            .into_iter()
            .find(|check| check.id == "out-of-sequence")
            .unwrap()
            .listed
    };
    assert_eq!(listed(&two), ["A2 to B1"]);
    two.project.options.ignore_external_relationships = true;
    assert_eq!(listed(&two), Vec::<String>::new());
}

#[test]
fn a_mandatory_date_with_a_predecessor_can_override_logic() {
    let mut s = chain(10);
    activity(&mut s, 1).primary_constraint = Some(Constraint {
        kind: ConstraintType::MandatoryStart,
        date: Some(at("2026-01-05 08:00")),
    });
    assert_eq!(
        check(&s, "constraints-overriding-logic").outcome,
        Outcome::Pass
    );
    activity(&mut s, 5).primary_constraint = Some(Constraint {
        kind: ConstraintType::MandatoryStart,
        date: Some(at("2026-01-05 08:00")),
    });
    assert_eq!(
        check(&s, "constraints-overriding-logic").outcome,
        Outcome::Fail
    );
}

#[test]
fn long_lags_are_more_than_44_days() {
    let mut s = chain(10);
    s.relationships.get_mut(&RelationshipId(2)).unwrap().lag = Minutes(45 * DAY);
    assert_eq!(check(&s, "long-lags").outcome, Outcome::Fail);
}

#[test]
fn long_lags_count_days_on_the_activity_calendar() {
    let mut s = chain(10);
    s.calendars
        .get_mut(&CalendarId(1))
        .unwrap()
        .hours_per_period
        .day = Some(Minutes(600));
    s.relationships.get_mut(&RelationshipId(2)).unwrap().lag = Minutes(45 * DAY);
    assert_eq!(check(&s, "long-lags").offenders, 0);
}

#[test]
fn start_to_finish_relationships_are_listed() {
    let mut s = chain(10);
    s.relationships.get_mut(&RelationshipId(2)).unwrap().kind = RelationshipType::StartToFinish;
    assert_eq!(check(&s, "start-to-finish").listed, ["A1 to A2"]);
}

#[test]
fn thresholds_change_outcomes_and_listing_is_bounded() {
    let mut s = chain(200);
    for id in 2..=200 {
        s.relationships.get_mut(&RelationshipId(id)).unwrap().lag = Minutes(60);
    }
    let strict = assess(&s, &[], &Thresholds::default());
    let lags = |a: &super::Assessment| {
        a.checks
            .iter()
            .find(|c| c.id == "dcma-03-lags")
            .unwrap()
            .clone()
    };
    assert_eq!(lags(&strict).outcome, Outcome::Fail);
    assert_eq!(lags(&strict).offenders, 199);
    assert_eq!(lags(&strict).listed.len(), super::MAX_LISTED);
    let relaxed = assess(
        &s,
        &[],
        &Thresholds {
            lags_percent: 100.0,
            ..Thresholds::default()
        },
    );
    assert_eq!(lags(&relaxed).outcome, Outcome::Pass);
}

#[test]
fn float_lost_to_a_calendar_gap_shows_both_numbers() {
    // A1 finishes Friday. Eight hours of finish-to-finish lag on a seven-day
    // calendar reach Saturday, which A2's five-day calendar moves to Monday.
    let mut s = chain(3);
    s.project.data_date = Some(at("2026-01-09 08:00"));
    let mut seven = s.calendars[&CalendarId(1)].clone();
    seven.id = CalendarId(2);
    let shift = seven.work_week.0[1].clone();
    seven.work_week = WorkWeek(std::array::from_fn(|_| shift.clone()));
    s.calendars.insert(CalendarId(2), seven);
    for n in [1, 3] {
        activity(&mut s, n).calendar = CalendarId(2);
    }
    s.relationships.clear();
    link(&mut s, 1, 1, 2, RelationshipType::FinishToFinish, 480);
    link(&mut s, 2, 2, 3, RelationshipType::FinishToStart, 0);
    let check = check(&s, "calendar-gap-float");
    assert_eq!(check.offenders, 1);
    assert_eq!(check.listed, ["A1: -8.0h total float, 8.0h true"]);
}

/// 7 of 100 is 7.000000000000001 as a rate, which isn't over a 7% limit.
#[test]
fn a_count_exactly_at_its_limit_passes() {
    let offenders: Vec<String> = (0..7).map(|n| format!("A{n}")).collect();
    let check = super::share(("id", "name"), offenders.clone(), 100, 7.0);
    assert_eq!(check.outcome, Outcome::Pass, "{check:?}");
    let mut over = offenders;
    over.push("A7".to_owned());
    let check = super::share(("id", "name"), over, 100, 7.0);
    assert_eq!(check.outcome, Outcome::Fail);
}

#[test]
fn redundant_logic_finds_a_link_a_longer_chain_implies() {
    let mut s = chain(5);
    link(&mut s, 100, 1, 4, RelationshipType::FinishToStart, 0);
    let result = check(&s, "redundant-logic");
    assert_eq!(result.outcome, Outcome::Fail);
    assert_eq!(result.listed, ["A1 to A4"]);
    assert_eq!(result.population, 5);
}

#[test]
fn redundant_logic_passes_a_chain_with_no_shortcut() {
    let result = check(&chain(30), "redundant-logic");
    assert_eq!(result.outcome, Outcome::Pass);
    assert_eq!(result.offenders, 0);
}

#[test]
fn redundant_logic_needs_the_chain_to_hold_the_successor_as_late() {
    // A lag on the shortcut isn't implied by a chain without it.
    let mut s = chain(5);
    link(&mut s, 100, 1, 4, RelationshipType::FinishToStart, 60);
    assert_eq!(check(&s, "redundant-logic").offenders, 0);
    // Nor is a shortcut past a lead, or past a start-to-start link.
    let mut s = chain(5);
    s.relationships.get_mut(&RelationshipId(3)).unwrap().lag = Minutes(-60);
    link(&mut s, 100, 1, 4, RelationshipType::FinishToStart, 0);
    assert_eq!(check(&s, "redundant-logic").offenders, 0);
    let mut s = chain(5);
    s.relationships.get_mut(&RelationshipId(3)).unwrap().kind = RelationshipType::StartToStart;
    link(&mut s, 100, 1, 4, RelationshipType::FinishToStart, 0);
    assert_eq!(check(&s, "redundant-logic").offenders, 0);
}

#[test]
fn redundant_logic_finds_every_shortcut_around_branches() {
    // 1 -> 2 -> 3 -> 4 and 1 -> 5 -> 4, with shortcuts 1 -> 3 and 2 -> 4.
    let mut s = chain(5);
    s.relationships.remove(&RelationshipId(5));
    link(&mut s, 100, 1, 5, RelationshipType::FinishToStart, 0);
    link(&mut s, 101, 5, 4, RelationshipType::FinishToStart, 0);
    link(&mut s, 102, 1, 3, RelationshipType::FinishToStart, 0);
    link(&mut s, 103, 2, 4, RelationshipType::FinishToStart, 0);
    let thresholds = Thresholds {
        redundant_logic_percent: 50.0,
        ..Thresholds::default()
    };
    let result = assess(&s, &[], &thresholds)
        .checks
        .into_iter()
        .find(|check| check.id == "redundant-logic")
        .unwrap();
    assert_eq!(result.listed, ["A1 to A3", "A2 to A4"]);
    assert_eq!(result.outcome, Outcome::Pass, "2 of 7 is under 50%");
}

#[test]
fn redundant_logic_still_answers_when_the_links_loop() {
    let mut s = chain(5);
    link(&mut s, 100, 5, 2, RelationshipType::FinishToStart, 0);
    link(&mut s, 101, 1, 3, RelationshipType::FinishToStart, 0);
    // 1 -> 3 is implied by 1 -> 2 -> 3, and around the loop 1 -> 3 -> 4 ->
    // 5 -> 2 implies 1 -> 2 too.
    assert_eq!(
        check(&s, "redundant-logic").listed,
        ["A1 to A2", "A1 to A3"]
    );
}
