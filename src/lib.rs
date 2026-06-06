#[cfg(test)]
pub mod harness;

#[cfg(test)]
mod tests {
    pub mod end_to_end;
    pub mod phase_breakdown;
}

// Alpenglow consensus timing parameters.
// Timeout(i) = delta_timeout + delta_block
// where delta_timeout = 3 * delta (network latency of a staked node).
//
// We assume delta = 50ms, giving delta_timeout = 150ms.
// Banking stage production is judged against delta_block alone (400ms).
// The full Timeout(i) = 550ms is the consensus-layer deadline.
#[cfg(test)]
pub struct SlotBudget {
    pub delta_block_ms: u64,
    pub delta_ms: u64,
}

#[cfg(test)]
impl SlotBudget {
    pub const fn new() -> Self {
        Self {
            delta_block_ms: 400,
            delta_ms: 50,
        }
    }

    pub fn delta_timeout_ms(&self) -> u64 {
        3 * self.delta_ms
    }

    pub fn full_timeout_ms(&self) -> u64 {
        self.delta_timeout_ms() + self.delta_block_ms
    }

    pub fn verdict(&self, ms: u64) -> &'static str {
        if ms <= self.delta_block_ms {
            "PASS"
        } else {
            "OVER"
        }
    }
}

#[cfg(test)]
impl Default for SlotBudget {
    fn default() -> Self {
        Self::new()
    }
}