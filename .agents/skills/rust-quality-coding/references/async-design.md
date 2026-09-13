# Async & Design Rules（小册异步/应用篇）

> 来源：《Rust 语言从入门到实战》13–17、21–28、答疑（三）、结束语。
> 忽略纯 GUI/游戏玩法细节；保留可迁移工程约束。

## Async / Tokio

### async-drive-explicitly
**规则：** 用 `async` 定义 Future，必须用 `.await` 或 Runtime 的 `block_on`/`poll` 驱动；勿假设 Future 会自动执行。  
**为何重要：** 未驱动的 Future 什么都不做，导致“写了异步却没跑起来”。  
**正确：** `async {}.await` 或 `rt.block_on(fut)`。  
**反例：** 创建 `let f = async { ... };` 后既不 `.await` 也不 `block_on`。  
**来源：** 13

### async-await-only-in-async
**规则：** 只在 `async` 函数/块内使用 `.await`；`main` 不得直接标 `async`，须经 Runtime 入口（如 `#[tokio::main]`）展开。  
**为何重要：** 语言规则强制最外层由 Runtime 驱动，否则编译失败或无法启动。  
**正确：** `#[tokio::main] async fn main()` 或手动 `Builder::...block_on(...)`。  
**反例：** 在同步 `fn` 里写 `.await`，或写 `async fn main` 无 Runtime。  
**来源：** 13

### prefer-tokio-as-default-runtime
**规则：** 新异步服务默认选 Tokio 作为事实标准 Runtime（除非项目已绑定其他 Runtime）。  
**为何重要：** 生态（Hyper/Axum/tonic 等）与 Tokio 深度耦合，混用 Runtime 成本高。  
**正确：** `tokio = { version = "1", features = [...] }` + `#[tokio::main]`。  
**反例：** 无理由同时引入多个 Runtime 并跨 Runtime 传 Future。  
**来源：** 13, 21

### choose-runtime-flavor-deliberately
**规则：** 多核 I/O 服务用 multi-thread；临时/测试/嵌入同步代码用 `current_thread`。  
**为何重要：** flavor 决定调度与线程模型；选错会浪费资源或阻塞不当。  
**正确：** 默认 `new_multi_thread`；桥接同步侧用 `new_current_thread().block_on`。  
**反例：** 在同步库里嵌套创建多线程 Runtime 却不说明生命周期。  
**来源：** 13, 17

### await-only-async-io-apis
**规则：** 仅对带 `async` 的 I/O/定时器/channel 等 API 加 `.await`；内存数据结构操作不要乱加。  
**为何重要：** 避免无意义 await，并明确“何时真正让出执行权”。  
**正确：** `tokio::fs::read(...).await`、`interval.tick().await`。  
**反例：** 对 `Vec::push` / `HashMap` 操作写 `.await`。  
**来源：** 13

### use-tokio-async-io-not-std-blocking
**规则：** 在 async 上下文中优先用 Tokio 的 `net`/`fs`/`process`/`time`，不要直接调用会阻塞 OS 线程的 std 同步 I/O。  
**为何重要：** 合作式调度下，阻塞 OS 线程会拖垮同核上其他 task。  
**正确：** `TcpListener::bind(...).await`、`tokio::process::Command`。  
**反例：** 在 `async fn` 里同步 `std::fs::read` / 阻塞 socket。  
**来源：** 13, 14, 17

### spawn-tasks-for-connection-boundaries
**规则：** 每个独立连接/工作单元用 `tokio::spawn` 起 task，accept 循环本身保持运行。  
**为何重要：** 连接级隔离与并发；服务端 accept 循环必须永不退出（除非关机）。  
**正确：** `loop { let (s,_) = listener.accept().await?; tokio::spawn(async move { ... }); }`  
**反例：** 在单个 task 里串行处理所有连接；或 accept 循环因单次错误就退出进程。  
**来源：** 14

### join-handle-for-task-lifetime
**规则：** 需要结果或保证子任务结束时，持有并 `.await` `JoinHandle`；勿假设子 task 随父 task 结束。  
**为何重要：** Tokio 子 task 可活过父 task；`main` 先退出会杀进程/中断子任务。  
**正确：** `let h = spawn(...); let r = h.await?;`  
**反例：** `spawn` 后丢弃 handle，且主流程立刻结束。  
**来源：** 13, 16

### treat-join-as-result-for-panic
**规则：** 对 `JoinHandle` 的 await 结果按 `Result` 处理：task 内 panic 表现为 `Err`。  
**为何重要：** 可能返回错误就必须用 `Result` 包一层，便于上层处理。  
**正确：** `match h.await { Ok(v) => ..., Err(join_err) => ... }`  
**反例：** 无条件 `unwrap` 后假定永远是业务 Ok。  
**来源：** 13

### spawn-starts-immediately
**规则：** 记住 `spawn` 后任务立即调度执行；`JoinHandle.await` 只是等待结束，不是启动开关。  
**为何重要：** 错误理解会导致错误的同步/时序假设。  
**正确：** 先 spawn 多个任务再统一 join/select。  
**反例：** 以为只有 `.await` handle 任务才会开始跑。  
**来源：** 16

