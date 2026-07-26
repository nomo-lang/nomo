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

第一个结构化并发小切片使用显式词法 scope，并只在 spawn 点显式创建并发：

```nomo
import std.result
import std.task

suspend fn child(message: string) -> string {
    task.yield_now()
    return message
}

suspend fn main() -> void {
    task.scope {
        let handle = task.spawn child("ready")
        let joined: Result<string, TaskError> = task.join(handle)
        let completed: bool = result.is_ok(joined)
    }
}
```

`task.spawn child(args)` 与带括号的旧 `task.spawn(worker, input)` 有意保持
不同。结构化形式从 child 返回类型推导 scope-owned `Task<T>`，单参数 join
必须且只能消费一次该 handle，并返回 `Result<T, TaskError>`。

## 已实现的 P1 小切片

Native C99 后端遇到最终到达 `task.yield_now()` 或 `task.sleep(...)` 的 suspend
调用链时会生成：

- 栈上分配、带显式 state 且内嵌 child frame 的 root frame；
- 每个真正可能挂起的函数各自的 poll/drop pair；
- 直接 poll child，并把 child 的 `PENDING` 逐层传播到 root；
- inline initial poll；
- 仅在返回 `PENDING` 后进入的 64 槽 owner-local 有界 FIFO ready queue；容量
  用尽时明确报告 saturation，不允许无界增长；
- 带 generation 校验、monotonic deadline、按 deadline/generation
  确定性排序与幂等 disarm 的有界 owner-local timer table；
- 入同一有界 FIFO 的内嵌 structured child frame，以及 child 完成时重新入队
  parent 的单一 owner-local waiter edge；
- structured spawn 无法进入 64 槽 ready queue 时，由 join 构造
  `TaskError { code: "queue_full", ... }`；
- 在 drop child frame 前，把 typed child result 恰好一次 move 到 join 的成功
  payload；
- 编译器在 normal fallthrough 与最终 `return` 的 scope 边界插入清理：取消未
  join child、从 ready queue 移除其 entry、disarm timer，并在执行 scope
  后语句或完成 return 前 drop frame；
- 直接 structured `?` binding 的 owned Err/None 传播，并在 helper 完成与
  parent wakeup 前清理 live sibling；
- 每个 yield 或 child call 上精确的顶层局部变量 liveness；
- managed ARC/COW frame 字段各自的 ownership bit；
- release 前先清 ownership bit、按 child-first 顺序执行的幂等 frame drop。

这一小切片不会创建 OS thread、heap task、reactor 或 atomic metadata。ready
的零时长 timer 不注册也不入队；正时长 timer 只有在 deadline 到达并把 owner
frame 移入 ready queue 后才会再次 poll。生成的 context 会记录 poll、yield、
frame drop/live frame、入队/出队/饱和/取消、
structured spawn/join/join suspension/取消，以及 timer
注册/到期/取消/live/peak 计数。
Native 程序只在设置 `NOMO_ASYNC_METRICS_PATH` 时导出版本化
`nomo-c99-current-thread` JSON；普通运行不会执行 metrics I/O。P1 benchmark
会在 measured run 之后单独执行探针。ARC primitive counter 仍明确标记为
unavailable，而不是伪装成 0。
在 suspension 前已经死亡的局部变量会直接 release，不进入 frame。suspension
后仍使用的不可变局部变量会 move 到 frame；恢复后只为当前 segment 真正引用
的值生成 non-owning C alias。内嵌 child 先 inline poll；同步完成时不分配也不
进入 ready queue。正常完成和显式 early root drop 共用同一条 child-first 幂等
清理路径。不可变且 frame-safe 的调用参数按源码顺序只求值一次；共享 managed
值 retain 进 child frame，owned temporary 直接 transfer，owned 结果会在
child drop 前 move 到调用方的不可变 binding。

上述 inline fast path 适用于普通 direct suspend call。structured spawn
会真正创建并发：不可变且 frame-safe 的参数只求值一次，随后初始化内嵌 child
frame 并将其调度到有界 FIFO。join 只在目标 child 尚未完成时挂起；child
完成会唤醒一个 owner-local waiter。显式 join 清理与 parent 清理都会执行幂等
child drop。该切片不创建 heap task、OS thread、atomic reference count 或
全局 work-stealing queue。

Browser WASM 的有界沙盒解释器可以运行同一份源码。目前
`task.yield_now()` 只表示 cooperative boundary；它还不会把控制权交还给
host Promise 或浏览器 event loop。`task.sleep` 在 browser sandbox 中既不阻塞
也不求值 duration，而是返回
`TaskError { code: "runtime_unavailable", ... }`。structured child body
目前同样不会在 browser 中执行，其 join 返回同一稳定错误。

## 有意保留的限制

