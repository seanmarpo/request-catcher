use actix_web::{test, web, App};
use request_catcher::{routes, AppState, CreateBucketPayload};
use serde_json::{json, Value};

const PASSWORD_HEADER: &str = "X-Bucket-Password";
const TEST_PASSWORD: &str = "test_password_123";

/// Builds the app with the same routes `main` uses.
fn create_test_app() -> App<
    impl actix_web::dev::ServiceFactory<
        actix_web::dev::ServiceRequest,
        Config = (),
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
        InitError = (),
    >,
> {
    App::new()
        .app_data(web::Data::new(AppState::default()))
        .configure(routes)
}

/// Returns every value of header `name` in a captured request.
fn header_values<'a>(request: &'a Value, name: &str) -> Vec<&'a str> {
    request["headers"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|pair| pair[0] == name)
        .map(|pair| pair[1].as_str().unwrap())
        .collect()
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
    assert_eq!(requests[0]["query"], "id=123&name=test");
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
    assert_eq!(
        header_values(&requests[0], "x-custom-header"),
        ["custom-value"]
    );
    assert_eq!(header_values(&requests[0], "x-api-key"), ["secret-key-123"]);
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
            .set_json(json!({"index": i}))
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

    // Newest first: page 1 starts with the last request sent
    let requests_page1 = response["requests"].as_array().unwrap();
    assert_eq!(requests_page1[0]["path"], "/test-bucket/api/resource/1005");

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
    // The oldest remaining request is the 6th sent (first 5 were evicted)
    assert_eq!(requests_page2[499]["path"], "/test-bucket/api/resource/6");
}

// ---------------------------------------------------------------------------
// Helpers for the tests below
// ---------------------------------------------------------------------------

fn create_req(name: &str) -> test::TestRequest {
    test::TestRequest::post()
        .uri(&format!("/api/create/{name}"))
        .set_json(&CreateBucketPayload {
            password: TEST_PASSWORD.to_string(),
        })
}

fn fetch_req(uri: &str) -> test::TestRequest {
    test::TestRequest::get()
        .uri(uri)
        .insert_header((PASSWORD_HEADER, TEST_PASSWORD))
}

#[actix_web::test]
async fn test_pagination_is_newest_first() {
    let app = test::init_service(create_test_app()).await;
    test::call_service(&app, create_req("pg").to_request())
        .await
        .status();
    for i in 1..=25 {
        let req = test::TestRequest::get()
            .uri(&format!("/pg/{i}"))
            .to_request();
        test::call_service(&app, req).await;
    }

    let page1 = test::call_and_read_body_json::<_, _, Value>(
        &app,
        fetch_req("/api/requests/pg?page=1&page_size=10").to_request(),
    )
    .await;
    let paths: Vec<&str> = page1["requests"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["path"].as_str().unwrap())
        .collect();
    assert_eq!(paths.first(), Some(&"/pg/25"));
    assert_eq!(paths.last(), Some(&"/pg/16"));

    let page3 = test::call_and_read_body_json::<_, _, Value>(
        &app,
        fetch_req("/api/requests/pg?page=3&page_size=10").to_request(),
    )
    .await;
    assert_eq!(page3["requests"][4]["path"], "/pg/1");
}

#[actix_web::test]
async fn test_huge_page_number_does_not_panic() {
    let app = test::init_service(create_test_app()).await;
    test::call_service(&app, create_req("pg").to_request())
        .await
        .status();
    let uri = format!("/api/requests/pg?page={}&page_size=500", usize::MAX);
    let resp =
        test::call_and_read_body_json::<_, _, Value>(&app, fetch_req(&uri).to_request()).await;
    assert_eq!(resp["requests"].as_array().unwrap().len(), 0);
}

#[actix_web::test]
async fn test_request_ids_are_sequential_and_survive_clear() {
    let app = test::init_service(create_test_app()).await;
    test::call_service(&app, create_req("ids").to_request())
        .await
        .status();
    for _ in 0..2 {
        test::call_service(&app, test::TestRequest::get().uri("/ids").to_request()).await;
    }
    let req = test::TestRequest::post()
        .uri("/api/clear/ids")
        .insert_header((PASSWORD_HEADER, TEST_PASSWORD))
        .to_request();
    test::call_service(&app, req).await;
    test::call_service(&app, test::TestRequest::get().uri("/ids").to_request()).await;

    let resp = test::call_and_read_body_json::<_, _, Value>(
        &app,
        fetch_req("/api/requests/ids").to_request(),
    )
    .await;
    assert_eq!(resp["requests"][0]["id"], 3);
}

#[actix_web::test]
async fn test_non_ascii_bucket_name_rejected() {
    let app = test::init_service(create_test_app()).await;
    assert_eq!(
        test::call_service(&app, create_req("caf%C3%A9").to_request())
            .await
            .status(),
        400
    );
}

