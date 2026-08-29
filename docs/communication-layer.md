# 通信层（已实现）

对应小册第 11–13 章，Rust 顺序是**先 TCP 再 WebSocket**（TCP 没有 HTTP 升级，更容易看清抽象）。

源码：

- 说明书：`crates/kim-core/src/`
- TCP：`crates/kim-tcp/src/`
- 验收：`crates/kim-tcp/tests/echo.rs`（TCP）、`crates/kim-ws/tests/echo.rs`（WS）。没有独立 echo 二进制。

## 这一层解决什么问题

以后会有三种线：浏览器↔网关（WS）、App↔网关（TCP）、网关↔Chat（内网 TCP）。监听、拆包、心跳、断开如果写三遍，业务会烂。通信层让上层只说：

- 服务端：`start` / `push(channel_id, 字节)`
- 客户端：`connect` / `send` / `read`

协议差异闷在 `Conn` 里。

## 链路图（先看图）

下面几张图说的都是**已经落地的服务端 Channel**。`TcpClient` 写侧还是 Mutex，不要对号入座。  
图是纯文本，打开文件就能看，不用渲染。

### 1. 这一层在整座楼里的位置

```text
业务 EchoHandler          Accept / Receive / Disconnect
        │                      ▲
        │ push（拿信箱）         │ receive（被读专员喊）
        ▼                      │
kim-core  ChannelMap ──clone──► 写信箱 ──► 写专员 ──► 写半边 ──┐
              │     立刻放锁         ▲                         │
              │                     │ Ping 也扔进来              ▼
              └──────────────► 读专员 ──► 读半边 ────────► 内核 TCP ◄──► Alice 客户端
```

业务只碰到 Handler 和信箱。插座和内核在两专员下面。

### 2. 一条连接上的两个专员（不对称）

```text
张三 push ─┐
李四 push ─┼──► Alice 的写信箱（mpsc）──► 写专员（唯一 write）──► 网线 ──► Alice
心跳 Pong ─┘         ▲
                     │
                     │  Channel 可 clone，所有人持有的是信箱，不是专员本人
                     │
Alice 打字 ──► 网线 ──► 读专员（唯一 read）──► receive（业务）
                                      └── Ping 也丢给写信箱
```

写是 **N 对 1**（投递）；读是 **1 对 1**（监听）。

### 3. 查表为什么必须先 clone 再放锁

```text
张三要发给 Alice
    │
    ├─ 1. 拿 ChannelMap 读锁，get("alice")
    ├─ 2. clone 出 Channel（里面是信箱）
    ├─ 3. 立刻放掉表锁     ← 别人这时可以登录 / 断线 / 查表
    ├─ 4. push 进写信箱    ← 短同步，只管入队顺序
    └─ 5. 写专员按 FIFO 取出，独自 write 插座

表锁 护字典
信箱 护「发给 Alice 的顺序」
专员 护插座
三件事不要握成一把大锁。
```

### 4. echo 一条消息怎么走

```text
测试里的 TcpClient            服务端
    │
    │  connect + 第一帧 "alice"
    │─────────────────────────────────────────► Acceptor 读出名字
    │                                           ChannelMap.add
    │                                           拆读写半边，拉起两专员
    │
    │  帧 Binary "hello 0"
    │─────────────────────────────────────────► 读专员拆帧
    │                                           receive(EchoHandler)
    │                                           push "hello 0 from server"
    │                                           写信箱 ──► 写专员 ──► write
    │  read 到回声
    │◄─────────────────────────────────────────
```

业务只出现在 Accept 和 receive。中间全是通信层。

## 应用层帧（我们画的边界）

TCP 没有「一条消息」的边界。`kim-tcp` 使用：

```text
| opcode 1 字节 | length 4 字节小端 | payload N 字节 |
```

`opcode` 与 WebSocket 对齐：Binary / Close / Ping / Pong。  
编解码：`crates/kim-tcp/src/codec.rs`。单测覆盖半包、粘包、超大包拒绝。

这是**应用层帧**，不是 TCP 段，也不是 IP 包。内核仍按自己的 MSS/窗口把数据切成段。

## Trait 谁调用谁

```text
TcpServer::start
    accept 一条内核 TCP 连接
    Acceptor::accept(conn)  → 业务返回 channel_id，失败则断开
    Channel::pair           → 读半边给读循环，写半边给写协程
    ChannelMap.add
    读循环：
        Ping  → 自己回 Pong（业务看不见）
        Close → 结束
        Binary → MessageListener::receive(Agent, payload)
    断开：
        ChannelMap.remove
        StateListener::disconnect
```

