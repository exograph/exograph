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

export async function test(actualPayload, testvariables, unorderedSelections) {
    var $ = testvariables;

    // substituted in from Rust
    const expectedPayload = "%%JSON%%";

    await assert_equals(expectedPayload, actualPayload, [], unorderedSelections);
}

async function assert_equals(expected, actual, path, unorderedSelections) {
    switch (typeof (expected)) {
        case "object": {
            if (Array.isArray(expected)) {
                await assert_array_equal(expected, actual, path, unorderedSelections);
            } else {
                // recursively verify that all key/values in expectedResponse are present in actualValue
                for (const key in expected) {
                    const expectedValue = expected[key];
                    const actualValue = actual[key];

                    const new_path = [...path, key];

                    await assert_equals(expectedValue, actualValue, new_path, unorderedSelections);
                }

                // recursively verify that no extraneous key/values are present in actualValue
                for (const key in actual) {
                    if (expected[key] === undefined) {
                        throw new ExographError("unexpected key " + key.toString() + " in actual response " + " at " + path_to_string(path))
                    }
                }
            }

            break;
        }
        case "function": {
            let result = expected(actual);

            if (result === undefined) {
                throw new ExographError("assertion function for field " + path_to_string(path) + " did not return a value, cannot check")
            }

            // if this function is a Promise, resolve the promise before asserting
            if (Object.getPrototypeOf(result) === Promise.prototype) {
                result = await result;
            }

            // assert true
            if (result === false) {
                throw new ExographError("assert function failed for field " + path_to_string(path) + "!\nactual: " + JSON.stringify(actual))
            }
            break;
        }
        default: {
            if (expected !== actual) {
                throw new ExographError("assert failed: expected " + expected + " on key " + path_to_string(path) + ", got " + actual)
            }
            break;
        }
    }
}

async function assert_array_equal(expected, actual, path, unorderedSelections) {
    if (expected.length !== actual.length) {
        throw new ExographError("assert failed: expected array length " + expected.length + ", got " + actual.length)
    }
    const unordered = array_contains_path(unorderedSelections, path);

    if (unordered) {
        await assert_equal_unordeded(expected, actual, path, unorderedSelections);
    } else {
        // We still assert one by one, since at a lower level we may have unordered arrays
        for (let i = 0; i < expected.length; i++) {
            const expected_item = expected[i];
            const actual_item = actual[i];
            await assert_equals(expected_item, actual_item, path, unorderedSelections);
        }
    }
}

async function assert_equal_unordeded(expected, actual, path, unorderedSelections) {
    if (expected.length !== actual.length) {
        throw new ExographError("assert failed: expected array length " + expected.length + ", got " + actual.length)
    }

    if (expected.length !== 0) {
        const expected_item = expected.pop();

        let match = false;
        for (let i = 0; i < actual.length; i++) {
            const actual_item = actual[i];

            try {
                await assert_equals(expected_item, actual_item, path, unorderedSelections);
                // An element matched, remove it from the actual array
                actual.splice(i, 1);
                match = true;
                break;
            } catch (e) {
                if (e instanceof ExographError) {
                    // ignore (we might find a match later)
                } else {
                    throw e;
                }
            }
        }

        if (match) {
            // Test the remaining items in the array
            await assert_equal_unordeded(expected, actual, path, unorderedSelections);
        } else {
            throw new ExographError("assert failed: could not find a match for " + JSON.stringify(expected_item) + " at " + path_to_string(path)) + " in the actual array";
        }
    }
}

function array_contains_path(paths, path) {
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

// Nicer path for printing (e.g. 'a.b.c')
function path_to_string(path) {
    return "'" + path.join(".") + "'";
}