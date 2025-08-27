use std::env;
use serde_json::json;
use ccstatus::core::network::debug_logger::get_debug_logger;

fn main() {
    println!("🧪 Testing Dual Logger Implementation");
    
    let debug_setting = env::var("CCSTATUS_DEBUG").unwrap_or_else(|_| "unset".to_string());
    println!("CCSTATUS_DEBUG = {}", debug_setting);
    
    // Get the debug logger
    let logger = get_debug_logger();
    println!("Logger enabled: {}", logger.is_enabled());
    println!("Session ID: {}", logger.get_session_id());
    println!();
    
    // Test 1: Debug logging (should be gated by CCSTATUS_DEBUG)
    println!("📝 Testing debug logging...");
    logger.debug_sync("TestComponent", "test_event", "This is a test debug message");
    logger.network_probe_start("Green", 3000, "test_probe_123".to_string());
    
    // Test 2: JSONL operational logging (should always work)
    println!("📊 Testing JSONL operational logging...");
    
    let jsonl_entry = json!({
        "timestamp": chrono::Local::now().to_rfc3339(),
        "type": "jsonl_error",
        "code": 500,
        "message": "Test API Error",
        "error_timestamp": "2024-01-01T12:01:00Z",
        "session_id": logger.get_session_id()
    });
    
    match logger.jsonl_sync(jsonl_entry) {
        Ok(_) => println!("✅ JSONL entry written successfully"),
        Err(e) => println!("❌ JSONL write failed: {}", e),
    }
    
    let tail_complete_entry = json!({
        "timestamp": chrono::Local::now().to_rfc3339(),
        "type": "tail_scan_complete",
        "code": 0,
        "message": "count=1",
        "session_id": logger.get_session_id()
    });
    
    match logger.jsonl_sync(tail_complete_entry) {
        Ok(_) => println!("✅ Tail scan complete entry written successfully"),
        Err(e) => println!("❌ Tail scan write failed: {}", e),
    }
    
    println!();
    println!("🎯 Test completed! Check ~/.claude/ccstatus/ for log files.");
    println!("Expected files:");
    if logger.is_enabled() {
        println!("  - ccstatus-debug.log (flat text format)");
    } else {
        println!("  - ccstatus-debug.log should NOT exist");
    }
    println!("  - ccstatus-jsonl-error.json (NDJSON format, always created)");
}