use actix_web::{test, web, App};
use request_catcher::{
    capture_request, clear_bucket_requests, create_bucket, delete_bucket, get_bucket_requests,
    list_buckets, AppState, CreateBucketPayload,
};
use serde_json::json;

const PASSWORD_HEADER: &str = "X-Bucket-Password";
const TEST_PASSWORD: &str = "test_password_123";

/// Helper function to create a test app with initialized state
fn create_test_app() -> App<
    impl actix_web::dev::ServiceFactory<
        actix_web::dev::ServiceRequest,
        Config = (),
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
        InitError = (),
    >,
> {
    let app_state = web::Data::new(AppState {
        buckets: Default::default(),
    });

    App::new()
        .app_data(app_state.clone())
        .app_data(web::PayloadConfig::new(10 * 1024 * 1024)) // 10MB
        .service(
            web::scope("/api")
                .route("/buckets", web::get().to(list_buckets))
                .route(
                    "/clear/{bucket_name}",
                    web::post().to(clear_bucket_requests),
                )
                .route("/delete/{bucket_name}", web::delete().to(delete_bucket))
                .route("/create/{bucket_name}", web::post().to(create_bucket))
                .route(
                    "/requests/{bucket_name}",
                    web::get().to(get_bucket_requests),
                ),
        )
        .route("/{path:.*}", web::route().to(capture_request))
}

#[actix_web::test]
async fn test_create_bucket() {
    let app = test::init_service(create_test_app()).await;

    let payload = CreateBucketPayload {
        password: TEST_PASSWORD.to_string(),
    };

    let req = test::TestRequest::post()
        .uri("/api/create/test-bucket")
        .set_json(&payload)
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
}

#[actix_web::test]
async fn test_create_bucket_with_empty_password() {
    let app = test::init_service(create_test_app()).await;

    let payload = CreateBucketPayload {
        password: "".to_string(),
    };

    let req = test::TestRequest::post()
        .uri("/api/create/test-bucket")
        .set_json(&payload)
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);
}

