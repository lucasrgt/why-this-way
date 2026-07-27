mod common;

use common::*;
use std::{fs, path::PathBuf};
use wtw::{
    CollectRequest, Graph, GraphEdge, GraphNode, RecordKind, RecordStatus, Source, collect, explain, export_graph, guard, health, show,
    supersede,
};

#[test]
fn init_collect_retrieve_and_export_form_one_versioned_graph() {
    let repo = Repo::bare();
    wtw::init(&repo.root, &[PathBuf::from("AGENTS.md")]).unwrap();
    wtw::init(&repo.root, &[PathBuf::from("AGENTS.md")]).unwrap();
    assert!(repo.root.join(".agent-first/wtw/records/decisions").is_dir());
    assert_eq!(
        fs::read_to_string(repo.root.join("AGENTS.md"))
            .unwrap()
            .matches("<!-- wtw:instructions:start -->")
            .count(),
        1
    );

    repo.configure_same(&linked_pair(), &[]);
    let result = collect(&repo.root, request()).unwrap();
    assert_eq!(result.recorded.len(), 2);
    assert_eq!(collect(&repo.root, request()).unwrap().duplicates, 2);

    let decision = show(&repo.root, "direct-appdb").unwrap();
    assert_eq!(decision.kind, RecordKind::Decision);
    let stored = fs::read_to_string(repo.root.join(".agent-first/wtw/records/decisions/direct-appdb.toml")).unwrap();
    assert!(stored.contains("[[links]]") && stored.contains("rel = \"upholds\""));

    let context = explain(&repo.root, "change backend persistence", &["src/backend/orders.rs".into()], 12, &[]).unwrap();
    assert_eq!(context.records.len(), 2);
    assert_eq!(context.edges.len(), 1);
    let graph = export_graph(&repo.root).unwrap();
    assert_eq!((graph.nodes.len(), graph.edges.len()), (2, 1));
}

#[test]
fn collection_fails_closed_on_bad_evidence_shape_links_and_disagreement() {
    let repo = Repo::initialized();
    let mut bad = decision("bad-evidence");
    bad.authority.quote = "invented authority".into();
    repo.configure_same(&[bad], &[]);
    assert!(collect(&repo.root, request()).unwrap_err().to_string().contains("literal evidence"));

    let mut bad = invariant("bad-link");
    bad.links.push(wtw::Link {
        rel: wtw::Relation::Establishes,
        to: "wtw://decision/direct-appdb".into(),
        basis: "Wrong direction".into(),
    });
    repo.configure_same(&[bad], &[]);
    assert!(collect(&repo.root, request()).unwrap_err().to_string().contains("does not exist"));

    repo.configure(&[decision("not-confirmed")], &[], &[], &[]);
    assert!(collect(&repo.root, request()).unwrap().recorded.is_empty());
}

#[test]
fn supersession_preserves_history_and_health_validates_relation_kinds() {
    let repo = Repo::initialized();
    repo.configure_same(&[decision("direct-appdb"), decision("direct-appdb-v2")], &[]);
    collect(&repo.root, request()).unwrap();
    supersede(
        &repo.root,
        "wtw://decision/direct-appdb",
        "wtw://decision/direct-appdb-v2",
        "The accepted v2 decision replaces the original",
    )
    .unwrap();
    assert_eq!(show(&repo.root, "direct-appdb").unwrap().status, RecordStatus::Superseded);
    assert!(health(&repo.root, &[], false).unwrap().passed);

    let invalid = Graph {
        schema: 1,
        nodes: vec![GraphNode {
            uri: "avp://proof/x".into(),
            kind: "proof".into(),
            status: "active".into(),
            title: "Proof".into(),
            scopes: vec![],
        }],
        edges: vec![GraphEdge {
            from: "avp://proof/x".into(),
            rel: "records_violation_of".into(),
            to: "wtw://decision/direct-appdb-v2".into(),
            basis: "Wrong semantic relation".into(),
        }],
    };
    let result = health(&repo.root, &[invalid], false).unwrap();
    assert!(!result.passed);
    assert!(result.issues.iter().any(|issue| issue.code == "incompatible_link"));
}

