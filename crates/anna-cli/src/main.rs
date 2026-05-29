use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, bail};
use clap::{Parser, Subcommand, ValueEnum};

use anna_adapters::claude_code::{ClaudeCodeReader, ClaudeCodeWriter};
use anna_adapters::codex::{CodexReader, CodexWriter};
use anna_adapters::copilot_cli::{CopilotCliReader, CopilotCliWriter};
use anna_adapters::cursor::{CursorReader, CursorWriter};
use anna_adapters::kiro::{KiroReader, KiroWriter};
use anna_adapters::opencode::{OpenCodeReader, OpenCodeWriter};
use anna_core::{
    PipelineOptions, PipelineOutcome, Reader, Scope, Writer, default_global_root,
    detect_project_files, render_report, run_pipeline, run_pipeline_from_ir,
};

#[derive(Parser)]
#[command(
    name = "anna",
    version,
    about = "Convert agent configurations between Kiro, Copilot CLI, OpenCode, Codex, Claude Code, and Cursor",
    before_help = r#"
⠀⠀⠀⠀⠀⠀⣀⣤⣤⣤⣀⠀⠀⠀⠀⠀⠀⠀⣠⠀⠀⠀⠀⠀
⠀⠀⠀⠀⢠⣾⡿⠟⠛⠻⢿⣷⡄⠀⠀⠀⣠⣾⣿⣇⠀⠀⠀⡀
⠀⠀⠀⠀⣿⣿⠁⣶⡆⠀⠈⣿⣿⠀⣠⣾⡿⢻⣿⣧⣤⣶⣿⣿
⠀⠀⠀⢀⣿⣿⣆⡀⠀⢀⣰⣿⣿⣾⡿⠋⢀⣾⣿⡿⠟⣿⣿⠏
⠀⢀⣴⡿⠛⠙⣿⣿⣿⣿⣿⣿⡿⠋⠀⢠⣾⡿⢃⣠⣾⡿⠋⠀
⢴⠟⠋⠀⠀⠀⢸⣿⣇⠀⣿⣿⡁⢀⣴⣿⣿⣵⣿⡿⠋⠀⠀⠀
⠀⠀⠀⠀⠀⠀⠀⢻⣿⣆⠘⠿⣿⣿⣿⣿⠿⠻⣿⣧⠀⠀⠀⠀
⠀⠀⠀⠀⠀⠀⠀⠀⠹⣿⣷⣄⡀⠀⠀⠀⠀⠀⢹⣿⣇⠀⠀⠀
⠀⠀⠀⠀⠀⠀⠀⠀⠀⠈⠙⠿⣿⣷⣶⣤⣤⡄⠀⣿⣿⠀⠀⠀
⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠉⠉⠛⢿⣿⣆⣿⣿⠀⠀⠀
⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢻⣿⣿⠇⠀⠀⠀
⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠘⣿⠏⠀⠀⠀⠀
⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠁⠀⠀⠀⠀⠀
  Calypte anna
"#
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Convert configuration from one agent to another.
    Convert {
        /// Source agent
        #[arg(long)]
        from: AgentName,
        /// Target agent
        #[arg(long)]
        to: AgentName,
        /// Scope: global (user-level) or project (local)
        #[arg(long, default_value = "global")]
        scope: ScopeArg,
        /// Show write plan without writing files
        #[arg(long)]
        dry_run: bool,
        /// Fail on any lossy entry (exit code 2)
        #[arg(long)]
        strict: bool,
        /// Skip interactive prompts
        #[arg(short = 'y', long)]
        yes: bool,
        /// Project root path (only valid with --scope project)
        path: Option<PathBuf>,
    },
    /// Export agent configuration to IR JSON.
    Export {
        /// Source agent
        #[arg(long)]
        from: AgentName,
        /// Scope
        #[arg(long, default_value = "global")]
        scope: ScopeArg,
        /// Output IR JSON file path
        #[arg(short, long)]
        output: PathBuf,
        /// Source root path (only valid with --scope project)
        path: Option<PathBuf>,
    },
    /// Import IR JSON into an agent's configuration.
    Import {
        /// Target agent
        #[arg(long)]
        to: AgentName,
        /// Scope
        #[arg(long, default_value = "global")]
        scope: ScopeArg,
        /// Skip interactive prompts
        #[arg(short = 'y', long)]
        yes: bool,
        /// Fail on any lossy entry (exit code 2)
        #[arg(long)]
        strict: bool,
        /// IR JSON file to import
        ir_file: PathBuf,
        /// Destination root path (only valid with --scope project)
        dst: Option<PathBuf>,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum AgentName {
    Kiro,
    #[value(name = "copilot-cli")]
    CopilotCli,
    #[value(name = "opencode")]
    OpenCode,
    #[value(name = "codex")]
    Codex,
    #[value(name = "claude-code")]
    ClaudeCode,
    #[value(name = "cursor")]
    Cursor,
}

#[derive(Clone, Copy, ValueEnum)]
enum ScopeArg {
    Global,
    Project,
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::from(3)
        }
    }
}

