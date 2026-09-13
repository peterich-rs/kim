---
name: rust-quality-coding
description: >
  High-quality idiomatic Rust coding and design rules distilled from the booklet
  《Rust 语言从入门到实战》(ownership, types, traits/std traits, smart pointers,
  error handling, async/Tokio, Axum, unsafe, macros, lifetimes). Use when writing,
  reviewing, refactoring, or designing Rust code in this repo — especially APIs,
  trait design, error layers, async boundaries, and type-driven modeling. Complements
  rust-skills (broad rules) and rust-strict (security lints); prefer this skill for
  design judgment taught by the booklet. Invoke with /rust-quality-coding.
---

# Rust Quality Coding（小册提炼）

基于掘金小册《Rust 语言从入门到实战》全书提炼的**可执行编码与设计规范**。目标是约束 Agent：先写对 Rust 心智模型，再写功能。

与现有 skill 分工：

| Skill | 职责 |
|-------|------|
| **rust-quality-coding**（本 skill） | 小册设计哲学 + 决策树 + 硬约束清单 |
| `rust-skills` | 细粒度规则库（按需打开 `rules/`） |
| `rust-strict` | unwrap/unsafe/密钥等安全审计 |
| `rust-async-patterns` | Tokio 进阶模式细节 |

冲突时：**安全规则以 `rust-strict` 为准**；设计取舍以本 skill 为准。

## When to Apply

编写 / 审查 / 重构任何 `.rs` 时默认启用。尤其：

- 新类型、公共 API、trait 抽象
- 错误分层、`Result` / `Option` 处理
- async 边界、共享状态、channel 选型
- 字符串 / 切片 / 智能指针选择
- 是否引入生命周期、`dyn`、宏、`unsafe`

## 设计哲学（全书主线）

1. **所有权三态贯穿一切**：拥有 / 不可变借 / 可变借。先问「谁拥有？谁可变？」，再写代码。
2. **显式优于隐式**：`mut`、`clone`、`unsafe`、`as`、失败路径都要留足迹。
3. **实用三阶段**：先跑通 → 再追求架构美感 → 真正遇到瓶颈再抠生命周期/零拷贝。**不要怕 `.clone()`**。
4. **trait 做能力配置，不做深 OOP 继承树**。
5. **类型驱动设计**：用 `enum`/newtype/洋葱类型把非法状态排除在编译期外。
6. **库 API 不向外暴露生命周期参数**，避免 `'a` 传染上层。
7. **Unsafe 极小化封装层**：Safe 自证安全；Unsafe 由人审计且尽量薄。

## Agent 工作流

写 Rust 前按序自检：

```
- [ ] 所有权：参数该吃 T / 借 &T / 可变 &mut T？业务 struct 是否优先自持有字段？
- [ ] 字符串/切片：入参用 &str / &[T]，不要 &String / &Vec<T>
- [ ] 错误：可恢复用 Result；库用 thiserror；应用边界用 anyhow；禁止无脑 unwrap
- [ ] Trait：约束 vs 能力配置；关联类型 vs 类型参数；dyn 是否对象安全
- [ ] Std traits：需要 Debug/Clone/Default/PartialEq 时 derive；Display 手写；Copy 慎用
- [ ] 异步：Tokio；不在 async 里阻塞；重 CPU 用 spawn_blocking；共享用 Arc+Mutex 或 channel
- [ ] 生命周期：业务优先 owned；库 API 尽量不暴露 'a
- [ ] Unsafe/宏：能不用就不用；必须用则最小边界 + 文档化不变量
```

## 硬约束速查

### 所有权与借用

- 默认 **move**；仅小固定尺寸类型依赖 **Copy**。需要多份所有权 → 显式 `.clone()` 或 `Arc`。
- 默认不可变；需要改才加 `mut`。
- **别名 XOR 可变**：同时多个 `&` **或** 一个 `&mut`，不可交叠。
- 函数：只读 → `&T`/`&str`；要改 → `&mut T`；转移所有权才吃 `T`。
- 业务模型字段优先 **拥有型**（`String`/`Vec`/`PathBuf`）；含引用的 struct 需生命周期，初学/业务层少用。

