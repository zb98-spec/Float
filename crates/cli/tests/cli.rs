//! End-to-end tests for the `totalfloat` binary.

#![allow(clippy::unwrap_used, reason = "test helpers outside #[test] functions")]

use std::path::Path;
use std::process::Command;

fn totalfloat() -> Command {
    Command::new(env!("CARGO_BIN_EXE_totalfloat"))
}

#[test]
fn version_prints_binary_name_and_version() {
    let output = totalfloat().arg("--version").output().unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!("totalfloat {}\n", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn help_succeeds_and_names_the_binary() {
    let output = totalfloat().arg("--help").output().unwrap();
    assert!(output.status.success());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("Usage: totalfloat")
    );
}

#[test]
fn unknown_argument_is_a_usage_error() {
    let output = totalfloat().arg("--no-such-flag").output().unwrap();
    assert_eq!(output.status.code(), Some(2));
}

const FIXTURE: &str = "../../fixtures/xer/model-mapping.xer";

fn run(args: &[&str]) -> (Option<i32>, String, String) {
    let out = totalfloat().args(args).output().unwrap();
    (
        out.status.code(),
        String::from_utf8(out.stdout).unwrap(),
        String::from_utf8(out.stderr).unwrap(),
    )
}

#[test]
fn inspect_human() {
    let (code, stdout, _) = run(&["inspect", FIXTURE]);
    assert_eq!(code, Some(0));
    insta::assert_snapshot!(stdout);
}

#[test]
fn inspect_json() {
    let (code, stdout, _) = run(&["inspect", FIXTURE, "--json"]);
    assert_eq!(code, Some(0));
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(doc["schema"], "totalfloat.inspect/1");
    insta::assert_snapshot!(stdout);
}

#[test]
fn roundtrip_human_and_json() {
    let (code, stdout, _) = run(&["roundtrip", FIXTURE]);
    assert_eq!(code, Some(0));
    insta::assert_snapshot!("roundtrip_human", stdout);
    let (code, stdout, _) = run(&["roundtrip", FIXTURE, "--json"]);
    assert_eq!(code, Some(0));
    insta::assert_snapshot!("roundtrip_json", stdout);
}

#[test]
fn missing_file_is_an_error_with_code_and_hint() {
    let (code, stdout, stderr) = run(&["roundtrip", "no-such-file.xer", "--json"]);
    assert_eq!(code, Some(1));
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(doc["schema"], "totalfloat.error/1");
    assert_eq!(doc["error"]["code"], "file-not-readable");
    assert!(doc["error"]["hint"].as_str().unwrap().contains("path"));
    assert_eq!(stderr, "");
    // Human mode: error and hint on stderr, nothing on stdout.
    let (code, stdout, stderr) = run(&["inspect", "no-such-file.xer"]);
    assert_eq!(code, Some(1));
    assert_eq!(stdout, "");
    assert!(stderr.starts_with("error[file-not-readable]: "), "{stderr}");
    assert!(stderr.contains("\n  hint: "), "{stderr}");
}

#[test]
fn non_xer_input_is_reported() {
    let path = std::env::temp_dir().join(format!("totalfloat-not-xer-{}.xer", std::process::id()));
    std::fs::write(&path, b"hello\n").unwrap();
    let (code, stdout, _) = run(&["inspect", path.to_str().unwrap(), "--json"]);
    std::fs::remove_file(&path).unwrap();
    assert_eq!(code, Some(1));
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(doc["error"]["code"], "not-an-xer");
}

#[test]
fn missing_argument_is_a_usage_error() {
    let (code, _, _) = run(&["inspect"]);
    assert_eq!(code, Some(2));
}

#[test]
fn anonymize_writes_a_new_file_with_counts_and_no_source_values() {
    let directory = tempfile::tempdir().unwrap();
    let output_path = directory.path().join("anonymous.xer");
    let fixture_path = FIXTURE;
    let output = totalfloat()
        .args([
            "anonymize",
            fixture_path,
            "--seed",
            "42",
            "--report",
            "--json",
            "-o",
        ])
        .arg(&output_path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["schema"], "totalfloat.anonymize/1");
    assert_eq!(report["external_validation"], "not-run");
    assert!(report["scrubbed_cells"].as_u64().unwrap() > 10);
    assert!(!String::from_utf8_lossy(&output.stdout).contains("Synthetic"));
    let bytes = std::fs::read(&output_path).unwrap();
    assert!(!String::from_utf8_lossy(&bytes).contains("Synthetic"));
    let second = directory.path().join("second.xer");
    assert!(
        totalfloat()
            .args(["anonymize", fixture_path, "--seed", "42", "-o"])
            .arg(&second)
            .status()
            .unwrap()
            .success()
    );
    assert_eq!(std::fs::read(second).unwrap(), bytes);
    let output = totalfloat()
        .args(["anonymize", fixture_path, "--json", "-o"])
        .arg(&output_path)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(std::fs::read(output_path).unwrap(), bytes);
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["error"]["code"], "output-exists");
}

#[test]
fn refused_anonymization_does_not_create_an_output() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("private.xer");
    let output = directory.path().join("anonymous.xer");
    std::fs::write(&input, b"ERMHDR\t23.12\t2026-01-01\tProject\tAlice\tBob\tCompany\tProject Management\tUSD\n%T\tFUTURE\n%F\topaque\n%R\tPRIVATE_SECRET\n%E\n").unwrap();
    let result = totalfloat()
        .args(["anonymize", "--json", "-o"])
        .arg(&output)
        .arg(&input)
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(1));
    assert!(!output.exists());
    assert!(!String::from_utf8_lossy(&result.stdout).contains("PRIVATE_SECRET"));
    assert!(
        !result
            .stderr
            .windows(14)
            .any(|value| value == b"PRIVATE_SECRET")
    );
}

const FRESH: &str = "../../fixtures/xer/fresh-writer.xer";

#[test]
fn schedule_human() {
    let (code, stdout, _) = run(&["schedule", FRESH]);
    assert_eq!(code, Some(0));
    insta::assert_snapshot!(stdout);
}

#[test]
fn schedule_json() {
    let (code, stdout, _) = run(&["schedule", FRESH, "--json"]);
    assert_eq!(code, Some(0));
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(doc["schema"], "totalfloat.schedule/1");
    insta::assert_json_snapshot!(doc);
}

/// `schedule` has no other name.
#[test]
fn schedule_has_no_alias() {
    let alias = concat!("f", "9");
    let (code, stdout, stderr) = run(&[alias, FRESH]);
    assert_eq!(code, Some(2), "{stderr}");
    assert_eq!(stdout, "");
    assert!(
        stderr.contains(&format!("unrecognized subcommand '{alias}'")),
        "{stderr}"
    );
}

#[test]
fn schedule_writes_a_new_file_and_a_second_run_changes_nothing() {
    let directory = tempfile::tempdir().unwrap();
    let first = directory.path().join("first.xer");
    let second = directory.path().join("second.xer");
    let source = std::fs::read(FRESH).unwrap();

    let (code, ..) = run(&["schedule", FRESH, "-o", first.to_str().unwrap()]);
    assert_eq!(code, Some(0));
    assert_eq!(std::fs::read(FRESH).unwrap(), source);
    assert_ne!(std::fs::read(&first).unwrap(), source);

    let (code, stdout, _) = run(&[
        "schedule",
        first.to_str().unwrap(),
        "-o",
        second.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(code, Some(0));
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(doc["projects"][0]["changed_activities"], 0);
    assert_eq!(
        std::fs::read(&first).unwrap(),
        std::fs::read(&second).unwrap()
    );
}

#[test]
fn schedule_says_what_changed_and_where_it_starts() {
    let (code, stdout, _) = run(&["schedule", FRESH, "--json"]);
    assert_eq!(code, Some(0));
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let project = &doc["projects"][0];
    assert_eq!(project["changed_dates_or_float"], 2);
    assert_eq!(project["changes"]["by_field"]["early_start"], 2);
    // A file never scheduled differs from both ends of its logic.
    assert_eq!(project["changes"]["starts_at"][0], "TF-001");
    assert_eq!(project["changes"]["starts_at"][1], "TF-002");
    assert_eq!(project["changes"]["starts_at_total"], 2);
    let first = &project["changes"]["activities"][0];
    assert_eq!(first["code"], "TF-001");
    assert_eq!(first["fields"][0]["field"], "early_start");
    assert_eq!(first["fields"][0]["from"], serde_json::Value::Null);
}

#[test]
fn schedule_notes_a_project_finish_its_activities_disagree_with() {
    let directory = tempfile::tempdir().unwrap();
    let file = edited(
        directory.path(),
        "rel-fs-positive",
        &[
            ("PROJECT", "1", "scd_end_date", "2026-02-02 17:00"),
            ("TASK", "2", "early_end_date", "2026-03-02 17:00"),
        ],
    );
    let (_, stdout, _) = run(&["schedule", &file, "--json"]);
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(doc["projects"][0]["previous_finish"], "2026-02-02 17:00");
    assert_eq!(doc["projects"][0]["activity_finish"], "2026-03-02 17:00");
    let (_, stdout, _) = run(&["schedule", &file]);
    assert!(stdout.contains(
        "note: the file's project finish is 2026-02-02 17:00, but its unfinished work finishes 2026-03-02 17:00"
    ));
}

#[test]
fn schedule_refuses_to_replace_a_file() {
    let directory = tempfile::tempdir().unwrap();
    let existing = directory.path().join("existing.xer");
    std::fs::write(&existing, "keep").unwrap();
    let (code, stdout, _) = run(&[
        "schedule",
        FRESH,
        "-o",
        existing.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(code, Some(1));
    assert!(stdout.contains("output-exists"));
    assert_eq!(std::fs::read(&existing).unwrap(), b"keep");
}

/// Float paths asked for one run, ending at an activity, and the
/// file's own options left as they were.
#[test]
fn schedule_finds_float_paths_asked_for() {
    let directory = tempfile::tempdir().unwrap();
    let out = directory.path().join("paths.xer");
    let out = out.to_str().unwrap();
    let (code, stdout, stderr) = run(&[
        "schedule",
        FRESH,
        "--float-paths",
        "3",
        "--float-path-to",
        "TF-002",
        "--float-paths-by",
        "free-float",
        "-o",
        out,
    ]);
    assert_eq!(code, Some(0), "{stdout}{stderr}");
    let (_, rows, _) = run(&[
        "query",
        out,
        "--columns",
        "code,float_path,float_path_order",
        "--json",
    ]);
    let rows: serde_json::Value = serde_json::from_str(&rows).unwrap();
    let path = |code: &str| {
        let row = rows["rows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["code"] == code)
            .unwrap();
        (row["float_path"].clone(), row["float_path_order"].clone())
    };
    assert_eq!(path("TF-001"), (1.into(), 1.into()), "{rows}");
    assert_eq!(path("TF-002"), (1.into(), 2.into()), "{rows}");
    let options = |file: &str| {
        let (_, out, _) = run(&["inspect", file, "--json"]);
        let doc: serde_json::Value = serde_json::from_str(&out).unwrap();
        doc["projects"][0]["options"].clone()
    };
    assert_eq!(options(out), options(FRESH));
    // A bad end activity or no paths is refused, and nothing is written.
    let refused = directory.path().join("refused.xer");
    let refused = refused.to_str().unwrap();
    for args in [
        ["--float-paths", "3", "--float-path-to", "NOPE"],
        ["--float-paths", "0", "--float-paths-by", "total-float"],
    ] {
        let mut all = vec!["schedule", FRESH, "-o", refused];
        all.extend(args);
        let (code, stdout, _) = run(&all);
        assert_eq!(code, Some(1), "{args:?}: {stdout}");
        assert!(!std::path::Path::new(refused).exists());
    }
    let (code, ..) = run(&["schedule", FRESH, "--float-path-to", "TF-002"]);
    assert_eq!(code, Some(2));
}

/// Float paths are refused where they can't be found or
/// written, with nothing written.
#[test]
fn schedule_refuses_float_paths_it_cannot_find_or_write() {
    let directory = tempfile::tempdir().unwrap();
    let fresh = std::fs::read_to_string(FRESH).unwrap();
    let out = directory.path().join("out.xer");
    let out = out.to_str().unwrap();
    // TF-001 as a level of effort: no path ends at a span.
    let loe = directory.path().join("loe.xer");
    std::fs::write(
        &loe,
        fresh.replacen("CP_Drtn\tTT_Task\t", "CP_Drtn\tTT_LOE\t", 1),
    )
    .unwrap();
    // A TASK table without the float path columns.
    let mut fields = Vec::new();
    let bare: Vec<String> = fresh
        .split("\r\n")
        .flat_map(|line| line.split('\n'))
        .scan(String::new(), |table, line| {
            if let Some(name) = line.strip_prefix("%T\t") {
                *table = name.to_owned();
            }
            if table != "TASK" {
                return Some(line.to_owned());
            }
            let cells: Vec<&str> = line.split('\t').collect();
            if line.starts_with("%F") {
                fields = cells.iter().map(|c| c.starts_with("float_path")).collect();
            }
            if !line.starts_with("%F") && !line.starts_with("%R") {
                return Some(line.to_owned());
            }
            let kept: Vec<&str> = cells
                .iter()
                .zip(&fields)
                .filter(|(_, drop)| !**drop)
                .map(|(c, _)| *c)
                .collect();
            Some(kept.join("\t"))
        })
        .collect();
    let columns = directory.path().join("columns.xer");
    std::fs::write(&columns, bare.join("\r\n")).unwrap();
    for (file, end, error) in [
        (loe.to_str().unwrap(), "TF-001", "bad-float-path-end"),
        (columns.to_str().unwrap(), "TF-002", "no-float-path-columns"),
    ] {
        let (code, stdout, _) = run(&[
            "schedule",
            file,
            "--float-paths",
            "2",
            "--float-path-to",
            end,
            "-o",
            out,
            "--json",
        ]);
        assert_eq!(code, Some(1), "{stdout}");
        assert!(stdout.contains(error), "{stdout}");
        assert!(!std::path::Path::new(out).exists());
    }
}

#[test]
fn schedule_moves_the_data_date_it_is_given() {
    let (code, stdout, _) = run(&[
        "schedule",
        FRESH,
        "--data-date",
        "2026-02-02 08:00",
        "--json",
    ]);
    assert_eq!(code, Some(0));
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(doc["projects"][0]["data_date"], "2026-02-02 08:00");
}

#[test]
fn schedule_keeps_planned_dates_with_moved_work_not_started() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("moved.xer");
    let (code, ..) = run(&[
        "schedule",
        FRESH,
        "--data-date",
        "2026-02-02 08:00",
        "-o",
        output.to_str().unwrap(),
    ]);
    assert_eq!(code, Some(0));
    let text = String::from_utf8_lossy(&std::fs::read(&output).unwrap()).into_owned();
    let lines: Vec<&str> = text.lines().collect();
    let fields = lines
        .iter()
        .skip_while(|line| !line.starts_with("%T\tTASK"))
        .nth(1)
        .unwrap();
    let column = |name: &str| fields.split('\t').position(|f| f == name).unwrap();
    let row = lines
        .iter()
        .skip_while(|line| !line.starts_with("%T\tTASK"))
        .nth(2)
        .unwrap();
    let cells: Vec<&str> = row.split('\t').collect();
    assert_eq!(cells[column("restart_date")], "2026-02-02 08:00");
    assert_eq!(
        cells[column("target_start_date")],
        cells[column("restart_date")]
    );
    assert_eq!(
        cells[column("target_end_date")],
        cells[column("reend_date")]
    );
}

#[cfg(unix)]
#[test]
fn schedule_output_takes_the_source_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source.xer");
    std::fs::copy(FRESH, &source).unwrap();
    std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o644)).unwrap();
    let output = directory.path().join("output.xer");
    run(&[
        "schedule",
        source.to_str().unwrap(),
        "-o",
        output.to_str().unwrap(),
    ]);
    let mode = std::fs::metadata(&output).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o644);
}

#[test]
fn schedule_explains_bad_input() {
    let error = |args: &[&str], expected: &str| {
        let (code, stdout, _) = run(args);
        assert_eq!(code, Some(1), "{args:?}");
        assert!(stdout.contains(expected), "{stdout}");
    };
    error(
        &["schedule", FRESH, "--data-date", "soon", "--json"],
        "invalid-date",
    );
    error(
        &["schedule", FRESH, "--project", "999999", "--json"],
        "no-such-project",
    );
    error(&["schedule", FIXTURE, "--json"], "schedule-refused");
}

#[test]
fn schedule_reports_without_writing_when_a_file_is_empty_of_projects() {
    let directory = tempfile::tempdir().unwrap();
    let empty = directory.path().join("empty.xer");
    std::fs::write(
        &empty,
        "ERMHDR\t23.12\t2026-01-01\tProject\tA\tA\tA\tP\tUSD\n%E\n",
    )
    .unwrap();
    let (code, stdout, _) = run(&["schedule", empty.to_str().unwrap(), "--json"]);
    assert_eq!(code, Some(1));
    assert!(stdout.contains("no-projects"), "{stdout}");
}

const CODED: &str = "../../fixtures/xer/fresh-writer.xer";

#[test]
fn inspect_lists_status_codes_resources_and_named_diagnostics() {
    let (code, stdout, _) = run(&["inspect", CODED, "--json"]);
    assert_eq!(code, Some(0));
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let project = &doc["projects"][0];
    assert_eq!(project["status"]["not_started"].as_u64(), Some(2));
    assert_eq!(doc["activity_codes"][0]["name"], "Synthetic phase");
    assert_eq!(doc["activity_codes"][0]["values"][0]["value"], "WORK");
    assert_eq!(doc["resources"][0]["short_name"], "TF-LABOR");
    let named = doc["diagnostics"]["mapping_details"].as_array().unwrap();
    assert!(
        named
            .iter()
            .any(|n| n["field"] == "sched_outer_depend_type"),
        "{named:?}"
    );
}

#[test]
fn query_shows_and_filters_on_resources_and_codes() {
    let (code, stdout, _) = run(&[
        "query",
        CODED,
        "--resource",
        "synthetic LABOR",
        "--columns",
        "code,resources,codes",
        "--json",
    ]);
    assert_eq!(code, Some(0));
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(doc["total"], 1);
    assert_eq!(doc["rows"][0]["code"], "TF-001");
    assert_eq!(doc["rows"][0]["resources"], "TF-LABOR");
    assert_eq!(doc["rows"][0]["codes"], "Synthetic phase=WORK");
}

#[test]
fn a_wrong_code_or_resource_says_what_the_file_has() {
    let (code, _, stderr) = run(&["query", CODED, "--code", "Trade=ELEC"]);
    assert_eq!(code, Some(1));
    assert!(stderr.contains("Synthetic phase: WORK"), "{stderr}");
    let (code, _, stderr) = run(&["query", CODED, "--resource", "NOBODY"]);
    assert_eq!(code, Some(1));
    assert!(stderr.contains("TF-LABOR"), "{stderr}");
}

#[test]
fn a_code_matches_ignoring_case() {
    let (code, stdout, _) = run(&[
        "query",
        CODED,
        "--code",
        "synthetic PHASE=work",
        "--limit",
        "0",
        "--json",
    ]);
    assert_eq!(code, Some(0));
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(doc["total"], 1);
}

#[test]
fn calendar_says_whether_a_date_is_worked() {
    let (code, stdout, _) = run(&[
        "calendar",
        CODED,
        "synthetic FIVE-day workweek",
        "--from",
        "2026-01-02",
        "--to",
        "2026-01-03",
        "--json",
    ]);
    assert_eq!(code, Some(0), "{stdout}");
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(doc["schema"], "totalfloat.calendar/1");
    assert_eq!(doc["week"][0]["weekday"], "monday");
    let days = doc["days"].as_array().unwrap();
    assert_eq!(days.len(), 2);
    assert!(days[0]["hours"].as_f64().unwrap() > 0.0, "{days:?}");
    assert_eq!(days[1]["weekday"], "saturday");
    assert_eq!(days[1]["hours"].as_f64(), Some(0.0));
}

#[test]
fn calendar_human() {
    let (code, stdout, _) = run(&[
        "calendar",
        CODED,
        "100",
        "--from",
        "2026-01-02",
        "--to",
        "2026-01-04",
    ]);
    assert_eq!(code, Some(0));
    insta::assert_snapshot!(stdout);
}

#[test]
fn an_open_range_lists_exceptions_but_no_days() {
    let (code, stdout, _) = run(&["calendar", CODED, "100", "--from", "2026-01-02", "--json"]);
    assert_eq!(code, Some(0));
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(doc["days"].as_array().unwrap().len(), 0);
}

#[test]
fn a_blank_code_value_matches_nothing() {
    let (code, _, stderr) = run(&["query", CODED, "--code", "Synthetic phase="]);
    assert_eq!(code, Some(1));
    assert!(stderr.contains("no-such-code"), "{stderr}");
}

#[test]
fn calendar_refusals_say_what_to_do() {
    let (code, _, stderr) = run(&["calendar", CODED, "Nope"]);
    assert_eq!(code, Some(1));
    assert!(stderr.contains("Synthetic five-day workweek"), "{stderr}");
    let (code, _, stderr) = run(&["calendar", CODED, "100", "--from", "soon"]);
    assert_eq!(code, Some(1));
    assert!(stderr.contains("invalid-date"), "{stderr}");
    let (code, _, stderr) = run(&[
        "calendar",
        CODED,
        "100",
        "--from",
        "2026-02-01",
        "--to",
        "2026-01-01",
    ]);
    assert_eq!(code, Some(1));
    assert!(stderr.contains("invalid-range"), "{stderr}");
    let (code, _, stderr) = run(&[
        "calendar",
        CODED,
        "100",
        "--from",
        "2026-01-01",
        "--to",
        "2028-01-01",
    ]);
    assert_eq!(code, Some(1));
    assert!(stderr.contains("range-too-long"), "{stderr}");
}

/// Every calendar every file offers can be shown.
#[test]
fn calendar_shows_every_corpus_calendar() {
    let Some(root) = std::env::var_os("TOTALFLOAT_CORPUS") else {
        eprintln!("TOTALFLOAT_CORPUS unset; skipping calendar corpus test");
        return;
    };
    let mut shown = 0;
    for entry in std::fs::read_dir(std::path::Path::new(&root).join("files")).unwrap() {
        for file in std::fs::read_dir(entry.unwrap().path()).unwrap() {
            let file = file.unwrap().path();
            let (code, stdout, _) = run(&["inspect", file.to_str().unwrap(), "--json"]);
            if code != Some(0) {
                continue;
            }
            let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
            for calendar in doc["calendars"].as_array().unwrap() {
                let id = calendar["id"].to_string();
                let (code, _, stderr) = run(&["calendar", file.to_str().unwrap(), &id]);
                assert_eq!(code, Some(0), "{file:?} {id} {stderr}");
                shown += 1;
            }
        }
    }
    eprintln!("{shown} calendars shown");
    assert!(shown > 100);
}

const FS_LINK: &str = "../../fixtures/xer/batches/001/rel-fs-positive.xer";

fn edit_json(args: &[&str]) -> serde_json::Value {
    let mut all = vec!["edit", FS_LINK, "--json"];
    all.extend_from_slice(args);
    let (code, stdout, stderr) = run(&all);
    assert_eq!(code, Some(0), "{stdout}{stderr}");
    serde_json::from_str(&stdout).unwrap()
}

#[test]
fn edit_human() {
    let (code, stdout, _) = run(&[
        "edit",
        FS_LINK,
        "--duration",
        "A1=15d",
        "--link",
        "A2>A1:SS",
    ]);
    assert_eq!(code, Some(1), "{stdout}");
    let (code, stdout, _) = run(&["edit", FS_LINK, "--duration", "A1=15d"]);
    assert_eq!(code, Some(0));
    insta::assert_snapshot!(stdout);
}

#[test]
fn edit_reports_the_move_and_writes_nothing_by_default() {
    let doc = edit_json(&["--duration", "A1=15d"]);
    assert_eq!(doc["schema"], "totalfloat.edit/1");
    assert_eq!(doc["output"], serde_json::Value::Null);
    assert_eq!(doc["projects"][0]["scheduled_finish"], "2026-02-02 17:00");
    assert_eq!(doc["projects"][0]["previous_finish"], "2026-01-13 17:00");
    assert_eq!(doc["projects"][0]["activities"], 2);
}