fn run() -> anyhow::Result<ExitCode> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Convert {
            from,
            to,
            scope,
            dry_run,
            strict,
            yes,
            path,
        } => {
            let scope = resolve_scope(scope, &path, from)?;
            let src_root = resolve_root(scope, path.clone(), from)?;
            let dst_root = resolve_root(scope, path, to)?;

            // Scope auto-detection prompt
            let scope = maybe_prompt_scope(scope, &src_root, from, yes)?;
            let src_root = if scope == Scope::Project && src_root == default_global_root(agent_id(from)).unwrap_or_default() {
                std::env::current_dir().context("cannot determine CWD")?
            } else {
                src_root
            };
            let dst_root = if scope == Scope::Project && dst_root == default_global_root(agent_id(to)).unwrap_or_default() {
                std::env::current_dir().context("cannot determine CWD")?
            } else {
                dst_root
            };

            let reader = make_reader(from);
            let writer = make_writer(to);
            let opts = PipelineOptions { dry_run, strict, yes };

            let outcome = run_pipeline(
                reader.as_ref(),
                writer.as_ref(),
                &src_root,
                scope,
                &dst_root,
                scope,
                opts,
            )?;

            Ok(outcome_to_exit_code(outcome))
        }
        Commands::Export {
            from,
            scope,
            output,
            path,
        } => {
            let scope = resolve_scope(scope, &path, from)?;
            let src_root = resolve_root(scope, path, from)?;

            let reader = make_reader(from);
            let (ir, lossy) = reader.read(&src_root, scope)?;

            if !lossy.is_empty() {
                eprint!("{}", render_report(&lossy));
            }

            let json = anna_ir::to_string_pretty(&ir)
                .context("failed to serialize IR")?;
            std::fs::write(&output, &json)
                .with_context(|| format!("failed to write {}", output.display()))?;

            println!("Exported to {}", output.display());
            Ok(ExitCode::SUCCESS)
        }
        Commands::Import {
            to,
            scope,
            yes,
            strict,
            ir_file,
            dst,
        } => {
            let scope = resolve_scope(scope, &dst, to)?;
            let dst_root = resolve_root(scope, dst, to)?;

            let raw = std::fs::read_to_string(&ir_file)
                .with_context(|| format!("failed to read {}", ir_file.display()))?;
            let ir = anna_ir::from_str(&raw)
                .with_context(|| format!("failed to parse IR from {}", ir_file.display()))?;

            let writer = make_writer(to);
            let opts = PipelineOptions {
                dry_run: false,
                strict,
                yes,
            };

            let outcome = run_pipeline_from_ir(&ir, Vec::new(), writer.as_ref(), &dst_root, scope, opts)?;
            Ok(outcome_to_exit_code(outcome))
        }
    }
}

fn resolve_scope(scope_arg: ScopeArg, path: &Option<PathBuf>, _agent: AgentName) -> anyhow::Result<Scope> {
    let scope = match scope_arg {
        ScopeArg::Global => Scope::Global,
        ScopeArg::Project => Scope::Project,
    };

    // global + path → error
    if scope == Scope::Global && path.is_some() {
        bail!("cannot specify a path with --scope global");
    }

    Ok(scope)
}

fn resolve_root(scope: Scope, path: Option<PathBuf>, agent: AgentName) -> anyhow::Result<PathBuf> {
    match scope {
        Scope::Global => {
            let id = agent_id(agent);
            default_global_root(id)
                .with_context(|| format!("cannot determine global root for agent '{id}'"))
        }
        Scope::Project => {
            Ok(path.unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))))
        }
    }
}

fn maybe_prompt_scope(scope: Scope, _root: &PathBuf, agent: AgentName, yes: bool) -> anyhow::Result<Scope> {
    use std::io::IsTerminal;

    if scope != Scope::Global || yes || !std::io::stdin().is_terminal() {
        return Ok(scope);
    }

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let id = agent_id(agent);
    let hits = detect_project_files(id, &cwd);

    if hits.is_empty() {
        return Ok(scope);
    }

    let file_list: Vec<_> = hits.iter().map(|p| p.display().to_string()).collect();
    eprintln!(
        "检测到当前目录下存在 {} 的项目级配置（{}）。",
        id,
        file_list.join(", ")
    );
    eprint!("是否切换到 --scope project ？[Y/n] ");

    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    let answer = line.trim().to_ascii_lowercase();

    if answer.is_empty() || answer == "y" || answer == "yes" {
        Ok(Scope::Project)
    } else {
        Ok(Scope::Global)
    }
}

fn agent_id(agent: AgentName) -> &'static str {
    match agent {
        AgentName::Kiro => "kiro",
        AgentName::CopilotCli => "copilot-cli",
        AgentName::OpenCode => "opencode",
        AgentName::Codex => "codex",
        AgentName::ClaudeCode => "claude-code",
        AgentName::Cursor => "cursor",
    }
}

fn make_reader(agent: AgentName) -> Box<dyn Reader> {
    match agent {
        AgentName::Kiro => Box::new(KiroReader),
        AgentName::CopilotCli => Box::new(CopilotCliReader),
        AgentName::OpenCode => Box::new(OpenCodeReader),
        AgentName::Codex => Box::new(CodexReader),
        AgentName::ClaudeCode => Box::new(ClaudeCodeReader),
        AgentName::Cursor => Box::new(CursorReader),
    }
}

fn make_writer(agent: AgentName) -> Box<dyn Writer> {
    match agent {
        AgentName::Kiro => Box::new(KiroWriter),
        AgentName::CopilotCli => Box::new(CopilotCliWriter),
        AgentName::OpenCode => Box::new(OpenCodeWriter),
        AgentName::Codex => Box::new(CodexWriter),
        AgentName::ClaudeCode => Box::new(ClaudeCodeWriter),
        AgentName::Cursor => Box::new(CursorWriter),
    }
}

fn outcome_to_exit_code(outcome: PipelineOutcome) -> ExitCode {
    match outcome {
        PipelineOutcome::Written { files } => {
            println!("写入完成（{files} 个文件）。");
            ExitCode::SUCCESS
        }
        PipelineOutcome::DryRun { .. } => ExitCode::SUCCESS,
        PipelineOutcome::StrictAborted { .. } => ExitCode::from(2),
        PipelineOutcome::UserAborted => ExitCode::from(1),
    }
}
