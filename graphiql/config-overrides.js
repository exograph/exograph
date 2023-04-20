// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

// config-overrides.js
module.exports = function override(config, env) {
    // determine publicPath automatically, as we will not know this due to EXO_PLAYGROUND_HTTP_PATH.
    config.output.publicPath = "auto";

    return config
}