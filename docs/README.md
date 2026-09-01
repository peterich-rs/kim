# 学习文档

这是本仓库自己沉淀的笔记，不是小册原文。这里只记：

- 当前代码真正长什么样
- 分层和 trait 的合同（业务可以假设什么、不可以假设什么）
- 容易记混的词

## 阅读顺序

1. [glossary.md](glossary.md) — 进程、端口、长连接、帧……后台入门词
2. [architecture.md](architecture.md) — crate 职责、进门怎么走
3. [communication-layer.md](communication-layer.md) — 通信层。先看「链路图」（纯文本）
4. [protocol-container.md](protocol-container.md) — 已落地的 WebSocket、业务包、容器规格
5. [link-layer-login.md](link-layer-login.md) — 已落地的登录、会话、互踢
6. [control-layer-chat.md](control-layer-chat.md) — 已落地的在线单聊 / 群聊
7. [reliable-delivery.md](reliable-delivery.md) — 已落地的 ACK / 写扩散 / 离线 Pull
8. [group-royal.md](group-royal.md) — 已落地的群 join/quit/detail 与可选 Royal HTTP
9. [web-sdk.md](web-sdk.md) — 已落地的 TypeScript Web SDK
10. [user-social-inbox.md](user-social-inbox.md) — 资料 / 好友 / 服务端会话
11. [mobile-client.md](mobile-client.md) — kim-client + Flutter 壳（WSS / WGateway）
12. [media.md](media.md) — R2 图床（upload Worker + 自定义域读）
13. [bench.md](bench.md) — kimbench
14. [perf.md](perf.md) — 写路径 / 缓冲 / 寻址
15. [routing.md](routing.md) — router HTTP lookup
16. [gray.md](gray.md) — zone 灰度
17. [observability.md](observability.md) — kim-metrics
18. [deploy.md](deploy.md) — Docker Compose / GHCR / VPS

卡住时：先看文档里的「合同」和「执行链」，再去对应源码。文档和代码打架时，以代码为准。

## 缺口盘点（不是已落地设计）

- [production-gaps.md](production-gaps.md) — 对照当前代码的生产缺口：产品正确性、安全、以及运行时/开源库硬化。修复落地后应删掉对应条目，或把已拍板的设计写回上面的专题文档。

## 实施设计（切片，执行前写）

节奏：盘点 → 切片细化 → 写代码。不要对着 gaps 直接开巨型 PR。

- [impl/README.md](impl/README.md) — 切片表。切片 1（G-02 / G-08 Chat 长连接）已落地；切片 2 设计见 [impl/02-persist-first.md](impl/02-persist-first.md)（G-09）

## 文档怎么保持不腐烂

- **只写已经落地或已经拍板的设计。** 尚未实现的标成「以后」。缺口清单单独放 [production-gaps.md](production-gaps.md)，不要把「缺什么」写进专题文档冒充现状。
- 改了 trait、帧格式、分层，**同一轮把这里改掉**。文档和代码打架时，以代码为准。
- 不复制付费小册章节。

## 其它

| 路径 | 是什么 |
|---|---|
| [research/rust-kim-replica-feasibility.md](../research/rust-kim-replica-feasibility.md) | 能不能用 Rust 复刻、库怎么选 |
