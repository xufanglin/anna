//! 适配器接口（Reader / Writer）以及写入计划类型。

use std::path::{Path, PathBuf};

use anna_ir::Ir;

use crate::lossy::LossyEntry;
use crate::scope::Scope;

/// 写入计划中单个文件的模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlannedMode {
    Create,
    Overwrite,
}

impl PlannedMode {
    pub fn label(self) -> &'static str {
        match self {
            PlannedMode::Create => "Create",
            PlannedMode::Overwrite => "Overwrite",
        }
    }
}

/// 写入计划中的一项。
#[derive(Debug, Clone)]
pub struct PlannedFile {
    pub path: PathBuf,
    pub content: Vec<u8>,
    pub mode: PlannedMode,
}

/// 一次写入计划：要写哪些文件 + 写入侧产生的 lossy 条目。
#[derive(Debug, Clone, Default)]
pub struct WritePlan {
    pub files: Vec<PlannedFile>,
    pub lossy: Vec<LossyEntry>,
}

/// 适配器错误。各 adapter 自己的错误类型最终归一到这里。
#[derive(Debug, thiserror::Error)]
pub enum AdapterError {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("parse error in {path}: {message}")]
    Parse { path: PathBuf, message: String },
    #[error("unsupported feature: {0}")]
    Unsupported(String),
}

pub type AdapterResult<T> = Result<T, AdapterError>;

/// 来源 reader：把 agent 原生布局读为 IR + 一组读侧 lossy 条目。
pub trait Reader: Send + Sync {
    fn agent_id(&self) -> &'static str;
    fn read(&self, root: &Path, scope: Scope) -> AdapterResult<(Ir, Vec<LossyEntry>)>;
}

/// 目标 writer：先 plan、再 write。dry-run 只跑 plan。
pub trait Writer: Send + Sync {
    fn agent_id(&self) -> &'static str;
    fn plan(&self, ir: &Ir, root: &Path, scope: Scope) -> AdapterResult<WritePlan>;
    fn write(&self, plan: &WritePlan) -> AdapterResult<()>;
}

/// 默认 `write` 实现：把所有 `PlannedFile` 真实写到磁盘。
///
/// - `Create`：目标文件不存在时写入；已存在则报错。
/// - `Overwrite`：无条件写入（覆盖已有文件）。
///
/// 各 adapter 一般直接调它而不是自己实现。
pub fn execute_plan(plan: &WritePlan) -> AdapterResult<()> {
    for file in &plan.files {
        if let Some(parent) = file.path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| AdapterError::Io {
                path: parent.to_path_buf(),
                source: e,
            })?;
        }
        if file.mode == PlannedMode::Create && file.path.exists() {
            eprintln!(
                "note: {} already exists, skipping (source and target share this file)",
                file.path.display()
            );
            continue;
        }
        std::fs::write(&file.path, &file.content).map_err(|e| AdapterError::Io {
            path: file.path.clone(),
            source: e,
        })?;
    }
    Ok(())
}