#[test]
fn suite_health_requires_a_resolved_active_proof_for_every_active_invariant() {
    let repo = Repo::initialized();
    repo.configure_same(&[invariant("module-write-ownership")], &[]);
    collect(&repo.root, request()).unwrap();
    let missing = health(&repo.root, &[], true).unwrap();
    assert_eq!(missing.issues[0].code, "unproved_invariant");

    let proof = Graph {
        schema: 1,
        nodes: vec![GraphNode {
            uri: "avp://proof/module-write-ownership".into(),
            kind: "proof".into(),
            status: "active".into(),
            title: "Write ownership proof".into(),
            scopes: vec!["src/backend/**".into()],
        }],
        edges: vec![GraphEdge {
            from: "avp://proof/module-write-ownership".into(),
            rel: "proves".into(),
            to: "wtw://invariant/module-write-ownership".into(),
            basis: "The criterion fails on cross-module writes".into(),
        }],
    };
    assert!(health(&repo.root, &[proof], true).unwrap().passed);
}

#[test]
fn guard_uses_relevant_records_and_two_identical_judges() {
    let repo = Repo::initialized();
    repo.configure_same(&linked_pair(), &[]);
    collect(&repo.root, request()).unwrap();
    repo.write("src/backend/handler.rs", "repository.save(entity);\n");
    let expected = finding("wtw://decision/direct-appdb");
    repo.configure_same(&[], std::slice::from_ref(&expected));
    let result = guard(&repo.root, "change persistence", "HEAD", &[], &[], false).unwrap();
    assert_eq!(result.findings, vec![expected]);

    repo.configure(&[], &[], &[finding("wtw://decision/direct-appdb")], &[]);
    assert!(
        guard(&repo.root, "change persistence", "HEAD", &[], &[], false)
            .unwrap()
            .findings
            .is_empty()
    );
}

#[test]
fn corrupt_storage_and_unsafe_inputs_fail_closed() {
    let repo = Repo::initialized();
    assert!(explain(&repo.root, "", &[], 12, &[]).is_err());
    assert!(explain(&repo.root, "task", &[], 0, &[]).is_err());
    assert!(guard(&repo.root, "task", "--bad", &[], &[], false).is_err());
    repo.write(".agent-first/wtw/records/invariants/broken.toml", "schema = 99\n");
    assert!(export_graph(&repo.root).is_err());
}

#[test]
fn collection_rejects_every_untrusted_boundary_and_conflicting_identity() {
    let repo = Repo::initialized();
    let duplicated = CollectRequest {
        task: "task".into(),
        sources: vec![
            Source {
                name: "same".into(),
                content: "first source".into(),
            },
            Source {
                name: "same".into(),
                content: "second source".into(),
            },
        ],
        base: "HEAD".into(),
    };
    assert!(
        collect(&repo.root, duplicated)
            .unwrap_err()
            .to_string()
            .contains("duplicate source")
    );

    let oversized = CollectRequest {
        task: "task".into(),
        sources: vec![Source {
            name: "huge".into(),
            content: "x".repeat(170_000),
        }],
        base: "HEAD".into(),
    };
    assert!(collect(&repo.root, oversized).unwrap_err().to_string().contains("exceeds"));

    repo.configure_same(&[decision("stable-id")], &[]);
    collect(&repo.root, request()).unwrap();
    let mut changed = decision("stable-id");
    changed.title = "Different meaning".into();
    repo.configure_same(&[changed], &[]);
    assert!(
        collect(&repo.root, request())
            .unwrap_err()
            .to_string()
            .contains("different content")
    );

    let invalids = [
        {
            let mut item = decision("valid-before-change");
            item.id = "UPPERCASE".into();
            item
        },
        {
            let mut item = decision("missing-scope");
            item.scopes.clear();
            item
        },
        {
            let mut item = decision("invented-evidence");
            item.evidence[0] = "this was never in any source".into();
            item
        },
        {
            let mut item = decision("bad-decision");
            item.alternatives.clear();
            item
        },
        {
            let mut item = invariant("bad-invariant");
            item.alternatives = decision("source").alternatives;
            item
        },
    ];
    for item in invalids {
        repo.configure_same(&[item], &[]);
        assert!(collect(&repo.root, request()).is_err());
    }
    repo.configure_same(&vec![decision("many"); 25], &[]);
    assert!(collect(&repo.root, request()).unwrap_err().to_string().contains("too many"));
}

