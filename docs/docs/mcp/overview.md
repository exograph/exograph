---
sidebar_position: 0
---

# Model Context Protocol

Once you define your Exograph model, there is a pretty much nothing you need to do to start using it with LLMs. Exograph offers the [Model Context Protocol (MCP)](https://modelcontextprotocol.io) protocol, which allows you to connect your Exograph model to any MCP client.

In [MCP tutorial](/mcp-tutorial), we walked through setting up an MCP client (Claude Desktop) which can use the MCP server to answer questions based on the database. Let's peek under the hood and explore a few options that Exograph offers.

## Default tools

By default, the MCP server offers operates in the `combined` mode. In this mode, the MCP server offers a single tool: `execute_query`. Its description gives LLMs sufficient information to form appropriate queries. Specifically, the description includes the GraphQL schema. Exograph also offers the `separate` mode, where it offers two 

To set the mode, set the `EXO_MCP_MODE` environment variable to `separate` or `combined`.

```sh
EXO_MCP_MODE=separate exo dev
```

## Customizing tools

When you use Exograph as a part of an agentic workflow, you may want to customize tools to focus on specific parts of the schema. 

Exograph let's you define profiles that specify types, queries, and mutations.


By default, Exograph's MCP server doesn't expose any mutations (even if access control allows them). However, you can enable mutations selectively by creating a profile.

## Specifying profiles

By default, all queries and no mutations are exposed. You can customize this behavior by creating profiles.

```yaml
[[mcp.profiles]]
name = "membership_management"
queries.models.include = ["Membership*", "User"]
queries.models.exclude = ["Venue", "Concert"]
queries.operations.include = ["memberships", "user"]
queries.operations.exclude = ["*Agg"]
mutations.models.include = ["Membership*"]
mutations.models.exclude = ["Venue", "Concert"]
mutations.operations.include = ["createMembership", "updateMembership"]
mutations.operations.exclude = ["deleteMembership"]

[[mcp.profiles]]
name = "concert_management"
queries.models.include = ["Concert", "Venue"]
queries.models.exclude = ["Membership*"]
queries.operations.include = ["concerts"]
queries.operations.exclude = ["*Agg"]
```

## Disabling MCP API

To disable the MCP endpoint, set the `EXO_ENABLE_MCP` environment variable to `false`.

## Exo MCP Bridge

Not all MCP clients offer connecting to a remote MCP server over HTTP. Even those that do, come with a few limitations. For example, Claude Desktop requires that the MCP server be available on publicly accessible server. Exograph ships with an executable `exo-mcp-bridge` on one side offers the `stdio` protocol, and the other side connects to the MCP server over HTTP. It also offers setting up headers and cookies to pass through to the MCP server, which is useful for authentication.

You would typically use `exo-mcp-bridge` in the configuration of your MCP client as shown in the [MCP tutorial](/mcp-tutorial).

The bridge required one mandatory argument `--endpoint` which is the MCP server URL.

```sh
exo-mcp-bridge --endpoint http://localhost:9876/mcp
```

The bridge also supports the `--header` and `--cookie` flags to pass through headers and cookies to the MCP server.

```sh
exo-mcp-bridge --header "Authorization: Bearer <your-token>" --cookie "session_id=<your-session-id>"
```




