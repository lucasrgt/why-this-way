use anyhow::{Context, Result, anyhow, bail};
use chrono::{DateTime, Utc};
use clap::{Args, Parser, Subcommand};
use glob::Pattern;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
#[rustfmt::skip]
use std::{collections::{BTreeMap, HashMap, HashSet}, ffi::OsString, fs, io::{self, BufRead, Write}, path::{Component, Path, PathBuf}, process::{Command, Stdio}};
use tempfile::tempdir;
use ulid::Ulid;

const ROOT: &str = ".wtw";
const LEGACY_ROOT: &str = ".agent-first/wtw";
const SKILL: &str = include_str!("../assets/why-this-way/SKILL.md");
const CONFIG: &str = include_str!("../assets/config.toml");
const IGNORE: &str = include_str!("../assets/gitignore");
const INSTRUCTIONS: &str = include_str!("../assets/AGENT_INSTRUCTIONS.md");
const START: &str = "<!-- wtw:instructions:start -->";
const END: &str = "<!-- wtw:instructions:end -->";
const MAX_PROMPT: usize = 800_000;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
#[rustfmt::skip]
pub enum RecordKind { Decision, Invariant }

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[rustfmt::skip]
pub enum RecordStatus { Active, Superseded, Retired }

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
#[rustfmt::skip]
pub enum Relation { Establishes, Upholds, Supersedes }

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[rustfmt::skip]
pub enum AuthorityKind { HumanStatement, AcceptedPlan, Adr, Policy, Contract, MergedChange }

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[rustfmt::skip]
pub struct Alternative { pub statement: String, pub rejected_because: String }

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[rustfmt::skip]
pub struct Authority { pub kind: AuthorityKind, pub source: String, pub quote: String }

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
#[rustfmt::skip]
pub struct Link { pub rel: Relation, pub to: String, pub basis: String }

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
#[rustfmt::skip]
pub struct Candidate { pub id: String, pub kind: RecordKind, pub title: String, pub statement: String, #[serde(default)] pub rationale: String, #[serde(default)] pub alternatives: Vec<Alternative>, #[serde(default)] pub violation: String, pub scopes: Vec<String>, pub authority: Authority, pub evidence: Vec<String>, #[serde(default)] pub links: Vec<Link> }

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
#[rustfmt::skip]
pub struct Record { pub schema: u8, pub id: String, pub kind: RecordKind, pub status: RecordStatus, pub title: String, pub statement: String, #[serde(default, skip_serializing_if = "String::is_empty")] pub rationale: String, #[serde(default, skip_serializing_if = "Vec::is_empty")] pub alternatives: Vec<Alternative>, #[serde(default, skip_serializing_if = "String::is_empty")] pub violation: String, pub scopes: Vec<String>, pub authority: Authority, pub evidence: Vec<String>, #[serde(default, skip_serializing_if = "Vec::is_empty")] pub links: Vec<Link>, pub recorded_at: DateTime<Utc>, pub recorded_by: String, pub recorded_commit: String }

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[rustfmt::skip]
pub struct GraphNode { pub uri: String, pub kind: String, pub status: String, pub title: String, pub scopes: Vec<String> }

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
#[rustfmt::skip]
pub struct GraphEdge { pub from: String, pub rel: String, pub to: String, pub basis: String }

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[rustfmt::skip]
pub struct Graph { pub schema: u8, pub nodes: Vec<GraphNode>, pub edges: Vec<GraphEdge> }

#[derive(Clone, Debug, Serialize, PartialEq)]
#[rustfmt::skip]
pub struct ExplainResult { pub records: Vec<Record>, pub edges: Vec<GraphEdge> }

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[rustfmt::skip]
pub struct Finding { pub record_uri: String, pub path: String, pub line: u64, pub evidence: String, pub reason: String }

#[derive(Clone, Debug, Serialize, PartialEq)]
#[rustfmt::skip]
pub struct GuardResult { pub records_checked: usize, pub findings: Vec<Finding>, pub health: HealthResult }

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[rustfmt::skip]
pub struct HealthIssue { pub code: String, pub message: String }

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[rustfmt::skip]
pub struct HealthResult { pub passed: bool, pub records: usize, pub issues: Vec<HealthIssue> }

#[derive(Clone, Debug, Default)]
#[rustfmt::skip]
pub struct CollectRequest { pub task: String, pub sources: Vec<Source>, pub base: String }

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[rustfmt::skip]
pub struct Source { pub name: String, pub content: String }

#[derive(Clone, Debug, Serialize, PartialEq)]
#[rustfmt::skip]
pub struct CollectResult { pub candidates_found: usize, pub duplicates: usize, pub recorded: Vec<Record> }

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[rustfmt::skip]
struct Config { schema: u8, judge: Judge }
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[rustfmt::skip]
struct Judge { command: Vec<String> }
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[rustfmt::skip]
struct Extraction { records: Vec<Candidate> }
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[rustfmt::skip]
struct Audit { findings: Vec<Finding> }

impl Record {
    pub fn uri(&self) -> String {
        format!("wtw://{}/{}", kind_name(self.kind), self.id)
    }
}

pub fn repository(path: &Path) -> Result<PathBuf> {
    fs::canonicalize(git(path, &["rev-parse", "--show-toplevel"])?.trim()).context("resolve repository root")
}

#[rustfmt::skip]
fn data_dir(root: &Path) -> PathBuf { match std::env::var_os("CSM_STORAGE_ROOT") { Some(path) => { let path = PathBuf::from(path); let path = if path.is_absolute() { path } else { root.join(path) }; path.join("wtw") }, None => root.join(ROOT) } }

#[rustfmt::skip]
fn store_exclude(root: &Path) -> String { let relative = data_dir(root).strip_prefix(root).ok().map(|path| path.to_string_lossy().replace('\\', "/")).unwrap_or_else(|| "__csm_external_store__".into()); format!(":(exclude){relative}/**") }
fn migrate_legacy_root(root: &Path) -> Result<()> {
    let legacy = root.join(LEGACY_ROOT);
    if !legacy.exists() {
        return Ok(());
    }

    let current = root.join(ROOT);
    if current.exists() {
        bail!("both {LEGACY_ROOT} and {ROOT} exist; merge the durable WTW records before retrying");
    }

    fs::rename(&legacy, &current).with_context(|| format!("migrate {LEGACY_ROOT} to {ROOT}"))?;
    let legacy_parent = root.join(".agent-first");
    if legacy_parent.is_dir() && legacy_parent.read_dir()?.next().is_none() {
        fs::remove_dir(legacy_parent)?;
    }
    Ok(())
}

#[rustfmt::skip]
pub fn init(root: &Path, agent_files: &[PathBuf]) -> Result<()> { let root = repository(root)?; let managed = std::env::var_os("CSM_STORAGE_ROOT").is_some(); if !managed { migrate_legacy_root(&root)?; } let data = data_dir(&root); for directory in ["decisions", "invariants"] { fs::create_dir_all(data.join(format!("records/{directory}")))?; } write_new(data.join("config.local.toml"), CONFIG)?; fs::write(data.join("SKILL.md"), SKILL)?; if !managed { append_once(root.join(".gitignore"), IGNORE)?; for file in agent_files { safe_relative(file)?; upsert_block(root.join(file), INSTRUCTIONS)?; } } Ok(()) }

pub fn collect(root: &Path, request: CollectRequest) -> Result<CollectResult> {
    let root = repository(root)?;
    require_text("task", &request.task)?;
    validate_revision(&request.base)?;
    git(&root, &["rev-parse", "--verify", &request.base])?;
    let mut sources = BTreeMap::from([("task".into(), request.task)]);
    for source in request.sources {
        require_text("source name", &source.name)?;
        require_text("source content", &source.content)?;
        if sources.insert(source.name, source.content).is_some() {
            bail!("duplicate source name")
        }
    }
    sources.insert("diff".into(), diff(&root, &request.base)?);
    let envelope = serde_json::to_string(&sources)?;
    if envelope.len() > 160_000 {
        bail!("collection envelope exceeds 160000 bytes")
    }
    let first = validate_candidates(judge::<Extraction>(&root, &collect_prompt(&envelope, None)?)?.records, &sources)?;
    let second = validate_candidates(
        judge::<Extraction>(&root, &collect_prompt(&envelope, Some(&first))?)?.records,
        &sources,
    )?;
    let confirmed = first.into_iter().filter(|item| second.contains(item)).collect::<Vec<_>>();
    let mut all = load(&root)?;
    validate_candidate_links(&confirmed, &all)?;
    let mut recorded = Vec::new();
    let mut duplicates = 0;
    for candidate in confirmed.iter().cloned() {
        if let Some(existing) = all.iter().find(|record| record.kind == candidate.kind && record.id == candidate.id) {
            if semantic(existing) == candidate {
                duplicates += 1;
                continue;
            }
            bail!("record id {} already exists with different content", candidate.id)
        }
        let record = materialize(&root, candidate)?;
        write_record(&root, &record)?;
        all.push(record.clone());
        recorded.push(record);
    }
    apply_supersessions(&root, &mut all)?;
    Ok(CollectResult {
        candidates_found: confirmed.len(),
        duplicates,
        recorded,
    })
}

pub fn explain(root: &Path, task: &str, paths: &[String], limit: usize, external: &[Graph]) -> Result<ExplainResult> {
    let root = repository(root)?;
    require_text("task", task)?;
    if limit == 0 || limit > 50 {
        bail!("limit must be between 1 and 50")
    }
    let terms = tokens(&format!("{task} {}", paths.join(" ")));
    let mut ranked = load(&root)?
        .into_iter()
        .filter(|r| r.status == RecordStatus::Active)
        .map(|record| {
            let scoped = paths
                .iter()
                .any(|path| record.scopes.iter().any(|scope| Pattern::new(scope).is_ok_and(|p| p.matches(path))));
            let overlap = record_terms(&record).intersection(&terms).count();
            (usize::from(scoped) * 1000 + overlap, record)
        })
        .filter(|(score, _)| *score > 0)
        .collect::<Vec<_>>();
    ranked.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.id.cmp(&b.1.id)));
    let records = ranked.into_iter().take(limit).map(|(_, record)| record).collect::<Vec<_>>();
    let uris = records.iter().map(Record::uri).collect::<HashSet<_>>();
    let graph = merge_graphs(export_graph(&root)?, external);
    let edges = graph
        .edges
        .into_iter()
        .filter(|edge| uris.contains(&edge.from) || uris.contains(&edge.to))
        .collect();
    Ok(ExplainResult { records, edges })
}

