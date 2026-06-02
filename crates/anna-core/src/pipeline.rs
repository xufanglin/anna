//! 转换 pipeline 与交互式确认 helper。

use std::io::IsTerminal;
use std::path::{Path, PathBuf};

use anna_ir::{Ir, McpTransport};

use crate::adapter::{AdapterError, AdapterResult, Reader, WritePlan, Writer};
use crate::lossy::{LossyEntry, render_report};
use crate::scope::Scope;

/// pipeline 行为参数。
#[derive(Debug, Clone, Copy, Default)]
pub struct PipelineOptions {
    /// `--dry-run`：仅产出计划，不实际写文件。
    pub dry_run: bool,
    /// `--strict`：任何 lossy 条目都视作失败。
    pub strict: bool,
    /// `-y` / `--yes`：跳过交互提示。lossy 报告仍打印。
    pub yes: bool,
}

/// pipeline 的运行结果。退出码由调用方根据该枚举决定。
#[derive(Debug)]
pub enum PipelineOutcome {
    /// 写入完成（也可能没有任何 lossy）。
    Written { files: usize },
    /// dry-run 完成，未写入。
    DryRun { plan: WritePlan },
    /// strict 模式因 lossy 中止。
    StrictAborted { lossy: Vec<LossyEntry> },
    /// 用户在交互提示中拒绝。
    UserAborted,
}

/// 对 read+plan 阶段产生的 lossy 全集做一次性确认。
pub fn run_pipeline(
    reader: &dyn Reader,
    writer: &dyn Writer,
    src: &Path,
    src_scope: Scope,
    dst: &Path,
    dst_scope: Scope,
    opts: PipelineOptions,
) -> AdapterResult<PipelineOutcome> {
    let (mut ir, mut lossy) = reader.read(src, src_scope)?;
    expand_relative_paths(&mut ir, src);
    let mut plan = writer.plan(&ir, dst, dst_scope)?;
    lossy.append(&mut plan.lossy);
    plan.lossy = lossy;
    finalize(plan, opts, writer)
}

/// 与 `run_pipeline` 类似，但来源是已经反序列化好的 IR（用于 `import`）。
pub fn run_pipeline_from_ir(
    ir: &anna_ir::Ir,
    read_lossy: Vec<LossyEntry>,
    writer: &dyn Writer,
    dst: &Path,
    dst_scope: Scope,
    opts: PipelineOptions,
) -> AdapterResult<PipelineOutcome> {
    let mut plan = writer.plan(ir, dst, dst_scope)?;
    let mut combined = read_lossy;
    combined.append(&mut plan.lossy);
    plan.lossy = combined;
    finalize(plan, opts, writer)
}

fn finalize(
    plan: WritePlan,
    opts: PipelineOptions,
    writer: &dyn Writer,
) -> AdapterResult<PipelineOutcome> {
    let lossy_count = plan.lossy.len();

    // dry-run 路径：打印计划与 lossy 报告后直接返回。
    if opts.dry_run {
        print_plan(&plan);
        if lossy_count > 0 {
            print!("{}", render_report(&plan.lossy));
        }
        return Ok(PipelineOutcome::DryRun { plan });
    }

    // strict 路径：任何 lossy 立刻中止。
    if opts.strict && lossy_count > 0 {
        print!("{}", render_report(&plan.lossy));
        return Ok(PipelineOutcome::StrictAborted { lossy: plan.lossy });
    }

    // 始终先把 lossy 报告打出来（如果有的话）。
    if lossy_count > 0 {
        print!("{}", render_report(&plan.lossy));
    }

    // 是否需要交互提示？
    if lossy_count > 0 && !opts.yes {
        match prompt_user(&plan.lossy)? {
            ConfirmDecision::Proceed => {}
            ConfirmDecision::Abort => return Ok(PipelineOutcome::UserAborted),
        }
    }

    let count = plan.files.len();
    writer.write(&plan)?;
    Ok(PipelineOutcome::Written { files: count })
}

fn print_plan(plan: &WritePlan) {
    if plan.files.is_empty() {
        println!("dry-run：没有文件需要写入。");
        return;
    }
    println!("dry-run 写入计划（{} 个文件）", plan.files.len());
    for f in &plan.files {
        println!(
            "  [{}] {} ({} bytes)",
            f.mode.label(),
            f.path.display(),
            f.content.len()
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConfirmDecision {
    Proceed,
    Abort,
}

fn prompt_user(lossy: &[LossyEntry]) -> AdapterResult<ConfirmDecision> {
    use std::io::{BufRead, Write};

    // 非交互终端：当作 Abort（防止脚本里默默吞掉 loss）。
    let stdin = std::io::stdin();
    if !stdin.is_terminal() {
        eprintln!("非交互终端，跳过确认并中止（使用 -y 自动确认）");
        return Ok(ConfirmDecision::Abort);
    }

    let stdout = std::io::stdout();
    loop {
        {
            let mut out = stdout.lock();
            write!(out, "继续写入？[y/N/details/abort]: ").map_err(|e| AdapterError::Io {
                path: std::path::PathBuf::new(),
                source: e,
            })?;
            out.flush().map_err(|e| AdapterError::Io {
                path: std::path::PathBuf::new(),
                source: e,
            })?;
        }

        let mut line = String::new();
        stdin
            .lock()
            .read_line(&mut line)
            .map_err(|e| AdapterError::Io {
                path: std::path::PathBuf::new(),
                source: e,
            })?;
        let answer = line.trim().to_ascii_lowercase();
        match answer.as_str() {
            "y" | "yes" => return Ok(ConfirmDecision::Proceed),
            "" | "n" | "no" => return Ok(ConfirmDecision::Abort),
            "abort" | "a" => return Ok(ConfirmDecision::Abort),
            "details" | "d" => {
                // 重新打印一次报告（已含详情），然后再次提示。
                print!("{}", render_report(lossy));
                continue;
            }
            other => {
                println!("未识别的选项 '{other}'，请输入 y / N / details / abort。");
                continue;
            }
        }
    }
}

/// Expand relative paths (`./`, `../`) and tilde paths (`~/`) in MCP server
/// commands to absolute paths. Relative paths resolve against the source root;
/// tilde paths resolve against the user's home directory.
fn expand_relative_paths(ir: &mut Ir, src_root: &Path) {
    let home = dirs::home_dir();
    for srv in &mut ir.mcp_servers {
        if let McpTransport::Stdio { command, .. } = &mut srv.transport {
            if command.starts_with("./") || command.starts_with("../") {
                let resolved = src_root.join(&*command);
                if let Ok(abs) = resolved.canonicalize() {
                    *command = abs.to_string_lossy().into_owned();
                } else {
                    *command = normalize_path(&resolved);
                }
            } else if command.starts_with("~/") {
                if let Some(ref h) = home {
                    let resolved = h.join(&command[2..]);
                    *command = resolved.to_string_lossy().into_owned();
                }
            }
        }
    }
}

/// Normalize a path by resolving `.` and `..` components without requiring the path to exist.
fn normalize_path(path: &Path) -> String {
    let mut components = Vec::new();
    for comp in path.components() {
        match comp {
            std::path::Component::ParentDir => { components.pop(); }
            std::path::Component::CurDir => {}
            other => components.push(other),
        }
    }
    let result: PathBuf = components.iter().collect();
    result.to_string_lossy().into_owned()
}