### use-framed-codecs-over-raw-bytes
**规则：** 业务层不要手搓 TCP 粘包/拆包；用 `Framed` + Codec（如 `LengthDelimitedCodec`）读写完整消息。  
**为何重要：** 字节流边界问题属于传输层，不应污染业务心智模型。  
**正确：** `Framed::new(stream, LengthDelimitedCodec::new())` + `next()`/`send()`。  
**反例：** 手写 offset 缓冲区、用“猜长度/猜 UTF-8”当帧边界。  
**来源：** 14

### choose-codec-by-protocol
**规则：** 按协议选 Codec：逐字节用 Bytes；文本行用 Lines；自定义分隔用 AnyDelimiter；通用二进制/文本用 LengthDelimited；不够再自定义 Codec。  
**为何重要：** Codec 选型决定正确性与可维护性。  
**正确：** 长度前缀协议用 `LengthDelimitedCodec`。  
**反例：** 大消息仍假设“一次 read 读完”。  
**来源：** 14

### handle-eof-explicitly
**规则：** 网络读返回 `n==0`（EOF）时显式结束该连接 task，勿继续业务写。  
**为何重要：** EOF 表示对端写关闭；忽略会导致空转或错误假设。  
**正确：** `if n == 0 { return; }`  
**反例：** 把 `n==0` 当普通短读继续解析。  
**来源：** 14

### prefer-question-mark-in-async-main
**规则：** 异步入口用 `Result<(), Box<dyn Error>>`（或项目统一错误类型）+ `?` 传播，减少样板。  
**为何重要：** 防御式编程更清晰，错误路径一致。  
**正确：** `async fn main() -> Result<(), Box<dyn std::error::Error>>`  
**反例：** 到处 `unwrap`/`expect` 且无边界说明。  
**来源：** 14

### async-is-infectious-design-boundaries
**规则：** 调用 async 的函数自身也须 async；在模块边界显式划定 async 岛屿，避免无意传染到整棵调用树。  
**为何重要：** 传染性会迫使大面积改签名；边界清晰才能混合同步代码。  
**正确：** 核心 I/O 层 async；纯计算保持同步并在边界桥接。  
**反例：** 仅为调用一个 async 工具而把无关纯函数全改成 async。  
**来源：** 13, 17

### spawn-blocking-for-cpu-or-sync-libs
**规则：** 在 async 主体中执行重 CPU 或不可改的同步库时，用 `spawn_blocking`（或独立线程），勿直接堵在 async worker 上。  
**为何重要：** 防止阻塞 OS 线程导致并发吞吐骤降。  
**正确：** `let r = tokio::task::spawn_blocking(|| heavy()).await?;`  
**反例：** 在 `async fn` 里直接跑长时间 `a_heavy_work()`。  
**来源：** 17

### block-on-to-call-async-from-sync
**规则：** 同步主体要调少量 async API 时，创建局部 Runtime 并 `block_on`，优先 `current_thread` 临时场景。  
**为何重要：** 正确桥接两个王国，避免非法 `.await`。  
**正确：** `Builder::new_current_thread().enable_all().build()?.block_on(foo())`  
**反例：** 在同步函数里硬写 `.await`。  
**来源：** 17

### async-trait-when-needed
**规则：** 在 trait 中需要 async 方法时，使用稳定方案或 `async_trait`（按当前 toolchain）；定义与 impl 两侧标注一致。  
**为何重要：** 历史限制下否则无法在 trait 声明 async fn。  
**正确：** `#[async_trait] trait T { async fn f(); }` 且 impl 同样标注。  
**反例：** 裸写 `trait T { async fn f(); }` 而不处理兼容性。  
**来源：** 17（注：新版 Rust 可能已原生支持，以当前版本为准）

### cooperative-scheduling-yield-at-await
**规则：** 设计长逻辑时在合适的 `.await` 点让出；避免在 task 内长时间无 await 的紧循环。  
**为何重要：** Tokio 是合作式调度，不 await 就不会切换同核任务。  
**正确：** I/O/channel/time 边界 await；超长计算拆分或 `spawn_blocking`。  
**反例：** async task 内百万次纯计算循环无让出。  
**来源：** 13, 17


## Shared state

### avoid-mutable-statics
**规则：** 不要用 `static mut`/可变全局静态共享状态；用局部创建 + 显式传递（`Arc` 等）。  
**为何重要：** 可变全局易数据竞争，且需 unsafe；Rust 明确不推荐。  
**正确：** 在 `main`/App 组装状态再 `clone` 分发。  
**反例：** `static mut DB: Vec<_>` 跨任务写。  
**来源：** 15

### move-into-spawned-tasks
**规则：** `spawn` 的 async 块若捕获外部数据，优先 `async move` 拿走所有权或克隆共享句柄。  
**为何重要：** 子 task 可能活过当前栈帧，借用局部变量不安全。  
**正确：** `spawn(async move { ... arc.clone() ... })`  
**反例：** `spawn(async { use_local_borrow })` 指望外层一定会 await 到结束。  
**来源：** 15