pub fn guard(root: &Path, task: &str, base: &str, paths: &[String], external: &[Graph], suite: bool) -> Result<GuardResult> {
    let root = repository(root)?;
    validate_revision(base)?;
    git(&root, &["rev-parse", "--verify", base])?;
    let patch = diff(&root, base)?;
    let changed = if paths.is_empty() {
        changed_paths(&root, base)?
    } else {
        normalized(paths.to_vec())
    };
    let context = explain(&root, task, &changed, 12, external)?;
    let health = health(&root, external, suite)?;
    if patch.trim().is_empty() || context.records.is_empty() {
        return Ok(GuardResult {
            records_checked: context.records.len(),
            findings: vec![],
            health,
        });
    }
    let known = context.records.iter().map(Record::uri).collect::<HashSet<_>>();
    let overhead = guard_prompt(task, &context.records, "", None)?.len() + 100_000;
    let limit = MAX_PROMPT
        .checked_sub(overhead)
        .filter(|value| *value >= 10_000)
        .context("WTW guard context is too large")?;
    let mut findings = Vec::new();
    for chunk in patch_chunks(&patch, limit)? {
        let first = validate_findings(
            judge::<Audit>(&root, &guard_prompt(task, &context.records, &chunk, None)?)?.findings,
            &known,
            &changed,
            &chunk,
        )?;
        let second = validate_findings(
            judge::<Audit>(&root, &guard_prompt(task, &context.records, &chunk, Some(&first))?)?.findings,
            &known,
            &changed,
            &chunk,
        )?;
        for finding in first.into_iter().filter(|item| second.contains(item)) {
            if !findings.contains(&finding) {
                findings.push(finding);
            }
        }
        if findings.len() > 24 {
            bail!("judge returned too many findings")
        }
    }
    Ok(GuardResult {
        records_checked: context.records.len(),
        findings,
        health,
    })
}

