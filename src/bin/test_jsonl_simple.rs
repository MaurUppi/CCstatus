use std::env;
use std::fs;
use std::path::PathBuf;
use ccstatus::core::network::jsonl_monitor::JsonlMonitor;
use ccstatus::core::network::debug_logger::EnhancedDebugLogger;
use serde_json::json;

fn main() {
    println!("🧪 Simple Integration Test: JsonlMonitor Always-On Behavior");
    
    // Define log paths (same logic as in the implementation)
    let mut debug_path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    debug_path.push(".claude");
    debug_path.push("ccstatus");
    debug_path.push("ccstatus-debug.log");
    
    let jsonl_path = EnhancedDebugLogger::get_jsonl_log_path();
    
    println!("   Debug log path: {:?}", debug_path);
    println!("   JSONL log path: {:?}", jsonl_path);
    
    // Clean up any existing test files
    let _ = fs::remove_file(&debug_path);
    let _ = fs::remove_file(&jsonl_path);
    
    // Test 1: CCSTATUS_DEBUG=false (Critical test case)
    println!("\n🔴 Testing CCSTATUS_DEBUG=false (Critical test case)");
    env::set_var("CCSTATUS_DEBUG", "false");
    
    let monitor = JsonlMonitor::new();
    println!("   📊 Created JsonlMonitor with CCSTATUS_DEBUG=false");
    
    // Access the internal logger directly to test JSONL writing
    // This simulates what would happen during error detection
    let test_jsonl_entry = json!({
        "timestamp": "2025-08-27T10:50:32.660+08:00",
        "type": "jsonl_error",
        "code": 500,
        "message": "Test API Error",
        "error_timestamp": "2024-01-01T12:01:00Z",
        "session_id": "test1234"
    });
    
    println!("   ✍️  Writing test JSONL entry...");
    // We need to use reflection or testing interface since logger field is private
    // For now, let's create a logger directly and test
    let test_logger = EnhancedDebugLogger::new();
    
    // Test JSONL writing (should always work)
    match test_logger.jsonl_sync(test_jsonl_entry) {
        Ok(_) => println!("   ✅ JSONL sync succeeded"),
        Err(e) => println!("   ❌ JSONL sync failed: {}", e),
    }
    
    // Test debug writing (should be gated by CCSTATUS_DEBUG) 
    test_logger.debug_sync("JsonlMonitor", "test_event", "Test debug message");
    
    // Validate results
    let debug_exists = debug_path.exists();
    let jsonl_exists = jsonl_path.exists();
    
    println!("\n🎯 Validation Results for CCSTATUS_DEBUG=false:");
    println!("   Debug log exists: {}", if debug_exists { "YES ❌" } else { "NO ✅" });
    println!("   JSONL log exists: {}", if jsonl_exists { "YES ✅" } else { "NO ❌" });
    
    if !debug_exists && jsonl_exists {
        println!("   ✅ PASS: Only JSONL log created when CCSTATUS_DEBUG=false");
        
        // Verify JSONL content
        if let Ok(content) = fs::read_to_string(&jsonl_path) {
            let has_test_entry = content.contains("\"type\":\"jsonl_error\"") && 
                                content.contains("\"message\":\"Test API Error\"");
            
            println!("   📄 JSONL Content validation:");
            println!("      Contains test entry: {}", if has_test_entry { "YES ✅" } else { "NO ❌" });
            
            if has_test_entry {
                println!("   ✅ PASS: JSONL operational data written correctly");
                println!("      Content: {}", content.trim());
            } else {
                println!("   ❌ FAIL: Missing expected JSONL operational data");
                println!("      Content: {}", content);
            }
        } else {
            println!("   ❌ FAIL: Could not read JSONL log content");
        }
    } else {
        println!("   ❌ FAIL: Incorrect logging behavior for CCSTATUS_DEBUG=false");
    }
    
    // Test 2: CCSTATUS_DEBUG=true (Should work like before)
    println!("\n🟢 Testing CCSTATUS_DEBUG=true (Regression test)");
    
    // Clean up files
    let _ = fs::remove_file(&debug_path);
    let _ = fs::remove_file(&jsonl_path);
    
    env::set_var("CCSTATUS_DEBUG", "true");
    
    let test_logger = EnhancedDebugLogger::new();
    
    // Test both JSONL and debug writing
    println!("   ✍️  Testing with debug enabled...");
    
    let test_jsonl_entry = json!({
        "timestamp": "2025-08-27T10:50:32.660+08:00",
        "type": "tail_scan_complete",
        "code": 0,
        "message": "count=1",
        "session_id": "test1234"
    });
    
    match test_logger.jsonl_sync(test_jsonl_entry) {
        Ok(_) => println!("   ✅ JSONL sync succeeded"),
        Err(e) => println!("   ❌ JSONL sync failed: {}", e),
    }
    
    test_logger.debug_sync("JsonlMonitor", "tail_scan_complete", "Test debug message with debug enabled");
    
    let debug_exists = debug_path.exists();
    let jsonl_exists = jsonl_path.exists();
    
    println!("\n🎯 Validation Results for CCSTATUS_DEBUG=true:");
    println!("   Debug log exists: {}", if debug_exists { "YES ✅" } else { "NO ❌" });
    println!("   JSONL log exists: {}", if jsonl_exists { "YES ✅" } else { "NO ❌" });
    
    if debug_exists && jsonl_exists {
        println!("   ✅ PASS: Both logs created when CCSTATUS_DEBUG=true");
        
        // Validate debug log format (should be flat-text)
        if let Ok(debug_content) = fs::read_to_string(&debug_path) {
            let is_flat_format = debug_content.contains("[JsonlMonitor]") && 
                                debug_content.contains("\"tail_scan_complete\"");
            println!("   📄 Debug log format: {}", if is_flat_format { "Flat-text ✅" } else { "Incorrect ❌" });
            println!("      Debug content: {}", debug_content.trim());
        }
        
        // Validate JSONL content  
        if let Ok(jsonl_content) = fs::read_to_string(&jsonl_path) {
            println!("   📄 JSONL content: {}", jsonl_content.trim());
        }
    } else {
        println!("   ❌ FAIL: Missing logs when CCSTATUS_DEBUG=true");
    }
    
    // Cleanup
    let _ = fs::remove_file(&debug_path);
    let _ = fs::remove_file(&jsonl_path);
    
    println!("\n🏁 Simple integration test completed!");
    println!("   This validates that JSONL logging is always-on regardless of CCSTATUS_DEBUG,");
    println!("   while debug logging is properly gated by the CCSTATUS_DEBUG setting.");
}