# What TotalFloat supports

What TotalFloat does today, by area. Where a feature has a gap, it says so. [Limitations](limitations.md) covers what it doesn't do and what it refuses.

## Files

- **XER:** reads every table and field in a file, keeping the ones it doesn't use as they are. An unedited file saves back byte for byte, and an edited one changes only what you changed. Several projects in one file, and links between them, are kept.
- **New schedules:** `totalfloat new`, [schedule text](text.md) or a spreadsheet.
- **Microsoft Project XML:** [import and export](import.md), with tasks, links, resources and assignments. Costs are written in US dollars.
- **Calendars:** `export --to ics` writes activities and milestones as an iCalendar file for Outlook, Google Calendar and phones.
- **Spreadsheets:** build a schedule from an `.xlsx` workbook or CSV files, update progress from a status table, export any `query` as CSV or Excel, and read edits made in an exported sheet back in.
- **Not yet:** P6 XML (PMXML), and Microsoft Project `.mpp` files.

## Calendars

- Work weeks with several shifts a day, holidays and exceptions, and exceptions inherited from a base calendar.
- Global, project and resource calendars, and all four lag calendar settings.
- Hours per day, week, month and year, which set what a day or week of duration means.
- Edits: add or remove exceptions, change the work week, add a calendar.

## Scheduling

- Forward and backward passes, all four relationship types, and positive and negative lag.
- The data date, the project's must-finish-by date and open ends, with the option to make open ends critical.
- Loops are refused, with every activity in the loop listed.
- Linked projects scheduled together, or each on its own logic (`schedule --ignore-external`).
- A scheduling log of settings, problems found and results (`schedule --log`).
- Every rule is written down, with its sources, in [docs/rules](../rules/README.md).

## Activity types

- Task dependent; start and finish milestones.
- Level of effort and WBS summary, calculated as spans. How links between two spans behave is a first version.
- Resource dependent, on one resource calendar.

## Constraints

- Start and finish on, on or before, and on or after, as primary and secondary constraints.
- Mandatory start and finish, as late as possible, and expected finish.

## Progress

- Actual dates, remaining duration, suspend and resume.
- Out-of-sequence progress under Retained Logic or Progress Override, and a list of the links progress broke.
- Update progress as planned (`edit --as-planned`).
- Duration types and percent complete types: duration, physical and units.
- [Timesheets](timesheets.md) per resource, filled in and applied as progress.

## Float and the critical path

- Total float (start, finish or smallest) and free float.
- Critical by a total-float threshold or by the longest path, with driving relationships.
- Multiple float paths, numbered by total or free float.
- Tracing logic: predecessors, successors, the driving path to an activity, and [why a date is what it is](explain.md).

## Resources and costs

- Labor, nonlabor and material resources, roles, and assignments by resource or role.
- Units, prices and costs on assignments, moved by duration edits; rates that change over time.
- Resource curves and typed spreads.
- Expenses and cost accounts.
- Resource usage per week or month against each resource's limit.

## Baselines and earned value

- Compare with a baseline file or a [history](history.md) baseline, and variance reports.
- Earned value: PV, EV, AC, SPI, CPI and EAC.
- Baselines stored inside an XER file aren't read yet.

## Project data

- WBS, activity codes, project codes, user-defined fields, OBS and activity notes, read and edited.
- Activity steps, which drive physical percent complete where the project measures by steps.
- An [EPS](portfolio.md) kept beside the schedules, with roll-ups per node.
- Global change: `edit --where FILTER --set FIELD=VALUE`.
- [Schedule checks](check.md) against your own thresholds and the thresholds a file stores on activity float and durations.

## Views and reports

- The [local app](serve.md): activity table with editing, filters, grouping and sorting; Gantt chart with logic lines and the driving path; activity network; saved layouts; print to PDF.
- Building a schedule in the app: activities, WBS nodes and links. Deleting a WBS node, and editing an activity's codes, notes and resources in the inspector, aren't there yet.
- [`query`](query.md) from the command line, the [DCMA 14-point check](check.md) with six more checks, and [reports](report.md) from templates as Markdown, CSV or HTML.

## History and teams

- Every change recorded, with undo, revert, branches for what-ifs, and merge ([history](history.md)).
- Portfolio and capacity across projects by resource and role.
- [Monte Carlo schedule risk](risk.md) on the finish date, with correlation, criticality, sensitivity and risk events.
- **Not yet:** a multi-user server, sign-in, and resource leveling.