/// `edit` gives what a person gets by changing the file and running `schedule`.
#[test]
fn edit_matches_changing_the_file_by_hand() {
    let directory = tempfile::tempdir().unwrap();
    let edited = directory.path().join("edited.xer");
    let (code, stdout, _) = run(&[
        "edit",
        FS_LINK,
        "--duration",
        "A1=120h",
        "-o",
        edited.to_str().unwrap(),
    ]);
    assert_eq!(code, Some(0), "{stdout}");
    let text = std::fs::read(FS_LINK).unwrap();
    let text = String::from_utf8(text).unwrap();
    let mut table = String::new();
    let mut fields: Vec<String> = Vec::new();
    let mut lines = Vec::new();
    for line in text.split("\r\n") {
        let mut cells: Vec<String> = line.split('\t').map(str::to_owned).collect();
        match cells[0].as_str() {
            "%T" => table = cells[1].clone(),
            "%F" => fields = cells[1..].to_vec(),
            "%R" if table == "TASK" => {
                let at = |name: &str| fields.iter().position(|f| f == name).unwrap() + 1;
                if cells[at("task_code")] == "A1" {
                    let (original, remaining) =
                        (at("target_drtn_hr_cnt"), at("remain_drtn_hr_cnt"));
                    cells[original] = "120".to_owned();
                    cells[remaining] = "120".to_owned();
                }
            }
            _ => {}
        }
        lines.push(cells.join("\t"));
    }
    let by_hand = directory.path().join("by-hand.xer");
    std::fs::write(&by_hand, lines.join("\r\n")).unwrap();
    let scheduled = directory.path().join("scheduled.xer");
    let (code, ..) = run(&[
        "schedule",
        by_hand.to_str().unwrap(),
        "-o",
        scheduled.to_str().unwrap(),
    ]);
    assert_eq!(code, Some(0));
    let columns = "code,early_start,early_finish,late_start,late_finish,total_float_hours";
    let dates =
        |file: &std::path::Path| run(&["query", file.to_str().unwrap(), "--columns", columns]).1;
    assert_eq!(dates(&edited), dates(&scheduled));
}

#[test]
fn an_edited_file_reads_back_and_recalculates_with_no_changes() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("out.xer");
    let before = std::fs::read(FS_LINK).unwrap();
    let (code, ..) = run(&[
        "edit",
        FS_LINK,
        "--duration",
        "A1=3d",
        "--relink",
        "A1>A2:SS+2d",
        "--constraint",
        "A2=start-on-or-after@2026-02-02 08:00",
        "-o",
        output.to_str().unwrap(),
    ]);
    assert_eq!(code, Some(0));
    assert_eq!(std::fs::read(FS_LINK).unwrap(), before);
    let (code, stdout, _) = run(&["schedule", output.to_str().unwrap(), "--json"]);
    assert_eq!(code, Some(0));
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(doc["projects"][0]["changed_activities"], 0);
    let (_, stdout, _) = run(&[
        "query",
        output.to_str().unwrap(),
        "--preds",
        "A2",
        "--columns",
        "link_type,link_lag_hours",
        "--json",
    ]);
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(doc["rows"][0]["link_type"], "SS");
    assert_eq!(doc["rows"][0]["link_lag_hours"].as_f64(), Some(16.0));
}

#[test]
fn a_refused_edit_writes_nothing() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("out.xer");
    let (code, _, stderr) = run(&[
        "edit",
        FS_LINK,
        "--duration",
        "A1=3d",
        "--link",
        "A2>A1",
        "-o",
        output.to_str().unwrap(),
    ]);
    assert_eq!(code, Some(1));
    assert!(stderr.contains("circular-logic"), "{stderr}");
    assert!(!output.exists());
}

#[test]
fn edit_refusals_say_what_to_do() {
    for (args, wanted) in [
        (vec!["edit", FS_LINK], "nothing-to-change"),
        (vec!["edit", FS_LINK, "--duration", "A1"], "invalid-edit"),
        (
            vec!["edit", FS_LINK, "--duration", "A1=soon"],
            "invalid-edit",
        ),
        (
            vec!["edit", FS_LINK, "--duration", "NOPE=1d"],
            "no-such-activity",
        ),
        (vec!["edit", FS_LINK, "--link", "A1"], "invalid-edit"),
        (vec!["edit", FS_LINK, "--link", "A1>A2:XX"], "invalid-edit"),
        (
            vec![
                "edit",
                FS_LINK,
                "--constraint",
                "A1=sometime@2026-01-01 08:00",
            ],
            "invalid-edit",
        ),
        (
            vec!["edit", FS_LINK, "--edits", "/no/such/file"],
            "file-not-readable",
        ),
    ] {
        let (code, _, stderr) = run(&args);
        assert_eq!(code, Some(1), "{args:?}");
        assert!(stderr.contains(wanted), "{args:?}: {stderr}");
    }
}

/// A broken flag, a missing WBS, two unknown activities and a link to the
/// activity that couldn't be added.
const MANY_WRONG: [&str; 10] = [
    "--duration",
    "A1=soon",
    "--duration",
    "A9=2d",
    "--add",
    "B1;New;NOWBS;2d",
    "--link",
    "A1>B1",
    "--name",
    "A3=x",
];

#[test]
fn every_refused_edit_is_reported_at_once() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("out.xer");
    let mut args = vec!["edit", FS_LINK];
    args.extend(MANY_WRONG);
    args.extend(["-o", output.to_str().unwrap()]);
    let (code, _, stderr) = run(&args);
    assert_eq!(code, Some(1), "{stderr}");
    assert!(!output.exists());
    insta::assert_snapshot!("edit_refusals_human", stderr);
    let mut args = vec!["edit", FS_LINK, "--json"];
    args.extend(MANY_WRONG);
    let (code, stdout, _) = run(&args);
    assert_eq!(code, Some(1));
    insta::assert_snapshot!("edit_refusals_json", stdout);
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let problems = doc["error"]["problems"].as_array().unwrap();
    assert_eq!(problems.len(), 5);
    let link = problems
        .iter()
        .find(|p| p["source"] == "--link A1>B1")
        .unwrap();
    let add = problems
        .iter()
        .find(|p| p["code"] == "no-such-wbs")
        .unwrap();
    assert_eq!(link["depends_on"], add["edit"]);
    let typo = problems
        .iter()
        .find(|p| p["source"] == "--duration A9=2d")
        .unwrap();
    assert!(
        typo["hint"]
            .as_str()
            .unwrap()
            .starts_with("did you mean A1 or A2?")
    );
}

#[test]
fn a_refusal_of_something_the_file_holds_depends_on_nothing() {
    // The add is refused because A1 exists, and the self-link for its own
    // reason: it didn't need the add.
    let (code, stdout, _) = run(&[
        "edit",
        FS_LINK,
        "--json",
        "--add",
        "A1;Again;rel-fs-positive;2d",
        "--link",
        "A1>A1",
    ]);
    assert_eq!(code, Some(1));
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let problems = doc["error"]["problems"].as_array().unwrap();
    assert_eq!(problems.len(), 2, "{doc}");
    assert!(problems[1].get("depends_on").is_none(), "{doc}");
}

#[test]
fn an_error_for_the_whole_batch_still_lists_unread_flags() {
    let (code, _, stderr) = run(&["edit", FS_LINK, "--project", "999", "--duration", "A1=soon"]);
    assert_eq!(code, Some(1));
    assert!(stderr.contains("error[no-such-project]"), "{stderr}");
    assert!(
        stderr.contains("--duration A1=soon: soon isn't a valid span"),
        "{stderr}"
    );
}

/// Seven edits: two refused, one needing a refused one, four that apply.
const PARTLY_WRONG: [&str; 14] = [
    "--duration",
    "A1=2d",
    "--duration",
    "A2=3d",
    "--duration",
    "A9=1d",
    "--add",
    "B1;New;NOWBS;2d",
    "--link",
    "A1>B1",
    "--name",
    "A1=First",
    "--name",
    "A2=Second",
];

#[test]
fn skip_failed_applies_the_rest_and_lists_what_it_skipped() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("out.xer");
    let mut args = vec!["edit", FS_LINK];
    args.extend(PARTLY_WRONG);
    args.extend(["-o", output.to_str().unwrap()]);
    // Without the flag, nothing is written.
    let (code, _, stderr) = run(&args);
    assert_eq!(code, Some(1), "{stderr}");
    assert!(!output.exists());
    args.push("--skip-failed");
    let (code, stdout, stderr) = run(&args);
    assert_eq!(code, Some(3), "{stdout}{stderr}");
    assert!(output.exists());
    insta::assert_snapshot!(
        "edit_skip_failed",
        stdout.replace(output.to_str().unwrap(), "<out>")
    );
    let (_, stdout, _) = run(&["query", output.to_str().unwrap(), "--columns", "code,name"]);
    assert!(
        stdout.contains("First") && stdout.contains("Second"),
        "{stdout}"
    );
    // Looking without -o lists the same and writes nothing.
    let mut args = vec!["edit", FS_LINK, "--json", "--skip-failed"];
    args.extend(PARTLY_WRONG);
    let (code, stdout, _) = run(&args);
    assert_eq!(code, Some(3));
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(doc["output"], serde_json::Value::Null);
    let skipped = doc["skipped"].as_array().unwrap();
    assert_eq!(skipped.len(), 3, "{doc}");
    let add = skipped.iter().find(|p| p["code"] == "no-such-wbs").unwrap();
    assert_eq!(skipped[2]["code"], "needs-refused-edit");
    assert_eq!(skipped[2]["depends_on"], add["edit"]);
    assert_eq!(doc["applied"].as_array().unwrap().len(), 4);
}

#[test]
fn skip_failed_takes_the_good_rows_of_a_status_table() {
    let directory = tempfile::tempdir().unwrap();
    let table = directory.path().join("status.csv");
    std::fs::write(
        &table,
        "activity,actual_start\nA1,2026-01-12 08:00\nNOPE,2026-01-12 08:00\n",
    )
    .unwrap();
    let (code, stdout, stderr) = run(&[
        "edit",
        FS_LINK,
        "--skip-failed",
        "--data-date",
        "2026-01-13 08:00",
        "--progress-table",
        table.to_str().unwrap(),
    ]);
    assert_eq!(code, Some(3), "{stdout}{stderr}");
    assert!(stdout.contains("line 3: no project holds NOPE"), "{stdout}");
    assert!(stdout.contains("A1 is in progress"), "{stdout}");
}

#[test]
fn emitted_commands_replay_to_the_same_report() {
    let directory = tempfile::tempdir().unwrap();
    let saved = directory.path().join("commands.json");
    let (code, first, stderr) = run(&[
        "edit",
        FS_LINK,
        "--json",
        "--duration",
        "A1=2d",
        "--link",
        "A2>A1:SS",
        "--unlink",
        "A1>A2",
        "--data-date",
        "2026-01-12 08:00",
        "--emit-commands",
        saved.to_str().unwrap(),
    ]);
    assert_eq!(code, Some(0), "{stderr}");
    let doc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&saved).unwrap()).unwrap();
    assert_eq!(doc["schema"], "totalfloat.command/1");
    assert_eq!(doc["options"]["data_date"], "2026-01-12 08:00");
    let (code, again, stderr) = run(&[
        "edit",
        FS_LINK,
        "--json",
        "--edits",
        saved.to_str().unwrap(),
    ]);
    assert_eq!(code, Some(0), "{stderr}");
    assert_eq!(first, again);
    // A second emit to the same path is refused, like any output.
    let (code, ..) = run(&[
        "edit",
        FS_LINK,
        "--duration",
        "A1=2d",
        "--emit-commands",
        saved.to_str().unwrap(),
    ]);
    assert_eq!(code, Some(1));
}

#[test]
fn a_command_document_from_a_later_version_is_refused_by_name() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("later.json");
    std::fs::write(
        &path,
        r#"{"schema": "totalfloat.command/9", "edits": [], "later": 1}"#,
    )
    .unwrap();
    let (code, _, stderr) = run(&["edit", FS_LINK, "--edits", path.to_str().unwrap()]);
    assert_eq!(code, Some(1));
    assert!(stderr.contains("unsupported-schema"), "{stderr}");
    assert!(stderr.contains("totalfloat.command/1"), "{stderr}");
    std::fs::write(
        &path,
        r#"{"schema": "totalfloat.command/1", "edits": [], "options": {"project": 5}}"#,
    )
    .unwrap();
    let (code, _, stderr) = run(&[
        "edit",
        FS_LINK,
        "--project",
        "1",
        "--edits",
        path.to_str().unwrap(),
    ]);
    assert_eq!(code, Some(1));
    assert!(stderr.contains("conflicting-options"), "{stderr}");
}

/// Every key an error prints is in the published schema.
#[test]
fn error_json_matches_the_published_schema() {
    let schema: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string("../../docs/schemas/error-1.schema.json").unwrap(),
    )
    .unwrap();
    let mut args = vec!["edit", FS_LINK, "--json"];
    args.extend(MANY_WRONG);
    let (_, stdout, _) = run(&args);
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(doc["schema"], schema["$id"]);
    let error = &schema["properties"]["error"]["properties"];
    for key in doc["error"].as_object().unwrap().keys() {
        assert!(error.get(key).is_some(), "{key} isn't in the schema");
    }
    let problem = &schema["$defs"]["problem"]["properties"];
    for item in doc["error"]["problems"].as_array().unwrap() {
        for key in item.as_object().unwrap().keys() {
            assert!(problem.get(key).is_some(), "{key} isn't in the schema");
        }
    }
}

#[test]
fn refused_items_of_an_edits_file_are_named_by_number() {
    let directory = tempfile::tempdir().unwrap();
    let edits = directory.path().join("edits.json");
    std::fs::write(
        &edits,
        r#"[{"op": "set_duration", "activity": "A1", "original": "2d"},
            {"op": "set_name", "activity": "NOPE", "name": "x"},
            {"op": "set_calendar", "activity": "A2", "calendar": "Nowhere"}]"#,
    )
    .unwrap();
    let (code, stdout, _) = run(&[
        "edit",
        FS_LINK,
        "--json",
        "--edits",
        edits.to_str().unwrap(),
    ]);
    assert_eq!(code, Some(1));
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let sources: Vec<&str> = doc["error"]["problems"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["source"].as_str().unwrap())
        .collect();
    assert_eq!(sources, ["edits.json item 2", "edits.json item 3"]);
}

#[test]
fn a_new_link_is_saved_above_every_id_in_the_file() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("out.xer");
    let (code, stdout, stderr) = run(&[
        "edit",
        FS_LINK,
        "--unlink",
        "A1>A2",
        "--link",
        "A1>A2:FF+1d",
        "-o",
        output.to_str().unwrap(),
    ]);
    assert_eq!(code, Some(0), "{stdout}{stderr}");
    let (_, stdout, _) = run(&[
        "query",
        output.to_str().unwrap(),
        "--succs",
        "A1",
        "--columns",
        "code,link_type",
    ]);
    assert!(stdout.contains("FF"), "{stdout}");
}

#[test]
fn flags_work_in_any_order_the_edits_need() {
    let directory = tempfile::tempdir().unwrap();
    let first = directory.path().join("first.xer");
    let second = directory.path().join("second.xer");
    let (code, ..) = run(&[
        "edit",
        FS_LINK,
        "--constraint",
        "A1=start-on-or-after@2026-02-02 08:00",
        "-o",
        first.to_str().unwrap(),
    ]);
    assert_eq!(code, Some(0));
    let (code, stdout, stderr) = run(&[
        "edit",
        first.to_str().unwrap(),
        "--constraint",
        "A1=finish-on-or-before@2026-03-02 17:00",
        "--clear-constraint",
        "A1",
        "-o",
        second.to_str().unwrap(),
    ]);
    assert_eq!(code, Some(0), "{stdout}{stderr}");
    let (_, stdout, _) = run(&[
        "query",
        second.to_str().unwrap(),
        "--constrained",
        "--columns",
        "code,constraint",
    ]);
    assert!(stdout.contains("finish-on-or-before"), "{stdout}");
    let (code, _, stderr) = run(&["edit", FS_LINK, "--project", "999", "--duration", "A1=1d"]);
    assert_eq!(code, Some(1));
    assert!(stderr.contains("no-such-project"), "{stderr}");
}

/// A rename changes one cell of what `schedule` writes, so no stored result is lost.
fn assert_a_rename_changes_one_line(
    directory: &std::path::Path,
    name: &str,
    project: &str,
    code: &str,
) {
    let (renamed, scheduled) = (
        directory.join("renamed.xer"),
        directory.join("scheduled.xer"),
    );
    let new_name = format!("{code}=A new name");
    let renamed_text = renamed.to_str().unwrap();
    let scheduled_text = scheduled.to_str().unwrap();
    run(&[
        "edit",
        name,
        "--project",
        project,
        "--name",
        &new_name,
        "-o",
        renamed_text,
    ]);
    run(&["schedule", name, "--project", project, "-o", scheduled_text]);
    let lines = |path: &std::path::Path| {
        let bytes = std::fs::read(path).unwrap();
        bytes
            .split(|b| *b == b'\n')
            .map(<[u8]>::to_vec)
            .collect::<Vec<_>>()
    };
    let (first, second) = (lines(&renamed), lines(&scheduled));
    assert_eq!(first.len(), second.len(), "{name}");
    let different = first.iter().zip(&second).filter(|(a, b)| a != b).count();
    assert_eq!(different, 1, "{name}");
    std::fs::remove_file(&renamed).unwrap();
    std::fs::remove_file(&scheduled).unwrap();
}

#[test]
fn an_added_activity_is_scheduled_saved_and_settled() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("out.xer");
    let (_, stdout, _) = run(&[
        "query",
        FS_LINK,
        "--columns",
        "wbs",
        "--limit",
        "1",
        "--json",
    ]);
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let wbs = doc["rows"][0]["wbs"].as_str().unwrap();
    let new = format!("N1;Inspect;{wbs};5d");
    let (code, stdout, stderr) = run(&[
        "edit",
        FS_LINK,
        "--add",
        &new,
        "--link",
        "A1>N1",
        "--link",
        "N1>A2",
        "-o",
        output.to_str().unwrap(),
    ]);
    assert_eq!(code, Some(0), "{stdout}{stderr}");
    let saved = output.to_str().unwrap();
    let (code, stdout, _) = run(&["schedule", saved, "--json"]);
    assert_eq!(code, Some(0));
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(doc["projects"][0]["changed_activities"], 0, "{doc}");
    assert_eq!(doc["projects"][0]["activities"], 3);
    let (_, stdout, _) = run(&[
        "query",
        saved,
        "--text",
        "N1",
        "--columns",
        "early_start",
        "--json",
    ]);
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(doc["rows"][0]["early_start"].is_string(), "{doc}");
    let bytes = std::fs::read(&output).unwrap();
    let text = String::from_utf8_lossy(&bytes);
    let row = text.lines().find(|line| line.contains("\tN1\t")).unwrap();
    assert!(row.contains("PT_Normal"), "{row}");
    let (code, stdout, _) = run(&["roundtrip", saved]);
    assert_eq!(code, Some(0), "{stdout}");
}

#[test]
fn a_deleted_activity_leaves_nothing_behind() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("out.xer");
    let saved = output.to_str().unwrap();
    let (code, stdout, stderr) = run(&["edit", CODED, "--delete", "TF-001", "-o", saved, "--json"]);
    assert_eq!(code, Some(0), "{stdout}{stderr}");
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(
        doc["applied"][0]
            .as_str()
            .unwrap()
            .starts_with("TF-001 deleted"),
        "{doc}"
    );
    let bytes = std::fs::read(&output).unwrap();
    assert!(!String::from_utf8_lossy(&bytes).contains("TF-001"));
    let (code, stdout, _) = run(&["schedule", saved, "--json"]);
    assert_eq!(code, Some(0));
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(doc["projects"][0]["changed_activities"], 0, "{doc}");
}

#[test]
fn an_added_activity_shows_in_the_impact() {
    let (_, stdout, _) = run(&[
        "query",
        FS_LINK,
        "--columns",
        "wbs",
        "--limit",
        "1",
        "--json",
    ]);
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let new = format!(
        "N1;Pour; cure;{};40h",
        doc["rows"][0]["wbs"].as_str().unwrap()
    );
    let doc = edit_json(&["--add", &new, "--link", "A2>N1"]);
    assert_eq!(doc["projects"][0]["added"][0], "N1");
    assert_eq!(doc["projects"][0]["became_critical"][0], "N1");
    assert_eq!(doc["applied"][0], "N1 added, 40h long, with no links yet");
}

#[test]
fn a_new_activity_needs_its_calendar_in_the_same_edit() {
    let (code, _, stderr) = run(&[
        "edit",
        FS_LINK,
        "--add",
        "N1;Pour;ROOT;5d",
        "--calendar",
        "N1=100",
    ]);
    assert_eq!(code, Some(1));
    assert!(stderr.contains("add_activity"), "{stderr}");
}

#[test]
fn a_new_activity_in_a_file_of_many_projects_needs_a_project() {
    let (code, _, stderr) = run(&["edit", FIXTURE, "--add", "N1;Pour;ROOT;5d"]);
    assert_eq!(code, Some(1));
    assert!(stderr.contains("needs-project"), "{stderr}");
}

/// Takes a fixture's progress away and records it again, then reads A2.
fn progressed(fixture: &str) -> serde_json::Value {
    let directory = tempfile::tempdir().unwrap();
    let (blank, again) = (
        directory.path().join("blank.xer"),
        directory.path().join("again.xer"),
    );
    let (code, stdout, stderr) = run(&[
        "edit",
        fixture,
        "--clear-progress",
        "A2",
        "-o",
        blank.to_str().unwrap(),
    ]);
    assert_eq!(code, Some(0), "{stdout}{stderr}");
    let (code, stdout, stderr) = run(&[
        "edit",
        blank.to_str().unwrap(),
        "--actual-start",
        "A2=2026-01-05 08:00",
        "--remaining",
        "A2=4h",
        "-o",
        again.to_str().unwrap(),
    ]);
    assert_eq!(code, Some(0), "{stdout}{stderr}");
    let (_, stdout, _) = run(&[
        "query",
        again.to_str().unwrap(),
        "--text",
        "A2",
        "--columns",
        "status,early_start,early_finish,remaining_duration_hours",
        "--json",
    ]);
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    doc["rows"][0].clone()
}

/// A2 started before its predecessor A1, with 4 hours left. Worked by hand: A1
/// takes all of Monday on weekdays, 08:00-12:00 and 13:00-17:00, and A2 runs on
/// a calendar of 08:00-24:00 every day.
#[test]
fn recorded_progress_calculates_as_the_mode_says() {
    let retained = progressed("../../fixtures/xer/batches/001/oos-retained.xer");
    assert_eq!(retained["status"], "in-progress");
    assert_eq!(retained["remaining_duration_hours"].as_f64(), Some(4.0));
    // The link holds, so the rest waits for A1: Monday evening.
    assert_eq!(retained["early_start"], "2026-01-12 17:00");
    assert_eq!(retained["early_finish"], "2026-01-12 21:00");
    let overridden = progressed("../../fixtures/xer/batches/001/oos-override.xer");
    // The link is ignored, so the rest runs from the data date: Monday morning.
    assert_eq!(overridden["early_start"], "2026-01-12 08:00");
    assert_eq!(overridden["early_finish"], "2026-01-12 12:00");
}

#[test]
fn an_update_measures_actuals_against_the_new_data_date() {
    let future = "A1=2026-01-13 08:00";
    let (code, _, stderr) = run(&["edit", FS_LINK, "--actual-start", future]);
    assert_eq!(code, Some(1));
    assert!(stderr.contains("actual-after-data-date"), "{stderr}");
    let doc = edit_json(&[
        "--actual-start",
        future,
        "--actual-finish",
        "A1=2026-01-13 17:00",
        "--data-date",
        "2026-01-19 08:00",
    ]);
    assert_eq!(doc["projects"][0]["data_date"], "2026-01-19 08:00");
    assert!(
        doc["applied"][0].as_str().unwrap().contains("completed"),
        "{doc}"
    );
}

