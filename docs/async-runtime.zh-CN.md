# 异步运行时

本文记录 Proposed 异步与并发 RFC 在当前工具链中的真实实现状态，不代表所有
RFC acceptance gate 已经通过。

- [RFC 0031](https://github.com/nomo-lang/rfcs/blob/main/zh-CN/rfcs/0031-direct-style-suspend-functions-and-structured-concurrency.md)
  定义 direct-style suspend effect、stackless lowering、frame 析构与结构化并发。
- [RFC 0032](https://github.com/nomo-lang/rfcs/blob/main/zh-CN/rfcs/0032-sharded-executor-reactor-and-blocking-pool.md)
  定义 executor/reactor、owner affinity、平台后端与 blocking pool 迁移。
- [RFC 0033](https://github.com/nomo-lang/rfcs/blob/main/zh-CN/rfcs/0033-task-ownership-transfer-and-concurrent-values.md)
  定义跨任务转移与显式共享能力。
- [RFC 0034](https://github.com/nomo-lang/rfcs/blob/main/zh-CN/rfcs/0034-async-runtime-acceptance-and-benchmark-gates.md)
  定义正确性、可移植性、内存和性能门禁。
- [RFC 0035](https://github.com/nomo-lang/rfcs/blob/main/zh-CN/rfcs/0035-monotonic-suspend-timers-and-blocking-sleep-migration.md)
  定义 owner-local timer 与阻塞 sleep 边界。

[English](async-runtime.md)

## 语言表面

可能挂起的函数使用 `suspend fn`，调用点保持 direct-style：

```nomo
package app.main

import std.io
import std.task

suspend fn yield_once() -> void {
    task.yield_now()
}

suspend fn main() -> void {
    io.println("before")
    yield_once()
    io.println("after")
}
```

普通 `fn` 不能调用 suspend 函数；编译器报告 E0870，而不是偷偷引入运行时。
只声明或调用 always-ready suspend 函数不会创建 executor。

编译器还会拒绝传递调用图最终到达阻塞兼容 API `time.sleep` 或
`time.sleep_millis` 的 `suspend fn`。E0891 只报告函数/API 调用路径，绝不回显
参数值。同步函数与旧的隔离 worker 仍可使用阻塞 API。非阻塞
`task.sleep(Duration) -> Result<void, TaskError>` 已可用于 native C99
current-thread backend。duration 只求值一次；非正时长 inline 完成，正时长注册
owner-local monotonic timer。browser sandbox 在 host-driven timer backend
落地前返回稳定的 `runtime_unavailable` result。

## 已实现的 P1 小切片

Native C99 后端遇到最终到达 `task.yield_now()` 或 `task.sleep(...)` 的 suspend
调用链时会生成：

- 栈上分配、带显式 state 且内嵌 child frame 的 root frame；
- 每个真正可能挂起的函数各自的 poll/drop pair；
- 直接 poll child，并把 child 的 `PENDING` 逐层传播到 root；
- inline initial poll；
- 仅在返回 `PENDING` 后进入的单槽 current-thread ready queue 路径；
- 带 generation 校验、monotonic deadline、按 deadline/generation
  确定性排序与幂等 disarm 的有界 owner-local timer table；
- 每个 yield 或 child call 上精确的顶层局部变量 liveness；
- managed ARC/COW frame 字段各自的 ownership bit；
- release 前先清 ownership bit、按 child-first 顺序执行的幂等 frame drop。

这一小切片不会创建 OS thread、heap task、reactor 或 atomic metadata。ready
的零时长 timer 不注册也不入队；正时长 timer 只有在 deadline 到达并把 owner
frame 移入 ready queue 后才会再次 poll。生成的 context 会记录 poll、yield、
frame drop/live frame、入队/出队，以及 timer 注册/到期/取消/live/peak 计数。
Native 程序只在设置 `NOMO_ASYNC_METRICS_PATH` 时导出版本化
`nomo-c99-current-thread` JSON；普通运行不会执行 metrics I/O。P1 benchmark
会在 measured run 之后单独执行探针。ARC primitive counter 仍明确标记为
unavailable，而不是伪装成 0。
在 suspension 前已经死亡的局部变量会直接 release，不进入 frame。suspension
后仍使用的不可变局部变量会 move 到 frame；恢复后只为当前 segment 真正引用
的值生成 non-owning C alias。内嵌 child 先 inline poll；同步完成时不分配也不
进入 ready queue。正常完成和显式 early root drop 共用同一条 child-first 幂等
清理路径。

Browser WASM 的有界沙盒解释器可以运行同一份源码。目前
`task.yield_now()` 只表示 cooperative boundary；它还不会把控制权交还给
host Promise 或浏览器 event loop。`task.sleep` 在 browser sandbox 中既不阻塞
也不求值 duration，而是返回
`TaskError { code: "runtime_unavailable", ... }`。

## 有意保留的限制

对暂不支持的挂起形态，编译器报告 E0876，而不是生成错误代码。当前
`task.yield_now()` 和对无参数、真正可能挂起函数的调用必须是独立语句；
`task.sleep(Duration)` 必须作为不可变 `let` 的 initializer，并绑定为
`Result<void, TaskError>`。所在 `suspend fn` 仍须 non-generic、无参数且返回
`void`。不可变的顶层 scalar、string、struct、enum、Result 与已支持 array
局部变量可以跨 suspension 存活，前提是所有传递 value field 都满足
frame-safe。mutable local、borrow、guard、resource handle 或包含它的 wrapper、
递归 suspend graph、控制流或其他表达式内部挂起、通用 suspend 函数参数/返回值、
`?`、显式 panic、spawn/join、取消和 reactor I/O 都属于后续小 PR。

既有 `task.spawn` 仍是兼容用的隔离 native worker API，不是新的 async task
constructor，而且当前仍是一 worker 一 native thread。RFC 0032 要求后续将它
迁移到有界、惰性的 blocking pool。

## 正确性与成本门禁

当前切片已经用 generated-C 测试和 AddressSanitizer 检查精确 spill、dead local
的 suspension 前清理、child-before-parent ownership bit 清零、重复显式 drop、
monotonic 不提前唤醒、零时长 timer fast path，以及挂起 child timer 的取消。
后续实现仍必须用测试和证据证明：

- error、cancellation、timeout 和 panic 路径仅对 frame 中的 ARC/COW 值
  release 一次；
- 不允许不安全 mutable borrow 或 guard 跨 suspension point；
- 未使用 suspension 的程序没有 runtime、thread、coroutine metadata 或普通
  collection atomic 成本；
- synchronous-ready 路径不分配、不进入 ready queue；
- 兼容 C99 与 browser WASM，并继续覆盖 Linux、macOS/BSD 和 Windows reactor；
- 固定版本、公平 workload 的 Nomo 与 Go 对比，不能通过削弱对照来达标。

P0/P1 控制组与原始证据格式位于
[`performance/async`](../performance/async/README.zh-CN.md)，当前小切片的可运行
示例位于 [`examples/async_yield`](../examples/async_yield) 与
[`examples/async_timer`](../examples/async_timer)。
