# 族迹 ClanTrail 实现状态核对报告

**日期**：2026-08-10
**对照文档**：
- `brainstorm-tomb-keeper-2026-08-06.md`（头脑风暴版，纯离线 / 无强制账号）
- `product-strategy-rev-account-2026-08-07.md`（修订版，本地优先 + 可选账号 + 图模型一步到位）
**核对方式**：直接读源码（`src-core/`、`src-server/`、`web/src/`、`web/src-tauri/`、迁移 V1–V4）并比对 git 历史。

---

## 一、一句话结论

**后端（数据 + 计算）已把 P0 能力基本全量实现；但前端 UI 在最近一次提交（84007f6）中被整体删除，当前源码不可编译；账号/同步、离线地图、本地通知推送、隐私锁、PDF、二维码、家族协作均未落地。** 修订版相对头脑风暴版的新增项（可选账号、图模型、家族协作）仅有数据层的“预留”（如 `Edge.side`、同步列 `version/deleted`），没有任何实现代码。

---

## 二、核对发现的严重异常

`git ls-files web/src` 当前仅跟踪 8 个文件：
`App.tsx`、`api/client.ts`、`main.tsx`、`styles.css`、`types.ts`、`utils/{coord,location,navigation}.ts`、`vite-env.d.ts`。

而 `git show --stat 84007f6`（"refactor: 完成项目重命名为族迹 ClanTrail"）显示该提交**删除了全部页面与组件**：
`web/src/pages/AddGravePage.tsx`（198 行）、`web/src/components/{Toast,BottomSheet,ConfirmSheet,Layout,MapNavSheet}.tsx` 等全部 `--`。

后果：
- `App.tsx` 仍 `import` 16 个页面（如 `./pages/ClanListPage`）和 `./components/Toast`，但文件已不存在 → **项目当前无法 `npm run build` / 无法启动**。
- `web/dist/` 里 8/7 的 JS 包是删除前的旧构建残留，**不能反映当前源码，也无法在源码层继续扩展**。

> 建议立即从父提交 `b280586` 恢复 `web/src/pages/*` 与 `web/src/components/*`（`git show b280586:web/src/pages/xxx.tsx` 可取回），再对齐 84007f6 的路由/字段重命名。

---

## 三、已实现明细（可运行，均在后端）

| 规划项 | 状态 | 证据 |
|--------|------|------|
| **P0-1 图关系模型（Person 稳定 ID + Relation 图）** | ✅ 后端已实现 | `Member.id` 用 `Uuid` 稳定 ID；`Edge` 实体含 `spouse/son/daughter` + `side(born/married)` 支持跨族；`list_graph_by_clan` / `list_egograph`（BFS）已落地 |
| **P0-2 忌日/节日本地提醒（计算层）** | ✅ 后端已实现 ⚠️ 推送未接 | `db.list_reminders` 用 `lunar-lite` 做农历换算（1900–2100），含清明/重阳/春节/中元/寒衣节；但 `web/src-tauri/src/lib.rs` **未初始化 notification 插件**，OS 级本地通知未接 |
| **P0-3 真实照片本地存储** | ✅ 后端已实现 | `upload_image` / `upload_image_base64`：相对路径落 `upload_dir`、`file_path` 存 DB、魔数校验、目录穿越防护 |
| **P0-4 数据备份/导出（schema 版本）** | ✅ 后端已实现 | `/export`（zip + `manifest.json` + `schema_version=4` + `VACUUM INTO`）/`/import`（校验 + 回滚 + 保留 5 份）；迁移已到 V4 |
| **P1-4 检索筛选（基础）** | 🟡 部分实现 | `search_graves/members/clans` 关键词 `LIKE`；**无拼音首字母、无按墓园/辈分维度筛选、无独立索引** |
| **P1-2 墓位导航（调起外部地图）** | 🟡 工具函数已实现 | `utils/navigation.ts` 的 `openExternalMap` 构造高德/百度/腾讯 URI（仅传坐标+地名，符合隐私约定），断网/无 App 由系统兜底；但调用它的 `MapPage` 已删除 |
| **数据模型预留（为同步/协作）** | 🟡 仅预留 | 迁移 V2 给所有表加 `version`/`deleted` 同步列；`Edge.side` 支持跨族语义；但**无同步逻辑、无 Membership/角色表** |