### arc-mutex-as-default-shared-mut
**规则：** 多 task 共享可变数据默认用 `Arc<tokio::sync::Mutex<T>>`（异步锁），锁内完成读改校验。  
**为何重要：** 固定模式可覆盖大部分场景，且编译器保证无数据竞争。  
**正确：** `let db = Arc::new(Mutex::new(vec)); let c = db.clone();` + `lock().await`。  
**反例：** 多 task `async move` 同一 `Vec`；或只用 `Arc` 却要可变。  
**来源：** 15

### prefer-tokio-mutex-in-async
**规则：** 在 `.await` 路径上使用 `tokio::sync::Mutex`，不要跨 await 长时间持有会阻塞调度的同步锁（若必须用 std Mutex，持锁区间不得 `.await`）。  
**为何重要：** 异步中持有阻塞锁会死锁/饿死 worker。  
**正确：** `let g = arc.lock().await; /* 短临界区 */ drop(g);`  
**反例：** `std::sync::Mutex` 锁定后 `.await` 网络请求。  
**来源：** 15（结合 17 阻塞注意点）

### use-rwlock-for-read-heavy
**规则：** 读多写少时用 `tokio::sync::RwLock`：多读并行，写独占。  
**为何重要：** Mutex 读也互斥，读多场景吞吐差。  
**正确：** `lock.read().await` / `lock.write().await`，作用域结束即释放。  
**反例：** 高频只读路径仍用 Mutex。  
**来源：** 15

### use-atomics-for-simple-scalars
**规则：** 共享简单标量（bool/整数计数等）优先 `std::sync::atomic::*`，避免 `Arc<Mutex<u32>>`。  
**为何重要：** 原子类型更轻，可利用硬件原子支持。  
**正确：** `Arc<AtomicU32>` + 合适的 `Ordering`。  
**反例：** 仅为递增计数包一层异步 Mutex。  
**来源：** 15

### short-critical-sections
**规则：** 持锁只做必要读写，校验也在锁内完成需要原子性的部分；尽快释放。  
**为何重要：** 缩小竞争窗口，避免把无关 await 放进临界区。  
**正确：** lock → 改字段 → assert/读 → 释放 → 再 await 其他。  
**反例：** 持锁期间 sleep/网络/长计算。  
**来源：** 15

### prefer-message-passing-when-ownership-fits
**规则：** 若状态可由单一所有者维护，优先 channel + 代理 task，而不是到处加锁。  
**为何重要：** 避免锁竞争与所有权纠结，心智模型更清晰（代理模式）。  
**正确：** workers 发命令，单一 `task_c` 独占 `db` 并串行应用。  
**反例：** 每个 worker 都 `Arc<Mutex<Db>>` 却本来可以单写者。  
**来源：** 15, 16


## Channels

### pick-channel-by-topology
**规则：** 按拓扑选型：多生产者单消费者用 mpsc；一次性请求/响应用 oneshot；多播用 broadcast；单写多读且只关心最新值用 watch。  
**为何重要：** 选型错误会导致丢消息、重复消费或无法表达模式。  
**正确：** 命令队列 mpsc；配置热更新 watch；事件总线 broadcast。  
**反例：** 用 unbounded mpsc 硬撑发布订阅。  
**来源：** 16

### clone-only-senders-for-mpsc
**规则：** mpsc 只 clone `Sender`；保持单一 `Receiver`。  
**为何重要：** MPSC 语义就是多生产者单消费者。  
**正确：** `let tx2 = tx.clone();`  
**反例：** 试图复制多个 rx 当 fan-out（应改 broadcast/watch）。  
**来源：** 16

### prefer-bounded-mpsc-with-backpressure
**规则：** 默认用有界 `mpsc::channel(capacity)`；仅在明确可接受内存风险时用 unbounded。  
**为何重要：** 有界提供背压；无界可能撑爆内存。  
**正确：** `mpsc::channel::<Msg>(100)`。  
**反例：** 生产远快于消费仍用 unbounded。  
**来源：** 16

### handle-send-errors-when-rx-dropped
**规则：** `send().await` 失败要处理（对端已关）；接收端用 `while let Some(x) = rx.recv().await`。  
**为何重要：** rx drop 后继续 send 必失败；None 表示通道关闭。  
**正确：** `if tx.send(v).await.is_err() { return; }`  
**反例：** 忽略 send 错误或假设 recv 永不结束却不关通道。  
**来源：** 16

### mpsc-plus-oneshot-for-request-response
**规则：** 在请求消息中携带 `oneshot::Sender<Reply>`，处理方回发结果，实现进程内 Req/Response。  
**为何重要：** 固定、类型安全的 RPC 模式，无需外部消息队列。  
**正确：** `mpsc::channel::<(Req, oneshot::Sender<Resp>)>(n)`。  
**反例：** 另起全局队列只为一次回调。  
**来源：** 16

