#[cfg(test)]
mod phase_breakdown {
    use {
        crate::harness::harness::{
            N_ITERS, make_genesis, summarise, percentiles, wrap_transactions,
        },
        crossbeam_channel::unbounded,
        solana_core::banking_stage::{
            committer::Committer,
            consumer::Consumer,
        },
        solana_ledger::genesis_utils::GenesisConfigInfo,
        solana_poh::{
            record_channels::record_channels,
            transaction_recorder::TransactionRecorder,
        },
        solana_runtime::bank::Bank,
        solana_system_transaction as system_transaction,
    };

    /// Measures the four internal phases of Consumer::process_and_record_transactions:
    ///   load_execute_us  — SVM execution + account loading
    ///   freeze_lock_us   — bank freeze contention
    ///   record_us        — PoH record I/O
    ///   commit_us        — state write-back
    ///
    /// Exposes which phase dominates and where cold-start jitter originates.
    #[test]
    fn measure_phase_timings() {
        let GenesisConfigInfo {
            genesis_config,
            mint_keypair,
            ..
        } = make_genesis(1_000_000_000);

        let (bank, _forks) = Bank::new_with_bank_forks_for_tests(&genesis_config);

        let (record_sender, mut record_receiver) = record_channels(false);
        let recorder = TransactionRecorder::new(record_sender);
        record_receiver.restart(bank.bank_id());

        let (replay_vote_sender, _) = unbounded();
        let committer = Committer::new(None, replay_vote_sender, None);
        let consumer = Consumer::new(committer, recorder, None);

        // Per-phase sample buckets
        let mut load_exec = Vec::with_capacity(N_ITERS);
        let mut freeze    = Vec::with_capacity(N_ITERS);
        let mut record    = Vec::with_capacity(N_ITERS);
        let mut commit    = Vec::with_capacity(N_ITERS);

        for i in 0..N_ITERS {
            let start_hash = bank.last_blockhash();
            let recipient  = solana_pubkey::new_rand();
            let tx  = system_transaction::transfer(&mint_keypair, &recipient, 1, start_hash);
            let txs = wrap_transactions(vec![tx]);

            let output = consumer.process_and_record_transactions(&bank, &txs);
            let t = &output
                .execute_and_commit_transactions_output
                .execute_and_commit_timings;

            eprintln!(
                "[iter {:>2}]  load_execute={:>6}µs  \
                 freeze_lock={:>6}µs  record={:>6}µs  commit={:>6}µs",
                i, t.load_execute_us, t.freeze_lock_us, t.record_us, t.commit_us,
            );

            load_exec.push(t.load_execute_us);
            freeze.push(t.freeze_lock_us);
            record.push(t.record_us);
            commit.push(t.commit_us);
        }

        // Report
        println!("\n=== slot-pulse: phase breakdown ({N_ITERS} iters) ===\n");
        println!(
            "  {:<14}  {:>6}  {:>6}  {:>6}  {:>6}  {:>6}  {:>6}  {:>6}",
            "phase", "min", "mean", "max", "jitter", "p50", "p95", "p99"
        );

        for (name, vals) in [
            ("load_execute", &load_exec),
            ("freeze_lock",  &freeze),
            ("record",       &record),
            ("commit",       &commit),
        ] {
            let (min, mean, max) = summarise(vals);
            let (p50, p95, p99) = percentiles(vals);
            println!(
                "  {:<14}  {:>6}  {:>6}  {:>6}  {:>6}  {:>6}  {:>6}  {:>6}",
                name, min, mean, max,
                max.saturating_sub(min),
                p50, p95, p99,
            );
        }
    }
}