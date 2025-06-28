#![cfg(not(target_family = "wasm"))]

mod error;
mod execute_query_tool;
mod executor;
mod introspection_tool;
mod mcp_router;
mod protocol_version;
mod tool;
mod tools_creator;

pub use mcp_router::McpRouter;
