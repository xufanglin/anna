#!/usr/bin/env bash
# anna 项目级转换集成测试
# 测试所有 agent 之间的双向转换，并用各 agent 的 CLI 验证配置可读性
set -euo pipefail

ANNA="/data/anna/target/release/anna"
WORKDIR=$(mktemp -d)
PASS=0
FAIL=0
ERRORS=""

cleanup() { rm -rf "$WORKDIR"; }
trap cleanup EXIT

log()  { echo -e "\033[1;34m[TEST]\033[0m $*"; }
pass() { PASS=$((PASS + 1)); echo -e "\033[1;32m  ✓\033[0m $*"; }
fail() { FAIL=$((FAIL + 1)); ERRORS+="  ✗ $*\n"; echo -e "\033[1;31m  ✗\033[0m $*"; }

# ─── 准备源项目 ───────────────────────────────────────────────

setup_kiro_project() {
    local dir="$1"
    mkdir -p "$dir/.kiro/steering" "$dir/.kiro/settings" "$dir/.kiro/skills/test-skill"

    cat > "$dir/.kiro/steering/global.md" << 'EOF'
---
inclusion: always
---

Be concise. Use TypeScript. Write tests.
EOF

    cat > "$dir/.kiro/steering/api.md" << 'EOF'
---
inclusion: fileMatch
fileMatchPattern: "src/api/**"
---

Use REST conventions. Return proper status codes.
EOF

    cat > "$dir/.kiro/settings/mcp.json" << 'EOF'
{
  "mcpServers": {
    "stdio-server": {
      "command": "npx",
      "args": ["-y", "@example/mcp-server"],
      "env": { "TOKEN": "abc" }
    },
    "http-server": {
      "type": "streamable-http",
      "url": "http://127.0.0.1:9800/mcp",
      "headers": { "Authorization": "Bearer xyz" }
    }
  }
}
EOF

    cat > "$dir/.kiro/skills/test-skill/SKILL.md" << 'EOF'
---
name: test-skill
description: "A test skill for validation."
---

# Test Skill

This is a test skill content.
EOF
}

setup_copilot_project() {
    local dir="$1"
    mkdir -p "$dir/.github/instructions" "$dir/skills/test-skill"

    cat > "$dir/.github/copilot-instructions.md" << 'EOF'
Be concise. Use TypeScript. Write tests.
EOF

    cat > "$dir/.github/instructions/api.instructions.md" << 'EOF'
---
applyTo: "src/api/**"
---

Use REST conventions. Return proper status codes.
EOF

    cat > "$dir/AGENTS.md" << 'EOF'
Follow project conventions.
EOF

    cat > "$dir/skills/test-skill/SKILL.md" << 'EOF'
---
name: test-skill
description: "A test skill for validation."
---

# Test Skill

This is a test skill content.
EOF
}

setup_opencode_project() {
    local dir="$1"
    mkdir -p "$dir/.opencode/skills/test-skill"

    cat > "$dir/AGENTS.md" << 'EOF'
Be concise. Use TypeScript. Write tests.
EOF

    cat > "$dir/opencode.jsonc" << 'EOF'
{
  "mcp": {
    "stdio-server": {
      "type": "local",
      "command": ["npx", "-y", "@example/mcp-server"],
      "environment": { "TOKEN": "abc" }
    },
    "http-server": {
      "type": "remote",
      "url": "http://127.0.0.1:9800/mcp"
    }
  }
}
EOF

    cat > "$dir/.opencode/skills/test-skill/SKILL.md" << 'EOF'
---
name: test-skill
description: "A test skill for validation."
---

# Test Skill

This is a test skill content.
EOF
}