### 字符串与切片

| 需要 | 用 |
|------|----|
| 拥有可改文本 | `String` |
| 只读视图 / API 入参 | `&str` |
| 协议字节 | `&[u8]` / `Bytes` |
| 路径 | `Path` / `PathBuf` |
| OS 原生串 | `OsStr` / `OsString` |
| C 边界 | `CStr` / `CString` |

禁止把字节下标当「字符」；UTF-8 转换用 `from_utf8`（Result），默认禁止 `*_unchecked`。

### 类型与枚举

- **配置/状态变体** → `enum`（可带 payload）；**数据模型** → `struct`。
- `match` 穷尽；只读字段用 `ref`/`&` 模式，避免无意义 partial move。
- 封闭集合用 enum；开放扩展用 trait（+ 必要时 `dyn`）。
- newtype 区分语义相同但域不同的 ID/单位；`type` 别名做洋葱简化。

### Trait 设计

- Trait = **协议约束** + **能力配置**，不是 Java 式继承。
- **孤儿规则**：`impl Trait for Type` 时 Trait 或 Type 至少一方在当前 crate。
- 无多态需求 → **关联类型**；一对多多态 → **trait 类型参数**（`Add<Rhs>`）。
- 返回「同一 trait、编译期一种具体类型」→ `impl Trait`；运行时多种类型/异质集合 → `Box<dyn Trait>` / `&dyn Trait`。
- **对象安全**：勿在要 dyn 的 trait 里放构造函数/`Self` 返回值/无 receiver 的泛型方法；方法优先 `&self`/`&mut self`。
- 使用 trait 方法前 **import trait**；同名冲突用 UFCS：`<T as Trait>::method`。

### 标准库常见 Trait

| Trait | 做法 |
|-------|------|
| `Debug` | 几乎总是 `#[derive(Debug)]` |
| `Default` | 可 derive；配合 `..Default::default()` |
| `Clone` | 需要多所有权时 derive；**鼓励显式 clone** |
| `Copy` | 仅全字段 Copy 的小类型；必须同时 Clone；语言故意抬高成本 |
| `Display` | **手写**；实现后自动有 `ToString` |
| `PartialEq`/`Eq` | 需要相等比较时 derive；浮点谨慎 |
| `PartialOrd`/`Ord` | 需要排序时四件套一起考虑 |
| `From`/`Into` | 实现 `From`，免费得 `Into`；可失败用 `TryFrom` |
| `AsRef`/`AsMut` | 廉价借用转换（如 `String`→`str`） |
| `Deref` | 智能指针语义；**禁止当继承用** |
| `Drop` | 仅外部资源清理；简单类型靠 RAII 即可 |

### 智能指针

- `Box<T>`：堆上独占、递归类型、`Box<dyn Trait>`。
- `Arc<T>`：跨 task/线程共享所有权；**不能直接可变** → `Arc<Mutex<T>>` / `Arc<RwLock<T>>`。
- `Arc::clone` 只加引用计数，不要求 `T: Clone`。
- API 入参：能接受 `&T` 就不要强制 `&Box<T>` / `&Arc<T>`（除非语义需要）。

### 错误处理

- 不可恢复 → `panic!`/`unwrap`/`expect`（仅不变量/测试）。
- 可恢复 → `Result`；必须处理（`?` / `match`），禁止忽略。
- **库 crate**：`thiserror` 定义错误枚举 + `#[from]`。
- **应用/二进制边界**：`anyhow::Result` + `context`。
- 错误要冒泡到架构选定的边界再处理；用 `From`/`map_err`/`#[from]` 对齐类型。

### 异步 / Tokio

