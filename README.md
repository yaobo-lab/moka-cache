# moka-cache

`moka-cache` 是一个基于 [`moka`](https://crates.io/crates/moka) 的轻量级 K-V 缓存工具库。
它对 `moka::sync::Cache` 做了一层简单封装，提供了：

- 统一的过期时间模型 `Expiration`
- 基于 `serde` + `bincode` 的泛型对象缓存
- 可选的缓存移除/过期回调
- 常用缓存操作，如 `insert`、`get`、`remove`、`refresh`

这个 crate 适合需要在 Rust 项目中快速缓存字符串、数字、字节数组或结构体数据的场景。

## Features

- 基于 `moka::sync::Cache`
- 支持缓存任意实现了 `Serialize` / `DeserializeOwned` 的类型
- 支持多种 TTL：
  - `Never`
  - `Millis(u64)`
  - `Second(u64)`
  - `Minute(u64)`
  - `Hour(u64)`
- 支持通过回调监听缓存项被移除的原因
- 提供手动刷新 TTL 的 `refresh`

## Install

```bash
cargo add moka-cache
```

## Quick Start

```rust
use moka::notification::RemovalCause;
use moka_cache::{Expiration, MokaCache, MokaCacheData};
use std::sync::Arc;

fn on_remove(key: Arc<String>, _value: MokaCacheData, cause: RemovalCause) {
    println!("key={key}, cause={cause:?}");
}

fn main() -> anyhow::Result<()> {
    let cache = MokaCache::new_default(Some(on_remove), 512);

    cache.insert("message", "hello moka", Expiration::Minute(5))?;

    let value = cache.get::<_, String>("message");
    assert_eq!(value, Some((Expiration::Minute(5), "hello moka".to_string())));

    Ok(())
}
```

## Cache Struct Example

```rust
use moka_cache::{Expiration, MokaCache};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct AppConfig {
    path: String,
    cache_capacity: u32,
}

fn main() -> anyhow::Result<()> {
    let cache = MokaCache::new_default(None, 128);

    let config = AppConfig {
        path: "./data".to_string(),
        cache_capacity: 1024,
    };

    cache.insert("app-config", config.clone(), Expiration::Never)?;

    let value = cache.get::<_, AppConfig>("app-config");
    assert_eq!(value, Some((Expiration::Never, config)));

    Ok(())
}
```

## API Overview

### Create Cache

```rust
let cache = MokaCache::new_default(None, 512);
```

- 第一个参数：可选移除回调
- 第二个参数：最大容量 `max_capacity`

### Insert Data

```rust
cache.insert("key", 123_i32, Expiration::Second(30))?;
```

### Read Data

```rust
let value = cache.get::<_, i32>("key");
```

返回值类型：

```rust
Option<(Expiration, V)>
```

其中：

- `Expiration` 是写入时设置的过期策略
- `V` 是反序列化后的实际值

### Remove Data

```rust
cache.remove("key");
```

### Check Existence

```rust
let exists = cache.contains_key("key");
```

### Get Expiration Policy

```rust
let exp = cache.get_exp("key");
```

### Refresh TTL

`refresh` 会按原来的过期策略重新写入当前 key，从而延长生命周期：

```rust
cache.refresh("key")?;
```

如果 key 不存在，会返回错误；如果过期策略是 `Expiration::Never`，则不会执行刷新。

## About Expiration Cleanup

当前库基于 `moka::sync::Cache`，并暴露了：

```rust
cache.check_exp_interval();
```

这个方法内部会调用 `run_pending_tasks()`，适合在后台线程中定期执行，以便更及时地触发过期清理和回调。例如：

```rust
use std::{thread, time::Duration};

thread::spawn({
    let cache = cache_handler.clone();
    move || loop {
        thread::sleep(Duration::from_millis(50));
        cache.check_exp_interval();
    }
});
```

如果你的业务依赖“过期后尽快触发移除回调”，建议显式调度这个方法。

## Supported Value Types

只要类型实现了以下 trait，就可以直接缓存：

- `serde::Serialize`
- `serde::de::DeserializeOwned`

例如：

- `String`
- `bool`
- `i32` / `u32` / `u64`
- `Vec<u8>`
- 自定义结构体

## Public Types

项目当前暴露的核心类型包括：

- `MokaCache`
- `MokaCacheHandler = Arc<MokaCache>`
- `MokaCacheData = (Expiration, Vec<u8>)`
- `Expiration`

## Notes

- key 类型统一使用字符串语义，接口接收 `AsRef<str>`
- value 会以 `bincode` 二进制格式写入缓存
- 读取时如果反序列化失败，会记录错误日志并返回 `None`
- `refresh` 的实现是“读取旧值后重新插入”，适合 TTL 续期场景

## License

MIT