对暂不支持的挂起形态，编译器报告 E0876，而不是生成错误代码。当前
`task.yield_now()` 和不返回值的 suspend 调用必须是独立语句；返回值的 suspend
调用与 `task.sleep(Duration)` 必须作为不可变顶层 `let` 的 initializer。
所在 `suspend fn` 仍须 non-generic；参数、结果和跨 suspension local 必须是
不可变且 frame-safe 的 scalar、string、struct、enum、Result 或已支持 array。
async `main` 仍只返回 `void`。mutable 参数/local、borrow、guard、resource
handle 或包含它的 wrapper、递归 suspend graph、控制流、嵌套表达式或参数表达式
内部挂起、下述 structured binding 之外的 `?`、其他表达式内部的 panic、
显式取消传播和 reactor I/O 都属于后续小 PR。

structured spawn/join 当前只允许出现在顶层 `task.scope` body。每个 spawn
handle 必须使用推导得到的不可变 binding 且不得离开 scope；若要观察结果，
只能恰好 join 一次。target 必须是直接、未限定、non-generic 的顶层
`suspend fn`，参数不可变且 frame-safe。其返回类型会形成 `Task<T>`，而
`task.join(handle)` 返回
`Result<T, TaskError>`。最终 `return` 会先把表达式求值到私有 owned
temporary，再由编译器取消并 drop 所有未 join child，然后把 temporary move
到 helper frame，最后完成 helper 并唤醒 root frame。normal fallthrough
也会在执行下一条语句前做同样清理。scope 顶层不可变
`let value: T = expression?` 只求值一次；Err/None 路径先把传播 carrier 保存为
owned frame state，再取消并 drop 该语句处所有 live child，最后完成 helper
并唤醒 parent。成功 payload 可以通过既有 frame liveness 方案跨越后续
suspension。当前 `expression` 可以是非挂起表达式或直接
`task.join(handle)?`，且仍要求显式类型标注。取消后不会继续执行 child body。
顶层直接 `panic(message)` 会先求值并拥有不可挂起的 message，再传播 panic。
child panic 会停止 executor；root 随后递归取消全部未完成 child、移除 ready
entry、解除 timer、drop 全部 frame、执行 runtime shutdown 与 metrics export，
最后打印并 release 原始消息，以状态 1 退出。`debug.panic` 走同一 statement
路径。Browser WASM 返回同样的 runtime error，同时仍不执行 structured child
body。嵌套 scope、scope 内嵌套控制流、非最终 scope return、defer/unsafe、
其他位置的 `?`、其他表达式内部的 panic、显式取消、deadline、channel 与
select 仍属于后续切片。
E0871、E0872、E0875 与 E0876 会在 codegen 前拒绝这些情况。

既有 `task.spawn` 仍是兼容用的隔离 native worker API，不是新的 async task
constructor，而且当前仍是一 worker 一 native thread。RFC 0032 要求后续将它
迁移到有界、惰性的 blocking pool。

## 正确性与成本门禁

当前切片已经用 generated-C 测试和 AddressSanitizer 检查精确 spill、dead local
的 suspension 前清理、child-before-parent ownership bit 清零、重复显式 drop、
monotonic 不提前唤醒、零时长 timer fast path，以及挂起 child timer 的取消。
structured task 还覆盖 FIFO 交错、单次 join ownership、waiter wakeup、类型化
queue saturation、browser 不执行 child，以及幂等 child cleanup。
managed typed result 还会用 AddressSanitizer 覆盖 child 到 join 的 ownership
transfer、嵌套 helper 唤醒 root frame、post-join scope return 和 parent 重复
drop。scope cancellation 还会在 AddressSanitizer 下覆盖 armed timer、从未
poll 的 ready child，以及在 root-frame wakeup 前取消的 typed helper return
和 typed `?` error propagation，包括 managed 参数、传播 error 与结果 release。
panic 测试还覆盖 managed 同步消息、spawn child panic、root 递归取消、
armed sibling timer、精确 frame/task/timer counter 与 browser-WASM 边界。
后续实现仍必须用测试和证据证明：

- 其余 error、cancellation、timeout 和嵌套表达式或 runtime-originated panic
  路径仅对 frame 中的 ARC/COW 值 release 一次；
- 不允许不安全 mutable borrow 或 guard 跨 suspension point；
- 未使用 suspension 的程序没有 runtime、thread、coroutine metadata 或普通
  collection atomic 成本；
- synchronous-ready 路径不分配、不进入 ready queue；
- 兼容 C99 与 browser WASM，并继续覆盖 Linux、macOS/BSD 和 Windows reactor；
- 固定版本、公平 workload 的 Nomo 与 Go 对比，不能通过削弱对照来达标。

P0/P1 控制组与原始证据格式位于
[`performance/async`](../performance/async/README.zh-CN.md)，当前小切片的可运行
示例位于 [`examples/async_yield`](../examples/async_yield) 与
[`examples/async_timer`](../examples/async_timer)，以及
[`examples/async_structured_void`](../examples/async_structured_void) 与
[`examples/async_structured_results`](../examples/async_structured_results)，以及
[`examples/async_structured_return`](../examples/async_structured_return) 与
[`examples/async_structured_cancel`](../examples/async_structured_cancel)，以及
[`examples/async_structured_return_cancel`](../examples/async_structured_return_cancel) 与
[`examples/async_structured_question_cancel`](../examples/async_structured_question_cancel)，以及
[`examples/async_structured_panic_cleanup`](../examples/async_structured_panic_cleanup)。