- 默认 Tokio；Future 必须被 `.await` / `block_on` 驱动。
- async 有**传染性**：在模块边界划定 async 岛，纯计算保持同步。
- async 内禁止阻塞 std I/O；重 CPU / 同步库 → `spawn_blocking`。
- sync 调少量 async → 局部 `current_thread` Runtime + `block_on`。
- 连接级 `spawn`；业务消息用 **Framed + Codec**，勿手搓粘包。
- 共享可变：默认 `Arc<tokio::sync::Mutex<T>>`；读多写少 `RwLock`；能消息传递则优于共享可变。
- Channel：mpsc（多生产者单消费者）、oneshot（一次应答）、broadcast、watch（最新值）按拓扑选；优先有界 channel 背压。
- 锁守卫**不要跨 `.await`**（若必须，用 tokio Mutex 并清楚代价）。

### Web（Axum）

- 留在 Tokio/Tower/Axum 技术栈；Router 按模块 nest。
- Handler：extractor 抽取；**body extractor 放最后**；DTO 用 serde 类型。
- 返回统一 `impl IntoResponse` / `Result<..., StatusCode>`；404 设 fallback。
- 状态：`State<AppState>` + 连接池；领域模型与 DTO 分离。
- 日志用 `tracing`，不用 `println!`。

### 生命周期

- 结构体借引用才标 `'a`；函数返回引用须绑定到某个输入生命周期。
- **库公共 API 尽量不暴露生命周期**；上层用 owned / `Arc`。
- 业务代码优先 clone/owned；性能优化放到第三阶段。

### 宏与 Unsafe

- 宏：优先函数；声明宏只消除真实重复；写完 `cargo expand` 验证；**禁止滥用**。
- Unsafe：仅五类超能力场景；封装成最小 safe 门面；FFI 用 `repr(C)` / bindgen / 边界转换；默认禁止 `get_unchecked` / `from_utf8_unchecked`。
- 避免 `static mut`；跨任务状态用 `Arc`/`Atomic`/channel。

## 场景 → 首选模式

| 场景 | 首选 |
|------|------|
| API 字符串入参 | `&str` |
| 业务配置对象 | owned `struct` + `Default` |
| 多状态协议/状态机 | `enum` + 穷尽 `match` |
| 开放插件能力 | trait；异质集合 `Box<dyn Trait>` |
| 库错误 | `thiserror` |
| 应用错误边界 | `anyhow` |
| 异步服务 | Tokio multi-thread |
| 多 task 改同一数据 | `Arc<Mutex<_>>` 或 mpsc 单写者 |
| 请求-应答 | mpsc + oneshot |
| TCP 消息 | Framed + LengthDelimited（或合适 Codec） |
| 解析协议文本/二进制 | 小 parser 组合（Nom 风格） |
| 必须 unsafe | 极薄封装层 + 安全对外 API |

## 反模式（直接拒绝）

- 为通过编译乱加 `'static` / 无意义 `clone` 掩耳盗铃（必要 clone 可以，但要懂为何）
- 公共 API 泄漏生命周期导致上层全员标 `'a`
- `&String` / `&Vec<T>` 作函数参数
- 业务层到处 `unwrap`
- async 里同步阻塞 I/O 或长时间占着 worker 算 CPU
- 用 `Deref` 模拟继承；用宏生成本可用函数表达的逻辑
- 把 `unsafe` 散落在业务代码而非隔离层
- 可变全局 `static mut` 当共享数据库

## 详细规则

需要展开某条规则的正反例与出处时再读：

- [references/core.md](references/core.md) — 所有权、字符串、类型、trait、std trait、智能指针、错误、宏、生命周期、unsafe
- [references/async-design.md](references/async-design.md) — Tokio、共享状态、channel、Axum、parser、工程架构

源材料：本地小册 `NuggetsBooklet/Rust 语言从入门到实战/`（01–30 + 开篇/结束语/答疑）。