| Trait | 谁实现 | 谁调用 | 干什么 |
|---|---|---|---|
| `Conn` | `kim-tcp`、`kim-ws` | 握手、Channel 读写 | 读/写一帧 |
| `Acceptor` | 业务（现在 EchoHandler） | TcpServer 接进来之后 | 这是谁 |
| `MessageListener` | 业务 | Channel 读循环 | 收到业务字节 |
| `StateListener` | 业务 | 连接结束时 | 清理 |
| `Agent` | Channel 内部 | 业务在 receive 里 | 只能 id + push，不能 Close |
| `Dialer` / `TcpDialer` | 业务或 IdentityDialer | Client::connect | 拨号 + 握手 |
| `Server` / `Client` | `TcpServer` / `TcpClient` | example 的 main | 启动、推、收 |

业务插槽故意瘦：`Agent` 不暴露关连接，避免 Handler 误把别人踢下线。关连接是通信层的事。

取 `ChannelMap` 时：**先 clone 出 Channel，再 await 写网络**，不要握着整张表的锁等 IO。

## 读写拆分：两专员，锁在桌子上（已落地）

小册 demo 用一把大锁包住「遍历连接表 + `WriteFrame`」。真正要防的不是「一条连接又能读又能写」（TCP 全双工，读写同时进行没问题），而是：

- **两个写**交错：帧头和内容搅在一起
- **两个读**互偷字节

服务端 `Channel` **已经**按「专员」模型实现，不是还没做到。图见上文第 2 张。源码：`crates/kim-core/src/channel.rs`、`channel_map.rs`，握手后 `TcpConn::into_split`。

- **写**：N 对 1。谁都可以 `push`，顺序 = **入队顺序**。真正碰插座的只有写协程。
- **读**：1 对 1。读专员自己守网线，有数据再喊业务。别人不持有「读信箱」去使唤他。

「不加锁」指的是：**不再握着互斥锁做整段网络 syscall**。同步还在，只是挪到写专员的 FIFO 桌子上（入队/出队，临界区很短）。桌子满了 `push` 失败，这是反压，防止慢连接撑爆内存。

两把不同的锁，不要混：

| 在哪 | 保护什么 | 现在怎么做 |
|---|---|---|
| `ChannelMap` 的 `RwLock` | 字典：名字→Channel | **还在**。查到 clone 立刻放锁 |
| 写专员的 `mpsc` | 多个人往同一插座投递的顺序 | 已落地。插座上无写写竞争 |
| 读循环 | 谁从插座 read | 每个连接一个任务，无多读者 |

`MessageListener` 在 `start` 前登记，读循环跑起来后不热替换，所以「注册 Receiver」没有运行时锁。我们的 `receive` 在读循环里 `await`（同一连接串行），不需要为「同时喊两次业务」加锁。小册若 `go Receive`，那是业务层自己要线程安全。

尚未对齐的一点：**`TcpClient` 写侧**仍是 `Mutex` 包着写半边（自己 `send` + 心跳 Ping）。客户端写并发少，先这样。服务端 Channel 才是完整的「信箱 + 写专员」。

## echo 执行链（已跑通）

以 `crates/kim-tcp/tests/echo.rs` 的回声发第一条 `hello 0`：

1. Server `bind` 占用端口，内核开始 listen。  
2. Client `connect`：内核三次握手；`IdentityDialer` 发第一帧，内容是 `"alice"`。  
3. `EchoHandler::accept` 读出名字，当作 channel_id。  
4. 连接拆成读写两半，进入 ChannelMap。  
5. Client `send("hello 0")` → 编码成帧 → 内核 TCP 送达。  
6. 读循环拆出 Binary 帧 → `receive` → 拼上 ` from server` → `agent.push`。  
7. 写协程出队、写帧；Client `read` 打印 `hello 0 from server`。  
8. 断开则 `Disconnect("alice")`。

业务只出现在 3、6、8。中间全是通信层。JWT 登录在 `examples/gateway` 的 Handler，**不**进 `TcpServer` / `WsServer`。crate 测试里的 EchoHandler 仍用第一帧名字。

## 心跳

- 客户端按间隔发 `Ping`（`ClientOptions.heartbeat`）。  
- 服务端读循环见 Ping 回 Pong，并靠 `read_wait` 判断对面是不是死了。  
- `read_wait` 必须大于心跳间隔，否则活连接会被误判超时。

## 内核 TCP 和「改操作系统」

`kim-tcp` **使用**内核 TCP，**不实现**拥塞控制。应用层以后会动的是：

- 缓冲合并、writev、`TCP_NODELAY`、连接数、超时  
- 再往后：io_uring、`SO_REUSEPORT`  

改内核、换拥塞算法、用户态协议栈，是流量和尾延迟证明瓶颈在内核之后的事，大厂才养专门团队。本项目把合同留在 `Conn`：谁履行可靠有序有边界都可以换；默认履行者是内核 TCP。

## 怎么验收通信层没坏

```bash
cargo test -p kim-tcp --test echo
cargo test -p kim-ws --test echo
```
