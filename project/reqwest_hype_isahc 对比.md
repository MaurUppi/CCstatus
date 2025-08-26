# reqwest / hyper / isahc

## 一句话总结
**reqwest**：像 Python requests 一样简单易用的高层客户端，适合快速开发。
**hyper**：追求极致性能的底层 HTTP 引擎，给你完全控制权但需要更多代码。
**isahc**：基于久经考验的 libcurl，功能最全但依赖外部库。

## **reqwest** - 高层级易用客户端

**优势：**
- **极简 API**：设计类似 Python requests，学习曲线平缓
- **功能全面**：内置 JSON、cookies、代理、重定向、超时等
- **中间件系统**：支持拦截器和自定义处理
- **文档完善**：社区活跃，示例丰富

```rust
// reqwest 示例 - 简洁直观
let resp = reqwest::get("https://api.github.com/users/octocat")
    .await?
    .json::<User>()
    .await?;
```

**劣势：**
- 较重的依赖链
- 性能开销相对较高
- 定制化程度有限

## **hyper** - 底层高性能引擎

**优势：**
- **极致性能**：Rust HTTP 生态的性能基准
- **高度可控**：完全控制连接、池化、协议细节
- **零成本抽象**：接近裸机性能
- **协议完整**：HTTP/1.1, HTTP/2, HTTP/3 支持

```rust
// hyper 示例 - 更多样板代码但更可控
let https = HttpsConnector::new();
let client = Client::builder().build::<_, hyper::Body>(https);
let resp = client.get(uri).await?;
```

**劣势：**
- **复杂度高**：需要手动处理很多细节
- **样板代码多**：简单请求也需要较多设置
- **学习曲线陡峭**

## **isahc** - 基于 libcurl 的混合方案

**优势：**
- **成熟稳定**：基于久经考验的 libcurl
- **协议支持广**：支持 HTTP/3、各种认证方式
- **C互操作**：与现有 C/C++ 代码集成友好
- **功能丰富**：继承 libcurl 的所有特性

```rust
// isahc 示例 - 平衡的复杂度
let mut response = isahc::get_async("https://example.com").await?;
let body = response.text().await?;
```

**劣势：**
- **外部依赖**：需要系统安装 libcurl
- **性能开销**：FFI 调用带来额外开销
- **调试困难**：错误信息可能来自 C 层

## **详细对比表**

| 特性 | reqwest | hyper | isahc |
|------|---------|-------|-------|
| **易用性** | ⭐⭐⭐⭐⭐ | ⭐⭐ | ⭐⭐⭐ |
| **性能** | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ |
| **功能完整性** | ⭐⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ |
| **文档质量** | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐⭐ |
| **生态集成** | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐⭐ |
| **二进制大小** | 较大 | 中等 | 中等 |
| **编译时间** | 长 | 中等 | 中等 |

## **相同点**

- 都支持 **async/await** 异步编程
- 都提供 **TLS/HTTPS** 支持
- 都支持 **连接池和复用**
- 都有活跃的社区维护
- 都支持**流式处理**大文件

## **选择建议**

**选择 reqwest 如果：**
- 需要快速原型开发
- 团队经验有限
- 功能需求标准
- 性能要求不是极致

**选择 hyper 如果：**
- 性能是首要考虑
- 需要精细控制HTTP行为
- 构建高性能服务器
- 愿意投入学习成本

**选择 isahc 如果：**
- 需要特殊协议支持
- 与 C/C++ 系统集成
- 需要 libcurl 的特定功能
- 团队熟悉 curl 生态

实际上，reqwest 底层就使用了 hyper，所以在大多数场景下，reqwest 是最佳平衡点。