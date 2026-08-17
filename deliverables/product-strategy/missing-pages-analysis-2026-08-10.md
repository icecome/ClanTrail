# 缺失 8 个页面的性质分析与前后端契约断裂报告

日期：2026-08-10
范围：`web/src/pages` 缺失页面成因、分类与后续方案
关联：`frontend-recovery-2026-08-10.md`、`implementation-status-2026-08-10.md`

---

## 一、核心结论（先看这个）

1. **8 个页面不是"被删了没恢复"，而是"从未被创建过"。** git 全历史核查，这 8 个文件的创建次数均为 **0**，不存在任何可供 `git checkout` 恢复的源。
2. **其中 5 个并非功能缺失**，只是把已恢复页面里的现有功能"拆成独立子页"的重构计划——功能今天就能用。
3. **真正的功能缺口只有 3 个**：`GraphPage`（关系图谱 UI）、`PrivacyPage`（隐私锁，后端也无）、`MemberDirectPage`（跨墓位直达人物）。
4. **发现了比缺页更严重的问题：前后端 API 契约已断裂。** 后端已重命名为 `Clan/Grave/Member/Edge/Image`，前端 `client.ts` 仍请求 `families/tombs/persons/photos/tomb-groups`，后端**零个**旧路由别名 → 恢复后的前端**能编译能打包，但运行时大部分接口会 404**。

---

## 二、取证：8 个页面在 git 全历史中的存在性

```
GraphPage.tsx        -> 历史中被创建次数: 0
BackupPage.tsx       -> 历史中被创建次数: 0
PrivacyPage.tsx      -> 历史中被创建次数: 0
GpsPage.tsx          -> 历史中被创建次数: 0
AboutPage.tsx        -> 历史中被创建次数: 0
MemberDetailPage.tsx -> 历史中被创建次数: 0
MemberListPage.tsx   -> 历史中被创建次数: 0
MemberDirectPage.tsx -> 历史中被创建次数: 0
```

对照组（已成功恢复的页面，全部存在过）：

```
ClanListPage / ClanDetailPage / GraveDetailPage / RemindersPage
MapPage / AddGravePage / SettingsPage / MemberFormPage  -> 均为 1
```

git 全历史中出现过的 pages 仅 9 个文件（旧命名）：
`AddTombPage / FamiliesPage / FamilyDetailPage / FamilyListPage / MapPage / PersonFormPage / RemindersPage / SettingsPage / TombDetailPage`

**判定**：`84007f6` 的 `App.tsx` 引用的是作者**规划中的未来页面**，属于"先写路由、后补页面"的写法，页面还没写就提交了。因此"恢复"在物理上不成立。

---

## 三、8 个页面的真实性质分类

### A 类：功能已存在，只是未拆分成独立页（5 个，非缺口）

| 页面 | 功能现状 | 当前所在位置 |
|------|----------|--------------|
| `BackupPage` | **已实现** 导出/导入备份，含覆盖确认弹窗 | `SettingsPage.tsx:16-44,69-78,116-118` |
| `AboutPage` | **已实现** 版本 0.1.0 与产品简介 | `SettingsPage.tsx:95,101` |
| `GpsPage` | **已实现** 定位说明文案（"使用当前经纬度"引导） | `SettingsPage.tsx:107,110` |
| `MemberListPage` | **已实现** 安葬人物列表（含增删） | `GraveDetailPage.tsx:244`（"安葬人物（N）"） |
| `MemberDetailPage` | **已实现** 人物信息与关系的查看/编辑/删除 | `GraveDetailPage.tsx:13-132` + `MemberFormPage` |

> 这 5 个页面若创建，属于**信息架构重构**（把设置页/详情页拆成更细的子页），不是补功能。不做也不影响可用性。

### B 类：真实功能缺口（3 个）

| 页面 | 缺口性质 | 后端支撑 | 实现成本 |
|------|----------|----------|----------|
| `GraphPage` | **P0-1 关系图谱可视化 UI 完全缺失** | 后端已就绪：`/api/clans/:id/graph`、`/api/members/:id/egograph` | 中—高（需选型图渲染库；已有 `feature-plans/relationship-graph-plan-2026-08-09.md` 规划） |
| `PrivacyPage` | **P1-5 隐私锁，全栈均无** | 后端**无**任何 auth/lock 接口 | 高（前后端都要做） |
| `MemberDirectPage` | 跨墓位直达人物详情（`/member/:pid/view`） | 后端有 `/api/members` | 低（B 类里最容易） |

