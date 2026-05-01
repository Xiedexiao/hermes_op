# TuriX-CUA Runtime Bridge 契约说明

## 1. 文档目标

本文档只定义一件事：`TuriX-CUA` 在 `Hermes Operator` 里的真实角色是**桌面能力 benchmark + 外部 runtime bridge 参考**，不是已经内嵌进 Hermes 原生核心的 parity 层。

如果没有真实系统权限、没有外部 bridge 进程、没有可审计的 handoff，文档和 UI 都不能把桌面 GUI 能力写成“已经可用”。

## 2. 三层边界

| 层级 | 角色 | 不能被误写成 |
| --- | --- | --- |
| Hermes 原生安全闭环 | 负责任务理解、规划、校验、dry-run、审计和可回放 handoff | 真实桌面 GUI 控制权 |
| TuriX 外部 runtime bridge | 负责把 handoff 交给外部桌面运行时，并在真实权限存在时执行 GUI 动作 | Hermes 核心里的原生 parity 层 |
| OSWorld / SOTA / 真实 GUI 权限 | 真实环境里的 benchmark 和系统权限依赖 | 可被截图、日志或文档“模拟出来”的能力 |

## 3. 契约范围

以下契约属于外部 bridge 面：

- `turix_cua_probe`
- `turix_cua_plan_command`
- `turix_cua_run`

这组契约只描述外部 runtime 的连接状态、handoff 包装、启动前置条件和执行结果，不描述 Hermes 核心的原生 GUI 能力。

## 4. 与本地 Runtime Adapter 的边界

以下契约仍然属于 Hermes 本地 runtime adapter：

- `runtime_adapter_execute_skill_tool`
- `runtime_adapter_probe_desktop_executor`
- `runtime_adapter_execute_desktop_action`

这些契约处理本地命令、dry-run 探测和审计事件。它们不等同于真实 GUI bridge。

## 5. 运行时约束

- Hermes 原生安全闭环只能声明自己完成了什么准备、校验和审计。
- `TuriX-CUA` bridge 只能在真实宿主环境和真实权限存在时返回可执行结果。
- 如果 Accessibility、屏幕录制、窗口控制或宿主权限缺失，bridge 必须显式失败。
- 任何 OSWorld 分数、SOTA 结论或 GUI 成功率，都必须绑定真实 benchmark 或真实执行结果，不能从静态文档推导。

## 6. 不建议使用的表述

- “已实现 TuriX parity”
- “Hermes 原生支持真实 GUI bridge”
- “通过文档/截图即可证明 OSWorld 或 SOTA 能力”
- “没有系统权限也能完成真实桌面操作”

## 7. 关联文档

- [Hermes Operator 功能设计文档](./hermes-agent-desktop-functional-design.md)
- [Hermes Operator 领域模型与契约文档](./hermes-agent-desktop-domain-contracts.md)
