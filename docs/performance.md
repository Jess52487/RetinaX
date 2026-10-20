# Performance & Gas Optimization

## Benchmarking Setup

We use Criterion for micro-benchmarking.

Run locally:

## Contract gas budget

Per-function cost for `vision_records`, and the harness that measures it, are
in [gas-budget.md](gas-budget.md). The harness is ignored by default. Run it
with:

```bash
cargo test -p vision_records --test gas_budget -- --ignored --nocapture
```
