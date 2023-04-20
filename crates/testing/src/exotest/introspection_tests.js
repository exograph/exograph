// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

import { buildClientSchema } from "embedded://graphql/utilities/buildClientSchema.mjs"
import { getIntrospectionQuery } from "embedded://graphql/utilities/getIntrospectionQuery.mjs"
import { assertValidSchema } from "embedded://graphql/type/validate.mjs"

export async function assertSchema(endpoint) {
    let response = await fetch(endpoint, {
        method: "POST",
        headers: {
            "Content-Type": "application/json"
        },
        body: JSON.stringify({"query": getIntrospectionQuery()})
    })

    const schema = (await response.json())["data"]
    const clientSchema = buildClientSchema(schema)

    assertValidSchema(clientSchema)
}