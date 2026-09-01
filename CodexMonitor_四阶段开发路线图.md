# CodexMonitor 四阶段开发路线图

> 项目定位：将 CodexMonitor 从“Codex 图形化客户端 + 单实例 Agent Monitor”逐步升级为 **Codex Global Observability / Unified Control Center**，最终具备跨 Surface 观测、会话互通、产品化运行，以及基于真实运行数据的自动模型调度优化能力。

---

## 0. 总体目标

最终希望做到：

```text
Codex CLI / PowerShell
Codex Desktop
IDE Codex
CodexMonitor 内置 Codex
        │
        ↓
Global Source Layer
        │
        ↓
Canonical Thread / Turn / Agent Runtime
        │
        ↓
CodexMonitor
├─ Main / Sub-Agent Tree
├─ Model
├─ Token
├─ Status / Runtime
├─ Workspace / Session
├─ Usage Analytics
└─ Historical Analysis
```

核心要求：

1. 不猜测运行时事实。
2. `Observed ≠ Requested ≠ Estimated`。
3. `LIVE / NEAR LIVE / HISTORICAL` 必须严格区分。
4. 历史数据不得冒充实时数据。
5. 同一个 Thread 从多个来源被观察时不得重复创建 Agent、不得重复累计 Token。
6. UI 只是 Runtime State 的投影，不作为事实来源。
7. Collector / Runtime Store 生命周期必须独立于页面生命周期。
8. 新协议接入遵循：**真实取证 → Fixtures → Contract → TDD → 接入 UI**。

---

# Phase 1 — Local Runtime Observability

## 状态

**PASS / 已完成**

## 目标

解决：

> CodexMonitor 自己启动的 Codex，能否被可靠实时观测？

## 已完成能力

### Runtime 基础设施

- Event Normalizer
- `ThreadRuntimeState`
- `TurnRuntimeState`
- `AgentAssignment`
- 全局常驻 Runtime Store
- hydration / catch-up
- Runtime selector
- duplicate-event 幂等处理
- 轻微乱序保护
- reset / reconnect 非负增量保护
- provenance / source
- server timestamp / observedAt 分离

### Agent Monitor

已支持：

- Main Agent / Sub-Agent 实时调用树
- 父子关系
- Running / Waiting / idle 等已证实状态
- Runtime / duration
- observed model（有协议证据时）
- Input / Output / Cached / Total Token
- Workspace 筛选
- Session 筛选
- Current Chat 自动识别
- Current Session 自动置顶和选择
- 完整 threadId 去重
- Clear Live Runtime
- Live / Historical 严格隔离

### UI 与导航

- Full Page Agent Monitor
- Chat + Agent Monitor 同实例 Split View
- Split View compact layout
- Global Home
- Workspace Overview
- Chat / Thread
- `← Home`
- `← Workspace`
- 全部使用 SPA 内部导航，不依赖 reload

### Conversation 管理

已接入正式 Codex app-server：

```text
thread/delete
```

支持：

- Archive / Delete 分离
- 永久 Delete
- destructive confirmation
- Running / Waiting 删除保护
- Main Thread 删除时按标准行为级联 spawned descendants
- 删除后 reconciliation
- Sidebar / Recent Threads / Agent Monitor 同步清理

## 人工验收

- 场景 A：PASS
- 场景 B：PASS
- 场景 C：PASS
- 场景 D：PASS

## 已知非阻塞边界

- Sub-Agent observed model 可能没有可靠 LIVE 来源，因此允许 `unavailable`
- requested model 不得冒充 observed model
- `model/rerouted` 尚需真实 fixture
- Token 是 usage/model-call 边界更新，不是逐 token streaming
- 父 Thread 与子 Thread Token 包含关系未知时保持独立
- `turn/completed ≠ thread completed`
- `notLoaded` 不等于 Completed / Failed

## Phase 1 冻结原则

除明确回归或协议升级外：

> **不要再重构 Phase 1 Runtime 核心语义。**

Phase 1 作为后续所有来源接入的可信基线。

---

# Phase 2 — Global Sources

## 状态

**PASS / COMPLETE**

## 核心目标

解决：

> 不是由 CodexMonitor 自己启动的 Codex，Monitor 能否同样观察？

目标来源：

