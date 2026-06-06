#[cfg(test)]
pub mod harness {
    use {
        solana_ledger::genesis_utils::{
            GenesisConfigInfo, bootstrap_validator_stake_lamports,
            create_genesis_config_with_leader,
        },
        solana_runtime_transaction::runtime_transaction::RuntimeTransaction,
        solana_transaction::{Transaction, sanitized::SanitizedTransaction},
    };

    // How many consecutive slot executions to measure.
    // Jitter = max - min across all iterations.
    pub const N_ITERS: usize = 10;

    /// Build a genesis config whose slot lives long enough for N_ITERS
    /// iterations without the slot expiring mid-run.
    pub fn make_genesis(lamports: u64) -> GenesisConfigInfo {
        let validator_pubkey = solana_pubkey::new_rand();
        let mut info = create_genesis_config_with_leader(
            lamports,
            &validator_pubkey,
            bootstrap_validator_stake_lamports(),
        );
        // Stretch the slot so it doesn't expire across N_ITERS iterations.
        info.genesis_config.ticks_per_slot *= 1024;
        info
    }

    /// Wrap raw transactions into RuntimeTransaction so they can be
    /// handed directly to the Consumer without going through the full
    /// SigVerify + deserialize pipeline.
    pub fn wrap_transactions(
        txs: Vec<Transaction>,
    ) -> Vec<RuntimeTransaction<SanitizedTransaction>> {
        txs.into_iter()
            .map(RuntimeTransaction::from_transaction_for_tests)
            .collect()
    }

    /// Compute min / mean / max over a slice of u64 samples.
    pub fn summarise(vals: &[u64]) -> (u64, u64, u64) {
        let min = *vals.iter().min().unwrap_or(&0);
        let max = *vals.iter().max().unwrap_or(&0);
        let mean = if vals.is_empty() {
            0
        } else {
            vals.iter().sum::<u64>() / vals.len() as u64
        };
        (min, mean, max)
    }

    /// Compute p50 / p95 / p99 percentiles.
    /// Gives more signal than just min/mean/max for spotting tail latency.
    pub fn percentiles(vals: &[u64]) -> (u64, u64, u64) {
        let mut sorted = vals.to_vec();
        sorted.sort_unstable();
        let n = sorted.len();
        let p = |pct: f64| sorted[((n as f64 * pct) as usize).min(n - 1)];
        (p(0.50), p(0.95), p(0.99))
    }
}