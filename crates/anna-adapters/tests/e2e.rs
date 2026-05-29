//! End-to-end integration tests for anna adapters.

use std::path::PathBuf;

use anna_adapters::copilot_cli::{CopilotCliReader, CopilotCliWriter};
use anna_adapters::kiro::{KiroReader, KiroWriter};
use anna_adapters::opencode::{OpenCodeReader, OpenCodeWriter};
use anna_core::adapter::{Reader, Writer};
use anna_core::lossy::{LossyEntry, ids};
use anna_core::pipeline::{PipelineOptions, PipelineOutcome, run_pipeline};
use anna_core::scope::Scope;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tests")
        .join("fixtures")
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "anna-test-{}-{}",
        name,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn has_lossy_id(lossy: &[LossyEntry], id: &str) -> bool {
    lossy.iter().any(|e| e.id == id)
}

// 9.1-9.3: Fixtures are created as files above.

// 9.4: convert kiro → opencode --scope project
#[test]
fn convert_kiro_to_opencode_project() {
    let src = fixtures_dir().join("kiro-project");
    let dst = temp_dir("kiro-to-opencode");

    let reader = KiroReader;
    let writer = OpenCodeWriter;
    let opts = PipelineOptions { dry_run: false, strict: false, yes: true };

    let outcome = run_pipeline(&reader, &writer, &src, Scope::Project, &dst, Scope::Project, opts).unwrap();

    match outcome {
        PipelineOutcome::Written { files } => assert!(files > 0),
        other => panic!("expected Written, got {other:?}"),
    }

    // Check output files exist
    assert!(dst.join("AGENTS.md").is_file());
    assert!(dst.join("opencode.jsonc").is_file());
    assert!(dst.join(".opencode/skills/my-skill/SKILL.md").is_file());

    // Check lossy entries from the plan
    let (ir, _) = reader.read(&src, Scope::Project).unwrap();
    let plan = writer.plan(&ir, &dst, Scope::Project).unwrap();

    assert!(has_lossy_id(&plan.lossy, ids::KIRO_STEERING_FILEMATCH_LOST));
    assert!(has_lossy_id(&plan.lossy, ids::KIRO_STEERING_ALWAYS_MERGED));
    assert!(has_lossy_id(&plan.lossy, ids::KIRO_MCP_AUTOAPPROVE_DROPPED));

    std::fs::remove_dir_all(&dst).unwrap();
}

// 9.5: convert opencode → kiro --scope project
#[test]
fn convert_opencode_to_kiro_project() {
    let src = fixtures_dir().join("opencode-project");
    let dst = temp_dir("opencode-to-kiro");

    let reader = OpenCodeReader;
    let writer = KiroWriter;
    let opts = PipelineOptions { dry_run: false, strict: false, yes: true };

    let outcome = run_pipeline(&reader, &writer, &src, Scope::Project, &dst, Scope::Project, opts).unwrap();

    match outcome {
        PipelineOutcome::Written { files } => assert!(files > 0),
        other => panic!("expected Written, got {other:?}"),
    }

    // Should write imported.md
    assert!(dst.join(".kiro/steering/imported.md").is_file());
    // Should write mcp.json
    assert!(dst.join(".kiro/settings/mcp.json").is_file());
    // Should write skill
    assert!(dst.join(".kiro/skills/my-skill/SKILL.md").is_file());

    // Check lossy: comments lost on read side
    let (_ir, read_lossy) = reader.read(&src, Scope::Project).unwrap();

    assert!(has_lossy_id(&read_lossy, ids::OPENCODE_OPENCODEJSONC_COMMENTS_LOST));

    std::fs::remove_dir_all(&dst).unwrap();
}

// 9.6: convert kiro → copilot-cli --scope project
#[test]
fn convert_kiro_to_copilot_project() {
    let src = fixtures_dir().join("kiro-project");
    let dst = temp_dir("kiro-to-copilot");

    let reader = KiroReader;
    let writer = CopilotCliWriter;
    let opts = PipelineOptions { dry_run: false, strict: false, yes: true };

    let outcome = run_pipeline(&reader, &writer, &src, Scope::Project, &dst, Scope::Project, opts).unwrap();

    match outcome {
        PipelineOutcome::Written { files } => assert!(files > 0),
        other => panic!("expected Written, got {other:?}"),
    }

    // fileMatch → applyTo (no lossy for this conversion)
    assert!(dst.join(".github/instructions/api-style.instructions.md").is_file());
    // Always → copilot-instructions.md
    assert!(dst.join(".github/copilot-instructions.md").is_file());
    // Skills
    assert!(dst.join("skills/my-skill/SKILL.md").is_file());

    // MCP triggers no-project-scope
    let (ir, _) = reader.read(&src, Scope::Project).unwrap();
    let plan = writer.plan(&ir, &dst, Scope::Project).unwrap();
    assert!(has_lossy_id(&plan.lossy, ids::COPILOT_CLI_MCP_NO_PROJECT_SCOPE));

    std::fs::remove_dir_all(&dst).unwrap();
}