```text
Monitor-owned app-server → LIVE
CLI / PowerShell rollout → NEAR LIVE
Codex Desktop rollout    → NEAR LIVE
Ended Sessions           → HISTORICAL
```

---

## Phase 2.1 — Global Source Core

### 状态

**PASS**

### 已完成

建立统一：

- `SourceEnvelope`
- sourceKind
- temporalClass
- sourceInstanceId
- codexHome identity
- source file identity
- generation
- byte cursor
- source timestamp
- observed timestamp
- lag
- freshness
- schema fingerprint
- confidence / provenance

身份规则：

```text
CodexThreadKey
= (codexHome.identity, fullThreadId)

CodexTurnKey
= (CodexThreadKey, fullTurnId)
```

来源优先级：

```text
app-server LIVE
    >
rollout NEAR LIVE
    >
HISTORICAL
```

关键规则：

- 同一个 Thread 的不同来源进入不同 source lane
- Token 不跨来源相加
- rollout 不覆盖 fresh LIVE
- LIVE → rollout fallback 使用累计 snapshot，不做跨来源 delta
- HISTORICAL 不驱动 Running / Waiting

---

## Phase 2.2 — Rollout Tail Watcher

### 状态

**PASS**

### 已完成

数据链：

```text
CodexHomeSource[]
→ rollout discovery
→ filesystem wake signal
→ periodic reconciliation
→ read_rollout_delta()
→ RolloutRecordParser
→ SourceEnvelope
→ SourceAuthorityRegistry
```

支持：

- 多 CODEX_HOME 基础结构
- rollout 文件发现
- append
- resume 继续写同一文件
- UTF-8 byte cursor
- partial-line buffer
- checkpoint
- generation/reset
- Windows read/write/delete sharing
- duplicate filesystem notification
- missed notification reconciliation
- retry/backoff
- freshness / health

重要原则：

> OS watcher 只负责“唤醒”，byte cursor + reconciliation 才负责正确性。

---

## Phase 2.3 — External CLI Near-Live E2E

### 状态

**PASS**

### 已完成

真实端到端链路已经验证：

```text
独立 PowerShell
    ↓
codex CLI
    ↓
rollout JSONL
    ↓
Rollout Tail Watcher
    ↓
Source Authority Registry
    ↓
CodexMonitor
```

- 独立 CLI 新 Session 自动发现
- 第二 Turn / resume
- Main + Sub-Agent
- observed model
- Thread / Turn Token
- confirmed parent / child
- Near-Live lag
- checkpoint restart
- temporary lock recovery
- missed notification + reconciliation
- 同 Thread LIVE + rollout canonical 去重
- Token 不跨来源重复累计
- Phase 2.3b A：真实 LIVE + rollout Pair Gate PASS
- Phase 2.3b B：CLI-running-before-Monitor Gate PASS

---

## Phase 2.4 — CLI → Agent Monitor UI

### 状态

**PASS**

### 已完成

实现：

> 在普通 PowerShell 中运行 Codex 时，Agent Monitor 自动 Near-Live 出现对应任务。

已建立：

- 后端只读 canonical Source Snapshot / update API
- 独立 `GlobalSourceViewStore`
- Phase 1 LIVE Runtime + Global Source canonical view 的统一 selector
- `(codexHome.identity, fullThreadId)` canonical 去重
- fresh LIVE 优先
- LIVE stale → rollout cumulative snapshot fallback
- Token 不跨来源相加
- All Sources / Monitor LIVE / CLI NEAR LIVE 来源筛选
- Session / Agent source 与 freshness 标识
- 外部 CLI Main / Sub-Agent、Model、Status、Runtime、Token UI 投影

UI 来源示例：

```text
Source: CLI
Temporal: NEAR LIVE
Freshness: 1.8s ago
```

### 人工验收

- 场景 A：PASS。Monitor 先启动并打开 Agent Monitor，随后外部 CLI 新建 Main + 两个 Sub-Agent；Session 自动以 NEAR LIVE 出现，无需刷新。
- 场景 B：PASS。外部 CLI 先运行，Monitor 后启动；成功 catch-up 并继续读取后续 append。
- 场景 C：PASS。同一 Monitor-owned Thread 同时存在 LIVE + rollout；UI 仅显示一个 canonical Session / Agent，LIVE 优先且 Token 不双算。

---

## Phase 2.4.1 — Agent Monitor UX / Observability Polish

### 状态

**PASS**

### 已完成

