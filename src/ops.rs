use crate::prelude::*;
// OPS utility module - Central execution and utility functions
// Implements Java OPS class functionality with Rust enhancements

use crate::op::Op;
use crate::wrappers::logging::LoggingWrapper;
use crate::{DryContext, OpError, WetContext};
use std::panic::Location;

/// Central execution function with automatic logging wrapper
/// Equivalent to Java OPS.perform() method
pub async fn perform<T>(
    op: Box<dyn Op<T>>,
    dry: &mut DryContext,
    wet: &mut WetContext,
) -> OpResult<T>
where
    T: Send + 'static,
{
    // Get caller information for dynamic op naming
    let trigger_name = get_caller_trigger_name();

    // Wrap op with logging (matches Java behavior)
    let logged_op = LoggingWrapper::new(op, trigger_name);

    // Execute with logging
    logged_op.perform(dry, wet).await
}

/// Stack trace analysis to get caller class name
/// Equivalent to Java getCallerCallerClassName()
#[track_caller]
pub fn get_caller_trigger_name() -> String {
    let location = Location::caller();
    format!(
        "{}::{}",
        location
            .file()
            .split('/')
            .last()
            .unwrap_or("unknown")
            .replace(".rs", ""),
        location.line()
    )
}

/// Wrap nested op exception with context
/// Equivalent to Java wrapNestedOpException(String, Exception)
pub fn wrap_nested_op_exception(trigger_name: &str, error: OpError) -> OpError {
    match error {
        OpError::ExecutionFailed(msg) => {
            OpError::ExecutionFailed(format!("Op '{}' failed: {}", trigger_name, msg))
        }
        OpError::Timeout { timeout_ms } => OpError::ExecutionFailed(format!(
            "Op '{}' timed out after {}ms",
            trigger_name, timeout_ms
        )),
        OpError::Context(msg) => {
            OpError::Context(format!("Op '{}' context error: {}", trigger_name, msg))
        }
        OpError::BatchFailed(msg) => {
            OpError::BatchFailed(format!("Batch op '{}' failed: {}", trigger_name, msg))
        }
        // Classified failures keep their identity through wrapping — the
        // wrap adds human context to the CHAIN, never touches class/code.
        OpError::WrappedClassified {
            chain,
            code,
            class,
            reason,
            arg_urn,
            hint,
        } => OpError::WrappedClassified {
            chain: format!("Batch op '{}' failed: {}", trigger_name, chain),
            code,
            class,
            reason,
            arg_urn,
            hint,
        },
        OpError::Classified {
            code,
            class,
            message,
            arg_urn,
            hint,
        } => OpError::WrappedClassified {
            chain: format!("Op '{}' failed: {}: {}", trigger_name, code, message),
            code,
            class,
            reason: message,
            arg_urn,
            hint,
        },
        OpError::Aborted(reason) => {
            // Aborted errors should preserve their nature and not be wrapped as execution failures
            OpError::Aborted(format!("Op '{}' aborted: {}", trigger_name, reason))
        }
        OpError::Trigger(msg) => {
            OpError::Trigger(format!("Op '{}' internal error: {}", trigger_name, msg))
        }
        OpError::Other(boxed_error) => {
            OpError::ExecutionFailed(format!("Op '{}' failed: {}", trigger_name, boxed_error))
        }
    }
}

/// Wrap nested op exception without op name
/// Equivalent to Java wrapNestedOpException(Exception)
pub fn wrap_nested_exception(error: Box<dyn std::error::Error + Send + Sync>) -> OpError {
    OpError::Other(error)
}

