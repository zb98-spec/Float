//! Runs every `totalfloat` command in the user guide, so it can't drift. A
//! `serve` line runs until it prints its link.

#![allow(clippy::unwrap_used, reason = "test helpers outside #[test] functions")]

use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::process::{Command, Stdio};

const FIRST: &str = "../../fixtures/xer/batches/001/rel-fs-positive.xer";
const SECOND: &str = "../../fixtures/xer/batches/001/rel-fs-negative.xer";

/// Splits a command line, keeping double-quoted arguments whole.
fn words(line: &str) -> Vec<String> {
    let mut words = vec![String::new()];
    let mut quoted = false;
    for letter in line.chars() {
        match letter {
            '"' => quoted = !quoted,
            ' ' if !quoted => words.push(String::new()),
            other => words.last_mut().unwrap().push(other),
        }
    }
    words
}

/// Each `totalfloat` line in a page's `sh` blocks.
fn commands(text: &str) -> Vec<String> {
    let mut commands = Vec::new();
    let mut inside = false;
    for line in text.lines() {
        if line.starts_with("```") {
            inside = line == "```sh";
        } else if inside && line.starts_with("totalfloat ") {
            commands.push(line.to_owned());
        }
    }
    commands
}

/// Runs a `serve` line until it prints its link, then stops it, as Ctrl-C
/// would; what it printed. `--open` is refused, so the test never opens a
/// browser.
fn serve_until_linked(parts: &[String], directory: &Path) -> String {
    assert!(
        !parts.iter().any(|part| part == "--open"),
        "`{}`: the guide's runnable `serve` lines can't use --open; say it in the text",
        parts.join(" ")
    );
    let mut child = Command::new(env!("CARGO_BIN_EXE_totalfloat"))
        .args(&parts[1..])
        .current_dir(directory)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut printed = String::new();
    for line in BufReader::new(child.stdout.take().unwrap()).lines() {
        let line = line.unwrap();
        printed.push_str(&line);
        printed.push('\n');
        if line.trim().starts_with("http://127.0.0.1:") {
            let _ = child.kill();
            let _ = child.wait();
            return printed;
        }
    }
    let mut errors = String::new();
    child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut errors)
        .unwrap();
    let code = child.wait().unwrap().code();
    panic!(
        "`{}` stopped ({code:?}) without printing a link: {printed}{errors}",
        parts.join(" ")
    );
}

fn seed(directory: &Path) {
    std::fs::copy(
        "../../fixtures/msproject/plant.xml",
        directory.join("plan.xml"),
    )
    .unwrap();
    std::fs::copy(FIRST, directory.join("plant.xer")).unwrap();
    std::fs::write(
        directory.join("plan.tfs"),
        "totalfloat-schedule 1\n\nproject PLANT \"New Plant\"\n  data-date 2026-03-02 08:00\nwbs PLANT.CIV   \"Civil works\"\n\nactivity A1010 \"Excavate\"   5d\nactivity A1020 \"Pour foundations\" 10d after A1010 FS\n",
    )
    .unwrap();
    std::fs::copy(
        "../../fixtures/xer/two-projects.xer",
        directory.join("two-projects.xer"),
    )
    .unwrap();
    // A crew at work, and its timesheet for the week of 5 January, filled in.
    std::fs::copy("../../fixtures/xer/usage.xer", directory.join("crew.xer")).unwrap();
    std::fs::write(
        directory.join("crew.csv"),
        "activity,name,resource,planned_start,planned_finish,budget_hours,actual_hours,hours,remaining_hours,actual_start,actual_finish,percent\nA1,Synthetic active task,CREW,2026-01-01 08:00,2026-01-12 08:00,40.0,16.0,8,16,,,60\n",
    )
    .unwrap();
    std::fs::copy(FIRST, directory.join("update-1.xer")).unwrap();
    std::fs::copy(SECOND, directory.join("update-2.xer")).unwrap();
    std::fs::write(
        directory.join("status.csv"),
        "activity,actual_start,actual_finish,remaining,percent\nA1,2026-01-12 08:00,2026-01-12 17:00,,\nA2,2026-01-13 08:00,,4h,50\n",
    )
    .unwrap();
    std::fs::write(
        directory.join("float.toml"),
        "title = \"Work with float\"\ncolumns = [\"code\", \"name\", \"total_float_hours\"]\ngroup = \"wbs\"\nsort = [{ column = \"total_float_hours\", descending = true }]\n\n[filter]\nfloat_min = 1\n",
    )
    .unwrap();
    std::fs::write(
        directory.join("my-edits.json"),
        r#"[{"op": "set_duration", "activity": "A1", "original": "15d"}]"#,
    )
    .unwrap();
    std::fs::write(
        directory.join("my-build.json"),
        r#"[
  {"op": "add_activity", "code": "A1", "name": "Excavate", "wbs": "PLANT", "duration": "5d"},
  {"op": "add_activity", "code": "A2", "name": "Pour", "wbs": "PLANT", "duration": "10d"},
  {"op": "add_link", "predecessor": "A1", "successor": "A2"}
]"#,
    )
    .unwrap();
    // A spreadsheet to build from, as CSV files and as a workbook.
    let plan = directory.join("plan");
    std::fs::create_dir(&plan).unwrap();
    let sheets = [
        (
            "project",
            "code,name,data_date\nPLANT,New Plant,2026-03-02 08:00\n",
        ),
        (
            "activities",
            "code,name,wbs_path,duration\nA1010,Excavate,PLANT.CIV,5d\nA1020,Pour foundations,PLANT.CIV,10d\n",
        ),
        ("relationships", "predecessor,successor\nA1010,A1020\n"),
    ];
    let mut book = rust_xlsxwriter::Workbook::new();
    for (name, text) in sheets {
        std::fs::write(plan.join(format!("{name}.csv")), text).unwrap();
        let mut title = name.to_owned();
        title[..1].make_ascii_uppercase();
        let sheet = book.add_worksheet().set_name(title).unwrap();
        for (r, line) in text.lines().enumerate() {
            for (c, cell) in line.split(',').enumerate() {
                let (r, c) = (u32::try_from(r).unwrap(), u16::try_from(c).unwrap());
                sheet.write_string(r, c, cell).unwrap();
            }
        }
    }
    book.save(directory.join("plan.xlsx")).unwrap();
    std::fs::write(
        directory.join("risk.json"),
        r#"{"default": {"min": -10, "max": 25}, "estimates": {"A1": {"min": "4h", "likely": "8h", "max": "16h", "distribution": "pert"}}, "correlations": [{"activities": ["A1", "A2"], "coefficient": 0.5}]}"#,
    )
    .unwrap();
    std::fs::write(
        directory.join("my-thresholds.toml"),
        "logic_percent = 3.0\nhigh_float_days = 30.0\n",
    )
    .unwrap();
}

