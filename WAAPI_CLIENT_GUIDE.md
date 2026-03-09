# WAAPI Client 使用对比

## 原始方式 (rpc_caller_waapi.rs)

```rust
use std::error::Error;
use serde_json::json;
use wamp_async::{
    try_into_kwargs, try_into_wamp_dict, Client, ClientConfig, ClientRole, SerializerType,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    // 1. 建立连接
    let (mut client, (evt_loop, _rpc_evt_queue)) = Client::connect(
        "ws://localhost:8080/waapi",
        Some(
            ClientConfig::default()
                .set_ssl_verify(false)
                .set_roles(vec![ClientRole::Caller])
                .set_serializers(vec![SerializerType::Json]),
        ),
    )
    .await?;
    println!("Connected !!");

    // 2. 手动启动事件循环（容易忘记！）
    tokio::spawn(evt_loop);

    // 3. 加入 realm
    println!("Joining realm");
    client.join_realm("realm1").await?;

    // 4. 手动转换参数格式
    let send_kwargs = try_into_kwargs(json!({
        "from": {
            "ofType": ["Event"]
        },
    }))?;
    let options = try_into_wamp_dict(json!({
        "return": ["name", "id", "type", "path"]
    }))?;

    // 5. 调用 RPC
    match client
        .call(
            "ak.wwise.core.object.get",
            None,
            Some(send_kwargs),
            Some(options),
        )
        .await
    {
        Ok((res_args, res_kwargs)) => {
            // 6. 手动处理返回值元组
            println!("\tGot {:?} {:?}", res_args, res_kwargs);
        }
        Err(e) => {
            println!("Error calling ({:?})", e);
        }
    };

    // 7. 离开 realm
    println!("Leaving realm");
    client.leave_realm().await?;

    // 8. 断开连接
    client.disconnect().await;
    Ok(())
}
```

**问题：**
- 需要 8 个步骤才能完成一次调用
- 容易忘记启动事件循环
- 需要手动转换参数和处理返回值
- 大量样板代码

---

## 新的简化方式 (waapi_client_simple.rs)

```rust
use serde_json::json;
use std::error::Error;
use wamp_async::WaapiClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    // 1. 创建并连接（自动启动事件循环、自动 join realm）
    println!("Connecting to WAAPI server...");
    let mut client = WaapiClient::builder()
        .host("localhost")
        .port(8080)
        .connect()
        .await?;

    println!("Connected successfully!");

    // 2. 直接调用（自动转换参数，返回 JSON Value）
    println!("\nCalling ak.wwise.core.object.get...");
    let result = client
        .call(
            "ak.wwise.core.object.get",
            Some(json!({
                "from": {
                    "ofType": ["Event"]
                }
            })),
            Some(json!({
                "return": ["name", "id", "type", "path"]
            })),
        )
        .await?;

    // 3. 直接使用 JSON Value
    println!("\nResult:");
    println!("{}", serde_json::to_string_pretty(&result)?);

    // 4. 关闭连接（自动 leave realm + disconnect）
    println!("\nClosing connection...");
    client.close().await?;
    println!("Done!");

    Ok(())
}
```

**优势：**
- 只需 4 个步骤完成整个流程
- 构建器模式配置，清晰直观
- 自动管理连接生命周期（事件循环、realm）
- 参数自动转换
- 返回值是易用的 `serde_json::Value`
- 更少的样板代码

---

## API 对比

| 特性 | 原始 API | 简化 API |
|------|----------|----------|
| 配置方式 | 手动创建 `ClientConfig` | 构建器模式链式调用 |
| 事件循环 | 需要手动 `tokio::spawn` ⚠️ | 自动启动 ✅ |
| Realm 管理 | 需要手动 join/leave | 自动管理 ✅ |
| 参数转换 | 需要调用 `try_into_kwargs` 等 | 自动转换 ✅ |
| 返回值 | `(Option<Vec>, Option<Map>)` 元组 | `serde_json::Value` ✅ |
| 错误处理 | `WampError` | `WaapiError` (更友好) ✅ |
| 代码行数 | ~60 行 | ~30 行 ✅ |

---

## 使用建议

### 何时使用简化 API (`WaapiClient`)

✅ **推荐使用场景：**
- 只需调用 WAAPI RPC（不做服务端）
- 想要简单快速的集成
- 不需要底层 WAMP 协议细节
- 将 WAAPI 集成到应用中

### 何时使用原始 API (`Client`)

✅ **推荐使用场景：**
- 需要实现 RPC Callee（提供服务）
- 需要 Pub/Sub 功能
- 需要完全控制 WAMP 协议细节
- 需要自定义序列化器或传输层

---

## 配置选项

`WaapiClientBuilder` 支持的配置：

```rust
WaapiClient::builder()
    .host("localhost")        // WAAPI 服务器地址 (默认: "localhost")
    .port(8080)               // WAAPI 服务器端口 (默认: 8080)
    .connect()
    .await?
```

---

## 复用连接

`WaapiClient` 支持长连接复用，可以多次调用 `call`：

```rust
let mut client = WaapiClient::builder()
    .host("localhost")
    .port(8080)
    .connect()
    .await?;

// 第一次调用
let events = client.call(
    "ak.wwise.core.object.get", 
    Some(json!({"from": {"ofType": ["Event"]}})),
    Some(json!({"return": ["name", "id"]}))
).await?;

// 第二次调用（复用连接）
let info = client.call("ak.wwise.core.getInfo", None, None).await?;

// 第三次调用
let version = client.call("ak.wwise.core.getVersion", None, None).await?;

// 关闭连接
client.close().await?;
```

---

## 错误处理

    "ak.wwise.core.object.get",
    Some(json!({"from": {"ofType": ["Event"]}})),
    None

`WaapiError` 提供了更清晰的错误信息：

```rust
match client.call("ak.wwise.core.object.get", ...).await {
    Ok(result) => println!("Success: {}", result),
    Err(WaapiError::ConnectionFailed(e)) => println!("连接失败: {}", e),
    Err(WaapiError::CallFailed(e)) => println!("调用失败: {}", e),
    Err(e) => println!("其他错误: {}", e),
}
```
