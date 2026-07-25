# Async runtime 基准框架

语言： [English](README.md) | 中文

本目录实现 RFC 0034 的证据契约，但不声称当前 P1 compiler 已经是生产级
async runtime。P0 manifest 保留两个 control：

- `sync_unused`：编译使用 string、array 与确定性有序 map 的同步程序，并确认
  generated C 不含 async、thread 或 atomic symbol；
- `ready_call_control`：用始终 ready 的 Nomo `suspend fn` 调用链和固定 Go
  reference 执行同一确定性计算。它验证 source、binary、output、toolchain、
  sampling 与 result schema 管线，但明确不能用于性能宣传。

独立的 `manifest-p1.json` 会再次运行两个 zero-cost control，并启用
`yield_counter_probe`、`timer_counter_probe`、`task_spawn_complete`、
`structured_cancel_probe` 与 `structured_return_cancel_probe`。
counter 探针不混入 measured sample；它们单独设置
`NOMO_ASYNC_METRICS_PATH`，再按 `counter-catalog.json` 校验
版本化的 current-thread JSON 契约。两层 frame、两次 yield 会传递一个 managed
参数和结果，并必须得到：heap/slab frame allocation 为 0、幂等 frame drop
为 2、peak live frame 为 2、ready queue 往返为 2、poll 为 5、cooperative
yield 为 2。timer 探针还要求
零时长路径 inline ready，正时长恰好注册、到期和入队各一次，总计两次 poll，
无取消，并在退出时保持 live timer 为 0。
spawn/complete workload 在同一固定单核配置下运行 32 个 scope-owned Nomo
`Task<void>` child 与 32 个 Go goroutine；它校验精确的 spawn/join/join
suspension counter 及 frame/queue 清理，但在 runtime 仍是 current-thread-only
时不具备性能声明资格。

RFC 要求的所有 async workload 已经登记在 manifest 中；未实现项保持 disabled
并记录阶段，避免“未覆盖”看起来像“已通过”。owner-local timer 的注册、到期、
取消、live 与 peak-live counter 已可用。current-thread executor 现在使用
64 槽有界 FIFO，并通过 `ready_queue_saturations` 记录被拒绝的入队；多任务
saturation workload 还会证明被拒绝的 spawn 转化为类型化 join error。
取消的 queued entry 与未完成 task 分别由 `ready_queue_cancellations` 和
`task_cancellations` 精确计数。structured `Task<T>` result 已有
generated-C、native、WASM 边界与
post-join 嵌套 scope return、AddressSanitizer 正确性覆盖；scope 取消还通过
已启用的 runtime-counter gate 覆盖 armed timer、ready queue 清理，以及在
root-frame wakeup 前取消的 typed final helper return。ARC primitive counter
仍明确标记 unavailable，而不是伪装成 0。mutable/affine suspend 参数、
非最终/`?`/panic unwind path、取消传播与多任务 timer-wheel workload
仍未完成。

## 运行方式

先构建 Nomo CLI，并使用 `manifest.json` 固定的 Go patch version：

```sh
cargo build --release --locked --bin nomo
python3 scripts/async_benchmark.py \
  --nomo target/release/nomo \
  --require-clean \
  --output performance/results/async-p0.json
python3 scripts/async_benchmark.py \
  --nomo target/release/nomo \
  --manifest performance/async/manifest-p1.json \
  --require-clean \
  --output performance/results/async-p1.json
```

Go patch 不匹配、少于五次 measured run、两侧输出 byte 不同、出现 stderr、
async/atomic symbol 非零、generated-C symbol 数量不符、runtime counter
未知/负数/缺失、build 失败，或使用 `--require-clean` 时 checkout 不干净，
harness 都会失败。每份结果记录 manifest、harness、counter catalog、source、
Nomo/Go/C toolchain 与产物 binary 的 SHA-256。请求的 metrics path 无法打开时，
程序只返回不包含路径的通用错误。

CI 上传原始 P0 与 P1 JSON，不把 hosted runner timing 当成稳定 baseline。
后续在受控机器采集 release evidence 时还要设置 `NOMO_BENCH_POWER_MODE` 并
强制 process affinity。当前记录每个 process 的 wall time、CPU time 与 POSIX
`wait4` peak RSS，但还不记录 steady RSS。这些 sample 只验证 harness 与
counter 管线，不会计算 Nomo/Go 比率。

## 变更控制

workload 语义、输出 byte、build flag、measurement method 或 result schema 变化
时，必须升级 schema/series version。不能通过替换 Go version、payload、
safety check 或 sample selection 隐藏未达成的目标。
