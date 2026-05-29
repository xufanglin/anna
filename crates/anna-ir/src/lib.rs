//! anna 的中间表示（IR）。
//!
//! 设计参见 `openspec/changes/add-mvp-converters/design.md` 的 D2 / D3。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// 当前 IR JSON 文档的版本号。读取到其他版本视为硬错误。
pub const ANNA_IR_VERSION: u32 = 1;

/// 顶层 IR 容器。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ir {
    /// 序列化版本号；反序列化时要求等于 [`ANNA_IR_VERSION`]。
    #[serde(deserialize_with = "deserialize_ir_version")]
    pub anna_ir_version: u32,
    #[serde(default)]
    pub instructions: Vec<Instruction>,
    #[serde(default)]
    pub mcp_servers: Vec<McpServer>,
    #[serde(default)]
    pub skills: Vec<Skill>,
}

impl Ir {
    /// 创建一个空 IR，版本号自动置为 [`ANNA_IR_VERSION`]。
    pub fn empty() -> Self {
        Self {
            anna_ir_version: ANNA_IR_VERSION,
            instructions: Vec::new(),
            mcp_servers: Vec::new(),
            skills: Vec::new(),
        }
    }
}

impl Default for Ir {
    fn default() -> Self {
        Self::empty()
    }
}

/// 一条 instruction（即 steering / AGENTS.md 的一段内容）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Instruction {
    pub name: String,
    pub content: String,
    pub inclusion: Inclusion,
    /// Optional description (used by Cursor's agent-requested rules).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// 该 instruction 进入上下文的方式。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Inclusion {
    Always,
    FileMatch { pattern: String },
    Manual,
}

/// 一台 MCP 服务器的配置。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpServer {
    pub name: String,
    pub transport: McpTransport,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    /// Kiro 独有：是否禁用该 server。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled: Option<bool>,
    /// Kiro 独有：自动批准的工具名列表。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_approve: Option<Vec<String>>,
}

/// MCP server 的传输方式（MCP 协议定义的三种 transport）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum McpTransport {
    Stdio {
        command: String,
        #[serde(default)]
        args: Vec<String>,
    },
    StreamableHttp {
        url: String,
        #[serde(default)]
        headers: BTreeMap<String, String>,
    },
    Sse {
        url: String,
        #[serde(default)]
        headers: BTreeMap<String, String>,
    },
}

/// 一个 skill（即 `<dir>/<name>/SKILL.md`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub content: String,
}

/// IR JSON 反序列化错误。
#[derive(Debug, thiserror::Error)]
pub enum IrError {
    #[error(
        "anna_ir_version mismatch: expected {expected}, got {actual} (this build of anna does not support that version)"
    )]
    VersionMismatch { expected: u32, actual: u32 },
    #[error("invalid IR JSON: {0}")]
    Json(#[from] serde_json::Error),
}

/// 从字符串读入一份 IR，校验版本。
pub fn from_str(input: &str) -> Result<Ir, IrError> {
    // 先解析为通用 JSON 值，单独检查 anna_ir_version 字段，
    // 这样能在结构反序列化之前给出更清晰的版本号错误。
    let value: serde_json::Value = serde_json::from_str(input)?;
    let actual = value
        .get("anna_ir_version")
        .and_then(|v| v.as_u64())
        .ok_or(IrError::VersionMismatch {
            expected: ANNA_IR_VERSION,
            actual: 0,
        })?;
    if actual as u32 != ANNA_IR_VERSION {
        return Err(IrError::VersionMismatch {
            expected: ANNA_IR_VERSION,
            actual: actual as u32,
        });
    }
    let ir: Ir = serde_json::from_value(value)?;
    Ok(ir)
}

/// 把 IR 序列化为美化过的 JSON。
pub fn to_string_pretty(ir: &Ir) -> Result<String, IrError> {
    Ok(serde_json::to_string_pretty(ir)?)
}

