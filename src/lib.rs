pub mod batch;
pub mod batch_metadata;
pub mod contexts;
pub mod error;
pub mod failure;
pub mod loop_op;
pub mod macros;
pub mod op;
pub mod op_metadata;
pub mod ops;
pub mod structured_queries;
pub mod trigger;
pub mod wrappers;

#[cfg(test)]
mod control_flow_tests;

pub use batch::BatchOp;
pub use contexts::{DryContext, WetContext};
pub use error::OpError;
pub use failure::AttributionClass;
pub use loop_op::LoopOp;
pub use macros::AggregationStrategy;
pub use op::Op;
pub use op_metadata::{OpMetadata, TriggerFuse, ValidationReport};
pub use ops::{get_caller_trigger_name, perform, wrap_nested_op_exception};
pub use structured_queries::{
    generate_outline_schema, FlatOutlineEntry, ListingOutline, OutlineEntry, OutlineMetadata,
};
pub use trigger::*;
pub use wrappers::logging::LoggingWrapper;
pub use wrappers::timeout::TimeBoundWrapper;
pub use wrappers::validating::ValidatingWrapper;

pub type OpResult<T> = std::result::Result<T, OpError>;

pub mod prelude {
    pub use serde::{Deserialize, Serialize};
    pub use serde_json;
    pub use std::any::Any;
    pub use std::sync::Arc;
    pub use tracing::{debug, error, info, trace, warn};

    pub use crate::{DryContext, Op, OpError, OpMetadata, OpResult, WetContext};
    pub use async_trait::async_trait;
    pub use chrono::{DateTime, Utc};
    pub use std::collections::HashMap;
}
