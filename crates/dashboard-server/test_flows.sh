#!/bin/bash
# Test script for Keycloak multi-tenant auth flows
# Prerequisites: docker compose up -d, cargo run

set -e

BASE_URL="http://localhost:3000"
KEYCLOAK_URL="http://localhost:8080"

echo "=== Waiting for services to be ready ==="
sleep 2

# Step 1: Super-admin login
echo ""
echo "=== Step 1: Super-admin Login ==="
LOGIN_RESPONSE=$(curl -s -X POST "$BASE_URL/auth/login" \
  -H "Content-Type: application/json" \
  -d '{"username": "superadmin", "password": "SuperSecret999!"}')

echo "Login Response: $LOGIN_RESPONSE"
SUPER_ADMIN_TOKEN=$(echo $LOGIN_RESPONSE | grep -o '"access_token":"[^"]*"' | cut -d'"' -f4)
echo "Super Admin Token: ${SUPER_ADMIN_TOKEN:0:50}..."

# Step 2: Create tenant
TENANT_ID="test-company-v7"
echo ""
echo "=== Step 2: Create Tenant ($TENANT_ID) ==="
TENANT_RESPONSE=$(curl -s -X POST "$BASE_URL/tenants" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $SUPER_ADMIN_TOKEN" \
  -d "{
    \"tenant_id\": \"$TENANT_ID\",
    \"company_name\": \"Test Company V7 Inc\",
    \"display_name\": \"Test Company V7\",
    \"admin_email\": \"admin@testcompany-v7.com\",
    \"app_url\": \"http://localhost:5173\"
  }")

echo "Tenant Response: $TENANT_RESPONSE"
TENANT_ID=$(echo $TENANT_RESPONSE | grep -o '"tenant_id":"[^"]*"' | cut -d'"' -f4)
echo "Tenant ID: $TENANT_ID"

# Step 3: Get Keycloak token for tenant admin
echo ""
echo "=== Step 3: Login as Tenant Admin via Keycloak ==="
echo "Note: Tenant admin credentials are username='admin', password='ChangeMe123!'"

KEYCLOAK_TOKEN_RESPONSE=$(curl -s -X POST "$KEYCLOAK_URL/realms/$TENANT_ID/protocol/openid-connect/token" \
  -H "Content-Type: application/x-www-form-urlencoded" \
  -d "grant_type=password" \
  -d "client_id=hyper-analytics" \
  -d "username=admin" \
  -d "password=ChangeMe123!")

echo "Keycloak Token Response: $KEYCLOAK_TOKEN_RESPONSE"

if echo "$KEYCLOAK_TOKEN_RESPONSE" | grep -q "error"; then
  echo "Keycloak login failed. Tenant may still be initializing. Waiting..."
  sleep 5
  KEYCLOAK_TOKEN_RESPONSE=$(curl -s -X POST "$KEYCLOAK_URL/realms/$TENANT_ID/protocol/openid-connect/token" \
    -H "Content-Type: application/x-www-form-urlencoded" \
    -d "grant_type=password" \
    -d "client_id=hyper-analytics" \
    -d "username=admin" \
    -d "password=ChangeMe123!")
  echo "Keycloak Token Response (retry): $KEYCLOAK_TOKEN_RESPONSE"
fi

TENANT_ADMIN_TOKEN=$(echo $KEYCLOAK_TOKEN_RESPONSE | grep -o '"access_token":"[^"]*"' | cut -d'"' -f4)
echo "Tenant Admin Token: ${TENANT_ADMIN_TOKEN:0:50}..."

# Step 4: Create user as tenant admin
echo ""
echo "=== Step 4: Create User as Tenant Admin ==="
USER_RESPONSE=$(curl -s -X POST "$BASE_URL/users" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $TENANT_ADMIN_TOKEN" \
  -d '{"username": "testuser", "email": "testuser@testcompany.com"}')

echo "User Response: $USER_RESPONSE"
USER_ID=$(echo $USER_RESPONSE | grep -o '"user_id":"[^"]*"' | cut -d'"' -f4)
echo "User ID: $USER_ID"

# Step 5: Create project as tenant admin
echo ""
echo "=== Step 5: Create Project as Tenant Admin ==="
PROJECT_RESPONSE=$(curl -s -X POST "$BASE_URL/projects" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $TENANT_ADMIN_TOKEN" \
  -d '{"name": "My First Project"}')

echo "Project Response: $PROJECT_RESPONSE"
PROJECT_ID=$(echo $PROJECT_RESPONSE | grep -o '"id":"[^"]*"' | cut -d'"' -f4)
PROJECT_TOKEN=$(echo $PROJECT_RESPONSE | grep -o '"project_token":"[^"]*"' | cut -d'"' -f4)
echo "Project ID: $PROJECT_ID"
echo "Project Token: $PROJECT_TOKEN"

# Step 6: List projects
echo ""
echo "=== Step 6: List Projects ==="
PROJECTS_LIST=$(curl -s -X GET "$BASE_URL/projects" \
  -H "Authorization: Bearer $TENANT_ADMIN_TOKEN")

echo "Projects: $PROJECTS_LIST"

# Step 7: Test ingestion with project token
echo ""
echo "=== Step 7: Test Ingestion with Project Token ==="
INGEST_RESPONSE=$(curl -s -X POST "$BASE_URL/push_batch" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $PROJECT_TOKEN" \
  -d '{
    "session": {
      "device_type": "desktop",
      "platform": "web",
      "browser": "chrome",
      "country": "US",
      "city": "San Francisco",
      "user_agent": "Mozilla/5.0"
    },
    "events": [
      {
        "event_id": "550e8400-e29b-41d4-a716-446655440000",
        "event_name": "page_view",
        "event_timestamp": 1709500800000,
        "user_id": "user123",
        "properties": "{\"page\": \"/home\"}"
      }
    ]
  }')

echo "Ingest Response: $INGEST_RESPONSE"

# Step 8: Get current user info
echo ""
echo "=== Step 8: Get Current User Info (/me) ==="
ME_RESPONSE=$(curl -s -X GET "$BASE_URL/me" \
  -H "Authorization: Bearer $TENANT_ADMIN_TOKEN")

echo "Current User: $ME_RESPONSE"

# Step 9: Test invalid token
echo ""
echo "=== Step 9: Test Invalid Token ==="
INVALID_RESPONSE=$(curl -s -X POST "$BASE_URL/push_batch" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer invalid-token" \
  -d '{
    "session": {"device_type": "desktop", "platform": "web", "browser": "chrome", "country": "US", "city": "SF", "user_agent": "test"},
    "events": []
  }')

echo "Invalid Token Response: $INVALID_RESPONSE"

echo ""
echo "=== All tests completed ==="