#[actix_web::test]
async fn test_repeated_headers_and_query_params_preserved() {
    let app = test::init_service(create_test_app()).await;
    test::call_service(&app, create_req("dup").to_request())
        .await
        .status();
    let req = test::TestRequest::get()
        .uri("/dup/x?a=1&a=2&b")
        .append_header(("X-Test", "one"))
        .append_header(("X-Test", "two"))
        .to_request();
    test::call_service(&app, req).await;

    let resp = test::call_and_read_body_json::<_, _, Value>(
        &app,
        fetch_req("/api/requests/dup").to_request(),
    )
    .await;
    let captured = &resp["requests"][0];
    assert_eq!(captured["query"], "a=1&a=2&b");
    assert_eq!(header_values(captured, "x-test"), ["one", "two"]);
}

#[actix_web::test]
async fn test_remote_addr_uses_peer_address_by_default() {
    let app = test::init_service(create_test_app()).await;
    test::call_service(&app, create_req("ip").to_request())
        .await
        .status();
    let req = test::TestRequest::get()
        .uri("/ip")
        .peer_addr("203.0.113.7:4444".parse().unwrap())
        .insert_header(("Fly-Client-IP", "198.51.100.1"))
        .to_request();
    test::call_service(&app, req).await;

    let resp = test::call_and_read_body_json::<_, _, Value>(
        &app,
        fetch_req("/api/requests/ip").to_request(),
    )
    .await;
    // Header is ignored unless CLIENT_IP_HEADER is configured
    assert_eq!(resp["requests"][0]["remote_addr"], "203.0.113.7");
}

#[actix_web::test]
async fn test_remote_addr_uses_configured_header() {
    let state = AppState {
        client_ip_header: Some("fly-client-ip".parse().unwrap()),
        ..Default::default()
    };
    let app =
        test::init_service(App::new().app_data(web::Data::new(state)).configure(routes)).await;
    test::call_service(&app, create_req("ip").to_request())
        .await
        .status();

    // Header present: use it
    let req = test::TestRequest::get()
        .uri("/ip")
        .peer_addr("203.0.113.7:4444".parse().unwrap())
        .insert_header(("Fly-Client-IP", "198.51.100.1"))
        .to_request();
    test::call_service(&app, req).await;
    // Header absent: fall back to peer address
    let req = test::TestRequest::get()
        .uri("/ip")
        .peer_addr("203.0.113.7:4444".parse().unwrap())
        .to_request();
    test::call_service(&app, req).await;

    let resp = test::call_and_read_body_json::<_, _, Value>(
        &app,
        fetch_req("/api/requests/ip").to_request(),
    )
    .await;
    assert_eq!(resp["requests"][1]["remote_addr"], "198.51.100.1");
    assert_eq!(resp["requests"][0]["remote_addr"], "203.0.113.7");
}

#[actix_web::test]
async fn test_wrong_password_delete_leaves_bucket_capturing() {
    let app = test::init_service(create_test_app()).await;
    test::call_service(&app, create_req("keep").to_request())
        .await
        .status();
    let req = test::TestRequest::delete()
        .uri("/api/delete/keep")
        .insert_header((PASSWORD_HEADER, "wrong"))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status(), 401);

    let req = test::TestRequest::get().uri("/keep").to_request();
    assert_eq!(test::call_service(&app, req).await.status(), 200);
    // Password is unchanged
    assert_eq!(
        test::call_and_read_body_json::<_, _, Value>(
            &app,
            fetch_req("/api/requests/keep").to_request()
        )
        .await["total"],
        1
    );
}

#[actix_web::test]
async fn test_ui_is_embedded() {
    let app = test::init_service(create_test_app()).await;
    for uri in ["/ui/", "/ui/index.html", "/ui/bucket.html"] {
        let resp = test::call_service(&app, test::TestRequest::get().uri(uri).to_request()).await;
        assert_eq!(resp.status(), 200, "{uri}");
        assert_eq!(
            resp.headers().get("content-type").unwrap(),
            "text/html; charset=utf-8"
        );
    }
    for uri in ["/", "/ui"] {
        let resp = test::call_service(&app, test::TestRequest::get().uri(uri).to_request()).await;
        assert_eq!(resp.status(), 302, "{uri}");
        assert_eq!(resp.headers().get("location").unwrap(), "/ui/");
    }
}

#[actix_web::test]
async fn test_version_endpoint() {
    let app = test::init_service(create_test_app()).await;
    let req = test::TestRequest::get().uri("/api/version").to_request();
    let body = test::call_and_read_body(&app, req).await;
    assert_eq!(body, env!("CARGO_PKG_VERSION"));
}
