# WTW paired agent benchmark

The same task was run once without WTW and once with one relevant, versioned WTW record. Arm order was randomized. A deterministic evaluator outside the agent classified the resulting code.

| Case | Baseline | WTW | Classification |
| --- | --- | --- | --- |
| account-recovery-privacy | pass | pass | passing_tie |
| server-authoritative-pricing | pass | pass | passing_tie |
| order-audit-retention | pass | pass | passing_tie |
| customer-name-expand-contract | contradiction | pass | prevention |
| authenticated-tenant-authority | pass | pass | passing_tie |

| Measurement | Result |
| --- | ---: |
| Proven preventions | 1 |
| Passing ties | 4 |
| Regressions | 0 |
| WTW explain observed | 5/5 |
| WTW guard observed | 5/5 |
| Overall result | PASS |

## Counting rule

A prevention is counted only when the baseline arm contradicts the accepted decision and the WTW arm passes. A baseline that already passes is a passing tie, not a prevention. Incomplete arms remain visible and are not converted into wins.

## Limitations

- The repositories and accepted decisions are realistic but synthetic.
- A single run per arm is too small for a universal prevention-rate claim.
- The deterministic evaluator measures the targeted contradiction, not general code quality.
- A prevention is counted only when the baseline contradicts a decision and the WTW arm passes.
