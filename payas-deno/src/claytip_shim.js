({
    executeQuery: async function (query_string, variables) {
        var args = [query_string];

        if (variables) {
            let stringified_variables = JSON.stringify(variables);
            args[1] = stringified_variables;
        }

        let result = await Deno.core.opAsync("op_claytip_execute_query", args);
        return result;
    }
})