use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
use tempfile::TempDir;
use wtw::{Alternative, Authority, AuthorityKind, Candidate, CollectRequest, Finding, Link, RecordKind, Relation, Source};

pub struct Repo {
    _temp: TempDir,
    judge_dir: TempDir,
    pub root: PathBuf,
}

#[derive(Serialize)]
struct Records<'a> {
    records: &'a [Candidate],
}

#[derive(Serialize)]
struct Findings<'a> {
    findings: &'a [Finding],
}

impl Repo {
    pub fn bare() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().to_path_buf();
        git(&root, &["init", "-q"]);
        git(&root, &["config", "user.name", "Test User"]);
        git(&root, &["config", "user.email", "test@example.com"]);
        git(&root, &["config", "core.autocrlf", "false"]);
        fs::write(root.join("AGENTS.md"), "# Instructions\n").unwrap();
        fs::write(root.join("baseline.txt"), "baseline\n").unwrap();
        git(&root, &["add", "."]);
        git(&root, &["commit", "-qm", "initial"]);
        Self {
            _temp: temp,
            judge_dir: tempfile::tempdir().unwrap(),
            root,
        }
    }

    pub fn initialized() -> Self {
        let repo = Self::bare();
        wtw::init(&repo.root, &[PathBuf::from("AGENTS.md")]).unwrap();
        git(&repo.root, &["add", "."]);
        git(&repo.root, &["commit", "-qm", "adopt wtw"]);
        repo
    }

    pub fn configure(&self, collect_first: &[Candidate], collect_second: &[Candidate], guard_first: &[Finding], guard_second: &[Finding]) {
        let guard_first_evidence = guard_first.first().map(|finding| finding.evidence.as_str()).unwrap_or("");
        let guard_second_evidence = guard_second.first().map(|finding| finding.evidence.as_str()).unwrap_or("");
        let collect_first = serde_json::to_string(&Records { records: collect_first }).unwrap();
        let collect_second = serde_json::to_string(&Records { records: collect_second }).unwrap();
        let guard_first = serde_json::to_string(&Findings { findings: guard_first }).unwrap();
        let guard_second = serde_json::to_string(&Findings { findings: guard_second }).unwrap();
        let source = format!(
            r#"use std::io::Read;
fn main() {{
    let mut prompt = String::new();
    std::io::stdin().read_to_string(&mut prompt).unwrap();
    assert!(prompt.len() <= 800_000);
    let is_guard = prompt.contains("bounded Why This Way guard");
    if !is_guard {{
        assert!(prompt.contains("Every record uses exactly these fields"));
        assert!(prompt.contains("rejected_because"));
        assert!(prompt.contains("Authority uses exactly kind, source, quote"));
        assert!(prompt.contains("source must be an exact top-level key from SOURCES"));
        assert!(prompt.contains("Evidence is an array of literal strings"));
        assert!(prompt.contains("Link entries use exactly rel, to, basis"));
        assert!(prompt.contains("full wtw://decision/<id> or wtw://invariant/<id> URI"));
    }}
    let output = if is_guard {{
        if prompt.contains("Confirm only") {{
            if prompt.contains({guard_second_evidence:?}) {{ {guard_second:?} }} else {{ "{{\"findings\":[]}}" }}
        }} else if prompt.contains({guard_first_evidence:?}) {{ {guard_first:?} }} else {{ "{{\"findings\":[]}}" }}
    }} else if prompt.contains("Confirm only") {{ {collect_second:?} }} else {{ {collect_first:?} }};
    print!("{{}}", output);
}}"#
        );
        let source_path = self.judge_dir.path().join("judge.rs");
        let binary = self.judge_dir.path().join(if cfg!(windows) { "judge.exe" } else { "judge" });
        fs::write(&source_path, source).unwrap();
        assert!(
            Command::new("rustc")
                .arg(&source_path)
                .arg("-o")
                .arg(&binary)
                .status()
                .unwrap()
                .success()
        );
        let quoted = serde_json::to_string(&binary.to_string_lossy()).unwrap();
        fs::write(
            self.root.join(".agent-first/wtw/config.local.toml"),
            format!("schema = 1\n[judge]\ncommand = [{quoted}]\n"),
        )
        .unwrap();
    }

    pub fn configure_same(&self, candidates: &[Candidate], findings: &[Finding]) {
        self.configure(candidates, candidates, findings, findings);
    }

    pub fn write(&self, path: &str, content: &str) {
        let path = self.root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }
}

pub fn source() -> Source {
    Source {
        name: "docs/architecture.md".into(),
        content: "We choose direct\nAppDb access because repository abstractions obscure\nslice ownership. The repository-per-entity alternative was explicitly rejected. Each module writes only its own entities; a cross-module write is invalid.".into(),
    }
}

pub fn request() -> CollectRequest {
    CollectRequest {
        task: "Adopt the accepted backend architecture".into(),
        sources: vec![source()],
        base: "HEAD".into(),
    }
}

pub fn decision(id: &str) -> Candidate {
    Candidate {
        id: id.into(),
        kind: RecordKind::Decision,
        title: "Use AppDb directly".into(),
        statement: "Handlers access AppDb directly".into(),
        rationale: "Repository abstractions obscure slice ownership".into(),
        alternatives: vec![Alternative {
            statement: "Repository per entity".into(),
            rejected_because: "It obscures slice ownership".into(),
        }],
        violation: String::new(),
        scopes: vec!["src/backend/**".into()],
        authority: Authority {
            kind: AuthorityKind::Adr,
            source: "docs/architecture.md".into(),
            quote: "We choose direct AppDb access".into(),
        },
        evidence: vec![
            "repository abstractions obscure slice ownership".into(),
            "repository-per-entity alternative was explicitly rejected".into(),
        ],
        links: vec![],
    }
}

pub fn invariant(id: &str) -> Candidate {
    Candidate {
        id: id.into(),
        kind: RecordKind::Invariant,
        title: "Module write ownership".into(),
        statement: "Each module writes only its own entities".into(),
        rationale: "Keep ownership explicit".into(),
        alternatives: vec![],
        violation: "One module writes another module's entity".into(),
        scopes: vec!["src/backend/**".into()],
        authority: Authority {
            kind: AuthorityKind::Adr,
            source: "docs/architecture.md".into(),
            quote: "Each module writes only its own entities".into(),
        },
        evidence: vec![
            "Each module writes only its own entities".into(),
            "a cross-module write is invalid".into(),
        ],
        links: vec![],
    }
}

pub fn linked_pair() -> Vec<Candidate> {
    let invariant = invariant("module-write-ownership");
    let mut decision = decision("direct-appdb");
    decision.links.push(Link {
        rel: Relation::Upholds,
        to: "wtw://invariant/module-write-ownership".into(),
        basis: "Direct access keeps the writing module visible".into(),
    });
    vec![decision, invariant]
}

pub fn finding(uri: &str) -> Finding {
    Finding {
        record_uri: uri.into(),
        path: "src/backend/handler.rs".into(),
        line: 1,
        evidence: "repository.save(entity);".into(),
        reason: "The repository hides direct write ownership".into(),
    }
}

pub fn git(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git").arg("-C").arg(root).args(args).output().unwrap();
    assert!(output.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&output.stderr));
    String::from_utf8(output.stdout).unwrap().trim().into()
}
