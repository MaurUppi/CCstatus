# ErrorTracker Enhancement Implementation Plan

## Analysis of Current Issues

### Connection Error Pattern (from API-error.png)
- Shows "API Error (Connection error.)" with "TypeError (fetch failed)"  
- Retry pattern: 1s, 1s, 2s, 4s, 10s, 16s, 38s (exponential backoff)
- These are network-level failures (status code 0), not HTTP response errors
- Should classify as "connection_error" per current implementation

### Official Error Type Mappings (provided)
```
400 → invalid_request_error
401 → authentication_error  
403 → permission_error
404 → not_found_error
413 → request_too_large  
429 → rate_limit_error
500 → api_error
529 → overloaded_error
504 → socket hang up (incomplete mapping)
```

### Current Implementation Issues
1. **Medium Priority**: "network_error" fallback violates spec (should be "unknown_error")
2. **Medium Priority**: has_recent_errors() 60s window conflicts with RED gating spec
3. **Medium Priority**: Duplicative status determination logic
4. **Low Priority**: Unused _error_type parameter
5. **Missing**: 504 → "socket_hang_up" mapping incomplete in official list

## Consolidated Implementation Tasks

### Phase 1: Error Classification Fixes (Priority 1)
- [ ] Fix classify_connection_error() fallback: "network_error" → "unknown_error"
- [ ] Verify all HTTP status mappings match official spec
- [ ] Add missing 504 → "socket_hang_up" classification  
- [ ] Audit connection error classification for fetch failures (status 0)

### Phase 2: Architectural Safety (Priority 2)  
- [ ] Add documentation warning for has_recent_errors() - diagnostic only
- [ ] Ensure no RED gating logic uses ErrorTracker (use JsonlMonitor only)
- [ ] Consider renaming to has_recent_errors_diagnostic() for clarity

### Phase 3: Code Quality (Priority 3)
- [ ] Remove unused _error_type parameter OR use as hint for status 0
- [ ] Replace eprintln! with DebugLogger when available
- [ ] Deprecate duplicative determine_status() and calculate_percentiles()

### Phase 4: Integration Testing (Priority 4)
- [ ] Test connection error classification with fetch failures  
- [ ] Verify error type consistency across network monitoring pipeline
- [ ] Validate no architectural conflicts with JsonlMonitor/HttpMonitor

## Risk Assessment
- **Low Risk**: Error type string changes, documentation updates
- **Medium Risk**: Parameter cleanup, method renaming  
- **High Risk**: Removing duplicative methods (needs migration plan)