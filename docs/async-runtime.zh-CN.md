# 异步运行时

本文记录 Proposed 异步与并发 RFC 在当前工具链中的真实实现状态，不代表所有
RFC acceptance gate 已经通过。

- [RFC 0031](https://github.com/nomo-lang/rfcs/blob/main/zh-CN/rfcs/0031-direct-style-suspend-functions-and-structured-concurrency.md)
  定义 direct-style suspend effect、stackless lowering、frame 析构与结构化并发。
- [RFC 0032](https://github.com/nomo-lang/rfcs/blob/main/zh-CN/rfcs/0032-sharded-executor-reactor-and-blocking-pool.md)
  定义 executor/reactor、owner affinity、平台后端与 blocking pool 迁移。
- [RFC 0033](https://github.com/nomo-lang/rfcs/blob/main/zh-CN/rfcs/0033-concurrency-capabilities-and-shared-storage.md)
  定义跨任务转移与显式共享能力。
- [RFC 0034](https://github.com/nomo-lang/rfcs/blob/main/zh-CN/rfcs/0034-async-runtime-acceptance-gates.md)
  定义正确性、可移植性、内存和性能门禁。

[English](async-runtime.md)

## 语言表面

可能挂起的函数使用 `suspend fn`，调用点保持 direct-style：

```nomo
package app.main

import std.io
import std.task

suspend fn main() -> void {
    io.println("before")
    task.yield_now()
    io.println("after")
}
```

普通 `fn` 不能调用 suspend 函数；编译器报告 E0870，而不是偷偷引入运行时。
只声明或调用 always-ready suspend 函数不会创建 executor。

## 已实现的 P1 小切片

Native C99 后端遇到根函数中的 `task.yield_now()` 时会生成：

- 栈上分配且带显式 state 的 root frame；
- 返回 `READY` 或 `PENDING` 的 poll 函数；
- inline initial poll；
- 仅在返回 `PENDING` 后进入的单槽 current-thread ready queue 路径；
- 幂等的 root-frame drop 函数。

这一小切片不会创建 OS thread、heap task、reactor 或 atomic metadata。生成的
context 会在内部记录 poll、yield、入队和出队计数；等 P1 counter contract
稳定后再用版本化 benchmark 导出。

Browser WASM 的有界沙盒解释器可以运行同一份源码。目前
`task.yield_now()` 只表示 cooperative boundary；它还不会把控制权交还给
host Promise 或浏览器 event loop。

## 有意保留的限制

对暂不支持的挂起形态，编译器报告 E0876，而不是生成错误代码。当前
`task.yield_now()` 必须是无参数根 `suspend fn main() -> void` 中的独立语句。
跨 yield 存活的局部变量、嵌套 suspend 调用、控制流或表达式内部挂起、非 void
结果、timer、spawn/join、取消和 reactor I/O 都属于后续小 PR。

既有 `task.spawn` 仍是兼容用的隔离 native worker API，不是新的 async task
constructor，而且当前仍是一 worker 一 native thread。RFC 0032 要求后续将它
迁移到有界、惰性的 blocking pool。

## 正确性与成本门禁

后续实现必须用测试和证据证明：

- completion、error、cancellation、timeout 和 panic 路径仅对 frame 中的
  ARC/COW 值 release 一次；
- 不允许不安全 mutable borrow 或 guard 跨 suspension point；
- 未使用 suspension 的程序没有 runtime、thread、coroutine metadata 或普通
  collection atomic 成本；
- synchronous-ready 路径不分配、不进入 ready queue；
- 兼容 C99 与 browser WASM，并继续覆盖 Linux、macOS/BSD 和 Windows reactor；
- 固定版本、公平 workload 的 Nomo 与 Go 对比，不能通过削弱对照来达标。

P0 控制组与原始证据格式位于
[`performance/async`](../performance/async/README.zh-CN.md)，当前小切片的可运行
示例位于 [`examples/async_yield`](../examples/async_yield)。
