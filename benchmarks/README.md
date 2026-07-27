# Benchmarks

Why This Way publishes two different forms of evidence.

| Benchmark | Question |
| --- | --- |
| Paired agent benchmark | Does a coding agent contradict fewer known decisions when WTW is present? |
| Large-corpus stress | Does the WTW engine remain exact, bounded, healthy, and fail-closed with 1,024 and 10,000 versioned records? |

The paired benchmark uses small synthetic repositories, an unchanged coding
task, isolated agent homes, and deterministic external evaluators. A prevention
is counted only when the baseline contradicts a recorded decision and the WTW
arm does not. Baseline passes are reported as passing ties.

The stress corpus is deterministic and synthetic. Its judge is deterministic.
It measures storage, ranking, graph, guard plumbing, and fail-closed behavior.
It does not measure whether an LLM can infer the correct architectural decision.

## Run

```bash
cargo build --release --locked
python benchmarks/stress.py
python benchmarks/paired.py --model <model>
```

Machine-readable summaries and human-readable reports are committed under
`benchmarks/results/`.