pub fn show(root: &Path, id: &str) -> Result<Record> {
    let root = repository(root)?;
    find_record(&load(&root)?, id)
        .cloned()
        .with_context(|| format!("unknown record {id}"))
}

pub fn supersede(root: &Path, old: &str, by: &str, basis: &str) -> Result<Record> {
    let root = repository(root)?;
    require_text("basis", basis)?;
    let mut all = load(&root)?;
    let old_record = find_record(&all, old).cloned().with_context(|| format!("unknown record {old}"))?;
    let by_index = all
        .iter()
        .position(|r| r.uri() == by || r.id == by)
        .with_context(|| format!("unknown record {by}"))?;
    if old_record.uri() == all[by_index].uri() || old_record.kind != all[by_index].kind {
        bail!("supersession requires two different records of the same kind")
    }
    all[by_index].links.push(Link {
        rel: Relation::Supersedes,
        to: old_record.uri(),
        basis: basis.trim().into(),
    });
    all[by_index].links.sort();
    all[by_index].links.dedup();
    write_record(&root, &all[by_index])?;
    apply_supersessions(&root, &mut all)?;
    Ok(all[by_index].clone())
}

pub fn export_graph(root: &Path) -> Result<Graph> {
    let root = repository(root)?;
    let records = load(&root)?;
    let nodes = records
        .iter()
        .map(|r| GraphNode {
            uri: r.uri(),
            kind: kind_name(r.kind).into(),
            status: status_name(r.status).into(),
            title: r.title.clone(),
            scopes: r.scopes.clone(),
        })
        .collect();
    let mut edges = records
        .iter()
        .flat_map(|r| {
            r.links.iter().map(|l| GraphEdge {
                from: r.uri(),
                rel: relation_name(l.rel).into(),
                to: l.to.clone(),
                basis: l.basis.clone(),
            })
        })
        .collect::<Vec<_>>();
    edges.sort();
    edges.dedup();
    Ok(Graph { schema: 1, nodes, edges })
}

pub fn health(root: &Path, external: &[Graph], suite: bool) -> Result<HealthResult> {
    let root = repository(root)?;
    let local = export_graph(&root)?;
    let records = local.nodes.len();
    let graph = merge_graphs(local, external);
    let nodes = graph.nodes.iter().map(|n| (n.uri.as_str(), n)).collect::<HashMap<_, _>>();
    let mut issues = Vec::new();
    for edge in &graph.edges {
        let from = nodes.get(edge.from.as_str());
        let to = nodes.get(edge.to.as_str());
        match (from, to) {
            (Some(from), Some(to)) if !valid_edge(from, &edge.rel, to) => {
                issues.push(issue("incompatible_link", format!("{} --{}--> {}", edge.from, edge.rel, edge.to)));
            }
            (None, _) | (_, None) => {
                issues.push(issue("dangling_link", format!("{} --{}--> {}", edge.from, edge.rel, edge.to)));
            }
            _ => {}
        }
        if edge.basis.trim().is_empty() {
            issues.push(issue("empty_basis", format!("{} --{}--> {}", edge.from, edge.rel, edge.to)))
        }
    }
    if suite {
        for node in graph
            .nodes
            .iter()
            .filter(|n| n.uri.starts_with("wtw://invariant/") && n.status == "active")
        {
            if !graph.edges.iter().any(|edge| {
                edge.rel == "proves"
                    && edge.to == node.uri
                    && nodes
                        .get(edge.from.as_str())
                        .is_some_and(|source| source.kind == "proof" && source.status == "active")
            }) {
                issues.push(issue("unproved_invariant", node.uri.clone()));
            }
        }
    }
    issues.sort_by(|a, b| a.code.cmp(&b.code).then(a.message.cmp(&b.message)));
    issues.dedup();
    Ok(HealthResult {
        passed: issues.is_empty(),
        records,
        issues,
    })
}