#[test]
fn remaining_and_actuals_on_one_activity_are_checked_together() {
    let doc = edit_json(&[
        "--actual-start",
        "A1=2026-01-12 08:00",
        "--remaining",
        "A1=2h",
    ]);
    assert_eq!(doc["applied"][0], "A1 is in progress, with 2h remaining");
    let (code, _, stderr) = run(&[
        "edit",
        FS_LINK,
        "--actual-start",
        "A1=2026-01-12 08:00",
        "--remaining",
        "A1=0h",
    ]);
    assert_eq!(code, Some(1));
    assert!(stderr.contains("finished-work"), "{stderr}");
}

#[test]
fn moving_the_data_date_back_past_actuals_is_flagged() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("out.xer");
    let (code, ..) = run(&[
        "edit",
        FS_LINK,
        "--actual-start",
        "A1=2026-01-12 08:00",
        "-o",
        output.to_str().unwrap(),
    ]);
    assert_eq!(code, Some(0));
    let (code, stdout, _) = run(&[
        "edit",
        output.to_str().unwrap(),
        "--data-date",
        "2026-01-05 08:00",
        "--json",
    ]);
    assert_eq!(code, Some(0));
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(
        doc["warnings"][0]
            .as_str()
            .unwrap()
            .contains("after the data date"),
        "{doc}"
    );
}

#[test]
fn text_beyond_ascii_reaches_a_pure_ascii_file_as_windows_1252() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("out.xer");
    let saved = output.to_str().unwrap();
    let (code, stdout, stderr) = run(&["edit", FS_LINK, "--name", "A1=Café", "-o", saved]);
    assert_eq!(code, Some(0), "{stdout}{stderr}");
    let bytes = std::fs::read(&output).unwrap();
    assert!(bytes.windows(5).any(|w| w == b"Caf\xe9\t"));
    let (_, stdout, _) = run(&["query", saved, "--text", "A1", "--columns", "name"]);
    assert!(stdout.contains("Café"), "{stdout}");
    let (code, _, stderr) = run(&["edit", FS_LINK, "--name", "A1=Pour → cure"]);
    assert_eq!(code, Some(1));
    assert!(stderr.contains("edit-not-written"), "{stderr}");
}

#[test]
fn an_ignored_expected_finish_says_so() {
    let directory = tempfile::tempdir().unwrap();
    let edits = directory.path().join("edits.json");
    std::fs::write(
        &edits,
        r#"[{"op": "set_expected_finish", "activity": "A2", "date": "2026-01-05 17:00"}]"#,
    )
    .unwrap();
    let doc = edit_json(&["--edits", edits.to_str().unwrap()]);
    assert!(
        doc["warnings"][0].as_str().unwrap().contains("no effect"),
        "{doc}"
    );
}

#[test]
fn a_holiday_moves_work_and_the_file_settles() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("out.xer");
    let saved = output.to_str().unwrap();
    let (code, stdout, stderr) = run(&[
        "edit",
        FS_LINK,
        "--holiday",
        "Synthetic weekdays=2026-01-12",
        "-o",
        saved,
        "--json",
    ]);
    assert_eq!(code, Some(0), "{stdout}{stderr}");
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_ne!(
        doc["projects"][0]["scheduled_finish"], doc["projects"][0]["previous_finish"],
        "{doc}"
    );
    let (code, stdout, _) = run(&["schedule", saved, "--json"]);
    assert_eq!(code, Some(0));
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(doc["projects"][0]["changed_activities"], 0, "{doc}");
    let (_, stdout, _) = run(&["calendar", saved, "Synthetic weekdays", "--json"]);
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(doc["exceptions"][0]["date"], "2026-01-12");
    assert_eq!(doc["week"][0]["hours"].as_f64(), Some(8.0));
}

#[test]
fn any_global_calendar_can_be_chosen() {
    let doc = edit_json(&["--calendar", "A1=Synthetic project six-day"]);
    assert_eq!(
        doc["applied"][0],
        "A1 calendar set to Synthetic project six-day"
    );
}

#[test]
fn a_new_calendar_can_take_a_holiday_and_work_in_one_call() {
    let directory = tempfile::tempdir().unwrap();
    let edits = directory.path().join("edits.json");
    let output = directory.path().join("out.xer");
    std::fs::write(
        &edits,
        r#"[
            {"op": "add_calendar", "name": "Winter", "copy_of": "Synthetic weekdays"},
            {"op": "add_calendar_exception", "calendar": "Winter", "date": "2026-01-12"},
            {"op": "set_calendar", "activity": "A1", "calendar": "Winter"}
        ]"#,
    )
    .unwrap();
    let saved = output.to_str().unwrap();
    let (code, stdout, stderr) = run(&[
        "edit",
        FS_LINK,
        "--edits",
        edits.to_str().unwrap(),
        "-o",
        saved,
    ]);
    assert_eq!(code, Some(0), "{stdout}{stderr}");
    let (code, stdout, _) = run(&["schedule", saved, "--json"]);
    assert_eq!(code, Some(0));
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(doc["projects"][0]["changed_activities"], 0, "{doc}");
    let (_, stdout, _) = run(&["query", saved, "--text", "A1", "--columns", "calendar"]);
    assert!(stdout.contains("Winter"), "{stdout}");
    let (code, stdout, _) = run(&["calendar", saved, "Winter", "--json"]);
    assert_eq!(code, Some(0));
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(doc["kind"], "project");
    assert_eq!(doc["exceptions_total"], 1);
}

#[test]
fn a_code_is_written_read_back_and_removed() {
    let directory = tempfile::tempdir().unwrap();
    let first = directory.path().join("first.xer");
    let second = directory.path().join("second.xer");
    let (code, stdout, stderr) = run(&[
        "edit",
        CODED,
        "--add-code-value",
        "Synthetic phase=MORE;More work",
        "--code",
        "TF-002=Synthetic phase=MORE",
        "-o",
        first.to_str().unwrap(),
    ]);
    assert_eq!(code, Some(0), "{stdout}{stderr}");
    let (_, stdout, _) = run(&[
        "query",
        first.to_str().unwrap(),
        "--code",
        "Synthetic phase=MORE",
        "--columns",
        "code",
    ]);
    assert!(stdout.contains("TF-002"), "{stdout}");
    let (code, stdout, stderr) = run(&[
        "edit",
        first.to_str().unwrap(),
        "--clear-code",
        "TF-001=Synthetic phase",
        "-o",
        second.to_str().unwrap(),
    ]);
    assert_eq!(code, Some(0), "{stdout}{stderr}");
    let (_, stdout, _) = run(&[
        "query",
        second.to_str().unwrap(),
        "--columns",
        "code,codes",
        "--json",
    ]);
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(doc["rows"][0]["codes"], "");
    assert_eq!(doc["rows"][1]["codes"], "Synthetic phase=MORE");
    let (code, stdout, _) = run(&["schedule", second.to_str().unwrap(), "--json"]);
    assert_eq!(code, Some(0));
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(doc["projects"][0]["changed_activities"], 0, "{doc}");
}

#[test]
fn a_changed_code_rewrites_only_that_cell() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("out.xer");
    let (code, ..) = run(&[
        "edit",
        CODED,
        "--add-code-value",
        "Synthetic phase=MORE",
        "--code",
        "TF-001=Synthetic phase=MORE",
        "-o",
        output.to_str().unwrap(),
    ]);
    assert_eq!(code, Some(0));
    let bytes = std::fs::read(&output).unwrap();
    let text = String::from_utf8_lossy(&bytes);
    let task_codes = text
        .split("%T\tTASKACTV")
        .nth(1)
        .and_then(|rest| rest.split("%T").next())
        .unwrap();
    assert_eq!(task_codes.matches("%R").count(), 1, "{task_codes}");
}

#[test]
fn a_resource_is_assigned_saved_and_removed() {
    let directory = tempfile::tempdir().unwrap();
    let first = directory.path().join("first.xer");
    let second = directory.path().join("second.xer");
    let (_, stdout, _) = run(&["query", CODED, "--columns", "wbs", "--limit", "1", "--json"]);
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let new = format!("N1;Pour;{};40h", doc["rows"][0]["wbs"].as_str().unwrap());
    let (code, stdout, stderr) = run(&[
        "edit",
        CODED,
        "--add",
        &new,
        "--assign",
        "N1=TF-LABOR",
        "-o",
        first.to_str().unwrap(),
    ]);
    assert_eq!(code, Some(0), "{stdout}{stderr}");
    let bytes = std::fs::read(&first).unwrap();
    let text = String::from_utf8_lossy(&bytes);
    assert!(
        text.contains("COST_PER_QTY"),
        "new rows hold the usual settings"
    );
    let (code, stdout, stderr) = run(&[
        "edit",
        first.to_str().unwrap(),
        "--unassign",
        "TF-001=TF-LABOR",
        "-o",
        second.to_str().unwrap(),
    ]);
    assert_eq!(code, Some(0), "{stdout}{stderr}");
    let (_, stdout, _) = run(&[
        "query",
        second.to_str().unwrap(),
        "--resource",
        "TF-LABOR",
        "--columns",
        "code",
        "--json",
    ]);
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(doc["total"], 1);
    assert_eq!(doc["rows"][0]["code"], "N1");
    let (code, stdout, _) = run(&["schedule", second.to_str().unwrap(), "--json"]);
    assert_eq!(code, Some(0));
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(doc["projects"][0]["changed_activities"], 0, "{doc}");
}

#[test]
fn a_milestone_takes_no_resources() {
    let (code, _, stderr) = run(&["edit", CODED, "--assign", "TF-002=TF-LABOR"]);
    assert_eq!(code, Some(1));
    assert!(stderr.contains("no-resources-here"), "{stderr}");
}

/// Project 1 holds R1 and project 2 holds R2, so project 2 borrows R1.
#[test]
fn a_project_can_take_up_a_resource_another_project_holds() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("out.xer");
    let saved = output.to_str().unwrap();
    let (code, stdout, stderr) = run(&[
        "edit",
        FIXTURE,
        "--assign",
        "B1=R1",
        "--project",
        "2",
        "-o",
        saved,
    ]);
    assert_eq!(code, Some(0), "{stdout}{stderr}");
    let (_, stdout, _) = run(&["query", saved, "--text", "B1", "--columns", "resources"]);
    assert!(stdout.contains("R1"), "{stdout}");
}

#[test]
fn a_new_wbs_node_takes_new_work_and_the_file_settles() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("out.xer");
    let saved = output.to_str().unwrap();
    let (_, stdout, _) = run(&[
        "query",
        FS_LINK,
        "--columns",
        "wbs",
        "--limit",
        "1",
        "--json",
    ]);
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let root = doc["rows"][0]["wbs"].as_str().unwrap().to_owned();
    let (node, new) = (format!("CIV;Civil Works;{root}"), "N1;Pour;CIV;40h");
    let (code, stdout, stderr) = run(&[
        "edit",
        FS_LINK,
        "--add-wbs",
        &node,
        "--add",
        new,
        "--wbs",
        "A2=CIV",
        "-o",
        saved,
    ]);
    assert_eq!(code, Some(0), "{stdout}{stderr}");
    let (_, stdout, _) = run(&[
        "query",
        saved,
        "--wbs",
        "CIV",
        "--columns",
        "code",
        "--json",
    ]);
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(doc["total"], 2, "{doc}");
    let (code, stdout, _) = run(&["schedule", saved, "--json"]);
    assert_eq!(code, Some(0));
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(doc["projects"][0]["changed_activities"], 0, "{doc}");
    let bytes = std::fs::read(&output).unwrap();
    let text = String::from_utf8_lossy(&bytes);
    let row = text.lines().find(|l| l.contains("\tCIV\t")).unwrap();
    assert!(row.contains("Civil Works"), "{row}");
    let (code, stdout, _) = run(&["roundtrip", saved]);
    assert_eq!(code, Some(0), "{stdout}");
}

#[test]
fn a_renamed_wbs_node_changes_one_row() {
    let directory = tempfile::tempdir().unwrap();
    let first = directory.path().join("first.xer");
    let second = directory.path().join("second.xer");
    let (_, stdout, _) = run(&[
        "query",
        FS_LINK,
        "--columns",
        "wbs",
        "--limit",
        "1",
        "--json",
    ]);
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let root = doc["rows"][0]["wbs"].as_str().unwrap().to_owned();
    let node = format!("CIV;Civil Works;{root}");
    run(&[
        "edit",
        FS_LINK,
        "--add-wbs",
        &node,
        "-o",
        first.to_str().unwrap(),
    ]);
    let (code, stdout, stderr) = run(&[
        "edit",
        first.to_str().unwrap(),
        "--rename-wbs",
        "CIV=Site Civil",
        "-o",
        second.to_str().unwrap(),
    ]);
    assert_eq!(code, Some(0), "{stdout}{stderr}");
    let lines = |path: &std::path::Path| {
        let bytes = std::fs::read(path).unwrap();
        bytes
            .split(|b| *b == b'\n')
            .map(<[u8]>::to_vec)
            .collect::<Vec<_>>()
    };
    let (before, after) = (lines(&first), lines(&second));
    assert_eq!(before.len(), after.len());
    let different = before.iter().zip(&after).filter(|(a, b)| a != b).count();
    assert_eq!(different, 1);
}

#[test]
fn a_wbs_node_can_be_built_on_in_the_same_call() {
    let directory = tempfile::tempdir().unwrap();
    let edits = directory.path().join("edits.json");
    let output = directory.path().join("out.xer");
    let saved = output.to_str().unwrap();
    let (_, stdout, _) = run(&[
        "query",
        FS_LINK,
        "--columns",
        "wbs",
        "--limit",
        "1",
        "--json",
    ]);
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let root = doc["rows"][0]["wbs"].as_str().unwrap().to_owned();
    let text = format!(
        r#"[
            {{"op": "add_wbs", "code": "PH1", "name": "Phase 1", "parent": "{root}"}},
            {{"op": "add_wbs", "code": "CIV", "name": "Civil", "parent": "PH1"}},
            {{"op": "rename_wbs", "wbs": "CIV", "code": "SITE"}},
            {{"op": "add_wbs", "code": "PAD", "name": "Pad", "parent": "SITE"}}
        ]"#
    );
    std::fs::write(&edits, text).unwrap();
    let (code, stdout, stderr) = run(&[
        "edit",
        FS_LINK,
        "--edits",
        edits.to_str().unwrap(),
        "-o",
        saved,
    ]);
    assert_eq!(code, Some(0), "{stdout}{stderr}");
    let bytes = std::fs::read(&output).unwrap();
    let written = String::from_utf8_lossy(&bytes);
    let row = written.lines().find(|l| l.contains("\tPAD\t")).unwrap();
    assert!(row.contains("Pad"), "{row}");
}

#[test]
fn a_check_that_newly_fails_stops_the_save() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("out.xer");
    let saved = output.to_str().unwrap();
    let (code, stdout, stderr) = run(&[
        "edit", FS_LINK, "--unlink", "A1>A2", "--check", "-o", saved, "--json",
    ]);
    assert_eq!(code, Some(3), "{stdout}{stderr}");
    assert!(!output.exists());
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(doc["output"], serde_json::Value::Null);
    let failing = doc["projects"][0]["checks"]["newly_failing"]
        .as_array()
        .unwrap();
    assert!(
        failing.iter().any(|c| c["id"] == "open-ends"),
        "{failing:?}"
    );
    let (code, ..) = run(&["edit", FS_LINK, "--unlink", "A1>A2", "-o", saved]);
    assert_eq!(code, Some(0), "without --check it saves");
}

#[test]
fn a_harmless_edit_passes_the_check_and_saves() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("out.xer");
    let saved = output.to_str().unwrap();
    let (code, stdout, stderr) = run(&[
        "edit",
        FS_LINK,
        "--name",
        "A1=Renamed",
        "--check",
        "-o",
        saved,
    ]);
    assert_eq!(code, Some(0), "{stdout}{stderr}");
    assert!(output.exists());
}

fn new_args(output: &str) -> Vec<&str> {
    vec![
        "new",
        "--code",
        "PLANT",
        "--name",
        "New Plant",
        "--data-date",
        "2026-03-02 08:00",
        "-o",
        output,
    ]
}

#[test]
fn a_new_schedule_is_a_file_every_command_can_read() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("out.xer");
    let saved = output.to_str().unwrap();
    let (code, stdout, stderr) = run(&new_args(saved));
    assert_eq!(code, Some(0), "{stdout}{stderr}");
    let (code, stdout, _) = run(&["inspect", saved, "--json"]);
    assert_eq!(code, Some(0));
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(doc["projects"][0]["short_name"], "PLANT");
    let (code, stdout, _) = run(&["roundtrip", saved]);
    assert_eq!(code, Some(0), "{stdout}");
    let (code, _, stderr) = run(&new_args(saved));
    assert_eq!(code, Some(1));
    assert!(stderr.contains("output-exists"), "{stderr}");
}

#[test]
fn a_built_schedule_calculates_settles_and_is_checked() {
    let directory = tempfile::tempdir().unwrap();
    let edits = directory.path().join("edits.json");
    let output = directory.path().join("out.xer");
    let saved = output.to_str().unwrap();
    std::fs::write(
        &edits,
        r#"[
            {"op": "add_wbs", "code": "CIV", "name": "Civil", "parent": "PLANT"},
            {"op": "add_activity", "code": "M1", "name": "Start", "wbs": "PLANT",
             "activity_type": "start-milestone"},
            {"op": "add_activity", "code": "A1", "name": "Excavate", "wbs": "CIV", "duration": "5d"},
            {"op": "add_activity", "code": "A2", "name": "Pour", "wbs": "CIV", "duration": "10d"},
            {"op": "add_activity", "code": "M2", "name": "Finish", "wbs": "PLANT",
             "activity_type": "finish-milestone"},
            {"op": "add_link", "predecessor": "M1", "successor": "A1"},
            {"op": "add_link", "predecessor": "A1", "successor": "A2"},
            {"op": "add_link", "predecessor": "A2", "successor": "M2"}
        ]"#,
    )
    .unwrap();
    let mut args = new_args(saved);
    args.extend(["--edits", edits.to_str().unwrap(), "--json"]);
    let (code, stdout, stderr) = run(&args);
    assert_eq!(code, Some(0), "{stdout}{stderr}");
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(doc["schema"], "totalfloat.new/1");
    assert_eq!(doc["activities"], 4);
    assert_eq!(doc["relationships"], 3);
    assert_eq!(doc["finish"], "2026-03-20 17:00");
    let (code, stdout, _) = run(&["schedule", saved, "--json"]);
    assert_eq!(code, Some(0));
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(doc["projects"][0]["changed_activities"], 0, "{doc}");
    // Four activities are too few for DCMA's percentages: the first and last are
    // half of them, so Logic fails. Everything else about the network is sound.
    let (code, stdout, stderr) = run(&["check", saved, "--json"]);
    assert_eq!(code, Some(3), "{stderr}");
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let checks = doc["projects"][0]["checks"].as_array().unwrap();
    let outcome = |id: &str| {
        let check = checks.iter().find(|c| c["id"] == id).unwrap();
        check["outcome"].clone()
    };
    assert_eq!(outcome("open-ends"), "pass");
    assert_eq!(outcome("dcma-01-logic"), "fail");
    assert_eq!(outcome("dcma-12-critical-path-test"), "pass");
}

#[test]
fn the_same_inputs_make_the_same_file() {
    let directory = tempfile::tempdir().unwrap();
    let (first, second) = (
        directory.path().join("a.xer"),
        directory.path().join("b.xer"),
    );
    run(&new_args(first.to_str().unwrap()));
    run(&new_args(second.to_str().unwrap()));
    assert_eq!(
        std::fs::read(first).unwrap(),
        std::fs::read(second).unwrap()
    );
}

#[test]
fn schedules_sharing_a_code_get_their_own_guids() {
    let directory = tempfile::tempdir().unwrap();
    let (first, second) = (
        directory.path().join("a.xer"),
        directory.path().join("b.xer"),
    );
    run(&new_args(first.to_str().unwrap()));
    let path = second.to_str().unwrap();
    let mut other = new_args(path);
    other[4] = "Another Plant";
    run(&other);
    let guid = |file: &std::path::Path| {
        let text = std::fs::read_to_string(file).unwrap();
        let mut lines = text.lines().skip_while(|l| !l.starts_with("%T\tPROJECT"));
        let fields: Vec<&str> = lines.nth(1).unwrap().split('\t').collect();
        let row: Vec<&str> = lines.next().unwrap().split('\t').collect();
        let at = fields.iter().position(|f| *f == "guid").unwrap();
        row[at].to_owned()
    };
    assert_ne!(guid(&first), guid(&second));
}

/// A file a new schedule writes is as readable as any other new file.
#[cfg(unix)]
#[test]
fn a_new_schedule_follows_the_usual_file_permissions() {
    use std::os::unix::fs::PermissionsExt as _;
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("out.xer");
    run(&new_args(output.to_str().unwrap()));
    let mode = std::fs::metadata(&output).unwrap().permissions().mode();
    assert_eq!(mode & 0o400, 0o400);
    assert_eq!(mode & 0o111, 0, "a schedule isn't executable");
    assert_ne!(mode & 0o777, 0o600, "not a private temporary file");
}

#[test]
fn a_new_schedule_refuses_bad_input() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("out.xer");
    let mut blank = new_args(output.to_str().unwrap());
    blank[2] = " ";
    let (code, _, stderr) = run(&blank);
    assert_eq!(code, Some(1));
    assert!(stderr.contains("invalid-wbs-code"), "{stderr}");
    let long = "x".repeat(101);
    let mut too_long = new_args(output.to_str().unwrap());
    too_long[4] = &long;
    let (code, _, stderr) = run(&too_long);
    assert_eq!(code, Some(1));
    assert!(stderr.contains("name-too-long"), "{stderr}");
    let mut backwards = new_args(output.to_str().unwrap());
    backwards.extend(["--must-finish", "2020-01-01 17:00"]);
    let (code, _, stderr) = run(&backwards);
    assert_eq!(code, Some(1));
    assert!(stderr.contains("invalid-new-schedule"), "{stderr}");
    let mut undated = new_args(output.to_str().unwrap());
    undated[6] = "soon";
    let (code, _, stderr) = run(&undated);
    assert_eq!(code, Some(1));
    assert!(stderr.contains("invalid-date"), "{stderr}");
    assert!(!output.exists());
}

const PLAN: &str = "../../fixtures/msproject/plant.xml";

#[test]
fn an_import_calculates_settles_and_says_what_it_lost() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("plan.xer");
    let saved = output.to_str().unwrap();
    let (code, stdout, stderr) = run(&["import", PLAN, "-o", saved, "--json"]);
    assert_eq!(code, Some(0), "{stdout}{stderr}");
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(doc["calculated"], true, "{doc}");
    assert_eq!(doc["activities"], 5);
    assert_eq!(doc["relationships"], 4);
    assert_eq!(doc["wbs_nodes"], 3);
    let kinds: Vec<&str> = doc["notes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["kind"].as_str().unwrap())
        .collect();
    assert!(kinds.contains(&"summary-logic"), "{kinds:?}");
    assert!(kinds.contains(&"manual-task"), "{kinds:?}");
    let (code, stdout, _) = run(&["schedule", saved, "--json"]);
    assert_eq!(code, Some(0));
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(doc["projects"][0]["changed_activities"], 0, "{doc}");
    let (code, stdout, _) = run(&["roundtrip", saved]);
    assert_eq!(code, Some(0), "{stdout}");
    let (code, stdout, stderr) = run(&["check", saved, "--json"]);
    assert_ne!(code, Some(1), "{stdout}{stderr}");
}

#[test]
fn import_human() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("plan.xer");
    let (_, stdout, _) = run(&["import", PLAN, "-o", output.to_str().unwrap()]);
    let stdout = stdout.replace(output.to_str().unwrap(), "plan.xer");
    insta::assert_snapshot!(stdout);
}

