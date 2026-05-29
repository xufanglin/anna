//! anna 的转换核心：lossy 报告、适配器接口、pipeline。

pub mod adapter;
pub mod lossy;
pub mod pipeline;
pub mod scope;

pub use adapter::{
    AdapterError, AdapterResult, PlannedFile, PlannedMode, Reader, WritePlan, Writer, execute_plan,
};
pub use lossy::{LossyEntry, LossySide, LossySource, LossyTier, ids, render_report};
pub use pipeline::{PipelineOptions, PipelineOutcome, run_pipeline, run_pipeline_from_ir};
pub use scope::{Scope, default_global_root, detect_project_files};
