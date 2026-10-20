# Gas Budget Reference: `vision_records`

This page tracks the cost of the `vision_records` public functions and the
operations that sit closest to Soroban's per-transaction limits.

> **Status: static analysis only.** The measured CPU and memory figures come
> from the harness described below and have not been recorded in this
> document yet. The table in [Static ranking](#static-ranking) shows relative
> structural cost, not measured cost. Do not read it as fee or limit data.

## How to measure

The harness is `contracts/vision_records/tests/gas_budget.rs`. It runs the
most expensive operations against the host budget tracker and prints one row
per operation:

```bash
cargo test -p vision_records --test gas_budget -- --ignored --nocapture
```

The harness is `#[ignore]`d so `cargo test --all` does not run it. Record the
output in the [Measured costs](#measured-costs) table when you run it, along
with the commit hash and the `soroban-sdk` version.

## Static ranking

Produced by a read-only scan of the `pub fn` bodies in
`contracts/vision_records/src/lib.rs`. The score is a rough weighting:

```
score = storage_reads + 2 × storage_writes + TTL_extensions
      + 3 × cross_contract_calls + 3 × access_checks
      + 4 × loops + event_publishes
```

Higher scores mean more ledger entries touched, more cross-contract calls, and
more loops over stored data. Functions with many loops or cross-contract calls
are the most likely to grow with stored data.

| Score | Function                         | Line | Storage reads | Storage writes | Access checks | Loops |
|------:|----------------------------------|-----:|--------------:|---------------:|--------------:|------:|
| 42    | `grant_access`                   | 1615 | 3 | 2 | 4 | 4 |
| 40    | `add_records`                    | 1142 | 4 | 3 | 3 | 3 |
| 36    | `add_record`                     | 992  | 4 | 3 | 5 | 1 |
| 27    | `export_patient_data`            | 2840 | 3 | 0 | 3 | 3 |
| 26    | `get_record`                     | 1270 | 3 | 0 | 5 | 2 |
| 18    | `prepare_add_record`             | 2397 | 1 | 1 | 5 | 0 |
| 15    | `prepare_add_prescription`       | 2706 | 1 | 1 | 3 | 0 |
| 13    | `commit_add_record`              | 2459 | 2 | 3 | 1 | 0 |
| 11    | `commit_grant_access`            | 2553 | 2 | 2 | 1 | 0 |
| 10    | `resolve_conflict`               | 1529 | 1 | 0 | 3 | 0 |
| 9     | `register_user`                  | 899  | 0 | 1 | 2 | 0 |
| 9     | `get_records_by_ids`             | 2899 | 2 | 0 | 1 | 1 |
| 8     | `get_records`                    | 2916 | 1 | 0 | 1 | 1 |
| 8     | `get_patient_records_paginated`  | 1587 | 1 | 0 | 1 | 0 |

The list is truncated to the top entries. The full scan covers every public
function in `lib.rs`.

## Candidates for the top-5 optimization pass

These are the five highest-ranked operations with the most structure that could
be reduced. They are candidates, not confirmed hot spots. Measure before
changing any of them.

1. **`grant_access`**: the highest score. Each grant writes the grant entry and
   may write the patient's grantee list. Before appending, it scans that list
   linearly to avoid duplicates, so its cost grows with the number of grantees
   the patient has. A membership check that does not scan the list would remove
   that growth. Any such change must keep the list free of duplicates after a
   revoke and re-grant.
2. **`add_records`**: batch write path with a per-record loop. Compare the
   per-record cost against `add_record` to see whether batching saves storage
   work or only saves transaction overhead.
3. **`add_record`**: the single-record path. It runs five access checks before
   the write. Measure how much of its cost is authorization.
4. **`get_record`**: five access checks in sequence, from a direct permission
   lookup to the grant lookups. The checks are combined with `||`, so the order
   changes cost but not the result. Keep that order unless the measured gain
   justifies a change.
5. **`export_patient_data`** (added for #321): one storage read per record,
   plus one examination read for each examination record, plus the grantee
   list. The page size is capped at `MAX_EXPORT_PAGE_SIZE` (50) in
   `src/portability.rs`. Measure the 50-record case before raising it.

Any change to an access-check path needs its own review, because the order and
short-circuiting of those checks is part of the authorization logic.

## Measured costs

| Operation | CPU instructions | Memory bytes | Commit | SDK |
|-----------|-----------------:|-------------:|--------|-----|
| _not yet recorded_ | | | | |

## Soroban limits

Compare the measured numbers with the per-transaction limits published for the
network you deploy to. Check the current Soroban documentation for those
limits. The limits change between protocol versions, so they are not copied
here.
