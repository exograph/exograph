({
    executeQuery: async function (query_string, variables) {
        const result = await Deno[Deno.internal].core.opAsync("op_claytip_execute_query", query_string, variables);
        return result;
    },

    addResponseHeader: function (header, value) {
        return Deno[Deno.internal].core.ops.op_claytip_add_header(header, value)
    },

    setCookie: function (
        cookie
    ) {
        // https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Set-Cookie
        let cookieString = `${encodeURIComponent(cookie.name)}=${encodeURIComponent(cookie.value)}`;

        if (cookie.expires) {
            cookieString += `; Expires=${cookie.expires.toUTCString()}`
        }

        if (cookie.maxAge) {
            cookieString += `; Max-Age=${cookie.maxAge}`
        }

        if (cookie.domain) {
            cookieString += `; Domain=${cookie.domain}`
        }

        if (cookie.path) {
            cookieString += `; Path=${cookie.path}`
        }

        if (cookie.secure) {
            cookieString += `; Secure`
        }

        if (cookie.httpOnly) {
            cookieString += `; HttpOnly`
        }

        if (cookie.sameSite) {
            cookieString += `; SameSite=${cookie.sameSite}`
        }

        return Deno[Deno.internal].core.ops.op_claytip_add_header("Set-Cookie", cookieString)
    }
})