({
    executeQuery: async function (query_string, variables) {
        const result = await Deno.core.opAsync("op_claytip_execute_query", query_string, variables);
        return result;
    }
})