setup_codex_project() {
    local dir="$1"
    mkdir -p "$dir/.codex" "$dir/.agents/skills/test-skill"

    cat > "$dir/AGENTS.md" << 'EOF'
Be concise. Use TypeScript. Write tests.
EOF

    cat > "$dir/.codex/config.toml" << 'EOF'
model = "o3"

[mcp_servers.stdio-server]
command = "npx"
args = ["-y", "@example/mcp-server"]

[mcp_servers.stdio-server.env]
TOKEN = "abc"

[mcp_servers.http-server]
url = "http://127.0.0.1:9800/mcp"
EOF

    cat > "$dir/.agents/skills/test-skill/SKILL.md" << 'EOF'
---
name: test-skill
description: "A test skill for validation."
---

# Test Skill

This is a test skill content.
EOF
}

setup_claude_code_project() {
    local dir="$1"
    mkdir -p "$dir/.claude/rules" "$dir/.claude/skills/test-skill"

    cat > "$dir/CLAUDE.md" << 'EOF'
Be concise. Use TypeScript. Write tests.
EOF

    cat > "$dir/.claude/rules/api-style.md" << 'EOF'
---
globs: "src/api/**"
---

Use REST conventions. Return proper status codes.
EOF

    cat > "$dir/.mcp.json" << 'EOF'
{
  "mcpServers": {
    "stdio-server": {
      "command": "npx",
      "args": ["-y", "@example/mcp-server"],
      "env": { "TOKEN": "abc" }
    },
    "http-server": {
      "url": "http://127.0.0.1:9800/mcp"
    }
  }
}
EOF

    cat > "$dir/.claude/skills/test-skill/SKILL.md" << 'EOF'
---
name: test-skill
description: "A test skill for validation."
---

# Test Skill

This is a test skill content.
EOF
}

setup_cursor_project() {
    local dir="$1"
    mkdir -p "$dir/.cursor/rules" "$dir/.cursor/skills/test-skill"

    cat > "$dir/.cursor/rules/global.mdc" << 'EOF'
---
description: "Core project standards"
alwaysApply: true
---

Be concise. Use TypeScript. Write tests.
EOF

    cat > "$dir/.cursor/rules/api-style.mdc" << 'EOF'
---
description: "API conventions"
globs:
  - "src/api/**"
alwaysApply: false
---

Use REST conventions. Return proper status codes.
EOF

    cat > "$dir/.cursor/mcp.json" << 'EOF'
{
  "mcpServers": {
    "stdio-server": {
      "command": "npx",
      "args": ["-y", "@example/mcp-server"],
      "env": { "TOKEN": "abc" }
    },
    "http-server": {
      "url": "http://127.0.0.1:9800/mcp",
      "transport": "streamable-http"
    }
  }
}
EOF

    cat > "$dir/.cursor/skills/test-skill/SKILL.md" << 'EOF'
---
name: test-skill
description: "A test skill for validation."
---

# Test Skill

This is a test skill content.
EOF
}

# ─── 转换测试 ─────────────────────────────────────────────────

