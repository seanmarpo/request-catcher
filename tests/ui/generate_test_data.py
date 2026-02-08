#!/usr/bin/env python3
"""
UI Test Data Generator for Request Catcher

This script creates a test bucket and sends various types of HTTP requests
to help test the UI display functionality.

Usage:
    # Run with defaults
    ./generate_test_data.py

    # Custom server URL and bucket name
    ./generate_test_data.py --url http://localhost:8080 --bucket my-test

    # Custom password
    ./generate_test_data.py --password secret123
"""

import argparse
import base64
import json
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from typing import Dict, Optional


class Colors:
    """ANSI color codes for terminal output"""

    RED = "\033[0;31m"
    GREEN = "\033[0;32m"
    YELLOW = "\033[1;33m"
    BLUE = "\033[0;34m"
    CYAN = "\033[0;36m"
    MAGENTA = "\033[0;35m"
    NC = "\033[0m"  # No Color


class RequestCatcherTester:
    """Comprehensive test data generator for Request Catcher UI"""

    def __init__(self, base_url: str, bucket_name: str, password: str):
        self.base_url = base_url.rstrip("/")
        self.bucket_name = bucket_name
        self.password = password
        self.request_count = 0

    def print_step(self, message: str):
        """Print a step message"""
        print(f"{Colors.BLUE}==>{Colors.NC} {message}")

    def print_success(self, message: str):
        """Print a success message"""
        print(f"{Colors.GREEN}✓{Colors.NC} {message}")

    def print_error(self, message: str):
        """Print an error message"""
        print(f"{Colors.RED}✗{Colors.NC} {message}")

    def print_info(self, message: str):
        """Print an info message"""
        print(f"{Colors.CYAN}ℹ{Colors.NC} {message}")

    def check_server(self) -> bool:
        """Check if the server is running"""
        self.print_step("Checking if server is running...")
        try:
            req = urllib.request.Request(f"{self.base_url}/api/buckets")
            with urllib.request.urlopen(req, timeout=5) as response:
                if response.status == 200:
                    self.print_success(f"Server is running at {self.base_url}")
                    return True
                return False
        except Exception:
            self.print_error(f"Server is not running at {self.base_url}")
            self.print_info("Please start the server with: cargo run")
            return False

    def create_bucket(self) -> bool:
        """Create the test bucket"""
        self.print_step(f"Creating test bucket '{self.bucket_name}'...")

        data = json.dumps({"password": self.password}).encode("utf-8")
        req = urllib.request.Request(
            f"{self.base_url}/api/create/{self.bucket_name}",
            data=data,
            headers={"Content-Type": "application/json"},
            method="POST",
        )

        try:
            with urllib.request.urlopen(req) as response:
                status = response.status
                if status == 200:
                    self.print_success(f"Bucket created (status: {status})")
                    return True
        except urllib.error.HTTPError as e:
            if e.code == 409:
                self.print_success(f"Bucket already exists (status: {e.code})")
                return True
            else:
                self.print_error(f"Failed to create bucket (status: {e.code})")
                return False
        except Exception as e:
            self.print_error(f"Failed to create bucket: {str(e)}")
            return False
        return False

    def send_request(
        self,
        path: str,
        method: str = "GET",
        headers: Optional[Dict[str, str]] = None,
        body: Optional[bytes] = None,
        description: str = "",
    ) -> bool:
        """Send a test request"""
        self.request_count += 1
        if description:
            self.print_info(f"{self.request_count}. {description}")

        url = f"{self.base_url}/{self.bucket_name}{path}"

        # Prepare headers
        req_headers = headers or {}

        try:
            req = urllib.request.Request(
                url, data=body, headers=req_headers, method=method
            )
            with urllib.request.urlopen(req, timeout=10):
                self.print_success(f"{method} request sent")
                return True
        except Exception:
            # Still consider it success since the request was captured
            self.print_success(f"{method} request sent")
            return True

    def send_all_tests(self):
        """Send all test requests"""
        self.print_step("Sending test requests to demonstrate UI features...")
        print()

        # 1. Simple GET request with query parameters
        self.send_request(
            "/api/users?id=123&name=John&active=true",
            method="GET",
            headers={
                "User-Agent": "TestClient/1.0",
                "Authorization": "Bearer token123",
            },
            description="Sending GET request with query parameters...",
        )
        time.sleep(0.5)

        # 2. POST request with JSON body
        self.send_request(
            "/api/users",
            method="POST",
            headers={"Content-Type": "application/json", "X-Request-ID": "req-001"},
            body=json.dumps(
                {
                    "username": "john_doe",
                    "email": "john@example.com",
                    "age": 30,
                    "active": True,
                    "roles": ["user", "admin"],
                    "metadata": {"created": "2024-01-01", "lastLogin": "2024-01-15"},
                }
            ).encode("utf-8"),
            description="Sending POST request with JSON body...",
        )
        time.sleep(0.5)

        # 3. PUT request with nested JSON
        self.send_request(
            "/api/users/123",
            method="PUT",
            headers={"Content-Type": "application/json"},
            body=json.dumps(
                {
                    "profile": {
                        "firstName": "John",
                        "lastName": "Doe",
                        "address": {
                            "street": "123 Main St",
                            "city": "San Francisco",
                            "state": "CA",
                            "zip": "94102",
                        },
                    },
                    "preferences": {"theme": "dark", "notifications": True},
                }
            ).encode("utf-8"),
            description="Sending PUT request with nested JSON...",
        )
        time.sleep(0.5)

        # 4. PATCH request
        self.send_request(
            "/api/users/123",
            method="PATCH",
            headers={"Content-Type": "application/json"},
            body=json.dumps({"email": "newemail@example.com"}).encode("utf-8"),
            description="Sending PATCH request...",
        )
        time.sleep(0.5)

        # 5. DELETE request
        self.send_request(
            "/api/users/123?reason=inactive",
            method="DELETE",
            headers={"X-Admin-Key": "admin123"},
            description="Sending DELETE request...",
        )
        time.sleep(0.5)

        # 6. HEAD request
        self.send_request(
            "/api/status", method="HEAD", description="Sending HEAD request..."
        )
        time.sleep(0.5)

        # 7. OPTIONS request (CORS)
        self.send_request(
            "/api/users",
            method="OPTIONS",
            headers={
                "Origin": "https://example.com",
                "Access-Control-Request-Method": "POST",
                "Access-Control-Request-Headers": "Content-Type",
            },
            description="Sending OPTIONS request (CORS preflight)...",
        )
        time.sleep(0.5)

        # 8. POST with form data
        self.send_request(
            "/login",
            method="POST",
            headers={"Content-Type": "application/x-www-form-urlencoded"},
            body=b"username=john&password=secret123&remember=on",
            description="Sending POST with form data...",
        )
        time.sleep(0.5)

        # 9. POST with plain text
        self.send_request(
            "/api/notes",
            method="POST",
            headers={"Content-Type": "text/plain"},
            body=b"This is a plain text note without any JSON formatting.",
            description="Sending POST with plain text...",
        )
        time.sleep(0.5)

        # 10. POST with JSON array
        self.send_request(
            "/api/bulk-create",
            method="POST",
            headers={"Content-Type": "application/json"},
            body=json.dumps(
                {
                    "items": [
                        {"id": 1, "name": "Item One"},
                        {"id": 2, "name": "Item Two"},
                        {"id": 3, "name": "Item Three"},
                    ]
                }
            ).encode("utf-8"),
            description="Sending POST with JSON array...",
        )
        time.sleep(0.5)

        # 11. GET with many query parameters
        self.send_request(
            "/api/search?q=test&page=1&limit=10&sort=name&order=asc&filter=active&category=electronics",
            method="GET",
            headers={"Accept": "application/json"},
            description="Sending GET with many query parameters...",
        )
        time.sleep(0.5)

        # 12. POST with large JSON
        self.send_request(
            "/api/analytics",
            method="POST",
            headers={"Content-Type": "application/json"},
            body=json.dumps(
                {
                    "timestamp": "2024-01-15T10:30:00Z",
                    "events": [
                        {"type": "click", "element": "button", "x": 100, "y": 200},
                        {"type": "scroll", "position": 500},
                        {"type": "click", "element": "link", "x": 300, "y": 400},
                        {"type": "input", "field": "search", "value": "test query"},
                        {"type": "click", "element": "submit", "x": 150, "y": 250},
                    ],
                    "session": {
                        "id": "sess_abc123",
                        "duration": 3600,
                        "pages": ["/home", "/products", "/checkout"],
                        "userAgent": "Mozilla/5.0 (compatible)",
                    },
                }
            ).encode("utf-8"),
            description="Sending POST with large JSON body...",
        )
        time.sleep(0.5)

        # 13. POST with empty body
        self.send_request(
            "/api/ping", method="POST", description="Sending POST with empty body..."
        )
        time.sleep(0.5)

        # 14. GET with URL-encoded path
        self.send_request(
            "/api/files/my%20document.pdf?version=2",
            method="GET",
            description="Sending GET with URL-encoded path...",
        )
        time.sleep(0.5)

        # 15. POST with various JSON types
        self.send_request(
            "/api/config",
            method="POST",
            headers={"Content-Type": "application/json"},
            body=json.dumps(
                {
                    "enabled": True,
                    "disabled": False,
                    "count": 42,
                    "ratio": 3.14,
                    "nullValue": None,
                    "emptyString": "",
                    "nested": {"value": 100, "flag": True},
                }
            ).encode("utf-8"),
            description="Sending POST with various JSON types...",
        )
        time.sleep(0.5)

        # 16. POST with custom headers
        self.send_request(
            "/api/webhook",
            method="POST",
            headers={
                "Content-Type": "application/json",
                "X-Webhook-ID": "hook_123",
                "X-Signature": "sha256=abc123def456",
                "X-Event-Type": "user.created",
                "X-Timestamp": "1642252800",
            },
            body=json.dumps({"event": "user.created", "userId": 456}).encode("utf-8"),
            description="Sending POST with custom headers...",
        )
        time.sleep(0.5)

        # 17. GET webhook callback
        self.send_request(
            "/webhook/callback?status=success&transaction_id=txn_789&amount=99.99",
            method="GET",
            headers={"X-Service-Name": "PaymentGateway"},
            description="Sending GET webhook callback...",
        )
        time.sleep(0.5)

        # 18. POST OAuth token
        self.send_request(
            "/oauth/token",
            method="POST",
            headers={"Content-Type": "application/x-www-form-urlencoded"},
            body=b"grant_type=authorization_code&code=AUTH123&redirect_uri=https://example.com/callback",
            description="Sending POST OAuth token request...",
        )
        time.sleep(0.5)

        # 19. PUT with XML-like content
        self.send_request(
            "/api/document",
            method="PUT",
            headers={"Content-Type": "application/xml"},
            body=b"<note><to>User</to><from>Admin</from><body>Hello World</body></note>",
            description="Sending PUT with XML content...",
        )
        time.sleep(0.5)

        # 20. POST error simulation
        self.send_request(
            "/api/errors",
            method="POST",
            headers={"Content-Type": "application/json"},
            body=json.dumps(
                {
                    "error": {
                        "code": 500,
                        "message": "Internal Server Error",
                        "details": "Something went wrong",
                        "timestamp": "2024-01-15T10:30:00Z",
                    }
                }
            ).encode("utf-8"),
            description="Sending POST simulating an error...",
        )
        time.sleep(0.5)

        # 21. Multipart form data with file upload simulation
        boundary = "----WebKitFormBoundary7MA4YWxkTrZu0gW"
        multipart_body = (
            f"--{boundary}\r\n"
            f'Content-Disposition: form-data; name="title"\r\n\r\n'
            f"My Document\r\n"
            f"--{boundary}\r\n"
            f'Content-Disposition: form-data; name="description"\r\n\r\n'
            f"This is a test document upload\r\n"
            f"--{boundary}\r\n"
            f'Content-Disposition: form-data; name="file"; filename="document.pdf"\r\n'
            f"Content-Type: application/pdf\r\n\r\n"
            f"%PDF-1.4 [Binary content would be here]\r\n"
            f"--{boundary}--\r\n"
        ).encode("utf-8")

        self.send_request(
            "/api/upload",
            method="POST",
            headers={
                "Content-Type": f"multipart/form-data; boundary={boundary}",
                "X-Upload-ID": "upload_001",
            },
            body=multipart_body,
            description="Sending multipart/form-data file upload (PDF simulation)...",
        )
        time.sleep(0.5)

        # 22. Image upload simulation
        boundary2 = "----WebKitFormBoundary9XY5ZWxkTrZu1hW"
        image_multipart = (
            f"--{boundary2}\r\n"
            f'Content-Disposition: form-data; name="image"; filename="photo.jpg"\r\n'
            f"Content-Type: image/jpeg\r\n\r\n"
            f"[JPEG binary data would be here - ÿØÿà JFIF header]\r\n"
            f"--{boundary2}\r\n"
            f'Content-Disposition: form-data; name="caption"\r\n\r\n'
            f"Beautiful sunset photo\r\n"
            f"--{boundary2}--\r\n"
        ).encode("utf-8")

        self.send_request(
            "/api/photos",
            method="POST",
            headers={
                "Content-Type": f"multipart/form-data; boundary={boundary2}",
                "X-Image-Type": "jpeg",
            },
            body=image_multipart,
            description="Sending image upload (JPEG simulation)...",
        )
        time.sleep(0.5)

        # 23. Base64 encoded binary data
        binary_data = b"This is binary data: \x00\x01\x02\x03\xff\xfe\xfd"
        base64_encoded = base64.b64encode(binary_data).decode("ascii")

        self.send_request(
            "/api/binary-upload",
            method="POST",
            headers={
                "Content-Type": "application/octet-stream",
                "Content-Transfer-Encoding": "base64",
            },
            body=base64_encoded.encode("utf-8"),
            description="Sending base64-encoded binary data...",
        )
        time.sleep(0.5)

        # 24. Multiple file upload
        boundary3 = "----WebKitFormBoundaryABC123"
        multi_file_upload = (
            f"--{boundary3}\r\n"
            f'Content-Disposition: form-data; name="files[]"; filename="file1.txt"\r\n'
            f"Content-Type: text/plain\r\n\r\n"
            f"Content of file 1\r\n"
            f"--{boundary3}\r\n"
            f'Content-Disposition: form-data; name="files[]"; filename="file2.txt"\r\n'
            f"Content-Type: text/plain\r\n\r\n"
            f"Content of file 2\r\n"
            f"--{boundary3}\r\n"
            f'Content-Disposition: form-data; name="files[]"; filename="file3.txt"\r\n'
            f"Content-Type: text/plain\r\n\r\n"
            f"Content of file 3\r\n"
            f"--{boundary3}--\r\n"
        ).encode("utf-8")

        self.send_request(
            "/api/bulk-upload",
            method="POST",
            headers={"Content-Type": f"multipart/form-data; boundary={boundary3}"},
            body=multi_file_upload,
            description="Sending multiple file upload...",
        )
        time.sleep(0.5)

        # 25. CSV file upload
        csv_content = (
            "id,name,email,age\n"
            "1,John Doe,john@example.com,30\n"
            "2,Jane Smith,jane@example.com,25\n"
            "3,Bob Johnson,bob@example.com,35\n"
        )

        self.send_request(
            "/api/import/csv",
            method="POST",
            headers={
                "Content-Type": "text/csv",
                "Content-Disposition": 'attachment; filename="users.csv"',
            },
            body=csv_content.encode("utf-8"),
            description="Sending CSV file upload...",
        )
        time.sleep(0.5)

        # 26. Very large JSON payload (stress test)
        large_items = [
            {
                "id": i,
                "name": f"Item {i}",
                "description": f"This is a detailed description for item {i}" * 10,
                "tags": [f"tag{j}" for j in range(20)],
                "metadata": {
                    "created": "2024-01-01T00:00:00Z",
                    "updated": "2024-01-15T10:30:00Z",
                    "views": i * 100,
                },
            }
            for i in range(50)
        ]

        self.send_request(
            "/api/bulk-data",
            method="POST",
            headers={"Content-Type": "application/json"},
            body=json.dumps({"items": large_items}).encode("utf-8"),
            description="Sending very large JSON payload (50 items with nested data)...",
        )
        time.sleep(0.5)

        # 27. Request with extremely long URL (edge case)
        long_query = "&".join([f"param{i}=value{i}" for i in range(50)])
        self.send_request(
            f"/api/search?{long_query}",
            method="GET",
            headers={"Accept": "application/json"},
            description="Sending GET with extremely long query string (50 parameters)...",
        )
        time.sleep(0.5)

        # 28. Request with special characters in body
        special_chars_body = {
            "text": "Special chars: émojis 🎉🚀💻, quotes \"'`, newlines\n\ntabs\t\there",
            "unicode": "Unicode: 日本語, العربية, हिन्दी, Ελληνικά",
            "symbols": "Symbols: ©®™€£¥§¶†‡",
            "math": "Math: ∑∫∂√∞≠≤≥±×÷",
        }

        self.send_request(
            "/api/special-chars",
            method="POST",
            headers={"Content-Type": "application/json; charset=utf-8"},
            body=json.dumps(special_chars_body, ensure_ascii=False).encode("utf-8"),
            description="Sending POST with special characters and Unicode...",
        )
        time.sleep(0.5)

        # 29. GraphQL query
        graphql_query = {
            "query": """
                query GetUser($id: ID!) {
                    user(id: $id) {
                        id
                        name
                        email
                        posts {
                            title
                            content
                            comments {
                                author
                                text
                            }
                        }
                    }
                }
            """,
            "variables": {"id": "123"},
            "operationName": "GetUser",
        }

        self.send_request(
            "/graphql",
            method="POST",
            headers={"Content-Type": "application/json"},
            body=json.dumps(graphql_query).encode("utf-8"),
            description="Sending GraphQL query...",
        )
        time.sleep(0.5)

        # 30. SOAP-like XML request
        soap_body = """<?xml version="1.0" encoding="utf-8"?>
<soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/">
    <soap:Header>
        <Authentication>
            <Username>admin</Username>
            <Password>secret123</Password>
        </Authentication>
    </soap:Header>
    <soap:Body>
        <GetUserRequest xmlns="http://example.com/api">
            <UserId>12345</UserId>
            <IncludeProfile>true</IncludeProfile>
        </GetUserRequest>
    </soap:Body>
</soap:Envelope>"""

        self.send_request(
            "/api/soap",
            method="POST",
            headers={
                "Content-Type": "text/xml; charset=utf-8",
                "SOAPAction": "http://example.com/api/GetUser",
            },
            body=soap_body.encode("utf-8"),
            description="Sending SOAP XML request...",
        )
        time.sleep(0.5)

        # 31. Webhook with signature verification
        webhook_payload = {
            "event": "payment.completed",
            "data": {
                "transaction_id": "txn_abc123",
                "amount": 99.99,
                "currency": "USD",
                "customer": {"id": "cust_456", "email": "customer@example.com"},
            },
            "timestamp": 1642252800,
        }

        self.send_request(
            "/webhooks/payment",
            method="POST",
            headers={
                "Content-Type": "application/json",
                "X-Webhook-Signature": "sha256=1234567890abcdef1234567890abcdef12345678",
                "X-Webhook-ID": "wh_xyz789",
                "X-Webhook-Timestamp": "1642252800",
            },
            body=json.dumps(webhook_payload).encode("utf-8"),
            description="Sending webhook with signature headers...",
        )
        time.sleep(0.5)

        # 32. Request with cookie headers
        self.send_request(
            "/api/authenticated",
            method="GET",
            headers={
                "Cookie": "session_id=abc123; user_token=xyz789; preferences=theme:dark,lang:en",
                "Authorization": "Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9",
            },
            description="Sending GET with cookies and auth token...",
        )
        time.sleep(0.5)

        # 33. CORS preflight with multiple headers
        self.send_request(
            "/api/secure-endpoint",
            method="OPTIONS",
            headers={
                "Origin": "https://app.example.com",
                "Access-Control-Request-Method": "POST",
                "Access-Control-Request-Headers": "Content-Type, Authorization, X-Custom-Header",
                "Sec-Fetch-Mode": "cors",
                "Sec-Fetch-Site": "cross-site",
            },
            description="Sending complex CORS preflight request...",
        )
        time.sleep(0.5)

        # 34. Chunked encoding simulation
        self.send_request(
            "/api/stream",
            method="POST",
            headers={
                "Content-Type": "text/plain",
                "Transfer-Encoding": "chunked",
            },
            body=b"Chunk 1 of data\nChunk 2 of data\nChunk 3 of data",
            description="Sending request with chunked transfer encoding...",
        )
        time.sleep(0.5)

        # 35. Request with custom content types
        protobuf_like = b"\x08\x96\x01\x12\x04John\x1a\x10john@example.com"
        self.send_request(
            "/api/protobuf",
            method="POST",
            headers={
                "Content-Type": "application/x-protobuf",
                "X-Proto-Version": "3",
            },
            body=protobuf_like,
            description="Sending request with custom content type (protobuf-like)...",
        )
        time.sleep(0.5)

        # 36. Request with many custom headers
        many_headers = {
            "Content-Type": "application/json",
            "X-Request-ID": "req-" + "a" * 50,
            "X-Correlation-ID": "corr-123",
            "X-Client-Version": "1.2.3",
            "X-Platform": "iOS",
            "X-Device-ID": "device-456",
            "X-App-Version": "2.4.1",
            "X-Build-Number": "1234",
            "X-API-Key": "key_" + "x" * 40,
            "X-Session-ID": "sess_" + "y" * 40,
            "X-User-Agent": "MyApp/1.0 (iPhone; iOS 15.0)",
            "X-Timezone": "America/New_York",
            "X-Locale": "en-US",
            "X-Feature-Flags": "flag1,flag2,flag3",
        }

        self.send_request(
            "/api/telemetry",
            method="POST",
            headers=many_headers,
            body=json.dumps({"event": "app_started"}).encode("utf-8"),
            description="Sending request with many custom headers (14 headers)...",
        )
        time.sleep(0.5)

        # 37. Empty body with POST
        self.send_request(
            "/api/empty",
            method="POST",
            headers={"Content-Type": "application/json", "Content-Length": "0"},
            body=b"",
            description="Sending POST with explicit empty body...",
        )
        time.sleep(0.5)

        # 38. Nested arrays and objects (complex JSON)
        complex_json = {
            "users": [
                {
                    "id": 1,
                    "profile": {
                        "name": "John",
                        "contacts": [
                            {"type": "email", "value": "john@example.com"},
                            {"type": "phone", "value": "+1234567890"},
                        ],
                        "address": {
                            "street": {"number": 123, "name": "Main St"},
                            "city": "San Francisco",
                            "coordinates": {"lat": 37.7749, "lng": -122.4194},
                        },
                    },
                    "permissions": {
                        "read": True,
                        "write": False,
                        "admin": False,
                        "roles": ["user", "viewer"],
                    },
                },
            ],
            "metadata": {"version": "1.0", "timestamp": None, "count": 1},
        }

        self.send_request(
            "/api/complex-structure",
            method="POST",
            headers={"Content-Type": "application/json"},
            body=json.dumps(complex_json).encode("utf-8"),
            description="Sending deeply nested JSON structure...",
        )
        time.sleep(0.5)

        # 39. Request with URL-encoded special characters in path
        encoded_path = "/api/files/" + urllib.parse.quote(
            "my file (v2) [final].pdf", safe=""
        )
        self.send_request(
            encoded_path,
            method="GET",
            headers={"Accept": "application/pdf"},
            description="Sending GET with special characters in path...",
        )
        time.sleep(0.5)

        # 40. Rate limit test - rapid requests
        self.print_info("40. Sending rapid sequence of requests (rate limit test)...")
        for i in range(5):
            self.send_request(
                f"/api/ping?seq={i}",
                method="GET",
                headers={"X-Sequence": str(i)},
                body=None,
                description="",  # Suppress individual messages
            )
            time.sleep(0.1)
        self.print_success("Rapid requests sent (5 requests)")
        time.sleep(0.5)

        # 41. Request with malformed JSON
        malformed_json = '{"key": "value", "broken": }'
        self.send_request(
            "/api/malformed",
            method="POST",
            headers={"Content-Type": "application/json"},
            body=malformed_json.encode("utf-8"),
            description="Sending POST with malformed JSON...",
        )
        time.sleep(0.5)

        # 42. Request with very long single line
        long_line = {"data": "x" * 1000}
        self.send_request(
            "/api/long-line",
            method="POST",
            headers={"Content-Type": "application/json"},
            body=json.dumps(long_line).encode("utf-8"),
            description="Sending POST with very long single line (1000 chars)...",
        )
        time.sleep(0.5)

        # 43. Request simulating SSE
        self.send_request(
            "/api/events/subscribe",
            method="GET",
            headers={
                "Accept": "text/event-stream",
                "Cache-Control": "no-cache",
                "Connection": "keep-alive",
            },
            description="Sending SSE subscription request...",
        )
        time.sleep(0.5)

        # 44. Request with gzip content encoding header
        compressed_data = b"This would be gzip compressed data in real scenario"
        self.send_request(
            "/api/compressed",
            method="POST",
            headers={
                "Content-Type": "application/json",
                "Content-Encoding": "gzip",
            },
            body=compressed_data,
            description="Sending request with content-encoding header (gzip)...",
        )
        time.sleep(0.5)

        # 45. TRACE method (rarely used HTTP method)
        self.send_request(
            "/api/trace",
            method="TRACE",
            headers={"Max-Forwards": "10"},
            description="Sending TRACE request (rare HTTP method)...",
        )
        time.sleep(0.5)

        # 46. CONNECT method (tunneling/proxy method)
        self.send_request(
            "/api/proxy",
            method="CONNECT",
            headers={"Host": "example.com:443"},
            description="Sending CONNECT request (tunneling method)...",
        )
        time.sleep(0.5)

        # 47. WebSocket upgrade request
        self.send_request(
            "/chat",
            method="GET",
            headers={
                "Upgrade": "websocket",
                "Connection": "Upgrade",
                "Sec-WebSocket-Key": "dGhlIHNhbXBsZSBub25jZQ==",
                "Sec-WebSocket-Version": "13",
                "Sec-WebSocket-Protocol": "chat, superchat",
            },
            description="Sending WebSocket upgrade request...",
        )
        time.sleep(0.5)

        # 48. HTTP Basic Authentication
        basic_auth = base64.b64encode(b"username:password").decode("ascii")
        self.send_request(
            "/api/auth/basic",
            method="GET",
            headers={
                "Authorization": f"Basic {basic_auth}",
                "User-Agent": "TestClient/1.0",
            },
            description="Sending GET with HTTP Basic Authentication...",
        )
        time.sleep(0.5)

        # 49. HTTP Digest Authentication header
        self.send_request(
            "/api/auth/digest",
            method="GET",
            headers={
                "Authorization": 'Digest username="admin", realm="test@example.com", '
                'nonce="dcd98b7102dd2f0e8b11d0f600bfb0c093", uri="/api/auth/digest", '
                'response="6629fae49393a05397450978507c4ef1"',
            },
            description="Sending GET with HTTP Digest Authentication...",
        )
        time.sleep(0.5)

        # 50. Conditional request with If-Modified-Since
        self.send_request(
            "/api/resource/123",
            method="GET",
            headers={
                "If-Modified-Since": "Wed, 21 Oct 2015 07:28:00 GMT",
                "User-Agent": "TestClient/1.0",
            },
            description="Sending conditional GET with If-Modified-Since...",
        )
        time.sleep(0.5)

        # 51. Conditional request with If-None-Match (ETag)
        self.send_request(
            "/api/resource/456",
            method="GET",
            headers={
                "If-None-Match": '"33a64df551425fcc55e4d42a148795d9f25f89d4"',
                "Accept": "application/json",
            },
            description="Sending conditional GET with If-None-Match (ETag)...",
        )
        time.sleep(0.5)

        # 52. PUT with If-Match for optimistic locking
        self.send_request(
            "/api/resource/789",
            method="PUT",
            headers={
                "If-Match": '"686897696a7c876b7e"',
                "Content-Type": "application/json",
            },
            body=json.dumps({"status": "updated"}).encode("utf-8"),
            description="Sending PUT with If-Match (optimistic locking)...",
        )
        time.sleep(0.5)

        # 53. Range request for partial content
        self.send_request(
            "/api/files/large-file.bin",
            method="GET",
            headers={
                "Range": "bytes=0-1023",
                "Accept": "application/octet-stream",
            },
            description="Sending GET with Range header (partial content)...",
        )
        time.sleep(0.5)

        # 54. Multi-range request
        self.send_request(
            "/api/files/document.pdf",
            method="GET",
            headers={
                "Range": "bytes=0-499, 1000-1499, 2000-2499",
                "Accept": "application/pdf",
            },
            description="Sending GET with multi-range request...",
        )
        time.sleep(0.5)

        # 55. Query parameters with empty values
        self.send_request(
            "/api/search?keyword=&category=&page=1&limit=",
            method="GET",
            headers={"Accept": "application/json"},
            description="Sending GET with empty query parameter values...",
        )
        time.sleep(0.5)

        # 56. Query parameters without values
        self.send_request(
            "/api/filter?active&verified&premium",
            method="GET",
            headers={"Accept": "application/json"},
            description="Sending GET with query parameters without values...",
        )
        time.sleep(0.5)

        # 57. Array notation in query parameters
        self.send_request(
            "/api/items?ids[]=1&ids[]=2&ids[]=3&tags[]=urgent&tags[]=bug",
            method="GET",
            headers={"Accept": "application/json"},
            description="Sending GET with array notation in query params...",
        )
        time.sleep(0.5)

        # 58. Root path only
        self.send_request(
            "/",
            method="GET",
            headers={"Accept": "text/html"},
            description="Sending GET to root path only...",
        )
        time.sleep(0.5)

        # 59. Path with double slashes
        self.send_request(
            "/api//users//123",
            method="GET",
            headers={"Accept": "application/json"},
            description="Sending GET with double slashes in path...",
        )
        time.sleep(0.5)

        # 60. Path with dot segments
        self.send_request(
            "/api/../users/./123",
            method="GET",
            headers={"Accept": "application/json"},
            description="Sending GET with dot segments in path...",
        )
        time.sleep(0.5)

        # 61. Trailing slash variation
        self.send_request(
            "/api/users/",
            method="GET",
            headers={"Accept": "application/json"},
            description="Sending GET with trailing slash...",
        )
        time.sleep(0.5)

        # 62. Request with duplicate headers
        self.send_request(
            "/api/duplicates",
            method="GET",
            headers={
                "X-Custom-Header": "value1",
                "Accept": "application/json",
                "User-Agent": "TestClient/1.0",
            },
            description="Sending GET with headers (Note: Python urllib doesn't support true duplicate header names)...",
        )
        time.sleep(0.5)

        # 63. Request with very long header value
        long_header_value = "x" * 4096  # 4KB header value
        self.send_request(
            "/api/long-header",
            method="GET",
            headers={
                "X-Very-Long-Header": long_header_value,
                "Accept": "application/json",
            },
            description="Sending GET with very long header value (4KB)...",
        )
        time.sleep(0.5)

        # 64. Request with various Accept types - HTML
        self.send_request(
            "/",
            method="GET",
            headers={
                "Accept": "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
                "Accept-Language": "en-US,en;q=0.9",
            },
            description="Sending GET with Accept: text/html...",
        )
        time.sleep(0.5)

        # 65. Request with Accept: application/xml
        self.send_request(
            "/api/data.xml",
            method="GET",
            headers={
                "Accept": "application/xml, text/xml",
                "Accept-Charset": "utf-8, iso-8859-1;q=0.5",
            },
            description="Sending GET with Accept: application/xml...",
        )
        time.sleep(0.5)

        # 66. Request with Accept: */*
        self.send_request(
            "/api/resource",
            method="GET",
            headers={
                "Accept": "*/*",
                "User-Agent": "curl/7.68.0",
            },
            description="Sending GET with Accept: */*...",
        )
        time.sleep(0.5)

        # 67. Request with Accept: image/*
        self.send_request(
            "/api/avatar/123",
            method="GET",
            headers={
                "Accept": "image/*, image/webp, image/avif",
                "Cache-Control": "max-age=3600",
            },
            description="Sending GET with Accept: image/*...",
        )
        time.sleep(0.5)

        # 68. Content negotiation with Accept-Language
        self.send_request(
            "/api/content",
            method="GET",
            headers={
                "Accept": "application/json",
                "Accept-Language": "fr-FR,fr;q=0.9,en-US;q=0.8,en;q=0.7,de;q=0.6",
                "Accept-Encoding": "gzip, deflate, br",
            },
            description="Sending GET with complex Accept-Language negotiation...",
        )
        time.sleep(0.5)

        # 69. Request with comprehensive cache control headers
        self.send_request(
            "/api/cached-resource",
            method="GET",
            headers={
                "Cache-Control": "no-cache, no-store, must-revalidate, max-age=0",
                "Pragma": "no-cache",
                "Expires": "0",
                "If-Modified-Since": "Mon, 01 Jan 2024 00:00:00 GMT",
            },
            description="Sending GET with comprehensive cache control headers...",
        )
        time.sleep(0.5)

        # 70. Request with IPv6 in Host header
        self.send_request(
            "/api/ipv6-test",
            method="GET",
            headers={
                "Host": "[2001:db8::1]:8080",
                "Accept": "application/json",
            },
            description="Sending GET with IPv6 in Host header...",
        )
        time.sleep(0.5)

        # 71. Custom/invalid HTTP method
        self.send_request(
            "/api/custom",
            method="CUSTOM",
            headers={"X-Custom-Method": "true"},
            description="Sending CUSTOM method (non-standard HTTP method)...",
        )
        time.sleep(0.5)

        # 72. Request with no headers at all (minimal request)
        self.send_request(
            "/api/minimal",
            method="GET",
            headers={},
            description="Sending GET with no headers (minimal request)...",
        )
        time.sleep(0.5)

        # 73. Request with mixed case headers
        self.send_request(
            "/api/case-test",
            method="POST",
            headers={
                "content-type": "application/json",  # lowercase
                "Content-Length": "27",  # mixed case
                "ACCEPT": "application/json",  # uppercase
                "X-Custom-Header": "MixedCase",
            },
            body=json.dumps({"test": "case"}).encode("utf-8"),
            description="Sending POST with mixed case header names...",
        )
        time.sleep(0.5)

        # 74. Request with only query string (no path)
        self.send_request(
            "/?query=test&action=search",
            method="GET",
            headers={"Accept": "application/json"},
            description="Sending GET with only query string (no path)...",
        )
        time.sleep(0.5)

        # 75. Request simulating HTTP/2 pseudo-headers (as regular headers)
        self.send_request(
            "/api/http2-sim",
            method="POST",
            headers={
                "Content-Type": "application/json",
                "X-HTTP2-Method": "POST",
                "X-HTTP2-Path": "/api/http2-sim",
                "X-HTTP2-Scheme": "https",
                "X-HTTP2-Authority": "example.com",
            },
            body=json.dumps({"http2": "simulation"}).encode("utf-8"),
            description="Sending request simulating HTTP/2 pseudo-headers...",
        )
        time.sleep(0.5)

        # 76. Request with URL-encoded form data (nested parameters)
        self.send_request(
            "/api/form-nested",
            method="POST",
            headers={"Content-Type": "application/x-www-form-urlencoded"},
            body=b"user[name]=John&user[email]=john@example.com&user[prefs][theme]=dark&user[prefs][lang]=en",
            description="Sending POST with nested form parameters...",
        )
        time.sleep(0.5)

        # 77. Request with JSON-RPC format
        jsonrpc_request = {
            "jsonrpc": "2.0",
            "method": "sum",
            "params": [42, 23],
            "id": 1,
        }
        self.send_request(
            "/api/jsonrpc",
            method="POST",
            headers={"Content-Type": "application/json"},
            body=json.dumps(jsonrpc_request).encode("utf-8"),
            description="Sending JSON-RPC 2.0 request...",
        )
        time.sleep(0.5)

        # 78. Request with Server-Sent Events headers
        self.send_request(
            "/api/sse/updates",
            method="GET",
            headers={
                "Accept": "text/event-stream",
                "Cache-Control": "no-cache",
                "Connection": "keep-alive",
                "Last-Event-ID": "123",
            },
            description="Sending SSE request with Last-Event-ID...",
        )
        time.sleep(0.5)

        # 79. Request with custom port in Host header
        self.send_request(
            "/api/port-test",
            method="GET",
            headers={
                "Host": "example.com:8443",
                "Accept": "application/json",
            },
            description="Sending GET with custom port in Host header...",
        )
        time.sleep(0.5)

        # 80. Request with forwarded headers (proxy scenario)
        self.send_request(
            "/api/forwarded",
            method="GET",
            headers={
                "X-Forwarded-For": "203.0.113.195, 70.41.3.18, 150.172.238.178",
                "X-Forwarded-Proto": "https",
                "X-Forwarded-Host": "example.com",
                "X-Real-IP": "203.0.113.195",
                "Forwarded": "for=192.0.2.60;proto=https;by=203.0.113.43",
            },
            description="Sending GET with forwarded/proxy headers...",
        )
        time.sleep(0.5)

        # 81. Request with CORS headers (actual request after preflight)
        self.send_request(
            "/api/cors-actual",
            method="POST",
            headers={
                "Origin": "https://app.example.com",
                "Content-Type": "application/json",
                "X-Custom-Header": "custom-value",
            },
            body=json.dumps({"action": "create", "data": "test"}).encode("utf-8"),
            description="Sending actual CORS request (after preflight)...",
        )
        time.sleep(0.5)

        # 82. Request with security headers
        self.send_request(
            "/api/secure",
            method="GET",
            headers={
                "Strict-Transport-Security": "max-age=31536000; includeSubDomains",
                "X-Content-Type-Options": "nosniff",
                "X-Frame-Options": "DENY",
                "X-XSS-Protection": "1; mode=block",
                "Content-Security-Policy": "default-src 'self'",
            },
            description="Sending GET with security headers...",
        )
        time.sleep(0.5)

        # 83. Request with DNT and privacy headers
        self.send_request(
            "/api/privacy",
            method="GET",
            headers={
                "DNT": "1",
                "Sec-GPC": "1",
                "Sec-Fetch-Site": "same-origin",
                "Sec-Fetch-Mode": "navigate",
                "Sec-Fetch-User": "?1",
                "Sec-Fetch-Dest": "document",
            },
            description="Sending GET with privacy and security fetch headers...",
        )
        time.sleep(0.5)

        # 84. Request with Accept-Encoding variations
        self.send_request(
            "/api/encoding-test",
            method="GET",
            headers={
                "Accept-Encoding": "gzip, deflate, br, zstd",
                "Accept": "application/json",
            },
            description="Sending GET with multiple Accept-Encoding options...",
        )
        time.sleep(0.5)

        # 85. Request with User-Agent variations (mobile)
        self.send_request(
            "/api/mobile",
            method="GET",
            headers={
                "User-Agent": "Mozilla/5.0 (iPhone; CPU iPhone OS 15_0 like Mac OS X) AppleWebKit/605.1.15",
                "Accept": "text/html,application/xhtml+xml",
            },
            description="Sending GET with mobile User-Agent...",
        )
        time.sleep(0.5)

        # 86. Request with Referer header
        self.send_request(
            "/api/analytics/click",
            method="POST",
            headers={
                "Referer": "https://example.com/page?utm_source=google&utm_medium=cpc",
                "Content-Type": "application/json",
            },
            body=json.dumps({"element": "button", "action": "click"}).encode("utf-8"),
            description="Sending POST with Referer header...",
        )
        time.sleep(0.5)

        # 87. Request with multiple encoding in body
        self.send_request(
            "/api/encoding/latin1",
            method="POST",
            headers={
                "Content-Type": "text/plain; charset=ISO-8859-1",
            },
            body="Héllo Wörld with spëcial çharacters".encode("iso-8859-1"),
            description="Sending POST with ISO-8859-1 encoding...",
        )
        time.sleep(0.5)

        # 88. Request with UTF-16 encoding
        self.send_request(
            "/api/encoding/utf16",
            method="POST",
            headers={
                "Content-Type": "text/plain; charset=UTF-16",
            },
            body="UTF-16 encoded text: 你好世界".encode("utf-16"),
            description="Sending POST with UTF-16 encoding...",
        )
        time.sleep(0.5)

        # 89. Request with TE (Transfer-Encoding) header
        self.send_request(
            "/api/transfer",
            method="GET",
            headers={
                "TE": "trailers, deflate",
                "Accept": "application/json",
            },
            description="Sending GET with TE (Transfer-Encoding) header...",
        )
        time.sleep(0.5)

        # 90. Request with Expect: 100-continue
        self.send_request(
            "/api/large-upload",
            method="POST",
            headers={
                "Expect": "100-continue",
                "Content-Type": "application/octet-stream",
            },
            body=b"Large data payload that requires 100-continue",
            description="Sending POST with Expect: 100-continue header...",
        )

        print()

    def show_bucket_info(self):
        """Display bucket access information"""
        self.print_step("Test data generation complete!")
        print()
        self.print_info(f"Bucket Name: {self.bucket_name}")
        self.print_info(f"Password: {self.password}")
        print()
        self.print_success("Open the UI to view the captured requests:")
        url = f"{self.base_url}/ui/bucket.html?name={self.bucket_name}&password={self.password}"
        print(f"  {Colors.CYAN}{url}{Colors.NC}")
        print()
        self.print_info(f"Generated {self.request_count} test requests demonstrating:")
        print(
            "  • All HTTP methods (GET, POST, PUT, PATCH, DELETE, HEAD, OPTIONS, TRACE)"
        )
        print("  • JSON body formatting with syntax highlighting")
        print("  • Query parameters and headers")
        print("  • Form data and multipart uploads")
        print("  • File uploads (PDF, images, CSV, multiple files)")
        print("  • Binary data and base64 encoding")
        print("  • Plain text and XML content")
        print("  • Empty bodies")
        print("  • Custom and webhook headers")
        print("  • Large JSON payloads and stress tests")
        print("  • Very long URLs (50+ parameters)")
        print("  • Special characters and Unicode (emojis, international text)")
        print("  • GraphQL queries")
        print("  • SOAP/XML requests")
        print("  • Cookie and authentication headers")
        print("  • Complex CORS preflight")
        print("  • Chunked transfer encoding")
        print("  • Custom content types (protobuf)")
        print("  • Deeply nested JSON structures")
        print("  • URL-encoded paths")
        print("  • Rapid request sequences")
        print("  • Malformed JSON and edge cases")
        print("  • SSE subscription")
        print("  • Content encoding headers")
        print()

    def run(self):
        """Run the test data generation"""
        print()
        print("╔════════════════════════════════════════════════════════╗")
        print("║     Request Catcher - UI Test Data Generator          ║")
        print("╚════════════════════════════════════════════════════════╝")
        print()

        if not self.check_server():
            sys.exit(1)

        if not self.create_bucket():
            sys.exit(1)

        self.send_all_tests()
        self.show_bucket_info()


def main():
    """Main entry point"""
    parser = argparse.ArgumentParser(
        description="Generate comprehensive test data for Request Catcher UI testing",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  %(prog)s
  %(prog)s --url http://localhost:8080 --bucket my-test
  %(prog)s --bucket demo --password secret123
        """,
    )
    parser.add_argument(
        "--url",
        default="http://localhost:9090",
        help="Base URL of the Request Catcher server (default: http://localhost:9090)",
    )
    parser.add_argument(
        "--bucket",
        default="ui-test-bucket",
        help="Bucket name to create (default: ui-test-bucket)",
    )
    parser.add_argument(
        "--password", default="test123", help="Bucket password (default: test123)"
    )

    args = parser.parse_args()

    tester = RequestCatcherTester(args.url, args.bucket, args.password)
    tester.run()


if __name__ == "__main__":
    main()
