use common::{
    api_router::ApiRouter,
    http::{RequestPayload, ResponsePayload},
};
use resolver::GraphQLRouter;

pub struct SystemRouter {
    // TODO: add other routers here (or use a vector of routers)
    graphql_router: GraphQLRouter,
}

impl SystemRouter {
    pub fn new(graphql_router: GraphQLRouter) -> Self {
        Self { graphql_router }
    }

    pub async fn route<E: 'static>(
        &self,
        request: impl RequestPayload + Send,
        playground_request: bool,
    ) -> ResponsePayload<E> {
        ApiRouter::route(&self.graphql_router, request, playground_request).await
    }

    pub fn allow_introspection(&self) -> bool {
        self.graphql_router.allow_introspection()
    }
}