### drop-senders-to-close-receivers
**规则：** 需要结束接收循环时，确保所有 sender 被 drop（或显式关闭），否则 `recv` 会永久等待。  
**为何重要：** 忘记关通道会导致程序挂起（虽不忙等，但无法退出）。  
**正确：** 限制 tx 生命周期；关闭时 drop 所有 clone。  
**反例：** 代理 task 永久 `while let Some`，而发送方已结束却仍有 tx 存活/或未规划退出。  
**来源：** 16

### join-all-vs-select-first
**规则：** 要全部结果用 `join!` / 收集 JoinHandle；要“谁先完成用谁”用 `select!`，并明确取消/忽略其余任务的策略。  
**为何重要：** 两种搜集模式语义不同，混用导致延迟或资源泄漏。  
**正确：** 全等齐用 `tokio::join!`；竞速用 `tokio::select!`。  
**反例：** 顺序 await 一长串 handle 却期望“最快的先返回业务结果”。  
**来源：** 16

### store-join-handles-then-await
**规则：** 批量任务：先 `Vec` 存全部 `JoinHandle`，再统一 await/搜集；spawn 阶段不要串行 await。  
**为何重要：** 保证真正并发启动。  
**正确：** `for op in ops { tasks.push(spawn(...)); } for t in tasks { outs.push(t.await?); }`  
**反例：** `spawn` 后立刻 `await` 再 spawn 下一个。  
**来源：** 13, 16


## Web / Axum

### stay-on-tokio-stack-for-web
**规则：** Web 后端优先 Axum + Tower/Tower-http + Hyper/Tokio 技术栈，中间件复用 Tower Layer，勿自造平行中间件体系。  
**为何重要：** 与生态共享中间件，可预测、可组合。  
**正确：** `Router` + `.layer(TraceLayer::...)` 等。  
**反例：** 在 Axum 旁再写一套不兼容的中间件抽象。  
**来源：** 21

### router-nest-for-modules
**规则：** 用 `Router::nest` 按模块分层 URL，全局与模块路由分离。  
**为何重要：** 模块化、可预测的路由树。  
**正确：** `/api` nest `/users`、`/teams`。  
**反例：** 单文件堆砌全部扁平路由无边界。  
**来源：** 21

### handlers-are-async-extractors
**规则：** Handler 写成 async fn：参数为 Extractor，返回 `impl IntoResponse`（或具体响应类型）。  
**为何重要：** 声明式解析请求，减少样板并类型化。  
**正确：** `async fn h(Query(p): Query<P>) -> impl IntoResponse`  
**反例：** 手写从 Raw Request 抠字段的重复代码（无充分理由）。  
**来源：** 21, 22

### serde-typed-request-dtos
**规则：** Query/Form/Json 参数用带 `Deserialize` 的结构体；可选字段用 `Option<T>`。  
**为何重要：** 类型驱动解析，错误尽早在边界暴露。  
**正确：** `struct Input { foo: i32, bar: String, third: Option<i32> }`  
**反例：** 全部当 `HashMap<String, String>` 再手工转换。  
**来源：** 21, 22

### extractor-order-body-last
**规则：** 多 Extractor 时顺序可按意图排列，但会消耗 body 的（Json/Form/body）必须放在最后。  
**为何重要：** body 只能读一次；顺序错会导致后续 extractor 失败。  
**正确：** `Path`/`State`/`Query` 在前，`Json` 最后。  
**反例：** `Json` 放在仍需读 body 的 extractor 之前。  
**来源：** 答疑（三）/22

### handle-rejection-explicitly-when-needed
**规则：** 需要自定义校验错误时，用 `Result<Json<T>, JsonRejection>`（或对应 Rejection）分支处理。  
**为何重要：** 默认自动拒绝不够灵活；业务要可控错误响应。  
**正确：** match MissingContentType / JsonDataError / … + catch-all（non_exhaustive）。  
**反例：** 依赖默认 4xx 文案却无法对前端约定错误码。  
**来源：** 22

### into-response-unify-diverging-returns
**规则：** 同一 handler 返回多种响应形态时，各分支 `.into_response()`，保持返回类型一致。  
**为何重要：** `impl IntoResponse` 仍是单态具体类型；分歧类型需统一。  
**正确：** `Json(x).into_response()` / `Redirect::to(...).into_response()`  
**反例：** if/else 直接返回不同类型而不转换。  
**来源：** 22

### result-status-tuple-for-handler-errors
**规则：** 业务 handler 用 `Result<(StatusCode, Json<T>), (StatusCode, String)>`（或统一错误类型）表达成功/失败。  
**为何重要：** 错误在异步 Web 边界可映射为 HTTP 状态，传播路径清晰。  
**正确：** DB 失败 → `(StatusCode::INTERNAL_SERVER_ERROR, msg)`。  
**反例：** handler 内 `unwrap` 直接 panic 成 500 无上下文。  
**来源：** 22

### app-state-via-with-state
**规则：** 共享依赖（连接池等）放进 `AppState`，`Router::with_state`，handler 用 `State<T>` 提取。  
**为何重要：** 全局共享的标准模式，避免隐藏全局变量。  
**正确：** `struct AppState { dbpool: Pool }` + `State(pool)`。  
**反例：** handler 内每次新建连接或读静态全局。  
**来源：** 22