fn validate_candidates(items: Vec<Candidate>, sources: &BTreeMap<String, String>) -> Result<Vec<Candidate>> {
    if items.len() > 24 {
        bail!("judge returned too many records")
    }
    let envelope = comparable(&sources.values().map(String::as_str).collect::<Vec<_>>().join("\n"));
    let mut output = Vec::new();
    for mut item in items {
        valid_id(&item.id)?;
        for (name, value) in [
            ("title", &item.title),
            ("statement", &item.statement),
            ("authority source", &item.authority.source),
            ("authority quote", &item.authority.quote),
        ] {
            require_text(name, value)?;
        }
        let source = sources
            .get(&item.authority.source)
            .context("judge referenced unknown authority source")?;
        if !comparable(source).contains(&comparable(&item.authority.quote)) {
            bail!("authority quote is not literal evidence")
        }
        item.scopes = normalized(item.scopes);
        item.evidence = normalized(item.evidence);
        item.links.sort();
        item.links.dedup();
        if item.scopes.is_empty() || item.evidence.len() < 2 {
            bail!("record requires scopes and two evidence fragments")
        }
        for scope in &item.scopes {
            Pattern::new(scope).with_context(|| format!("invalid scope {scope}"))?;
        }
        if item.evidence.iter().any(|e| e.len() < 8 || !envelope.contains(&comparable(e))) {
            bail!("judge returned invented evidence")
        }
        match item.kind {
            RecordKind::Decision => {
                require_text("rationale", &item.rationale)?;
                if item.alternatives.is_empty() || !item.violation.trim().is_empty() {
                    bail!("decision requires alternatives and no violation")
                }
                for alt in &item.alternatives {
                    require_text("alternative", &alt.statement)?;
                    require_text("rejection reason", &alt.rejected_because)?;
                }
            }
            RecordKind::Invariant if item.violation.trim().is_empty() || !item.alternatives.is_empty() => {
                bail!("invariant requires a violation and no alternatives")
            }
            RecordKind::Invariant => {}
        }
        for link in &item.links {
            require_text("link target", &link.to)?;
            require_text("link basis", &link.basis)?;
            parse_wtw_uri(&link.to)?;
        }
        if !output.contains(&item) {
            output.push(item);
        }
    }
    Ok(output)
}

fn validate_candidate_links(candidates: &[Candidate], existing: &[Record]) -> Result<()> {
    let mut nodes = existing.iter().map(|r| (r.uri(), r.kind)).collect::<HashMap<_, _>>();
    nodes.extend(candidates.iter().map(|c| (format!("wtw://{}/{}", kind_name(c.kind), c.id), c.kind)));
    for candidate in candidates {
        for link in &candidate.links {
            let target = *nodes.get(&link.to).context("link target does not exist")?;
            let valid = matches!(
                (candidate.kind, link.rel, target),
                (
                    RecordKind::Decision,
                    Relation::Establishes | Relation::Upholds,
                    RecordKind::Invariant
                )
            ) || (link.rel == Relation::Supersedes && candidate.kind == target);
            if !valid {
                bail!("relation is incompatible with source and target kinds")
            }
        }
    }
    Ok(())
}

fn validate_findings(items: Vec<Finding>, known: &HashSet<String>, paths: &[String], patch: &str) -> Result<Vec<Finding>> {
    if items.len() > 24 {
        bail!("judge returned too many findings")
    }
    let mut output = Vec::new();
    for item in items {
        if !known.contains(&item.record_uri)
            || !paths.contains(&item.path)
            || item.line == 0
            || item.evidence.trim().is_empty()
            || item.reason.trim().is_empty()
            || !patch.contains(&item.evidence)
        {
            bail!("judge returned an invalid finding")
        }
        if !output.contains(&item) {
            output.push(item);
        }
    }
    Ok(output)
}

#[rustfmt::skip]
fn materialize(root: &Path, candidate: Candidate) -> Result<Record> {
    Ok(Record {
        schema: 1, id: candidate.id, kind: candidate.kind, status: RecordStatus::Active,
        title: candidate.title, statement: candidate.statement, rationale: candidate.rationale,
        alternatives: candidate.alternatives, violation: candidate.violation, scopes: candidate.scopes,
        authority: candidate.authority, evidence: candidate.evidence, links: candidate.links,
        recorded_at: Utc::now(),
        recorded_by: git(root, &["config", "user.name"]).unwrap_or_else(|_| "unknown".into()).trim().into(),
        recorded_commit: git(root, &["rev-parse", "HEAD"])?.trim().into(),
    })
}

#[rustfmt::skip]
fn semantic(record: &Record) -> Candidate {
    Candidate {
        id: record.id.clone(), kind: record.kind, title: record.title.clone(),
        statement: record.statement.clone(), rationale: record.rationale.clone(),
        alternatives: record.alternatives.clone(), violation: record.violation.clone(),
        scopes: record.scopes.clone(), authority: record.authority.clone(),
        evidence: record.evidence.clone(), links: record.links.clone(),
    }
}

fn load(root: &Path) -> Result<Vec<Record>> {
    let directory = data_dir(root).join("records");
    if !directory.exists() {
        bail!("Why This Way is not initialized; run wtw init")
    }
    let mut paths = Vec::new();
    for kind in ["decisions", "invariants"] {
        for entry in fs::read_dir(directory.join(kind))? {
            let path = entry?.path();
            if path.extension().is_some_and(|v| v == "toml") {
                paths.push(path);
            }
        }
    }
    paths.sort();
    paths
        .into_iter()
        .map(|path| {
            let record: Record =
                toml::from_str(&fs::read_to_string(&path)?).with_context(|| format!("invalid record {}", path.display()))?;
            validate_record(&record).with_context(|| format!("invalid record {}", path.display()))?;
            Ok(record)
        })
        .collect()
}

fn validate_record(record: &Record) -> Result<()> {
    if record.schema != 1 {
        bail!("unsupported schema")
    }
    valid_id(&record.id)?;
    require_text("title", &record.title)?;
    require_text("statement", &record.statement)?;
    for scope in &record.scopes {
        Pattern::new(scope)?;
    }
    if record.scopes.is_empty() || record.evidence.len() < 2 {
        bail!("missing scope or evidence")
    }
    match record.kind {
        RecordKind::Decision if record.rationale.trim().is_empty() || record.alternatives.is_empty() || !record.violation.is_empty() => {
            bail!("invalid decision shape")
        }
        RecordKind::Invariant if record.violation.trim().is_empty() || !record.alternatives.is_empty() => bail!("invalid invariant shape"),
        _ => {}
    }
    for link in &record.links {
        require_text("link basis", &link.basis)?;
        parse_wtw_uri(&link.to)?;
    }
    Ok(())
}

