# 学习文档

这是本仓库自己沉淀的笔记，不是小册原文。这里只记：

- 当前代码真正长什么样
- 分层和 trait 的合同（业务可以假设什么、不可以假设什么）
- 容易记混的词

## 阅读顺序

先打开交互图（浏览器打开 HTML），再读合同：

1. [总图](diagrams/kim-overview.html) — 本机三进程 + crate 分层
2. [业务包 Demo 时序](diagrams/pkt-demo.html) — 握手、本地 ping、echo 往返
3. [帧与业务包数据流](diagrams/protocol-stack.html) — 字节如何变成 command
4. [glossary.md](glossary.md) — 进程、端口、长连接、帧……后台入门词
5. [architecture.md](architecture.md) — crate 职责、进门怎么走
6. [communication-layer.md](communication-layer.md) — 通信层。先看「链路图」（纯文本）
7. [protocol-container.md](protocol-container.md) — 已落地的 WebSocket、业务包、容器规格
8. [link-layer-login.md](link-layer-login.md) — **M3 已落地**：指令 Router + JWT 登录 + 会话 + 互踢

卡住时：先看文档里的「合同」和「执行链」，再去对应源码。图和文字打架时，以代码为准。

## 文档怎么保持不腐烂

- **只写已经落地或已经拍板的设计。** 尚未实现的标成「以后」。
- 改了 trait、帧格式、分层，**同一轮把这里改掉**。文档和代码打架时，以代码为准。
- 不复制付费小册章节。

## 其它

| 路径 | 是什么 |
|---|---|
| [diagrams/](diagrams/) | 总图 / 时序 / 数据流的 HTML 与 JSON 规格 |
| [research/rust-kim-replica-feasibility.md](../research/rust-kim-replica-feasibility.md) | 能不能用 Rust 复刻、库怎么选 |
