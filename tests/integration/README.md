# Integration Tests Documentation

This directory contains comprehensive integration tests for the Request Catcher application. These tests ensure that all HTTP methods and request types are handled correctly.

## Test Overview

All tests are located in `integration_tests.rs` and cover the complete functionality of the application including bucket management, request capture, and authentication.

## Running Tests

```bash
# Run all integration tests
cargo test --test integration_tests

# Run a specific test
cargo test --test integration_tests test_capture_post_request_with_json

# Run tests with output
cargo test --test integration_tests -- --nocapture
```

## Test Categories

### 1. Bucket Management Tests

#### `test_create_bucket`
- **Purpose**: Verifies that buckets can be created successfully
- **Test Steps**: Creates a bucket with a valid password
- **Expected**: 200 OK response

#### `test_create_bucket_with_empty_password`
- **Purpose**: Ensures empty passwords are rejected
- **Test Steps**: Attempts to create a bucket with an empty password
- **Expected**: 400 Bad Request response

#### `test_create_duplicate_bucket`
- **Purpose**: Verifies that duplicate bucket names are prevented
- **Test Steps**: Creates a bucket, then attempts to create another with the same name
- **Expected**: 409 Conflict response on second attempt

#### `test_create_bucket_with_reserved_name_api`
- **Purpose**: Prevents creating buckets that conflict with API routes
- **Test Steps**: Attempts to create a bucket named "api"
- **Expected**: 400 Bad Request response

#### `test_create_bucket_with_reserved_name_ui`
- **Purpose**: Prevents creating buckets that conflict with UI routes
- **Test Steps**: Attempts to create a bucket named "ui"
- **Expected**: 400 Bad Request response

#### `test_create_bucket_with_empty_name`
- **Purpose**: Validates that bucket names cannot be empty
- **Test Steps**: Attempts to create a bucket with an empty name
- **Expected**: Non-success response (caught by routing or validation)

#### `test_create_bucket_with_special_characters`
- **Purpose**: Ensures only valid characters are allowed in bucket names
- **Test Steps**: Attempts to create buckets with @, space, /, \, ., ! characters
- **Expected**: 400 Bad Request for all invalid characters

#### `test_create_bucket_with_invalid_start_end_characters`
- **Purpose**: Validates that bucket names cannot start/end with hyphens or underscores
- **Test Steps**: Attempts to create buckets starting or ending with - or _
- **Expected**: 400 Bad Request response

#### `test_create_bucket_with_very_long_name`
- **Purpose**: Enforces maximum bucket name length
- **Test Steps**: Attempts to create a bucket with name > 100 characters
- **Expected**: 400 Bad Request response

#### `test_create_bucket_with_valid_names`
- **Purpose**: Verifies that legitimate bucket names are accepted
- **Test Steps**: Creates buckets with various valid name patterns
- **Expected**: All valid names are accepted successfully

#### `test_reserved_routes_still_work`
- **Purpose**: Ensures reserved routes continue to function after validation
- **Test Steps**: Accesses /api/buckets route
- **Expected**: Route works and returns bucket list

#### `test_list_buckets`
- **Purpose**: Verifies the bucket listing functionality
- **Test Steps**: Creates 3 buckets, then retrieves the list
- **Expected**: All 3 bucket names are returned in the list

#### `test_delete_bucket`
- **Purpose**: Ensures buckets can be deleted successfully
- **Test Steps**: Creates a bucket, deletes it, then verifies it's gone
- **Expected**: Bucket is successfully deleted and subsequent access returns 404

#### `test_delete_bucket_with_wrong_password`
- **Purpose**: Verifies authentication is required for deletion
- **Test Steps**: Attempts to delete a bucket with incorrect password
- **Expected**: 401 Unauthorized response

### 2. HTTP Method Tests

#### `test_capture_get_request`
- **Purpose**: Verifies GET requests are captured correctly
- **Test Steps**: Sends GET request with query parameters and headers
- **Expected**: Request captured with correct method, path, and query params
- **Notes**: Tests that query parameters are properly parsed

#### `test_capture_post_request_with_json`
- **Purpose**: Verifies POST requests with JSON bodies are captured
- **Test Steps**: Sends POST with complex JSON payload
- **Expected**: Request captured with valid JSON body intact
- **Notes**: Tests JSON serialization/deserialization

#### `test_capture_put_request`
- **Purpose**: Verifies PUT requests are captured correctly
- **Test Steps**: Sends PUT request with JSON body
- **Expected**: Request captured with method "PUT"

#### `test_capture_patch_request`
- **Purpose**: Verifies PATCH requests are captured correctly
- **Test Steps**: Sends PATCH request with JSON body
- **Expected**: Request captured with method "PATCH"

#### `test_capture_delete_request`
- **Purpose**: Verifies DELETE requests are captured correctly
- **Test Steps**: Sends DELETE request
- **Expected**: Request captured with method "DELETE"

#### `test_capture_head_request`
- **Purpose**: Verifies HEAD requests are captured correctly
- **Test Steps**: Sends HEAD request
- **Expected**: Request captured with method "HEAD"
- **Notes**: HEAD requests typically have no body

#### `test_capture_options_request`
- **Purpose**: Verifies OPTIONS requests are captured correctly
- **Test Steps**: Sends OPTIONS request
- **Expected**: Request captured with method "OPTIONS"
- **Notes**: Important for CORS preflight requests