fn write_record(root: &Path, record: &Record) -> Result<()> {
    validate_record(record)?;
    let directory = if record.kind == RecordKind::Decision {
        "decisions"
    } else {
        "invariants"
    };
    atomic(
        &data_dir(root).join(format!("records/{directory}/{}.toml", record.id)),
        &toml::to_string_pretty(record)?,
    )
}

fn apply_supersessions(root: &Path, records: &mut [Record]) -> Result<()> {
    let superseded = records
        .iter()
        .flat_map(|r| r.links.iter().filter(|l| l.rel == Relation::Supersedes).map(|l| l.to.clone()))
        .collect::<HashSet<_>>();
    for record in records {
        let wanted = if superseded.contains(&record.uri()) {
            RecordStatus::Superseded
        } else {
            record.status
        };
        if wanted != record.status {
            record.status = wanted;
            write_record(root, record)?;
        }
    }
    Ok(())
}

fn collect_prompt(envelope: &str, candidates: Option<&[Candidate]>) -> Result<String> {
    let phase = candidates
        .map(|items| {
            format!(
                "Confirm only candidates from this list, unchanged in every field: {}.",
                serde_json::to_string(items).unwrap()
            )
        })
        .unwrap_or_else(|| "Extract durable decisions and invariants.".into());
    Ok(format!(
        "You are the bounded Why This Way collector. {phase} Every record uses exactly these fields: id, kind, title, statement, rationale, alternatives, violation, scopes, authority, evidence, links. Alternative entries use exactly statement and rejected_because. Authority uses exactly kind, source, quote; source must be an exact top-level key from SOURCES such as task, diff, or a supplied source key, never a path merely mentioned inside a source. Evidence is an array of literal strings. Link entries use exactly rel, to, basis, and to must be a full wtw://decision/<id> or wtw://invariant/<id> URI. A decision requires a durable explicit choice in statement, a nonempty rationale, at least one rejected alternative, reusable glob scopes, an authority source key and literal quote, and two literal evidence fragments; violation is empty. An invariant requires a durable falsifiable truth in statement, a concrete violation, scopes, authority, and evidence; rationale is empty and alternatives is empty. Authority kinds are human_statement, accepted_plan, adr, policy, contract, or merged_change. Reject implementation descriptions, preferences, hypothetical ideas, unfinished work, errors, examples, and acceptance tests. IDs are lowercase semantic slugs. Allowed links are establishes/upholds from a decision to an invariant and supersedes between equal kinds; every link needs a basis. Return strict JSON {{\"records\":[]}}.\nSOURCES:\n{envelope}"
    ))
}

fn guard_prompt(task: &str, records: &[Record], patch: &str, findings: Option<&[Finding]>) -> Result<String> {
    let phase = findings
        .map(|f| {
            format!(
                "Confirm only supported findings from this list unchanged: {}.",
                serde_json::to_string(f).unwrap()
            )
        })
        .unwrap_or_else(|| "Find direct contradictions only.".into());
    Ok(format!(
        "You are the bounded Why This Way guard. {phase} Report only changed code that directly contradicts a supplied active decision or violates a supplied invariant. Every finding uses exactly these fields: record_uri, path, line, evidence, reason. record_uri is the exact supplied record URI. path is a changed path from the patch. line is the changed line number. evidence is one literal single-line substring from the patch. Code implementing the decision is not a finding. Return strict JSON {{\"findings\":[]}}.\nTASK:{task}\nRECORDS:{}\nPATCH:\n{patch}",
        serde_json::to_string(records)?
    ))
}

fn judge<T: serde::de::DeserializeOwned>(root: &Path, prompt: &str) -> Result<T> {
    if prompt.len() > MAX_PROMPT {
        bail!("judge prompt exceeds the safe WTW envelope")
    }
    let config = load_config(root)?;
    if config.schema != 1 || config.judge.command.is_empty() {
        bail!("unsupported or empty judge configuration")
    }
    let cwd = tempdir()?;
    let mut child = Command::new(&config.judge.command[0])
        .args(&config.judge.command[1..])
        .current_dir(cwd.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .context("start judge")?;
    child.stdin.take().context("open judge stdin")?.write_all(prompt.as_bytes())?;
    let output = child.wait_with_output()?;
    if !output.status.success() {
        bail!("judge exited with {}", output.status)
    }
    serde_json::from_slice(&output.stdout).context("judge returned invalid JSON")
}

fn load_config(root: &Path) -> Result<Config> {
    let user = dirs::config_dir().map(|p| p.join("why-this-way/config.toml"));
    let path = [
        Some(data_dir(root).join("config.local.toml")),
        Some(data_dir(root).join("config.toml")),
        user,
    ]
    .into_iter()
    .flatten()
    .find(|p| p.exists())
    .ok_or_else(|| anyhow!("missing judge configuration"))?;
    toml::from_str(&fs::read_to_string(path)?).context("invalid judge configuration")
}

fn merge_graphs(mut local: Graph, external: &[Graph]) -> Graph {
    for graph in external {
        if graph.schema == 1 {
            local.nodes.extend(graph.nodes.clone());
            local.edges.extend(graph.edges.clone());
        }
    }
    local.nodes.sort_by(|a, b| a.uri.cmp(&b.uri));
    local.nodes.dedup_by(|a, b| a.uri == b.uri);
    local.edges.sort();
    local.edges.dedup();
    local
}

fn valid_edge(from: &GraphNode, relation: &str, to: &GraphNode) -> bool {
    matches!(
        (from.kind.as_str(), relation, to.kind.as_str()),
        ("decision", "establishes" | "upholds", "invariant")
            | ("decision", "supersedes", "decision")
            | ("invariant", "supersedes", "invariant")
            | ("proof", "proves", "invariant")
            | ("example", "exemplifies", "decision" | "invariant")
            | ("scar", "records_violation_of", "invariant")
            | ("deferment", "tracks_blocker_for", "decision")
    )
}

fn find_record<'a>(records: &'a [Record], id: &str) -> Option<&'a Record> {
    let matches = records.iter().filter(|r| r.id == id || r.uri() == id).collect::<Vec<_>>();
    (matches.len() == 1).then(|| matches[0])
}

