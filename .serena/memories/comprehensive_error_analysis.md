# Comprehensive Error Analysis - All Error Images

## Error Patterns from Images

### API-error.png (Original)
- Connection errors with exponential backoff: 1s, 1s, 2s, 4s, 10s, 16s, 38s
- "API Error (Connection error.)" + "TypeError (fetch failed)"
- Classification: status 0 → "connection_error" ✓

### CC-ErrorCode-0.png (Additional HTTP Errors)
- 401 authentication_error ✓ (matches spec)
- 403 permission_error ✓ (matches spec)  
- 429 rate_limit_error ✓ (matches spec)
- 500 api_error ✓ (matches spec)
- **502 Bad Gateway** ❌ (MISSING from current classification)
- 504 socket hang up ✓ (matches spec)

### CC-ErrorCode-1.png (Additional Connection/Policy Errors)
- **Usage Policy violation** - API-level error (likely 400-level)
- **"Request timed out"** - timeout-specific connection issue
- **SSL certificate errors** ("unknown certificate verification error") - connection-level
- **"UNKNOWN_CERTIFICATE_VERIFICATION_ERROR"** - SSL/TLS handshake failures

## Missing Classifications Identified

### 1. HTTP Status 502 (Bad Gateway)
- Currently not handled in classify_http_status()
- Should map to "server_error" or specific "bad_gateway_error"
- Upstream service failures

### 2. Request Timeout Errors
- "Request timed out" messages
- Should be classified as "connection_error" or "timeout_error"
- Network-level timeouts vs API timeouts

### 3. SSL/Certificate Errors  
- Certificate verification failures
- TLS handshake errors
- Should be classified as "connection_error"

### 4. Usage Policy Violations
- API content policy violations
- Likely maps to 400 "invalid_request_error"
- Client-side request issues

## Updated Error Classification Requirements

```rust
// HTTP Status Mappings (add missing 502)
502 → "server_error" or "bad_gateway_error" (NEW)

// Connection Error Message Patterns (expand coverage)
"request timed out" → "connection_error"
"certificate verification" → "connection_error"  
"tls" / "ssl" errors → "connection_error"
"usage policy" → "invalid_request_error" (if no HTTP status)
```

## Impact on Enhancement Plan
- Add 502 Bad Gateway handling
- Expand connection error classification patterns
- Handle policy violations properly
- Test all new error patterns end-to-end