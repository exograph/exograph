// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

"%%PRELUDE%%"

export async function evaluate(testvariables) {
    var $ = testvariables;

    // substituted in from Rust
    const json = "%%JSON%%";

    // don't inadvertently pass back an invalid JSON object
    return JSON.parse(JSON.stringify(json));
}

export async function test(actualPayload, testvariables) {
    var $ = testvariables;

    // substituted in from Rust
    const expectedPayload = "%%JSON%%";

    var lastKey = undefined;

    async function _test(expected, actual) {
        switch (typeof(expected)) {
            case "object": {
                // recursively verify that all key/values in expectedResponse are present in actualValue
                for (const key in expected) {
                    lastKey = key;
                    const expectedValue = expected[key];
                    const actualValue = actual[key];

                    await _test(expectedValue, actualValue, testvariables);
                }

                // recursively verify that no extraneous key/values are present in actualValue
                for (const key in actual) {
                    if (expected[key] === undefined) {
                        throw new ExographError("unexpected key " + key.toString() + " in actual response")
                    }
                }

                break;
            }
            case "function": {
                let result = expected(actual);

                if (result === undefined) {
                    throw new ExographError("assertion function for field " + lastKey + " did not return a value, cannot check")
                }

                // if this function is a Promise, resolve the promise before asserting
                if (Object.getPrototypeOf(result) === Promise.prototype) {
                    result = await result;
                }

                // assert true
                if (result === false) {
                    throw new ExographError("assert function failed for field " + lastKey + "!\nactual: " + JSON.stringify(actual))
                }
                break;
            }
            default: {
                if (expected !== actual) {
                    throw new ExographError("assert failed: expected " + expected + " on key " + lastKey + ", got " + actual)
                }
                break;
            }
        }
    }

    await _test(expectedPayload, actualPayload);
}