#[actix_web::test]
async fn test_create_duplicate_bucket() {
    let app = test::init_service(create_test_app()).await;

    let payload = CreateBucketPayload {
        password: TEST_PASSWORD.to_string(),
    };

    // Create first bucket
    let req = test::TestRequest::post()
        .uri("/api/create/test-bucket")
        .set_json(&payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    // Try to create duplicate
    let req = test::TestRequest::post()
        .uri("/api/create/test-bucket")
        .set_json(&payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 409); // Conflict
}

#[actix_web::test]
async fn test_create_bucket_with_reserved_name_api() {
    let app = test::init_service(create_test_app()).await;

    let payload = CreateBucketPayload {
        password: TEST_PASSWORD.to_string(),
    };

    // Try to create bucket named "api" (reserved)
    let req = test::TestRequest::post()
        .uri("/api/create/api")
        .set_json(&payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400); // Bad Request
}

#[actix_web::test]
async fn test_create_bucket_with_reserved_name_ui() {
    let app = test::init_service(create_test_app()).await;

    let payload = CreateBucketPayload {
        password: TEST_PASSWORD.to_string(),
    };

    // Try to create bucket named "ui" (reserved)
    let req = test::TestRequest::post()
        .uri("/api/create/ui")
        .set_json(&payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400); // Bad Request
}

#[actix_web::test]
async fn test_reserved_routes_still_work() {
    let app = test::init_service(create_test_app()).await;

    // Verify /api/buckets route works (not captured as bucket)
    let req = test::TestRequest::get().uri("/api/buckets").to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    // Verify it returns a list, not a 404
    let body = test::read_body(resp).await;
    let _buckets: Vec<String> = serde_json::from_slice(&body).unwrap();
}

#[actix_web::test]
async fn test_list_buckets() {
    let app = test::init_service(create_test_app()).await;

    // Create a couple of buckets
    let payload = CreateBucketPayload {
        password: TEST_PASSWORD.to_string(),
    };

    for bucket_name in ["bucket1", "bucket2", "bucket3"] {
        let req = test::TestRequest::post()
            .uri(&format!("/api/create/{}", bucket_name))
            .set_json(&payload)
            .to_request();
        test::call_service(&app, req).await;
    }

    // List buckets
    let req = test::TestRequest::get().uri("/api/buckets").to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let body = test::read_body(resp).await;
    let buckets: Vec<String> = serde_json::from_slice(&body).unwrap();
    assert_eq!(buckets.len(), 3);
    assert!(buckets.contains(&"bucket1".to_string()));
    assert!(buckets.contains(&"bucket2".to_string()));
    assert!(buckets.contains(&"bucket3".to_string()));
}

#[actix_web::test]
async fn test_capture_get_request() {
    let app = test::init_service(create_test_app()).await;

    // Create bucket
    let payload = CreateBucketPayload {
        password: TEST_PASSWORD.to_string(),
    };
    let req = test::TestRequest::post()
        .uri("/api/create/test-bucket")
        .set_json(&payload)
        .to_request();
    test::call_service(&app, req).await;

    // Send GET request to bucket
    let req = test::TestRequest::get()
        .uri("/test-bucket/api/users?id=123&name=test")
        .insert_header(("User-Agent", "Test-Agent"))
        .insert_header(("Authorization", "Bearer token123"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    // Retrieve requests
    let req = test::TestRequest::get()
        .uri("/api/requests/test-bucket")
        .insert_header((PASSWORD_HEADER, TEST_PASSWORD))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let body = test::read_body(resp).await;
    let response: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let requests = response["requests"].as_array().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0]["method"], "GET");
    assert_eq!(requests[0]["path"], "/test-bucket/api/users");
    assert_eq!(requests[0]["query_params"]["id"], "123");
    assert_eq!(requests[0]["query_params"]["name"], "test");
}

#[actix_web::test]
async fn test_capture_post_request_with_json() {
    let app = test::init_service(create_test_app()).await;

    // Create bucket
    let payload = CreateBucketPayload {
        password: TEST_PASSWORD.to_string(),
    };
    let req = test::TestRequest::post()
        .uri("/api/create/test-bucket")
        .set_json(&payload)
        .to_request();
    test::call_service(&app, req).await;

    // Send POST request with JSON body
    let json_body = json!({
        "username": "john_doe",
        "email": "john@example.com",
        "age": 30,
        "active": true,
        "tags": ["developer", "rust"]
    });

    let req = test::TestRequest::post()
        .uri("/test-bucket/api/users")
        .insert_header(("Content-Type", "application/json"))
        .set_json(&json_body)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    // Retrieve requests
    let req = test::TestRequest::get()
        .uri("/api/requests/test-bucket")
        .insert_header((PASSWORD_HEADER, TEST_PASSWORD))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body = test::read_body(resp).await;
    let response: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let requests = response["requests"].as_array().unwrap();

    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0]["method"], "POST");
    assert_eq!(requests[0]["path"], "/test-bucket/api/users");

    // Verify JSON body was captured
    let captured_body: serde_json::Value =
        serde_json::from_str(requests[0]["body"].as_str().unwrap()).unwrap();
    assert_eq!(captured_body["username"], "john_doe");
    assert_eq!(captured_body["age"], 30);
    assert_eq!(captured_body["active"], true);
}

#[actix_web::test]
async fn test_capture_put_request() {
    let app = test::init_service(create_test_app()).await;

    // Create bucket
    let payload = CreateBucketPayload {
        password: TEST_PASSWORD.to_string(),
    };
    let req = test::TestRequest::post()
        .uri("/api/create/test-bucket")
        .set_json(&payload)
        .to_request();
    test::call_service(&app, req).await;

    // Send PUT request
    let json_body = json!({"status": "updated"});
    let req = test::TestRequest::put()
        .uri("/test-bucket/api/users/123")
        .set_json(&json_body)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    // Retrieve requests
    let req = test::TestRequest::get()
        .uri("/api/requests/test-bucket")
        .insert_header((PASSWORD_HEADER, TEST_PASSWORD))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body = test::read_body(resp).await;
    let response: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let requests = response["requests"].as_array().unwrap();

    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0]["method"], "PUT");
}

