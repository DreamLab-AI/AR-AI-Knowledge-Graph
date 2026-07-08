#![no_main]

//! Fuzz target for the agent-presence wire decoder (opcode 0x44).
//!
//! Sibling of `wire_decode` (opcode 0x43). Same contract: `decode_agent_presence
//! (any &[u8]) -> Result<AgentPresenceBatch, WireError>` must be total — no
//! panics, no UB, no out-of-bounds reads, for any input. Any divergence is a P1
//! release blocker per PRD-QE-002 §4.6.

use libfuzzer_sys::fuzz_target;
use visionclaw_xr_presence::agent_presence::decode_agent_presence;

fuzz_target!(|data: &[u8]| {
    let _ = decode_agent_presence(data);
});
