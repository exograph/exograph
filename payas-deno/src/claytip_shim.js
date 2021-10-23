({
    executeQuery: function (query_string, variables) {
        var args = [query_string];

        if (variables) {
            let stringified_variables = JSON.stringify(variables);
            args[1] = stringified_variables;
        }

        let result = Deno.core.opSync("op_claytip_execute_query", args);
        return result;
    }
})