#[actix_web::test]
async fn test_capture_patch_request() {
    let app = test::init_service(create_test_app()).await;

    // Create bucket
    let payload = CreateBucketPayload {
        password: TEST_PASSWORD.to_string(),
    };
    let req = test::TestRequest::post()
        .uri("/api/create/test-bucket")
        .set_json(&payload)
        .to_request();
    test::call_service(&app, req).await;

    // Send PATCH request
    let json_body = json!({"email": "newemail@example.com"});
    let req = test::TestRequest::patch()
        .uri("/test-bucket/api/users/123")
        .set_json(&json_body)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    // Retrieve requests
    let req = test::TestRequest::get()
        .uri("/api/requests/test-bucket")
        .insert_header((PASSWORD_HEADER, TEST_PASSWORD))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body = test::read_body(resp).await;
    let response: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let requests = response["requests"].as_array().unwrap();

    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0]["method"], "PATCH");
}

#[actix_web::test]
async fn test_capture_delete_request() {
    let app = test::init_service(create_test_app()).await;

    // Create bucket
    let payload = CreateBucketPayload {
        password: TEST_PASSWORD.to_string(),
    };
    let req = test::TestRequest::post()
        .uri("/api/create/test-bucket")
        .set_json(&payload)
        .to_request();
    test::call_service(&app, req).await;

    // Send DELETE request
    let req = test::TestRequest::delete()
        .uri("/test-bucket/api/users/123")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    // Retrieve requests
    let req = test::TestRequest::get()
        .uri("/api/requests/test-bucket")
        .insert_header((PASSWORD_HEADER, TEST_PASSWORD))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body = test::read_body(resp).await;
    let response: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let requests = response["requests"].as_array().unwrap();

    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0]["method"], "DELETE");
}

#[actix_web::test]
async fn test_capture_head_request() {
    let app = test::init_service(create_test_app()).await;

    // Create bucket
    let payload = CreateBucketPayload {
        password: TEST_PASSWORD.to_string(),
    };
    let req = test::TestRequest::post()
        .uri("/api/create/test-bucket")
        .set_json(&payload)
        .to_request();
    test::call_service(&app, req).await;

    // Send HEAD request
    let req = test::TestRequest::default()
        .method(actix_web::http::Method::HEAD)
        .uri("/test-bucket/api/users")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    // Retrieve requests
    let req = test::TestRequest::get()
        .uri("/api/requests/test-bucket")
        .insert_header((PASSWORD_HEADER, TEST_PASSWORD))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body = test::read_body(resp).await;
    let response: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let requests = response["requests"].as_array().unwrap();

    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0]["method"], "HEAD");
}

#[actix_web::test]
async fn test_capture_options_request() {
    let app = test::init_service(create_test_app()).await;

    // Create bucket
    let payload = CreateBucketPayload {
        password: TEST_PASSWORD.to_string(),
    };
    let req = test::TestRequest::post()
        .uri("/api/create/test-bucket")
        .set_json(&payload)
        .to_request();
    test::call_service(&app, req).await;

    // Send OPTIONS request
    let req = test::TestRequest::default()
        .method(actix_web::http::Method::OPTIONS)
        .uri("/test-bucket/api/users")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    // Retrieve requests
    let req = test::TestRequest::get()
        .uri("/api/requests/test-bucket")
        .insert_header((PASSWORD_HEADER, TEST_PASSWORD))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body = test::read_body(resp).await;
    let response: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let requests = response["requests"].as_array().unwrap();

    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0]["method"], "OPTIONS");
}