- Activity Filter：`Active / Fresh`、`All`、`Settled`，默认 `Active / Fresh`。
- 默认排序：Current、Running、Waiting、fresh LIVE、fresh NEAR LIVE、stale、settled/completed；同级按最后 observation/activity 倒序。
- 不同完整 `fullThreadId` 保留为独立 Session；相同 canonical identity 去重；多个 Turn 不生成多个视觉 Session。
- 已确认的 observed model 在 completed / stale / settled 后继续保留；model provenance 与 lifecycle/token authority 分离。
- 顶部 Token 指标明确为 `ROOT THREAD TOKENS`：仅显示当前唯一可见 Root canonical Thread 的 authoritative cumulative snapshot，不累加 child，也不跨 LIVE / rollout 相加。
- UI 明确区分 LIVE、NEAR LIVE、stale、settled 与 HISTORICAL；lifecycle 与 freshness 保持分离。
- Clear Live Runtime 的范围明确为 Monitor LIVE Runtime，不清理 CLI rollout、Global Source 或 Historical Session。
- Split View compact layout 与 Full Page 宽布局保持。
- Diagnostic journal 与长期 E2E Evidence Summary 分离；Evidence 采用字段白名单，不保存 raw prompt、raw reasoning 或敏感 diagnostic。

核心产品原则：

> Agent Monitor 主界面回答“现在发生什么”，历史 Session 由次级视图查看。

### Phase 2.4.1b — Global Source Historical Reconciliation Fix

**PASS**

- Rollout delta 先整批解析，再原子应用到 Source Authority Registry；解析失败不留下半提交 lifecycle，也不推进 cursor/checkpoint。
- 兼容真实旧 rollout 缺少 `cache_write_input_tokens`、`info`、`agent_path`、`session_id` 与旧 `task_complete.started_at` 的情况；非关键缺失保持 unavailable，不伪造为 `0`。
- freshness 以可靠 `sourceTimestamp` 为主，`observedTimestamp` 只描述 Monitor 看到记录的时间。
- stale unresolved Running / Waiting 只在 `All` 中展示，不污染默认 `Active / Fresh`。
- Summary 语义随 Activity Filter 变化：`Active / Fresh` 显示 fresh `ACTIVE`，`All` 显示带说明的 `RECORDED ACTIVE`，`Settled` 不显示活动统计卡。

### Phase 2.4.2 — Cross-View Deletion Reconciliation

**PASS / COMPLETE**

- 官方 `thread/delete` 成功后先持久化 deletion tombstone，再清退 Registry、Watcher/source、checkpoint 与 Monitor-owned cache；删除失败不创建 tombstone。
- tombstone 是 crash recovery 的持久依据；未完成 reconciliation 在启动时恢复并幂等重试。
- Registry 与 Watcher 双层拒绝 tombstoned Thread 的 LIVE / NEAR LIVE / HISTORICAL 再 ingest，旧 checkpoint、旧 filesystem event 与 historical discovery 均不得使其复活。
- Main 与 confirmed descendants 按 canonical Thread key 级联清退；无关 Thread 保持不变。
- checkpoint 不再保留 deleted rollout path，retired path 不再新增对应 `os error 2`。
- 清理 Monitor-owned activity、pin、custom name、thread params 与 detached-review 缓存。
- Desktop sidebar 仍只是 consumer view；不得修改 `session_index.jsonl`、`.codex-global-state.json`、`state_5.sqlite` 或 Desktop 私有 cache。

### 自动验证

- 聚焦测试：28/28 PASS。
- `npm run typecheck`：PASS。
- `npm run lint`：PASS，0 errors。
- `git diff --check`：PASS。
- Evidence Writer：PASS。
- 前端全量：1078/1084 PASS；其余 6 项是既有 locale/date 环境断言，与 Agent Monitor / Global Source 无关。

---

## Phase 2.5 — Codex Desktop Near-Live

### 状态

- Desktop forensics：**PASS**
- admission：**PASS**
- Desktop Near-Live overall：**COMPLETE**
- Slice 1 — File Owner / Replay Guard / Child Execution Boundary：**PASS**
- Slice 2 — Desktop Metadata + Producer Surface Classifier：**PASS**
- Desktop Near-Live Real E2E A/B/C/D：**PASS**
- Final Agent Monitor UI：**PASS**
- Phase 2.5：**PASS**
- Phase 2 Global Sources：**COMPLETE**