#[test]
fn an_import_refuses_what_it_can_t_read() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("plan.xer");
    let saved = output.to_str().unwrap();
    let (code, _, stderr) = run(&["import", FIXTURE, "-o", saved]);
    assert_eq!(code, Some(1));
    assert!(stderr.contains("import-refused"), "{stderr}");
    assert!(!output.exists());
    let binary = directory.path().join("plan.mpp");
    std::fs::write(&binary, [0xd0, 0xcf, 0x11, 0xe0, 0xff]).unwrap();
    let (code, _, stderr) = run(&["import", binary.to_str().unwrap(), "-o", saved]);
    assert_eq!(code, Some(1));
    assert!(stderr.contains(".mpp"), "{stderr}");
}

/// Ten elapsed days run straight through a weekend, on the calendar the import adds.
#[test]
fn elapsed_work_keeps_its_length_through_an_import() {
    let directory = tempfile::tempdir().unwrap();
    let (plan, output) = (
        directory.path().join("plan.xml"),
        directory.path().join("out.xer"),
    );
    std::fs::write(
        &plan,
        "<Project><Title>T</Title><StartDate>2026-03-02T08:00:00</StartDate>\
         <CalendarUID>1</CalendarUID><Calendars><Calendar><UID>1</UID><Name>S</Name>\
         <IsBaseCalendar>1</IsBaseCalendar></Calendar></Calendars><Tasks>\
         <Task><UID>1</UID><Name>Cure</Name><Duration>PT240H0M0S</Duration>\
         <DurationFormat>8</DurationFormat></Task></Tasks></Project>",
    )
    .unwrap();
    let saved = output.to_str().unwrap();
    let (code, stdout, stderr) = run(&["import", plan.to_str().unwrap(), "-o", saved, "--json"]);
    assert_eq!(code, Some(0), "{stdout}{stderr}");
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(doc["finish"], "2026-03-12 08:00", "{doc}");
    let (code, stdout, _) = run(&["schedule", saved, "--json"]);
    assert_eq!(code, Some(0));
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(doc["projects"][0]["changed_activities"], 0, "{doc}");
}

#[test]
fn an_export_opens_again_as_the_same_schedule() {
    let directory = tempfile::tempdir().unwrap();
    let (first, xml, second) = (
        directory.path().join("first.xer"),
        directory.path().join("plan.xml"),
        directory.path().join("second.xer"),
    );
    let (one, xml_path, two) = (
        first.to_str().unwrap(),
        xml.to_str().unwrap(),
        second.to_str().unwrap(),
    );
    run(&["import", PLAN, "-o", one]);
    let (code, stdout, stderr) =
        run(&["export", one, "--to", "msproject", "-o", xml_path, "--json"]);
    assert_eq!(code, Some(0), "{stdout}{stderr}");
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(doc["schema"], "totalfloat.export/1");
    let (code, stdout, stderr) = run(&["import", xml_path, "-o", two, "--json"]);
    assert_eq!(code, Some(0), "{stdout}{stderr}");
    let columns = "name,early_start,early_finish,total_float_hours";
    // By row, sorted: nothing holds the order of a node's work against its child nodes.
    let read = |file| {
        let stdout = run(&["query", file, "--columns", columns]).1;
        let mut rows: Vec<String> = stdout.lines().map(str::to_owned).collect();
        rows.sort();
        rows
    };
    assert_eq!(read(one), read(two));
}

#[test]
fn an_export_to_dot_draws_the_network_with_the_critical_path() {
    let directory = tempfile::tempdir().unwrap();
    let scheduled = directory.path().join("scheduled.xer");
    let scheduled = scheduled.to_str().unwrap();
    run(&["schedule", LINKED, "-o", scheduled]);
    let path = directory.path().join("network.dot");
    let path = path.to_str().unwrap();
    let (code, stdout, stderr) = run(&["export", scheduled, "--to", "dot", "-o", path, "--json"]);
    assert_eq!(code, Some(0), "{stdout}{stderr}");
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(doc["format"], "dot");
    assert_eq!(doc["notes"], serde_json::json!([]));
    let dot = std::fs::read_to_string(path).unwrap();
    assert!(dot.starts_with("digraph \"rel-fs-positive\" {\n"), "{dot}");
    assert!(
        dot.contains("  a1 -> a2 [label=\"FS +1.0h\", color=\"#c0392b\", penwidth=2];\n"),
        "{dot}"
    );
    assert!(dot.contains("TF 0.0h"), "{dot}");
    assert!(dot.ends_with("}\n"), "{dot}");
    // The same file gives the same network, and an existing path is refused.
    let again = directory.path().join("again.dot");
    run(&[
        "export",
        scheduled,
        "--to",
        "dot",
        "-o",
        again.to_str().unwrap(),
    ]);
    assert_eq!(std::fs::read_to_string(again).unwrap(), dot);
    let (code, _, stderr) = run(&["export", scheduled, "--to", "dot", "-o", path]);
    assert_eq!(code, Some(1));
    assert!(stderr.contains("output-exists"), "{stderr}");
}

#[test]
fn an_export_to_dot_says_when_the_file_holds_no_dates() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("network.dot");
    let (code, stdout, _) = run(&[
        "export",
        LINKED,
        "--to",
        "dot",
        "-o",
        path.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(code, Some(0));
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(doc["notes"][0]["kind"], "no-dates");
    assert_eq!(doc["notes"][0]["count"], 2);
}

#[test]
fn an_export_to_dot_names_links_to_other_projects_it_leaves_out() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("network.dot");
    let path = path.to_str().unwrap();
    let (code, stdout, stderr) = run(&[
        "export",
        TWO_PROJECTS,
        "--to",
        "dot",
        "--project",
        "1",
        "-o",
        path,
        "--json",
    ]);
    assert_eq!(code, Some(0), "{stderr}");
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(doc["notes"][0]["kind"], "external-links");
    assert_eq!(doc["notes"][0]["examples"], serde_json::json!(["A1 to B1"]));
    // Its link within the project is drawn.
    assert_eq!(
        std::fs::read_to_string(path).unwrap().matches("->").count(),
        1
    );
}

#[test]
fn an_export_refuses_a_project_with_no_calendars() {
    let directory = tempfile::tempdir().unwrap();
    let (held, xml) = (
        directory.path().join("in.xer"),
        directory.path().join("plan.xml"),
    );
    let text = std::fs::read_to_string(FIXTURE).unwrap();
    let mut keep = true;
    let lines: Vec<&str> = text
        .lines()
        .filter(|line| {
            if line.starts_with("%T") {
                keep = *line != "%T\tCALENDAR";
            }
            keep
        })
        .collect();
    std::fs::write(&held, lines.join("\n")).unwrap();
    let (input, output) = (held.to_str().unwrap(), xml.to_str().unwrap());
    let args = [
        "export",
        input,
        "--to",
        "msproject",
        "-o",
        output,
        "--project",
        "2",
    ];
    let (code, stdout, stderr) = run(&args);
    assert_eq!(code, Some(1), "{stdout}{stderr}");
    assert!(stderr.contains("no-calendars"), "{stderr}");
    assert!(!xml.exists());
}

#[test]
fn an_export_of_several_projects_needs_one_named() {
    let directory = tempfile::tempdir().unwrap();
    let xml = directory.path().join("plan.xml");
    let path = xml.to_str().unwrap();
    let (code, _, stderr) = run(&["export", FIXTURE, "--to", "msproject", "-o", path]);
    assert_eq!(code, Some(1));
    assert!(stderr.contains("needs-project"), "{stderr}");
    assert!(!xml.exists());
    let (code, stdout, stderr) = run(&[
        "export",
        FIXTURE,
        "--to",
        "msproject",
        "-o",
        path,
        "--project",
        "1",
    ]);
    assert_eq!(code, Some(0), "{stdout}{stderr}");
    let text = std::fs::read_to_string(&xml).unwrap();
    assert!(
        text.contains("<Title>Synthetic Project One</Title>"),
        "{text}"
    );
    assert!(!text.contains("Synthetic Project Two"), "{text}");
}

/// The cells of a file's only `TASKRSRC` row, by column.
fn assignment_cells(file: &str) -> std::collections::BTreeMap<String, String> {
    let text = std::fs::read_to_string(file).unwrap();
    let mut lines = text
        .lines()
        .skip_while(|line| *line != "%T\tTASKRSRC")
        .skip(1);
    let fields: Vec<&str> = lines.next().unwrap().split('\t').skip(1).collect();
    let row: Vec<&str> = lines.next().unwrap().split('\t').skip(1).collect();
    fields
        .into_iter()
        .zip(row)
        .map(|(f, v)| (f.to_owned(), v.to_owned()))
        .collect()
}

#[test]
fn units_go_on_with_an_assignment_and_follow_its_duration() {
    let directory = tempfile::tempdir().unwrap();
    let staffed = directory.path().join("staffed.xer");
    let longer = directory.path().join("longer.xer");
    let source = "../../fixtures/xer/fresh-writer.xer";
    let edits = directory.path().join("edits.json");
    std::fs::write(
        &edits,
        r#"[{"op": "unassign_resource", "activity": "TF-001", "resource": "TF-LABOR"},
            {"op": "assign_resource", "activity": "TF-001", "resource": "TF-LABOR", "units": 16}]"#,
    )
    .unwrap();
    let (code, stdout, stderr) = run(&[
        "edit",
        source,
        "--edits",
        edits.to_str().unwrap(),
        "-o",
        staffed.to_str().unwrap(),
    ]);
    assert_eq!(code, Some(0), "{stdout}{stderr}");
    assert!(stdout.contains("16 units at 2 an hour"), "{stdout}");
    let cells = assignment_cells(staffed.to_str().unwrap());
    assert_eq!(
        (
            cells["remain_qty"].as_str(),
            cells["remain_qty_per_hr"].as_str()
        ),
        ("16", "2")
    );
    // The activity's labor totals move with its assignment.
    let text = std::fs::read_to_string(&staffed).unwrap();
    let mut task = text.lines().skip_while(|line| *line != "%T\tTASK").skip(1);
    let fields: Vec<&str> = task.next().unwrap().split('\t').skip(1).collect();
    let first: Vec<&str> = task.next().unwrap().split('\t').skip(1).collect();
    let cell = |name: &str| first[fields.iter().position(|f| *f == name).unwrap()];
    assert_eq!(
        (cell("target_work_qty"), cell("remain_work_qty")),
        ("16", "16")
    );
    // Fixed duration and units: the units stay, and the rate halves.
    let (code, stdout, stderr) = run(&[
        "edit",
        staffed.to_str().unwrap(),
        "--duration",
        "TF-001=16h",
        "-o",
        longer.to_str().unwrap(),
    ]);
    assert_eq!(code, Some(0), "{stdout}{stderr}");
    assert!(
        stdout
            .contains("TF-LABOR: budgeted units per hour 2 to 1, remaining units per hour 2 to 1"),
        "{stdout}"
    );
    let cells = assignment_cells(longer.to_str().unwrap());
    assert_eq!(
        (
            cells["remain_qty"].as_str(),
            cells["remain_qty_per_hr"].as_str()
        ),
        ("16", "1")
    );
    assert_eq!(cells["target_qty"], "16");
}

#[test]
fn progress_as_planned_settles_and_needs_a_later_data_date() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("progressed.xer");
    let path = output.to_str().unwrap();
    let (code, stdout, stderr) = run(&[
        "edit",
        LINKED,
        "--data-date",
        "2026-01-13 12:00",
        "--as-planned",
        "-o",
        path,
        "--json",
    ]);
    assert_eq!(code, Some(0), "{stdout}{stderr}");
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let applied = doc["applied"].as_array().unwrap();
    assert!(
        applied[0]
            .as_str()
            .unwrap()
            .contains("2 started, 1 finished"),
        "{doc}"
    );
    let columns = "code,status,actual_start,actual_finish,remaining_duration_hours";
    let (_, table, _) = run(&["query", path, "--columns", columns]);
    assert!(
        table.contains("A1    completed    2026-01-12 08:00  2026-01-12 17:00"),
        "{table}"
    );
    assert!(
        table.contains("A2    in-progress  2026-01-13 09:00  -"),
        "{table}"
    );
    // Progress as planned leaves nothing for a recalculation to move.
    let (code, stdout, _) = run(&["schedule", path, "--json"]);
    assert_eq!(code, Some(0));
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(doc["projects"][0]["changed_dates_or_float"], 0, "{doc}");
    let earlier = [
        "edit",
        LINKED,
        "--data-date",
        "2026-01-07 12:00",
        "--as-planned",
    ];
    let (code, _, stderr) = run(&earlier);
    assert_eq!(code, Some(1));
    assert!(stderr.contains("data-date-not-later"), "{stderr}");
    // The data date can come from an --edits document, so this is checked
    // after reading it rather than by the argument parser.
    let (code, _, stderr) = run(&["edit", LINKED, "--as-planned"]);
    assert_eq!(code, Some(1), "{stderr}");
    assert!(stderr.contains("needs a new data date"), "{stderr}");
    // A correction says what really happened: A1 is still going.
    let (code, stdout, stderr) = run(&[
        "edit",
        LINKED,
        "--data-date",
        "2026-01-13 12:00",
        "--as-planned",
        "--actual-start",
        "A1=2026-01-12 08:00",
        "--remaining",
        "A1=4h",
    ]);
    assert_eq!(code, Some(0), "{stdout}{stderr}");
    assert!(
        stdout.contains("A1 is in progress, with 4h remaining"),
        "{stdout}"
    );
}

#[test]
fn a_baseline_shows_what_slipped_was_added_and_was_deleted() {
    let directory = tempfile::tempdir().unwrap();
    let (base, current) = (
        directory.path().join("base.xer"),
        directory.path().join("now.xer"),
    );
    let (base, current) = (base.to_str().unwrap(), current.to_str().unwrap());
    let (code, ..) = run(&["schedule", LINKED, "-o", base]);
    assert_eq!(code, Some(0));
    // A1 grows from one day to six, A2 goes and A3 comes.
    let (code, stdout, stderr) = run(&[
        "edit",
        base,
        "--duration",
        "A1=48h",
        "--delete",
        "A2",
        "--add",
        "A3;New work;rel-fs-positive;8h",
        "-o",
        current,
    ]);
    assert_eq!(code, Some(0), "{stdout}{stderr}");
    let columns = "code,baseline_start,baseline_finish,baseline_duration_hours,start_variance_hours,finish_variance_hours,duration_variance_hours";
    let (code, stdout, stderr) = run(&[
        "query",
        current,
        "--baseline",
        base,
        "--columns",
        columns,
        "--json",
    ]);
    assert_eq!(code, Some(0), "{stderr}");
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let row = |code: &str| {
        doc["rows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["code"] == code)
            .unwrap()
            .clone()
    };
    let slipped = row("A1");
    assert_eq!(slipped["baseline_finish"], "2026-01-12 17:00");
    assert_eq!(slipped["start_variance_hours"], 0.0);
    // Monday to the next Monday is a week, but only 40 working hours.
    assert_eq!(slipped["finish_variance_hours"], -40.0);
    assert_eq!(slipped["duration_variance_hours"], -40.0);
    let added = row("A3");
    assert!(added["baseline_finish"].is_null() && added["finish_variance_hours"].is_null());
    let summary = &doc["baseline"];
    assert_eq!(
        (summary["matched"].as_u64(), summary["only_here"].as_u64()),
        (Some(1), Some(1))
    );
    assert_eq!(summary["only_in_baseline"], 1);
    let (code, _, stderr) = run(&["query", current, "--columns", "finish_variance_hours"]);
    assert_eq!(code, Some(1));
    assert!(stderr.contains("needs-baseline"), "{stderr}");
    let (code, _, stderr) = run(&["query", current, "--baseline-project", "1"]);
    assert_eq!(code, Some(2), "{stderr}");
}

/// Work underway measures the same on both sides, so a file is its own baseline.
#[test]
fn a_file_compared_with_itself_has_no_variance() {
    let directory = tempfile::tempdir().unwrap();
    let underway = directory.path().join("underway.xer");
    let path = underway.to_str().unwrap();
    let args = [
        "edit",
        LINKED,
        "--data-date",
        "2026-01-13 12:00",
        "--as-planned",
        "-o",
        path,
    ];
    let (code, stdout, stderr) = run(&args);
    assert_eq!(code, Some(0), "{stdout}{stderr}");
    let columns = "code,status,start_variance_hours,finish_variance_hours,duration_variance_hours";
    let (code, stdout, stderr) = run(&[
        "query",
        path,
        "--baseline",
        path,
        "--columns",
        columns,
        "--json",
    ]);
    assert_eq!(code, Some(0), "{stderr}");
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let rows = doc["rows"].as_array().unwrap();
    assert!(rows.iter().any(|r| r["status"] == "in-progress"), "{doc}");
    for row in rows {
        for column in [
            "start_variance_hours",
            "finish_variance_hours",
            "duration_variance_hours",
        ] {
            assert_eq!(row[column], 0.0, "{row}");
        }
    }
}

#[test]
fn a_status_table_becomes_progress_or_names_every_bad_row() {
    let directory = tempfile::tempdir().unwrap();
    let table = directory.path().join("status.csv");
    let output = directory.path().join("updated.xer");
    std::fs::write(
        &table,
        "activity,actual_start,actual_finish,remaining\n\"A1\",\"2026-01-12 08:00\",2026-01-12 17:00,\nA2,2026-01-13 08:00,,4h\n",
    )
    .unwrap();
    let (code, stdout, stderr) = run(&[
        "edit",
        LINKED,
        "--data-date",
        "2026-01-13 12:00",
        "--progress-table",
        table.to_str().unwrap(),
        "-o",
        output.to_str().unwrap(),
    ]);
    assert_eq!(code, Some(0), "{stdout}{stderr}");
    assert!(stdout.contains("A1 is completed"), "{stdout}");
    assert!(
        stdout.contains("A2 is in progress, with 4h remaining"),
        "{stdout}"
    );
    // A bad date on line 3 and an unknown activity on line 4: both are named,
    // and nothing is written.
    std::fs::write(
        &table,
        "activity,actual_start\nA1,2026-01-12 08:00\nA2,2026-13-40 08:00\nNOPE,2026-01-12 08:00\n",
    )
    .unwrap();
    let refused = directory.path().join("refused.xer");
    let (code, _, stderr) = run(&[
        "edit",
        LINKED,
        "--data-date",
        "2026-01-13 12:00",
        "--progress-table",
        table.to_str().unwrap(),
        "-o",
        refused.to_str().unwrap(),
    ]);
    assert_eq!(code, Some(1), "{stderr}");
    assert!(stderr.contains("edits-refused"), "{stderr}");
    assert!(
        stderr.contains("line 3:") && stderr.contains("line 4:"),
        "{stderr}"
    );
    assert!(!stderr.contains("line 2:"), "{stderr}");
    assert!(!refused.exists());
}

/// `text` with the temporary `directory` and the separator after it written
/// as `<dir>/`, whether printed as is, in JSON or in Markdown, so snapshots
/// are the same on every OS. JSON escapes a Windows path's backslashes, and
/// Markdown escapes them and other markup characters such as `~`.
fn without_dir(text: &str, directory: &Path) -> String {
    let raw = format!("{}{}", directory.display(), std::path::MAIN_SEPARATOR);
    let json = raw.replace('\\', "\\\\");
    let markdown: String = raw
        .chars()
        .flat_map(|c| {
            let escaped = "\\`*_[]|~#<>&".contains(c);
            escaped.then_some('\\').into_iter().chain([c])
        })
        .collect();
    [markdown, json, raw]
        .iter()
        .fold(text.to_owned(), |text, form| text.replace(form, "<dir>/"))
}

/// A baseline and a later update: A1 grows, A3 is added and drives A2.
fn baseline_and_update(directory: &Path) -> (String, String) {
    let base = directory.join("base.xer").to_str().unwrap().to_owned();
    let now = directory.join("now.xer").to_str().unwrap().to_owned();
    let (code, ..) = run(&["schedule", LINKED, "-o", &base]);
    assert_eq!(code, Some(0));
    let (code, stdout, stderr) = run(&[
        "edit",
        &base,
        "--duration",
        "A1=48h",
        "--add",
        "A3;New work;rel-fs-positive;8h",
        "--link",
        "A3>A2",
        "-o",
        &now,
    ]);
    assert_eq!(code, Some(0), "{stdout}{stderr}");
    (base, now)
}

#[test]
fn variance_measures_an_update_against_its_baseline() {
    let directory = tempfile::tempdir().unwrap();
    let (base, now) = baseline_and_update(directory.path());
    let clean = |text: &str| without_dir(text, directory.path());
    let (code, stdout, stderr) = run(&["variance", &now, "--baseline", &base]);
    assert_eq!(code, Some(0), "{stderr}");
    insta::assert_snapshot!("variance_human", clean(&stdout));
    let (code, stdout, stderr) = run(&["variance", &now, "--baseline", &base, "--json"]);
    assert_eq!(code, Some(0), "{stderr}");
    insta::assert_snapshot!("variance_json", clean(&stdout));
}

/// `variance --markdown` and `--html`: the same facts as the text
/// report, as tables; a page written twice is the same bytes, and `-o`
/// never replaces a file.
#[test]
fn variance_writes_markdown_and_a_page() {
    let directory = tempfile::tempdir().unwrap();
    let (base, now) = baseline_and_update(directory.path());
    let clean = |text: &str| without_dir(text, directory.path());
    let (code, stdout, stderr) = run(&["variance", &now, "--baseline", &base, "--markdown"]);
    assert_eq!(code, Some(0), "{stderr}");
    insta::assert_snapshot!("variance_markdown", clean(&stdout));
    let page = |name: &str| {
        let path = directory.path().join(name);
        let path = path.to_str().unwrap().to_owned();
        let (code, stdout, stderr) =
            run(&["variance", &now, "--baseline", &base, "--html", "-o", &path]);
        assert_eq!(code, Some(0), "{stderr}");
        assert_eq!(stdout, format!("Wrote {path}\n"));
        std::fs::read_to_string(path).unwrap()
    };
    let first = page("variance.html");
    assert_eq!(first, page("again.html"));
    insta::assert_snapshot!("variance_html", clean(&first));
    let (code, _, stderr) = run(&[
        "variance",
        &now,
        "--baseline",
        &base,
        "--html",
        "-o",
        directory.path().join("variance.html").to_str().unwrap(),
    ]);
    assert_eq!(code, Some(1));
    assert!(
        stderr.contains("output-exists") && stderr.contains("choose a new output path"),
        "{stderr}"
    );
    let (code, ..) = run(&[
        "variance",
        &now,
        "--baseline",
        &base,
        "--html",
        "--markdown",
    ]);
    assert_eq!(code, Some(2));
}

/// Activity names with markup characters are escaped in both pages and in
/// Markdown.
#[test]
fn pages_escape_activity_names() {
    let directory = tempfile::tempdir().unwrap();
    let (base, _) = baseline_and_update(directory.path());
    let named = directory.path().join("named.xer");
    let named = named.to_str().unwrap();
    let (code, stdout, stderr) = run(&[
        "edit",
        &base,
        "--duration",
        "A1=48h",
        "--name",
        "A2=<b>Tom's \"rig\" & crane</b>",
        "-o",
        named,
    ]);
    assert_eq!(code, Some(0), "{stdout}{stderr}");
    let escaped = "&lt;b&gt;Tom's &quot;rig&quot; &amp; crane&lt;/b&gt;";
    let (_, page, _) = run(&["variance", named, "--baseline", &base, "--html"]);
    assert!(page.contains(escaped), "{page}");
    assert!(!page.contains("<b>"), "{page}");
    let (_, markdown, _) = run(&["variance", named, "--baseline", &base, "--markdown"]);
    assert!(
        markdown.contains("\\<b\\>Tom's \"rig\" \\& crane\\</b\\>"),
        "{markdown}"
    );
    let (code, page, _) = run(&["check", named, "--html"]);
    assert_eq!(code, Some(3));
    assert!(page.contains(escaped), "{page}");
    assert!(!page.contains("<b>"), "{page}");
}

/// Every key the command prints is in the published schema, and the reverse.
#[test]
fn variance_json_matches_the_published_schema() {
    let directory = tempfile::tempdir().unwrap();
    let (base, now) = baseline_and_update(directory.path());
    let schema: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string("../../docs/schemas/variance-1.schema.json").unwrap(),
    )
    .unwrap();
    let (_, stdout, _) = run(&["variance", &now, "--baseline", &base, "--json"]);
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(doc["schema"], schema["$id"]);
    let keys = |value: &serde_json::Value| {
        let mut keys: Vec<String> = value.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        keys
    };
    assert_eq!(keys(&doc), keys(&schema["properties"]));
    let project = &schema["properties"]["projects"]["items"]["properties"];
    assert_eq!(keys(&doc["projects"][0]), keys(project));
    assert_eq!(
        keys(&doc["projects"][0]["counts"]),
        keys(&project["counts"]["properties"])
    );
    assert_eq!(
        keys(&doc["projects"][0]["slips"][0]),
        keys(&schema["$defs"]["moved"]["properties"])
    );
}