fn parse_wtw_uri(uri: &str) -> Result<(RecordKind, &str)> {
    let tail = uri.strip_prefix("wtw://").context("WTW link target must use wtw://")?;
    let (kind, id) = tail.split_once('/').context("WTW URI requires kind and id")?;
    let kind = match kind {
        "decision" => RecordKind::Decision,
        "invariant" => RecordKind::Invariant,
        _ => bail!("unknown WTW URI kind"),
    };
    valid_id(id)?;
    Ok((kind, id))
}

fn changed_paths(root: &Path, base: &str) -> Result<Vec<String>> {
    let exclude = store_exclude(root);
    let mut paths = git(root, &["diff", "--name-only", base, "--", ".", &exclude])?
        .lines()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    paths.extend(
        git(root, &["ls-files", "--others", "--exclude-standard", "--", ".", &exclude])?
            .lines()
            .map(str::to_owned),
    );
    Ok(normalized(paths))
}

fn diff(root: &Path, base: &str) -> Result<String> {
    let exclude = store_exclude(root);
    let mut value = git(root, &["diff", "--no-ext-diff", "--unified=3", base, "--", ".", &exclude])?;
    for path in git(root, &["ls-files", "--others", "--exclude-standard", "--", ".", &exclude])?.lines() {
        let contents = fs::read_to_string(root.join(path)).with_context(|| format!("untracked file is not auditable text: {path}"))?;
        value.push_str(&format!("\ndiff --git a/{path} b/{path}\n--- /dev/null\n+++ b/{path}\n"));
        for line in contents.lines() {
            value.push_str(&format!("+{line}\n"));
        }
    }
    Ok(value)
}

#[rustfmt::skip]
fn patch_chunks(patch: &str, limit: usize) -> Result<Vec<String>> {
    let (mut output, mut chunk, mut file, mut hunk) = (Vec::new(), String::new(), String::new(), String::new());
    for line in patch.split_inclusive('\n') {
        let starts_file = line.starts_with("diff --git "); let starts_hunk = line.starts_with("@@ ");
        if starts_file {
            file = line.into(); hunk.clear();
        } else if starts_hunk { hunk = line.into();
        }
        if chunk.len() + line.len() > limit {
            if !chunk.is_empty() { output.push(std::mem::take(&mut chunk)); }
            if !starts_file {
                chunk.push_str(&file);
                if !starts_hunk { chunk.push_str(&hunk); }
            }
        }
        if chunk.len() + line.len() > limit { bail!("one diff line exceeds the safe WTW judge envelope") }
        chunk.push_str(line);
    }
    if !chunk.is_empty() { output.push(chunk); }
    Ok(output)
}

fn git(root: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(["-c", "core.quotePath=false"])
        .args(args)
        .current_dir(root)
        .output()
        .context("start git")?;
    if !output.status.success() {
        bail!("git {} failed: {}", args.join(" "), String::from_utf8_lossy(&output.stderr).trim())
    }
    String::from_utf8(output.stdout).context("git output was not UTF-8")
}

fn record_terms(record: &Record) -> HashSet<String> {
    tokens(&format!(
        "{} {} {} {} {}",
        record.title,
        record.statement,
        record.rationale,
        record.violation,
        record.scopes.join(" ")
    ))
}
fn tokens(value: &str) -> HashSet<String> {
    value
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|p| p.len() > 1)
        .map(str::to_owned)
        .collect()
}
fn kind_name(kind: RecordKind) -> &'static str {
    if kind == RecordKind::Decision { "decision" } else { "invariant" }
}
fn status_name(status: RecordStatus) -> &'static str {
    match status {
        RecordStatus::Active => "active",
        RecordStatus::Superseded => "superseded",
        RecordStatus::Retired => "retired",
    }
}
fn relation_name(rel: Relation) -> &'static str {
    match rel {
        Relation::Establishes => "establishes",
        Relation::Upholds => "upholds",
        Relation::Supersedes => "supersedes",
    }
}
#[rustfmt::skip]
fn issue(code: &str, message: String) -> HealthIssue { HealthIssue { code: code.into(), message } }
fn valid_id(id: &str) -> Result<()> {
    if id.len() < 3
        || id.len() > 80
        || id.starts_with(['.', '-'])
        || !id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || ".-".contains(c))
    {
        bail!("id must be a 3-80 character lowercase semantic slug")
    }
    Ok(())
}
fn validate_revision(v: &str) -> Result<()> {
    if v.is_empty() || v.starts_with('-') || !v.chars().all(|c| c.is_ascii_alphanumeric() || "_./~^-".contains(c)) {
        bail!("invalid base revision")
    }
    Ok(())
}
fn require_text(name: &str, value: &str) -> Result<()> {
    if value.trim().is_empty() {
        bail!("{name} must not be empty")
    }
    Ok(())
}
fn comparable(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}
fn normalized(values: Vec<String>) -> Vec<String> {
    let mut v = values
        .into_iter()
        .map(|x| x.trim().replace('\\', "/"))
        .filter(|x| !x.is_empty())
        .collect::<Vec<_>>();
    v.sort();
    v.dedup();
    v
}
fn safe_relative(path: &Path) -> Result<()> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|p| matches!(p, Component::ParentDir | Component::RootDir | Component::Prefix(_)))
    {
        bail!("path must stay inside the repository: {}", path.display())
    }
    Ok(())
}
fn atomic(path: &Path, contents: &str) -> Result<()> {
    let temp = path.with_extension(format!("{}.tmp", Ulid::generate()));
    fs::write(&temp, contents)?;
    fs::rename(temp, path)?;
    Ok(())
}
fn write_new(path: PathBuf, contents: &str) -> Result<()> {
    if !path.exists() {
        fs::write(path, contents)?;
    }
    Ok(())
}
fn append_once(path: PathBuf, block: &str) -> Result<()> {
    let current = fs::read_to_string(&path).unwrap_or_default();
    if !current.replace("\r\n", "\n").contains(block.trim()) {
        fs::write(
            path,
            format!(
                "{}{}{}\n",
                current,
                if current.is_empty() || current.ends_with('\n') { "" } else { "\n" },
                block.trim()
            ),
        )?;
    }
    Ok(())
}
fn upsert_block(path: PathBuf, block: &str) -> Result<()> {
    let current = fs::read_to_string(&path).unwrap_or_default();
    let updated = if let (Some(s), Some(e)) = (current.find(START), current.find(END)) {
        format!("{}{}{}", &current[..s], block.trim(), &current[e + END.len()..])
    } else {
        format!(
            "{}{}{}\n",
            current,
            if current.is_empty() || current.ends_with('\n') { "" } else { "\n" },
            block.trim()
        )
    };
    fs::write(path, updated)?;
    Ok(())
}