#[actix_web::test]
async fn test_capture_request_with_form_data() {
    let app = test::init_service(create_test_app()).await;

    // Create bucket
    let payload = CreateBucketPayload {
        password: TEST_PASSWORD.to_string(),
    };
    let req = test::TestRequest::post()
        .uri("/api/create/test-bucket")
        .set_json(&payload)
        .to_request();
    test::call_service(&app, req).await;

    // Send POST request with form data
    let form_data = "username=john&password=secret123&remember=on";
    let req = test::TestRequest::post()
        .uri("/test-bucket/login")
        .insert_header(("Content-Type", "application/x-www-form-urlencoded"))
        .set_payload(form_data)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    // Retrieve requests
    let req = test::TestRequest::get()
        .uri("/api/requests/test-bucket")
        .insert_header((PASSWORD_HEADER, TEST_PASSWORD))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body = test::read_body(resp).await;
    let response: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let requests = response["requests"].as_array().unwrap();

    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0]["method"], "POST");
    assert!(requests[0]["body"]
        .as_str()
        .unwrap()
        .contains("username=john"));
}

#[actix_web::test]
async fn test_capture_request_with_custom_headers() {
    let app = test::init_service(create_test_app()).await;

    // Create bucket
    let payload = CreateBucketPayload {
        password: TEST_PASSWORD.to_string(),
    };
    let req = test::TestRequest::post()
        .uri("/api/create/test-bucket")
        .set_json(&payload)
        .to_request();
    test::call_service(&app, req).await;

    // Send request with custom headers
    let req = test::TestRequest::get()
        .uri("/test-bucket/api/test")
        .insert_header(("X-Custom-Header", "custom-value"))
        .insert_header(("X-API-Key", "secret-key-123"))
        .insert_header(("User-Agent", "Test-Client/1.0"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    // Retrieve requests
    let req = test::TestRequest::get()
        .uri("/api/requests/test-bucket")
        .insert_header((PASSWORD_HEADER, TEST_PASSWORD))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body = test::read_body(resp).await;
    let response: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let requests = response["requests"].as_array().unwrap();

    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0]["headers"]["x-custom-header"], "custom-value");
    assert_eq!(requests[0]["headers"]["x-api-key"], "secret-key-123");
}

#[actix_web::test]
async fn test_capture_multiple_requests() {
    let app = test::init_service(create_test_app()).await;

    // Create bucket
    let payload = CreateBucketPayload {
        password: TEST_PASSWORD.to_string(),
    };
    let req = test::TestRequest::post()
        .uri("/api/create/test-bucket")
        .set_json(&payload)
        .to_request();
    test::call_service(&app, req).await;

    // Send multiple requests
    for i in 1..=5 {
        let req = test::TestRequest::get()
            .uri(&format!("/test-bucket/api/resource/{}", i))
            .to_request();
        test::call_service(&app, req).await;
    }

    // Retrieve requests
    let req = test::TestRequest::get()
        .uri("/api/requests/test-bucket")
        .insert_header((PASSWORD_HEADER, TEST_PASSWORD))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body = test::read_body(resp).await;
    let response: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let requests = response["requests"].as_array().unwrap();

    assert_eq!(requests.len(), 5);
}

#[actix_web::test]
async fn test_clear_bucket_requests() {
    let app = test::init_service(create_test_app()).await;

    // Create bucket
    let payload = CreateBucketPayload {
        password: TEST_PASSWORD.to_string(),
    };
    let req = test::TestRequest::post()
        .uri("/api/create/test-bucket")
        .set_json(&payload)
        .to_request();
    test::call_service(&app, req).await;

    // Send some requests
    for i in 1..=3 {
        let req = test::TestRequest::get()
            .uri(&format!("/test-bucket/api/resource/{}", i))
            .to_request();
        test::call_service(&app, req).await;
    }

    // Clear requests
    let req = test::TestRequest::post()
        .uri("/api/clear/test-bucket")
        .insert_header((PASSWORD_HEADER, TEST_PASSWORD))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    // Verify requests are cleared
    let req = test::TestRequest::get()
        .uri("/api/requests/test-bucket")
        .insert_header((PASSWORD_HEADER, TEST_PASSWORD))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body = test::read_body(resp).await;
    let response: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let requests = response["requests"].as_array().unwrap();

    assert_eq!(requests.len(), 0);
}

