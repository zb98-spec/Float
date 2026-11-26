# Float bands

`float` shows how a project's unfinished work spreads across total float, in working days: how much is already late, how much is critical, and how much is close to it. It reads the float stored in the file, so schedule it first for fresh results.

```sh
totalfloat schedule plant.xer -o plant-scheduled.xer
totalfloat float plant-scheduled.xer
```

```text
Project rel-fs-positive (1): 2 unfinished, 2 critical
  below 0 days             0    0.0%
  0 days                   2  100.0%
  over 0 to 10 days        0    0.0%
  over 10 to 20 days       0    0.0%
  over 20 to 44 days       0    0.0%
  over 44 days             0    0.0%
```

Each line is a band, with how many activities fall in it and their share of the unfinished activities that have float. Float converts to days with each activity's calendar, or 8 hours a day where a calendar doesn't say. Completed work, level of effort and WBS summaries are left out, as [`check`](check.md) leaves them out. The 44-day band matches the high float limit of the DCMA assessment.

`--bands` sets the upper bounds of the bands above zero, in working days, ascending. Above the last one is a band of its own:

```sh
totalfloat float plant-scheduled.xer --bands 5,10,20
```

`--project` picks one project of a file with several, and `--json` prints schema `totalfloat.float/1`, which also lists the first 50 activity IDs in each band, lowest float first:

```sh
totalfloat float plant-scheduled.xer --project 1 --json
```

[`query --float-min --float-max`](query.md) lists every activity in a range of float, in hours.