### use-connection-pool
**规则：** DB 访问经连接池（如 bb8），不要为每请求裸建连。  
**为何重要：** 复用连接、处理断开重连等基础设施问题。  
**正确：** 启动时 `Pool::builder().build(manager)` 注入 State。  
**反例：** handler 里 `connect` 完即丢。  
**来源：** 22

### separate-domain-and-dto-types
**规则：** 区分领域模型（如 `Todo`）与入参 DTO（`CreateTodo`/`UpdateTodo`）；更新字段用 `Option` 表示可省。  
**为何重要：** API 边界与存储模型解耦，部分更新语义清晰。  
**正确：** create 只收 description；update 收 Option 字段。  
**反例：** 一个巨型 struct 既当 DB row 又当所有 API 入参。  
**来源：** 22

### design-routes-before-impl
**规则：** 先定 schema → Rust 类型 → Router endpoints → handler 签名 → 交互格式（JSON/Form）→ 再实现。  
**为何重要：** 函数签名即目录，降低返工。  
**正确：** 先写齐 4 个 handler 签名再填身体。  
**反例：** 边写 SQL 边改路由无总体设计。  
**来源：** 22

### tracing-not-println
**规则：** 生产代码用 `tracing`（+ subscriber）与 `TraceLayer`；用 `RUST_LOG` 控级别；禁止靠 `println!` 当日志。  
**为何重要：** 异步场景下结构化、可过滤、可关联请求。  
**正确：** `tracing_subscriber::fmt::init()` + `tracing::debug!(...)`。  
**反例：** 服务里到处 `println!`。  
**来源：** 21

### layer-middleware-at-right-scope
**规则：** Tower 中间件按需挂在全局 Router、子 Router 或单个路由；常用 Trace/Cors/Compression/Timeout/RequestId/HandleError。  
**为何重要：** 作用域过大会拖累无关路径，过小会漏防护。  
**正确：** 全站 Trace；仅 API 子树 Timeout。  
**反例：** 静态资源也套沉重业务中间件无理由。  
**来源：** 21

### type-checked-templates-when-ssr
**规则：** 服务端模板优先类型驱动引擎（如 Askama），让模板错误在编译期暴露。  
**为何重要：** 减少运行时调页面时间。  
**正确：** `#[derive(Template)] struct HelloTemplate { name: String }`  
**反例：** 纯字符串拼接 HTML 无校验。  
**来源：** 22

### fallback-for-404
**规则：** 为 Router 配置 `fallback`/`fallback_service`，统一 404 或 SPA 回退。  
**为何重要：** 未匹配路由行为可预期。  
**正确：** `app.fallback(handler_404)`。  
**反例：** 依赖框架默认裸 404 且无产品页。  
**来源：** 21, 22


## Parser / Nom

### parser-as-typed-function
**规则：** 把 Parser 建成 `Fn(Input) -> IResult<I, O>`：成功返回 `(rest, output)`，失败返回 Nom 错误。  
**为何重要：** 统一签名才能组合与测试。  
**正确：** `fn p(i: &str) -> IResult<&str, T>`  
**反例：** 解析函数直接 `unwrap` 字符串切片无剩余输入。  
**来源：** 28

### compose-small-parsers
**规则：** 先写小 parser（tag/digit/…），再用 combinator（`tuple`/`delimited`/`separated_pair`/`alt`/`map_res`）组装大 parser。  
**为何重要：** 可复用、可单测、代码接近数据结构。  
**正确：** `delimited(tag("("), parse_pair, tag(")"))`  
**反例：** 单函数内手写全部字符状态机且不可拆。  
**来源：** 28

### recursive-descent-decomposition
**规则：** 按递归下降拆问题：大 pattern → 子 pattern → 最小单元 → 组装领域类型。  
**为何重要：** 组合子思想的核心，复杂协议可维护。  
**正确：** `"#RRGGBB"` → `#` + 三次 `hex_primary` → `Color`。  
**反例：** 一上来写整文件巨型正则且无法局部测试。  
**来源：** 28

### zero-copy-and-explicit-errors
**规则：** 尽量零拷贝解析（输出借用输入切片），并用 Nom 错误路径规范处理失败用例。  
**为何重要：** 性能与安全解析是 Nom 的卖点。  
**正确：** 对非法输入 `assert!(parse(...).is_err())`。  
**反例：** 解析失败时静默返回默认值。  
**来源：** 28

### unit-test-parsers-in-isolation
**规则：** 每个小 parser 写独立单元测试；组合后再测端到端。  
**为何重要：** 组合子优势之一是组件可测。  
**正确：** `#[test] fn parse_color() { ... }`  
**反例：** 只在整应用手工跑一条样例。  
**来源：** 28