#[actix_web::test]
async fn test_clear_bucket_with_wrong_password() {
    let app = test::init_service(create_test_app()).await;

    // Create bucket
    let payload = CreateBucketPayload {
        password: TEST_PASSWORD.to_string(),
    };
    let req = test::TestRequest::post()
        .uri("/api/create/test-bucket")
        .set_json(&payload)
        .to_request();
    test::call_service(&app, req).await;

    // Try to clear with wrong password
    let req = test::TestRequest::post()
        .uri("/api/clear/test-bucket")
        .insert_header((PASSWORD_HEADER, "wrong_password"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 401); // Unauthorized
}

#[actix_web::test]
async fn test_delete_bucket() {
    let app = test::init_service(create_test_app()).await;

    // Create bucket
    let payload = CreateBucketPayload {
        password: TEST_PASSWORD.to_string(),
    };
    let req = test::TestRequest::post()
        .uri("/api/create/test-bucket")
        .set_json(&payload)
        .to_request();
    test::call_service(&app, req).await;

    // Delete bucket
    let req = test::TestRequest::delete()
        .uri("/api/delete/test-bucket")
        .insert_header((PASSWORD_HEADER, TEST_PASSWORD))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    // Verify bucket is deleted
    let req = test::TestRequest::get()
        .uri("/api/requests/test-bucket")
        .insert_header((PASSWORD_HEADER, TEST_PASSWORD))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 404); // Not Found
}

#[actix_web::test]
async fn test_delete_bucket_with_wrong_password() {
    let app = test::init_service(create_test_app()).await;

    // Create bucket
    let payload = CreateBucketPayload {
        password: TEST_PASSWORD.to_string(),
    };
    let req = test::TestRequest::post()
        .uri("/api/create/test-bucket")
        .set_json(&payload)
        .to_request();
    test::call_service(&app, req).await;

    // Try to delete with wrong password
    let req = test::TestRequest::delete()
        .uri("/api/delete/test-bucket")
        .insert_header((PASSWORD_HEADER, "wrong_password"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 401); // Unauthorized
}

#[actix_web::test]
async fn test_get_requests_without_password() {
    let app = test::init_service(create_test_app()).await;

    // Create bucket
    let payload = CreateBucketPayload {
        password: TEST_PASSWORD.to_string(),
    };
    let req = test::TestRequest::post()
        .uri("/api/create/test-bucket")
        .set_json(&payload)
        .to_request();
    test::call_service(&app, req).await;

    // Try to get requests without password header
    let req = test::TestRequest::get()
        .uri("/api/requests/test-bucket")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 401); // Unauthorized
}

#[actix_web::test]
async fn test_capture_request_to_nonexistent_bucket() {
    let app = test::init_service(create_test_app()).await;

    // Try to send request to non-existent bucket
    let req = test::TestRequest::get()
        .uri("/nonexistent-bucket/api/test")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 404); // Not Found
}

#[actix_web::test]
async fn test_capture_large_json_payload() {
    let app = test::init_service(create_test_app()).await;

    // Create bucket
    let payload = CreateBucketPayload {
        password: TEST_PASSWORD.to_string(),
    };
    let req = test::TestRequest::post()
        .uri("/api/create/test-bucket")
        .set_json(&payload)
        .to_request();
    test::call_service(&app, req).await;

    // Create a large JSON payload
    let mut large_array = Vec::new();
    for i in 0..1000 {
        large_array.push(json!({
            "id": i,
            "name": format!("Item {}", i),
            "description": "This is a test item with some description text",
            "value": i * 100,
        }));
    }
    let large_json = json!({
        "items": large_array,
        "metadata": {
            "count": 1000,
            "timestamp": "2024-01-01T00:00:00Z"
        }
    });

    // Send large POST request
    let req = test::TestRequest::post()
        .uri("/test-bucket/api/bulk-upload")
        .insert_header(("Content-Type", "application/json"))
        .set_json(&large_json)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    // Retrieve and verify
    let req = test::TestRequest::get()
        .uri("/api/requests/test-bucket")
        .insert_header((PASSWORD_HEADER, TEST_PASSWORD))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body = test::read_body(resp).await;
    let response: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let requests = response["requests"].as_array().unwrap();

    assert_eq!(requests.len(), 1);
    assert!(requests[0]["body"].as_str().unwrap().len() > 10000);
}

