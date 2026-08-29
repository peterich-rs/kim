# 部署

对照小册第 32 章。本机 Demo 仍是 `cargo run`。Compose 只起可选基础设施。

## 本机最小集

```text
fake-router :8088
fake-gateway :8001  (metrics :9001)
fake-tgateway :8003 (metrics :9003)
fake-chat :8002     (metrics :9002)
fake-royal :8080    可选
```

`deploy/compose.yml` profile：`metrics`（Prometheus）、`redis`、`postgres`。KIM 进程不进 compose。

TLS / 公网 WSS / TGateway TLS：**以后**。Compose 绑 localhost。

## 单机房（文档）

至少两个 zone 才能做灰度。Redis 哨兵、MySQL 主从是目标架构，不是本仓库默认。

## 同城双活（文档）

不实现。按 app 切 zone 可以让两个机房数据不相交；会话变更分区时要踢下线重登。