// 9.7: convert copilot-cli → kiro --scope project
#[test]
fn convert_copilot_to_kiro_project() {
    let src = fixtures_dir().join("copilot-project");
    let dst = temp_dir("copilot-to-kiro");

    let reader = CopilotCliReader;
    let writer = KiroWriter;
    let opts = PipelineOptions { dry_run: false, strict: false, yes: true };

    let outcome = run_pipeline(&reader, &writer, &src, Scope::Project, &dst, Scope::Project, opts).unwrap();

    match outcome {
        PipelineOutcome::Written { files } => assert!(files > 0),
        other => panic!("expected Written, got {other:?}"),
    }

    // applyTo → fileMatch (no lossy)
    let api_style = dst.join(".kiro/steering/api.md");
    assert!(api_style.is_file());
    let content = std::fs::read_to_string(&api_style).unwrap();
    assert!(content.contains("fileMatch"));
    assert!(content.contains("src/api/**"));

    // No fileMatch-related lossy
    let (ir, read_lossy) = reader.read(&src, Scope::Project).unwrap();
    let plan = writer.plan(&ir, &dst, Scope::Project).unwrap();
    assert!(!has_lossy_id(&read_lossy, ids::KIRO_STEERING_FILEMATCH_LOST));
    assert!(!has_lossy_id(&plan.lossy, ids::KIRO_STEERING_FILEMATCH_LOST));

    std::fs::remove_dir_all(&dst).unwrap();
}

// 9.8: --dry-run doesn't produce filesystem changes
#[test]
fn dry_run_no_changes() {
    let src = fixtures_dir().join("kiro-project");
    let dst = temp_dir("dry-run-test");

    let reader = KiroReader;
    let writer = OpenCodeWriter;
    let opts = PipelineOptions { dry_run: true, strict: false, yes: true };

    let outcome = run_pipeline(&reader, &writer, &src, Scope::Project, &dst, Scope::Project, opts).unwrap();

    assert!(matches!(outcome, PipelineOutcome::DryRun { .. }));

    // dst should be empty (only the dir itself)
    let entries: Vec<_> = std::fs::read_dir(&dst).unwrap().collect();
    assert!(entries.is_empty(), "dry-run should not create files");

    std::fs::remove_dir_all(&dst).unwrap();
}

// 9.9: --strict with lossy exits code 2
#[test]
fn strict_aborts_on_lossy() {
    let src = fixtures_dir().join("kiro-project");
    let dst = temp_dir("strict-test");

    let reader = KiroReader;
    let writer = OpenCodeWriter;
    let opts = PipelineOptions { dry_run: false, strict: true, yes: true };

    let outcome = run_pipeline(&reader, &writer, &src, Scope::Project, &dst, Scope::Project, opts).unwrap();

    assert!(matches!(outcome, PipelineOutcome::StrictAborted { .. }));

    std::fs::remove_dir_all(&dst).unwrap();
}

// 9.10: -y doesn't prompt (tested implicitly by all tests using yes: true)
// The lossy report is still printed (tested by the pipeline code path).

// 9.11: export then import round-trip
#[test]
fn export_import_roundtrip_kiro_project() {
    let src = fixtures_dir().join("kiro-project");
    let tmp = temp_dir("roundtrip-kiro");
    let ir_file = tmp.join("export.anna.json");

    // Export
    let reader = KiroReader;
    let (ir, _) = reader.read(&src, Scope::Project).unwrap();
    let json = anna_ir::to_string_pretty(&ir).unwrap();
    std::fs::write(&ir_file, &json).unwrap();

    // Import back
    let parsed = anna_ir::from_str(&json).unwrap();
    assert_eq!(ir, parsed);

    // Write to a new location
    let dst = tmp.join("reimported");
    std::fs::create_dir_all(&dst).unwrap();
    let writer = KiroWriter;
    let plan = writer.plan(&parsed, &dst, Scope::Project).unwrap();
    writer.write(&plan).unwrap();

    // Verify files exist
    assert!(dst.join(".kiro/steering/global.md").is_file());
    assert!(dst.join(".kiro/steering/api-style.md").is_file());
    assert!(dst.join(".kiro/settings/mcp.json").is_file());
    assert!(dst.join(".kiro/skills/my-skill/SKILL.md").is_file());

    std::fs::remove_dir_all(&tmp).unwrap();
}

#[test]
fn export_import_roundtrip_opencode_project() {
    let src = fixtures_dir().join("opencode-project");
    let tmp = temp_dir("roundtrip-opencode");

    let reader = OpenCodeReader;
    let (ir, _) = reader.read(&src, Scope::Project).unwrap();
    let json = anna_ir::to_string_pretty(&ir).unwrap();
    let parsed = anna_ir::from_str(&json).unwrap();
    assert_eq!(ir, parsed);

    let dst = tmp.join("reimported");
    std::fs::create_dir_all(&dst).unwrap();
    let writer = OpenCodeWriter;
    let plan = writer.plan(&parsed, &dst, Scope::Project).unwrap();
    writer.write(&plan).unwrap();

    assert!(dst.join("AGENTS.md").is_file());
    assert!(dst.join("opencode.jsonc").is_file());
    assert!(dst.join(".opencode/skills/my-skill/SKILL.md").is_file());

    std::fs::remove_dir_all(&tmp).unwrap();
}

#[test]
fn export_import_roundtrip_copilot_project() {
    let src = fixtures_dir().join("copilot-project");
    let tmp = temp_dir("roundtrip-copilot");

    let reader = CopilotCliReader;
    let (ir, _) = reader.read(&src, Scope::Project).unwrap();
    let json = anna_ir::to_string_pretty(&ir).unwrap();
    let parsed = anna_ir::from_str(&json).unwrap();
    assert_eq!(ir, parsed);

    let dst = tmp.join("reimported");
    std::fs::create_dir_all(&dst).unwrap();
    let writer = CopilotCliWriter;
    let plan = writer.plan(&parsed, &dst, Scope::Project).unwrap();
    writer.write(&plan).unwrap();

    assert!(dst.join(".github/copilot-instructions.md").is_file());
    assert!(dst.join(".github/instructions/api.instructions.md").is_file());
    assert!(dst.join("skills/my-skill/SKILL.md").is_file());

    std::fs::remove_dir_all(&tmp).unwrap();
}