### 3. Request Content Tests

#### `test_capture_request_with_form_data`
- **Purpose**: Verifies URL-encoded form data is captured
- **Test Steps**: Sends POST with application/x-www-form-urlencoded content
- **Expected**: Form data captured in body as string

#### `test_capture_request_with_custom_headers`
- **Purpose**: Ensures custom headers are captured
- **Test Steps**: Sends request with multiple custom headers
- **Expected**: All custom headers present in captured request

#### `test_capture_request_with_empty_body`
- **Purpose**: Verifies requests with no body are handled correctly
- **Test Steps**: Sends POST with empty body
- **Expected**: Request captured with empty string body

#### `test_capture_large_json_payload`
- **Purpose**: Tests handling of large payloads (within limits)
- **Test Steps**: Sends POST with JSON containing 1000 items
- **Expected**: Large payload captured successfully
- **Notes**: Tests that large payloads don't crash the system

#### `test_capture_request_with_special_characters_in_path`
- **Purpose**: Verifies URL-encoded paths are handled correctly
- **Test Steps**: Sends request with special characters in path
- **Expected**: Request captured with path preserved

### 4. Request Management Tests

#### `test_clear_bucket_requests`
- **Purpose**: Verifies requests can be cleared from a bucket
- **Test Steps**: Captures multiple requests, then clears them
- **Expected**: Bucket is empty after clearing

#### `test_clear_bucket_with_wrong_password`
- **Purpose**: Ensures authentication is required to clear requests
- **Test Steps**: Attempts to clear with wrong password
- **Expected**: 401 Unauthorized response

#### `test_get_requests_without_password`
- **Purpose**: Verifies password header is required
- **Test Steps**: Attempts to retrieve requests without password header
- **Expected**: 401 Unauthorized response

#### `test_capture_multiple_requests`
- **Purpose**: Verifies multiple requests are captured correctly
- **Test Steps**: Sends 5 requests to the same bucket
- **Expected**: All 5 requests are captured and retrievable

#### `test_concurrent_requests_to_same_bucket`
- **Purpose**: Tests thread-safety of concurrent requests
- **Test Steps**: Sends 10 requests to the same bucket
- **Expected**: All 10 requests captured without data loss

### 5. Error Handling Tests

#### `test_capture_request_to_nonexistent_bucket`
- **Purpose**: Verifies proper error when bucket doesn't exist
- **Test Steps**: Sends request to non-existent bucket
- **Expected**: 404 Not Found response

### 6. Metadata Tests

#### `test_request_timestamp_is_set`
- **Purpose**: Ensures timestamps are captured for each request
- **Test Steps**: Captures a request and checks timestamp
- **Expected**: Timestamp is present and > 0

## Test Structure

Each test follows a consistent pattern:

1. **Setup**: Initialize test app and create necessary buckets
2. **Action**: Perform the operation being tested
3. **Verification**: Assert expected outcomes
4. **Cleanup**: Tests are isolated, no manual cleanup needed

## Test Data

- **Default Password**: `test_password_123`
- **Password Header**: `X-Bucket-Password`
- **Max Payload Size**: 10MB
- **Reserved Bucket Names**: `api`, `ui`
- **Max Bucket Name Length**: 100 characters
- **Valid Bucket Name Characters**: Alphanumeric, hyphens (-), underscores (_)

## Coverage

The test suite covers:

- ✅ All HTTP methods (GET, POST, PUT, PATCH, DELETE, HEAD, OPTIONS)
- ✅ Bucket lifecycle (create, list, delete)
- ✅ Bucket name validation (reserved names, special characters, length limits)
- ✅ Request capture and retrieval
- ✅ Authentication and authorization
- ✅ Query parameter parsing
- ✅ Header capture
- ✅ JSON body handling
- ✅ Form data handling
- ✅ Large payloads
- ✅ Empty bodies
- ✅ Special characters in URLs
- ✅ Concurrent requests
- ✅ Error conditions
- ✅ Timestamp metadata
- ✅ Route protection (preventing bucket name conflicts)

## Adding New Tests

When adding new tests:

1. Follow the naming convention: `test_<action>_<scenario>`
2. Use descriptive test names that explain what's being tested
3. Include comments explaining the purpose and expected behavior
4. Ensure tests are isolated and don't depend on other tests
5. Test both success and failure scenarios
6. Update this README with the new test description

## CI/CD Integration

These tests should be run:
- Before every commit (via git hooks)
- In CI/CD pipeline before deployment
- After any changes to request handling logic
- When dependencies are updated

## Security Features Tested

- ✅ **Route Protection**: Reserved names (api, ui) cannot be used as bucket names
- ✅ **Input Validation**: Bucket names validated for length and character restrictions
- ✅ **Authentication**: Password protection for accessing bucket data
- ✅ **Authorization**: Password verification for destructive operations

## Known Limitations

- Tests use in-memory storage; persistent storage scenarios aren't tested
- Network-level issues (timeouts, disconnections) aren't simulated
- Binary file uploads are tested but not exhaustively
- Very large payloads (>10MB) that exceed limits aren't tested
- Case-sensitivity of bucket names on different platforms not explicitly tested