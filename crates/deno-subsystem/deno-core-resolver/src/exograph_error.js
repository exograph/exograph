// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

class ExographError extends Error {
    constructor(message) {
        super(message);
        this.name = "ExographError";
    }
}

// Need to register the ExographError class so that we can use it as a custom error (see exograph_ops.rs)
Deno[Deno.internal].core.registerErrorClass('ExographError', ExographError);