#[actix_web::test]
async fn test_capture_request_with_empty_body() {
    let app = test::init_service(create_test_app()).await;

    // Create bucket
    let payload = CreateBucketPayload {
        password: TEST_PASSWORD.to_string(),
    };
    let req = test::TestRequest::post()
        .uri("/api/create/test-bucket")
        .set_json(&payload)
        .to_request();
    test::call_service(&app, req).await;

    // Send POST with empty body
    let req = test::TestRequest::post()
        .uri("/test-bucket/api/test")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    // Retrieve requests
    let req = test::TestRequest::get()
        .uri("/api/requests/test-bucket")
        .insert_header((PASSWORD_HEADER, TEST_PASSWORD))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body = test::read_body(resp).await;
    let response: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let requests = response["requests"].as_array().unwrap();

    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0]["body"], "");
}

#[actix_web::test]
async fn test_capture_request_with_special_characters_in_path() {
    let app = test::init_service(create_test_app()).await;

    // Create bucket
    let payload = CreateBucketPayload {
        password: TEST_PASSWORD.to_string(),
    };
    let req = test::TestRequest::post()
        .uri("/api/create/test-bucket")
        .set_json(&payload)
        .to_request();
    test::call_service(&app, req).await;

    // Send request with special characters
    let req = test::TestRequest::get()
        .uri("/test-bucket/api/users/%20test%20/data")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    // Retrieve requests
    let req = test::TestRequest::get()
        .uri("/api/requests/test-bucket")
        .insert_header((PASSWORD_HEADER, TEST_PASSWORD))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body = test::read_body(resp).await;
    let response: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let requests = response["requests"].as_array().unwrap();

    assert_eq!(requests.len(), 1);
    assert!(requests[0]["path"]
        .as_str()
        .unwrap()
        .contains("test-bucket"));
}

#[actix_web::test]
async fn test_concurrent_requests_to_same_bucket() {
    let app = test::init_service(create_test_app()).await;

    // Create bucket
    let payload = CreateBucketPayload {
        password: TEST_PASSWORD.to_string(),
    };
    let req = test::TestRequest::post()
        .uri("/api/create/test-bucket")
        .set_json(&payload)
        .to_request();
    test::call_service(&app, req).await;

    // Send multiple requests
    for i in 0..10 {
        let req = test::TestRequest::post()
            .uri(&format!("/test-bucket/api/concurrent/{}", i))
            .set_json(&json!({"index": i}))
            .to_request();
        test::call_service(&app, req).await;
    }

    // Retrieve and verify all requests were captured
    let req = test::TestRequest::get()
        .uri("/api/requests/test-bucket")
        .insert_header((PASSWORD_HEADER, TEST_PASSWORD))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body = test::read_body(resp).await;
    let response: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let requests = response["requests"].as_array().unwrap();

    assert_eq!(requests.len(), 10);
}

#[actix_web::test]
async fn test_request_timestamp_is_set() {
    let app = test::init_service(create_test_app()).await;

    // Create bucket
    let payload = CreateBucketPayload {
        password: TEST_PASSWORD.to_string(),
    };
    let req = test::TestRequest::post()
        .uri("/api/create/test-bucket")
        .set_json(&payload)
        .to_request();
    test::call_service(&app, req).await;

    // Send request
    let req = test::TestRequest::get()
        .uri("/test-bucket/api/test")
        .to_request();
    test::call_service(&app, req).await;

    // Retrieve requests
    let req = test::TestRequest::get()
        .uri("/api/requests/test-bucket")
        .insert_header((PASSWORD_HEADER, TEST_PASSWORD))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body = test::read_body(resp).await;
    let response: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let requests = response["requests"].as_array().unwrap();

    assert_eq!(requests.len(), 1);
    assert!(requests[0]["timestamp"].is_number());
    assert!(requests[0]["timestamp"].as_i64().unwrap() > 0);
}

