//! The failure taxonomy — WHOSE problem a failure is.
//!
//! Defined ONCE here in `ops` (the leaf crate of the error path — capdag
//! depends on ops, so this is the deepest shared home) and re-exported by
//! capdag as the cartridge-contract surface (`capdag::AttributionClass`).
//! Cartridge error enums declare a `attribution_class()` per variant beside
//! `error_code()`; the bifaci ERR frame carries the class over the wire (all
//! four language runtimes mirror it); the orchestrator and the engine carry
//! it structurally to the run record. No layer ever infers another layer's
//! class from message text — an error that reaches a boundary without a
//! declared class is a contract violation. Attribution is always emitted at
//! source and is never inferred from message text.
//! See `docs/failure-taxonomy.md` (repo root) for the full architecture and
//! `capdag/docs/17.2-error-handling.md` for the protocol contract.

use serde::{Deserialize, Serialize};

/// Whose problem a failure is. Declared at the error's DEFINITION site,
/// carried structurally through every hop.
///
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AttributionClass {
    /// Deterministic on the INPUT (context overflow, invalid request,
    /// unsupported format). The user's to fix; retrying can never succeed —
    /// tasks failing with this class are marked permanently failed.
    Input,
    /// A compute resource was exhausted (GPU VRAM, host memory). Often
    /// transient (another process holding memory) — retryable.
    Resource,
    /// The environment failed (network, registry, model download/integrity,
    /// cartridge process death). Transient by nature — retryable.
    Environment,
    /// Everything else: a defect in the engine or a cartridge. Ours, said
    /// plainly. Retryable (races un-race), but never blamed on the user.
    Internal,
}

impl AttributionClass {
    /// The wire token — used in the ERR frame meta, the machine_runs
    /// columns, the gRPC proto, and the loom. One vocabulary everywhere.
    pub fn as_str(&self) -> &'static str {
        match self {
            AttributionClass::Input => "input",
            AttributionClass::Resource => "resource",
            AttributionClass::Environment => "environment",
            AttributionClass::Internal => "internal",
        }
    }

    /// Parse a wire token. Callers must reject `None` as a protocol error.
    pub fn from_wire(token: &str) -> Option<AttributionClass> {
        match token {
            "input" => Some(AttributionClass::Input),
            "resource" => Some(AttributionClass::Resource),
            "environment" => Some(AttributionClass::Environment),
            "internal" => Some(AttributionClass::Internal),
            _ => None,
        }
    }

    /// Whether retrying can NEVER succeed: the failure is a deterministic
    /// function of the input. Resource/environment/internal stay retryable
    /// (memory frees up, networks recover, races un-race).
    pub fn is_permanent(&self) -> bool {
        matches!(self, AttributionClass::Input)
    }
}

impl std::fmt::Display for AttributionClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // TEST1730: the wire vocabulary round-trips exactly and rejects unknowns.
    #[test]
    fn test1730_wire_tokens_round_trip() {
        for class in [
            AttributionClass::Input,
            AttributionClass::Resource,
            AttributionClass::Environment,
            AttributionClass::Internal,
        ] {
            assert_eq!(AttributionClass::from_wire(class.as_str()), Some(class));
        }
        assert_eq!(AttributionClass::from_wire("user-error"), None);
        assert_eq!(AttributionClass::from_wire(""), None);
    }

    // TEST1731: only Input is permanent — the retry machinery keys on this.
    #[test]
    fn test1731_only_input_is_permanent() {
        assert!(AttributionClass::Input.is_permanent());
        assert!(!AttributionClass::Resource.is_permanent());
        assert!(!AttributionClass::Environment.is_permanent());
        assert!(!AttributionClass::Internal.is_permanent());
    }
}
