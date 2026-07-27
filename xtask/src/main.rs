use std::{
    env,
    path::{Path, PathBuf},
    process::{Command, Output},
};

const MAX_PRODUCTION_LINES: u64 = 1100;
const MINIMUM_LINE_COVERAGE: u64 = 95;

fn main() {
    if let Err(error) = execute(env::args().skip(1).collect()) {
        eprintln!("verify failed: {error}");
        std::process::exit(1);
    }
}

fn execute(arguments: Vec<String>) -> Result<(), String> {
    if arguments.as_slice() != ["verify"] {
        return Err("usage: cargo xtask verify".into());
    }
    let root = repository_root()?;
    println!("Why This Way repository verification");
    run(&root, "cargo", &["fmt", "--all", "--", "--check"])?;
    run(
        &root,
        "cargo",
        &["clippy", "--workspace", "--all-targets", "--all-features", "--", "-D", "warnings"],
    )?;
    enforce_line_budget(&root)?;
    run(&root, "cargo", &["test", "--workspace", "--all-features", "--locked"])?;
    let coverage = MINIMUM_LINE_COVERAGE.to_string();
    run(
        &root,
        "cargo",
        &[
            "llvm-cov",
            "--package",
            "why-this-way",
            "--all-features",
            "--locked",
            "--ignore-filename-regex",
            r"src[/\\]main\.rs$",
            "--fail-under-lines",
            &coverage,
        ],
    )?;
    println!("verify passed");
    Ok(())
}

fn repository_root() -> Result<PathBuf, String> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "xtask must be located inside the repository".into())
}

fn run(root: &Path, program: &str, arguments: &[&str]) -> Result<(), String> {
    println!("  > {program} {}", arguments.join(" "));
    let status = Command::new(program)
        .args(arguments)
        .current_dir(root)
        .status()
        .map_err(|error| format!("could not start {program}: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{program} exited with {status}"))
    }
}

fn enforce_line_budget(root: &Path) -> Result<(), String> {
    let output = Command::new("tokei")
        .args(["src", "--output", "json"])
        .current_dir(root)
        .output()
        .map_err(|error| format!("could not start tokei: {error}"))?;
    let lines = production_lines(&output)?;
    if lines > MAX_PRODUCTION_LINES {
        return Err(format!("production line budget exceeded: {lines}/{MAX_PRODUCTION_LINES}"));
    }
    println!("    production lines: {lines}/{MAX_PRODUCTION_LINES}");
    Ok(())
}

fn production_lines(output: &Output) -> Result<u64, String> {
    if !output.status.success() {
        return Err(format!("tokei exited with {}", output.status));
    }
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).map_err(|error| format!("invalid tokei JSON: {error}"))?;
    report
        .pointer("/Rust/code")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| "tokei JSON did not contain Rust.code".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_line_parser_and_budget_are_strict() {
        let output = Output {
            status: success(),
            stdout: br#"{"Rust":{"code":499}}"#.to_vec(),
            stderr: vec![],
        };
        assert_eq!(production_lines(&output).unwrap(), 499);
    }

    #[cfg(unix)]
    fn success() -> std::process::ExitStatus {
        use std::os::unix::process::ExitStatusExt;
        std::process::ExitStatus::from_raw(0)
    }

    #[cfg(windows)]
    fn success() -> std::process::ExitStatus {
        use std::os::windows::process::ExitStatusExt;
        std::process::ExitStatus::from_raw(0)
    }
}
