# NetworkSegment 架构重设计与实施计划

**基于 cursor-gpt-5 反馈的优化方案**
**参考文档** [1-network_monitoring-Requirement-Final](1-network_monitoring-Requirement-Final.md)

---

## Related modules
- `src/core/network/`：网络检测核心实现与装配
- `tests/network/`：网络检测功能与回归测试


---

## 🚨 关键风险缓解策略（全局）

- 单一数据源与读写分离：`HttpMonitor` 作为唯一状态写入者；`NetworkSegment` 仅读。
- 可观测性约定：启用 `CCSTATUS_DEBUG` 时在阶段边界打印关键事件与耗时。
- 统一超时/资源上限：通过 `ccstatus_TIMEOUT_MS` 配置，并在代码侧设上限（≤6000ms）。
- 时间戳一致性：所有持久化时间均为本地时区 ISO-8601（含偏移）。

## 🎯 核心架构决策

### 采用"同步外壳 + 异步内核"模式（先解析凭证，再做门控）

**原理**: 保持 `Segment::collect()` 同步接口，内部通过已存在的 Tokio runtime 执行异步操作

```rust
impl Segment for NetworkSegment {
    fn collect(&self, input: &InputData) -> Option<SegmentData> {
        // 0) 同步外壳：读取 stdin 字段
        let total_duration_ms = input.cost.total_duration_ms;
        let session_id = &input.session_id;

        // 1) 先解析凭证；无凭证→写 unknown 并返回（单写者：HttpMonitor）
        let creds = self.resolve_credentials_sync(); // 内部用现有 Tokio runtime 执行异步获取
        if creds.is_none() {
            self.write_unknown_async();
            return self.render_current_state();
        }
        let creds = creds.unwrap();

        // 2) 门控优先级：COLD > RED > GREEN（本轮只执行一种；命中 COLD 则跳过 RED/GREEN）
        let gate_result = self.calculate_gate_priority(total_duration_ms, session_id, &input.transcript_path);

        match gate_result {
            GateType::Cold(uuid) => {
                self.execute_cold_probe(creds.clone(), uuid);
            },
            GateType::Red => {
                self.execute_red_probe(creds.clone(), &input.transcript_path);
            },
            GateType::Green => {
                self.execute_green_probe(creds.clone());
            },
            GateType::Skip => {}
        }

        // 3) 立即返回当前状态（不等待异步完成）
        self.render_current_state()
    }
}
```

---

## 📋 重新优化的三阶段实施计划

### Phase 1: Schema 扩展 + 状态基础设施

**目标**: 建立完整的数据基础和状态管理

#### 1.1 InputData Schema 扩展
```rust
// src/config/types.rs
#[derive(Deserialize)]
pub struct InputData {
    pub model: Model,
    pub workspace: Workspace,
    pub transcript_path: String,
    pub cost: CostData,           // 仅 stdin 中的 total_duration_ms
    pub session_id: String,       // 来自 stdin 的会话标识，用于 COLD 去重
}

#[derive(Deserialize)]
pub struct CostData {
    // 仅需要这一个字段，其他 stdin cost 字段（如 total_cost_usd/total_api_duration_ms）不引入
    pub total_duration_ms: u64,    // GREEN/RED 窗口计算核心
}
```

#### 1.2 状态结构完善（COLD 去重支持）
```rust
// src/core/segments/network/types.rs - 扩展 MonitoringState
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringState {
    pub last_green_window_id: u64,
    pub last_red_window_id: u64,
    pub last_cold_session_id: Option<String>,   // 新增：基于 session_id 的 COLD 去重
    pub last_cold_probe_at: Option<String>,     // 新增：本地时区时间戳
    pub state: NetworkStatus,
}
```

#### 1.3 ProbeMode 扩展
```rust
// src/core/segments/network/http_monitor.rs
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProbeMode {
    Green,
    Red,
    Cold,  // 新增：冷启动探测
}
```

#### 1.4 时间戳标准化
```rust
// 统一使用本地时区 ISO-8601 格式
fn get_local_timestamp() -> String {
    chrono::Local::now().to_rfc3339()
}
```

#### 1.5 环境变量与默认值
```
ccstatus_TIMEOUT_MS=3000            # 覆盖 COLD/GREEN/RED 的超时上限（min(值, 6000)）
ccstatus_COLD_WINDOW_MS=5000        # 覆盖默认 COLD 窗口阈值（ms）
CCSTATUS_DEBUG=true                 # 启用调试侧车日志
```

#### 风险与缓解（Phase 1）
- Schema 兼容性：为新增字段提供默认值/`Option`，避免老输入崩溃；新增字段先读后用。
- 状态落盘一致性：采用“临时文件 + fsync + 原子 rename”写盘流程，避免部分写入：
```rust
// 简化示例：原子落盘思路
impl HttpMonitor {
    async fn save_state_atomic(&self) -> Result<(), NetworkError> {
        let temp_path = self.state_path.with_extension("tmp");
        // 1) serialize -> write temp -> fsync
        // 2) atomic rename 覆盖正式文件
        tokio::fs::rename(&temp_path, &self.state_path).await?;
        Ok(())
    }
}
```
- 时间戳统一：落盘前统一调用 `get_local_timestamp()`，禁止混用 UTC/本地。