#[actix_web::test]
async fn test_create_bucket_with_empty_name() {
    let app = test::init_service(create_test_app()).await;

    let payload = CreateBucketPayload {
        password: TEST_PASSWORD.to_string(),
    };

    // Try to create bucket with empty name
    let req = test::TestRequest::post()
        .uri("/api/create/")
        .set_json(&payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    // This will likely be caught by routing, but we test it anyway
    assert!(!resp.status().is_success());
}

#[actix_web::test]
async fn test_create_bucket_with_special_characters() {
    let app = test::init_service(create_test_app()).await;

    let payload = CreateBucketPayload {
        password: TEST_PASSWORD.to_string(),
    };

    // Try to create bucket with invalid characters (URL encoded)
    // These are characters that would be rejected by our validation after URL decoding
    let invalid_names = vec![
        ("test%40bucket", "test@bucket"),  // @ symbol
        ("test%20bucket", "test bucket"),  // space
        ("test%2Fbucket", "test/bucket"),  // forward slash
        ("test%5Cbucket", "test\\bucket"), // backslash
        ("test%2Ebucket", "test.bucket"),  // period
        ("test%21bucket", "test!bucket"),  // exclamation
    ];

    for (url_name, display_name) in invalid_names {
        let req = test::TestRequest::post()
            .uri(&format!("/api/create/{}", url_name))
            .set_json(&payload)
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(
            resp.status(),
            400,
            "Bucket name '{}' should be rejected",
            display_name
        );
    }
}

#[actix_web::test]
async fn test_create_bucket_with_invalid_start_end_characters() {
    let app = test::init_service(create_test_app()).await;

    let payload = CreateBucketPayload {
        password: TEST_PASSWORD.to_string(),
    };

    // Try to create buckets that start or end with hyphen/underscore
    let invalid_names = vec!["-test", "_test", "test-", "test_"];

    for name in invalid_names {
        let req = test::TestRequest::post()
            .uri(&format!("/api/create/{}", name))
            .set_json(&payload)
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(
            resp.status(),
            400,
            "Bucket name '{}' should be rejected",
            name
        );
    }
}

#[actix_web::test]
async fn test_create_bucket_with_very_long_name() {
    let app = test::init_service(create_test_app()).await;

    let payload = CreateBucketPayload {
        password: TEST_PASSWORD.to_string(),
    };

    // Create a name longer than 100 characters
    let long_name = "a".repeat(101);

    let req = test::TestRequest::post()
        .uri(&format!("/api/create/{}", long_name))
        .set_json(&payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400); // Bad Request
}

#[actix_web::test]
async fn test_create_bucket_with_valid_names() {
    let app = test::init_service(create_test_app()).await;

    let payload = CreateBucketPayload {
        password: TEST_PASSWORD.to_string(),
    };

    // Test various valid bucket names
    let valid_names = vec![
        "test",
        "test123",
        "test-bucket",
        "test_bucket",
        "test-123_bucket",
        "MyBucket",
        "bucket1",
        "a",
        "test-bucket-with-many-parts",
    ];

    for name in valid_names {
        let req = test::TestRequest::post()
            .uri(&format!("/api/create/{}", name))
            .set_json(&payload)
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert!(
            resp.status().is_success(),
            "Bucket name '{}' should be accepted",
            name
        );
    }
}

#[actix_web::test]
async fn test_pagination() {
    let app = test::init_service(create_test_app()).await;

    // Create bucket
    let payload = CreateBucketPayload {
        password: TEST_PASSWORD.to_string(),
    };
    let req = test::TestRequest::post()
        .uri("/api/create/test-bucket")
        .set_json(&payload)
        .to_request();
    test::call_service(&app, req).await;

    // Send 25 requests
    for i in 1..=25 {
        let req = test::TestRequest::get()
            .uri(&format!("/test-bucket/api/resource/{}", i))
            .to_request();
        test::call_service(&app, req).await;
    }

    // Test default pagination (page 1, 50 items per page)
    let req = test::TestRequest::get()
        .uri("/api/requests/test-bucket")
        .insert_header((PASSWORD_HEADER, TEST_PASSWORD))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body = test::read_body(resp).await;
    let response: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(response["total"], 25);
    assert_eq!(response["page"], 1);
    assert_eq!(response["page_size"], 50);
    assert_eq!(response["total_pages"], 1);
    assert_eq!(response["requests"].as_array().unwrap().len(), 25);

    // Test pagination with custom page size (10 per page)
    let req = test::TestRequest::get()
        .uri("/api/requests/test-bucket?page=1&page_size=10")
        .insert_header((PASSWORD_HEADER, TEST_PASSWORD))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body = test::read_body(resp).await;
    let response: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(response["total"], 25);
    assert_eq!(response["page"], 1);
    assert_eq!(response["page_size"], 10);
    assert_eq!(response["total_pages"], 3);
    assert_eq!(response["requests"].as_array().unwrap().len(), 10);

    // Test page 2
    let req = test::TestRequest::get()
        .uri("/api/requests/test-bucket?page=2&page_size=10")
        .insert_header((PASSWORD_HEADER, TEST_PASSWORD))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body = test::read_body(resp).await;
    let response: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(response["page"], 2);
    assert_eq!(response["requests"].as_array().unwrap().len(), 10);

    // Test last page (page 3)
    let req = test::TestRequest::get()
        .uri("/api/requests/test-bucket?page=3&page_size=10")
        .insert_header((PASSWORD_HEADER, TEST_PASSWORD))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body = test::read_body(resp).await;
    let response: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(response["page"], 3);
    assert_eq!(response["requests"].as_array().unwrap().len(), 5);

    // Test page beyond last page (should return empty)
    let req = test::TestRequest::get()
        .uri("/api/requests/test-bucket?page=4&page_size=10")
        .insert_header((PASSWORD_HEADER, TEST_PASSWORD))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body = test::read_body(resp).await;
    let response: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(response["page"], 4);
    assert_eq!(response["requests"].as_array().unwrap().len(), 0);
}

#[actix_web::test]
async fn test_max_requests_per_bucket_limit() {
    let app = test::init_service(create_test_app()).await;

    // Create bucket
    let payload = CreateBucketPayload {
        password: TEST_PASSWORD.to_string(),
    };
    let req = test::TestRequest::post()
        .uri("/api/create/test-bucket")
        .set_json(&payload)
        .to_request();
    test::call_service(&app, req).await;

    // Send 1005 requests (limit is 1000)
    for i in 1..=1005 {
        let req = test::TestRequest::get()
            .uri(&format!("/test-bucket/api/resource/{}", i))
            .to_request();
        test::call_service(&app, req).await;
    }

    // Retrieve requests with max page_size (500) - should only have 1000 total (oldest ones removed)
    let req = test::TestRequest::get()
        .uri("/api/requests/test-bucket?page=1&page_size=500")
        .insert_header((PASSWORD_HEADER, TEST_PASSWORD))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body = test::read_body(resp).await;
    let response: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(response["total"], 1000);
    assert_eq!(response["total_pages"], 2);
    assert_eq!(response["requests"].as_array().unwrap().len(), 500);

    // Verify the first request on page 1 (should be the 6th one sent, as first 5 were removed)
    let requests_page1 = response["requests"].as_array().unwrap();
    assert!(requests_page1[0]["path"]
        .as_str()
        .unwrap()
        .contains("/resource/6"));

    // Get page 2 to verify the last request
    let req = test::TestRequest::get()
        .uri("/api/requests/test-bucket?page=2&page_size=500")
        .insert_header((PASSWORD_HEADER, TEST_PASSWORD))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body = test::read_body(resp).await;
    let response: serde_json::Value = serde_json::from_slice(&body).unwrap();

    let requests_page2 = response["requests"].as_array().unwrap();
    assert_eq!(requests_page2.len(), 500);
    // The last request should be the 1005th one sent
    assert!(requests_page2[499]["path"]
        .as_str()
        .unwrap()
        .contains("/resource/1005"));
}