### 目标

让 Codex Desktop 运行的任务也进入 Global Monitor。

优先复用：

- 默认 CODEX_HOME
- rollout adapter
- session metadata
- workspace/thread metadata

禁止：

- OCR
- 截屏识别
- UI scraping

真实取证范围：

- Desktop Session 身份
- Workspace 映射
- CLI / Desktop 来源判别
- Desktop rollout 写入行为
- Desktop 正在运行时的 freshness
- Desktop 与 CLI / Monitor 同 Thread 去重

注意：

`originator` 不能单独作为 CLI / Desktop 来源判断依据。

### 真实取证结论

- Desktop 实际使用默认 `C:\Users\DeanX\.codex`，Main 与三个直接 Sub-Agent 均写入标准 rollout 目录。
- Main 多 Turn / resume 继续追加同一 rollout；Sub-Agent 各有独立 rollout、confirmed parent、model、Token 与 lifecycle。
- Watcher 实测完整记录 lag 为 8–1334 ms，保持 `NEAR_LIVE`。
- Desktop Main 的 `source=vscode` 与 Desktop project/thread metadata 可组成强来源证据；`originator` 仍只作弱证据，单独 `source=vscode` 仍可能与 IDE 混淆。
- 长/compacted Thread 的 child rollout 会重放 parent `session_meta`；Slice 1 已固定 generation file owner 并隔离 boundary 前的 parent replay。

### Real E2E A/B/C/D 正式结论

```text
Phase 2.5 Desktop forensics = PASS
Slice 1 File Owner / Replay Guard = PASS
Slice 2 Metadata + Producer Surface = PASS
Real E2E A/B/C/D = PASS
Final Agent Monitor UI = PASS
Phase 2.5 = PASS
Phase 2 Global Sources = COMPLETE
```

**Desktop Near-Live Real E2E = PASS**

- Gate A — Monitor First：真实 Desktop Main/Sub-Agent 以独立 canonical `NEAR_LIVE` Thread 进入 Registry；confirmed parent、DESKTOP、model、lifecycle、workspace、累计 Token 与实时 tail 均正确。
- Gate B — Desktop First：Monitor 在 Main Running、Child 已完成时启动；503/648 ms 内完成首次 canonical observation，catch-up 重建状态，Main 后续 tail 为 406–803 ms，completion latency 为 293 ms。
- Gate C — Stale Orphan：目标被判定为 `DESKTOP_STALE_ORPHAN`，canonical Registry / Agent Monitor node 均为 0，未写 Desktop 私有数据库。
- Gate D — Surface Separation：Desktop 保持 `DESKTOP`；真实 external exec 即使携带弱 `originator=Codex Desktop` 仍保持 `CLI`。
- 每个真实 Thread 只有一个 canonical lane；Main/Child 不折叠，无重复节点、无 Token double count、无 Desktop/CLI 误分类。
- 脱敏 evidence 总索引：`docs/evidence/phase-2-5-real-e2e-summary.md`。

### Desktop Projection / Thread Authority Amendment

```text
canonical Thread existence
!= Desktop local_thread_catalog membership
!= Desktop sidebar visibility
```

- `local_thread_catalog`、`.codex-global-state.json`、project membership、sidebar/WebView 状态仅是 Desktop-owned supplemental projection metadata，不得单独创建 canonical Thread、Registry lane、Agent Runtime 或 Agent Monitor node。
- 权威优先级固定为：`Monitor deletion tombstone > confirmed rollout identity > authoritative app-server/persisted Thread state > Desktop projection metadata`。
- `session_index.jsonl` 存在与否都不是 Thread existence 的必要条件。
- 同一 `CodexThreadKey` 已有 Monitor deletion tombstone 时，后续 Desktop catalog/sidebar 观察不得使其复活；同标题但不同 full thread id 是独立 Thread。
- `DESKTOP_STALE_ORPHAN` 仅表示 Desktop projection 仍引用完整 fullThreadId、但 canonical Thread 已不存在。它不进入 `LIVE` / `NEAR_LIVE` / canonical `HISTORICAL`，不创建 Agent Runtime 或 Monitor node，只保留诊断观察。
- 无 tombstone 时，stale orphan 必须同时满足：精确 fullThreadId 仍在 catalog/sidebar、rollout 不存在、authoritative persisted Thread 不存在、`thread/read` 明确 nonexistent/not-found。证据不足或冲突时保持 `AMBIGUOUS`，不得猜测 ingest。
- Phase 2.5 禁止写入 `codex-dev.db`、`local_thread_catalog`、`state_5.sqlite`、`.codex-global-state.json` 和 Desktop WebView/cache。
- 正式 TDD gate 必须覆盖 stale catalog 完整证据、stale row + tombstone、合法 catalog + valid rollout、catalog-only ambiguous、同标题不同 fullThreadId 五类 fixture。
- 第一 Formal TDD 切片 `Desktop Compacted Child Rollout -> File Owner -> Replay Guard -> Child Execution Boundary` 已 PASS。
- 第二 Formal TDD 切片已实现只读 Desktop metadata reader、Producer Surface classifier、workspace/project mapping 与 `DESKTOP_STALE_ORPHAN` admission gate；Desktop 私有数据仍保持只读。