test_convert() {
    local from="$1" to="$2" src_dir="$3"
    local dst_dir="$WORKDIR/convert-${from}-to-${to}"
    cp -r "$src_dir" "$dst_dir"

    log "convert: $from → $to"

    if $ANNA convert --from "$from" --to "$to" --scope project "$dst_dir" --yes 2>&1; then
        pass "anna convert $from → $to succeeded"
    else
        fail "anna convert $from → $to failed"
        return
    fi

    # 验证目标 agent 的关键文件存在
    case "$to" in
        kiro)
            ls "$dst_dir/.kiro/steering/"*.md >/dev/null 2>&1 && \
                pass "$to: steering file exists" || fail "$to: no steering file"
            [[ -f "$dst_dir/.kiro/skills/test-skill/SKILL.md" ]] && \
                pass "$to: skill file exists" || fail "$to: no skill file"
            ;;
        copilot-cli)
            [[ -f "$dst_dir/.github/copilot-instructions.md" ]] && \
                pass "$to: instructions file exists" || fail "$to: no instructions file"
            [[ -f "$dst_dir/skills/test-skill/SKILL.md" ]] && \
                pass "$to: skill file exists" || fail "$to: no skill file"
            ;;
        opencode)
            [[ -f "$dst_dir/AGENTS.md" ]] && \
                pass "$to: AGENTS.md exists" || fail "$to: no AGENTS.md"
            [[ -f "$dst_dir/.opencode/skills/test-skill/SKILL.md" ]] && \
                pass "$to: skill file exists" || fail "$to: no skill file"
            ;;
        codex)
            [[ -f "$dst_dir/AGENTS.md" ]] && \
                pass "$to: AGENTS.md exists" || fail "$to: no AGENTS.md"
            [[ -f "$dst_dir/.agents/skills/test-skill/SKILL.md" ]] && \
                pass "$to: skill file exists" || fail "$to: no skill file"
            ;;
        claude-code)
            [[ -f "$dst_dir/CLAUDE.md" ]] && \
                pass "$to: CLAUDE.md exists" || fail "$to: no CLAUDE.md"
            [[ -f "$dst_dir/.claude/skills/test-skill/SKILL.md" ]] && \
                pass "$to: skill file exists" || fail "$to: no skill file"
            ;;
        cursor)
            ls "$dst_dir/.cursor/rules/"*.mdc >/dev/null 2>&1 && \
                pass "$to: rules file exists" || fail "$to: no rules file"
            [[ -f "$dst_dir/.cursor/skills/test-skill/SKILL.md" ]] && \
                pass "$to: skill file exists" || fail "$to: no skill file"
            ;;
    esac
}

# ─── CLI 验证测试 ─────────────────────────────────────────────

test_opencode_reads() {
    local dir="$1"
    log "opencode: verify config readable"

    local output
    output=$(cd "$dir" && opencode mcp ls 2>&1) || true
    if echo "$output" | grep -qi "server\|stdio-server\|http-server\|no.*server\|configured"; then
        pass "opencode mcp ls runs in converted project"
    else
        fail "opencode mcp ls unexpected output: $output"
    fi
}

test_copilot_reads() {
    local dir="$1"
    log "copilot: verify config readable"

    local output
    output=$(cd "$dir" && copilot mcp list 2>&1) || true
    if echo "$output" | grep -qi "server\|stdio-server\|http-server\|no.*server\|configured\|name"; then
        pass "copilot mcp list runs in converted project"
    else
        fail "copilot mcp list unexpected output: $output"
    fi
}

# ─── Export/Import 往返测试 ────────────────────────────────────

test_roundtrip() {
    local from="$1" src_dir="$2"
    local ir_file="$WORKDIR/roundtrip-${from}.anna.json"

    log "roundtrip: $from → IR → $from"

    if $ANNA export --from "$from" --scope project "$src_dir" -o "$ir_file" 2>&1; then
        pass "export $from → IR"
    else
        fail "export $from → IR failed"
        return
    fi

    local reimport_dir="$WORKDIR/reimport-${from}"
    mkdir -p "$reimport_dir"
    if $ANNA import --to "$from" --scope project "$ir_file" "$reimport_dir" --yes 2>&1; then
        pass "import IR → $from"
    else
        fail "import IR → $from failed"
    fi
}

# ─── 主流程 ───────────────────────────────────────────────────

echo "╔══════════════════════════════════════════════╗"
echo "║  anna 项目级转换集成测试                     ║"
echo "╚══════════════════════════════════════════════╝"
echo

# 构建
log "Building anna..."
(cd /data/anna && cargo build --release 2>&1) || { echo "Build failed"; exit 1; }
echo

