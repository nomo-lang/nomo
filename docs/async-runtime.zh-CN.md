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
- [RFC 0036](https://github.com/nomo-lang/rfcs/blob/main/zh-CN/rfcs/0036-bounded-channels-publication-moves-and-static-select.md)
  定义有界 channel、消费式 publication 与后续 static select 表面。
- [RFC 0037](https://github.com/nomo-lang/rfcs/blob/main/zh-CN/rfcs/0037-owner-affine-async-tcp-client-and-blocking-migration.md)
  定义有界、owner-affine 的异步 TCP client 与阻塞 API 迁移。

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

P2-TCP-A/B/C 加上聚焦的 P2-TCP-D Windows 数值地址小切片，还提供
direct-style connect、有界增量读取与完整写入：

```nomo
import std.net
import std.result

suspend fn main() -> void {
    let connected: Result<TcpStream, NetError> =
        net.connect("localhost", 8080, 1000)
}

suspend fn exchange(stream: TcpStream) -> void {
    let wrote: Result<void, NetError> =
        stream.write_string("ping", 1000)
    let received: Result<TcpTextChunk, NetError> =
        stream.read_string(4096, 1000)
}
```

Linux 与 macOS 会以 nonblocking socket 发起每次连接，并通过带 generation
校验的 epoll/kqueue registration 挂起。Windows 数值 IPv4/IPv6 connect 使用
`ConnectEx`，read/write 使用 `WSARecv`/`WSASend` 与 owner-local 64 槽固定
IOCP operation table。数值地址不会启动 OS thread。Linux 与 macOS 上最长
253 字节的 hostname 会进入一个惰性启动的 resolver worker；该 worker 前有
16 个 job 的固定容量，completion 通过 owner reactor 返回，最多按 resolver
顺序尝试 16 个 IPv4/IPv6 candidate。解析与所有 candidate 共用一个最长
15 分钟的 monotonic deadline。hostname 零 timeout 会 inline 返回，且不会
初始化 pool 或 reactor；Windows hostname 在余下的 P2-TCP-D resolver 子切片
落地前明确返回 `Unsupported`。`TcpStream` 固定到 owner executor，属于
Local/!Send。

每条 stream 同时最多有一个 pending read 和一个 pending write；同方向冲突
返回 `Busy`。`read` 返回一块 `Array<u32>` 字节，`read_string` 校验一块
UTF-8，二者都不会隐式 read-to-EOF。每个 payload 最大 1 MiB。write 仅跨
one-shot readiness 保留未发送后缀，每次 executor poll 最多推进 64 KiB 以
保证公平性，并且要么完整写入 payload，要么返回错误。timeout 与 structured
cancellation 会清除 registration 和 retained buffer；除非显式 close，
stream 仍可复用。

resolver 容量用尽返回 `Limit`，解析失败返回 `Resolve`；两者的诊断都不会
复制 hostname。queued job 可立即取消；已经进入系统 resolver 的调用采用
cooperative cancellation：调用者进入终态，但 executor shutdown 会等待
lookup 返回，以便把 worker 与 owner registration 恰好清理一次。这是
P2-TCP-C 的单 worker 聚焦切片，不是 RFC 0032 的通用 blocking pool。Windows
发生 timeout 或 structured cancellation 时，会把 pending read/write buffer
从 coroutine frame 脱离，调用 `CancelIoEx`，并让固定 IOCP slot 持有该 buffer
直到 late completion 被 drain；reactor shutdown 会在关闭 completion port 前
清空所有 live IOCP slot，避免 `OVERLAPPED` 指回已经释放的 frame。预览期的
阻塞名称是
`net.connect_blocking`、`read_to_string_blocking` 与
`write_string_blocking`；suspend 调用图到达这些 API 时报告 E0891。
当前 stackless 小切片需要像上例一样绑定每个完整 `Result`；在通用的
suspend-question lowering 落地前，直接对这些 I/O 操作使用 `?` 仍会报告
E0876。P2-TCP-B 尚未提供 `shutdown_write` 半关闭操作；在该独立生命周期
切片落地前请使用 `close`。示例见
[`examples/async_tcp_io`](../examples/async_tcp_io)。

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

### Publication move 与编译器内建 Send

structured spawn 现在是 publication boundary。编译器会为每个 child 参数做
私有 capability 推导；这一切片不允许用户实现或覆盖它：

| 值 | publication 行为 |
| --- | --- |
| 数值、`bool`、`char` | Copy；源 binding 仍可使用 |
| `string`、`CString`、`Array<T>`、`Map<K,V>`、普通 struct/enum | 所有嵌套类型均为 Send 时可发布；命名 binding 会被消费 |
| `File`、socket、HTTP server/exchange/stream、`ProcessChild`、SQLite/task/FFI handle | Local/!Send，直接拒绝 |

```nomo
let message: AgentMessage = build_message()
task.scope {
    let child = task.spawn consume(message)
    // E0881：message 已 publication-move 到 child
}
```

这里刻意没有 `move` 关键字，也没有 public `Send` interface；编译器根据
structured-spawn 参数位置决定消费语义。常量仍可重复使用，owned temporary
直接 transfer。P3-A 不允许只移动 `message.content` 这样的 managed field；
需要移动整个 aggregate，或先构造 temporary。E0880 报告直接 Local 值，
E0883 指出 structural Send 失败的第一个嵌套字段，E0881 报告同一 boundary
重复消费或 publication 后再次使用。

IR 会显式标记每个 consuming argument。Native C99 先初始化 embedded child
参数并设置 child ownership bit，再清除 parent parameter/local ownership bit，
不会对 moved value 执行 retain。child cancellation、queue rejection、join、
panic 与 parent drop 都汇入既有的 exactly-once child-frame cleanup。跨 shard
的 COW detach 要等 sharded executor 落地；当前实现只有一个 owner thread。

顶层不可变
`let cancelled: Result<void, TaskError> = task.cancel(handle)` 是 structured
cancel-and-join：它请求取消，等待 child 完成终态清理，再消费并 drop handle。
已经完成的 child 返回 `Ok(void)`；spawn 因 ready queue 满而失败时返回稳定的
`queue_full` error。这个 overload 与旧的同步 `task.cancel(Task)` worker
取消请求不同。

第一个 deadline 小切片也是 compiler-recognized structured scope：

```nomo
import std.task
import std.time

suspend fn bounded_work() -> string {
    task.deadline(time.duration_millis(50)) {
        let waited: Result<void, TaskError> =
            task.sleep(time.duration_millis(1000))
        task.check_cancelled()
    }
    return "completed"
}
```

duration 只求值一次。非正 duration 会在 body 执行前终止当前 suspend task，
形成 `TaskError { code: "timeout", ... }`，且不注册 timer。正 duration 会注册
一个 owner-local monotonic timer；normal fallthrough 会解除它。到期路径会先
取消当前 frame 的 child subtree 及 pending timer/ready registration，再完成
task。structured parent 通过 `task.join` 观察该错误，不会得到伪造的 child
返回值。root timeout 只打印稳定 code，并以非零状态退出。

`task.check_cancelled()` 不挂起、不分配也不入队，是显式 cooperative
observation point；生成的状态机也会在 runtime suspension 边界前后检查。若
ready operation 与 deadline 在同一 resume boundary 同时可观察，timeout
优先。

### 有界 Channel

P3-B current-thread 小切片增加 typed bounded FIFO：

```nomo
let created: Result<Channel<string>, ChannelError> =
    task.channel<string>(8)
let sent: Result<void, ChannelSendError<string>> =
    task.send(channel_value, message)
let received: Option<string> = task.receive(channel_value)
```

`task.channel<T>(capacity)` 接受 1 到 65,536 个 element，且经过 checked
arithmetic 后的 slot storage 不得超过 64 MiB。失败使用稳定的
`invalid_capacity`、`capacity_limit` 或 `allocation` code，且不格式化用户值。
`task.send` 与 `task.receive` 是 direct-style suspension point；
`task.try_send`、`task.try_receive` 永不挂起。`task.close` 幂等，会唤醒 blocked
operation、拒绝新 send，并让已缓冲值继续按 FIFO drain。

命名的非 Copy send 值会被 publication-move。成功路径把唯一 owner 转移给
receiver 或 ring slot；full、closed 与 runtime failure 通过
`ChannelTrySend<T>` 或 `ChannelSendError<T>` 恰好返还一个 owner。Channel
handle 的 copy 共享 current-thread control block；普通 array、map 与 string
仍使用 task-local 非原子 ARC/COW。本切片没有 atomic shim 或跨 shard 共享。

Native C99 使用 owner-local ring 和 FIFO sender/receiver registration。已有
receiver 时直接 handoff，否则 ring 满会挂起 sender。cancel、timeout、close、
wake 后尚未 resume、frame drop 与 normal completion 都会移除 registration，
并恰好一次 release 或返还 staged value。示例见
[`examples/async_bounded_channel`](../examples/async_bounded_channel)。

Browser WASM 尚未提供 host-driven channel backend。constructor 会在不求值
capacity 的情况下返回 `runtime_unavailable`；其他 channel operation 会在
求值可能被消费的 channel operand 或 send value 前报告 sandbox capability
error。

### 静态 receive/timer select

P3-C 增加一个由编译器识别、包含 2 到 8 个静态 arm 的 statement：

```nomo
task.select {
    task.receive(messages) => message {
        consume(message)
    }
    task.sleep(time.duration_millis(50)) => timeout {
        observe(timeout)
    }
}
```

首个切片只接受直接 `task.receive(Channel<T>)` 与 `task.sleep(Duration)`
operation。全部 operand 会先按源码从上到下各求值一次，再执行
cancellation/deadline readiness 检查。若多个 arm 已 ready，则源码中最靠前的
arm 获胜；否则全部 arm 会注册到同一个 owner-local select token。第一个成功
claim 会立即 unlink 或 disarm 所有 loser，并保证 owner frame 最多只入队一次；
迟到的 loser event 不会执行 arm body。

每个 arm 会在非空 lexical body 中绑定 operation result（`Option<T>` 或
`Result<void, TaskError>`）。首个 lowering 要求 normal fallthrough，并以
E0876 拒绝 `return`、`break`、`continue`、`?`、panic、defer、嵌套
scope/deadline/select 以及会挂起的 arm body。send/join select 与 general
structured exit 留到后续切片。Browser WASM 会在求值任何 arm operand 前报告
`runtime_unavailable`，不会用顺序执行伪装 select。示例见
[`examples/async_static_select`](../examples/async_static_select)。

## 已实现的 P1、P2 Reactor/P2-TCP-A/B/C/D-数值地址与 P3-B/P3-C 小切片

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
- 惰性初始化的 owner-local platform reactor：Linux 使用 epoll、macOS 使用
  kqueue、Windows 使用 IOCP。正时长 timer 以有界 timeout 进入 reactor；
  ready-only 工作与非正时长 timer 不初始化它；
- 64 槽有界 I/O owner table、slot generation 与 exclusive close；每个 pending
  TCP candidate 在 epoll/kqueue 上只使用一个内嵌 registration 和一个 timeout
  timer；Windows 每次数值 connect/read/write submission 使用一个固定表
  IOCP operation；
- 一个惰性 resolver worker、16 个固定 job slot、nonblocking owner wake pipe、
  最多 16 个复制后的地址 candidate、一个总 deadline，以及精确的 queued/
  running/cancelled/completed/live/peak 生命周期 counter；
- 每个源码 read/write operation 使用一个内嵌 registration；每个 stream
  direction 最多一个 pending operation，并支持 one-shot readiness rearm、
  完整 partial-write progress、有界 retained-byte metric，以及 timeout/
  cancellation 的恰好一次清理；
- Windows `OVERLAPPED` storage 位于 owner reactor 而非 coroutine frame，
  并提供 `CancelIoEx`、late completion 的 detached payload ownership、固定
  64-operation backpressure 与 shutdown drain；
- 入同一有界 FIFO 的内嵌 structured child frame，以及 child 完成时重新入队
  parent 的单一 owner-local waiter edge；
- structured spawn 无法进入 64 槽 ready queue 时，由 join 构造
  `TaskError { code: "queue_full", ... }`；
- 在 drop child frame 前，把 typed child result 恰好一次 move 到 join 的成功
  payload；
- 对 structured-spawn 的非 Copy 命名参数执行 compiler-known structural Send
  校验和 ownership-bit transfer；
- structured cancel-and-join 挂起边界：向 child subtree 传播取消、移除 ready
  queue/timer 注册、等待终态清理、返回 `Result<void, TaskError>`，再 drop
  已消费的 child frame；
- 每个 suspend function 可有一个非嵌套 `task.deadline(Duration)` scope，
  覆盖非正时长立即 timeout、饱和 monotonic deadline 计算、确定性的
  ready/timeout 检查、typed child failure 与 child-first cancellation cleanup；
- static receive/timer select token，覆盖源码顺序 ready arbitration、operand
  恰好一次求值、owner frame 单次 wake、eager loser cleanup，且不创建 heap
  task 或 per-select allocation；
- 编译器在 normal fallthrough 与最终 `return` 的 scope 边界插入清理：取消未
  join child、从 ready queue 移除其 entry、disarm timer，并在执行 scope
  后语句或完成 return 前 drop frame；
- 直接 structured `?` binding 的 owned Err/None 传播，并在 helper 完成与
  parent wakeup 前清理 live sibling；
- 每个 yield 或 child call 上精确的顶层局部变量 liveness；
- managed ARC/COW frame 字段各自的 ownership bit；
- release 前先清 ownership bit、按 child-first 顺序执行的幂等 frame drop。

这一小切片不会创建 OS thread、heap task 或 atomic metadata。ready 的零时长
timer 不注册、不入队，也不初始化 reactor。正时长 timer 会惰性创建一个
owner-local epoll、kqueue 或 IOCP instance，通过它等待而不是调用
`Sleep`/`nanosleep`，并在 metrics export 前关闭。timer 只有在 deadline
到达并把 owner frame 移入 ready queue 后才会再次 poll。生成的 context 会记录
poll、yield、frame drop/live frame、入队/出队/饱和/取消、
structured spawn/publication move/join/join suspension/取消、deadline
注册/到期/解除，以及 timer
注册/到期/取消/live/peak 计数；同时记录 reactor 初始化、wait、timeout、
completion、error、shutdown 与 live/peak 生命周期计数。纯 yield 探针要求所有
reactor counter 为 0；正时长 timer 探针要求各一次初始化、wait、timeout 与
shutdown，且退出时 live reactor 为 0。
P3-B channel 还记录 construction binding、buffered/direct delivery、
suspension、wakeup、close/cancel 与 buffer/waiter 的 live/peak 计数。
Native 程序只在设置 `NOMO_ASYNC_METRICS_PATH` 时导出版本化
`nomo-c99-current-thread` JSON；普通运行不会执行 metrics I/O。P1 benchmark
会在 measured run 之后单独执行探针。ARC primitive counter 仍明确标记为
unavailable，而不是伪装成 0。
在 suspension 前已经死亡的局部变量会直接 release，不进入 frame。suspension
后仍使用的不可变局部变量会 move 到 frame；恢复后只为当前 segment 真正引用
的值生成 non-owning C alias。内嵌 child 先 inline poll；同步完成时不分配也不
进入 ready queue。正常完成和显式 early root drop 共用同一条 child-first 幂等
清理路径。不可变且 frame-safe 的调用参数按源码顺序只求值一次。普通 direct
suspend call 会把共享 managed 值 retain 进 child frame；structured spawn
则会 publication-move 非 Copy 命名 binding。两种形式的 owned temporary
都直接 transfer；owned 结果会在 child drop 前 move 到调用方的不可变 binding。

上述 inline fast path 适用于普通 direct suspend call。structured spawn
会真正创建并发：不可变且 frame-safe 的参数只求值一次，完成 compiler-known
Send 校验，并把非 Copy 命名 binding publication-move 到内嵌 child frame，
再将其调度到有界 FIFO。join 只在目标 child 尚未完成时挂起；child
完成会唤醒一个 owner-local waiter。显式 join 清理与 parent 清理都会执行幂等
child drop。该切片不创建 heap task、OS thread、atomic reference count 或
全局 work-stealing queue。

structured cancel 为未来 shard acknowledgement 路径保留了可挂起语义，但
current-thread owner 可以 inline 完成取消与 frame 清理，所以 ready fast path
既不分配也不会额外往返 ready queue。generated ABI 仍保留
`NOMO_ASYNC_PENDING_CANCEL`，后续 owner-shard 实现可以等待 owner 确认终态
清理，而不改变源码语义。

Browser WASM 的有界沙盒解释器可以运行同一份源码。目前
`task.yield_now()` 只表示 cooperative boundary；它还不会把控制权交还给
host Promise 或浏览器 event loop。`task.sleep` 在 browser sandbox 中既不阻塞
也不求值 duration，而是返回
`TaskError { code: "runtime_unavailable", ... }`。structured child body
目前同样不会在 browser 中执行，其 join 和 structured cancel 返回同一稳定
错误，并消费 inert browser handle。
`task.deadline` 当前会在不求值 duration、也不执行 body 的前提下返回 sandbox
capability error。Channel operation 遵循上面的 capability 行为。Static
select 同样会在 operand 求值前报告 capability error；host-driven browser
deadline、channel 与 select 属于后续 backend 小切片。

## 有意保留的限制

对暂不支持的挂起形态，编译器报告 E0876，而不是生成错误代码。当前
`task.yield_now()` 和不返回值的 suspend 调用必须是独立语句；返回值的 suspend
调用与 `task.sleep(Duration)` 必须作为不可变顶层 `let` 的 initializer。
所在 `suspend fn` 仍须 non-generic；参数、结果和跨 suspension local 必须是
不可变且 frame-safe 的 scalar、string、struct、enum、Result 或已支持 array。
async `main` 仍只返回 `void`。mutable 参数/local、borrow、guard、resource
handle 或包含它的 wrapper、递归 suspend graph、控制流、嵌套表达式或参数表达式
内部挂起、下述 structured binding 之外的 `?`、其他表达式内部的 panic、
取消 token 和 reactor-backed socket/process/HTTP operation 都属于后续小 PR。
当前 P2 foundation 只统一 timer wait，尚不声称 network 或 process handle
已经是 nonblocking。

当前 deadline 小切片允许每个 suspend function 有一个非嵌套
`task.deadline(Duration) { ... }`。body 遵循与 `task.scope` 相同的顶层
structured spawn/join/cancel ownership 规则，并可使用当前已经支持的直接
suspension 形态与 `task.check_cancelled()`。它不产生值，且必须 normal
fallthrough。deadline body 内的嵌套 deadline/scope、控制流、`return`、`?`、
panic、defer 或 unsafe，需要后续 general structured-exit 与 nested-deadline
lowering。v0.1 不暴露 first-class cancellation token。

structured spawn/join 当前只允许出现在顶层 `task.scope` body。每个 spawn
handle 必须使用推导得到的不可变 binding 且不得离开 scope；若要观察结果，
只能由直接不可变 `task.join(handle)` 或 `task.cancel(handle)` binding 恰好
消费一次。structured cancel 在 child 进入终态并移除注册后才返回
`Result<void, TaskError>`；已经完成的 child 也会成功。之后不得再次 join 或
cancel 该 handle。target 必须是直接、未限定、non-generic 的顶层
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
其他位置的 `?`、其他表达式内部的 panic、取消 token、嵌套/通用 deadline
exit、跨 shard channel 与通用 send/join select 仍属于后续切片。P3-C 的
static receive/timer select 仅支持非空、normal fallthrough 且不包含嵌套
suspension 的 arm body。
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
显式 structured-cancel 测试覆盖 armed timer、精确 result/handle ownership
转移、generated pending ABI、native/browser 行为、精确 counter 与
AddressSanitizer 清理。
deadline 测试还覆盖非正时长 body suppression、normal disarm、child frame
持有 armed sleep 时 timeout、typed join failure、root secret-safe failure、
精确 timer/deadline counter、browser 不求值与 AddressSanitizer cleanup。
有界 channel 测试还覆盖 element/byte limit、FIFO wraparound、direct handoff、
full/empty try operation、buffered close、blocked sender/receiver wakeup、
repeated close、timeout cancellation、typed value recovery、跨 suspension
handle liveness、精确 counter、native C99/browser capability 行为与
AddressSanitizer/UndefinedBehaviorSanitizer cleanup。
static-select 测试还会覆盖源码顺序 immediate readiness、suspend-and-wake
arbitration、receive/timer loser removal、精确 select/live resource counter、
C99 生成、browser operand suppression 与 AddressSanitizer cleanup。
P3 manifest 会把同一个
容量 8、32 个值的 exchange 与固定单核 Go 对照执行，但结果仍不具备
performance claim 资格。
后续实现仍必须用测试和证据证明：

- 其余 error、cancellation、timeout 和嵌套表达式或 runtime-originated panic
  路径仅对 frame 中的 ARC/COW 值 release 一次；
- 不允许不安全 mutable borrow 或 guard 跨 suspension point；
- 未使用 suspension 的程序没有 runtime、thread、coroutine metadata 或普通
  collection atomic 成本；
- synchronous-ready 路径不分配、不进入 ready queue；
- 兼容 C99 与 browser WASM，并继续覆盖 Linux、macOS/BSD 和 Windows reactor；
- 固定版本、公平 workload 的 Nomo 与 Go 对比，不能通过削弱对照来达标。

P0/P1/P3 控制组与原始证据格式位于
[`performance/async`](../performance/async/README.zh-CN.md)，当前小切片的可运行
示例位于 [`examples/async_yield`](../examples/async_yield) 与
[`examples/async_timer`](../examples/async_timer)，以及
[`examples/async_structured_void`](../examples/async_structured_void) 与
[`examples/async_structured_results`](../examples/async_structured_results)，以及
[`examples/async_structured_return`](../examples/async_structured_return) 与
[`examples/async_structured_cancel`](../examples/async_structured_cancel)，以及
[`examples/async_structured_return_cancel`](../examples/async_structured_return_cancel) 与
[`examples/async_structured_question_cancel`](../examples/async_structured_question_cancel)，以及
[`examples/async_structured_explicit_cancel`](../examples/async_structured_explicit_cancel)，以及
[`examples/async_structured_panic_cleanup`](../examples/async_structured_panic_cleanup)。
有界 FIFO 示例位于
[`examples/async_bounded_channel`](../examples/async_bounded_channel)。
静态 receive/timer 选择示例位于
[`examples/async_static_select`](../examples/async_static_select)。