#### Phase 1 完成标准（含文档） 
- [ ] 仅 `cost.total_duration_ms`；包含 `session_id` 字段 
- [ ] `MonitoringState` 包含 COLD 去重字段 
- [ ] `ProbeMode` 包含 `Cold` 变体 
- [ ] 时间戳标准化为本地时区 ISO-8601
- [ ] 单元测试覆盖新增字段 
- [ ] 文档：更新本计划 Phase 1 进度与接口变更说明 
- [ ] 文档：为新增/修改的 `types.rs`、`http_monitor.rs` 添加 Rustdoc 
- [ ] 文档：如路径/职责变更，更新文档的 `Related modules` 小节
- [ ] 文档：同步 `tests/network/` 用例命名/说明与新增场景


### Phase 2: 时序编排 + 门控逻辑

**目标**: 实现完整的时序驱动监控逻辑

#### 2.1 门控优先级计算（COLD 窗口参数化 + 去重 & 并发抑制）
```rust
#[derive(Debug)]
enum GateType {
    Cold(String),    // 包含 session_id
    Red,
    Green,
    Skip,
}

impl NetworkSegment {
    fn calculate_gate_priority(&self, 
        total_duration_ms: u64, 
        session_id: &str,
        transcript_path: &str
    ) -> GateType {
        // 优先级1: COLD（stdin 统一门控；默认阈值 5000ms，可由 env ccstatus_COLD_WINDOW_MS 覆盖）
        if total_duration_ms < self.cold_window_ms {
            if self.should_cold_probe(session_id) && !self.in_flight() {
                return GateType::Cold(session_id.to_string());
            }
        }
        
        // 优先级2: RED 检测（有错误 + 命中窗口）
        if let Ok((error_detected, _)) = self.jsonl_monitor.scan_tail(transcript_path) {
            if error_detected && self.in_red_window(total_duration_ms) {
                return GateType::Red;
            }
        }
        
        // 优先级3: GREEN 检测
        if self.in_green_window(total_duration_ms) {
            return GateType::Green;
        }
        
        GateType::Skip
    }
    
    fn in_green_window(&self, total_duration_ms: u64) -> bool {
        let window_id = total_duration_ms / 300_000;
        let in_window = (total_duration_ms % 300_000) < 3_000;
        
        if in_window && self.current_state.monitoring_state.last_green_window_id != window_id {
            return true;
        }
        false
    }
    
    fn in_red_window(&self, total_duration_ms: u64) -> bool {
        let window_id = total_duration_ms / 10_000;
        let in_window = (total_duration_ms % 10_000) < 1_000;
        
        if in_window && self.current_state.monitoring_state.last_red_window_id != window_id {
            return true;
        }
        false
    }
    
    fn should_cold_probe(&self, session_id: &str) -> bool {
        // 去重：相同 session_id 不重复 COLD
        if let Some(last_id) = &self.current_state.monitoring_state.last_cold_session_id {
            if last_id == session_id { return false; }
        }
        true
    }
}
```

#### 2.2 异步执行核心
```rust
impl NetworkSegment {
    fn execute_cold_probe(&self, creds: ResolvedCreds, session_id: String) {
        let monitor = Arc::clone(&self.http_monitor);
        let cred_mgr = Arc::clone(&self.credential_manager);
        let uuid = session_id.clone();
        
        // 使用现有 runtime，短促执行
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                // in-flight 抑制
                if monitor.mark_in_flight(true) {
                    let _ = tokio::time::timeout(Duration::from_millis(monitor.cold_timeout_ms()), async {
                        let _ = monitor.probe(&creds, ProbeMode::Cold).await;
                        monitor.update_cold_dedup(uuid, get_local_timestamp()).await.ok();
                    }).await;
                    monitor.mark_in_flight(false);
                }
            });
        }
    }
    
    fn execute_green_probe(&self, creds: ResolvedCreds) {
        // 类似 COLD，但使用 ProbeMode::Green
        // 更新 last_green_window_id
    }
    
    fn execute_red_probe(&self, creds: ResolvedCreds, transcript_path: &str) {
        // 类似 COLD，但使用 ProbeMode::Red
        // 包含错误事件信息
        // 更新 last_red_window_id
    }
}
```

