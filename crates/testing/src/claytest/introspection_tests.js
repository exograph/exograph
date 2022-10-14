import { buildClientSchema } from "embedded://graphql/utilities/buildClientSchema.mjs"
import { assertValidSchema } from "embedded://graphql/type/validate.mjs"

const response = "%%RESPONSE%%";
const schema = response["data"]
const clientSchema = buildClientSchema(schema)

assertValidSchema(clientSchema)