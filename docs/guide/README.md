# TotalFloat user guide

TotalFloat works with Primavera P6 `.xer` files from the command line or through an AI agent. It schedules a file, tells you why dates are what they are, and never changes your original.

It doesn't need P6 installed. It's not P6 either, so read [Limitations](limitations.md) before you rely on its dates.

1. [Start here](start-here.md)
2. [Install](install.md)
3. [See what's in a file](inspect.md)
4. [Schedule a file](schedule.md)
5. [Change a schedule](edit.md)
6. [Collect timesheets](timesheets.md)
7. [Start a new schedule](new.md)
8. [Move schedules to and from Microsoft Project](import.md)
9. [Find activities](query.md)
10. [Find out why a date is what it is](explain.md)
11. [Check schedule quality](check.md)
12. [See how float spreads](float.md)
13. [Write a report](report.md)
14. [Compare two updates](diff.md)
15. [Keep projects in an EPS](portfolio.md)
16. [Run a schedule risk analysis](risk.md)
17. [Keep a schedule's history](history.md)
18. [Keep schedules in git](git.md)
19. [Schedules as text](text.md)
20. [Work with an agent](agent-recipes.md)
21. [Open the local app](serve.md)
22. [Limitations](limitations.md)
23. [What TotalFloat supports](supported.md)

Every command in this guide is run by a test, so the examples can't drift from what the program does. The examples use a small two-activity schedule, `plant.xer`.

## Things that work the same everywhere

- **Your file is safe.** Commands only read it. Anything written goes to a new file, and an existing file is never replaced.
- **`--json`** gives output for scripts and agents.
- **Exit codes:** `0` is fine, `1` is an error, `2` is a mistyped command, and `3` means it finished and found something to look at.
- **Errors** say what went wrong and what to try next.