#### 2.3 HttpMonitor 扩展支持
```rust
impl HttpMonitor {
    pub async fn write_unknown(&mut self) -> Result<(), NetworkError> {
        self.current_state.status = NetworkStatus::Unknown;
        self.current_state.monitoring_enabled = false;
        self.current_state.api_config = None;
        self.save_state().await
    }
    
    pub async fn update_cold_dedup(&mut self, session_id: String, timestamp: String) -> Result<(), NetworkError> {
        self.current_state.monitoring_state.last_cold_session_id = Some(session_id);
        self.current_state.monitoring_state.last_cold_probe_at = Some(timestamp);
        self.save_state().await
    }
}
```

#### 风险与缓解（Phase 2）
- 并发控制与去重：窗口去重 + `parent_uuid` 去重，并在探测入口设置 in-flight 抑制。
- 异步执行控制：复用现有 runtime，所有异步探测使用 `tokio::time::timeout` 包裹，避免长阻塞：
```rust
if let Ok(handle) = tokio::runtime::Handle::try_current() {
    handle.spawn(async move {
        let _ = tokio::time::timeout(Duration::from_millis(3000), async_work).await;
    });
} else {
    log::warn!("No async runtime; skipping probe for this tick");
}
```
- 错误窗口判定可靠性：对 `JsonlMonitor` 增加尾部读取大小与解析失败的降级策略。

#### Phase 2 完成标准（含文档）
- [ ] `collect()` 实现 COLD > RED > GREEN 门控优先级
- [ ] 窗口计算逻辑正确（包含去重）
- [ ] 异步探测执行不阻塞 `collect()`
- [ ] `JsonlMonitor` 错误检测集成
- [ ] `HttpMonitor` 支持三种探测模式
- [ ] 文档：更新门控时序图/伪代码与并发/去重策略说明
- [ ] 文档：更新 `.env` 示例与配置说明（含超时/窗口参数）
- [ ] 文档：为公共方法/类型补充 Rustdoc

### Phase 3: 集成测试 + 边界验证

**目标**: 确保系统在各种场景下正确工作

#### 3.1 窗口边界测试
- GREEN 窗口边界 (2999ms, 3000ms, 3001ms)
- RED 窗口边界 (999ms, 1000ms, 1001ms)
- 窗口 ID 去重验证
- 快速连续 stdin 事件处理

#### 3.2 去重机制测试
- COLD 去重：相同 session_id 只探测一次
- 窗口去重：同一窗口期内只探测一次
- 状态竞争：多个异步探测的原子性
 - in-flight 抑制：同一时刻仅允许一个 probe 在进行

#### 3.3 错误路径测试
- 无凭证场景：write_unknown 行为
- 网络失败：超时、连接拒绝等
- 错误分类：429 → degraded，5xx → error
- JsonlMonitor 错误检测准确性

#### 3.4 性能与稳定性测试
- 高频 stdin 事件（每 100ms 一次）
- 长时间运行稳定性
- 内存泄漏检测
- 状态文件完整性

#### 风险与缓解（Phase 3）
- 测试脆弱性：为涉及时间窗口的用例加入稳定化策略（固定时间源/宽容断言）。
- 环境依赖：网络失败类用例使用可控本地桩/超时，而非真实外网。

#### Phase 3 完成标准（含文档）
- [ ] 所有窗口边界情况测试通过
- [ ] 去重机制在高频场景下工作正常
- [ ] 错误处理路径覆盖完整
- [ ] 性能指标满足要求（`collect()` < 10ms）
- [ ] 24 小时稳定性测试通过
- [ ] 文档：记录测试结果与基准数据（含关键图表/表格）
- [ ] 文档：更新 README/用户指南中的“网络监控”能力说明
- [ ] 文档：在 PR 描述中勾选各阶段 checklist 并附测试报告链接

---

---

## 📊 预期性能指标

- **collect() 延迟**: < 10ms (同步部分)
- **探测频率**: GREEN 每 5 分钟，RED 每 10 秒（错误时）
- **内存占用**: < 10MB 增长
- **状态文件大小**: < 50KB
- **错误恢复时间**: < 30 秒

---

## 🔧 开发工具和调试

```bash
# 环境变量调试
export CCSTATUS_DEBUG=true              # 启用详细日志
export ccstatus_TIMEOUT_MS=3000         # 设置探测超时
export CCSTATUS_JSONL_TAIL_KB=32        # JsonlMonitor 读取大小

# 测试命令
cargo test network_segment -- --nocapture
cargo test window_calculation -- --nocapture
cargo test cold_dedup -- --nocapture

# 集成测试
./target/debug/ccstatus < test_input.json
```

这个重新设计的方案充分采纳了 cursor-gpt-5 的建议，特别是：

1. **架构不变性**: 保持 Segment trait 同步，避免大范围代码改动
2. **优先级门控**: COLD > RED > GREEN 确保逻辑清晰
3. **去重机制**: 双层去重（窗口 + session_id）避免重复探测
4. **渐进实施**: 三阶段循序渐进，降低风险
5. **性能优化**: 异步内核不阻塞主流程

整个方案在保持架构稳定性的前提下，实现了完整的时序驱动网络监控功能。