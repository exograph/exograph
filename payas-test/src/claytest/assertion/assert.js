globalThis.test = function (actualPayload, testvariables) {
    var $ = testvariables;
    const expectedPayload = "%%EXPECTED%%";

    var lastKey = undefined;

    function _test(expected, actual) {
        switch (typeof(expected)) {
            case "object": {
                // recursively verify that all key/values in expectedResponse are present in actualValue
                for (const key in expected) {
                    lastKey = key;
                    const expectedValue = expected[key];
                    const actualValue = actual[key];

                    _test(expectedValue, actualValue, testvariables);
                }

                // recursively verify that no extraneous key/values are present in actualValue
                for (const key in actual) {
                    if (expected[key] === undefined) {
                        throw new ClaytipError("unexpected key " + key.toString() + " in actual response")
                    }
                }

                break;
            }
            case "function": {
                const result = expected(actual);

                if (result === false) {
                    throw new ClaytipError("assert function failed for field " + lastKey + "!", expected)
                }
                break;
            }
            default: {
                if (expected !== actual) {
                    throw new ClaytipError("assert failed: expected " + expected + ", got " + actual)
                }
                break;
            }
        }
    }

    _test(expectedPayload, actualPayload);
}