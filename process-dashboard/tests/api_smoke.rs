use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
};
use http_body_util::BodyExt;
use process_dashboard::{app_router, build_pool, Repo};
use tempfile::tempdir;
use tower::ServiceExt;

#[tokio::test]
async fn health_and_repo_create_list_smoke_test() {
    let temp_dir = tempdir().expect("create temp dir");
    let db_path = temp_dir.path().join("dashboard.db");
    let database_url = format!("sqlite://{}", db_path.display());

    let pool = build_pool(&database_url).await.expect("build pool");
    let app = app_router(pool);

    let health_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/health")
                .method(Method::GET)
                .body(Body::empty())
                .expect("build request"),
        )
        .await
        .expect("health response");

    assert_eq!(health_response.status(), StatusCode::OK);

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/repos")
                .method(Method::POST)
                .header("content-type", "application/json")
                .body(Body::from(r#"{"identifier":"openai/codex"}"#))
                .expect("build request"),
        )
        .await
        .expect("create response");

    assert_eq!(create_response.status(), StatusCode::CREATED);

    let list_response = app
        .oneshot(
            Request::builder()
                .uri("/api/repos")
                .method(Method::GET)
                .body(Body::empty())
                .expect("build request"),
        )
        .await
        .expect("list response");

    assert_eq!(list_response.status(), StatusCode::OK);

    let body = list_response
        .into_body()
        .collect()
        .await
        .expect("read body")
        .to_bytes();

    let repos: Vec<Repo> = serde_json::from_slice(&body).expect("parse repos");
    assert_eq!(repos.len(), 1);
    assert_eq!(repos[0].identifier, "openai/codex");
}
