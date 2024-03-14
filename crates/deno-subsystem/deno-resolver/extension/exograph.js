// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.
const {
    op_exograph_execute_query,
    op_exograph_execute_query_priv,
    op_exograph_add_header,
    op_exograph_version,
    op_operation_name,
    op_operation_query,
    op_operation_proceed,
} = Deno.core.ensureFastOps();

export function exograph_version() {
    return op_exograph_version();
}

// TODO: There's a lot of duplication between the shim code and the extension.
// Ideally we'd get rid of the shim code and just expose the code directly from the extension.
//
globalThis.ExographExtension = ({
    executeQuery: async function (query_string, variables) {
        const result = await op_exograph_execute_query(query_string, variables);
        return result;
    },

    addResponseHeader: function (header, value) {
        return op_exograph_add_header(header, value)
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

        return op_exograph_add_header("Set-Cookie", cookieString)
    },

    executeQueryPriv: async function (query_string, variables, context_override) {
        const result = await op_exograph_execute_query_priv(query_string, variables, context_override);
        return result;
    },
})

globalThis.ExographOperation = ({
    name: function () {
        return op_operation_name()
    },
    proceed: async function () {
        return await op_operation_proceed()
    },
    query: function () {
        return op_operation_query()
    }
})