> 注：上述“后端已实现”项的**前端页面（家族列表、地图、时序/提醒、关系图、备份、隐私、成员表单等）均已在 84007f6 被删除**，因此用户实际能看到的可交互界面当前不存在。

---

## 四、待实现明细

### P0（Now）— 修订版新增/缺口
| 规划项 | 状态 | 说明 |
|--------|------|------|
| **P0-0 账号与同步骨架**（可选注册、本地身份、同步适配器桩） | ❌ 未实现 | 无 auth、无本地身份、无同步端点；仅数据表有 `version/deleted` 预留列 |
| **P0-5 离线地图薄兜底** | ❌ 未实现 | 仅有“调起外部地图”深链；**无离线瓦片缓存、无飞行模式标记渲染、无降级方位视图**（修订版风险项 #5 的 ToS 也未裁决） |

### P1（Next）
| 规划项 | 状态 | 说明 |
|--------|------|------|
| **P1-1 云同步落地**（E2E 加密 + 版本历史 + 冲突） | ❌ 未实现 | 无任何同步服务端/客户端代码 |
| **P1-3 生平时间轴**（Event 实体 + 模糊日期） | ❌ 未实现 | `models.rs` 无 `Event` 实体，无 `events` 表，无端点（仅 `Member.biography` 自由文本） |
| **P1-5 隐私加密锁**（App 启动锁 + 截屏防护） | ❌ 未实现 | Tauri 壳无锁逻辑；`PrivacyPage` 已删除；PIN 恢复策略（修订版风险 #4）未裁决 |
| **P1-6 纪念册 PDF** | ❌ 未实现 | 无渲染/导出逻辑 |
| **P1-7 二维码标记**（脱敏只读） | ❌ 未实现 | 与离线 NFR 冲突待裁决（修订版风险 #3） |
| **P1-8 家族共创基础**（命名空间/邀请码/QR + RBAC + 审核） | ❌ 未实现 | 无 `Membership` 模型、无角色、无邀请端点 |
| **P1-9 姻亲跨族关联**（关联申请 + 双族谱视图） | ❌ 未实现 | `Edge.side` 字段已支持跨族语义，但无申请机制与双视图 UI |

### P2（Later）— 全部未实现
CRDT 多人实时协作、跨族地图聚合、AR 实景找墓、碑文 OCR、语音口述生平、云祭扫/离线纪念馆、寻亲/失联墓位。

---

## 五、修订版相对头脑风暴版的新增项落地情况
- **可选账号 / 本地为真相源 / 云端加密镜像**：仅理念写入文档，**代码零落地**（无账号、无同步）。
- **图模型一步到位**：✅ 数据层已落地（这是后端最大的一块已完成工作），与修订版“直接 Relation 图，非两阶段树”一致。
- **家族协作网络（RBAC/邀请/跨族）**：❌ 完全未开始。

---

## 六、建议下一步（按性价比排序）
1. **紧急**：从 `b280586` 恢复 `web/src/pages/*` 与 `web/src/components/*`，使项目回到可编译、可运行、可继续扩展的状态。
2. **补齐 P0 收尾**：在 Tauri 壳初始化 `tauri_plugin_notification` 并接 `list_reminders` 做本地推送（P0-2 完整闭环）；评估高德 ToS 决定是否做离线瓦片缓存（P0-5）。
3. **P0-0 轻量桩**：即使 P1 才接云端，也先定义本地身份结构与同步适配器 trait，避免后续返工。
4. **P1 精选**：检索补拼音首字母+维度筛选；P1-3 生平时间轴需先加 `Event` 实体与迁移。
