# 项目长期记忆（clantrail）

## 关系图功能（2026-08-09 实施）

### 模型变更
- **Member** 从「墓主」升级为「人物」：grave_id 可空（在世者无墓），新增 clan_id（归族），is_alive（在世标记）
- **EdgeType** 精简为三种：spouse(配偶) / son(儿子) / daughter(女儿)，移除 parent/child/joint_burial
- son/daughter 只存单方向（父/母→子/女），反向由查询推导（WHERE member_id OR related_member_id）
- spouse 仍存双向对称

### 新增 API
- GET /api/members/:id — 获取单个成员
- GET /api/clans/:id/members?alive=true|false — 按族+在世筛选
- GET /api/clans/:id/graph — 全族关系图数据（人物+边）
- GET /api/members/:id/egograph — 人物 Ego 图（BFS 2跳）

### 前端新页面
- **MemberListPage**：`/clans/:id/members`，支持全部/已故/在世筛选 + 快速关联（方案B）
- **MemberDirectPage**：`/member/:pid/view`，独立成员详情（无墓也可查看）
- **GraphPage**：`/clans/:id/graph` 或 `/graph/:memberId`，d3-force 力导向图
- **ClanDetailPage**：顶部四卡横向滑动（成员/墓地/在世成员/关系图谱）

### 入口顺序（卡片）
成员 → 墓地 → 在世成员 → 关系图谱

### 尚未实现
- MemberFormPage 尚未支持在世成员创建（路由 `/member/:pid/edit` 已注册但未实现）
- GraphPage 的交互优化（拖拽、缩放、节点详情面板）
- 旧 parent/child/joint_burial 边迁移脚本
