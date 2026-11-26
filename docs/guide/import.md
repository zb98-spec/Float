# Move schedules to and from Microsoft Project

`import` reads an MS Project XML file and writes a new XER. In Project, use **Save As** and choose XML. `.mpp` files aren't read.

```sh
totalfloat import plan.xml -o plan.xer
```

```text
Project Plant & Yard Upgrade: 5 activities, 4 relationships, 3 WBS nodes, 2 calendars, 1 resources
finish: 2026-03-18 17:00
didn't translate exactly:
  1 x an effort-driven task keeps its duration, so its work won't drive it: Pour slab
  1 x Start No Earlier Than constraints kept; check each one is meant: Pour slab
  1 x a manually scheduled task is now calculated by its logic: Install fence
  1 x an inactive task was left out: Cancelled survey
  1 x a link on a summary task was left out, because summary tasks become WBS nodes, which hold no logic: Yard
  1 x an elapsed lag is now the same number of working days: Install fence
  1 x a lag given as a percent of its predecessor was left as no lag: Lighting
  1 x an assignment came across without its units: Excavate
Dates follow TotalFloat's own rules, so they can differ from MS Project's
Wrote plan.xer
```

Tasks, links, lag, constraints, calendars, resources, assignments and progress come across. Summary tasks become WBS nodes.

The list is part of the result, so read it. TotalFloat calculates by its own [rules](../rules/README.md), so the dates can differ from Project's. The main things it reports:

- **Logic and assignments on summary tasks.** Summary tasks become WBS nodes, which hold no logic or assignments, so those links and assignments are left out. Link and assign the work under the summary instead.
- **Start No Earlier Than constraints.** Each one is kept and listed, so you can check it is meant.
- **Manually scheduled tasks.** They're calculated by their logic.
- **Effort-driven tasks.** They keep their duration, and work doesn't drive it.
- **Assignments.** Their work, units and costs come across. A material resource's work is in its own units, so it's left out, as is any assignment with no work.
- **Resource limits.** A work resource's Max Units is its limit: 1 is one person, held as 1 unit per hour, and `query --usage` measures against it. An availability table becomes a limit that changes over time, with none outside its periods. Material and cost resources' limits aren't read.
- **Elapsed and percent lags.** An elapsed lag becomes the same number of working days, and a percent lag becomes no lag.
- **Elapsed durations.** They run on an Elapsed 24-Hour calendar that the import adds.
- **Repeating calendar exceptions.** Only the first date is kept, so add the rest with `--holiday`.
- **Alternate work weeks, splits and leveling delays.** These are left out.
- **Inactive tasks and deadlines.** These are left out too, and any link to an inactive task goes with it.

With no status date in the file, the data date is the last recorded work, or the project start if there's none.

If the engine can't calculate the result, the file is still written with no dates, and the report says why.

## Send a schedule to Microsoft Project

`export` writes a project as MS Project XML. To use it in Project, open it with **File > Open**.

TotalFloat checks its exports against Microsoft's published schema and reads them back through MPXJ, an independent reader ([validators](../validation/validators.md)). They haven't been tested in Project itself.

```sh
totalfloat export plant.xer --to msproject -o plant.xml
```

WBS nodes become summary tasks. Dates come across as calculated, and Project will recalculate them by its own rules. Name a project with `--project` if the file has several.

The list of what didn't translate works the same way. The main things are:

- **Scheduling options and a must-finish date.** MS Project XML doesn't hold them this way, so float calculated in Project can differ.
- **Activity IDs and WBS codes.** They're left out, and tasks are numbered by their place in the outline.
- **Level of effort and WBS summary activities.** They become plain tasks.
- **Start milestones.** MS Project XML has one kind of milestone, so one can move to the finish before it.
- **Secondary constraints and expected finishes.** A task in MS Project XML holds one constraint and no expected finish.
- **Suspend and resume dates.** They're left out, so the work reads as one unbroken span.
- **Start On and Finish On.** They become Must Start On and Must Finish On.
- **Links to other projects.** They're left out.
- **Assignments.** Their work, units and costs come across, apart from a material resource's units. Nonlabor resources become work resources. Costs are written as US dollars, since TotalFloat doesn't record a currency yet.
- **Resource limits.** A limit that holds throughout becomes Max Units, and one that changes over time becomes the resource's availability table. A material resource's limit, or one that is unset for part of the time, is left out. A resource with no limit gets the schema's default of 1.
- **WBS nodes with no activities.** They're left out, because a summary task's dates come from the tasks under it.