`docs/fixtures/desktop-rollout/desktop-subagent-compacted-prefix.jsonl` 的 file-owner/replay gate、Desktop projection/metadata fixtures、Real E2E A/B/C/D 与 Final Agent Monitor UI 均已通过。Phase 2.5 与 Phase 2 Global Sources 正式收口。

### 当前开发纪律

- Phase 1 Runtime 核心保持冻结，除明确回归外不修改其语义。
- Desktop 复用现有 rollout watcher、Global Source Core、Source Authority Registry 和 canonical view，不新增第二套 watcher。
- Global Source Core、Slice 1、Slice 2 与 Real E2E 修复保持冻结 PASS；无明确回归证据不得重做。
- Final UI 已交付 Desktop producer-surface filter/label、canonical Main/Sub-Agent 字段渲染、projection-only exclusion 与 Current Session 隔离。
- Phase 2 已冻结；本次收口停止，不开始 Phase 3。

### Locale baseline waiver

- Known non-blocking test debt：6 个 Phase 2.5 开始前已存在的 zh-CN locale/date assertions。
- Full frontend：1088 / 1094 PASS。
- New Phase 2.5 regressions：0。
- waiver 已批准；这 6 项不阻塞 Phase 2.5，本阶段不修复。

---

## Future Backlog — Historical Unified View

### 状态

**DEFERRED / NOT A PHASE 2 COMPLETION GATE**

### 目标

统一已结束 Session：

```text
Monitor
CLI
Desktop
IDE
```

进入统一 History Domain。

支持：

- 项目级历史
- Session级历史
- Agent历史
- Model usage
- Token usage
- Runtime
- Source
- freshness / settled

原则：

> Historical 永远不反向污染 Live Runtime。

---

## Phase 2 完成标准

当以下条件成立，可判定 Phase 2 PASS：

- Monitor-owned Codex：LIVE
- CLI：NEAR LIVE
- Desktop：NEAR LIVE
- Historical：后续独立统一读取，不阻塞本轮 Global Sources Near-Live 完成判定
- 同 Thread 跨来源不重复
- Token 不双算
- source/provenance/freshness 明确
- 外部 CLI/Desktop 任务可在 Monitor 中可靠出现

---

# Phase 3 — Cross-Surface Interoperability

## 目标

从“统一看见”升级为：

> **统一 Session / Project / Conversation 互通。**

目标 Surface：

- CodexMonitor
- Codex CLI
- Codex Desktop
- IDE Codex
- 后续 Codex Remote / Mobile

## 核心能力

### Phase 3.1 当前实现边界

External Thread admission / resume capability 使用独立的 Phase 3 状态，按
`CodexThreadKey` 关联 Phase 2 canonical observation：

```text
CodexThreadKey
    ↓
Phase 2 canonical observation
    +
Phase 3 admission / resume capability
```

Phase 2 Global Source Core 继续只承担 observed runtime truth、canonical
identity 与 source authority，不保存 resume/control-plane 语义。Phase 3
admission 状态位于 `shared/codex_core/external_thread_admission.rs`，包含
`exists`、`resumable`、writer/occupancy、workspace assignment、Surface
projections、`projectAssigned` 与 `sidebarVisible`。

当前约束：

