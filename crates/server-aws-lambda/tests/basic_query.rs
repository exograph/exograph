use serde_json::json;

mod common;

#[tokio::test]
async fn test_basic_query() {
    common::test_query(
        serde_json::from_str(include_str!("basic_query_input.json")).unwrap(),
        include_str!("model.clay"),
        json!({
            "isBase64Encoded": false,
            "statusCode": 200,
            "headers": {},
            "multiValueHeaders": {},
            "body": "{\"errors\": [{\"message\":\"Postgres operation failed\"}]}"
        }),
    )
    .await
}