/// Two projects joined by a link, both with stored dates.
const TWO_PROJECTS: &str = "../../fixtures/xer/two-projects.xer";

/// The file's relationship rows, as `id successor predecessor type lag`.
fn link_rows(file: &str) -> Vec<String> {
    let text = std::fs::read_to_string(file).unwrap();
    let rows = text.lines().skip_while(|line| *line != "%T\tTASKPRED");
    rows.skip(2)
        .take_while(|line| line.starts_with("%R"))
        .map(|line| line.split('\t').skip(1).collect::<Vec<_>>().join(" "))
        .collect()
}

fn early_start(file: &str, activity: &str) -> String {
    let columns = "code,early_start";
    let stdout = run(&[
        "query",
        file,
        "--text",
        activity,
        "--columns",
        columns,
        "--json",
    ])
    .1;
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    doc["rows"][0]["early_start"].as_str().unwrap().to_owned()
}

#[test]
fn a_link_between_projects_is_one_row_and_both_recalculate() {
    let directory = tempfile::tempdir().unwrap();
    let file = TWO_PROJECTS;
    let output = directory.path().join("added.xer");
    let (code, stdout, stderr) = run(&[
        "edit",
        file,
        "--link",
        "SYN1:A1>SYN2:B1",
        "-o",
        output.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(code, Some(0), "{stdout}{stderr}");
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(doc["projects"].as_array().unwrap().len(), 2, "{doc}");
    let added = output.to_str().unwrap();
    assert_eq!(link_rows(added).len(), link_rows(file).len() + 1);
    assert!(early_start(added, "B1") > early_start(file, "B1"));
    // Each project read the other's new dates, so calculating again moves nothing.
    let (code, stdout, stderr) = run(&["schedule", added, "--json"]);
    assert_eq!(code, Some(0), "{stdout}{stderr}");
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    for project in doc["projects"].as_array().unwrap() {
        assert_eq!(project["changed_dates_or_float"], 0, "{project}");
    }
}

#[test]
fn a_link_between_projects_changes_and_goes_in_one_row() {
    let directory = tempfile::tempdir().unwrap();
    let file = TWO_PROJECTS;
    let save = |name: &str, flags: &[&str]| {
        let output = directory.path().join(name);
        let path = output.to_str().unwrap().to_owned();
        let mut args = vec!["edit", file];
        args.extend_from_slice(flags);
        args.extend_from_slice(&["-o", &path]);
        let (code, stdout, stderr) = run(&args);
        assert_eq!(code, Some(0), "{stdout}{stderr}");
        path
    };
    let changed = link_rows(&save("changed.xer", &["--relink", "SYN1:A1>SYN2:B1:FF-2h"]));
    assert_eq!(changed.len(), 2, "{changed:?}");
    assert!(
        changed.iter().any(|row| row.ends_with("PR_FF -2")),
        "{changed:?}"
    );
    let removed = link_rows(&save("removed.xer", &["--unlink", "SYN1:A1>SYN2:B1"]));
    assert_eq!(removed.len(), 1, "{removed:?}");
    let swapped = link_rows(&save(
        "swapped.xer",
        &[
            "--unlink",
            "SYN1:A1>SYN2:B1:SS",
            "--link",
            "SYN1:A1>SYN2:B1:FF",
        ],
    ));
    assert_eq!(swapped.len(), 2, "{swapped:?}");
    assert!(
        !swapped.iter().any(|row| row.contains("PR_SS")),
        "{swapped:?}"
    );
}

#[test]
fn a_link_leaves_its_project_only_by_saying_so() {
    let file = TWO_PROJECTS;
    let (code, _, stderr) = run(&["edit", file, "--link", "A1>B1"]);
    assert_eq!(code, Some(1));
    assert!(stderr.contains("PROJECT:CODE"), "{stderr}");
    let args = ["edit", file, "--project", "1", "--link", "SYN1:A1>SYN2:B1"];
    let (code, _, stderr) = run(&args);
    assert_eq!(code, Some(1));
    assert!(stderr.contains("link-leaves-project"), "{stderr}");
    let (code, _, stderr) = run(&["edit", file, "--link", "SYN2:B1>SYN1:A1"]);
    assert_eq!(code, Some(1));
    assert!(stderr.contains("circular-logic"), "{stderr}");
    let args = [
        "edit",
        file,
        "--project",
        "2",
        "--unlink",
        "SYN1:A1>SYN1:M1",
    ];
    let (code, _, stderr) = run(&args);
    assert_eq!(code, Some(1));
    assert!(stderr.contains("isn't in project 2"), "{stderr}");
}

/// Typing mistakes after a colon say what they are, not that nothing matched.
#[test]
fn a_bad_type_or_lag_on_a_link_is_named() {
    for (link, wanted) in [
        ("A1>A2:XX", "XX isn't a valid link type"),
        ("A1>A2:FS+5x", "5x isn't a valid span"),
    ] {
        let (code, _, stderr) = run(&["edit", FS_LINK, "--link", link]);
        assert_eq!(code, Some(1), "{link}");
        assert!(stderr.contains(wanted), "{link}: {stderr}");
    }
    let (code, _, stderr) = run(&[
        "edit",
        FS_LINK,
        "--add",
        "A9;New work;rel-fs-positive;1d",
        "--link",
        "A1>A9:XX",
    ]);
    assert_eq!(code, Some(1));
    assert!(stderr.contains("XX isn't a valid link type"), "{stderr}");
}

#[test]
fn both_ends_in_one_project_can_name_it() {
    let directory = tempfile::tempdir().unwrap();
    let file = TWO_PROJECTS;
    let output = directory.path().join("out.xer");
    let path = output.to_str().unwrap();
    let (code, stdout, stderr) = run(&["edit", file, "--unlink", "SYN1:A1>SYN1:M1", "-o", path]);
    assert_eq!(code, Some(0), "{stdout}{stderr}");
    assert_eq!(link_rows(path).len(), link_rows(file).len() - 1);
}

/// One duration edit on every file that schedules saves, reads back and settles.
#[test]
fn an_edit_settles_on_every_corpus_file_that_schedules() {
    let Some(root) = std::env::var_os("TOTALFLOAT_CORPUS") else {
        eprintln!("TOTALFLOAT_CORPUS unset; skipping edit corpus test");
        return;
    };
    let directory = tempfile::tempdir().unwrap();
    let mut files = Vec::new();
    for entry in std::fs::read_dir(std::path::Path::new(&root).join("files")).unwrap() {
        for file in std::fs::read_dir(entry.unwrap().path()).unwrap() {
            files.push(file.unwrap().path());
        }
    }
    files.sort();
    let (mut edited, mut refused) = (0, Vec::new());
    for (index, file) in files.iter().enumerate() {
        let name = file.to_str().unwrap();
        let (_, stdout, _) = run(&[
            "query",
            name,
            "--status",
            "not-started",
            "--type",
            "task",
            "--limit",
            "1",
            "--columns",
            "project,code,original_duration_hours",
            "--json",
        ]);
        let Ok(doc) = serde_json::from_str::<serde_json::Value>(&stdout) else {
            continue;
        };
        let Some(row) = doc["rows"].get(0) else {
            continue;
        };
        let project = row["project"].to_string();
        let code = row["code"].as_str().unwrap();
        let hours = row["original_duration_hours"].as_f64().unwrap() + 8.0;
        let duration = format!("{code}={}h", hours.round());
        let output = directory.path().join(format!("{index}.xer"));
        let (status, stdout, stderr) = run(&[
            "edit",
            name,
            "--project",
            &project,
            "--duration",
            &duration,
            "-o",
            output.to_str().unwrap(),
            "--json",
        ]);
        if status != Some(0) {
            let (scheduled, ..) = run(&["schedule", name, "--project", &project]);
            assert_ne!(scheduled, Some(0), "{name}: {stdout}{stderr}");
            refused.push(index);
            continue;
        }
        edited += 1;
        let (status, stdout, _) = run(&[
            "schedule",
            output.to_str().unwrap(),
            "--project",
            &project,
            "--json",
        ]);
        assert_eq!(status, Some(0), "{name}");
        let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
        assert_eq!(doc["projects"][0]["changed_activities"], 0, "{name}");
        std::fs::remove_file(&output).unwrap();
        assert_a_rename_changes_one_line(directory.path(), name, &project, code);
    }
    eprintln!("{edited} files edited, {} refused", refused.len());
    assert!(edited >= 40, "{edited}");
}

/// Files whose stored results already match the engine come back unchanged.
#[test]
fn schedule_leaves_a_matching_corpus_file_byte_identical() {
    let Some(root) = std::env::var_os("TOTALFLOAT_CORPUS") else {
        eprintln!("TOTALFLOAT_CORPUS unset; skipping schedule corpus test");
        return;
    };
    let directory = tempfile::tempdir().unwrap();
    let (mut matching, mut scheduled, mut settled) = (0, 0, 0);
    let mut files = Vec::new();
    for entry in std::fs::read_dir(std::path::Path::new(&root).join("files")).unwrap() {
        for file in std::fs::read_dir(entry.unwrap().path()).unwrap() {
            files.push(file.unwrap().path());
        }
    }
    files.sort();
    for (index, file) in files.iter().enumerate() {
        let output = directory.path().join(format!("{index}.xer"));
        let (code, stdout, _) = run(&[
            "schedule",
            file.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "--json",
        ]);
        if code != Some(0) {
            continue;
        }
        scheduled += 1;
        let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
        let second = directory.path().join(format!("{index}-again.xer"));
        let (code, ..) = run(&[
            "schedule",
            output.to_str().unwrap(),
            "-o",
            second.to_str().unwrap(),
        ]);
        assert_eq!(code, Some(0), "{file:?}");
        if std::fs::read(&output).unwrap() == std::fs::read(&second).unwrap() {
            settled += 1;
        } else {
            eprintln!("second run changed {file:?}");
        }
        let unchanged = doc["projects"].as_array().unwrap().iter().all(|project| {
            project["changed_activities"] == 0
                && project["scheduled_finish"] == project["previous_finish"]
        });
        if unchanged {
            matching += 1;
            assert_eq!(
                std::fs::read(&output).unwrap(),
                std::fs::read(file).unwrap(),
                "{file:?}"
            );
        }
    }
    eprintln!(
        "schedule corpus: {scheduled} files scheduled, {matching} byte-identical, {settled} unchanged by a second run"
    );
    assert!(matching > 0, "no corpus file matched");
}

const LINKED: &str = "../../fixtures/xer/batches/001/rel-fs-positive.xer";

fn query(name: &str, args: &[&str]) {
    let mut all = vec!["query", FIXTURE];
    all.extend(args);
    let (code, stdout, stderr) = run(&all);
    assert_eq!(code, Some(0), "{stderr}");
    insta::assert_snapshot!(name, stdout);
}

/// Percent complete by type, and units and costs from the assignments.
#[test]
fn query_shows_percent_complete_units_and_costs() {
    let directory = tempfile::tempdir().unwrap();
    let path = |name: &str| directory.path().join(name).to_str().unwrap().to_owned();
    let (scheduled, staffed, underway) = (path("s.xer"), path("a.xer"), path("p.xer"));
    let edits = path("edits.json");
    std::fs::write(
        &edits,
        r#"[{"op": "unassign_resource", "activity": "TF-001", "resource": "TF-LABOR"},
            {"op": "assign_resource", "activity": "TF-001", "resource": "TF-LABOR", "units": 16}]"#,
    )
    .unwrap();
    let source = "../../fixtures/xer/fresh-writer.xer";
    for args in [
        vec!["schedule", source, "-o", &scheduled],
        vec!["edit", &scheduled, "--edits", &edits, "-o", &staffed],
        vec![
            "edit",
            &staffed,
            "--data-date",
            "2026-01-05 12:00",
            "--as-planned",
            "-o",
            &underway,
        ],
    ] {
        let (code, stdout, stderr) = run(&args);
        assert_eq!(code, Some(0), "{args:?}: {stdout}{stderr}");
    }
    let columns = "code,status,percent_type,percent_complete,duration_percent,physical_percent,units_percent,budgeted_units,actual_units,remaining_units,budgeted_cost,actual_cost,remaining_cost,at_completion_cost";
    let (code, stdout, stderr) = run(&["query", &underway, "--columns", columns]);
    assert_eq!(code, Some(0), "{stderr}");
    insta::assert_snapshot!("query_percent_units_costs", stdout);
}

#[test]
fn query_lists_everything_by_default() {
    query("query_all", &[]);
}

#[test]
fn query_by_project_and_wbs() {
    query("query_project", &["--project", "2"]);
    query("query_wbs", &["--wbs", "SYN1"]);
}

/// A WBS node by its path picks that node alone, though another branch has a
/// node with the same code; its code picks every node that has it.
#[test]
fn query_by_wbs_path_picks_one_node_of_a_shared_code() {
    // The plant schedule with a second node coded 2, under node 1: the new
    // node is `wbs_id` 7, and A9 goes in it.
    let directory = tempfile::tempdir().unwrap();
    let (plant, shared) = (
        directory.path().join("plant.xer"),
        directory.path().join("shared.xer"),
    );
    let (plant, shared) = (plant.to_str().unwrap(), shared.to_str().unwrap());
    for args in [
        vec!["import", "../../fixtures/msproject/plant.xml", "-o", plant],
        vec![
            "edit",
            plant,
            "--add-wbs",
            "2;Yard drains;1",
            "--add",
            "A9;Drain;7;3d",
            "-o",
            shared,
        ],
    ] {
        let (code, stdout, stderr) = run(&args);
        assert_eq!(code, Some(0), "{args:?}: {stdout}{stderr}");
    }
    let codes = |wbs: &str| {
        let (code, stdout, stderr) = run(&["query", shared, "--wbs", wbs, "--columns", "code"]);
        assert_eq!(code, Some(0), "{wbs}: {stderr}");
        stdout
            .lines()
            .skip(1)
            .filter(|line| !line.starts_with("Showing"))
            .map(str::trim)
            .collect::<Vec<_>>()
            .join(" ")
    };
    assert_eq!(codes("Plant & Yard Upgrade.1.2"), "A9");
    assert_eq!(codes("2"), "A5 A6 A9");
    assert_eq!(codes("Plant & Yard Upgrade.2"), "A5 A6");
    assert_eq!(codes("Plant & Yard Upgrade.1"), "A2 A3 A9");
    assert_eq!(codes("Plant & Yard Upgrade"), "A2 A3 A5 A6 A8 A9");
    let (code, stdout, _) = run(&["query", shared, "--wbs", "Plant & Yard Upgrade.3", "--json"]);
    assert_eq!(code, Some(1));
    let error: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(error["error"]["code"], "no-such-wbs");
    assert!(
        !error["error"]["hint"].as_str().unwrap().contains("PROJWBS"),
        "{stdout}"
    );
}

#[test]
fn query_by_activity_code() {
    query("query_code", &["--code", "Synthetic global codes=A"]);
}

#[test]
fn query_by_status_and_type() {
    query("query_status", &["--status", "not-started"]);
    query(
        "query_type",
        &["--type", "resource", "--type", "finish-milestone"],
    );
}

#[test]
fn query_by_float_and_critical() {
    query(
        "query_float",
        &["--float-min", "0", "--float-max", "100000"],
    );
    query("query_critical", &["--critical"]);
}

#[test]
fn query_by_date_range() {
    query(
        "query_dates",
        &[
            "--start-from",
            "2026-01-01 00:00",
            "--finish-to",
            "2026-12-31 23:59",
        ],
    );
}

#[test]
fn query_lists_constraints_calendars_and_durations() {
    let (code, stdout, stderr) = run(&[
        "query",
        FIXTURE,
        "--constrained",
        "--json",
        "--columns",
        "code,constraint,constraint_date,calendar,original_duration_hours",
    ]);
    assert_eq!(code, Some(0), "{stderr}");
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(doc["total"], 1);
    let row = &doc["rows"][0];
    assert_eq!(row["code"], "M1");
    assert_eq!(row["constraint"], "finish-on-or-before");
    assert_eq!(row["constraint_date"], "2026-02-01 17:00");
    assert_ne!(row["calendar"], serde_json::Value::Null);
    assert_eq!(row["original_duration_hours"], 0.0);
}

#[test]
fn query_by_date_finds_finished_work_only_when_asked_and_by_actual_date() {
    let directory = tempfile::tempdir().unwrap();
    let file = edited(
        directory.path(),
        "rel-fs-positive",
        &[
            ("TASK", "1", "status_code", "TK_Complete"),
            ("TASK", "1", "act_start_date", "2026-01-05 08:00"),
            ("TASK", "1", "act_end_date", "2026-01-06 17:00"),
            ("TASK", "1", "early_start_date", "2026-03-02 08:00"),
            ("TASK", "2", "early_start_date", "2026-03-03 08:00"),
        ],
    );
    let codes = |args: &[&str]| -> Vec<String> {
        let mut all = vec!["query", &file, "--json"];
        all.extend(args);
        let (_, stdout, _) = run(&all);
        let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
        doc["rows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["code"].as_str().unwrap().to_owned())
            .collect()
    };
    let lookahead = ["--start-to", "2026-03-31 17:00"];
    assert_eq!(codes(&lookahead).len(), 1);
    let finished = ["--status", "completed", "--start-to", "2026-01-31 17:00"];
    assert_eq!(codes(&finished).len(), 1);
    let not_then = ["--status", "completed", "--start-from", "2026-03-01 08:00"];
    assert_eq!(codes(&not_then).len(), 0);
    // Completed work asked for on one side of an --any group, and the
    // dates on the other, still finds it.
    let in_group = [
        "--status",
        "completed",
        "--any",
        "start_to=2026-01-31 17:00",
    ];
    assert_eq!(codes(&in_group), codes(&finished));
    let status_in_group = [
        "--start-to",
        "2026-01-31 17:00",
        "--any",
        "status=completed",
    ];
    assert_eq!(codes(&status_in_group), codes(&finished));
}

#[test]
fn inspect_shows_the_must_finish_date() {
    let directory = tempfile::tempdir().unwrap();
    let file = edited(
        directory.path(),
        "rel-fs-positive",
        &[("PROJECT", "1", "plan_end_date", "2026-02-02 17:00")],
    );
    let (_, stdout, _) = run(&["inspect", &file]);
    assert!(
        stdout.contains("must finish by 2026-02-02 17:00"),
        "{stdout}"
    );
    let (_, stdout, _) = run(&["inspect", &file, "--json"]);
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(doc["projects"][0]["must_finish_by"], "2026-02-02 17:00");
}

#[test]
fn query_by_text_ignoring_case() {
    query("query_text", &["--text", "MILESTONE"]);
}

#[test]
fn query_follows_relationships() {
    query("query_preds", &["--preds", "M1"]);
    query(
        "query_succs",
        &["--succs", "A1", "--columns", "code,link_type"],
    );
}

#[test]
fn query_traces_a_driving_path() {
    let (code, stdout, stderr) = run(&[
        "query",
        LINKED,
        "--driving-path-to",
        "2",
        "--columns",
        "id,link_type,link_lag_hours,depth",
    ]);
    assert_eq!(code, Some(0), "{stderr}");
    insta::assert_snapshot!("query_driving", stdout);
}

#[test]
fn query_pages_and_chooses_columns() {
    query(
        "query_paging",
        &["--limit", "1", "--offset", "1", "--columns", "id,code"],
    );
    query("query_count_only", &["--limit", "0"]);
}

#[test]
fn query_past_the_last_row_says_so() {
    query("query_past_the_end", &["--offset", "100"]);
}

#[test]
fn query_finds_a_neighbor_in_another_project() {
    query(
        "query_other_project",
        &[
            "--project",
            "2",
            "--preds",
            "B1",
            "--columns",
            "project,code",
        ],
    );
}

#[test]
fn query_will_not_trace_a_network_that_lost_rows() {
    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("lost.xer");
    let text = std::fs::read_to_string(LINKED).unwrap();
    let broken = text.replace("PR_FS\t1\t", "PR_FS\tbad\t");
    assert_ne!(text, broken);
    std::fs::write(&file, broken).unwrap();
    let (code, stdout, _) = run(&[
        "query",
        file.to_str().unwrap(),
        "--driving-path-to",
        "2",
        "--json",
    ]);
    assert_eq!(code, Some(1));
    assert!(stdout.contains("incomplete-network"), "{stdout}");
}

/// A copy of the linked fixture with one link's lag unreadable, so mapping drops it.
fn with_a_lost_link(dir: &tempfile::TempDir) -> String {
    let text = std::fs::read_to_string(LINKED).unwrap();
    let broken = text.replace("PR_FS\t1\t", "PR_FS\tbad\t");
    assert_ne!(text, broken);
    let file = dir.path().join("lost.xer");
    std::fs::write(&file, broken).unwrap();
    file.to_str().unwrap().to_owned()
}

#[test]
fn check_refuses_a_project_that_lost_a_link() {
    let dir = tempfile::tempdir().unwrap();
    let (code, stdout, _) = run(&["check", &with_a_lost_link(&dir), "--json"]);
    assert_eq!(code, Some(1));
    assert!(stdout.contains("incomplete-network"), "{stdout}");
}

#[test]
fn diff_names_a_project_that_lost_a_link_instead_of_calling_it_deleted() {
    let dir = tempfile::tempdir().unwrap();
    let file = with_a_lost_link(&dir);
    let (code, stdout, _) = run(&["diff", LINKED, &file, "--json"]);
    assert_eq!(code, Some(3));
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(doc["incomplete"].as_array().unwrap().len(), 1);
    assert_eq!(doc["projects"].as_array().unwrap().len(), 0);
}

#[test]
fn diff_refuses_a_file_with_no_projects() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("empty.xer");
    std::fs::write(&file, "ERMHDR\t23.12\n%E\n").unwrap();
    let file = file.to_str().unwrap();
    let (code, stdout, _) = run(&["diff", file, file, "--json"]);
    assert_eq!(code, Some(1));
    assert!(stdout.contains("no-projects"), "{stdout}");
}

#[test]
fn a_reader_that_goes_away_ends_the_command_quietly() {
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_totalfloat"))
        .args(["query", LINKED, "--json"])
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdout.take());
    assert_eq!(child.wait().unwrap().code(), Some(0));
}

#[test]
fn query_explains_bad_input() {
    let error = |args: &[&str], expected: &str| {
        let mut all = vec!["query", FIXTURE, "--json"];
        all.extend(args);
        let (code, stdout, _) = run(&all);
        assert_eq!(code, Some(1), "{args:?}");
        assert!(stdout.contains(expected), "{stdout}");
    };
    error(&["--wbs", "NOPE"], "no-such-wbs");
    error(&["--code", "NOPE"], "no-such-code");
    error(&["--code", "Synthetic global codes=NOPE"], "no-such-code");
    error(&["--preds", "NOPE"], "no-such-activity");
    error(&["--columns", "nope"], "no-such-column");
    error(&["--project", "99"], "no-such-project");
    error(&["--start-from", "soon"], "invalid-date");
    error(&["--driving-path-to", "A1"], "schedule-refused");
    let (code, ..) = run(&["query", FIXTURE, "--preds", "M1", "--succs", "A1"]);
    assert_eq!(code, Some(2));
}

