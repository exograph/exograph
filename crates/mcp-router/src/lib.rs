#![cfg(not(target_family = "wasm"))]

mod execute_query_tool;
mod executor;
mod introspection_tool;
mod mcp_router;
mod tool;
mod tools_creator;

pub use mcp_router::McpRouter;
