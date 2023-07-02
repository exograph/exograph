import {
  Fetcher,
  fetcherReturnToPromise,
} from "@graphiql/toolkit";
import {
  getIntrospectionQuery,
  buildClientSchema,
  IntrospectionQuery,
  GraphQLSchema,
} from "graphql";

export type SchemaError = "EmptySchema" | "InvalidSchema" | "NetworkError";

export async function fetchSchema(
  fetcher: Fetcher
): Promise<GraphQLSchema | SchemaError> {
  const source = getIntrospectionQuery();
  try {
    const executionResult = await fetcher({ query: source });
    const fetcherResult = await fetcherReturnToPromise(executionResult);

    if (fetcherResult?.data && "__schema" in fetcherResult.data) {
      return buildClientSchema(fetcherResult.data as IntrospectionQuery);
    } else {
      if (typeof fetcherResult === "string") {
        return "InvalidSchema";
      } else {
        // When we have no queries (which GraphQL spec doesn't allow, but can happen in an Exograph model), we get this error message.
        // We use this to print an informative message to the user.
        const noQueryMessage = (fetcherResult as any).errors?.find((error: any) => {
          return error.message === "No such operation 'Query'";
        });

        if (noQueryMessage) {
          return "EmptySchema";
        } else {
          return "InvalidSchema"
        }
      }
    }
  } catch (e) {
    return "NetworkError";
  }
}
