//! Every tool through an in-process MCP client.

use rmcp::model::{CallToolRequestParams, CallToolResult};
use rmcp::service::RunningService;
use rmcp::{RoleClient, ServiceExt};
use serde_json::{Value, json};

use super::Server;

const FIXTURE: &str = "../../fixtures/xer/fresh-writer.xer";
const LINKED: &str = "../../fixtures/xer/batches/001/rel-fs-positive.xer";

async fn client() -> RunningService<RoleClient, ()> {
    let (server_side, client_side) = tokio::io::duplex(1 << 20);
    tokio::spawn(async move {
        let service = Server::new().serve(server_side).await.unwrap();
        let _ = service.waiting().await;
    });
    ().serve(client_side).await.unwrap()
}

fn text(result: &CallToolResult) -> Value {
    let content = result.content.first().unwrap();
    let text = content.as_text().unwrap();
    serde_json::from_str(&text.text).unwrap()
}

async fn call(client: &RunningService<RoleClient, ()>, name: &str, args: Value) -> (bool, Value) {
    let Value::Object(arguments) = args else {
        panic!("arguments must be an object");
    };
    let request = CallToolRequestParams::new(name.to_owned()).with_arguments(arguments);
    let result = client.call_tool(request).await.unwrap();
    (result.is_error == Some(true), text(&result))
}

async fn open(client: &RunningService<RoleClient, ()>, path: &str) -> String {
    let (failed, doc) = call(client, "open", json!({"path": path})).await;
    assert!(!failed, "{doc}");
    doc["handle"].as_str().unwrap().to_owned()
}

#[tokio::test]
async fn every_tool_is_listed_with_a_description() {
    let client = client().await;
    let tools = client.list_all_tools().await.unwrap();
    let mut names: Vec<_> = tools.iter().map(|t| t.name.to_string()).collect();
    names.sort();
    assert_eq!(
        names,
        [
            "apply_text",
            "calendar",
            "check",
            "close",
            "diff",
            "edit",
            "explain",
            "export",
            "float",
            "global_change",
            "history_baseline",
            "history_branch",
            "history_diff",
            "history_export",
            "history_import",
            "history_init",
            "history_log",
            "history_merge",
            "history_proposals",
            "history_revert",
            "history_review",
            "history_show",
            "history_verify",
            "import",
            "inspect",
            "new",
            "open",
            "portfolio_capacity",
            "portfolio_show",
            "query",
            "report",
            "risk",
            "schedule",
            "usage",
            "variance"
        ]
    );
    assert!(tools.iter().all(|t| t.description.is_some()));
    client.cancel().await.unwrap();
}

fn lengthen() -> Value {
    json!([{"op": "set_duration", "activity": "A1", "original": "15d"}])
}

async fn finish(client: &RunningService<RoleClient, ()>, handle: &str) -> Value {
    let args = json!({"handle": handle, "text": "A1", "columns": ["remaining_duration_hours"]});
    let (failed, doc) = call(client, "query", args).await;
    assert!(!failed, "{doc}");
    doc["rows"][0]["remaining_duration_hours"].clone()
}

