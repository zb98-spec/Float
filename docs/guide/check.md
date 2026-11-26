# Check schedule quality

`check` runs the DCMA 14-point assessment and seven more checks. It exits `3` if any check fails.

```sh
totalfloat check plant.xer
```

Each line gives a result, a measure and how many items are outside the rule, and a failed check lists up to 50 of them. `n/a` means the file can't answer that check. Missed tasks and the baseline execution index always say so, because an XER export doesn't hold a baseline.

For a report to keep, use `--markdown`:

```sh
totalfloat check plant.xer --markdown
```

To send it on, write one HTML page with `--html`. It opens in any browser and can go straight into an email or a shared folder. Each project has a table of every check, with failures marked in red, then a table for each failed check of the activities or relationships it lists, up to 50:

```sh
totalfloat check plant.xer --html -o checks.html
```

`-o FILE` writes a new file and refuses one that already exists; it works with the text report and `--markdown` too. The page has its style inline and nothing else: no scripts, no links to other files and no time of writing, so the same file always gives the same page.

Change the limits with a TOML file. Anything you leave out keeps its default:

```toml
logic_percent = 3.0
high_float_days = 30.0
```

```sh
totalfloat check plant.xer --config my-thresholds.toml
```

## Your own thresholds

Add a `[[threshold]]` to the config for each rule of your own: the activities in its scope that break it are raised, and it's reported like a check, failing if any are.

```toml
[[threshold]]
name = "Float under a week"
column = "total_float_hours"
comparison = "<"
value = 40
scope = { status = ["not-started", "in-progress"] }
```

- `column`: one of [`query`](query.md)'s columns, such as `total_float_hours`, `early_finish` or `code:Phase`.
- `comparison`: `<`, `<=`, `>`, `>=`, `=` or `!=`.
- `value`: a number, text such as a date `"2026-06-30 17:00"`, or `true`/`false` for a column such as `critical`, compared with the column's value: numbers by size, text and dates as written. A value of another kind than the column holds is refused, so a rule is never silently passed. An empty value never breaks a threshold.
- `scope` (optional): `query`'s filters, by their option names without the dashes, for the activities it covers. Every activity by default.
- `name` (optional): what the report calls it; by default, the rule.

In `--json`, thresholds come under `thresholds`, one per project, as `threshold-1`, `threshold-2` and so on, in the order of the config, each listing the first 50 activities it raises in Activity ID order.

## Thresholds stored in the file

A file can hold the thresholds P6 monitors for its projects (`PROJTHRS`, each on a parameter in `THRSPARM`). `check` measures them too, after your own, as `p6-threshold-ID`, named `P6 threshold:` and the parameter's name. An activity in the threshold's WBS node, or under it, is raised when its value is below the threshold's low limit or above its high limit. Limits are hours, as P6 stores them.

```text
  FAIL P6 threshold: Total float, hours: 2 of 2 activities have total_float_hours outside 8 to 40 (2 of 2)
       TF-001, TF-002
```

Thresholds on total float, free float, remaining duration and original duration are measured. Others are listed as not applicable, saying why: one the file doesn't enable (its status isn't `TS_Enable`); one that watches WBS nodes as a whole; one that watches a window of dates; one on a start or finish variance, which needs a baseline; and one on a field not measured yet. A failed one counts towards the exit code as any check does; `stored_thresholds = false` in the config leaves the file's thresholds out.

[CHECKS.md](../CHECKS.md) lists every check, its limit and where the limit comes from. A failed check is a prompt to look, not a verdict on the schedule.