#[derive(Parser)]
#[command(name = "wtw", version, about = "Preserve why this repository works this way")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}
#[derive(Subcommand)]
enum Commands {
    Init(InitArgs),
    Collect(CollectArgs),
    Explain(QueryArgs),
    Guard(GuardArgs),
    Show(ShowArgs),
    Health(HealthArgs),
    Supersede(SupersedeArgs),
    Export,
    Mcp,
}
#[derive(Args)]
struct InitArgs {
    #[arg(long, default_value = "AGENTS.md")]
    agent_file: Vec<PathBuf>,
}
#[derive(Args)]
#[rustfmt::skip]
struct CollectArgs {#[arg(long)]task:String,#[arg(long)]source:Vec<PathBuf>,#[arg(long,default_value="HEAD")]base:String,#[arg(long)]json:bool}
#[derive(Args)]
#[rustfmt::skip]
struct QueryArgs {#[arg(long)]task:String,#[arg(long)]path:Vec<String>,#[arg(long,default_value_t=12)]limit:usize,#[arg(long)]graph:Vec<PathBuf>,#[arg(long)]json:bool}
#[derive(Args)]
#[rustfmt::skip]
struct GuardArgs {#[arg(long)]task:String,#[arg(long,default_value="HEAD")]base:String,#[arg(long)]path:Vec<String>,#[arg(long)]graph:Vec<PathBuf>,#[arg(long)]suite:bool,#[arg(long)]json:bool}
#[derive(Args)]
#[rustfmt::skip]
struct ShowArgs { #[arg(long)] id: String, #[arg(long)] json: bool }
#[derive(Args)]
#[rustfmt::skip]
struct HealthArgs { #[arg(long)] graph: Vec<PathBuf>, #[arg(long)] suite: bool, #[arg(long)] json: bool }
#[derive(Args)]
#[rustfmt::skip]
struct SupersedeArgs { #[arg(long)] id: String, #[arg(long)] by: String, #[arg(long)] basis: String, #[arg(long)] json: bool }

pub fn run_cli_env() -> Result<i32> {
    let current = std::env::current_dir()?;
    run_cli_at(std::env::args_os().collect(), &current, &mut io::stdin().lock(), &mut io::stdout())
}
pub fn run_cli_at(arguments: Vec<OsString>, current: &Path, input: &mut dyn BufRead, output: &mut dyn Write) -> Result<i32> {
    let cli = match Cli::try_parse_from(arguments) {
        Ok(v) => v,
        Err(e) if e.use_stderr() => return Err(e.into()),
        Err(e) => {
            write!(output, "{e}")?;
            return Ok(0);
        }
    };
    match cli.command {
        Commands::Init(a) => {
            init(current, &a.agent_file)?;
            writeln!(output, "Why This Way initialized.")?
        }
        Commands::Collect(a) => {
            let root = repository(current)?;
            let sources = read_sources(&root, &a.source)?;
            print_value(
                &collect(
                    &root,
                    CollectRequest {
                        task: a.task,
                        sources,
                        base: a.base,
                    },
                )?,
                a.json,
                output,
            )?
        }
        Commands::Explain(a) => {
            let graphs = read_graphs(current, &a.graph)?;
            print_value(&explain(current, &a.task, &a.path, a.limit, &graphs)?, a.json, output)?
        }
        Commands::Guard(a) => {
            let graphs = read_graphs(current, &a.graph)?;
            let result = guard(current, &a.task, &a.base, &a.path, &graphs, a.suite)?;
            let failed = !result.health.passed || !result.findings.is_empty();
            print_value(&result, a.json, output)?;
            return Ok(i32::from(failed));
        }
        Commands::Show(a) => print_value(&show(current, &a.id)?, a.json, output)?,
        Commands::Health(a) => {
            let graphs = read_graphs(current, &a.graph)?;
            let result = health(current, &graphs, a.suite)?;
            let failed = !result.passed;
            print_value(&result, a.json, output)?;
            return Ok(i32::from(failed));
        }
        Commands::Supersede(a) => print_value(&supersede(current, &a.id, &a.by, &a.basis)?, a.json, output)?,
        Commands::Export => writeln!(output, "{}", serde_json::to_string_pretty(&export_graph(current)?)?)?,
        Commands::Mcp => mcp_stream(input, output)?,
    }
    Ok(0)
}

fn read_sources(root: &Path, paths: &[PathBuf]) -> Result<Vec<Source>> {
    let internal = data_dir(root).strip_prefix(root).ok().map(Path::to_path_buf);
    paths
        .iter()
        .map(|path| {
            safe_relative(path)?;
            if internal.as_ref().is_some_and(|directory| path.starts_with(directory)) {
                bail!("WTW internal files cannot be collection sources")
            }
            Ok(Source {
                name: path.to_string_lossy().replace('\\', "/"),
                content: fs::read_to_string(root.join(path)).with_context(|| format!("read {}", path.display()))?,
            })
        })
        .collect()
}
fn read_graphs(root: &Path, paths: &[PathBuf]) -> Result<Vec<Graph>> {
    let root = repository(root)?;
    paths
        .iter()
        .map(|p| {
            safe_relative(p)?;
            let graph: Graph = serde_json::from_str(&fs::read_to_string(root.join(p))?)?;
            if graph.schema != 1 {
                bail!("unsupported graph schema")
            }
            Ok(graph)
        })
        .collect()
}
fn print_value<T: Serialize>(value: &T, _json_output: bool, output: &mut dyn Write) -> Result<()> {
    writeln!(output, "{}", serde_json::to_string_pretty(value)?)?;
    Ok(())
}

pub fn mcp_stream(reader: &mut dyn BufRead, output: &mut dyn Write) -> Result<()> {
    for line in reader.lines() {
        let request: Value = serde_json::from_str(&line?)?;
        if request.get("id").is_none() {
            continue;
        }
        let id = request["id"].clone();
        let response = match request["method"].as_str().unwrap_or_default() {
            "initialize" => {
                json!({"jsonrpc":"2.0","id":id,"result":{"protocolVersion":"2025-11-25","capabilities":{"tools":{}},"serverInfo":{"name":"why-this-way","version":env!("CARGO_PKG_VERSION")}}})
            }
            "ping" => json!({"jsonrpc":"2.0","id":id,"result":{}}),
            "tools/list" => json!({"jsonrpc":"2.0","id":id,"result":{"tools":mcp_tools()}}),
            "tools/call" => match mcp_call(&request["params"]) {
                Ok(value) => {
                    json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":serde_json::to_string(&value)?}],"structuredContent":value}})
                }
                Err(e) => json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":e.to_string()}],"isError":true}}),
            },
            _ => json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":"method not found"}}),
        };
        writeln!(output, "{}", serde_json::to_string(&response)?)?;
        output.flush()?;
    }
    Ok(())
}

fn mcp_tools() -> Value {
    json!([
        {"name":"wtw_collect","description":"Collect confirmed decisions and invariants","inputSchema":{"type":"object","required":["repository","task"],"properties":{"repository":{"type":"string"},"task":{"type":"string"},"sources":{"type":"array","items":{"type":"object"}},"base":{"type":"string"}}}},
        {"name":"wtw_explain","description":"Retrieve governing decisions and invariants","inputSchema":{"type":"object","required":["repository","task"],"properties":{"repository":{"type":"string"},"task":{"type":"string"},"paths":{"type":"array","items":{"type":"string"}},"limit":{"type":"integer"}}}},
        {"name":"wtw_guard","description":"Audit a diff against governing records","inputSchema":{"type":"object","required":["repository","task"],"properties":{"repository":{"type":"string"},"task":{"type":"string"},"base":{"type":"string"},"paths":{"type":"array","items":{"type":"string"}},"suite":{"type":"boolean"}}}},
        {"name":"wtw_show","description":"Read one WTW record","inputSchema":{"type":"object","required":["repository","id"],"properties":{"repository":{"type":"string"},"id":{"type":"string"}}}},
        {"name":"wtw_health","description":"Validate the WTW graph","inputSchema":{"type":"object","required":["repository"],"properties":{"repository":{"type":"string"},"suite":{"type":"boolean"}}}},
        {"name":"wtw_export","description":"Export Agent First graph nodes and edges","inputSchema":{"type":"object","required":["repository"],"properties":{"repository":{"type":"string"}}}}
    ])
}

fn mcp_call(params: &Value) -> Result<Value> {
    let name = params["name"].as_str().unwrap_or_default();
    let a = &params["arguments"];
    let root = Path::new(a["repository"].as_str().context("repository is required")?);
    let strings = |name: &str| -> Vec<String> {
        a[name]
            .as_array()
            .map(|v| v.iter().filter_map(Value::as_str).map(str::to_owned).collect::<Vec<_>>())
            .unwrap_or_default()
    };
    match name {
        "wtw_collect" => Ok(serde_json::to_value(collect(
            root,
            CollectRequest {
                task: a["task"].as_str().context("task is required")?.into(),
                sources: serde_json::from_value(a["sources"].clone()).unwrap_or_default(),
                base: a["base"].as_str().unwrap_or("HEAD").into(),
            },
        )?)?),
        "wtw_explain" => Ok(serde_json::to_value(explain(
            root,
            a["task"].as_str().context("task is required")?,
            &strings("paths"),
            a["limit"].as_u64().unwrap_or(12) as usize,
            &[],
        )?)?),
        "wtw_guard" => Ok(serde_json::to_value(guard(
            root,
            a["task"].as_str().context("task is required")?,
            a["base"].as_str().unwrap_or("HEAD"),
            &strings("paths"),
            &[],
            a["suite"].as_bool().unwrap_or(false),
        )?)?),
        "wtw_show" => Ok(serde_json::to_value(show(root, a["id"].as_str().context("id is required")?)?)?),
        "wtw_health" => Ok(serde_json::to_value(health(root, &[], a["suite"].as_bool().unwrap_or(false))?)?),
        "wtw_export" => Ok(serde_json::to_value(export_graph(root)?)?),
        _ => bail!("unknown tool {name}"),
    }
}
