mod common;

use common::*;
use serde_json::{Value, json};
use std::{ffi::OsString, io::Cursor, process::Command};

fn args(values: &[&str]) -> Vec<OsString> {
    values.iter().map(OsString::from).collect()
}

fn cli(repo: &Repo, values: &[&str]) -> anyhow::Result<(i32, String)> {
    let mut output = Vec::new();
    let code = wtw::run_cli_at(args(values), &repo.root, &mut Cursor::new(Vec::<u8>::new()), &mut output)?;
    Ok((code, String::from_utf8(output).unwrap()))
}

#[test]
fn cli_covers_the_complete_daily_contract_without_manual_add() {
    let repo = Repo::initialized();
    repo.write("docs/architecture.md", &source().content);
    repo.configure_same(&linked_pair(), &[]);
    let (code, collected) = cli(
        &repo,
        &[
            "wtw",
            "collect",
            "--task",
            "Adopt the accepted backend architecture",
            "--source",
            "docs/architecture.md",
            "--json",
        ],
    )
    .unwrap();
    assert_eq!(code, 0);
    assert_eq!(
        serde_json::from_str::<Value>(&collected).unwrap()["recorded"]
            .as_array()
            .unwrap()
            .len(),
        2
    );

    let (_, explained) = cli(
        &repo,
        &[
            "wtw",
            "explain",
            "--task",
            "backend persistence",
            "--path",
            "src/backend/order.rs",
            "--json",
        ],
    )
    .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&explained).unwrap()["records"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert!(
        cli(&repo, &["wtw", "show", "--id", "direct-appdb", "--json"])
            .unwrap()
            .1
            .contains("AppDb")
    );
    assert_eq!(cli(&repo, &["wtw", "health", "--json"]).unwrap().0, 0);
    assert!(cli(&repo, &["wtw", "export"]).unwrap().1.contains("wtw://decision/direct-appdb"));

    let help = Command::new(env!("CARGO_BIN_EXE_wtw")).arg("--help").output().unwrap();
    let help = String::from_utf8(help.stdout).unwrap();
    assert!(help.contains("collect") && help.contains("guard"));
    assert!(!help.lines().any(|line| line.trim_start().starts_with("add")));
}

#[test]
fn guard_and_health_exit_codes_are_machine_safe() {
    let repo = Repo::initialized();
    repo.configure_same(&linked_pair(), &[]);
    wtw::collect(&repo.root, request()).unwrap();
    repo.write("src/backend/handler.rs", "repository.save(entity);\n");
    repo.configure_same(&[], &[finding("wtw://decision/direct-appdb")]);
    assert_eq!(
        cli(&repo, &["wtw", "guard", "--task", "change persistence", "--json"]).unwrap().0,
        1
    );
    assert_eq!(cli(&repo, &["wtw", "health", "--suite", "--json"]).unwrap().0, 1);
}

#[test]
fn mcp_lists_and_calls_the_same_core_operations() {
    let repo = Repo::initialized();
    repo.configure_same(&[decision("direct-appdb")], &[]);
    wtw::collect(&repo.root, request()).unwrap();
    let requests = [
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
        json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"wtw_show","arguments":{"repository":repo.root,"id":"direct-appdb"}}}),
        json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"wtw_export","arguments":{"repository":repo.root}}}),
    ];
    let input = requests.iter().map(Value::to_string).collect::<Vec<_>>().join("\n");
    let mut output = Vec::new();
    wtw::mcp_stream(&mut Cursor::new(input), &mut output).unwrap();
    let responses = String::from_utf8(output)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(responses.len(), 4);
    assert_eq!(responses[1]["result"]["tools"].as_array().unwrap().len(), 6);
    assert_eq!(responses[2]["result"]["structuredContent"]["id"], "direct-appdb");
    assert_eq!(responses[3]["result"]["structuredContent"]["nodes"].as_array().unwrap().len(), 1);
}

#[test]
fn packaged_entrypoint_reaches_the_shared_cli() {
    let output = Command::new(env!("CARGO_BIN_EXE_wtw")).arg("--version").output().unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        format!("wtw {}", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn cli_initializes_supersedes_and_reads_federated_graph_files() {
    let bare = Repo::bare();
    assert_eq!(cli(&bare, &["wtw", "init"]).unwrap().0, 0);
    assert!(bare.root.join(".wtw/SKILL.md").is_file());

    let repo = Repo::initialized();
    repo.configure_same(&[decision("old-way"), decision("new-way")], &[]);
    wtw::collect(&repo.root, request()).unwrap();
    assert_eq!(
        cli(
            &repo,
            &[
                "wtw",
                "supersede",
                "--id",
                "old-way",
                "--by",
                "new-way",
                "--basis",
                "Accepted replacement",
                "--json",
            ],
        )
        .unwrap()
        .0,
        0
    );

    repo.write("graph.json", &json!({"schema":1,"nodes":[],"edges":[]}).to_string());
    assert_eq!(cli(&repo, &["wtw", "health", "--graph", "graph.json", "--json"]).unwrap().0, 0);
    repo.write("bad-graph.json", r#"{"schema":2,"nodes":[],"edges":[]}"#);
    assert!(cli(&repo, &["wtw", "health", "--graph", "bad-graph.json"]).is_err());
    assert!(cli(&repo, &["wtw", "collect", "--task", "bad source", "--source", ".wtw/SKILL.md",],).is_err());
    assert_eq!(cli(&repo, &["wtw", "--help"]).unwrap().0, 0);
    assert!(cli(&repo, &["wtw", "unknown-command"]).is_err());
}

#[test]
fn mcp_covers_notifications_errors_and_every_operation() {
    let repo = Repo::initialized();
    repo.configure_same(&[decision("direct-appdb")], &[]);
    let requests = [
        json!({"jsonrpc":"2.0","method":"notifications/initialized","params":{}}),
        json!({"jsonrpc":"2.0","id":1,"method":"ping","params":{}}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"wtw_collect","arguments":{"repository":repo.root,"task":"Adopt the accepted backend architecture","sources":[source()]}}}),
        json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"wtw_explain","arguments":{"repository":repo.root,"task":"backend","paths":["src/backend/x.rs"],"limit":5}}}),
        json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"wtw_guard","arguments":{"repository":repo.root,"task":"backend"}}}),
        json!({"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"wtw_health","arguments":{"repository":repo.root}}}),
        json!({"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"missing","arguments":{"repository":repo.root}}}),
        json!({"jsonrpc":"2.0","id":7,"method":"missing","params":{}}),
    ];
    let input = requests.iter().map(Value::to_string).collect::<Vec<_>>().join("\n");
    let mut output = Vec::new();
    wtw::mcp_stream(&mut Cursor::new(input), &mut output).unwrap();
    let responses = String::from_utf8(output)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(responses.len(), 7);
    assert_eq!(responses[0]["result"], json!({}));
    assert_eq!(responses[1]["result"]["structuredContent"]["recorded"].as_array().unwrap().len(), 1);
    assert_eq!(responses[2]["result"]["structuredContent"]["records"].as_array().unwrap().len(), 1);
    assert_eq!(responses[3]["result"]["structuredContent"]["findings"], json!([]));
    assert_eq!(responses[4]["result"]["structuredContent"]["passed"], true);
    assert_eq!(responses[5]["result"]["isError"], true);
    assert_eq!(responses[6]["error"]["code"], -32601);
}
