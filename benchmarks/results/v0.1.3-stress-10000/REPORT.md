# WTW large-corpus stress: 10,000 records

This benchmark exercises Why This Way against a versioned synthetic repository with an equal mix of decisions and invariants.

| Measurement | Result |
| --- | ---: |
| Records loaded and exported | 10,000/10,000 |
| Retrieval exact hits | 64/64 |
| Target ranked first | 64/64 |
| Result limit respected | 64/64 |
| Unrelated-query results | 0 |
| Contradicting diff findings | 1 |
| Compliant diff findings | 0 |
| Corrupt storage rejected | yes |
| Graph health | pass |
| Explain latency p50 | 937.75 ms |
| Explain latency p95 | 1001.49 ms |
| Explain latency max | 1053.22 ms |
| Overall result | PASS |

## Interpretation

WTW recalled every deliberately relevant record, ranked it first, kept the response bounded, rejected corrupt truth, and distinguished a known contradiction from a compliant change.

## Limitations

- The corpus and judge are deterministic synthetic fixtures.
- Retrieval probes use unique vocabulary and exact governing paths.
- This stress test measures storage, ranking, graph, guard plumbing, and fail-closed behavior; it does not measure whether an LLM discovers the correct decision.