A project that lost tasks or links when its file was read is refused, and so is one with no calendars.

## Put the schedule in a calendar

`export --to ics` writes a project's activities as an iCalendar file, which Outlook, Google Calendar, Apple Calendar and most phones open:

```sh
totalfloat schedule plant.xer -o plant-scheduled.xer
totalfloat export plant-scheduled.xer --to ics -o plant.ics
```

Each activity is an event named by its ID and name, with its status and total float in the description. Work not started shows its early dates, work under way its actual start and early finish, and completed work its actual dates. A milestone is an event at one moment. Critical activities and milestones are tagged `CRITICAL` and `MILESTONE`, so a calendar app can filter them.

Times are written as local clock times, with no time zone, as the schedule holds them. Each event keeps the same identifier from one export to the next, so importing an update moves events rather than adding copies. Activities the file holds no dates for are left out, and the notes count them, so schedule the file first.

## Build a schedule from a spreadsheet

`import` also builds a new schedule from a spreadsheet: an `.xlsx` workbook, or a folder of CSV files. A workbook has a sheet, and a folder a `.csv` file, for each part. Name each sheet or file as below; a workbook's other sheets are left alone.

```sh
totalfloat import plan.xlsx -o plan-from-workbook.xer
totalfloat import plan/ -o plan-from-csv.xer
```

| Sheet (file) | Columns |
|---|---|
| Project (`project.csv`), needed | `code`, `name`, `data_date`; and if wanted `must_finish` and `calendar`. One row. |
| Activities (`activities.csv`), needed | `code`, `name`; and if wanted `wbs_path`, `type`, `duration` (such as `5d`) or `original_duration_hours`, `calendar`, `constraint` with `constraint_date`, and `resources` (resource IDs, separated by commas) |
| WBS (`wbs.csv`) | `wbs_path`, `name` |
| Resources (`resources.csv`) | `resource`, `name`; and if wanted `type` (`labor`, `nonlabor` or `material`), `units_per_hour`, `calendar` and `max_units_per_hour`, its limit (2 is two people at once) |
| Relationships (`relationships.csv`) | `predecessor`, `successor`; and if wanted `link_type` (`FS`, `SS`, `FF` or `SF`, FS if left blank), and `lag` (such as `1d`) or `link_lag_hours` |

Columns have `query`'s names where it has the column, so a `query --csv` export can be the start of one. The first row of each sheet names its columns, in any order and any case, each once. Sheet and file names, and values such as `Task` or `Start-On-Or-After`, can be in any case. A `wbs_path` starts at the project code, as `PLANT.CIV.PIERS`. A node the WBS sheet doesn't name is made with its code as its name. Dates are `YYYY-MM-DD HH:MM`; in a workbook, a cell formatted as a date works too, in either of Excel's date systems, and a duration cell such as `8:00` is that many hours. WBS nodes are made in the order the WBS sheet lists them.

Each row becomes the edits `new --edits` would apply, so the result is the same as the equivalent command document. Nothing is built unless every row is good. Each problem is reported at its sheet and row: a column the sheet doesn't have, a date that isn't one, or a link to an activity the Activities sheet doesn't list.

```text
error[sheet-refused]: plan.xlsx has 2 problems, so nothing was built
  plan.xlsx Activities row 1: Activities has no column colour
  plan.xlsx Relationships row 7: successor Z9 isn't on the Activities sheet
```

Formulas aren't worked out: a cell gives the value the spreadsheet last saved. Updating an existing schedule from a spreadsheet works for progress only, with `edit --progress-table`.
