#[cfg(test)]
mod end_to_end {
    use {
        crate::{
            SlotBudget,
            harness::harness::{N_ITERS, make_genesis, summarise, percentiles},
        },
        agave_banking_stage_ingress_types::BankingPacketBatch,
        crossbeam_channel::unbounded,
        solana_core::{
            banking_stage::BankingStage,
            banking_stage::transaction_scheduler::scheduler_controller::SchedulerConfig,
            banking_trace::{BankingTracer, Channels},
            validator::{BlockProductionMethod, SchedulerPacing},
        },
        solana_entry::entry_or_marker::EntryOrMarker,
        solana_ledger::{
            blockstore::Blockstore,
            genesis_utils::GenesisConfigInfo,
            get_tmp_ledger_path_auto_delete,
        },
        solana_perf::packet::to_packet_batches,
        solana_poh::poh_recorder::create_test_recorder,
        solana_runtime::bank::Bank,
        solana_system_transaction as system_transaction,
        std::{
            num::NonZeroUsize,
            sync::{Arc, atomic::Ordering},
            thread::sleep,
            time::{Duration, Instant},
        },
        tokio::sync::mpsc,
    };

    /// Measures wall-clock time from packet send → committed entry arrival
    /// for N_ITERS consecutive single-transfer transactions through a live
    /// BankingStage instance.  Reports against the Alpenglow slot budget.
    #[test]
    fn measure_slot_commit_latency() {
        agave_logger::setup();

        let budget = SlotBudget::new();

        let GenesisConfigInfo {
            genesis_config,
            mint_keypair,
            ..
        } = make_genesis(1_000_000_000);

        let (bank, bank_forks) = Bank::new_with_bank_forks_for_tests(&genesis_config);
        let start_hash = bank.last_blockhash();

        let banking_tracer = BankingTracer::new_disabled();
        let Channels {
            non_vote_sender,
            non_vote_receiver,
            tpu_vote_sender,
            tpu_vote_receiver,
            gossip_vote_sender,
            gossip_vote_receiver,
        } = banking_tracer.create_channels();

        let ledger_path = get_tmp_ledger_path_auto_delete!();
        let blockstore = Arc::new(
            Blockstore::open(ledger_path.path()).expect("open blockstore"),
        );

        let (exit, poh_recorder, _poh_controller, transaction_recorder,
             poh_service, entry_receiver) =
            create_test_recorder(bank, blockstore, None, None);

        let (replay_vote_sender, _replay_vote_receiver) = unbounded();

        let banking_stage = BankingStage::new_num_threads(
    BlockProductionMethod::CentralSchedulerGreedy,
    poh_recorder.clone(),
    transaction_recorder,
    non_vote_receiver,
    tpu_vote_receiver,
    gossip_vote_receiver,
    mpsc::channel(1).1,
    NonZeroUsize::new(4).unwrap(),
    SchedulerConfig {
        scheduler_pacing: SchedulerPacing::Disabled,
    },
    None,
    replay_vote_sender,
    None,
    bank_forks,
    None,
    Arc::default(),
    Arc::default(), // SchedulerPriorityFloor — new argument
);

        let mut samples: Vec<u64> = Vec::with_capacity(N_ITERS);

        for i in 0..N_ITERS {
            let recipient = solana_pubkey::new_rand();
            let tx = system_transaction::transfer(
                &mint_keypair, &recipient, 1, start_hash,
            );
            let batches = to_packet_batches(&[tx], 1);

            let t0 = Instant::now();
            non_vote_sender
                .send(BankingPacketBatch::new(batches))
                .unwrap();

            let elapsed_ms = loop {
                if let Ok((_bank, (EntryOrMarker::Entry(e), _))) =
                    entry_receiver.try_recv()
                {
                    if !e.transactions.is_empty() {
                        break t0.elapsed().as_millis() as u64;
                    }
                }
                sleep(Duration::from_millis(1));
            };

            eprintln!("[iter {:>2}] commit latency: {:>4} ms", i, elapsed_ms);
            samples.push(elapsed_ms);
        }

        // Teardown
        drop(non_vote_sender);
        drop(tpu_vote_sender);
        drop(gossip_vote_sender);
        banking_stage.join().unwrap();
        exit.store(true, Ordering::Relaxed);
        poh_service.join().unwrap();
        drop(poh_recorder);
        for (_bank, (eom, _)) in entry_receiver.iter() {
            let _ = eom.unwrap_entry();
        }

        // Report
        let (min, mean, max) = summarise(&samples);
        let (p50, p95, p99) = percentiles(&samples);
        let jitter = max - min;

        println!("\n=== slot-pulse: end-to-end commit latency ({N_ITERS} iters) ===\n");
        println!("  min={min}ms  mean={mean}ms  max={max}ms  jitter={jitter}ms");
        println!("  p50={p50}ms  p95={p95}ms  p99={p99}ms\n");
        println!("Alpenglow slot budget (delta_block = {}ms):", budget.delta_block_ms);
        println!("  full timeout : {}ms", budget.full_timeout_ms());
        println!("  mean verdict : {}", budget.verdict(mean));
        println!("  max  verdict : {}", budget.verdict(max));
        println!("  p99  verdict : {}", budget.verdict(p99));

        assert_eq!(samples.len(), N_ITERS);
        assert!(samples.iter().all(|&ms| ms > 0));
    }
}