- 相同完整 ID 的多 Surface projection 只对应一个 admission record；同标题不同 ID 独立。
- tombstone 优先，已删除 identity 不被后续较低权威 observation 复活。
- workspace assignment 复用 longest-root；同长冲突保持 ambiguous/unassigned。
- writer/occupancy 无直接证据时为 `UNKNOWN`，不支持 force takeover。
- `projectAssigned` 与 `sidebarVisible` 只接受直接 Surface projection evidence；cwd、catalog presence 或 source kind 均不能推导它们。
- exact-ID `thread/read` / `thread/resume` 统一构造；成功响应的 `result.thread.id` 必须与请求完整 ID 完全一致。
- `thread/resume` 不 fallback 到 `thread/start`，也不等同于 `turn/start`。
- Phase 3.1.1 本地 contract/state TDD 已 PASS；真实六向 Resume E2E 为 GO / NOT STARTED，Phase 3.2 为 NOT STARTED。
- 6 个既有 zh-CN locale/date failures 继续作为已批准的 non-blocking test debt，不阻塞 Phase 3.1.1。
- 本地不存在 `../Codex`，因此 upstream protocol hash 未刷新；该项记为 non-blocking verification gap。

### 1. Project / Workspace 互通

Monitor 能够读取 Codex Desktop / CLI 使用的项目和 Workspace。

### 2. Conversation / Session 互通

Monitor 能浏览：

```text
Workspace
├─ Session A
├─ Session B
└─ Session C
```

并显示完整 Conversation。

### 3. Monitor 创建标准 Codex Session

Monitor 新建的正常 Main Session：

- Desktop 能识别
- CLI 能 resume
- Remote 能继续

禁止创建只能由 Monitor 自己识别的私有 Session。

### 4. Main Session 与 Sub-Agent 的呈现边界

建议：

```text
Desktop / Mobile
→ 主要显示用户 Main Session

Agent Monitor
→ 展示完整 Main → Sub-Agent Tree
```

避免 Sub-Agent Thread 塞满普通 Conversation Sidebar。

### 5. Session Identity

统一：

```text
Project
→ Thread
→ Turn
→ Agent Assignment
```

保证不同 Surface 指向同一个 canonical entity。

## Phase 3 完成标准

- Monitor 能浏览 Desktop/CLI 项目与历史对话
- Monitor 创建的标准 Main Session 可被其他 Codex Surface识别
- CLI / Desktop 可继续同一 Session
- Main/Sub-Agent 不发生错误重复
- 跨 Surface 的 Thread identity 稳定

---

# Phase 4 — Productization

## 目标

把 CodexMonitor 从开发项目变成真正每天使用的软件。

## 主要能力

### 启动体验

- 打开 Codex 自动伴随启动 Monitor
- 或统一启动器：
  - Codex
  - Monitor
- Monitor 可后台常驻

### 系统托盘

支持：

- Idle
- Running
- Sub-Agent active
- Completed
- Failed

任务开始时自动更新托盘状态。

### 独立 Monitor Window

最终支持：

```text
显示器 1
Codex Chat / IDE

显示器 2
Agent Monitor
```

要求共享同一 Runtime Source，而不是启动两个独立 Runtime Store。

### 正式发布

- Windows `.exe`
- Installer
- Desktop shortcut
- 自动更新
- 配置迁移
- 稳定版本管理
- 正式日志与诊断
- Crash recovery

### UI 产品化

- Compact / Full 模式
- Source / freshness
- Agent Timeline
- Session History
- Usage Analytics
- 项目级统计

## Phase 4 完成标准

> 不需要开发环境即可像普通桌面软件一样长期稳定运行。

---

# Advanced Phase — Adaptive Model Router

> 高级阶段，保留但不在四个核心阶段完成前提前实施。

## 目标

从：

> “Monitor 告诉我 Agent 用了什么模型”

升级为：

> “Monitor 根据长期真实数据帮助 Model Router 选择更合理的模型。”

最终甚至可以：

> 自动调整 Main / Sub-Agent 的模型策略。

---

## 第一阶段：分析，不自动控制

收集：

```text
Task Type
Agent Role
Model
Token
Cached Token
Runtime
Retries
Success / Failure
Review Result
```

形成：

```text
Repository Search
→ Luna 平均成本最低

Implementation
→ Sol 稳定性最高

Risk Review
→ Terra 质量更高
```

---

## 第二阶段：Router Recommendation

Monitor 输出建议：

