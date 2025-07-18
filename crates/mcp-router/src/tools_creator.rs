use std::sync::Arc;

use common::env_const::EXO_WWW_AUTHENTICATE_HEADER;
use core_plugin_shared::profile::{SchemaProfile, SchemaProfiles};
use core_resolver::system_resolver::GraphQLSystemResolver;
use core_router::SystemLoadingError;
use exo_env::Environment;

use crate::{
    execute_query_tool::ExecuteQueryTool, introspection_tool::IntrospectionTool, tool::Tool,
};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum McpToolMode {
    CombineIntrospection,
    SeparateIntrospection,
}

pub fn create_tools(
    env: &dyn Environment,
    schema_profiles: Option<SchemaProfiles>,
    create_resolver: &impl Fn(&SchemaProfile) -> Result<Arc<GraphQLSystemResolver>, SystemLoadingError>,
) -> Result<Vec<Box<dyn Tool>>, SystemLoadingError> {
    let www_authenticate_header = env.get(EXO_WWW_AUTHENTICATE_HEADER);

    let tool_mode = if env.get_or_else("EXO_MCP_MODE", "combined") == "separate" {
        McpToolMode::SeparateIntrospection
    } else {
        McpToolMode::CombineIntrospection
    };

    let profiles = schema_profiles.unwrap_or_default();

    let tools: Vec<Box<dyn Tool>> = if profiles.is_empty() {
        let resolver = create_resolver(&SchemaProfile::queries_only())?;
        create_tool_for_profile(None, tool_mode, www_authenticate_header, resolver)?
    } else {
        let mut tools: Vec<Box<dyn Tool>> = Vec::new();
        for (name, profile) in profiles {
            let resolver = create_resolver(&profile)?;

            tools.extend(create_tool_for_profile(
                Some(name.clone()),
                tool_mode,
                www_authenticate_header.clone(),
                resolver.clone(),
            )?);
        }
        tools
    };

    Ok(tools)
}

fn create_tool_for_profile(
    profile: Option<String>,
    tool_mode: McpToolMode,
    www_authenticate_header: Option<String>,
    resolver: Arc<GraphQLSystemResolver>,
) -> Result<Vec<Box<dyn Tool>>, SystemLoadingError> {
    let (execute_query_name, introspection_name) = match profile {
        Some(profile) => (
            format!("execute_query-{}", profile),
            format!("introspection-{}", profile),
        ),
        None => ("execute_query".to_string(), "introspection".to_string()),
    };

    let mut tools: Vec<Box<dyn Tool>> = vec![Box::new(ExecuteQueryTool::new(
        execute_query_name,
        resolver.clone(),
        www_authenticate_header,
        tool_mode,
    ))];

    if tool_mode == McpToolMode::SeparateIntrospection {
        tools.push(Box::new(IntrospectionTool::new(
            introspection_name,
            resolver.clone(),
        )));
    }

    Ok(tools)
}