---

## 四、更严重的问题：前后端 API 契约断裂

### 路由对照

| 前端 `client.ts` 请求 | 后端 `lib.rs` 实际提供 | 状态 |
|----------------------|------------------------|------|
| `/api/families` | `/api/clans` | **断裂** |
| `/api/tombs` | `/api/graves` | **断裂** |
| `/api/persons` | `/api/members` | **断裂** |
| `/api/photos`、`/api/photos/upload` | `/api/images`、`/api/images/upload` | **断裂** |
| `/api/tomb-groups` | `/api/burial-groups`、`/api/clans/:id/groups` | **断裂** |
| 关系 `relations` | `/api/edges`、`/api/members/:id/edges` | **断裂** |
| `/api/export`、`/api/import` | 同名 | 对齐 |
| `/api/reminders` | 同名 | 对齐 |
| `/api/dev/seed`、`/api/health` | 同名 | 对齐 |

**后端保留旧路由别名数量：0**（`grep -c "families|/tombs|/persons" src-server/src/lib.rs` = 0）

### 类型层同样断裂

`web/src/types.ts` 仍定义 `Family / TombGroup / Tomb / Person / Photo / Relation`，后端模型已是 `Clan / BurialGroup / Grave / Member / Image / Edge`。

### 实际影响

| 页面 | 运行时状态 |
|------|-----------|
| `SettingsPage`（备份导出/导入） | **可用**（`/export`、`/import` 对齐） |
| `RemindersPage`（忌日提醒） | **可用**（`/reminders` 对齐） |
| `ClanListPage` / `ClanDetailPage` / `GraveDetailPage` / `AddGravePage` / `MemberFormPage` / `MapPage` | **全部 404 不可用** |

> 这解释了 `84007f6` 删除前端的**真实动机**：作者完成了后端全栈重命名（Clan/Grave/Member/Edge），前端页面与 client 全是旧模型，已无法对接，于是整体删除准备按 `deliverables/design-prototype/DESIGN.md` 重写。删除是"重写前的清场"，但新页面尚未写出就提交了。

---

## 五、为什么本次恢复没有补这 8 个页面

1. **物理上无法"恢复"**：git 全历史中不存在这些文件，无源可取。
2. **"补充" ≠ "恢复"**：补充等于从零实现新功能（尤其 `GraphPage`/`PrivacyPage`），属于新增开发任务，超出"让项目恢复可编译可运行"的授权范围，且涉及技术选型（图渲染库、加密方案），需要决策。
3. **优先级判断有误需修正**：当时以"能编译 + 能打包"作为完工标准，**未验证运行时 API 契约**。现已发现契约断裂才是阻塞可用性的头号问题，其优先级**高于**补页面。

---

## 六、建议的后续方案（按优先级）

### P0：修复 API 契约（必须先做，否则页面再多也跑不通）

| 方案 | 做法 | 优点 | 缺点 |
|------|------|------|------|
| **A. 前端适配（推荐）** | 改写 `client.ts` 路径 + `types.ts` 模型名，对齐后端 `Clan/Grave/Member/Edge/Image` | 单点修改、契约唯一真源在后端、无技术债 | 页面内变量名需小幅跟改 |
| B. 后端加旧路由别名 | 后端注册 `/api/families` 等转发到新 handler | 前端零改动、最快 | 双套命名长期共存，技术债 |
| C. 按 DESIGN.md 重写前端 | 遵循作者原意，用设计系统重做全部页面 | 最终形态最好 | 工作量最大，短期不可用 |

### P1：补 B 类真实缺口
1. `MemberDirectPage`（成本最低，先做）
2. `GraphPage`（按 `feature-plans/relationship-graph-plan-2026-08-09.md` 落地，对应 P0-1）
3. `PrivacyPage`（需先定后端隐私锁方案）

### P2：A 类信息架构拆分
`BackupPage / AboutPage / GpsPage / MemberListPage / MemberDetailPage` —— 功能已可用，可在 UI 改版时一并拆分，不紧急。

### 清理项
`App.tsx` 的 `isSubFlow` 仍残留指向未创建子路由的路径字符串（无害，不影响编译），实现对应页面时一并更新。
