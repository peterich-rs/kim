# 基准测试

对照小册第 25 章。本文只记仓库里的 `examples/kimbench`。

```bash
cargo run -p chat
cargo run -p gateway
cargo run -p kimbench -- login -c 20 -t 4
cargo run -p kimbench -- user  -c 20 -t 4
cargo run -p kimbench -- group -c 10 -t 2 -m 4 -p 0.5
```

`-a` 默认 `ws://127.0.0.1:8001/`。`tcp://127.0.0.1:8003` 或 `127.0.0.1:8003` 走 TGateway。

输出：Summary（RPS / RT）、5 桶直方图、p10/50/75/90/99、status 分布。百分位是 nearest-rank：`ceil(p*n).saturating_sub(1)`。

`login` 的 RT 含连接+鉴权；`user` / `group` 的计时区不含登录。群聊只把创建者的 `Flag=Response` 记入样本。账号是 `bench-{run_id}-…`，避免互踢污染直方图。

**数字是机器相关的，不要写进 git。** CI 只跑 `Stats` 单测和 4 次登录 e2e，不是 soak。
