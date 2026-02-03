use rmcp::handler::server::tool::ToolRouter;
use rmcp::model::{CallToolResult, Content, ErrorData, Implementation, ProtocolVersion, ServerCapabilities, ServerInfo};
use rmcp::{ServerHandler, tool, tool_handler, tool_router};

#[derive(Clone)]
#[allow(dead_code)]
pub struct OattyMcpCore {
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl OattyMcpCore {
    fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "Get the current counter value")]
    async fn search(&self) -> Result<CallToolResult, ErrorData> {
        Ok(CallToolResult::success(vec![Content::text("1".to_string())]))
    }
}

#[tool_handler]
impl ServerHandler for OattyMcpCore {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            protocol_version: ProtocolVersion::LATEST,
            server_info: Implementation {
                name: "Oatty".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                title: Some("Oatty MCP".to_string()),
                ..Default::default()
            },
            instructions: Some("".to_string()),
        }
    }
}
