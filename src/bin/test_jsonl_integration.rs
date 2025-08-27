use std::env;
use std::fs;
use std::path::PathBuf;
use ccstatus::core::network::jsonl_monitor::JsonlMonitor;
use ccstatus::core::network::debug_logger::EnhancedDebugLogger;

fn main() {
    println!("🧪 Integration Test: JsonlMonitor Always-On Behavior");
    
    // Define log paths (same logic as in the implementation)
    let mut debug_path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    debug_path.push(".claude");
    debug_path.push("ccstatus"); 
    debug_path.push("ccstatus-debug.log");
    
    let jsonl_path = EnhancedDebugLogger::get_jsonl_log_path();
    
    // Clean up any existing test files
    let _ = fs::remove_file(&debug_path);
    let _ = fs::remove_file(&jsonl_path);
    
    // Test 1: CCSTATUS_DEBUG=false (Critical test case)
    println!("\n🔴 Testing CCSTATUS_DEBUG=false (Critical test case)");
    env::set_var("CCSTATUS_DEBUG", "false");
    
    let monitor = JsonlMonitor::new();
    
    // Create a test file with mock JSONL content that contains API errors
    let test_jsonl_content = r#"
{"message":{"content":[{"text":"API Error: 500 Internal Server Error"}]},"timestamp":"2024-01-01T12:00:00Z","isApiErrorMessage":true}
{"message":{"content":[{"text":"Normal message"}]},"timestamp":"2024-01-01T12:01:00Z","isApiErrorMessage":false}
{"message":{"content":[{"text":"API error 429 rate limited"}]},"timestamp":"2024-01-01T12:02:00Z","isApiErrorMessage":false}
"#.trim();
    
    // Create temporary test file
    let test_file_path = "/tmp/test_transcript.jsonl";
    fs::write(test_file_path, test_jsonl_content).expect("Failed to create test file");
    
    // Use the real JsonlMonitor integration path
    println!("   📊 Scanning test transcript with JsonlMonitor...");
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    match rt.block_on(monitor.scan_tail(test_file_path)) {
        Ok((error_detected, last_error)) => {
            println!("   ✅ Scan completed: error_detected={}, last_error={:?}", error_detected, last_error);
        }
        Err(e) => {
            println!("   ❌ Scan failed: {}", e);
        }
    }
    
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
            let has_error_type = content.contains("\"type\":\"jsonl_error\"");
            let has_tail_complete = content.contains("\"type\":\"tail_scan_complete\"");
            
            println!("   📄 JSONL Content validation:");
            println!("      Contains jsonl_error: {}", if has_error_type { "YES ✅" } else { "NO ❌" });
            println!("      Contains tail_scan_complete: {}", if has_tail_complete { "YES ✅" } else { "NO ❌" });
            
            if has_error_type && has_tail_complete {
                println!("   ✅ PASS: JSONL operational data written correctly");
            } else {
                println!("   ❌ FAIL: Missing expected JSONL operational data");
                println!("      JSONL Content: {}", content);
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
    
    let monitor = JsonlMonitor::new();
    
    // Scan again with debug enabled
    println!("   📊 Scanning test transcript with debug enabled...");
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    match rt.block_on(monitor.scan_tail(test_file_path)) {
        Ok((error_detected, last_error)) => {
            println!("   ✅ Scan completed: error_detected={}, last_error={:?}", error_detected, last_error);
        }
        Err(e) => {
            println!("   ❌ Scan failed: {}", e);
        }
    }
    
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
        }
    } else {
        println!("   ❌ FAIL: Missing logs when CCSTATUS_DEBUG=true");
    }
    
    // Cleanup
    let _ = fs::remove_file(test_file_path);
    let _ = fs::remove_file(&debug_path);  
    let _ = fs::remove_file(&jsonl_path);
    
    println!("\n🏁 Integration test completed!");
    println!("   This test validates the real JsonlMonitor integration path,");
    println!("   not just the isolated EnhancedDebugLogger behavior.");
}