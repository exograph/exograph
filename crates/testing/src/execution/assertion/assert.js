// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

"%%PRELUDE%%"

class AssertionError extends Error {
    constructor(message, path, unorderedField = false) {
        super(message);
        this.path = path;
        this.unorderedField = unorderedField;
    }
}

// Called from Rust to substitute variables in `variable`, `headers`, and `auth` from gql/exotest files.
//
// The Rust code replaces `%%JSON%%` with the stringified JSON object to be substituted in.
// See `evaluate_using_deno` in `mod.rs` for more details.
export async function evaluate(testvariables) {
    var $ = testvariables;

    // substituted in from Rust
    const json = "%%JSON%%";

    // don't inadvertently pass back an invalid JSON object
    return JSON.parse(JSON.stringify(json));
}

// Variable-substitution-sensitive assertion that the actual response matches the expected response.
//
// The Rust code replaces `%%JSON%%` with the stringified JSON object to be substituted in.
// See `dynamic_assert_using_deno` in `mod.rs` for more details.
export async function dynamic_assert(actualPayload, testvariables, unorderedSelections, rpcMetadata) {
    var $ = testvariables;

    // substituted in from Rust (literally replaced in the source code)
    let expectedPayload = "%%JSON%%";

    // For RPC responses, auto-inject jsonrpc and id if they're missing from the expected payload
    // This happens AFTER variable substitution, so the expectedPayload is now a proper object
    if (rpcMetadata && typeof expectedPayload === "object" && expectedPayload !== null && !Array.isArray(expectedPayload)) {
        if (!expectedPayload.hasOwnProperty("jsonrpc")) {
            expectedPayload.jsonrpc = rpcMetadata.jsonrpc;
        }
        if (!expectedPayload.hasOwnProperty("id")) {
            expectedPayload.id = rpcMetadata.id;
        }
    }

    try {
        await assertEquals(expectedPayload, actualPayload, [], unorderedSelections);
    } catch (e) {
        if (e instanceof AssertionError) {
            throw new ExographError(`assertion failed at '${e.path.join(".")}': ${e.message}`);
        } else {
            throw e;
        }
    }
}

export async function assert(expected, actual, unorderedSelections) {
    try {
        await assertEquals(expected, actual, [], unorderedSelections);
    } catch (e) {
        if (e instanceof AssertionError) {
            throw new ExographError(`assertion failed at '${e.path.join(".")}': ${e.message}`);
        } else {
            throw e;
        }
    }
}

async function assertEquals(expected, actual, path, unorderedSelections) {
    switch (typeof (expected)) {
        case "object": {
            if (Array.isArray(expected)) {
                await assertArrayEqual(expected, actual, path, unorderedSelections);
            } else {
                // recursively verify that all key/values in expectedResponse are present in actualValue
                for (const key in expected) {
                    const expectedValue = expected[key];
                    const actualValue = actual[key];

                    const newPath = [...path, key];

                    await assertEquals(expectedValue, actualValue, newPath, unorderedSelections);
                }

                // verify that no extraneous key/values are present in actualValue
                for (const key in actual) {
                    if (expected[key] === undefined) {
                        throw new AssertionError(`unexpected key ${key} in actual response`, path)
                    }
                }
            }

            break;
        }
        case "function": {
            let result = expected(actual);

            if (result === undefined) {
                throw new AssertionError("assertion function did not return a value, cannot check", path)
            }

            // if this function is a Promise, resolve the promise before asserting
            if (Object.getPrototypeOf(result) === Promise.prototype) {
                result = await result;
            }

            if (result === false) {
                throw new AssertionError(`assert function failed actual: ${JSON.stringify(actual)}`, path)
            }
            break;
        }
        default: {
            if (expected !== actual) {
                throw new AssertionError(`expected ${expected}, got ${actual}`, path)
            }
            break;
        }
    }
}

async function assertArrayEqual(expected, actual, path, unorderedSelections) {
    if (expected.length !== actual.length) {
        throw new AssertionError(`expected array length ${expected.length}, got ${actual.length}`, path);
    }
    const unordered = arrayContainsPath(unorderedSelections, path);

    if (unordered) {
        await assertEqualUnordeded(expected, actual, path, unorderedSelections);
    } else {
        // We still assert one by one, since at a lower level we may have unordered arrays
        for (let i = 0; i < expected.length; i++) {
            const expectedItem = expected[i];
            const actualItem = actual[i];
            await assertEquals(expectedItem, actualItem, path, unorderedSelections);
        }
    }
}

async function assertEqualUnordeded(expected, actual, path, unorderedSelections) {
    if (expected.length !== actual.length) {
        throw new AssertionError(`expected array length ${expected.length}, got ${actual.length}`, path);
    }

    if (expected.length !== 0) {
        const expectedItem = expected.pop();

        for (let i = 0; i < actual.length; i++) {
            const actualItem = actual[i];

            try {
                await assertEquals(expectedItem, actualItem, path, unorderedSelections);
                // An element matched, remove it from the actual array
                actual.splice(i, 1);
                break;
            } catch (e) {
                if (e instanceof AssertionError) {
                    if (e.path.length !== path.length && e.unorderedField) {
                        // There was an error at a deeper level for a nested unordered array, so report this root cause
                        throw e;
                    }
                    // ignore (we might find a match at a later index)
                } else {
                    throw e;
                }
            }
        }

        if (expected.length === actual.length) { // if we had a match, we would have removed the matching item from the array, thus making the lengths same
            // Test the remaining items in the array
            await assertEqualUnordeded(expected, actual, path, unorderedSelections);
        } else {
            throw new AssertionError(`could not find ${JSON.stringify(expectedItem)} in the actual array`, path, true);
        }
    }
}

function arrayContainsPath(paths, path) {
    for (const item of paths) {
        if (item.length !== path.length) {
            continue;
        }

        let match = true;
        for (let i = 0; i < item.length; i++) {
            if (item[i] !== path[i]) {
                match = false;
                break;
            }
        }

        if (match) {
            return true;
        }
    }

    return false;
}
