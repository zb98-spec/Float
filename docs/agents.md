# Using TotalFloat from an agent

`totalfloat mcp` serves the Model Context Protocol over stdio, so any MCP client can open, schedule and question P6 schedules. It makes no network calls, and your agent is whichever one you already use ([ARCHITECTURE.md, Bring your own agent](ARCHITECTURE.md#bring-your-own-agent)).

All dates and floats come from the deterministic engine. An agent asks for them; it doesn't work them out.

## Connecting a client

Install TotalFloat first ([Install](guide/install.md)). The one-command install puts `totalfloat` on your `PATH`; otherwise use its full path below.

### Claude Code

```sh
claude mcp add totalfloat -- totalfloat mcp
```

### Claude Desktop

In Claude Desktop, open **Settings**, choose **Developer**, then **Edit Config**. That opens `claude_desktop_config.json`, which is `%APPDATA%\Claude\claude_desktop_config.json` on Windows and `~/Library/Application Support/Claude/claude_desktop_config.json` on macOS. Add this, then quit Claude Desktop completely and open it again.

```json
{
  "mcpServers": {
    "totalfloat": {
      "command": "totalfloat",
      "args": ["mcp"]
    }
  }
}
```

On Windows, if Claude Desktop can't start `totalfloat`, give the full path to `totalfloat.exe`. The one-command install puts it in `%LOCALAPPDATA%\Programs\totalfloat`, and `(Get-Command totalfloat).Source` in PowerShell prints the path. JSON needs each backslash doubled:

```json
"command": "C:\\Users\\YOUR-NAME\\AppData\\Local\\Programs\\totalfloat\\totalfloat.exe"
```

### Other clients

Point the client at the command `totalfloat mcp` with the stdio transport. It takes no arguments or environment variables.

## Tools

Open a file first. The handle saves re-sending a large file, and it lasts until `close` or the end of the session.

On Windows, tools take Windows paths. In JSON each backslash is doubled, as in `"C:\\data\\plant.xer"`, and forward slashes work too: `"C:/data/plant.xer"`.

| Tool | What it does |
|---|---|
| `open` | Opens an XER and returns a handle, with each project's activity count, data date and stored finish |
| `inspect` | Export version, encoding, projects with status counts and finish, tables, calendars, activity codes, resources and anything that couldn't be mapped |
| `import` | Converts an MS Project XML file to a new XER and reports what didn't translate |
| `new` | Starts a new schedule in a new file and returns a handle |
| `edit` | Changes the schedule, recalculates and reports the impact. `dry_run` looks without keeping it |
| `calendar` | What a calendar works: hours, the week, exception dates, and each day in a range |
| `schedule` | Calculates dates, float and the critical path, and keeps the results on the handle. Stops before changing anything if a project can't be calculated |
| `query` | Finds activities by filter, or follows `preds`, `succs` and `driving_path_to` |
| `explain` | Walks a date, or total float, back to its origin |
| `check` | Runs the DCMA 14-point assessment and seven more checks, listing up to 50 offenders each. `format` `markdown` or `html` returns the report as a document instead of JSON |
| `diff` | Compares two handles, usually consecutive updates. `counts` is complete, and each list keeps 100 items |
| `variance` | Measures a handle against a baseline handle: the finish and milestone variance, the largest slips and float losses, and critical path changes. `format` `markdown` or `html` returns the report as a document instead of JSON |
| `export` | Writes the handle's file to a new path. An existing file is never replaced |
| `close` | Ends a handle. Results that weren't exported are lost |

`query` always reports the `total` and returns a page: 50 rows by default, at most 200. Use `offset` for more, and `columns` to keep rows small. An unknown argument is an error, so a misspelled filter can't return everything.

`explain` keeps 20 steps from each end of a chain, and `total_steps` says how long it was. Use `query` with `driving_path_to` to page through the rest.

Values come from the file until you call `schedule`, then from the calculation. The source file never changes.

A call that fails returns a result marked as an error, holding a `totalfloat.error/1` document with a `code`, a `message` and a `hint` on what to try next. Arguments that don't fit a tool's schema, such as a status that doesn't exist, are also errors, but carry a plain message from the MCP library instead.

## Example: what drives the project finish?

1. `open` with `{"path": "/data/plant.xer"}` returns handle `s1`.
2. `schedule` with `{"handle": "s1"}` reports the finish and critical count.
3. `query` with `{"handle": "s1", "critical": true, "columns": ["code", "early_finish"]}` lists the critical activities. The one with the latest early finish sets the project finish.
4. `explain` with `{"handle": "s1", "activity": "A2", "field": "early_finish"}` walks that date back to its origin:

   ```text
   A2 early finish 2026-01-13 17:00: its early start and its duration
   A2 early start 2026-01-13 09:00: FS with 1h lag on the predecessor calendar from A1 early finish
   A1 early finish 2026-01-12 17:00: its early start and its duration
   A1 early start 2026-01-12 08:00: data date
   ```

5. `query` with `{"handle": "s1", "driving_path_to": "A2"}` gives every driving predecessor, including ties, nearest first.

## Limits

- Dates come from TotalFloat's own engine and can differ from the ones stored in a file; `schedule` lists where.
- Importing files written by `export` into other tools hasn't been verified.
- A project is refused, with a reason, if it lost rows in mapping or needs something the engine doesn't do, such as resource leveling or Actual Dates mode.