#[test]
fn graph_health_reports_dangling_empty_and_supports_every_suite_relation() {
    let repo = Repo::initialized();
    repo.configure_same(&linked_pair(), &[]);
    collect(&repo.root, request()).unwrap();
    let graph = Graph {
        schema: 1,
        nodes: vec![
            GraphNode {
                uri: "rtw://example/handler".into(),
                kind: "example".into(),
                status: "active".into(),
                title: "Handler".into(),
                scopes: vec![],
            },
            GraphNode {
                uri: "nya://scar/cross-write".into(),
                kind: "scar".into(),
                status: "active".into(),
                title: "Cross write".into(),
                scopes: vec![],
            },
            GraphNode {
                uri: "wmw://deferment/migrate".into(),
                kind: "deferment".into(),
                status: "active".into(),
                title: "Migration".into(),
                scopes: vec![],
            },
        ],
        edges: vec![
            GraphEdge {
                from: "rtw://example/handler".into(),
                rel: "exemplifies".into(),
                to: "wtw://decision/direct-appdb".into(),
                basis: "Canonical handler".into(),
            },
            GraphEdge {
                from: "nya://scar/cross-write".into(),
                rel: "records_violation_of".into(),
                to: "wtw://invariant/module-write-ownership".into(),
                basis: "Historical violation".into(),
            },
            GraphEdge {
                from: "wmw://deferment/migrate".into(),
                rel: "tracks_blocker_for".into(),
                to: "wtw://decision/direct-appdb".into(),
                basis: "Blocked migration".into(),
            },
            GraphEdge {
                from: "rtw://example/handler".into(),
                rel: "exemplifies".into(),
                to: "wtw://invariant/missing".into(),
                basis: String::new(),
            },
        ],
    };
    let result = health(&repo.root, &[graph], false).unwrap();
    assert!(result.issues.iter().any(|issue| issue.code == "dangling_link"));
    assert!(result.issues.iter().any(|issue| issue.code == "empty_basis"));
}

#[test]
fn guard_short_circuits_clean_work_and_rejects_invalid_findings_and_runner_config() {
    let repo = Repo::initialized();
    repo.configure_same(&[decision("direct-appdb")], &[]);
    collect(&repo.root, request()).unwrap();
    assert!(
        guard(&repo.root, "persistence", "HEAD", &[], &[], false)
            .unwrap()
            .findings
            .is_empty()
    );

    repo.write("src/backend/handler.rs", "repository.save(entity);\n");
    let mut invalid = finding("wtw://decision/direct-appdb");
    invalid.line = 0;
    repo.configure_same(&[], &[invalid]);
    assert!(guard(&repo.root, "persistence", "HEAD", &["src/backend/handler.rs".into()], &[], false).is_err());

    repo.write(".agent-first/wtw/config.local.toml", "schema = 1\n[judge]\ncommand = []\n");
    assert!(
        guard(&repo.root, "persistence", "HEAD", &[], &[], false)
            .unwrap_err()
            .to_string()
            .contains("empty judge")
    );
    assert!(show(&repo.root, "missing").is_err());
}

#[test]
fn supersession_rejects_self_and_cross_kind_and_retired_records_export() {
    let repo = Repo::initialized();
    repo.configure_same(&[decision("one"), invariant("rule")], &[]);
    collect(&repo.root, request()).unwrap();
    assert!(supersede(&repo.root, "one", "one", "self").is_err());
    assert!(supersede(&repo.root, "rule", "one", "cross kind").is_err());

    let path = repo.root.join(".agent-first/wtw/records/invariants/rule.toml");
    let current = fs::read_to_string(&path)
        .unwrap()
        .replace("status = \"active\"", "status = \"retired\"");
    fs::write(path, current).unwrap();
    assert_eq!(
        export_graph(&repo.root)
            .unwrap()
            .nodes
            .into_iter()
            .find(|node| node.uri.ends_with("/rule"))
            .unwrap()
            .status,
        "retired"
    );
}
