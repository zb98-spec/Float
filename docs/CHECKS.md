# Schedule checks

`totalfloat check <file>` runs the 14 checks of the DCMA assessment and seven more. It reports a result, a measure and the offending activities or relationships for each one, up to 50 listed. It exits `3` if any check fails.

```sh
totalfloat check plant.xer                  # readable summary
totalfloat check plant.xer --markdown       # a report to keep
totalfloat check plant.xer --html -o c.html # one HTML page to send on
totalfloat check plant.xer --json           # totalfloat.check/1
totalfloat check plant.xer --config my.toml
```

A check that the file can't answer says `n/a`. It never passes by default.

Float and dates are the values stored in the file, and the engine's where the file has none. Durations convert to days with the calendar's hours per day, or `hours_per_day` if it has none. "Unfinished" leaves out completed work, level of effort and WBS summaries.

## DCMA 14-point

| # | Check | Counts | Out of | Default limit | Config key |
|---|---|---|---:|---:|---|
| 1 | Logic | Missing a predecessor or successor | Unfinished activities | 5% | `logic_percent` |
| 2 | Leads | Negative lag | Relationships | 0% | `leads_percent` |
| 3 | Lags | Positive lag | Relationships | 5% | `lags_percent` |
| 4 | Relationship types | Not finish-to-start | Relationships | 90% finish-to-start | `finish_to_start_percent` |
| 5 | Hard constraints | Start on, finish on, either on-or-before, either mandatory | Unfinished activities | 5% | `hard_constraints_percent` |
| 6 | High float | Total float over 44 days | Unfinished with float | 5% | `high_float_days`, `high_float_percent` |
| 7 | Negative float | Total float below zero | Unfinished with float | 0% | `negative_float_percent` |
| 8 | High duration | Original duration over 44 days | Unfinished activities | 5% | `high_duration_days`, `high_duration_percent` |
| 9 | Invalid dates | Actuals after the data date, or forecasts before it | All activities | 0% | none |
| 10 | Resources | Work with no resource | Unfinished with work | 0% | `resources_percent` |
| 11 | Missed tasks | Needs a baseline | | n/a | none |
| 12 | Critical path test | Delays a longest-path activity 600 days and checks the finish moves | | Finish moves | none |
| 13 | Critical path length index | (Length + float to the required finish) / length, in elapsed time | | 0.95 | `critical_path_length_index` |
| 14 | Baseline execution index | Needs a baseline | | n/a | none |

An XER export doesn't hold a baseline, so checks 11 and 14 are `n/a`. Check 13 needs a required finish, `PROJECT.plan_end_date`. Check 8 uses original duration because the file has no baseline duration, and it doesn't apply the source's planning-window filter.

## Further checks

| Check | Counts | Default limit | Config key |
|---|---|---:|---|
| Open ends | Unfinished activities with no predecessor or no successor, excusing one project start and one project finish | None allowed | none |
| Out-of-sequence progress | Relationships broken by recorded progress | None allowed | none |
| Constraints overriding logic | Mandatory dates on activities with a predecessor | None allowed | none |
| Long lags | Lag or lead over 44 days | None allowed | `long_lag_days` |
| Start-to-finish relationships | Any | None allowed | none |
| Float lost to a calendar gap | Unfinished activities whose float differs from the true value, listing both numbers ([TF-032](rules/TF-032-float-across-calendar-gaps.md)) | 5% | `calendar_gap_percent` |
| Redundant logic | Finish-to-start links without lag that a longer chain of finish-to-start links without leads already implies, so they can never drive alone. Chains don't run through level of effort or WBS summaries, and a very large network that would take too long to search gives `n/a` | None allowed | `redundant_logic_percent` |

## Thresholds

Pass a TOML file with `--config`. Anything left out keeps its default, and an unknown key is an error:

```toml
logic_percent = 3.0
high_float_days = 30.0
hours_per_day = 10.0
```

## Sources

The 14 checks and their limits come from DCMA's own public description of the assessment, the *EVMS Program Analysis Pamphlet* (DCMA-EA PAM 200.1, §4), a U.S. government work ([rules/sources.md](rules/sources.md#dcma-14pt)), and the [GAO schedule guide's DCMA page](https://gaoinnovations.gov/schedule-guide/dod-defense-contract-management-agency.html). The further checks and their limits are TotalFloat's own additions. Which constraints count as hard varies between sources. All of these are defaults to adjust, and the assessment is a review aid, not a pass or fail on a schedule's quality.