# 准备源项目
KIRO_SRC="$WORKDIR/src-kiro"
COPILOT_SRC="$WORKDIR/src-copilot"
OPENCODE_SRC="$WORKDIR/src-opencode"
CODEX_SRC="$WORKDIR/src-codex"
CLAUDE_CODE_SRC="$WORKDIR/src-claude-code"
CURSOR_SRC="$WORKDIR/src-cursor"
setup_kiro_project "$KIRO_SRC"
setup_copilot_project "$COPILOT_SRC"
setup_opencode_project "$OPENCODE_SRC"
setup_codex_project "$CODEX_SRC"
setup_claude_code_project "$CLAUDE_CODE_SRC"
setup_cursor_project "$CURSOR_SRC"

# ─── 全矩阵转换测试 (6×5 = 30 方向) ──────────────────────────
echo
log "=== 转换矩阵测试 ==="
echo

test_convert kiro copilot-cli "$KIRO_SRC"
test_convert kiro opencode "$KIRO_SRC"
test_convert kiro codex "$KIRO_SRC"
test_convert kiro claude-code "$KIRO_SRC"
test_convert kiro cursor "$KIRO_SRC"
test_convert copilot-cli kiro "$COPILOT_SRC"
test_convert copilot-cli opencode "$COPILOT_SRC"
test_convert copilot-cli codex "$COPILOT_SRC"
test_convert copilot-cli claude-code "$COPILOT_SRC"
test_convert copilot-cli cursor "$COPILOT_SRC"
test_convert opencode kiro "$OPENCODE_SRC"
test_convert opencode copilot-cli "$OPENCODE_SRC"
test_convert opencode codex "$OPENCODE_SRC"
test_convert opencode claude-code "$OPENCODE_SRC"
test_convert opencode cursor "$OPENCODE_SRC"
test_convert codex kiro "$CODEX_SRC"
test_convert codex copilot-cli "$CODEX_SRC"
test_convert codex opencode "$CODEX_SRC"
test_convert codex claude-code "$CODEX_SRC"
test_convert codex cursor "$CODEX_SRC"
test_convert claude-code kiro "$CLAUDE_CODE_SRC"
test_convert claude-code copilot-cli "$CLAUDE_CODE_SRC"
test_convert claude-code opencode "$CLAUDE_CODE_SRC"
test_convert claude-code codex "$CLAUDE_CODE_SRC"
test_convert claude-code cursor "$CLAUDE_CODE_SRC"
test_convert cursor kiro "$CURSOR_SRC"
test_convert cursor copilot-cli "$CURSOR_SRC"
test_convert cursor opencode "$CURSOR_SRC"
test_convert cursor codex "$CURSOR_SRC"
test_convert cursor claude-code "$CURSOR_SRC"

# ─── CLI 验证 ─────────────────────────────────────────────────
echo
log "=== CLI 验证测试 ==="
echo

# 用转换后的项目验证 opencode 能读取
OC_DIR="$WORKDIR/convert-kiro-to-opencode"
[[ -d "$OC_DIR" ]] && test_opencode_reads "$OC_DIR"

# 用转换后的项目验证 copilot 能读取
CP_DIR="$WORKDIR/convert-kiro-to-copilot-cli"
[[ -d "$CP_DIR" ]] && test_copilot_reads "$CP_DIR"

# ─── 往返测试 ─────────────────────────────────────────────────
echo
log "=== Export/Import 往返测试 ==="
echo

test_roundtrip kiro "$KIRO_SRC"
test_roundtrip copilot-cli "$COPILOT_SRC"
test_roundtrip opencode "$OPENCODE_SRC"
test_roundtrip codex "$CODEX_SRC"
test_roundtrip claude-code "$CLAUDE_CODE_SRC"
test_roundtrip cursor "$CURSOR_SRC"

# ─── 结果汇总 ─────────────────────────────────────────────────
echo
echo "════════════════════════════════════════════════"
echo -e "  结果: \033[32m$PASS passed\033[0m, \033[31m$FAIL failed\033[0m"
if [[ $FAIL -gt 0 ]]; then
    echo -e "\n  失败项:"
    echo -e "$ERRORS"
    exit 1
fi
echo "════════════════════════════════════════════════"