/// Convert any error to OpError with context
/// Equivalent to Java wrapNestedRuntimeException(Exception)
pub fn wrap_runtime_exception(error: Box<dyn std::error::Error + Send + Sync>) -> OpError {
    OpError::ExecutionFailed(format!("Runtime error: {}", error))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestOp;

    #[async_trait]
    impl Op<i32> for TestOp {
        async fn perform(&self, _dry: &mut DryContext, _wet: &mut WetContext) -> OpResult<i32> {
            Ok(42)
        }

        fn metadata(&self) -> OpMetadata {
            OpMetadata::builder("TestOp").build()
        }
    }

    // TEST0005: Confirm the perform() utility wraps an op with automatic logging and returns its result
    #[tokio::test]
    async fn test0005_perform_with_auto_logging() {
        let mut dry = DryContext::new();
        let mut wet = WetContext::new();

        let op = Box::new(TestOp);

        let result = perform(op, &mut dry, &mut wet).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }

    // TEST0006: Verify get_caller_trigger_name() returns a string containing the module path with "::"
    #[test]
    fn test0006_caller_trigger_name() {
        let name = get_caller_trigger_name();
        assert!(name.contains("ops"));
        assert!(name.contains("::"));
    }

    // TEST0007: Confirm wrap_nested_op_exception wraps an error with the op name in the message
    #[test]
    fn test0007_wrap_nested_op_exception() {
        let original_error = OpError::ExecutionFailed("original error".to_string());
        let wrapped = wrap_nested_op_exception("TestOp", original_error);

        match wrapped {
            OpError::ExecutionFailed(msg) => {
                assert!(msg.contains("TestOp"));
                assert!(msg.contains("original error"));
            }
            _ => panic!("Expected ExecutionFailed error"),
        }
    }

    // TEST1903: wrapping preserves a classified failure's identity — the
    // wrap enriches the human CHAIN only, never the class/code/reason
    // (docs/failure-taxonomy.md).
    #[test]
    fn test1903_wrap_preserves_classification() {
        use crate::failure::AttributionClass;

        let classified = OpError::Classified {
            code: "CONTEXT_OVERFLOW".to_string(),
            class: AttributionClass::Input,
            message: "prompt too large".to_string(),
            arg_urn: Some("media:prompt;textable".to_string()),
            hint: None,
        };
        let wrapped = wrap_nested_op_exception("GenerateOp", classified);
        match wrapped {
            OpError::WrappedClassified {
                chain,
                code,
                class,
                reason,
                arg_urn,
                ..
            } => {
                assert!(chain.contains("GenerateOp"), "the wrap names the op");
                assert!(chain.contains("prompt too large"));
                assert_eq!(code, "CONTEXT_OVERFLOW");
                assert_eq!(class, AttributionClass::Input);
                assert_eq!(reason, "prompt too large");
                assert_eq!(
                    arg_urn.as_deref(),
                    Some("media:prompt;textable"),
                    "the origin's argument attribution survives wrapping verbatim"
                );
            }
            other => panic!("expected WrappedClassified, got {:?}", other),
        }

        // Re-wrapping an already-wrapped classified failure only grows the
        // chain — the identity fields are untouched.
        let rewrapped = wrap_nested_op_exception(
            "OuterBatch",
            OpError::WrappedClassified {
                chain: "Op 'GenerateOp' failed: CONTEXT_OVERFLOW: prompt too large".to_string(),
                code: "CONTEXT_OVERFLOW".to_string(),
                class: AttributionClass::Input,
                reason: "prompt too large".to_string(),
                arg_urn: Some("media:prompt;textable".to_string()),
                hint: None,
            },
        );
        match rewrapped {
            OpError::WrappedClassified {
                chain,
                code,
                class,
                reason,
                arg_urn,
                ..
            } => {
                assert!(chain.contains("OuterBatch"));
                assert!(chain.contains("GenerateOp"));
                assert_eq!(code, "CONTEXT_OVERFLOW");
                assert_eq!(class, AttributionClass::Input);
                assert_eq!(reason, "prompt too large");
                assert_eq!(arg_urn.as_deref(), Some("media:prompt;textable"));
            }
            other => panic!("expected WrappedClassified, got {:?}", other),
        }
    }

    // TEST1904: the emit source's hint — its next step for the user — is
    // carried verbatim through wrapping and re-wrapping, and a failure that
    // declared none stays without one: wrapping never supplies a hint.
    #[test]
    fn test1904_wrap_carries_hint() {
        use crate::failure::AttributionClass;

        let refusal = OpError::Classified {
            code: "NO_HANDLER".to_string(),
            class: AttributionClass::Environment,
            message: "cartridge ggufcartridge is not installed".to_string(),
            arg_urn: None,
            hint: Some("capdag cartridge install ggufcartridge".to_string()),
        };
        let wrapped = wrap_nested_op_exception("PeerCallOp", refusal);
        assert_eq!(
            wrapped.failure_hint(),
            Some("capdag cartridge install ggufcartridge"),
            "the origin's next step survives wrapping verbatim"
        );
        let rewrapped = wrap_nested_op_exception("SummarizeOp", wrapped);
        assert!(rewrapped.to_string().contains("SummarizeOp"));
        assert_eq!(
            rewrapped.failure_hint(),
            Some("capdag cartridge install ggufcartridge"),
            "re-wrapping keeps the origin's next step"
        );

        let unhinted = OpError::Classified {
            code: "NO_HANDLER".to_string(),
            class: AttributionClass::Environment,
            message: "no handler for the cap".to_string(),
            arg_urn: None,
            hint: None,
        };
        let rewrapped = wrap_nested_op_exception(
            "SummarizeOp",
            wrap_nested_op_exception("PeerCallOp", unhinted),
        );
        assert_eq!(
            rewrapped.failure_hint(),
            None,
            "no hint declared means none served through any wrapping"
        );
    }

    // TEST0008: Verify wrap_runtime_exception converts a boxed std error into an OpError::ExecutionFailed
    #[test]
    fn test0008_wrap_runtime_exception() {
        let error = Box::new(std::io::Error::new(std::io::ErrorKind::Other, "test error"));
        let wrapped = wrap_runtime_exception(error);

        match wrapped {
            OpError::ExecutionFailed(msg) => {
                assert!(msg.contains("Runtime error"));
                assert!(msg.contains("test error"));
            }
            _ => panic!("Expected ExecutionFailed error"),
        }
    }
}