### prefer-combinators-over-ad-hoc-string-ops-for-protocols
**规则：** 对协议/结构化文本，优先 Nom（或同类）而非散落的 `find/split`；极简情况才用 String/正则。  
**为何重要：** 协议会变复杂；组合子可演进到流式/二进制。  
**正确：** CSV/坐标/色值用 separated_list / delimited。  
**反例：** 网络协议用多次 `split(',')` 且无剩余输入概念。  
**来源：** 28, 答疑（三）


## FFI / Interop（跨运行时、UI、语言边界）

### keep-heavy-work-off-ui-thread
**规则：** GUI 主循环线程只做渲染与轻逻辑；LLM/YOLO 等重活放到后台 `std::thread`（或专用池），经 channel 通信。  
**为何重要：** 重活堵 UI 会卡顿；框架甚至可能杀掉长回调。  
**正确：** UI → mpsc → worker；结果 `invoke_from_event_loop` 回写。  
**反例：** 在按钮回调里同步跑模型推理。  
**来源：** 25, 26, 答疑（三）

### update-ui-only-on-ui-thread
**规则：** 后台线程禁止直接摸 UI 对象；必须投递到 UI 事件循环再改属性。  
**为何重要：** 所有权/线程安全；否则竞争或编译/运行失败。  
**正确：** `slint::invoke_from_event_loop(move || ui.set_...)`  
**反例：** worker 里直接 `ui.set_dialog(...)`。  
**来源：** 25, 26

### weak-handles-across-threads
**规则：** 跨线程持有 UI 用弱引用（`as_weak`），在事件循环回调里再 upgrade。  
**为何重要：** 避免循环引用与悬垂生命周期。  
**正确：** `let ui_handle = ui.as_weak();` 传入线程。  
**反例：** 把强引用 UI 迁入长期线程不管窗口生命周期。  
**来源：** 25, 26

### signal-worker-shutdown-on-exit
**规则：** 窗口关闭时通过 channel 发退出哨兵，结束后台 loop；不要对永不结束的 worker `join` 堵死 UI。  
**为何重要：** 双 loop 应用需协同退出，否则泄漏或强制杀进程。  
**正确：** `on_close_requested` → `send("_exit_")`。  
**反例：** 主界面关了 worker 仍阻塞在 `recv`。  
**来源：** 25, 26

### top-level-properties-as-ui-api
**规则：** UI 与 Rust 的交互收敛到顶层 component 的 property/callback，而不是深入查找子控件句柄。  
**为何重要：** 扁平、可测试的边界；符合声明式 UI 范式。  
**正确：** `in-out property` + `callback`，Rust `on_*` / `set_*`。  
**反例：** 在 Rust 里按控件树到处找 child。  
**来源：** 25, 26

### map-string-types-at-ffi-boundary
**规则：** 跨 UI/FFI 边界显式转换字符串（如 Slint string ↔ Rust `String`：`.to_string()` / `.into()`）。  
**为何重要：** 不同类型系统不能隐式混用。  
**正确：** 边界函数内集中转换。  
**反例：** 假设两种 string 可直接当同一类型传来传去。  
**来源：** 26；答疑（三）对 C/Rust 字符串亦同理

### c-string-maps-to-cstr
**规则：** C 的 `char*` 在 Rust 侧映射为 `CStr`/`CString` 等 FFI 类型，勿当 Rust `String`/`char` 混用。  
**为何重要：** C `char` 是字节；Rust `char` 是 Unicode scalar（4 字节）；字符串模型不同。  
**正确：** 边界用 `std::ffi::CStr`。  
**反例：** 把 C 指针当 UTF-8 `String` 无校验。  
**来源：** 答疑（三）/30（规范相关）

### convert-cli-engine-to-library-api
**规则：** 把 clap 驱动的引擎改造成库函数：`Args` 可变结构体/参数，由 UI 或其他前端注入；`main` 只做薄封装。  
**为何重要：** 同一引擎可被 CLI/GUI/服务复用。  
**正确：** `start_engine(task, model, path) -> Result<PathBuf>`。  
**反例：** GUI 通过再起一个子进程 CLI 凑合（无充分理由）。  
**来源：** 26

### cache-expensive-model-loads
**规则：** 模型等大资源在启动或首次后缓存，避免每次点击重复加载。  
**为何重要：** 加载成本高，直接决定交互延迟。  
**正确：** UI 就绪回调加载一次，推理只调已加载实例。  
**反例：** 每次 Detect 都重新读 safetensors。  
**来源：** 答疑（三）/26

### feature-flags-for-accelerators
**规则：** CUDA/Metal 等加速用 Cargo features 开关，设备选择逻辑集中（CPU 回退路径明确）。  
**为何重要：** 跨平台可编译、可部署。  
**正确：** `get_device(cpu)` + `--features cuda|metal`。  
**反例：** 硬编码某 GPU API 导致其他平台无法构建。  
**来源：** 24, 答疑（三）


## Project / architecture

### type-driven-design
**规则：** 优先用类型表达不变量与场景信息（Duration、DTO、枚举任务、NewType），让误用在编译期失败。  
**为何重要：** “类型化 = 纳入更多规范信息”，减少运行时出错。  
**正确：** `Duration::from_millis(10)`；`enum YoloTask { Detect, Pose }`。  
**反例：** 到处 `i64` 魔法数表示毫秒/状态码。  
**来源：** 13, 22, 24, 结束语

