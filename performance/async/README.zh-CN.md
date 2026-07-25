# Async runtime 基准框架

语言： [English](README.md) | 中文

本目录实现 RFC 0034 的证据契约，但不声称 P0 compiler 已经是生产级 async
runtime。目前仅启用两个 workload：

- `sync_unused`：编译使用 string、array 与确定性有序 map 的同步程序，并确认
  generated C 不含 async、thread 或 atomic symbol；
- `ready_call_control`：用始终 ready 的 Nomo `suspend fn` 调用链和固定 Go
  reference 执行同一确定性计算。它验证 source、binary、output、toolchain、
  sampling 与 result schema 管线，但明确不能用于性能宣传。

RFC 要求的所有 async workload 已经登记在 manifest 中；未实现项保持 disabled，
并记录预计实现阶段，避免“未覆盖”看起来像“已通过”。

独立的 P1 `async_yield` 实现已经提供嵌套 stackless suspend-call frame 与
current-thread executor；顶层不可变局部变量已经使用精确 liveness spill 和
child-first ownership-aware frame drop。但它不会混入这组 P0 measurement
series：参数/返回值 frame、完整 unwind path、structured spawn/join、timer
与 runtime counter export 尚未完成。后续会用版本化 P1 series 启用相关
workload，不会改写既有 P0 证据。

## 运行方式

先构建 Nomo CLI，并使用 `manifest.json` 固定的 Go patch version：

```sh
cargo build --release --locked --bin nomo
python3 scripts/async_benchmark.py \
  --nomo target/release/nomo \
  --require-clean \
  --output performance/results/async-p0.json
```

Go patch 不匹配、少于五次 measured run、两侧输出 byte 不同、出现 stderr、
async/atomic symbol 非零、build 失败，或使用 `--require-clean` 时 checkout 不干净，
harness 都会失败。每份结果记录 manifest、harness、counter catalog、source、
Nomo/Go/C toolchain 与产物 binary 的 SHA-256。

CI 上传原始 P0 JSON，不把 hosted runner 的 timing 当成稳定 baseline。后续在
受控机器采集 release evidence 时还要设置 `NOMO_BENCH_POWER_MODE`，并在 P1/P2
强制 process affinity。P0 记录每个 process 的 wall time、CPU time 与 POSIX
`wait4` peak RSS，但还不记录 steady RSS。这些 sample 只验证 harness 管线，
不会计算 Nomo/Go 比率。

## 变更控制

workload 语义、输出 byte、build flag、measurement method 或 result schema 变化
时，必须升级 schema/series version。不能通过替换 Go version、payload、
safety check 或 sample selection 隐藏未达成的目标。