#[tokio::test]
async fn a_dry_run_leaves_the_handle_alone() {
    let client = client().await;
    let handle = open(&client, LINKED).await;
    let before = finish(&client, &handle).await;
    let args = json!({"handle": handle, "edits": lengthen(), "dry_run": true});
    let (failed, doc) = call(&client, "edit", args).await;
    assert!(!failed, "{doc}");
    assert_eq!(doc["schema"], "totalfloat.edit/1");
    assert_eq!(doc["kept"], false);
    assert_eq!(doc["projects"][0]["scheduled_finish"], "2026-02-02 17:00");
    assert_eq!(finish(&client, &handle).await, before);
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn an_edit_stays_on_the_handle_for_every_other_tool() {
    let client = client().await;
    let handle = open(&client, LINKED).await;
    let original = open(&client, LINKED).await;
    let args = json!({"handle": handle, "edits": lengthen()});
    let (failed, doc) = call(&client, "edit", args).await;
    assert!(!failed, "{doc}");
    assert_eq!(finish(&client, &handle).await.as_f64(), Some(120.0));
    let (failed, doc) = call(&client, "diff", json!({"from": original, "to": handle})).await;
    assert!(!failed, "{doc}");
    assert_eq!(doc["projects"][0]["counts"]["changed"], 1);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("out.xer");
    let args = json!({"handle": handle, "path": path});
    let (failed, doc) = call(&client, "export", args).await;
    assert!(!failed, "{doc}");
    let saved = open(&client, path.to_str().unwrap()).await;
    assert_eq!(finish(&client, &saved).await.as_f64(), Some(120.0));
    let args = json!({"handle": saved});
    let (failed, doc) = call(&client, "schedule", args).await;
    assert!(!failed, "{doc}");
    assert_eq!(doc["projects"][0]["changed_activities"], 0);
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn a_check_that_newly_fails_leaves_the_handle_alone() {
    let client = client().await;
    let handle = open(&client, LINKED).await;
    let before = finish(&client, &handle).await;
    let edits = json!([
        {"op": "remove_link", "predecessor": "A1", "successor": "A2"},
        {"op": "set_duration", "activity": "A1", "original": "15d"}
    ]);
    let args = json!({"handle": handle, "edits": edits, "check": true});
    let (failed, doc) = call(&client, "edit", args).await;
    assert!(!failed, "{doc}");
    assert_eq!(doc["kept"], false, "{doc}");
    assert_eq!(finish(&client, &handle).await, before);
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn a_new_schedule_is_built_and_saved_through_the_tools() {
    let client = client().await;
    let directory = tempfile::tempdir().unwrap();
    let start = directory.path().join("start.xer");
    let args = json!({
        "path": start, "code": "PLANT", "name": "New Plant", "data_date": "2026-03-02 08:00"
    });
    let (failed, doc) = call(&client, "new", args.clone()).await;
    assert!(!failed, "{doc}");
    let handle = doc["handle"].as_str().unwrap().to_owned();
    let edits = json!([
        {"op": "add_activity", "code": "A1", "name": "Dig", "wbs": "PLANT", "duration": "5d"},
        {"op": "add_activity", "code": "A2", "name": "Pour", "wbs": "PLANT", "duration": "5d"},
        {"op": "add_link", "predecessor": "A1", "successor": "A2"}
    ]);
    let (failed, doc) = call(&client, "edit", json!({"handle": handle, "edits": edits})).await;
    assert!(!failed, "{doc}");
    assert_eq!(doc["projects"][0]["scheduled_finish"], "2026-03-13 17:00");
    let done = directory.path().join("done.xer");
    let (failed, doc) = call(&client, "export", json!({"handle": handle, "path": done})).await;
    assert!(!failed, "{doc}");
    let (failed, doc) = call(&client, "new", args).await;
    assert!(failed, "{doc}");
    assert_eq!(doc["error"]["code"], "output-exists");
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn a_new_schedule_can_be_given_as_text() {
    let client = client().await;
    let directory = tempfile::tempdir().unwrap();
    let start = directory.path().join("start.xer");
    let text = "totalfloat-schedule 1\n\nproject PLANT \"New Plant\"\n  data-date 2026-03-02 08:00\n\nactivity A1 \"Dig\" 5d\nactivity A2 \"Pour\" 5d after A1\n";
    let (failed, doc) = call(&client, "new", json!({"path": start, "text": text})).await;
    assert!(!failed, "{doc}");
    assert_eq!(doc["projects"][0]["activities"], 2, "{doc}");
    let handle = doc["handle"].as_str().unwrap().to_owned();
    let (failed, doc) = call(&client, "schedule", json!({"handle": handle})).await;
    assert!(!failed, "{doc}");
    assert_eq!(
        doc["projects"][0]["scheduled_finish"], "2026-03-13 17:00",
        "{doc}"
    );

    let broken = directory.path().join("broken.xer");
    let text = "totalfloat-schedule 1\n\nproject PLANT \"New Plant\"\n  data-date 2026-03-02 08:00\n\nactivity A1 \"Dig\" 5d after A0\n";
    let (failed, doc) = call(&client, "new", json!({"path": broken, "text": text})).await;
    assert!(failed, "{doc}");
    assert_eq!(doc["error"]["code"], "text-refused");
    let problem = &doc["error"]["problems"][0];
    assert_eq!(problem["code"], "unknown-activity");
    assert_eq!(
        (problem["line"].as_u64(), problem["column"].as_u64()),
        (Some(6), Some(28))
    );
    let (failed, doc) = call(&client, "new", json!({"path": broken})).await;
    assert!(failed, "{doc}");
    assert_eq!(doc["error"]["code"], "invalid-argument");
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn an_import_reports_what_didn_t_translate() {
    let client = client().await;
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("plan.xer");
    let args = json!({"path": "../../fixtures/msproject/plant.xml", "output": output});
    let (failed, doc) = call(&client, "import", args).await;
    assert!(!failed, "{doc}");
    assert_eq!(doc["schema"], "totalfloat.import/1");
    assert_eq!(doc["calculated"], true, "{doc}");
    assert!(
        doc["notes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|n| n["kind"] == "summary-logic")
    );
    open(&client, output.to_str().unwrap()).await;
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn export_writes_the_edited_handle_as_ms_project_xml() {
    let client = client().await;
    let handle = open(&client, LINKED).await;
    let args = json!({"handle": handle, "edits": lengthen()});
    let (failed, doc) = call(&client, "edit", args).await;
    assert!(!failed, "{doc}");
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("plan.xml");
    let args = json!({"handle": handle, "path": path, "format": "msproject"});
    let (failed, doc) = call(&client, "export", args).await;
    assert!(!failed, "{doc}");
    assert_eq!(doc["schema"], "totalfloat.export/1");
    assert!(!doc["notes"].as_array().unwrap().is_empty(), "{doc}");
    let written = std::fs::read_to_string(&path).unwrap();
    assert!(
        written.contains("<Duration>PT120H0M0S</Duration>"),
        "{written}"
    );
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn query_compares_with_a_baseline_file() {
    let client = client().await;
    let handle = open(&client, LINKED).await;
    let path = std::fs::canonicalize(LINKED).unwrap();
    let args =
        json!({"handle": handle, "baseline": path, "columns": ["code", "finish_variance_hours"]});
    let (failed, doc) = call(&client, "query", args).await;
    assert!(!failed, "{doc}");
    assert_eq!(doc["baseline"]["matched"], 2, "{doc}");
    assert_eq!(doc["baseline"]["only_here"], 0);
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn variance_compares_two_open_schedules() {
    let client = client().await;
    let (now, was) = (open(&client, LINKED).await, open(&client, LINKED).await);
    let args = json!({"current": now, "baseline": was, "top": 3});
    let (failed, doc) = call(&client, "variance", args).await;
    assert!(!failed, "{doc}");
    assert_eq!(doc["schema"], "totalfloat.variance/1");
    assert_eq!(doc["projects"][0]["counts"]["slipped"], 0, "{doc}");
    client.cancel().await.unwrap();
}

/// The text a tool returns as it is, for a document.
async fn document(
    client: &RunningService<RoleClient, ()>,
    tool: &'static str,
    args: Value,
) -> String {
    let Value::Object(arguments) = args else {
        unreachable!()
    };
    let request = CallToolRequestParams::new(tool).with_arguments(arguments);
    let result = client.call_tool(request).await.unwrap();
    let text = result
        .content
        .first()
        .unwrap()
        .as_text()
        .unwrap()
        .text
        .clone();
    assert_ne!(result.is_error, Some(true), "{text}");
    text
}

/// `check` and `variance` return what the commands write for `format`
///, and JSON without it.
#[tokio::test]
async fn check_and_variance_return_documents_when_asked() {
    let client = client().await;
    let (now, was) = (open(&client, LINKED).await, open(&client, LINKED).await);
    let loaded = crate::loaded::Loaded::new(crate::location::read(LINKED.as_ref()).unwrap());
    let checked = crate::check::report(
        LINKED,
        &loaded,
        None,
        (
            &totalfloat_check::Thresholds::default(),
            &crate::check::Own::default(),
        ),
    )
    .unwrap();
    let measured = crate::variance::report((LINKED, &loaded), (LINKED, &loaded), 10).unwrap();
    for (format, name) in [
        (crate::page::Format::Html, "html"),
        (crate::page::Format::Markdown, "markdown"),
    ] {
        let got = document(&client, "check", json!({"handle": now, "format": name})).await;
        assert_eq!(got, crate::check::render(&checked, &loaded, format));
        let args = json!({"current": now, "baseline": was, "format": name});
        let got = document(&client, "variance", args).await;
        assert_eq!(got, crate::variance::render(&measured, format));
    }
    let page = document(&client, "check", json!({"handle": now, "format": "html"})).await;
    assert!(page.starts_with("<!doctype html>"), "{page}");
    let (failed, _) = raw_call(&client, "check", json!({"handle": now, "format": "csv"})).await;
    assert!(failed);
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn a_refused_edit_leaves_the_handle_alone() {
    let client = client().await;
    let handle = open(&client, LINKED).await;
    let before = finish(&client, &handle).await;
    let edits = json!([
        {"op": "set_duration", "activity": "A1", "original": "15d"},
        {"op": "add_link", "predecessor": "A2", "successor": "A1"}
    ]);
    let (failed, doc) = call(&client, "edit", json!({"handle": handle, "edits": edits})).await;
    assert!(failed);
    assert_eq!(doc["error"]["code"], "circular-logic");
    assert_eq!(finish(&client, &handle).await, before);
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn calendar_lists_a_range_of_days() {
    let client = client().await;
    let handle = open(&client, FIXTURE).await;
    let args =
        json!({"handle": handle, "calendar": "100", "from": "2026-01-03", "to": "2026-01-03"});
    let (failed, doc) = call(&client, "calendar", args).await;
    assert!(!failed, "{doc}");
    assert_eq!(doc["days"][0]["hours"].as_f64(), Some(0.0));
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn open_returns_a_handle_and_a_summary() {
    let client = client().await;
    let (failed, doc) = call(&client, "open", json!({"path": FIXTURE})).await;
    assert!(!failed);
    assert_eq!(doc["handle"], "s1");
    assert_eq!(doc["projects"][0]["activities"], 2);
    let (failed, doc) = call(&client, "open", json!({"path": "/no/such/file.xer"})).await;
    assert!(failed);
    assert_eq!(doc["error"]["code"], "file-not-readable");
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn inspect_describes_an_open_file() {
    let client = client().await;
    let handle = open(&client, FIXTURE).await;
    let (failed, doc) = call(&client, "inspect", json!({"handle": handle})).await;
    assert!(!failed);
    assert_eq!(doc["schema"], "totalfloat.inspect/1");
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn schedule_keeps_results_for_query_and_export() {
    let client = client().await;
    let handle = open(&client, FIXTURE).await;
    let (_, before) = call(&client, "query", json!({"handle": handle})).await;
    assert_eq!(before["rows"][0]["early_start"], Value::Null);

    let (failed, doc) = call(&client, "schedule", json!({"handle": handle})).await;
    assert!(!failed, "{doc}");
    assert_eq!(doc["projects"][0]["changed_activities"], 2);

    let (_, after) = call(&client, "query", json!({"handle": handle})).await;
    assert_eq!(after["rows"][0]["early_start"], "2026-01-05 08:00");

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("out.xer");
    let args = json!({"handle": handle, "path": path.to_str().unwrap()});
    let (failed, doc) = call(&client, "export", args.clone()).await;
    assert!(!failed, "{doc}");
    assert_ne!(
        std::fs::read(&path).unwrap(),
        std::fs::read(FIXTURE).unwrap()
    );
    let (failed, doc) = call(&client, "export", args).await;
    assert!(failed);
    assert_eq!(doc["error"]["code"], "output-exists");
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn query_is_paged_and_capped() {
    let client = client().await;
    let handle = open(&client, FIXTURE).await;
    let args = json!({"handle": handle, "limit": 1, "offset": 1, "columns": ["id"]});
    let (_, doc) = call(&client, "query", args).await;
    assert_eq!(doc["total"], 2);
    assert_eq!(doc["returned"], 1);
    let (_, doc) = call(
        &client,
        "query",
        json!({"handle": handle, "limit": 100_000}),
    )
    .await;
    assert_eq!(doc["limit"], 200);
    let (failed, doc) = call(&client, "query", json!({"handle": handle, "wbs": "NOPE"})).await;
    assert!(failed);
    assert_eq!(doc["error"]["code"], "no-such-wbs");
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn query_filters_by_status_and_type() {
    let client = client().await;
    let handle = open(&client, FIXTURE).await;
    let args = json!({"handle": handle, "status": ["completed"], "type": ["task"]});
    let (failed, doc) = call(&client, "query", args).await;
    assert!(!failed, "{doc}");
    assert_eq!(doc["total"], 0);
    client.cancel().await.unwrap();
}

/// `any` takes groups as objects of `query`'s own filters, typed as they are
///: an activity meets at least one whole group.
#[tokio::test]
async fn query_any_groups_widen() {
    let client = client().await;
    let handle = open(&client, FIXTURE).await;
    let ids = |doc: &Value| -> std::collections::BTreeSet<String> {
        doc["rows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["id"].to_string())
            .collect()
    };
    let mut apart = std::collections::BTreeSet::new();
    for group in [
        json!({"text": "work"}),
        json!({"type": ["finish-milestone"]}),
    ] {
        let mut args = json!({"handle": handle, "columns": ["id"], "limit": 200});
        for (name, value) in group.as_object().unwrap() {
            args[name] = value.clone();
        }
        let (failed, doc) = call(&client, "query", args).await;
        assert!(!failed, "{doc}");
        apart.extend(ids(&doc));
    }
    let args = json!({
        "handle": handle,
        "columns": ["id"],
        "limit": 200,
        "any": [{"text": "work"}, {"type": ["finish-milestone"]}],
    });
    let (failed, doc) = call(&client, "query", args).await;
    assert!(!failed, "{doc}");
    assert_eq!(apart.len(), 2);
    assert_eq!(ids(&doc), apart);
    // A whole group has to be met: critical work isn't there.
    let args = json!({"handle": handle, "any": [{"text": "work", "critical": true}]});
    let (failed, doc) = call(&client, "query", args).await;
    assert!(!failed, "{doc}");
    assert_eq!(doc["total"], 0);
    // An empty group would find everything, so it's refused.
    for empty in [json!({}), json!({"text": ""})] {
        let args = json!({"handle": handle, "any": [empty, {"critical": true}]});
        let (failed, doc) = call(&client, "query", args).await;
        assert!(failed, "{doc}");
    }
    client.cancel().await.unwrap();
}

/// The question an agent is most likely to ask.
#[tokio::test]
async fn what_drives_the_project_finish() {
    let client = client().await;
    let handle = open(&client, LINKED).await;
    let (_, doc) = call(&client, "schedule", json!({"handle": handle})).await;
    assert_eq!(doc["projects"][0]["scheduled_finish"], "2026-01-13 17:00");
    let args = json!({"handle": handle, "activity": "A2", "field": "early_finish"});
    let (failed, doc) = call(&client, "explain", args).await;
    assert!(!failed, "{doc}");
    assert_eq!(doc["chains"][0]["origin"], "data-date");
    let steps = doc["chains"][0]["steps"].as_array().unwrap();
    assert_eq!(steps.last().unwrap()["code"], "A1");

    let args = json!({"handle": handle, "driving_path_to": "A2", "columns": ["code", "depth"]});
    let (_, doc) = call(&client, "query", args).await;
    assert_eq!(doc["rows"], json!([{"code": "A1", "depth": 1}]));
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn check_reports_every_check() {
    let client = client().await;
    let handle = open(&client, LINKED).await;
    let (failed, doc) = call(&client, "check", json!({"handle": handle})).await;
    assert!(!failed, "{doc}");
    assert_eq!(doc["schema"], "totalfloat.check/1");
    assert_eq!(doc["projects"][0]["checks"].as_array().unwrap().len(), 20);
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn float_counts_activities_in_bands() {
    let client = client().await;
    let handle = open(&client, LINKED).await;
    let (failed, _) = call(&client, "schedule", json!({"handle": handle})).await;
    assert!(!failed);
    let (failed, doc) = call(&client, "float", json!({"handle": handle, "bands": [5]})).await;
    assert!(!failed, "{doc}");
    assert_eq!(doc["schema"], "totalfloat.float/1");
    let bands = doc["projects"][0]["bands"].as_array().unwrap();
    assert_eq!(bands.len(), 4);
    assert_eq!(bands[1]["label"], "0 days");
    assert_eq!(bands[1]["activities"], 2);
    let (failed, doc) = call(&client, "float", json!({"handle": handle, "bands": [5, 2]})).await;
    assert!(failed, "{doc}");
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn diff_compares_two_handles() {
    let client = client().await;
    let first = open(&client, LINKED).await;
    let second = open(&client, LINKED).await;
    call(&client, "schedule", json!({"handle": second})).await;
    let args = json!({"from": first, "to": second});
    let (failed, doc) = call(&client, "diff", args).await;
    assert!(!failed, "{doc}");
    assert_eq!(doc["schema"], "totalfloat.diff/1");
    assert_eq!(doc["projects"][0]["counts"]["became_critical"], 2);
    let (failed, doc) = call(&client, "diff", json!({"from": "s9", "to": second})).await;
    assert!(failed);
    assert_eq!(doc["error"]["code"], "no-such-handle");
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn close_ends_a_handle() {
    let client = client().await;
    let handle = open(&client, FIXTURE).await;
    let args = json!({"handle": handle});
    let (failed, _) = call(&client, "close", args.clone()).await;
    assert!(!failed);
    let (failed, doc) = call(&client, "close", args.clone()).await;
    assert!(failed);
    assert_eq!(doc["error"]["code"], "no-such-handle");
    let (failed, doc) = call(&client, "inspect", args).await;
    assert!(failed);
    assert_eq!(doc["error"]["code"], "no-such-handle");
    client.cancel().await.unwrap();
}

async fn raw_call(
    client: &RunningService<RoleClient, ()>,
    name: &str,
    args: Value,
) -> (bool, String) {
    let Value::Object(arguments) = args else {
        panic!("arguments must be an object");
    };
    let request = CallToolRequestParams::new(name.to_owned()).with_arguments(arguments);
    match client.call_tool(request).await {
        Ok(result) => {
            let content = result.content.first().unwrap();
            let text = content.as_text().unwrap().text.clone();
            (result.is_error == Some(true), text)
        }
        Err(error) => (true, error.to_string()),
    }
}

#[tokio::test]
async fn a_misspelled_argument_fails_instead_of_returning_everything() {
    let client = client().await;
    let handle = open(&client, LINKED).await;
    let args = json!({"handle": handle, "driving_path": "A2"});
    let (failed, doc) = call(&client, "query", args).await;
    assert!(failed);
    assert_eq!(doc["error"]["code"], "unknown-argument");
    let (failed, _) = raw_call(&client, "inspect", json!({"handle": handle, "nope": 1})).await;
    assert!(failed);
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn arguments_of_the_wrong_shape_are_errors() {
    let client = client().await;
    let handle = open(&client, LINKED).await;
    let args = json!({"handle": handle, "status": ["done"]});
    let (failed, text) = raw_call(&client, "query", args).await;
    assert!(failed, "{text}");
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn relations_cannot_be_combined() {
    let client = client().await;
    let handle = open(&client, LINKED).await;
    let args = json!({"handle": handle, "preds": "A2", "succs": "A1"});
    let (failed, doc) = call(&client, "query", args).await;
    assert!(failed);
    assert_eq!(doc["error"]["code"], "conflicting-relations");
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn export_survives_the_source_file_moving() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source.xer");
    std::fs::copy(FIXTURE, &source).unwrap();
    let client = client().await;
    let handle = open(&client, source.to_str().unwrap()).await;
    std::fs::remove_file(&source).unwrap();
    let path = directory.path().join("out.xer");
    let args = json!({"handle": handle, "path": path.to_str().unwrap()});
    let (failed, doc) = call(&client, "export", args).await;
    assert!(!failed, "{doc}");
    assert!(path.exists());
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn inspect_reports_the_size_after_scheduling() {
    let client = client().await;
    let handle = open(&client, FIXTURE).await;
    let (_, before) = call(&client, "inspect", json!({"handle": handle})).await;
    call(&client, "schedule", json!({"handle": handle})).await;
    let (_, after) = call(&client, "inspect", json!({"handle": handle})).await;
    assert_ne!(before["bytes"], after["bytes"]);
    client.cancel().await.unwrap();
}

async fn client_of(server: Server) -> RunningService<RoleClient, ()> {
    let (server_side, client_side) = tokio::io::duplex(1 << 20);
    tokio::spawn(async move {
        let service = server.serve(server_side).await.unwrap();
        let _ = service.waiting().await;
    });
    ().serve(client_side).await.unwrap()
}

#[tokio::test]
async fn a_file_is_mapped_once_until_it_changes() {
    let server = Server::new();
    let mappings = std::sync::Arc::clone(&server.mappings);
    let count = || mappings.load(std::sync::atomic::Ordering::Relaxed);
    let client = client_of(server).await;
    let handle = open(&client, LINKED).await;
    assert_eq!(count(), 1);

    for tool in ["inspect", "query", "check"] {
        call(&client, tool, json!({"handle": handle})).await;
    }
    let args = json!({"handle": handle, "activity": "A2"});
    call(&client, "explain", args).await;
    let args = json!({"from": handle, "to": handle});
    call(&client, "diff", args).await;
    assert_eq!(count(), 1);

    call(&client, "schedule", json!({"handle": handle})).await;
    assert_eq!(count(), 2);
    let (_, doc) = call(&client, "query", json!({"handle": handle})).await;
    assert_eq!(doc["rows"][0]["early_start"], "2026-01-12 08:00");
    assert_eq!(count(), 2);
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn work_runs_outside_the_lock() {
    let server = Server::new();
    server.open_file(FIXTURE.to_owned()).await.unwrap();
    let inner = server.clone();
    let closed = server
        .on_session("s1", move |_| Ok(inner.lock()?.open.len()))
        .await;
    assert_eq!(closed.unwrap(), 1);
}

#[tokio::test]
async fn a_panic_in_a_tool_is_an_error_and_leaves_files_open() {
    let server = Server::new();
    server.open_file(FIXTURE.to_owned()).await.unwrap();
    let failed = server
        .on_session::<()>("s1", |_| panic!("a bug in a report"))
        .await;
    assert_eq!(failed.unwrap_err().code, "tool-failed");
    assert!(server.session("s1").is_ok());
}

#[tokio::test]
async fn schedules_on_one_handle_run_one_at_a_time() {
    let server = Server::new();
    let opened = server.open_file(LINKED.to_owned()).await.unwrap();
    let schedule = || super::ScheduleParams {
        handle: opened.handle.clone(),
        project: None,
        data_date: None,
        ignore_external: false,
        float_paths: None,
        float_path_to: None,
        float_paths_by: None,
    };
    let (first, second) = tokio::join!(server.calculate(schedule()), server.calculate(schedule()));
    first.unwrap();
    // The second started from the first's results, so nothing is left to change.
    let second = serde_json::to_value(second.unwrap()).unwrap();
    assert_eq!(second["projects"][0]["changed_activities"], 0);
}

#[tokio::test]
async fn every_refused_edit_comes_back_at_once() {
    let client = client().await;
    let handle = open(&client, LINKED).await;
    let edits = json!([
        {"op": "set_duration", "activity": "A9", "original": "2d"},
        {"op": "set_duration", "activity": "A1", "original": "2d"},
        {"op": "set_name", "activity": "NOPE", "name": "x"}
    ]);
    let (failed, doc) = call(&client, "edit", json!({"handle": handle, "edits": edits})).await;
    assert!(failed, "{doc}");
    assert_eq!(doc["error"]["code"], "edits-refused");
    let problems = doc["error"]["problems"].as_array().unwrap();
    assert_eq!(problems.len(), 2, "{doc}");
    assert_eq!(problems[0]["edit"], 1);
    assert_eq!(problems[0]["source"], "edit 1");
    assert_eq!(problems[1]["edit"], 3);
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn skip_failed_keeps_what_applies() {
    let client = client().await;
    let handle = open(&client, LINKED).await;
    let edits = json!([
        {"op": "set_duration", "activity": "A9", "original": "2d"},
        {"op": "set_duration", "activity": "A1", "original": "15d"}
    ]);
    let args = json!({"handle": handle, "edits": edits, "skip_failed": true});
    let (failed, doc) = call(&client, "edit", args).await;
    assert!(!failed, "{doc}");
    assert_eq!(doc["kept"], true);
    assert_eq!(doc["skipped"][0]["source"], "edit 1");
    assert_eq!(finish(&client, &handle).await.as_f64(), Some(120.0));
    client.cancel().await.unwrap();
}

/// A copy of the linked fixture with its history started through MCP.
async fn store(client: &RunningService<RoleClient, ()>, directory: &std::path::Path) -> String {
    let file = directory.join("plant.xer");
    std::fs::copy(LINKED, &file).unwrap();
    let args = json!({"file": file, "reason": "As submitted"});
    let (failed, doc) = call(client, "history_init", args).await;
    assert!(!failed, "{doc}");
    directory.join("plant.tfdb").display().to_string()
}

#[tokio::test]
async fn an_agent_records_on_a_store_with_a_reason() {
    let client = client().await;
    let directory = tempfile::tempdir().unwrap();
    let store = store(&client, directory.path()).await;
    let handle = open(&client, &store).await;
    let args = json!({"handle": handle, "edits": lengthen()});
    let (failed, doc) = call(&client, "edit", args).await;
    assert!(failed, "{doc}");
    assert_eq!(doc["error"]["code"], "reason-needed");
    let args = json!({"handle": handle, "edits": lengthen(), "reason": "Steel late"});
    let (failed, doc) = call(&client, "edit", args).await;
    assert!(!failed, "{doc}");
    assert_eq!(doc["kept"], true);
    assert_eq!(finish(&client, &handle).await.as_f64(), Some(120.0));
    let (failed, doc) = call(&client, "history_log", json!({"store": store})).await;
    assert!(!failed, "{doc}");
    let latest = &doc["changesets"][0];
    assert_eq!(latest["author_kind"], "agent");
    assert_eq!(latest["message"], "Steel late");
    assert_ne!(latest["author"], "");
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn an_agent_applies_edited_text_as_a_proposal() {
    let client = client().await;
    let directory = tempfile::tempdir().unwrap();
    let store = store(&client, directory.path()).await;
    let handle = open(&client, &store).await;
    let text_path = directory.path().join("plant.tfs");
    let args = json!({"handle": handle, "path": text_path, "format": "tfs"});
    let (failed, doc) = call(&client, "export", args).await;
    assert!(!failed, "{doc}");
    let text = std::fs::read_to_string(&text_path).unwrap();
    let unchanged = json!({"handle": handle, "text": text, "reason": "Nothing"});
    let (failed, doc) = call(&client, "apply_text", unchanged).await;
    assert!(failed, "{doc}");
    assert_eq!(doc["error"]["code"], "nothing-to-change");
    let (line, _) = text.split_once('\n').unwrap();
    assert!(line.starts_with("totalfloat-schedule"));
    let a1 = text
        .lines()
        .find(|l| l.starts_with("activity A1 "))
        .unwrap()
        .to_owned();
    let longer = a1.replacen(" 1d", " 15d", 1).replacen(" 8h", " 15d", 1);
    assert_ne!(a1, longer, "{a1}");
    let edited = text.replace(&a1, &longer);
    let args = json!({"handle": handle, "text": edited, "reason": "Crash it", "propose": true});
    let (failed, doc) = call(&client, "apply_text", args).await;
    assert!(!failed, "{doc}");
    let (failed, doc) = call(&client, "history_proposals", json!({"store": store})).await;
    assert!(!failed, "{doc}");
    let proposal = doc["proposals"][0]["name"].as_str().unwrap().to_owned();
    let accept = crate::history::Action::Accept {
        store: store.clone().into(),
        at: proposal,
        message: Some("Agreed".to_owned()),
        author: Some("Pat".to_owned()),
        json: true,
    };
    let (accepted, _) = crate::output::captured(|| crate::history::run(&accept));
    accepted.unwrap();
    let (failed, doc) = call(&client, "history_proposals", json!({"store": store})).await;
    assert!(!failed, "{doc}");
    assert_eq!(doc["proposals"].as_array().unwrap().len(), 0);
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn an_agent_proposes_and_the_review_tool_accepts() {
    let client = client().await;
    let directory = tempfile::tempdir().unwrap();
    let store = store(&client, directory.path()).await;
    let handle = open(&client, &store).await;
    let args =
        json!({"handle": handle, "edits": lengthen(), "reason": "Crash it", "propose": true});
    let (failed, doc) = call(&client, "edit", args).await;
    assert!(!failed, "{doc}");
    let (failed, doc) = call(&client, "history_proposals", json!({"store": store})).await;
    assert!(!failed, "{doc}");
    let proposal = doc["proposals"][0]["name"].as_str().unwrap().to_owned();
    // The agent that proposed it can't accept it; a person decides.
    let args = json!({"store": store, "proposal": proposal, "decision": "accept", "reason": "ok"});
    let (failed, doc) = call(&client, "history_review", args).await;
    assert!(failed, "{doc}");
    assert_eq!(doc["error"]["code"], "own-proposal");
    let accept = crate::history::Action::Accept {
        store: store.clone().into(),
        at: proposal.clone(),
        message: Some("Agreed".to_owned()),
        author: Some("Pat".to_owned()),
        json: true,
    };
    let (accepted, _) = crate::output::captured(|| crate::history::run(&accept));
    accepted.unwrap();
    let (failed, doc) = call(&client, "history_proposals", json!({"store": store})).await;
    assert!(!failed, "{doc}");
    assert_eq!(doc["proposals"].as_array().unwrap().len(), 0);
    let args = json!({"store": store, "action": "create", "name": "what-if"});
    let (failed, doc) = call(&client, "history_branch", args).await;
    assert!(!failed, "{doc}");
    let (failed, doc) = call(
        &client,
        "history_verify",
        json!({"store": store, "replay": true}),
    )
    .await;
    assert!(!failed, "{doc}");
    assert_eq!(doc["damage"].as_array().unwrap().len(), 0);
    let (failed, doc) = call(&client, "history_revert", json!({"store": store})).await;
    assert!(failed, "{doc}");
    assert_eq!(doc["error"]["code"], "reason-needed");
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn a_handle_behind_its_branch_can_t_record() {
    let client = client().await;
    let directory = tempfile::tempdir().unwrap();
    let store = store(&client, directory.path()).await;
    let handle = open(&client, &store).await;
    let first = open(&client, &store).await;
    let args = json!({"handle": first, "edits": lengthen(), "reason": "One"});
    let (failed, doc) = call(&client, "edit", args).await;
    assert!(!failed, "{doc}");
    // The other handle still holds the import, and main has moved on.
    let args = json!({"handle": handle, "edits": lengthen(), "reason": "Two"});
    let (failed, doc) = call(&client, "edit", args).await;
    assert!(failed, "{doc}");
    assert_eq!(doc["error"]["code"], "branch-moved");
    // A handle on a file can't take a reason it would lose.
    let file = open(&client, LINKED).await;
    let args = json!({"handle": file, "edits": lengthen(), "reason": "Why"});
    let (failed, doc) = call(&client, "edit", args).await;
    assert!(failed, "{doc}");
    assert_eq!(doc["error"]["code"], "not-a-store");
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn a_global_change_finds_what_query_finds_and_changes_each() {
    let client = client().await;
    let handle = open(&client, LINKED).await;
    let filter = json!({"type": ["task"]});
    let mut args = filter.clone();
    args["handle"] = json!(handle);
    args["columns"] = json!(["code"]);
    let (failed, doc) = call(&client, "query", args).await;
    assert!(!failed, "{doc}");
    let found: Vec<_> = doc["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["code"].clone())
        .collect();
    let args = json!({
        "handle": handle, "filter": filter, "set": ["suffix= (rev)"], "dry_run": true
    });
    let (failed, doc) = call(&client, "global_change", args).await;
    assert!(!failed, "{doc}");
    let applied = doc["applied"].as_array().unwrap();
    assert_eq!(applied.len(), found.len(), "{doc}");
    assert_eq!(doc["kept"], false);
    let args = json!({"handle": handle, "filter": {"wbz": "X"}, "set": ["suffix=x"]});
    let (failed, doc) = call(&client, "global_change", args).await;
    assert!(failed);
    assert!(doc.to_string().contains("unknown-argument"), "{doc}");
    // An expression, worked out for each activity, and one of the wrong kind.
    let args = json!({
        "handle": handle, "filter": filter,
        "set": ["remaining_duration=original_duration*0.5"], "dry_run": true
    });
    let (failed, doc) = call(&client, "global_change", args).await;
    assert!(!failed, "{doc}");
    assert!(doc["applied"].to_string().contains("remaining"), "{doc}");
    let args = json!({"handle": handle, "filter": filter, "set": ["duration=early_start"]});
    let (failed, doc) = call(&client, "global_change", args).await;
    assert!(failed);
    assert!(doc.to_string().contains("invalid-expression"), "{doc}");
    // A filter that narrows nothing, or asks for more than activities, is refused.
    for filter in [json!({}), json!({"type": ["task"], "limit": 1})] {
        let args = json!({"handle": handle, "filter": filter, "set": ["suffix=x"]});
        let (failed, doc) = call(&client, "global_change", args).await;
        assert!(failed, "{filter}");
        assert!(
            doc.to_string().contains("invalid-filter")
                || doc.to_string().contains("unknown-argument"),
            "{doc}"
        );
    }
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn usage_gives_a_resource_s_units_per_period() {
    let client = client().await;
    let handle = open(&client, "../../fixtures/xer/usage.xer").await;
    let args = json!({"handle": handle, "resource": "CREW", "by": "month"});
    let (failed, doc) = call(&client, "usage", args).await;
    assert!(!failed, "{doc}");
    assert_eq!(doc["schema"], "totalfloat.usage/1");
    assert_eq!(doc["periods"][0]["total"], 160.0, "{doc}");
    assert_eq!(doc["periods"][0]["limit"], 176.0, "{doc}");
    let args = json!({"handle": handle, "resource": "NOPE"});
    let (failed, doc) = call(&client, "usage", args).await;
    assert!(failed);
    assert!(doc.to_string().contains("no-such-resource"), "{doc}");
    client.cancel().await.unwrap();
}

/// The edit tool takes a resource's limit when it's added and later, as its
/// schema shows, and usage measures against it.
#[tokio::test]
async fn edit_sets_a_resource_s_limit() {
    let client = client().await;
    let tools = client.list_all_tools().await.unwrap();
    let edit = tools.iter().find(|t| t.name == "edit").unwrap();
    let schema = serde_json::to_string(&edit.input_schema).unwrap();
    assert!(schema.contains("set_resource_limit"), "{schema}");
    assert!(schema.contains("max_units_per_hour"), "{schema}");
    let handle = open(&client, "../../fixtures/xer/usage.xer").await;
    let edits = json!([
        {"op": "add_resource", "resource": "ELEC", "name": "Electricians", "max_units_per_hour": 2},
        {"op": "set_resource_limit", "resource": "CREW", "max_units_per_hour": 3},
    ]);
    let (failed, doc) = call(&client, "edit", json!({"handle": handle, "edits": edits})).await;
    assert!(!failed, "{doc}");
    let args = json!({"handle": handle, "resource": "CREW", "by": "month"});
    let (failed, doc) = call(&client, "usage", args).await;
    assert!(!failed, "{doc}");
    // 176 work hours in January at 3 units an hour.
    assert_eq!(doc["periods"][0]["limit"], 528.0, "{doc}");
    client.cancel().await.unwrap();
}

/// The report tool returns what `report` prints for the same template and
/// file.
#[tokio::test]
async fn report_returns_what_the_command_prints() {
    let client = client().await;
    let handle = open(&client, LINKED).await;
    let loaded = crate::loaded::Loaded::new(crate::location::read(LINKED.as_ref()).unwrap());
    let toml = "title = \"Durations\"\ncolumns = [\"code\", \"original_duration_hours\"]\ntotals = [\"original_duration_hours\"]\n";
    let cases = [
        (
            json!({"handle": handle, "template": "critical-path"}),
            crate::report::Template::named("critical-path").unwrap(),
            crate::report::Format::Markdown,
        ),
        (
            json!({"handle": handle, "text": toml, "format": "csv"}),
            crate::report::Template::parse(toml, "text").unwrap(),
            crate::report::Format::Csv,
        ),
    ];
    for (args, template, format) in cases {
        let Value::Object(arguments) = args else {
            unreachable!()
        };
        let request = CallToolRequestParams::new("report").with_arguments(arguments);
        let result = client.call_tool(request).await.unwrap();
        assert_ne!(result.is_error, Some(true));
        let got = &result.content.first().unwrap().as_text().unwrap().text;
        let printed = crate::report::render(&template, (LINKED, &loaded), None, format).unwrap();
        assert_eq!(got, &printed);
    }
    assert!(
        crate::report::render(
            &crate::report::Template::parse(toml, "text").unwrap(),
            (LINKED, &loaded),
            None,
            crate::report::Format::Markdown
        )
        .unwrap()
        .contains("| **Total** |")
    );
    // A file path isn't a built-in name, and one of template or text is needed.
    for (args, code) in [
        (
            json!({"handle": handle, "template": "/etc/passwd"}),
            "no-such-template",
        ),
        (json!({"handle": handle}), "missing-argument"),
        (
            json!({"handle": handle, "template": "look-ahead", "text": toml}),
            "missing-argument",
        ),
        (
            json!({"handle": handle, "text": "title = 1"}),
            "bad-template",
        ),
    ] {
        let (failed, doc) = call(&client, "report", args).await;
        assert!(failed, "{doc}");
        assert_eq!(doc["error"]["code"], code, "{doc}");
    }
    client.cancel().await.unwrap();
}

/// `schedule` with `ignore_external` calculates each project on its own logic,
/// as `schedule --ignore-external` does.
#[tokio::test]
async fn schedule_can_ignore_links_to_other_projects() {
    let text = std::fs::read_to_string("../../fixtures/xer/two-projects.xer").unwrap();
    let from = "%R\t1002\t2\t201\t1\t101\tPR_SS\t-1";
    assert!(text.contains(from));
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("linked.xer");
    std::fs::write(
        &path,
        text.replace(from, "%R\t1002\t2\t201\t1\t101\tPR_FS\t16"),
    )
    .unwrap();
    let client = client().await;
    let finish_of = |doc: &Value| {
        doc["projects"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["code"] == "SYN2")
            .unwrap()["scheduled_finish"]
            .clone()
    };
    let ignored = open(&client, path.to_str().unwrap()).await;
    let (failed, doc) = call(
        &client,
        "schedule",
        json!({"handle": ignored, "ignore_external": true}),
    )
    .await;
    assert!(!failed, "{doc}");
    assert_eq!(doc["ignore_external"], true);
    assert_eq!(finish_of(&doc), "2026-01-06 13:15");
    let counted = open(&client, path.to_str().unwrap()).await;
    let (failed, doc) = call(&client, "schedule", json!({"handle": counted})).await;
    assert!(!failed, "{doc}");
    assert_eq!(finish_of(&doc), "2026-01-15 13:15");
    client.cancel().await.unwrap();
}

/// `schedule` measures float from the opened projects' finish where the
/// projects say so, and counts only the projects it's asked to.
#[tokio::test]
async fn schedule_measures_float_from_the_opened_projects_finish() {
    let text = std::fs::read_to_string("../../fixtures/xer/two-projects.xer").unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("opened.xer");
    assert!(text.contains("SD_Both\tY\tN"));
    std::fs::write(&path, text.replace("SD_Both\tY\tN", "SD_Both\tN\tN")).unwrap();
    let client = client().await;
    let finish_of = |doc: &Value, code: &str| {
        doc["projects"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["code"] == code)
            .unwrap()["scheduled_finish"]
            .clone()
    };
    let together = open(&client, path.to_str().unwrap()).await;
    let (failed, doc) = call(&client, "schedule", json!({"handle": together})).await;
    assert!(!failed, "{doc}");
    assert_eq!(finish_of(&doc, "SYN1"), "2026-01-20 08:00");
    assert_eq!(finish_of(&doc, "SYN2"), "2026-01-20 08:00");
    let alone = open(&client, path.to_str().unwrap()).await;
    let (failed, doc) = call(&client, "schedule", json!({"handle": alone, "project": 2})).await;
    assert!(!failed, "{doc}");
    assert_eq!(finish_of(&doc, "SYN2"), "2026-01-06 13:15");
    client.cancel().await.unwrap();
}

/// `risk` gives what the command gives for the same inputs and seed.
#[tokio::test]
async fn risk_matches_the_command() {
    let client = client().await;
    let handle = open(&client, LINKED).await;
    let (failed, doc) = call(
        &client,
        "risk",
        json!({"handle": handle, "inputs": {"default": {"min": -10, "max": 25}}, "iterations": 300, "seed": 4}),
    )
    .await;
    assert!(!failed, "{doc}");
    let loaded = crate::loaded::Loaded::new(crate::location::read(LINKED.as_ref()).unwrap());
    let inputs: crate::risk::InputsFile =
        serde_json::from_value(json!({"default": {"min": -10, "max": 25}})).unwrap();
    let run = crate::risk::Run {
        project: None,
        iterations: 300,
        seed: 4,
        target: None,
    };
    let cli =
        serde_json::to_value(crate::risk::report(LINKED, &loaded, &inputs, &run).unwrap()).unwrap();
    assert_eq!(doc["percentiles"], cli["percentiles"]);
    assert_eq!(doc["criticality"], cli["criticality"]);
    let (failed, doc) = call(&client, "risk", json!({"handle": handle, "inputs": {}})).await;
    assert!(failed, "{doc}");
    assert_eq!(doc["error"]["code"], "risk-refused");
    client.cancel().await.unwrap();
}

/// `portfolio_show` gives what `portfolio show --json` gives.
#[tokio::test]
async fn portfolio_show_reads_a_portfolio_file() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::copy(
        "../../fixtures/xer/two-projects.xer",
        directory.path().join("two.xer"),
    )
    .unwrap();
    let path = directory.path().join("plan.portfolio.toml");
    std::fs::write(
        &path,
        "schema = \"totalfloat.portfolio/1\"\n[[eps]]\nid = \"ENT\"\nname = \"Enterprise\"\n[[project]]\nfile = \"two.xer\"\neps = \"ENT\"\n",
    )
    .unwrap();
    let client = client().await;
    let (failed, doc) = call(
        &client,
        "portfolio_show",
        json!({"path": path.to_str().unwrap()}),
    )
    .await;
    assert!(!failed, "{doc}");
    assert_eq!(doc["schema"], "totalfloat.portfolio-show/1");
    assert_eq!(doc["nodes"][0]["totals"]["projects"], 2);
    let (failed, doc) = call(&client, "portfolio_show", json!({"path": "nope.toml"})).await;
    assert!(failed);
    assert_eq!(doc["error"]["code"], "portfolio-refused");
    client.cancel().await.unwrap();
}

/// `portfolio_capacity` gives a resource's demand against its supply.
#[tokio::test]
async fn portfolio_capacity_reads_a_portfolio_file() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::copy(
        "../../fixtures/xer/usage.xer",
        directory.path().join("a.xer"),
    )
    .unwrap();
    let path = directory.path().join("plan.portfolio.toml");
    std::fs::write(
        &path,
        "schema = \"totalfloat.portfolio/1\"\n[[eps]]\nid = \"E\"\nname = \"E\"\n[[project]]\nfile = \"a.xer\"\neps = \"E\"\n",
    )
    .unwrap();
    let client = client().await;
    let (failed, doc) = call(
        &client,
        "portfolio_capacity",
        json!({"path": path.to_str().unwrap(), "resource": "crew"}),
    )
    .await;
    assert!(!failed, "{doc}");
    assert_eq!(doc["schema"], "totalfloat.portfolio-capacity/1");
    assert_eq!(doc["resources"][0]["short_name"], "CREW");
    assert_eq!(doc["resources"][0]["periods_over"], 2);
    client.cancel().await.unwrap();
}

/// Choosing a curve, an even spread or typed units as JSON ops through
/// the edit tool writes what the CLI flags do: the `curv_id` cell, and for
/// typed units the marker and spread cells, which read back as typed.
#[tokio::test]
async fn an_assignment_s_spread_is_chosen_through_the_edit_tool() {
    const CURVES: &str = "../../fixtures/xer/curves.xer";
    let client = client().await;
    let directory = tempfile::tempdir().unwrap();
    let cell = |path: &std::path::Path, field: &str| {
        let raw = totalfloat_xer::raw::read(std::fs::read(path).unwrap()).unwrap();
        let table = raw.table("TASKRSRC").unwrap();
        table.rows[0]
            .get(table.field_index(field).unwrap())
            .unwrap()
            .to_owned()
    };
    for (n, (op, curve, spread)) in [
        (
            json!({"op": "set_curve", "activity": "A1", "resource": "R1", "curve": "Linear"}),
            "1",
            "",
        ),
        (
            json!({"op": "set_curve", "activity": "A1", "resource": "R1"}),
            "",
            "",
        ),
        (
            json!({
                "op": "set_spread", "activity": "A1", "resource": "R1",
                "period": "day", "start": "2026-01-05", "units": [12, 4, 0, 12, 12]
            }),
            "9",
            "12:8;4:8;0:8;12:8;12:8",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let handle = open(&client, CURVES).await;
        let (failed, doc) = call(&client, "edit", json!({"handle": handle, "edits": [op]})).await;
        assert!(!failed, "{doc}");
        let path = directory.path().join(format!("{n}.xer"));
        let (failed, doc) = call(&client, "export", json!({"handle": handle, "path": path})).await;
        assert!(!failed, "{doc}");
        assert_eq!(cell(&path, "curv_id"), curve, "{op}");
        assert_eq!(cell(&path, "remain_crv"), spread, "{op}");
        let raw = totalfloat_xer::raw::read(std::fs::read(&path).unwrap()).unwrap();
        let read = totalfloat_xer::to_model(&raw).0.swap_remove(0);
        let typed = read
            .assignments
            .values()
            .next()
            .unwrap()
            .typed_spread
            .clone();
        assert_eq!(typed.is_some(), !spread.is_empty(), "{op}");
    }
    let handle = open(&client, CURVES).await;
    let wrong = json!([{"op": "set_curve", "activity": "A1", "resource": "R1", "curve": "Lineer"}]);
    let (failed, doc) = call(&client, "edit", json!({"handle": handle, "edits": wrong})).await;
    assert!(failed, "{doc}");
    let problem = &doc["error"]["problems"][0];
    assert_eq!(problem["code"], "no-such-curve", "{doc}");
    assert!(
        problem["hint"].as_str().unwrap().contains("Linear"),
        "{doc}"
    );
    client.cancel().await.unwrap();
}
