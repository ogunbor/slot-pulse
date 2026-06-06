# slot-pulse

A benchmarking harness for measuring Agave's banking-stage timing against the
Alpenglow consensus slot budget. Built inside the Agave repository.

## What it measures

Solana's banking stage must produce a committed block within the slot deadline
defined by the Alpenglow consensus protocol (Figure 2 of the whitepaper):

```
Timeout(i) = delta_timeout + delta_block
           = 150ms + 400ms
           = 550ms
```

where:
- `delta_block = 400ms` — the protocol-specified block time
- `delta_timeout = 3 * delta` — conservative bound from the paper
- `delta = 50ms` — assumed network latency of a staked node

Banking stage production is judged against `delta_block` (400ms) alone.
The full `Timeout(i) = 550ms` is the consensus-layer deadline.

## Two harnesses

### 1. End-to-end commit latency (`measure_slot_commit_latency`)
Measures wall-clock time from packet send to committed entry arrival through
a live `BankingStage` instance. Reports min, mean, max, jitter, and p50/p95/p99
percentiles against the Alpenglow slot budget.

### 2. Phase breakdown (`measure_phase_timings`)
Measures the four internal phases of `Consumer::process_and_record_transactions`:

| Phase | What it measures |
|---|---|
| `load_execute_us` | SVM execution + account loading |
| `freeze_lock_us` | Bank freeze contention |
| `record_us` | PoH record I/O |
| `commit_us` | State write-back |

Reports min, mean, max, jitter, and p50/p95/p99 per phase — exposing which
phase dominates and where cold-start jitter originates.

## Sample results

### End-to-end
```
min=2ms  mean=20ms  max=179ms  jitter=177ms
p50=4ms  p95=179ms  p99=179ms

Alpenglow slot budget (delta_block = 400ms):
  full timeout : 550ms
  mean verdict : PASS
  max  verdict : PASS
  p99  verdict : PASS
```

### Phase breakdown
```
phase           min   mean    max  jitter   p50   p95   p99
load_execute    312    370    421     109   377   421   421
freeze_lock       0      0      0       0     0     0     0
record           33     42     59      26    44    59    59
commit           93    134    207     114   145   207   207
```

The iter 0 spike in end-to-end latency (179ms) is a cold-start artifact —
lazy program loading, cache warming, and allocator growth. Steady-state
after that is 2–5ms. `load_execute` dominates at steady-state (~370µs mean),
consistent with the SVM execution being the primary cost per transaction.
`freeze_lock` is zero throughout, indicating no bank freeze contention under
single-transaction load.

## Project structure

```
slot-pulse/
├── Cargo.toml
└── src/
    ├── lib.rs                    # SlotBudget struct + module declarations
    ├── harness.rs                # Shared test helpers (genesis, stats, sanitize)
    └── tests/
        ├── end_to_end.rs         # End-to-end commit latency harness
        └── phase_breakdown.rs    # Per-phase timing harness
```

## How to run

From the Agave repo root:

```bash
cargo test -p slot-pulse -- --nocapture
```

Run a single harness:

```bash
cargo test -p slot-pulse measure_slot_commit_latency -- --nocapture
cargo test -p slot-pulse measure_phase_timings -- --nocapture
```

## Important: source patches required

The phase breakdown harness requires two temporary patches to the Agave source
before running, and must be reverted afterwards.

**Apply patches:**
```bash
sed -i 's/^mod committer;$/pub mod committer;/' core/src/banking_stage.rs
sed -i 's/^mod consumer;$/pub mod consumer;/' core/src/banking_stage.rs
sed -i 's/pub(crate) execute_and_commit_timings/pub execute_and_commit_timings/' core/src/banking_stage/consumer.rs
```

**Revert patches:**
```bash
sed -i 's/^pub mod committer;$/mod committer;/' core/src/banking_stage.rs
sed -i 's/^pub mod consumer;$/mod consumer;/' core/src/banking_stage.rs
sed -i 's/pub execute_and_commit_timings/pub(crate) execute_and_commit_timings/' core/src/banking_stage/consumer.rs
```

## Limitations

- Single-transaction batches only — does not capture scheduler contention
- Isolated `Consumer` — no full multi-thread scheduler
- `delta = 50ms` is an assumption, not a measured value
- Cold-start iter 0 inflates max and p99 in end-to-end results

## Built against

- Agave `v4.2.0-alpha.0`
- Rust `1.96.0`