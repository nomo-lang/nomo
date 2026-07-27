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
`structured_cancel_probe`、`structured_return_cancel_probe` 与
`structured_question_cancel_probe`，以及
`structured_explicit_cancel_probe` 与
`structured_deadline_probe`、`structured_panic_cleanup_probe`、
`publication_move_probe`。
counter 探针不混入 measured sample；它们单独设置
`NOMO_ASYNC_METRICS_PATH`，再按 `counter-catalog.json` 校验
版本化的 current-thread JSON 契约。panic 使用显式 expected-failure 契约，
精确校验 stderr 和 exit status，不会把意外命令失败当作通过。两层 frame、
两次 yield 会传递一个 managed
参数和结果，并必须得到：heap/slab frame allocation 为 0、幂等 frame drop
为 2、peak live frame 为 2、ready queue 往返为 2、poll 为 5、cooperative
yield 为 2。timer 探针还要求
零时长路径 inline ready，正时长恰好注册、到期和入队各一次，总计两次 poll，
无取消，并在退出时保持 live timer 为 0；同时要求 platform reactor 惰性
初始化、bounded wait、timeout 与 shutdown 各一次，退出时没有 reactor error
或 live reactor。纯 yield 探针要求 reactor 初始化次数为 0，证明 ready 工作
不承担 handle 成本。
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
root-frame wakeup 前取消的 typed final helper return 与 `?` error
propagation。panic gate 会保留 managed child message，通过 root 取消 armed
timer sibling，drop 全部 frame，导出 counter，最后才以原始 panic 退出。
显式 cancel-and-join gate 会消费 scope-owned handle、disarm live timer，并且
只在终态清理完成后返回类型化成功，同时保持 current-thread fast path 零分配。
ARC primitive counter 仍明确标记 unavailable，而不是伪装成 0。
deadline gate 会在同一个 owner-local table 上同时注册 deadline 与更长的 child
sleep，要求 timeout 优先、取消 sleep registration、构造 typed join error，并
验证三个 deadline 专用 counter，同时保持 frame allocation 与 atomic symbol
为零。
publication-move gate 会把一个带嵌套 COW storage 的 managed aggregate
transfer 到 structured child，要求 `publication_moves` 恰好为 1，并拒绝
generated retain、thread、atomic 与 heap-frame 证据。
`manifest-p3.json` 增加 current-thread bounded-channel 门禁：容量 8 的 Nomo
ring 与固定 `GOMAXPROCS=1` 的 Go channel 都传递 32 个 `u64` 值。Nomo probe
要求 buffered/direct-handoff、suspension、wakeup、close 以及 buffer/waiter
live/peak counter 全部精确匹配。它还重复运行 `sync_unused` 与 async yield
probe，并禁止出现 `nomo_channel_`，证明未构造 channel 的代码没有 channel
storage 或 metadata。Nomo/Go sample 仅作为证据，不构成性能声明。
同一 manifest 还会启用 `static_select_probe`：两个 empty receive 与一个正时长
timer 注册到同一个 token，稍后的 direct handoff 获胜，两个 loser 会在
owner frame 单次 wake 前被清理，最终 waiter/timer live counter 都回到零。
该探针会校验精确 select counter 并禁止 thread/atomic symbol；它是正确性门禁，
不是性能声明。
第一个 P2 foundation 会在 Linux 生成 epoll、macOS 生成 kqueue、Windows
生成 IOCP。既有正时长 timer 现在使用统一 reactor wait，不再调用 timer
专用 sleep primitive。P2-TCP-A 为 pending 数值地址 connect 增加一个带
generation 校验的 epoll/kqueue registration，并由 native fixture 精确验证
registration、completion、handle 与清理 counter。P2-TCP-B 增加有界增量
read 与完整 write，并精确验证 timeout、cancellation、readiness rearm、
retained bytes、handle、operation 与清理 counter；每次 poll 最多写 64 KiB，
使公平性不依赖宿主 socket buffer。P2-TCP-C 增加 16 个 live job、单 worker
的 hostname resolver；Unix 使用 nonblocking owner completion pipe，Windows
向 owner IOCP 投递 bounded completion。fixture 覆盖数值
地址零线程 fast path、hostname 成功与不泄露 secret 的失败、零 timeout
不初始化、queued cancellation、running cooperative cancellation、17 个请求
的精确饱和边界，以及 shutdown 后 resolver live resource 全归零。聚焦的
P2-TCP-D Windows 切片增加 `ConnectEx`、`WSARecv` 与 `WSASend`，
并使用 owner-local 64 槽固定 IOCP operation table；metrics 会分别记录
submitted、completed、cancelled、live 与 peak operation，cancellation 会在
frame drop 前转移 payload storage，shutdown 会 drain late completion。
browser fixture 会验证 typed `Unsupported` capability rejection 发生在
host、port 或 timeout operand 求值前。HTTP/SSE 与 process-pipe registration
仍等待各自的聚焦小切片。这是 correctness/lifecycle 证据，不是跨语言性能声明。
P2-PROC-A 现已版本化 `process_pipe_contract`：它固定 suspend
start/resume/frame ABI，并证明原生占位路径返回 `unsupported` 时不会发出阻塞
注册表或辅助线程。该 workload 在 P2-PROC-B 提供真实 Unix 注册和生命周期
计数器之前继续保持禁用。
mutable/affine suspend 参数、非最终 return、其他位置的 `?`、嵌套表达式或
runtime-originated panic unwind、取消传播与多任务 timer-wheel workload
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
python3 scripts/async_benchmark.py \
  --nomo target/release/nomo \
  --manifest performance/async/manifest-p3.json \
  --require-clean \
  --output performance/results/async-p3.json
```

Go patch 不匹配、少于五次 measured run、两侧输出 byte 不同、出现 stderr、
async/atomic symbol 非零、generated-C symbol 数量不符、runtime counter
未知/负数/缺失、build 失败，或使用 `--require-clean` 时 checkout 不干净，
harness 都会失败。每份结果记录 manifest、harness、counter catalog、source、
Nomo/Go/C toolchain 与产物 binary 的 SHA-256。请求的 metrics path 无法打开时，
程序只返回不包含路径的通用错误。

CI 上传原始 P0、P1 与 P3 JSON，不把 hosted runner timing 当成稳定 baseline。
后续在受控机器采集 release evidence 时还要设置 `NOMO_BENCH_POWER_MODE` 并
强制 process affinity。当前记录每个 process 的 wall time、CPU time 与 POSIX
`wait4` peak RSS，但还不记录 steady RSS。这些 sample 只验证 harness 与
counter 管线，不会计算 Nomo/Go 比率。

## 变更控制

workload 语义、输出 byte、build flag、measurement method 或 result schema 变化
时，必须升级 schema/series version。不能通过替换 Go version、payload、
safety check 或 sample selection 隐藏未达成的目标。