#[test]
fn every_command_in_the_guide_runs() {
    let mut count = 0;
    let mut pages: Vec<_> = std::fs::read_dir("../../docs/guide")
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    pages.sort();
    for page in pages {
        let directory = tempfile::tempdir().unwrap();
        seed(directory.path());
        for command in commands(&std::fs::read_to_string(&page).unwrap()) {
            let parts = words(&command);
            if parts[1] == "serve" {
                let printed = serve_until_linked(&parts, directory.path());
                assert!(printed.contains("TotalFloat is serving"), "{printed}");
                count += 1;
                continue;
            }
            let output = Command::new(env!("CARGO_BIN_EXE_totalfloat"))
                .args(&parts[1..])
                .current_dir(directory.path())
                .stdin(Stdio::null())
                .output()
                .unwrap();
            let code = output.status.code();
            assert!(
                matches!(code, Some(0 | 3)),
                "{}: `{command}` exited {code:?}: {}",
                page.display(),
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(!output.stdout.is_empty(), "`{command}` printed nothing");
            count += 1;
        }
    }
    assert!(count >= 15, "only {count} commands found");
}

#[test]
fn every_command_has_a_page() {
    let guide: String = std::fs::read_dir("../../docs/guide")
        .unwrap()
        .map(|entry| std::fs::read_to_string(entry.unwrap().path()).unwrap())
        .collect();
    for command in [
        "inspect",
        "schedule",
        "query",
        "explain",
        "check",
        "float",
        "diff",
        "roundtrip",
        "anonymize",
        "history",
        "textconv",
        "fmt",
        "serve",
        "report",
        "timesheet",
    ] {
        assert!(
            guide.contains(&format!("totalfloat {command} ")),
            "the guide doesn't run {command}"
        );
    }
    assert!(guide.contains("totalfloat mcp"));
}

/// The install guide's PowerShell lines that add a folder to the user `Path`,
/// run against a scratch user variable in its place: they keep every entry,
/// add the folder once, and work when the variable doesn't exist yet.
#[cfg(windows)]
#[test]
fn the_windows_path_lines_keep_entries_and_add_once() {
    const NAME: &str = "TOTALFLOAT_GUIDE_TEST_PATH";

    /// Removes the scratch variable, even when an assertion fails.
    struct Scratch;
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = powershell(&format!(
                "[Environment]::SetEnvironmentVariable('{NAME}', $null, 'User')"
            ));
        }
    }

    fn powershell(script: &str) -> String {
        let output = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "`{script}` failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    }

    let page = std::fs::read_to_string("../../docs/guide/install.md").unwrap();
    let mut blocks = Vec::new();
    let mut block: Option<String> = None;
    for line in page.lines() {
        let line = line.trim_end();
        if let Some(text) = block.as_mut() {
            if line == "```" {
                blocks.push(block.take().unwrap());
            } else {
                text.push_str(line);
                text.push('\n');
            }
        } else if line == "```powershell" {
            block = Some(String::new());
        }
    }
    let lines = blocks
        .into_iter()
        .find(|block| block.contains("SetEnvironmentVariable('Path'"))
        .expect("install.md has no PowerShell lines that set Path");
    let script = lines.replace("'Path'", &format!("'{NAME}'"));
    assert!(!script.contains("'Path'"));

    let _scratch = Scratch;
    let read = format!("[Environment]::GetEnvironmentVariable('{NAME}', 'User')");

    // A new account's Path: one entry, stored with %USERPROFILE% unexpanded.
    powershell(&format!(
        "New-ItemProperty -Path HKCU:\\Environment -Name {NAME} -PropertyType ExpandString -Force -Value '%USERPROFILE%\\AppData\\Local\\Microsoft\\WindowsApps;C:\\Other;' | Out-Null"
    ));
    powershell(&script);
    powershell(&script);
    let profile = std::env::var("USERPROFILE").unwrap();
    assert_eq!(
        powershell(&read),
        format!(
            "{profile}\\AppData\\Local\\Microsoft\\WindowsApps;C:\\Other;C:\\Tools\\totalfloat"
        )
    );

    // No user Path at all.
    powershell(&format!(
        "[Environment]::SetEnvironmentVariable('{NAME}', $null, 'User')"
    ));
    powershell(&script);
    powershell(&script);
    assert_eq!(powershell(&read), "C:\\Tools\\totalfloat");
}