```text
Current routing:
Explorer → Sol

Historical evidence:
Luna achieves comparable result
with 42% lower token usage

Recommendation:
Explorer → Luna
```

只建议，不自动修改。

---

## 第三阶段：Controlled Auto-Tuning

加入明确安全边界后：

```text
Observability
    ↓
Performance Analytics
    ↓
Routing Recommendation
    ↓
Policy Guard
    ↓
Model Router Update
```

可以自动调整：

- Explorer model
- Developer model
- Reviewer model
- reasoning effort
- fallback order

---

## 自动调整硬约束

### 不能只看 Token

模型评价至少考虑：

```text
Cost
+ Latency
+ Success rate
+ Retry rate
+ Review quality
+ Task complexity
```

### 必须保留回滚

每次自动策略变化：

- versioned
- auditable
- reversible

### 必须有最低样本量

不能因为 1~2 次任务就改变长期 Router 策略。

### 高风险角色更保守

例如：

- Risk reviewer
- destructive operation reviewer
- architecture reviewer

不能仅因成本降低自动切到能力明显更弱模型。

### Observability 仍然是事实层

Adaptive Router 只能消费观测数据：

```text
Runtime facts
→ Analytics
→ Decision
```

不能反过来修改观测事实。

---

# 项目总体架构终态

```text
                         Codex Ecosystem

         CLI       Desktop       IDE       Monitor
          │           │           │           │
          └───────────┴───────────┴───────────┘
                          │
                  Global Source Layer
                          │
          ┌───────────────┼────────────────┐
          │               │                │
        LIVE          NEAR LIVE       HISTORICAL
          │               │                │
          └───────────────┴────────────────┘
                          │
                  Canonical Runtime
                          │
          ┌───────────────┼──────────────────┐
          │               │                  │
      Agent Tree      Usage Analytics    Session History
          │               │                  │
          └───────────────┴──────────────────┘
                          │
                    Model Analytics
                          │
                  Adaptive Model Router
```

---

# 开发纪律

后续所有阶段继续遵守：

1. **Evidence first**
2. 真实协议取证后再编码
3. Fixtures 必须脱敏并可重放
4. TDD 优先
5. 无证据字段保持 unavailable
6. Live / Near-Live / Historical 严格分层
7. requested / observed / estimated 严格分离
8. Token 不跨来源重复累计
9. UI 不作为事实来源
10. 新阶段不得无理由破坏上一阶段冻结基线

---

# 当前进度

```text
Phase 1 — Local Runtime Observability
PASS ✅

Phase 2 — Global Sources
├─ 2.1 Global Source Core
│  PASS ✅
├─ 2.2 Rollout Tail Watcher
│  PASS ✅
├─ 2.3 External CLI Near-Live E2E
│  PASS ✅
├─ 2.4 CLI → Agent Monitor UI
│  PASS ✅
├─ 2.4.1 Agent Monitor UX / Observability Polish
│  PASS ✅
├─ 2.4.1b Global Source Historical Reconciliation Fix
│  PASS ✅
├─ 2.4.2 Cross-View Deletion Reconciliation
│  PASS / COMPLETE ✅
├─ 2.5 Desktop Near-Live
│  FORENSICS PASS / SLICE 1 PASS / SLICE 2 PASS / REAL E2E A/B/C/D PASS / FINAL UI PASS ✅
└─ Historical Unified View
   DEFERRED / NOT A PHASE 2 COMPLETION GATE

Phase 3 — Cross-Surface Interoperability
├─ 3.0 FORENSICS COMPLETE
├─ 3.1.1 PASS
├─ Cross-Surface Resume Real E2E
│  GO / NOT STARTED
└─ 3.2 NOT STARTED

Phase 4 — Productization
NOT STARTED

Advanced — Adaptive Model Router
RESERVED
```

---

# 下一开发起点

下一任务：

**Cross-Surface Resume Real E2E（GO / NOT STARTED）**

核心验收：

> Phase 2.5 Desktop forensics、Slice 1、Slice 2、Real E2E A/B/C/D 与 Final Agent Monitor UI 均已 PASS；Phase 2 Global Sources 正式 COMPLETE。

真实取证报告：`docs/desktop-near-live-forensics.md`。

Phase 3.1.1 已 PASS。Cross-Surface Resume Real E2E 仅为 GO，尚未开始；
Phase 3.2 仍为 NOT STARTED。