### trait-over-deep-oop-hierarchies
**规则：** 用 trait 做能力/约束组合，而非深继承树；平等看待标准库与自定义 trait。  
**为何重要：** Rust 的扩展与复用模型是平铺组合。  
**正确：** `trait Task: Module + Sized { fn load...; fn report...; }` + turbofish `run::<YoloV8>`。  
**反例：** 为复用强行模拟庞大类继承。  
**来源：** 24, 27, 结束语

### flatten-state-with-composition
**规则：** 复杂领域用组合铺平数据（ECS 式 Component/Resource，或小结构体组合），行为放在小函数/system。  
**为何重要：** 复用属性集、降低耦合；与 trait 哲学一致。  
**正确：** Component 标记 + Query 取数；Resource 单例。  
**反例：** 巨型 God Object 拥有所有字段与方法。  
**来源：** 27, 结束语

### one-system-one-responsibility
**规则：** 每个 system/模块函数只做一件事；跨关注点用 Event 解耦。  
**为何重要：** 复杂系统可演进，心智负担低。  
**正确：** `snake_eating` 发 `GrowthEvent`，`snake_growth` 订阅。  
**反例：** 一个函数里吃食物、长大、碰撞、刷新 UI 全干。  
**来源：** 27

### explicit-system-ordering
**规则：** 有依赖的更新用 `.before`/`.after`（或等价调度）声明顺序。  
**为何重要：** 帧内竞态/顺序 bug 难查。  
**正确：** `input.before(movement)`；`game_over.after(movement)`。  
**反例：** 依赖隐式注册顺序。  
**来源：** 27

### resources-for-singletons
**规则：** 全局唯一状态用 Resource（或 AppState），并在变更点维护一致性（增删都更新索引结构）。  
**为何重要：** 单例语义清晰；避免重复生成等逻辑 bug。  
**正确：** 食物位置 `HashSet` Resource，吃掉时清理。  
**反例：** 到处散落静态变量描述同一资源。  
**来源：** 27, 答疑（三）

### module-boundaries-by-engine
**规则：** 可执行前端（CLI/GUI）与核心引擎分模块；引擎目录可直接从原 bin 重构为 `mod`。  
**为何重要：** 复用与测试；保持 crate 边界清晰。  
**正确：** `src/yolov8engine/{mod,model,...}` + 薄 `main`。  
**反例：** 所有逻辑堆在 `main.rs`。  
**来源：** 26

### multi-bin-in-cargo-toml
**规则：** 多入口用 `[[bin]]` 声明 path；`cargo run --bin name`。  
**为何重要：** server/client 等分离清晰。  
**正确：** `server.rs` / `client.rs` 各一 bin。  
**反例：** 注释切换 main 内容。  
**来源：** 14

### clap-for-real-clis
**规则：** 非玩具 CLI 用 clap 派生参数（默认值、help、枚举 ValueEnum）；配置不要散落魔法字符串。  
**为何重要：** 生产力与可发现性。  
**正确：** `#[derive(Parser)] struct Args`。  
**反例：** 长期维护项目只靠 `env::args().nth(1)`。  
**来源：** 14, 23, 24

### anyhow-at-app-boundaries
**规则：** 应用/示例边界可用 `anyhow::Result` 降低错误类型噪音；库核心仍应考虑精确错误类型。  
**为何重要：** 边界减轻心智负担；库要可组合。  
**正确：** `main/run -> anyhow::Result<()>`。  
**反例：** 库的每个公开 API 都抹成 anyhow 且丢失错误语义。  
**来源：** 23, 24

### prefer-release-for-compute-workloads
**规则：** 推理/图像等计算密集路径用 `--release` 评测与交付。  
**为何重要：** Debug 性能会误导优化结论。  
**正确：** `cargo run --release -- ...`  
**反例：** 用 debug 构建判断模型“太慢”并大改架构。  
**来源：** 23, 24

### single-static-binary-deployment-mindset
**规则：** 交付物追求少依赖单二进制；大资源（模型）与代码分离、路径可配置。  
**为何重要：** 相对 Py+C++ 堆依赖，Rust 部署优势要保持。  
**正确：** 模型路径 CLI/配置注入；二进制可拷贝运行。  
**反例：** 运行时假设固定相对路径且无文档。  
**来源：** 23, 26


## General engineering（应用章可迁移）

### ownership-first-concurrency
**规则：** 并发方案必须先满足所有权（move/`Arc`/消息），编译通过后再谈性能；不要用 unsafe 全局绕过。  
**为何重要：** 编译器拒绝的是数据竞争；过关则杜绝一类概率性线上 bug。  
**正确：** 按编译器提示改成 `Arc<Mutex<_>>` 或 channel 代理。  
**反例：** `static mut` + unsafe 沉默竞争。  
**来源：** 15, 结束语

