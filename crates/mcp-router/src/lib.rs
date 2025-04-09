#![cfg(not(target_family = "wasm"))]

pub mod mcp_router;

pub use mcp_router::McpRouter;