/// Every key the command prints is in the published schema, and the reverse.
#[test]
fn query_json_matches_the_published_schema() {
    let schema: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string("../../docs/schemas/query-1.schema.json").unwrap(),
    )
    .unwrap();
    let keys = |value: &serde_json::Value| {
        let mut keys: Vec<String> = value.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        keys
    };
    let mut required: Vec<String> = schema["required"]
        .as_array()
        .unwrap()
        .iter()
        .map(|key| key.as_str().unwrap().to_owned())
        .collect();
    required.sort();
    // Without a baseline or an earned value column: exactly the required keys.
    let plain = [
        "query",
        FIXTURE,
        "--json",
        "--columns",
        "code,name,total_float_hours",
    ];
    let (code, stdout, _) = run(&plain);
    assert_eq!(code, Some(0));
    let plain: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(plain["schema"], schema["$id"]);
    assert_eq!(keys(&plain), required);
    // With one: every key, and every column.
    let args = [
        "query",
        FIXTURE,
        "--json",
        "--columns",
        "all",
        "--baseline",
        FIXTURE,
        "--by-cost-account",
    ];
    let (code, stdout, stderr) = run(&args);
    assert_eq!(code, Some(0), "{stderr}");
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(keys(&doc), keys(&schema["properties"]));
    assert_eq!(
        keys(&doc["baseline"]),
        keys(&schema["properties"]["baseline"]["properties"])
    );
    assert_eq!(
        keys(&doc["earned_value_total"]),
        keys(&schema["properties"]["earned_value_total"]["properties"])
    );

    let row = &schema["properties"]["rows"]["items"]["properties"];
    assert_eq!(keys(&doc["rows"][0]), keys(row));
    for row in doc["rows"].as_array().unwrap() {
        assert!(row["id"].is_u64());
        assert!(row["code"].is_string());
        assert!(row["critical"].is_boolean());
        assert!(row["remaining_duration_hours"].is_number());
        let status = row["status"].as_str().unwrap();
        let allowed = schema["properties"]["rows"]["items"]["properties"]["status"]["enum"]
            .as_array()
            .unwrap();
        assert!(allowed.iter().any(|value| value == status));
    }
}

/// A copy of a batch fixture with table cells changed, keyed by the row's first cell.
fn edited(directory: &std::path::Path, name: &str, cells: &[(&str, &str, &str, &str)]) -> String {
    let batch = format!("../../fixtures/xer/batches/001/{name}.xer");
    let source = if std::path::Path::new(&batch).exists() {
        batch
    } else {
        format!("../../fixtures/xer/{name}.xer")
    };
    let text = std::fs::read_to_string(source).unwrap();
    let ending = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let mut lines: Vec<String> = text.split(ending).map(str::to_owned).collect();
    for (table, key, field, value) in cells {
        let start = lines
            .iter()
            .position(|line| line == &format!("%T\t{table}"))
            .unwrap();
        let header: Vec<String> = lines[start + 1].split('\t').map(str::to_owned).collect();
        let column = header.iter().position(|f| f == field).unwrap();
        let row = lines[start + 2..]
            .iter()
            .position(|line| line.split('\t').nth(1) == Some(key))
            .unwrap();
        let mut row_cells: Vec<String> = lines[start + 2 + row]
            .split('\t')
            .map(str::to_owned)
            .collect();
        (*value).clone_into(&mut row_cells[column]);
        lines[start + 2 + row] = row_cells.join("\t");
    }
    let file = directory.join(format!("{name}.xer"));
    std::fs::write(&file, lines.join(ending)).unwrap();
    file.to_str().unwrap().to_owned()
}

/// Every way a chain can start, with a snapshot of how it reads.
#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one table of cases, one for each origin"
)]
fn explain_reaches_every_origin() {
    let directory = tempfile::tempdir().unwrap();
    let batch = |name: &str| format!("../../fixtures/xer/batches/001/{name}.xer");
    let edit = |case: &str, name: &str, cells: &[(&str, &str, &str, &str)]| {
        let dir = directory.path().join(case);
        std::fs::create_dir(&dir).unwrap();
        edited(&dir, name, cells)
    };
    let started: [(&str, &str, &str, &str); 2] = [
        ("TASK", "1", "status_code", "TK_Active"),
        ("TASK", "1", "act_start_date", "2026-01-05 08:00"),
    ];
    let mut measured_from_actual = started.to_vec();
    measured_from_actual.push(("SCHEDOPTIONS", "1", "sched_lag_early_start_flag", "N"));
    measured_from_actual.push(("TASKPRED", "1", "lag_hr_cnt", "200"));
    let mut resumed = started.to_vec();
    resumed.push(("TASK", "1", "suspend_date", "2026-01-06 08:00"));
    resumed.push(("TASK", "1", "resume_date", "2026-01-20 08:00"));
    let cases = [
        (
            "data_date",
            "data-date",
            batch("rel-fs-positive"),
            "1",
            "early_start",
        ),
        (
            "duration_and_link",
            "data-date",
            batch("rel-fs-positive"),
            "2",
            "early_finish",
        ),
        (
            "project_finish",
            "project-finish",
            batch("rel-fs-positive"),
            "2",
            "late_finish",
        ),
        (
            "constraint",
            "constraint",
            batch("c-start-after"),
            "2",
            "early_start",
        ),
        (
            "level_of_effort",
            "level-of-effort-span",
            batch("loe"),
            "3",
            "early_start",
        ),
        (
            "wbs_summary",
            "wbs-summary-span",
            batch("wbs-summary"),
            "3",
            "early_start",
        ),
        (
            "external_date",
            "external-date",
            edit(
                "external",
                "rel-fs-positive",
                &[("TASK", "1", "external_early_start_date", "2026-02-02 08:00")],
            ),
            "1",
            "early_start",
        ),
        (
            "must_finish_by",
            "must-finish-by",
            edit(
                "deadline",
                "rel-fs-positive",
                &[("PROJECT", "1", "plan_end_date", "2026-02-02 17:00")],
            ),
            "2",
            "late_finish",
        ),
        (
            "project_start",
            "project-start",
            edit(
                "start",
                "rel-fs-positive",
                &[("PROJECT", "1", "plan_start_date", "2026-02-02 08:00")],
            ),
            "1",
            "early_start",
        ),
        (
            "open_end",
            "open-end",
            edit(
                "open",
                "open-ends",
                &[("SCHEDOPTIONS", "1", "sched_open_critical_flag", "Y")],
            ),
            "1",
            "late_finish",
        ),
        (
            "expected_finish",
            "expected-finish",
            edit(
                "expected",
                "rel-fs-positive",
                &[
                    ("SCHEDOPTIONS", "1", "sched_use_expect_end_flag", "Y"),
                    ("TASK", "1", "expect_end_date", "2026-01-13 17:00"),
                ],
            ),
            "1",
            "early_finish",
        ),
        (
            "resume",
            "resume",
            edit("resume", "rel-fs-positive", &resumed),
            "1",
            "early_start",
        ),
        (
            "actual",
            "actual",
            edit("actual", "rel-ss-positive", &measured_from_actual),
            "2",
            "early_start",
        ),
        (
            "as_late_as_possible",
            "project-finish",
            edit(
                "alap",
                "open-ends",
                &[("TASK", "2", "cstr_type", "CS_ALAP")],
            ),
            "2",
            "early_start",
        ),
    ];
    for (name, origin, file, activity, field) in cases {
        let (code, stdout, stderr) = run(&["explain", &file, activity, "--field", field]);
        assert_eq!(code, Some(0), "{name}: {stderr}");
        insta::assert_snapshot!(format!("explain_{name}"), stdout);
        let (_, json, _) = run(&["explain", &file, activity, "--field", field, "--json"]);
        let doc: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(doc["schema"], "totalfloat.explain/1");
        assert_eq!(doc["chains"][0]["origin"], origin, "{name}: {stdout}");
    }
}

/// A chain leaves the project at a link to another one and stops there.
#[test]
fn explain_stops_at_another_project() {
    let directory = tempfile::tempdir().unwrap();
    let file = edited(
        directory.path(),
        "model-mapping",
        &[("TASK", "101", "act_start_date", "2026-03-02 08:00")],
    );
    let (code, stdout, stderr) = run(&["explain", &file, "B1"]);
    assert_eq!(code, Some(0), "{stderr}");
    insta::assert_snapshot!("explain_other_project", stdout);
    let (_, json, _) = run(&["explain", &file, "B1", "--json"]);
    let doc: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(doc["chains"][0]["origin"], serde_json::Value::Null);
}

#[test]
fn explain_total_float_shows_both_dates() {
    let (code, stdout, _) = run(&["explain", LINKED, "1", "--field", "total_float"]);
    assert_eq!(code, Some(0));
    insta::assert_snapshot!("explain_total_float", stdout);
    let (_, json, _) = run(&["explain", LINKED, "1", "--field", "total_float", "--json"]);
    let doc: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(doc["chains"].as_array().unwrap().len(), 2);
    assert_eq!(doc["field"], "total_float");
    assert!(doc["total_float_hours"].is_number());
}

/// docs/rules/TF-005-no-forecast-before-the-data-date.md, item 7: A2 has
/// started and its mandatory finish leaves too little time for its eight
/// remaining hours after A1. `explain` gives the link from A1 as what sets its
/// early dates, and the mandatory finish as what sets its late dates.
#[test]
fn explain_shows_what_sets_each_date_on_started_work_past_its_mandatory_finish() {
    let directory = tempfile::tempdir().unwrap();
    let file = edited(
        directory.path(),
        "rel-fs-positive",
        &[
            ("TASK", "2", "status_code", "TK_Active"),
            ("TASK", "2", "act_start_date", "2026-01-09 08:00"),
            ("TASK", "2", "cstr_type", "CS_MANDFIN"),
            ("TASK", "2", "cstr_date", "2026-01-13 12:00"),
        ],
    );
    let (code, stdout, stderr) = run(&["explain", &file, "A2", "--field", "total_float"]);
    assert_eq!(code, Some(0), "{stderr}");
    insta::assert_snapshot!("explain_started_past_mandatory_finish", stdout);
    let (_, json, _) = run(&["explain", &file, "A2", "--field", "total_float", "--json"]);
    let doc: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(doc["total_float_hours"].as_f64().unwrap() < 0.0, "{json}");
}

#[test]
fn explain_explains_bad_input() {
    let (code, stdout, _) = run(&["explain", LINKED, "NOPE", "--json"]);
    assert_eq!(code, Some(1));
    assert!(stdout.contains("no-such-activity"), "{stdout}");
    let (code, stdout, _) = run(&["explain", FIXTURE, "A1", "--json"]);
    assert_eq!(code, Some(1));
    assert!(stdout.contains("schedule-refused"), "{stdout}");
    let (code, ..) = run(&["explain", LINKED, "1", "--field", "nope"]);
    assert_eq!(code, Some(2));
}

#[test]
fn check_human_and_exit_code() {
    let (code, stdout, _) = run(&["check", LINKED]);
    assert_eq!(code, Some(3));
    insta::assert_snapshot!("check_human", stdout);
}

#[test]
fn check_markdown() {
    let (code, stdout, _) = run(&["check", LINKED, "--markdown"]);
    assert_eq!(code, Some(3));
    insta::assert_snapshot!("check_markdown", stdout);
}

/// `check --html`: every check with its outcome, the same bytes each
/// time, and never over an existing file.
#[test]
fn check_writes_a_page() {
    const USAGE: &str = "../../fixtures/xer/usage.xer";
    let directory = tempfile::tempdir().unwrap();
    let page = |name: &str| {
        let path = directory.path().join(name);
        let path = path.to_str().unwrap().to_owned();
        let (code, stdout, stderr) = run(&["check", USAGE, "--html", "-o", &path]);
        assert_eq!(code, Some(3), "{stderr}");
        assert_eq!(stdout, format!("Wrote {path}\n"));
        std::fs::read_to_string(path).unwrap()
    };
    let first = page("check.html");
    assert_eq!(first, page("again.html"));
    insta::assert_snapshot!("check_html", first);
    let (_, stdout, _) = run(&["check", USAGE, "--json"]);
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let checks = doc["projects"][0]["checks"].as_array().unwrap();
    assert_eq!(checks.len(), 20);
    for check in checks {
        let name = check["name"].as_str().unwrap();
        let row = match check["outcome"].as_str().unwrap() {
            "fail" => format!("<tr class=\"fail\"><td>{name}</td><td class=\"fail\">FAIL</td>"),
            "pass" => format!("<tr><td>{name}</td><td class=\"pass\">pass</td>"),
            _ => format!("<tr><td>{name}</td><td class=\"na\">n/a</td>"),
        };
        assert!(first.contains(&row), "{row}");
    }
    let (code, _, stderr) = run(&[
        "check",
        USAGE,
        "--html",
        "-o",
        directory.path().join("check.html").to_str().unwrap(),
    ]);
    assert_eq!(code, Some(1));
    assert!(stderr.contains("output-exists"), "{stderr}");
    let (code, ..) = run(&["check", USAGE, "--html", "--markdown"]);
    assert_eq!(code, Some(2));
}

#[test]
fn check_json_lists_every_check() {
    let (_, stdout, _) = run(&["check", LINKED, "--json"]);
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(doc["schema"], "totalfloat.check/1");
    assert_eq!(doc["projects"][0]["checks"].as_array().unwrap().len(), 20);
}

#[test]
fn check_config_changes_thresholds_and_refuses_unknown_keys() {
    let directory = tempfile::tempdir().unwrap();
    let config = directory.path().join("thresholds.toml");
    let path = config.to_str().unwrap();
    std::fs::write(
        &config,
        "logic_percent = 100.0\nhard_constraints_percent = 100.0\nlags_percent = 100.0\nresources_percent = 100.0\n",
    )
    .unwrap();
    let (code, stdout, _) = run(&["check", LINKED, "--config", path, "--json"]);
    assert_eq!(code, Some(0), "{stdout}");

    std::fs::write(&config, "logic_percentage = 1.0\n").unwrap();
    let (code, stdout, _) = run(&["check", LINKED, "--config", path, "--json"]);
    assert_eq!(code, Some(1));
    assert!(stdout.contains("invalid-config"), "{stdout}");
}

/// The fresh fixture, then the same file scheduled and given a change.
fn update_pair(directory: &std::path::Path) -> (String, String) {
    let earlier = directory.join("earlier.xer");
    let later = directory.join("later.xer");
    let (code, ..) = run(&["schedule", FRESH, "-o", earlier.to_str().unwrap()]);
    assert_eq!(code, Some(0));
    let text = std::fs::read_to_string(&earlier).unwrap();
    let header = text
        .lines()
        .position(|l| l == "%T\tTASK" || l == "%T\tTASK\r")
        .unwrap();
    let fields: Vec<&str> = text.lines().nth(header + 1).unwrap().split('\t').collect();
    let column = fields
        .iter()
        .position(|f| *f == "remain_drtn_hr_cnt")
        .unwrap();
    let mut lines: Vec<String> = text.split('\n').map(str::to_owned).collect();
    let mut cells: Vec<String> = lines[header + 2].split('\t').map(str::to_owned).collect();
    "1".clone_into(&mut cells[column]);
    lines[header + 2] = cells.join("\t");
    std::fs::write(&later, lines.join("\n")).unwrap();
    (
        earlier.to_str().unwrap().to_owned(),
        later.to_str().unwrap().to_owned(),
    )
}

#[test]
fn diff_flags_a_critical_cut_and_exits_three() {
    let directory = tempfile::tempdir().unwrap();
    let (earlier, later) = update_pair(directory.path());
    let (code, stdout, _) = run(&["diff", &earlier, &later]);
    assert_eq!(code, Some(3), "{stdout}");
    let stdout = stdout.replace(directory.path().to_str().unwrap(), "");
    insta::assert_snapshot!("diff_human", stdout);
    let (code, stdout, _) = run(&["diff", &earlier, &later, "--markdown"]);
    assert_eq!(code, Some(3));
    let stdout = stdout
        .replace(directory.path().to_str().unwrap(), "")
        .replace('\\', "/");
    insta::assert_snapshot!("diff_markdown", stdout);
}

#[test]
fn diff_of_a_file_with_itself_finds_nothing() {
    let (code, stdout, _) = run(&["diff", FRESH, FRESH, "--json"]);
    assert_eq!(code, Some(0), "{stdout}");
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(doc["projects"][0]["counts"]["changed"], 0);
}

/// Every key the command prints is in the published schema, and the reverse.
#[test]
fn diff_json_matches_the_published_schema() {
    let schema: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string("../../docs/schemas/diff-1.schema.json").unwrap(),
    )
    .unwrap();
    let directory = tempfile::tempdir().unwrap();
    let (earlier, later) = update_pair(directory.path());
    let (_, stdout, _) = run(&["diff", &earlier, &later, "--json"]);
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(doc["schema"], schema["$id"]);
    let keys = |value: &serde_json::Value| {
        let mut keys: Vec<String> = value.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        keys
    };
    assert_eq!(keys(&doc), keys(&schema["properties"]));
    let comparison = &schema["$defs"]["comparison"];
    let project = &doc["projects"][0];
    assert_eq!(keys(project), keys(&comparison["properties"]));
    assert_eq!(
        keys(&project["counts"]),
        keys(&comparison["properties"]["counts"]["properties"])
    );
    assert_eq!(
        keys(&project["project"]),
        keys(&comparison["properties"]["project"]["properties"])
    );
}

fn textconv(path: &str) -> String {
    let (code, stdout, stderr) = run(&["textconv", path]);
    assert_eq!(code, Some(0), "{stderr}");
    stdout
}

#[test]
fn textconv_gives_stable_sorted_text() {
    insta::assert_snapshot!("textconv_fixture", textconv(FIXTURE));
}

#[test]
fn textconv_doesn_t_care_what_order_the_rows_are_in() {
    let text = std::fs::read_to_string(LINKED).unwrap();
    // Reverse the TASK table's rows, as another writer might order them.
    let mut lines: Vec<&str> = text.split_inclusive('\n').collect();
    let start = lines
        .iter()
        .position(|l| l.starts_with("%T\tTASK"))
        .unwrap()
        + 2;
    let end = start
        + lines[start..]
            .iter()
            .take_while(|l| l.starts_with("%R"))
            .count();
    lines[start..end].reverse();
    let directory = tempfile::tempdir().unwrap();
    let shuffled = directory.path().join("shuffled.xer");
    std::fs::write(&shuffled, lines.concat()).unwrap();
    assert_eq!(textconv(shuffled.to_str().unwrap()), textconv(LINKED));
}

#[test]
fn a_duration_change_is_one_changed_input_line() {
    let directory = tempfile::tempdir().unwrap();
    let edited = directory.path().join("edited.xer");
    run(&[
        "edit",
        LINKED,
        "--duration",
        "A1=3d",
        "-o",
        edited.to_str().unwrap(),
    ]);
    let (before, after) = (textconv(LINKED), textconv(edited.to_str().unwrap()));
    let inputs = |text: &str| -> Vec<String> {
        text.lines()
            .filter(|l| l.starts_with("activity "))
            .map(str::to_owned)
            .collect()
    };
    let changed: Vec<_> = inputs(&before)
        .into_iter()
        .zip(inputs(&after))
        .filter(|(a, b)| a != b)
        .collect();
    assert_eq!(changed.len(), 1, "{changed:?}");
    assert!(changed[0].1.contains("original 24h"), "{changed:?}");
}

/// The lines of an XER with `RSRC.def_qty_per_hr` taken out.
fn without_rsrc_units(source: &str) -> Vec<String> {
    let mut table = String::new();
    let mut at = None;
    source
        .lines()
        .map(|line| {
            if let Some(name) = line.strip_prefix("%T\t") {
                name.clone_into(&mut table);
                at = None;
            }
            let mut cells: Vec<&str> = line.split('\t').collect();
            if table == "RSRC" && cells[0] == "%F" {
                at = cells.iter().position(|c| *c == "def_qty_per_hr");
            }
            if let Some(at) = at
                && table == "RSRC"
                && (cells[0] == "%F" || cells[0] == "%R")
            {
                cells.remove(at);
            }
            cells.join("\t")
        })
        .collect()
}