### result-for-fallible-paths
**规则：** 可能失败就返回 `Result`；可能缺失用 `Option`；在边界 `?` 传播，在最终边界映射用户错误。  
**为何重要：** Rust 标准错误哲学；异步 task/网络/解析皆同。  
**正确：** 解析/IO/join 全链路 Result。  
**反例：** 库函数 `unwrap` 期望“调用方保证不会坏”。  
**来源：** 13, 14, 22, 28, 结束语

### incremental-vertical-slices
**规则：** 大目标拆步骤：先最小可运行竖切，再叠加日志、解析、状态、DB；每步可验证。  
**为何重要：** 似慢实快，比一上来写满功能更稳。  
**正确：** Axum hello → static → trace → query → form → json → db → todo。  
**反例：** 第一周同时上 ORM、鉴权、微服务拆分。  
**来源：** 21, 22

### request-response-as-core-model
**规则：** 网络服务优先清晰的请求/响应（或在其上扩展流式）；handler/Service 保持 `Request -> Result<Response, Error>` 形态。  
**为何重要：** Tower/Axum/多数 RPC 的统一抽象。  
**正确：** Tower `Service`；Axum handler。  
**反例：** 无帧协议的裸 TCP 业务与 HTTP 服务混层。  
**来源：** 21, 答疑（三）

### events-for-decoupling
**规则：** 运行时内跨子系统通知优先 Event（或 channel 消息），避免直接互相调用造成环依赖。  
**为何重要：** 并行任务通信与解耦。  
**正确：** 写者 `EventWriter`，读者 `EventReader`。  
**反例：** A system 直接调用 B 的内部可变全局。  
**来源：** 27

### commands-to-mutate-hosted-world
**规则：** 在托管 Runtime（游戏引擎/类似世界状态）中，通过 Commands/消息变更世界，而不是外部持有裸指针乱改。  
**为何重要：** 生命周期与调度安全由 Runtime 托管。  
**正确：** `commands.spawn` / `despawn`。  
**反例：** 缓存 Entity 指针跨帧解引用无校验。  
**来源：** 27

### encode-protocol-not-business-in-transport
**规则：** 传输层解决帧/编码；业务层只处理完整消息类型。  
**为何重要：** 复杂度分层，避免业务被 TCP 细节缠死。  
**正确：** LengthDelimited → `String`/`Bytes` 指令 → `process()`。  
**反例：** 业务函数接收半包缓冲并自己拼。  
**来源：** 14

### agent-use-fixed-patterns
**规则：** 优先套用固定模式（三板斧 `Arc+Mutex+clone`、mpsc 代理、mpsc+oneshot RPC、Axum State+Extractor、Nom 组合），不要每次发明新并发架构。  
**为何重要：** 模式固定则心智负担低、评审成本低。  
**正确：** 先选表内模式，再证明需要定制。  
**反例：** 为小工具引入多层 actor 框架。  
**来源：** 15, 16, 22, 28

### learn-two-cores-then-compose
**规则：** 设计与重构时抓住所有权三态（拥有/共享借用/独占借用）与 trait 能力体系；其余特性挂接其上。  
**为何重要：** 结束语总结的学习与设计主绳。  
**正确：** 先问“谁拥有？谁可变？能力用哪个 trait？”  
**反例：** 先堆宏/生命周期标注绕过模型问题。  
**来源：** 结束语

### move-vs-copy-intentionally
**规则：** 大块数据默认 Move；仅对小且可复制类型依赖 Copy；共享用 `Arc`，独占堆资源用 `Box`。  
**为何重要：** 控制拷贝成本与所有权清晰度。  
**正确：** 任务间传 `Arc<Model>`；不要 Clone 大模型权重。  
**反例：** 对大 `Vec`/`String` 频繁按 Copy 思维复制。  
**来源：** 结束语（结合 15/23）

### raII-and-scope-based-resources
**规则：** 资源获取即初始化：用拥有型值管理堆资源，依赖作用域释放；跨异步边界想清何时 drop。  
**为何重要：** 相对手动 free/GC，这是 Rust 资源安全根基。  
**正确：** 连接/文件/锁守卫离开作用域即释放。  
**反例：** 忘记 drop 锁守卫或延长守卫跨越 await。  
**来源：** 结束语, 15


## 速查：场景 → 首选模式

| 场景 | 首选 |
|------|------|
| 异步服务入口 | Tokio multi-thread + `#[tokio::main]` |
| 多 task 改同一数据 | `Arc<tokio::sync::Mutex<T>>` 或 mpsc 单写者代理 |
| 读多写少 | `RwLock` |
| 请求带应答 | mpsc + oneshot |
| 配置热更新/关机信号 | watch |
| TCP 业务消息 | Framed + LengthDelimited（或合适 Codec） |
| CPU/同步库 in async | `spawn_blocking` |
| sync 调少量 async | `current_thread` + `block_on` |
| HTTP API | Axum Extractor + State + `IntoResponse` |
| 结构化文本/协议 | Nom 小 parser 组合 |
| GUI + 重计算 | 后台线程 + channel + UI 线程回投递 |
| 游戏/仿真世界 | ECS：Component + System + Event + Resource |
