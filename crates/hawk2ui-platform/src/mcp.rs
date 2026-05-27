//! Capability-scoped MCP tool API records.

use crate::{CapabilityTable, PlatformContext, PlatformDiagnostic, PlatformOperation};

/// MCP manifest declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct McpManifest {
    /// Required capability key.
    pub capability_key: String,
    /// Allowed MCP server identifiers.
    pub allowed_servers: Vec<String>,
    /// Allowed tool names.
    pub allowed_tools: Vec<String>,
}

impl McpManifest {
    /// Creates an MCP manifest declaration.
    #[must_use]
    pub fn new(
        capability_key: impl Into<String>,
        allowed_servers: impl IntoIterator<Item = impl Into<String>>,
        allowed_tools: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            capability_key: capability_key.into(),
            allowed_servers: allowed_servers.into_iter().map(Into::into).collect(),
            allowed_tools: allowed_tools.into_iter().map(Into::into).collect(),
        }
    }
}

/// Allowed MCP tool call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct McpToolCall {
    /// MCP server identifier.
    pub server_id: String,
    /// Tool name.
    pub tool_name: String,
}

/// MCP denial.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct McpDenied {
    /// MCP server identifier.
    pub server_id: String,
    /// Structured diagnostic.
    pub diagnostic: PlatformDiagnostic,
}

/// Capability-scoped MCP policy.
pub struct McpPolicy;

impl McpPolicy {
    /// Validates an MCP tool call against capabilities and manifest allowlists.
    ///
    /// # Errors
    ///
    /// Returns [`McpDenied`] when the capability, server, or tool is denied.
    pub fn call(
        capabilities: &CapabilityTable,
        manifest: &McpManifest,
        server_id: &str,
        tool_name: &str,
        context: PlatformContext,
    ) -> Result<McpToolCall, McpDenied> {
        capabilities
            .ensure_allowed(
                &manifest.capability_key,
                PlatformOperation::McpToolCall,
                context,
            )
            .map_err(|denial| McpDenied {
                server_id: server_id.into(),
                diagnostic: denial.diagnostic,
            })?;
        if !is_declared(&manifest.allowed_servers, server_id) {
            return Err(McpDenied {
                server_id: server_id.into(),
                diagnostic: PlatformDiagnostic::error(
                    "mcp.server.denied",
                    format!("MCP server is not declared: {server_id}"),
                ),
            });
        }
        if !is_declared(&manifest.allowed_tools, tool_name) {
            return Err(McpDenied {
                server_id: server_id.into(),
                diagnostic: PlatformDiagnostic::error(
                    "mcp.tool.denied",
                    format!("MCP tool is not declared: {tool_name}"),
                ),
            });
        }
        Ok(McpToolCall {
            server_id: server_id.into(),
            tool_name: tool_name.into(),
        })
    }
}

fn is_declared(allowed: &[String], value: &str) -> bool {
    is_stable_id(value) && allowed.iter().any(|entry| entry == value)
}

fn is_stable_id(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
}
