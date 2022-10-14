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