fn deserialize_ir_version<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;
    let v = u32::deserialize(deserializer)?;
    if v != ANNA_IR_VERSION {
        return Err(D::Error::custom(format!(
            "anna_ir_version mismatch: expected {ANNA_IR_VERSION}, got {v}"
        )));
    }
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_ir() -> Ir {
        let mut env = BTreeMap::new();
        env.insert("API_KEY".into(), "sk-abcd1234".into());
        Ir {
            anna_ir_version: ANNA_IR_VERSION,
            instructions: vec![
                Instruction {
                    name: "global".into(),
                    content: "always include this".into(),
                    inclusion: Inclusion::Always,
                    description: None,
                },
                Instruction {
                    name: "api-style".into(),
                    content: "see when working in api/".into(),
                    inclusion: Inclusion::FileMatch {
                        pattern: "src/api/**".into(),
                    },
                    description: None,
                },
            ],
            mcp_servers: vec![McpServer {
                name: "aws-docs".into(),
                transport: McpTransport::Stdio {
                    command: "uvx".into(),
                    args: vec!["awslabs.aws-documentation-mcp-server@latest".into()],
                },
                env,
                disabled: Some(false),
                auto_approve: Some(vec!["read".into()]),
            }],
            skills: vec![Skill {
                name: "writing".into(),
                description: "useful when writing prose".into(),
                content: "# Writing skill\n...".into(),
            }],
        }
    }

    #[test]
    fn roundtrip_preserves_data() {
        let original = sample_ir();
        let json = to_string_pretty(&original).expect("serialize");
        let parsed = from_str(&json).expect("deserialize");
        assert_eq!(original, parsed);
    }

    #[test]
    fn empty_ir_roundtrips() {
        let original = Ir::empty();
        let json = to_string_pretty(&original).expect("serialize");
        let parsed = from_str(&json).expect("deserialize");
        assert_eq!(original, parsed);
    }

    #[test]
    fn roundtrip_with_remote_transport() {
        let mut ir = Ir::empty();
        ir.mcp_servers.push(McpServer {
            name: "remote-srv".into(),
            transport: McpTransport::StreamableHttp {
                url: "https://example.com/mcp".into(),
                headers: BTreeMap::new(),
            },
            env: BTreeMap::new(),
            disabled: None,
            auto_approve: None,
        });
        let json = to_string_pretty(&ir).expect("serialize");
        let parsed = from_str(&json).expect("deserialize");
        assert_eq!(ir, parsed);
    }

    #[test]
    fn roundtrip_manual_inclusion() {
        let mut ir = Ir::empty();
        ir.instructions.push(Instruction {
            name: "rare".into(),
            content: "manual reference only".into(),
            inclusion: Inclusion::Manual,
            description: None,
        });
        let json = to_string_pretty(&ir).expect("serialize");
        let parsed = from_str(&json).expect("deserialize");
        assert_eq!(ir, parsed);
    }

    #[test]
    fn rejects_missing_version() {
        let json = r#"{ "instructions": [], "mcp_servers": [], "skills": [] }"#;
        let err = from_str(json).expect_err("must reject missing version");
        match err {
            IrError::VersionMismatch { expected, actual } => {
                assert_eq!(expected, ANNA_IR_VERSION);
                assert_eq!(actual, 0);
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn rejects_wrong_version() {
        let json = r#"{ "anna_ir_version": 99, "instructions": [], "mcp_servers": [], "skills": [] }"#;
        let err = from_str(json).expect_err("must reject wrong version");
        let msg = err.to_string();
        assert!(msg.contains("expected 1"), "msg = {msg}");
        assert!(msg.contains("99"), "msg = {msg}");
    }

    #[test]
    fn env_values_are_preserved_verbatim() {
        let ir = sample_ir();
        let json = to_string_pretty(&ir).expect("serialize");
        let parsed = from_str(&json).expect("deserialize");
        assert_eq!(
            parsed.mcp_servers[0].env.get("API_KEY"),
            Some(&"sk-abcd1234".to_string())
        );
    }
}
