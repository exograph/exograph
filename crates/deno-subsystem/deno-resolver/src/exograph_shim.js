// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

({
    executeQuery: async function (query_string, variables) {
        const result = await ExographExtension.executeQuery(query_string, variables);
        return result;
    },

    addResponseHeader: function (header, value) {
        return ExographExtension.addResponseHeader(header, value)
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

        return ExographExtension.addResponseHeader("Set-Cookie", cookieString)
    }
})