/// A file exported without `RSRC.def_qty_per_hr` takes a new resource at
/// the default 1 an hour, and says it reads back with none; units it can't
/// hold are refused with a hint naming the column.
#[test]
fn a_resource_table_without_units_per_hour_takes_a_new_resource() {
    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("no-units.xer");
    let source = std::fs::read_to_string("../../fixtures/xer/fresh-writer.xer").unwrap();
    let lines = without_rsrc_units(&source);
    // The fixture had the column; the copy doesn't.
    assert!(source.contains("def_qty_per_hr"));
    assert!(!lines.iter().any(|line| line.contains("def_qty_per_hr")));
    std::fs::write(&file, lines.join("\r\n") + "\r\n").unwrap();
    let file = file.to_str().unwrap();
    let out = directory.path().join("out.xer");
    let out = out.to_str().unwrap();

    let (status, stdout, stderr) = run(&[
        "edit",
        file,
        "--add-resource",
        "R9;Synthetic crane",
        "-o",
        out,
        "--json",
    ]);
    assert_eq!(status, Some(0), "{stdout}{stderr}");
    let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let warnings = report["warnings"].to_string();
    assert!(
        warnings.contains("R9 is written without its 1 unit an hour"),
        "{warnings}"
    );
    let written = std::fs::read_to_string(out).unwrap();
    assert!(written.contains("Synthetic crane") && !written.contains("def_qty_per_hr"));

    // A later RSRC section with the column doesn't matter: new rows go in
    // the first.
    let rsrc_header = source
        .lines()
        .skip_while(|line| *line != "%T\tRSRC")
        .nth(1)
        .unwrap();
    let end = lines
        .iter()
        .position(|line| line.starts_with("%E"))
        .unwrap();
    let mut sections = lines.clone();
    sections.splice(end..end, ["%T\tRSRC".to_owned(), rsrc_header.to_owned()]);
    let two = directory.path().join("two-sections.xer");
    std::fs::write(&two, sections.join("\r\n") + "\r\n").unwrap();
    let out_two = directory.path().join("out-two.xer");
    let (status, stdout, stderr) = run(&[
        "edit",
        two.to_str().unwrap(),
        "--add-resource",
        "R9;Synthetic crane",
        "-o",
        out_two.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(status, Some(0), "{stdout}{stderr}");
    assert!(
        stdout.contains("R9 is written without its 1 unit an hour"),
        "{stdout}"
    );

    let edits = directory.path().join("edits.json");
    std::fs::write(
        &edits,
        r#"{"schema": "totalfloat.command/1", "edits": [{"op": "add_resource", "resource": "R8", "name": "Synthetic hoist", "units_per_hour": 2}]}"#,
    )
    .unwrap();
    let refused_out = directory.path().join("refused.xer");
    let (status, stdout, _) = run(&[
        "edit",
        file,
        "--edits",
        edits.to_str().unwrap(),
        "-o",
        refused_out.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(status, Some(1), "{stdout}");
    let refused: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(refused["error"]["code"], "edit-not-written", "{stdout}");
    let hint = refused["error"]["hint"].as_str().unwrap();
    assert!(
        hint.contains("RSRC.def_qty_per_hr") && hint.contains("default of 1"),
        "{hint}"
    );
}

/// Resources and global code types are the file's, so a new one can't take a
/// name another project in the file holds.
#[test]
fn a_new_resource_or_global_code_type_can_t_repeat_another_project_s() {
    let two = "../../fixtures/xer/two-projects.xer";
    let directory = tempfile::tempdir().unwrap();
    let out = directory.path().join("out.xer");
    let out = out.to_str().unwrap();
    for (flag, value, code) in [
        ("--add-resource", "r1;Again", "duplicate-resource"),
        (
            "--add-code-type",
            "Synthetic global codes;global",
            "duplicate-code-type",
        ),
    ] {
        let (status, stdout, _) = run(&[
            "edit",
            two,
            "--project",
            "2",
            flag,
            value,
            "-o",
            out,
            "--json",
        ]);
        assert_eq!(status, Some(1), "{stdout}");
        assert!(stdout.contains(code), "{stdout}");
    }
    // A file with P6's full columns takes one.
    let (status, stdout, stderr) = run(&[
        "edit",
        "../../fixtures/xer/fresh-writer.xer",
        "--add-resource",
        "R9;Synthetic crane;nonlabor",
        "--add-code-type",
        "Area",
        "--add-code-value",
        "Area=EAST",
        "--assign",
        "TF-001=R9",
        "--code",
        "TF-001=Area=EAST",
        "-o",
        out,
    ]);
    assert_eq!(status, Some(0), "{stdout}{stderr}");
    let (_, stdout, _) = run(&["inspect", out]);
    assert!(stdout.contains("R9 Synthetic crane"), "{stdout}");
    assert!(stdout.contains("Area: EAST"), "{stdout}");
}

const ROLES: &str = "../../fixtures/xer/roles.xer";

/// A role added under another and assigned to work with no resource,
/// written back, and read by `query` and `inspect`.
#[test]
fn a_role_is_added_assigned_and_read_back() {
    let directory = tempfile::tempdir().unwrap();
    let out = directory.path().join("out.xer");
    let out = out.to_str().unwrap();
    let (status, stdout, stderr) = run(&[
        "edit",
        ROLES,
        "--add-role",
        "RIGGER;Rigger;SUPER",
        "--assign-role",
        "A1=RIGGER:8",
        "-o",
        out,
        "--json",
    ]);
    assert_eq!(status, Some(0), "{stdout}{stderr}");
    let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(report["applied"][0], "added role RIGGER", "{report}");
    assert!(
        report["applied"][1]
            .as_str()
            .unwrap()
            .starts_with("A1 assigned role RIGGER, with 8 units"),
        "{report}"
    );
    let (status, stdout, stderr) = run(&["query", out, "--columns", "code,resources,roles"]);
    assert_eq!(status, Some(0), "{stderr}");
    assert!(stdout.contains("FOREMAN, RIGGER, SUPER"), "{stdout}");
    let (status, stdout, _) = run(&["inspect", out]);
    assert_eq!(status, Some(0));
    assert!(stdout.contains("roles (3):"), "{stdout}");
    assert!(stdout.contains("RIGGER Rigger"), "{stdout}");
    for (flag, value, code) in [
        ("--add-role", "SUPER;Again", "duplicate-role"),
        ("--assign-role", "A1=NOPE", "no-such-role"),
        ("--add-role", "lonely", "invalid-edit"),
    ] {
        let (status, stdout, stderr) = run(&["edit", ROLES, flag, value, "--dry-run", "--json"]);
        assert_ne!(status, Some(0), "{flag} {value}");
        assert!(
            format!("{stdout}{stderr}").contains(code),
            "{flag} {value}: {stdout}{stderr}"
        );
    }
}

/// Removals come first, as with resources, so a role's units can be replaced
/// in one command.
#[test]
fn a_role_assignment_is_replaced_in_one_command() {
    let (status, stdout, stderr) = run(&[
        "edit",
        ROLES,
        "--unassign-role",
        "A1=SUPER",
        "--assign-role",
        "A1=SUPER:24",
        "--dry-run",
        "--json",
    ]);
    assert_eq!(status, Some(0), "{stdout}{stderr}");
    let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(report["applied"].as_array().unwrap().len(), 2, "{report}");
    assert!(
        report["applied"][1]
            .as_str()
            .unwrap()
            .starts_with("A1 assigned role SUPER, with 24 units"),
        "{report}"
    );
}

#[test]
fn a_role_in_a_file_of_several_projects_needs_one_named() {
    let (status, stdout, stderr) = run(&[
        "edit",
        "../../fixtures/xer/two-projects.xer",
        "--add-role",
        "ENG;Engineer",
        "--dry-run",
    ]);
    assert_ne!(status, Some(0));
    assert!(stderr.contains("needs-project"), "{stdout}{stderr}");
}

const EXPENSES: &str = "../../fixtures/xer/expenses.xer";

/// Expenses are added, costed and deleted by flags, counted in an
/// activity's total cost and earned value, and listed by `query` and
/// `inspect`.
#[test]
fn expenses_are_added_set_deleted_and_totalled() {
    let (status, stdout, stderr) = run(&[
        "query",
        EXPENSES,
        "--columns",
        "code,expenses,budgeted_cost,budgeted_expense_cost,at_completion_expense_cost,budget_at_completion",
        "--json",
    ]);
    assert_eq!(status, Some(0), "{stderr}");
    let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let a1 = report["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["code"] == "A1")
        .unwrap()
        .clone();
    assert_eq!(a1["expenses"], "Building permit", "{a1}");
    assert_eq!(a1["budgeted_expense_cost"], 1500.0, "{a1}");
    // An activity's total cost holds its expenses, and so does
    // its budget at completion.
    assert_eq!(a1["budget_at_completion"], a1["budgeted_cost"], "{a1}");
    assert!(a1["budgeted_cost"].as_f64().unwrap() >= 1500.0, "{a1}");
    // Work with expenses and no assignments has costs, but no units.
    let (status, stdout, _) = run(&[
        "query",
        EXPENSES,
        "--columns",
        "code,budgeted_units,units_percent,budgeted_cost",
        "--json",
    ]);
    assert_eq!(status, Some(0));
    let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let m1 = report["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["code"] == "M1")
        .unwrap()
        .clone();
    assert_eq!(m1["budgeted_units"], serde_json::Value::Null, "{m1}");
    assert_eq!(m1["units_percent"], serde_json::Value::Null, "{m1}");
    assert_eq!(m1["budgeted_cost"], 1850.0, "{m1}");

    let directory = tempfile::tempdir().unwrap();
    let out = directory.path().join("out.xer");
    let out = out.to_str().unwrap();
    let (status, stdout, stderr) = run(&[
        "edit",
        EXPENSES,
        "--add-expense",
        "A1=Crane;2400;SITE;Fees;end",
        "--set-expense",
        "M1=Skip hire;actual=600;remaining=600;accrual=start",
        "--delete-expense",
        "M1=Survey",
        "-o",
        out,
        "--json",
    ]);
    assert_eq!(status, Some(0), "{stdout}{stderr}");
    let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(report["applied"].as_array().unwrap().len(), 3, "{report}");
    let (status, stdout, _) = run(&["query", out, "--columns", "code,expenses"]);
    assert_eq!(status, Some(0));
    assert!(stdout.contains("Building permit, Crane"), "{stdout}");
    assert!(!stdout.contains("Survey"), "{stdout}");
    let (status, stdout, _) = run(&["inspect", out]);
    assert_eq!(status, Some(0));
    assert!(
        stdout.contains("expenses: 3 items, budget 5100, actual 600, remaining 4500"),
        "{stdout}"
    );
    for (flag, value, code) in [
        ("--add-expense", "A1=Crane;lots", "invalid-edit"),
        ("--add-expense", "A1=Crane;10;NOPE", "no-such-cost-account"),
        (
            "--add-expense",
            "A1=Building permit;10",
            "duplicate-expense",
        ),
        ("--set-expense", "A1=Nothing;actual=1", "no-such-expense"),
        (
            "--set-expense",
            "A1=Building permit;spent=1",
            "invalid-edit",
        ),
        ("--delete-expense", "A1=Nothing", "no-such-expense"),
    ] {
        let (status, stdout, stderr) = run(&["edit", EXPENSES, flag, value, "--dry-run", "--json"]);
        assert_ne!(status, Some(0), "{flag} {value}");
        assert!(
            format!("{stdout}{stderr}").contains(code),
            "{flag} {value}: {stdout}{stderr}"
        );
    }
}

const PROJECT_CODES: &str = "../../fixtures/xer/project-codes.xer";

#[test]
fn project_codes_are_added_set_cleared_and_shown() {
    let (status, stdout, _) = run(&["inspect", PROJECT_CODES]);
    assert_eq!(status, Some(0));
    assert!(stdout.contains("project codes: Region=WEST"), "{stdout}");
    assert!(stdout.contains("  Phase: DESIGN, DD"), "{stdout}");

    let directory = tempfile::tempdir().unwrap();
    let out = directory.path().join("out.xer");
    let out = out.to_str().unwrap();
    let (status, stdout, stderr) = run(&[
        "edit",
        PROJECT_CODES,
        "--add-project-code-type",
        "Sponsor",
        "--add-project-code-value",
        "Sponsor=CITY;City council",
        "--project-code",
        "Sponsor=CITY",
        "--project-code",
        "Region=EAST",
        "-o",
        out,
        "--json",
    ]);
    assert_eq!(status, Some(0), "{stdout}{stderr}");
    let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(report["applied"].as_array().unwrap().len(), 4, "{report}");
    let (status, stdout, _) = run(&["inspect", out, "--json"]);
    assert_eq!(status, Some(0));
    let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(
        report["projects"][0]["codes"],
        serde_json::json!({"Region": "EAST", "Sponsor": "CITY"})
    );
    for (flag, value, code) in [
        ("--project-code", "Region=NORTH", "no-such-project-code"),
        ("--project-code", "Regoin=WEST", "no-such-project-code"),
        (
            "--add-project-code-value",
            "Region=WEST",
            "duplicate-project-code",
        ),
        (
            "--add-project-code-type",
            "region",
            "duplicate-project-code-type",
        ),
        ("--project-code", "Region", "invalid-edit"),
        ("--clear-project-code", "Phase", "no-project-code"),
    ] {
        let (status, stdout, stderr) =
            run(&["edit", PROJECT_CODES, flag, value, "--dry-run", "--json"]);
        assert_ne!(status, Some(0), "{flag} {value}");
        assert!(
            format!("{stdout}{stderr}").contains(code),
            "{flag} {value}: {stdout}{stderr}"
        );
    }
}

#[test]
fn a_project_code_in_a_file_of_two_projects_needs_the_project() {
    let (status, stdout, stderr) = run(&[
        "edit",
        TWO_PROJECTS,
        "--add-project-code-type",
        "Region",
        "--add-project-code-value",
        "Region=WEST",
        "--project-code",
        "Region=WEST",
        "--dry-run",
        "--json",
    ]);
    assert_ne!(status, Some(0));
    let text = format!("{stdout}{stderr}");
    assert!(text.contains("needs-project"), "{text}");
    assert!(text.contains("--project-code Region=WEST"), "{text}");
    // Named, the project takes it, and the dictionary reaches both.
    let directory = tempfile::tempdir().unwrap();
    let out = directory.path().join("out.xer");
    let out = out.to_str().unwrap();
    let (status, stdout, stderr) = run(&[
        "edit",
        TWO_PROJECTS,
        "--project",
        "1",
        "--add-project-code-type",
        "Region",
        "--add-project-code-value",
        "Region=WEST",
        "--project-code",
        "Region=WEST",
        "-o",
        out,
    ]);
    assert_eq!(status, Some(0), "{stdout}{stderr}");
    let (_, stdout, _) = run(&["inspect", out, "--json"]);
    let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let codes: Vec<&serde_json::Value> = report["projects"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| &p["codes"])
        .collect();
    assert_eq!(
        codes,
        [
            &serde_json::json!({"Region": "WEST"}),
            &serde_json::json!({})
        ]
    );
    assert_eq!(report["project_codes"][0]["values_total"], 1);
}

const OBS: &str = "../../fixtures/xer/obs.xer";

#[test]
fn query_shows_each_activity_s_responsible_obs_and_edits_change_it() {
    let (status, stdout, stderr) = run(&["query", OBS, "--columns", "code,wbs,obs", "--json"]);
    assert_eq!(status, Some(0), "{stderr}");
    let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let rows = report["rows"].as_array().unwrap();
    assert!(
        rows.iter()
            .all(|r| r["obs"] == "Contractor" || r["obs"] == "Owner"),
        "{report}"
    );
    assert!(rows.iter().any(|r| r["obs"] == "Contractor"), "{report}");

    let directory = tempfile::tempdir().unwrap();
    let out = directory.path().join("out.xer");
    let out = out.to_str().unwrap();
    let (status, stdout, stderr) = run(&[
        "edit",
        OBS,
        "--add-obs",
        "Electrical;Contractor",
        "--wbs-obs",
        "WP=Electrical",
        "-o",
        out,
    ]);
    assert_eq!(status, Some(0), "{stdout}{stderr}");
    let (status, stdout, _) = run(&["query", out, "--columns", "code,wbs,obs"]);
    assert_eq!(status, Some(0));
    assert!(stdout.contains("Electrical"), "{stdout}");
    for (flag, value, code) in [
        ("--wbs-obs", "WP=Nobody", "no-such-obs"),
        ("--wbs-obs", "NOPE=Owner", "no-such-wbs"),
        ("--add-obs", "Owner", "duplicate-obs"),
        ("--add-obs", "Sub;Nobody", "no-such-obs"),
        ("--wbs-obs", "WP", "invalid-edit"),
    ] {
        let (status, stdout, stderr) = run(&["edit", OBS, flag, value, "--dry-run", "--json"]);
        assert_ne!(status, Some(0), "{flag} {value}");
        assert!(
            format!("{stdout}{stderr}").contains(code),
            "{flag} {value}: {stdout}{stderr}"
        );
    }
}

const NOTEBOOKS: &str = "../../fixtures/xer/notebooks.xer";

#[test]
fn query_shows_notes_as_text_and_edits_change_them() {
    let (status, stdout, stderr) = run(&["query", NOTEBOOKS, "--columns", "code,notes", "--json"]);
    assert_eq!(status, Some(0), "{stderr}");
    let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let a1 = report["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["notes"] != "")
        .unwrap()
        .clone();
    assert_eq!(
        a1["notes"],
        "Basis of schedule: Two crews & one crane.\nDurations from the 2025 job.\n\nRisks: Crane hire may slip",
        "{a1}"
    );
    let code = a1["code"].as_str().unwrap().to_owned();
    let (status, stdout, _) = run(&["query", NOTEBOOKS, "--columns", "code,notes"]);
    assert_eq!(status, Some(0));
    assert!(
        stdout.contains("crane. / Durations from the 2025 job. | Risks: Crane"),
        "{stdout}"
    );

    let directory = tempfile::tempdir().unwrap();
    let out = directory.path().join("out.xer");
    let out = out.to_str().unwrap();
    let (status, stdout, stderr) = run(&[
        "edit",
        NOTEBOOKS,
        "--set-note",
        &format!("{code}=Risks=Steel may be late"),
        "--delete-note",
        &format!("{code}=Basis of schedule"),
        "-o",
        out,
    ]);
    assert_eq!(status, Some(0), "{stdout}{stderr}");
    let (_, stdout, _) = run(&["query", out, "--columns", "code,notes", "--json"]);
    let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let row = report["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["code"] == code.as_str())
        .unwrap()
        .clone();
    assert_eq!(row["notes"], "Risks: Steel may be late", "{row}");
    for (flag, value, refusal) in [
        (
            "--add-note",
            format!("{code}=Risks=again"),
            "duplicate-note",
        ),
        ("--add-note", format!("{code}=Scope=x"), "wrong-topic"),
        ("--set-note", format!("{code}=Nope=x"), "no-such-topic"),
        ("--delete-note", code.clone(), "invalid-edit"),
    ] {
        let (status, stdout, stderr) =
            run(&["edit", NOTEBOOKS, flag, &value, "--dry-run", "--json"]);
        assert_ne!(status, Some(0), "{flag} {value}");
        assert!(
            format!("{stdout}{stderr}").contains(refusal),
            "{flag} {value}: {stdout}{stderr}"
        );
    }
}

const STEPS: &str = "../../fixtures/xer/steps.xer";

#[test]
fn query_shows_steps_and_step_edits_drive_physical_percent() {
    let columns = "code,physical_percent,steps,steps_percent";
    let (status, stdout, stderr) = run(&["query", STEPS, "--columns", columns, "--json"]);
    assert_eq!(status, Some(0), "{stderr}");
    let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let a1 = report["rows"][0].clone();
    assert_eq!(a1["code"], "A1");
    assert_eq!(a1["steps"], "Excavate 100%, Form and pour 50%, Backfill 0%");
    assert_eq!(a1["steps_percent"], 50.0);
    assert_eq!(a1["physical_percent"], 50.0);

    let directory = tempfile::tempdir().unwrap();
    let out = directory.path().join("out.xer");
    let out = out.to_str().unwrap();
    let (status, stdout, stderr) = run(&[
        "edit",
        STEPS,
        "--add-step",
        "A1=Cure;2",
        "--step",
        "A1=Form and pour=100%",
        "--complete-step",
        "A1=Backfill",
        "-o",
        out,
    ]);
    assert_eq!(status, Some(0), "{stdout}{stderr}");
    let (_, stdout, _) = run(&["query", out, "--columns", columns, "--json"]);
    let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let a1 = report["rows"][0].clone();
    // (1 x 100 + 2 x 100 + 1 x 100 + 2 x 0) / 6
    assert_eq!(a1["steps_percent"], 66.67, "{a1}");
    assert_eq!(a1["physical_percent"], 66.67, "{a1}");
    for (flag, value, code) in [
        ("--step", "A1=Nope=50", "no-such-step"),
        ("--step", "A1=Excavate=lots", "invalid-edit"),
        ("--add-step", "A1=Excavate", "duplicate-step"),
        ("--delete-step", "A1=Nope", "no-such-step"),
    ] {
        let (status, stdout, stderr) = run(&["edit", STEPS, flag, value, "--dry-run", "--json"]);
        assert_ne!(status, Some(0), "{flag} {value}");
        assert!(
            format!("{stdout}{stderr}").contains(code),
            "{flag} {value}: {stdout}{stderr}"
        );
    }
}

#[test]
fn a_step_name_with_a_semicolon_keeps_it_unless_a_weight_follows() {
    for (value, name) in [("A1=Mix; pour", "Mix; pour"), ("A1=Cure;2", "Cure")] {
        let (status, stdout, stderr) =
            run(&["edit", STEPS, "--add-step", value, "--dry-run", "--json"]);
        assert_eq!(status, Some(0), "{stdout}{stderr}");
        assert!(
            stdout.contains(&format!("A1 gained the step {name}")),
            "{stdout}"
        );
    }
}

#[test]
fn a_duration_percent_sets_remaining_and_remaining_sets_it_back() {
    let directory = tempfile::tempdir().unwrap();
    let out = directory.path().join("out.xer");
    let out = out.to_str().unwrap();
    let (code, stdout, stderr) = run(&[
        "edit",
        FS_LINK,
        "--actual-start",
        "A1=2026-01-05 08:00",
        "--percent",
        "A1=25",
        "-o",
        out,
    ]);
    assert_eq!(code, Some(0), "{stdout}{stderr}");
    let columns = "code,percent_type,percent_complete,duration_percent,remaining_duration_hours";
    let (_, stdout, _) = run(&["query", out, "--columns", columns, "--json"]);
    let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let a1 = report["rows"][0].clone();
    assert_eq!(a1["remaining_duration_hours"], 6.0, "{a1}");
    assert_eq!(a1["percent_complete"], 25.0, "{a1}");
    let again = directory.path().join("again.xer");
    let again = again.to_str().unwrap();
    let (code, stdout, stderr) = run(&["edit", out, "--remaining", "A1=2h", "-o", again]);
    assert_eq!(code, Some(0), "{stdout}{stderr}");
    let (_, stdout, _) = run(&["query", again, "--columns", columns, "--json"]);
    let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(report["rows"][0]["duration_percent"], 75.0, "{report}");
}

#[test]
fn user_defined_fields_are_added_set_shown_and_exported_as_text() {
    let directory = tempfile::tempdir().unwrap();
    let out = directory.path().join("out.xer");
    let out = out.to_str().unwrap();
    let (status, stdout, stderr) = run(&[
        "edit",
        ROLES,
        "--add-udf-type",
        "Risk;indicator",
        "--add-udf-type",
        "Crew size;integer;activity",
        "--udf",
        "A1=Risk=2",
        "--udf",
        "A1=Crew size=4",
        "--udf",
        "A1=Synthetic task number=4.256",
        "-o",
        out,
    ]);
    assert_eq!(status, Some(0), "{stdout}{stderr}");
    let columns = "code,udf:Risk,udf:crew size,udf:Synthetic task number";
    let (status, stdout, stderr) = run(&["query", out, "--columns", columns, "--json"]);
    assert_eq!(status, Some(0), "{stderr}");
    let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let a1 = report["rows"][0].clone();
    assert_eq!(a1["udf:Risk"], "UDF_G2", "{a1}");
    assert_eq!(a1["udf:crew size"], 4.0, "{a1}");
    assert_eq!(a1["udf:Synthetic task number"], 4.26, "{a1}");
    let text = directory.path().join("plan.tfs");
    let (status, stdout, stderr) =
        run(&["export", out, "--to", "tfs", "-o", text.to_str().unwrap()]);
    assert_eq!(status, Some(0), "{stdout}{stderr}");
    let text = std::fs::read_to_string(text).unwrap();
    assert!(text.contains("field Risk indicator"), "{text}");
    assert!(text.contains("  udf Risk=UDF_G2"), "{text}");
    for (flag, value, code) in [
        ("--udf", "A1=Synthetic task number=lots", "wrong-udf-type"),
        ("--udf", "A1=Nope=1", "no-such-udf"),
        ("--clear-udf", "M1=Synthetic task number", "no-udf-value"),
        ("--add-udf-type", "X;colour", "invalid-edit"),
        (
            "--add-udf-type",
            "Synthetic task number;text",
            "duplicate-udf",
        ),
    ] {
        let (status, stdout, stderr) = run(&["edit", ROLES, flag, value, "--dry-run", "--json"]);
        assert_ne!(status, Some(0), "{flag} {value}");
        assert!(
            format!("{stdout}{stderr}").contains(code),
            "{flag} {value}: {stdout}{stderr}"
        );
    }
    let (status, _, stderr) = run(&["query", ROLES, "--columns", "code,udf:Nope"]);
    assert_ne!(status, Some(0));
    assert!(stderr.contains("no-such-udf"), "{stderr}");
}

const WBS_SUMMARY: &str = "../../fixtures/xer/batches/001/wbs-summary.xer";

#[test]
fn a_global_change_finds_what_query_finds_and_lists_each_change() {
    let (status, stdout, _) = run(&[
        "query",
        WBS_SUMMARY,
        "--type",
        "task",
        "--columns",
        "code",
        "--json",
    ]);
    assert_eq!(status, Some(0));
    let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let found: Vec<String> = report["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["code"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(found, ["A1", "A2"]);
    let (status, stdout, stderr) = run(&[
        "edit",
        WBS_SUMMARY,
        "--where",
        "type=task",
        "--set",
        "duration+=1d",
        "--set",
        "suffix= (rev)",
        "--dry-run",
        "--json",
    ]);
    assert_eq!(status, Some(0), "{stdout}{stderr}");
    let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let applied: Vec<&str> = report["applied"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a.as_str().unwrap())
        .collect();
    // Each activity found gets each change, a day of its own calendar.
    assert_eq!(
        applied,
        [
            "A1 duration: original 8h to 16h, remaining 8h to 16h",
            "A1 renamed",
            "A2 duration: original 8h to 24h, remaining 8h to 24h",
            "A2 renamed",
        ],
        "{report}"
    );
}

#[test]
fn a_global_change_applies_to_every_activity_or_none() {
    let directory = tempfile::tempdir().unwrap();
    let out = directory.path().join("out.xer");
    // A2's calendar has 16-hour days, so taking 1d off its 8h is refused, and
    // A1's change goes with it.
    let (status, stdout, stderr) = run(&[
        "edit",
        WBS_SUMMARY,
        "--where",
        "type=task",
        "--set",
        "duration-=1d",
        "-o",
        out.to_str().unwrap(),
    ]);
    assert_ne!(status, Some(0), "{stdout}{stderr}");
    assert!(stderr.contains("negative-duration"), "{stderr}");
    assert!(!out.exists());
    for (filter, set, code) in [
        ("wbz=X", "suffix=x", "invalid-filter"),
        ("type=task", "colour=red", "invalid-change"),
        ("text=nothing-like-this", "suffix=x", "nothing-found"),
    ] {
        let (status, stdout, stderr) = run(&[
            "edit",
            WBS_SUMMARY,
            "--where",
            filter,
            "--set",
            set,
            "--dry-run",
        ]);
        assert_ne!(status, Some(0), "{filter} {set}");
        assert!(
            format!("{stdout}{stderr}").contains(code),
            "{filter}: {stdout}{stderr}"
        );
    }
    let (status, _, stderr) = run(&["edit", WBS_SUMMARY, "--set", "suffix=x"]);
    assert_ne!(status, Some(0));
    assert!(stderr.contains("--where"), "{stderr}");
}

/// `--set FIELD=EXPRESSION` works each activity's value out from its own
/// fields, and refuses before anything changes.
#[test]
fn a_global_change_sets_fields_from_expressions() {
    let directory = tempfile::tempdir().unwrap();
    let file = edited(
        directory.path(),
        "rel-fs-positive",
        &[
            ("TASK", "1", "early_start_date", "2026-01-05 08:00"),
            ("TASK", "1", "cstr_type", "CS_MSOA"),
            ("TASK", "1", "cstr_date", "2026-01-05 08:00"),
        ],
    );
    let out = directory.path().join("out.xer");
    let (status, stdout, stderr) = run(&[
        "edit",
        &file,
        "--where",
        "text=A1",
        "--set",
        "duration=original_duration*2.5",
        "--set",
        "constraint_date=early_start+1d+(2*4h)",
        "-o",
        out.to_str().unwrap(),
    ]);
    assert_eq!(status, Some(0), "{stdout}{stderr}");
    let (_, stdout, _) = run(&[
        "query",
        out.to_str().unwrap(),
        "--text",
        "A1",
        "--columns",
        "original_duration_hours,constraint,constraint_date",
        "--json",
    ]);
    let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let row = &report["rows"][0];
    assert_eq!(row["original_duration_hours"], 20.0, "{row}");
    // Two working days on the weekday calendar from Monday 08:00: Tuesday's finish.
    assert_eq!(row["constraint_date"], "2026-01-06 17:00", "{row}");
    assert_eq!(row["constraint"], "start-on-or-after", "{row}");
    // A number of hours from a duration over a duration.
    let (status, stdout, stderr) = run(&[
        "edit",
        out.to_str().unwrap(),
        "--where",
        "type=task",
        "--set",
        "remaining_duration=4h*(original_duration/8h)",
        "--dry-run",
        "--json",
    ]);
    assert_eq!(status, Some(0), "{stdout}{stderr}");
    let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let applied = report["applied"].to_string();
    assert!(
        applied.contains("A1 duration") && applied.contains("remaining 20h to 10h"),
        "{applied}"
    );
    assert!(
        applied.contains("A2 duration") && applied.contains("remaining 8h to 4h"),
        "{applied}"
    );

    let refused = |set: &str, filter: &str| {
        let (status, _, stderr) = run(&[
            "edit",
            &file,
            "--where",
            filter,
            "--set",
            set,
            "-o",
            directory.path().join("refused.xer").to_str().unwrap(),
        ]);
        assert_ne!(status, Some(0), "{set}");
        assert!(!directory.path().join("refused.xer").exists(), "{set}");
        stderr
    };
    let said = refused("duration=early_start", "text=A1");
    assert!(
        said.contains("it gives a date, and duration takes a duration"),
        "{said}"
    );
    let said = refused("constraint_date=data_date+1d", "type=task");
    assert!(
        said.contains("A2 has no primary constraint with a date to move"),
        "{said}"
    );
    let said = refused("remaining_duration=total_float*2", "type=task");
    assert!(said.contains("has no total_float"), "{said}");
    let said = refused("duration=original_duration-2d", "text=A1");
    assert!(said.contains("A1 would have a negative duration"), "{said}");
}

/// Each `--set` expression reads what the ones before it left.
#[test]
fn a_global_change_s_expressions_read_the_changes_before_them() {
    let directory = tempfile::tempdir().unwrap();
    let file = edited(directory.path(), "rel-fs-positive", &[]);
    // Each --set reads what the ones before it left: remaining follows a new
    // original on work not started, and a constraint set first can be moved.
    let applied = |sets: &[&str]| {
        let mut args = vec!["edit", file.as_str(), "--where", "text=A2"];
        for set in sets {
            args.extend(["--set", set]);
        }
        args.extend(["--dry-run", "--json"]);
        let (status, stdout, stderr) = run(&args);
        (status, format!("{stdout}{stderr}"))
    };
    let (status, said) = applied(&[
        "duration=original_duration*2",
        "remaining_duration=remaining_duration*0.5",
    ]);
    assert_eq!(status, Some(0), "{said}");
    assert!(said.contains("remaining 16h to 8h"), "{said}");
    let (status, said) = applied(&[
        "constraint=start-on-or-after@2026-01-05 08:00",
        "constraint_date=constraint_date+1d",
    ]);
    assert_eq!(status, Some(0), "{said}");
    let (status, said) = applied(&[
        "constraint=as-late-as-possible",
        "constraint_date=data_date+1d",
    ]);
    assert_ne!(status, Some(0), "{said}");
    assert!(
        said.contains("A2 has no primary constraint with a date to move"),
        "{said}"
    );
}

/// Finished work is skipped by remaining-duration and constraint-date
/// expressions, and a user field takes a number or a date as its type says.
#[test]
fn a_global_change_s_expressions_skip_finished_work_and_fill_user_fields() {
    let directory = tempfile::tempdir().unwrap();
    let file = edited(
        directory.path(),
        "rel-fs-positive",
        &[
            ("TASK", "1", "status_code", "TK_Complete"),
            ("TASK", "1", "act_start_date", "2026-01-05 08:00"),
            ("TASK", "1", "act_end_date", "2026-01-05 17:00"),
            ("TASK", "1", "remain_drtn_hr_cnt", "0"),
        ],
    );
    let (status, stdout, stderr) = run(&[
        "edit",
        &file,
        "--where",
        "type=task",
        "--set",
        "remaining_duration=original_duration*0.5",
        "--dry-run",
        "--json",
    ]);
    assert_eq!(status, Some(0), "{stdout}{stderr}");
    let applied =
        serde_json::from_str::<serde_json::Value>(&stdout).unwrap()["applied"].to_string();
    assert!(!applied.contains("A1"), "{applied}");
    assert!(applied.contains("A2 duration"), "{applied}");
    // A number field and a date field, added first.
    let fields = directory.path().join("fields.xer");
    let (status, _, stderr) = run(&[
        "edit",
        &file,
        "--add-udf-type",
        "Crew hours;number",
        "--add-udf-type",
        "Due;finish-date",
        "--add-udf-type",
        "Notes;text",
        "-o",
        fields.to_str().unwrap(),
    ]);
    assert_eq!(status, Some(0), "{stderr}");
    let fields = fields.to_str().unwrap();
    let out = directory.path().join("out.xer");
    let (status, stdout, stderr) = run(&[
        "edit",
        fields,
        "--where",
        "text=A2",
        "--set",
        "udf:Crew hours=original_duration/1h*1.5",
        "--set",
        "udf:Due=data_date+0h",
        "-o",
        out.to_str().unwrap(),
    ]);
    assert_eq!(status, Some(0), "{stdout}{stderr}");
    let (_, stdout, _) = run(&[
        "query",
        out.to_str().unwrap(),
        "--text",
        "A2",
        "--columns",
        "udf:Crew hours,udf:Due",
        "--json",
    ]);
    let row = serde_json::from_str::<serde_json::Value>(&stdout).unwrap()["rows"][0].clone();
    assert_eq!(row["udf:Crew hours"], 12.0, "{row}");
    assert!(row["udf:Due"].is_string(), "{row}");
    for (set, said) in [
        (
            "udf:Crew hours=data_date",
            "it gives a date, and udf:Crew hours takes a number",
        ),
        ("udf:Notes=1", "udf:Notes holds neither a number nor a date"),
        (
            "udf:Crew hours=original_duration",
            "it gives a duration, and udf:Crew hours takes a number or a date",
        ),
    ] {
        let (status, _, stderr) = run(&[
            "edit",
            fields,
            "--where",
            "text=A2",
            "--set",
            set,
            "--dry-run",
        ]);
        assert_ne!(status, Some(0), "{set}");
        assert!(stderr.contains(said), "{set}: {stderr}");
    }
}

/// Sol round 1: changes build on one another.
#[test]
fn a_global_change_s_changes_build_on_one_another() {
    let (status, stdout, stderr) = run(&[
        "edit",
        WBS_SUMMARY,
        "--where",
        "text=A1",
        "--set",
        "duration=5d",
        "--set",
        "duration+=1d",
        "--set",
        "prefix=New ",
        "--set",
        "suffix= (rev)",
        "--dry-run",
        "--json",
    ]);
    assert_eq!(status, Some(0), "{stdout}{stderr}");
    let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let applied: Vec<&str> = report["applied"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a.as_str().unwrap())
        .collect();
    assert_eq!(
        applied[..2],
        [
            "A1 duration: original 8h to 40h, remaining 8h to 40h",
            "A1 duration: original 40h to 48h, remaining 40h to 48h",
        ],
        "{report}"
    );
    let directory = tempfile::tempdir().unwrap();
    let out = directory.path().join("out.xer");
    let (status, stdout, stderr) = run(&[
        "edit",
        WBS_SUMMARY,
        "--where",
        "text=A1",
        "--set",
        "prefix=New ",
        "--set",
        "suffix= (rev)",
        "-o",
        out.to_str().unwrap(),
    ]);
    assert_eq!(status, Some(0), "{stdout}{stderr}");
    let (_, stdout, _) = run(&[
        "query",
        out.to_str().unwrap(),
        "--text",
        "A1",
        "--columns",
        "name",
    ]);
    assert!(
        stdout.contains("New Synthetic activity 1 (rev)"),
        "{stdout}"
    );
}

/// Sol round 2: a duration after a calendar change counts the new calendar's days.
#[test]
fn a_global_change_s_durations_count_days_of_a_calendar_it_sets() {
    let (status, stdout, stderr) = run(&[
        "edit",
        WBS_SUMMARY,
        "--where",
        "text=A1",
        "--set",
        "calendar=Synthetic seven-day sixteen-hour",
        "--set",
        "duration=5d",
        "--set",
        "duration+=1d",
        "--dry-run",
        "--json",
    ]);
    assert_eq!(status, Some(0), "{stdout}{stderr}");
    let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let applied = report["applied"].as_array().unwrap();
    assert_eq!(
        applied[2], "A1 duration: original 80h to 96h, remaining 80h to 96h",
        "{report}"
    );
}

/// `EXPENSES` with every WBS node estimating by a user performance factor of 2.
fn expenses_with_a_user_factor() -> String {
    use std::fmt::Write as _;
    let text = std::fs::read_to_string(EXPENSES).unwrap();
    let (mut in_wbs, mut fields) = (false, Vec::<String>::new());
    let mut out = String::new();
    for line in text.lines() {
        if line.starts_with("%T\t") {
            in_wbs = line == "%T\tPROJWBS";
        } else if in_wbs && line.starts_with("%F\t") {
            fields = line[3..].split('\t').map(str::to_owned).collect();
        } else if in_wbs && line.starts_with("%R\t") {
            let mut cells: Vec<String> = line[3..].split('\t').map(str::to_owned).collect();
            cells.resize(fields.len(), String::new());
            for (name, value) in [
                ("ev_etc_compute_type", "EE_PF_user"),
                ("ev_etc_user_value", "2"),
            ] {
                let at = fields.iter().position(|f| f == name).unwrap();
                value.clone_into(&mut cells[at]);
            }
            writeln!(out, "%R\t{}", cells.join("\t")).unwrap();
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// A user performance factor divides the budget left unless `--user-factor
/// multiply` says otherwise (docs/rules/TF-041-earned-value.md, item 7).
#[test]
fn a_user_performance_factor_divides_unless_told_to_multiply() {
    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("user-factor.xer");
    std::fs::write(&file, expenses_with_a_user_factor()).unwrap();
    let file = file.to_str().unwrap();
    let estimate = |extra: &[&str]| {
        let mut args = vec!["query", file, "--columns", "estimate_to_complete", "--json"];
        args.extend_from_slice(extra);
        let (status, stdout, stderr) = run(&args);
        assert_eq!(status, Some(0), "{stderr}");
        let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
        report["earned_value_total"]["estimate_to_complete"]
            .as_f64()
            .unwrap()
    };
    let default = estimate(&[]);
    assert!(default > 0.0, "the fixture has budget left");
    assert_eq!(estimate(&["--user-factor", "divide"]), default);
    // Budget left / 2 by default, budget left x 2 when told to multiply.
    let multiplied = estimate(&["--user-factor", "multiply"]);
    // Totals are rounded to cents, so four times a rounded total can differ.
    assert!(
        (multiplied - 4.0 * default).abs() < 0.05,
        "{multiplied} {default}"
    );
}

/// Suspend and resume: the dates are written, remaining work restarts
/// on the resume date, and clearing them gives the file back.
#[test]
fn suspend_and_resume_dates_are_edited() {
    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("pair.xer");
    std::fs::copy("../../fixtures/xer/two-projects.xer", &file).unwrap();
    let suspended = directory.path().join("suspended.xer");
    let (code, stdout, stderr) = run(&[
        "edit",
        file.to_str().unwrap(),
        "--suspend",
        "A1=2026-01-02 17:00/2026-01-12 08:00",
        "-o",
        suspended.to_str().unwrap(),
    ]);
    assert_eq!(code, Some(0), "{stderr}");
    assert!(
        stdout.contains("A1 suspended 2026-01-02 17:00, resuming 2026-01-12 08:00"),
        "{stdout}"
    );
    let (_, rows, _) = run(&[
        "query",
        suspended.to_str().unwrap(),
        "--text",
        "A1",
        "--columns",
        "code,early_start",
        "--json",
    ]);
    let rows: serde_json::Value = serde_json::from_str(&rows).unwrap();
    assert_eq!(rows["rows"][0]["early_start"], "2026-01-12 08:00", "{rows}");
    let text = std::fs::read_to_string(&suspended).unwrap();
    assert!(
        text.contains("2026-01-02 17:00\t2026-01-12 08:00"),
        "the dates are written side by side"
    );

    let cleared = directory.path().join("cleared.xer");
    let (code, stdout, stderr) = run(&[
        "edit",
        suspended.to_str().unwrap(),
        "--clear-suspend",
        "A1",
        "-o",
        cleared.to_str().unwrap(),
    ]);
    assert_eq!(code, Some(0), "{stderr}");
    assert!(stdout.contains("A1 no longer suspended"), "{stdout}");
    // Back to the original once recalculated, cell for cell.
    let recalculated = directory.path().join("recalculated.xer");
    run(&[
        "schedule",
        file.to_str().unwrap(),
        "-o",
        recalculated.to_str().unwrap(),
    ]);
    assert_eq!(
        std::fs::read(&cleared).unwrap(),
        std::fs::read(&recalculated).unwrap()
    );

    // Started and suspended in one command: progress goes first.
    let (code, stdout, stderr) = run(&[
        "edit",
        file.to_str().unwrap(),
        "--actual-start",
        "B1=2026-01-02 08:00",
        "--suspend",
        "B1=2026-01-02 17:00/2026-01-12 08:00",
        "--dry-run",
    ]);
    assert_eq!(code, Some(0), "{stdout} {stderr}");
    assert!(stdout.contains("B1 suspended 2026-01-02 17:00"), "{stdout}");

    let (code, _, stderr) = run(&[
        "edit",
        file.to_str().unwrap(),
        "--suspend",
        "A1=2026-01-02 17:00",
        "--dry-run",
    ]);
    assert_ne!(code, Some(0));
    assert!(stderr.contains("ACTIVITY=SUSPEND/RESUME"), "{stderr}");
}

/// `query --csv`: every match, the columns asked for, quoted where it
/// needs to be, and text a spreadsheet would run kept as text.
#[test]
fn query_exports_csv() {
    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("pair.xer");
    let text = std::fs::read_to_string("../../fixtures/xer/two-projects.xer")
        .unwrap()
        .replace("Synthetic finish milestone", "=Finish, then hand over");
    std::fs::write(&file, text).unwrap();
    let (code, stdout, stderr) = run(&[
        "query",
        file.to_str().unwrap(),
        "--columns",
        "code,name,codes,critical,total_float_hours",
        "--csv",
    ]);
    assert_eq!(code, Some(0), "{stderr}");
    assert_eq!(
        stdout,
        "code,name,codes,critical,total_float_hours\n\
         A1,Synthetic active task,Synthetic global codes=A,yes,-1.5\n\
         M1,\"'=Finish, then hand over\",,no,\n\
         B1,Synthetic resource task,Synthetic local codes=B,yes,0.0\n"
    );
    // Every match: no page of 50, and paging can't be asked for.
    let (code, _, stderr) = run(&["query", file.to_str().unwrap(), "--csv", "--limit", "1"]);
    assert_eq!(code, Some(2), "{stderr}");
}

/// `query --xlsx`: a workbook with a header row and every match,
/// numbers as numbers; an existing file is never replaced.
#[test]
fn query_exports_an_excel_workbook() {
    use std::io::Read;
    let directory = tempfile::tempdir().unwrap();
    let book = directory.path().join("activities.xlsx");
    let (code, stdout, stderr) = run(&[
        "query",
        "../../fixtures/xer/two-projects.xer",
        "--columns",
        "code,name,total_float_hours",
        "--xlsx",
        book.to_str().unwrap(),
    ]);
    assert_eq!(code, Some(0), "{stderr}");
    assert!(stdout.contains("Wrote 3 activities"), "{stdout}");
    let mut zip = zip::ZipArchive::new(std::fs::File::open(&book).unwrap()).unwrap();
    let mut read = |name: &str| {
        let mut text = String::new();
        zip.by_name(name)
            .unwrap()
            .read_to_string(&mut text)
            .unwrap();
        text
    };
    let sheet = read("xl/worksheets/sheet1.xml");
    let strings = read("xl/sharedStrings.xml");
    let workbook = read("xl/workbook.xml");
    assert!(workbook.contains("name=\"Activities\""), "{workbook}");
    for text in [
        "code",
        "name",
        "total_float_hours",
        "A1",
        "Synthetic active task",
        "B1",
    ] {
        assert!(strings.contains(&format!(">{text}<")), "{text}: {strings}");
    }
    // Four rows: the header and three activities, the float a number.
    assert_eq!(sheet.matches("<row ").count(), 4, "{sheet}");
    assert!(sheet.contains("<v>-1.5</v>"), "{sheet}");
    // Activities with nothing in the columns asked for are still rows.
    let blank = directory.path().join("blank.xlsx");
    let (code, _, stderr) = run(&[
        "query",
        "../../fixtures/xer/two-projects.xer",
        "--columns",
        "actual_finish",
        "--xlsx",
        blank.to_str().unwrap(),
    ]);
    assert_eq!(code, Some(0), "{stderr}");
    let mut blank_zip = zip::ZipArchive::new(std::fs::File::open(&blank).unwrap()).unwrap();
    let mut blank_sheet = String::new();
    blank_zip
        .by_name("xl/worksheets/sheet1.xml")
        .unwrap()
        .read_to_string(&mut blank_sheet)
        .unwrap();
    assert_eq!(blank_sheet.matches("<row ").count(), 4, "{blank_sheet}");
    let (code, _, stderr) = run(&[
        "query",
        "../../fixtures/xer/two-projects.xer",
        "--xlsx",
        book.to_str().unwrap(),
    ]);
    assert_eq!(code, Some(1), "{stderr}");
    assert!(stderr.contains("output-exists"), "{stderr}");
}

/// Costs are booked to a cost account by name, listed per activity,
/// and added up by account over the matching activities.
#[test]
fn costs_are_booked_to_accounts_and_added_up_by_account() {
    let directory = tempfile::tempdir().unwrap();
    let out = directory.path().join("out.xer");
    let out = out.to_str().unwrap();
    let (status, stdout, stderr) = run(&[
        "edit",
        EXPENSES,
        "--set-cost-account",
        "A1=resource:R1;SITE",
        "--set-cost-account",
        "M1=expense:Survey;SITE.PERMIT",
        "--set-cost-account",
        "A1=expense:Building permit;",
        "-o",
        out,
    ]);
    assert_eq!(status, Some(0), "{stdout}{stderr}");
    for said in [
        "A1's R1 is booked to cost account SITE",
        "M1's expense Survey is booked to cost account SITE.PERMIT",
        "A1's expense Building permit has no cost account",
    ] {
        assert!(stdout.contains(said), "{said}\n{stdout}");
    }
    let (status, stdout, stderr) = run(&[
        "query",
        out,
        "--columns",
        "code,cost_accounts,budgeted_cost",
        "--by-cost-account",
        "--json",
    ]);
    assert_eq!(status, Some(0), "{stderr}");
    let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let row = |code: &str| {
        report["rows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["code"] == code)
            .unwrap()
            .clone()
    };
    assert_eq!(row("A1")["cost_accounts"], "SITE");
    // Accounts show as paths of short names; this fixture's child account
    // already has the short name `SITE.PERMIT`.
    assert_eq!(row("M1")["cost_accounts"], "SITE.SITE.PERMIT");
    let accounts = report["cost_accounts"].as_array().unwrap();
    let names: Vec<Option<&str>> = accounts.iter().map(|a| a["account"].as_str()).collect();
    assert_eq!(names, [Some("SITE"), Some("SITE.SITE.PERMIT"), None]);
    assert_eq!(accounts[1]["parent"], "SITE");
    // Every cost lands in one account or none.
    let sum = |field: &str| -> f64 { accounts.iter().map(|a| a[field].as_f64().unwrap()).sum() };
    let rows: f64 = report["rows"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|r| r["budgeted_cost"].as_f64())
        .sum();
    assert!((sum("budgeted_cost") - rows).abs() < 0.01, "{accounts:?}");

    // `inspect` lists every account by the path edits take.
    let (status, stdout, _) = run(&["inspect", out]);
    assert_eq!(status, Some(0));
    assert!(stdout.contains("cost accounts (2):\n  SITE "), "{stdout}");
    assert!(stdout.contains("\n  SITE.SITE.PERMIT "), "{stdout}");

    let (status, stdout, _) = run(&["query", out, "--by-cost-account"]);
    assert_eq!(status, Some(0));
    assert!(
        stdout.contains("Costs by cost account:\n  SITE: budget"),
        "{stdout}"
    );
    assert!(stdout.contains("\n  (none): budget"), "{stdout}");

    let (status, _, stderr) = run(&[
        "edit",
        EXPENSES,
        "--set-cost-account",
        "A1=vendor:R1;SITE",
        "-o",
        directory.path().join("bad.xer").to_str().unwrap(),
    ]);
    assert_ne!(status, Some(0));
    assert!(stderr.contains("invalid-edit"), "{stderr}");
}

/// `--any` groups: an activity meets one whole group at least, besides
/// every other filter.
#[test]
fn any_groups_widen_and_the_other_filters_still_narrow() {
    let codes = |args: &[&str]| -> std::collections::BTreeSet<String> {
        let mut all = vec![
            "query",
            FIXTURE,
            "--json",
            "--limit",
            "100000",
            "--columns",
            "code",
        ];
        all.extend_from_slice(args);
        let (status, stdout, stderr) = run(&all);
        assert_eq!(status, Some(0), "{args:?}: {stderr}");
        let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
        report["rows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["code"].as_str().unwrap().to_owned())
            .collect()
    };
    let critical = codes(&["--critical"]);
    let named_b = codes(&["--text", "B"]);
    let not_started = codes(&["--status", "not-started"]);
    assert!(!critical.is_empty() && !named_b.is_empty());
    assert!(critical.is_disjoint(&named_b));
    // Any: critical, or B in the ID or name.
    let either = codes(&["--any", "critical=true", "--any", "text=B"]);
    assert_eq!(either, &critical | &named_b);
    // All, with groups: not started, and then either of them.
    let narrowed = codes(&[
        "--status",
        "not-started",
        "--any",
        "critical=true",
        "--any",
        "text=B",
    ]);
    assert_eq!(narrowed, &not_started & &either);
    assert!(narrowed.len() < either.len());
    // A group of two conditions has to meet both.
    let both = codes(&["--any", "critical=true;text=B"]);
    assert_eq!(both, &critical & &named_b);
    for wrong in ["critical=yes", "nothing", "colour=red", ""] {
        let (status, _, stderr) = run(&["query", FIXTURE, "--any", wrong]);
        assert_ne!(status, Some(0), "{wrong}");
        assert!(stderr.contains("--any"), "{wrong}: {stderr}");
    }
    // Usage and risks don't filter activities, so they refuse groups too.
    for other in [&["--usage", "CREW"][..], &["--risks"][..]] {
        let mut args = vec!["query", FIXTURE, "--any", "critical=true"];
        args.extend_from_slice(other);
        let (status, _, stderr) = run(&args);
        assert_eq!(status, Some(2), "{other:?}: {stderr}");
    }
}
