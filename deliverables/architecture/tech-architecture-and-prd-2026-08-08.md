# ClanTrail 技术架构方案 + 技术指南 + PRD + 演进路线

**日期**：2026-08-08
**视角**：APP 端为主，面向多用户云同步与家族共创
**输入**：`product-strategy-rev-account-2026-08-07.md`（产品战略）+ 代码库现状探查（2026-08-08）
**决策基线**：云端服务复用 Rust/Axum+src-core；移动端保留 Tauri v2；同步实体级+版本号 LWW；鉴权轻量自建+设备密钥 E2E；DB 补迁移框架+WAL+schema_version

---

## 0. 决策基线（本次拍板）

| 决策项 | 选定 | 理由摘要 |
|--------|------|---------|
| 云端服务技术栈 | 复用 Rust/Axum + src-core | 单语言、复用数据模型与导出逻辑，契合本地优先+E2E |
| 移动端栈 | 保留 Tauri v2 | 已出 APK、桌面移动共用代码、Rust 复用；按需补原生插件 |
| 同步模型 | 实体级 + 版本号 LWW | 每行带 version/updated_at，last-write-win + 版本历史回滚；L1 够用 |
| 鉴权加密 | 轻量自建 + 设备密钥 E2E | 一键生成密钥 + JWT，服务端零知识，契合隐私主权 |
| DB 演进 | 迁移框架 + WAL + schema_version | 修掉无迁移框架、无版本号的技术债 |

---

# Part A. 架构设计方案

## A1. 总体架构：三层 + 双进程

```
┌──────────────────────────── APP 端（Tauri v2，Android/桌面）────────────────────────────┐
│  React 前端 (Vite/TS)                                                                    │
│     │ HTTP(127.0.0.1:8080)                                                               │
│  ▼                                                                                       │
│  嵌入式 Axum 后端 (src-server::build_router)  ◄── 本地第一真相源                          │
│     │  rusqlite (WAL)                                                                    │
│  ▼                                                                                       │
│  本地 SQLite  (data_dir/clantrail.db)   ── 同步引擎 (新增 sync 模块) ──┐               │
│                                                                          │                │
└──────────────────────────────────────────────────────────────────────────┼──────────────┘
                                                                           │ HTTPS+JWT
                                                           E2E 密文家族数据 │ 同步元数据明文
                                                                           ▼
┌──────────────────────────── 云端服务（src-cloud，独立 Rust/Axum 二进制）──────────────────┐
│  Auth (JWT) · Sync API (push/pull) · Clan/Membership/Invite · ShareToken(只读)            │
│     │  复用 src-core 模型                                                                 │
│  ▼                                                                                       │
│  云端 DB (PostgreSQL 推荐 / SQLite 起步)                                                  │
│   - users / devices / clans / memberships / invite_tokens / share_tokens  (明文)         │
│   - sync_objects (clan_id, entity_type, entity_id, version, ciphertext, updated_at)      │
│   - sync_log (cursor 流)                                                                 │
└──────────────────────────────────────────────────────────────────────────────────────────┘
```

**核心原则**：
- **本地是第一真相源**：APP 内嵌 Axum + 本地 SQLite 永远可独立工作（L0）。
- **云端是加密镜像**：家族数据以 E2E 密文上云，服务端零知识；只有身份/同步元数据是明文。
- **复用 src-core**：云端服务与 APP 内嵌后端共享数据模型与序列化逻辑，单语言单模型。

## A2. 进程拓扑

| 进程 | 位置 | 职责 | 现状 |
|------|------|------|------|
| APP 前端 | 设备 | UI、交互 | 已有 |
| 嵌入式 Axum | 设备(127.0.0.1:8080) | 本地 CRUD、导出/导入、提醒 | 已有 |
| 本地 SQLite | 设备 | 第一真相源 | 已有 |
| 同步引擎 | 设备(新增 Rust 模块) | 推/拉、加解密、LWW 合并 | 新增 |
| 云端 Sync Service | 服务器(新增 src-cloud crate) | 鉴权、sync、clan、invite | 新增 |
| 云端 DB | 服务器 | 密文存储 + 元数据 | 新增 |

## A3. 数据模型演进

### 现有实体（保留）
Family / TombGroup / Tomb / Person / Photo / Relation(Spouse/Parent/Child/JointBurial) / Reminder。

### 需补字段
- `Relation` 增加 `side: TEXT`（`born` 原生家庭 / `married` 姻亲家庭）—— 跨族姻亲图的必要字段，修掉图模型缺口。
- 每张需同步的业务表（families/tombs/persons/photos/relations）增加 `version INTEGER NOT NULL DEFAULT 1`、`updated_at TEXT`、`deleted INTEGER NOT NULL DEFAULT 0`（软删，同步必需）。

### 新增实体（L1/L2）
| 实体 | 关键字段 | 说明 |
|------|---------|------|
| `User` | id, created_at, display_name?, status | L1 轻量身份，无密码 |
| `Device` | id, user_id, pubkey, created_at, last_seen | 设备密钥公钥，用于 E2E 包裹 |
| `Clan` | id, name, root_person_id?, owner_user_id, share_code, created_at | 家族命名空间 |
| `Membership` | clan_id, user_id, role, status, joined_at | RBAC：owner/admin/editor/viewer；status: pending/approved/revoked |
| `InviteToken` | token, clan_id, role, expires_at, max_uses, uses | 邀请码/链接/二维码统一来源 |
| `ShareToken` | token, clan_id, expires_at, scope=read_only | 游客只读令牌（无需账号） |
| `SyncCursor` | device_id, clan_id, last_cursor | 每设备每家族的拉取游标 |

> 本地侧：User/Device 仅存当前用户；Clan/Membership 缓存到本地供离线查看。云端侧：sync_objects 存密文 + version。

## A4. 同步引擎设计（实体级 LWW）

### 协议
- **Push**：客户端把本地 `version > last_pushed_version` 的行（含 deleted 软删）打包，**用 clan key 加密**，POST `/sync/push`。服务端存储 `sync_objects(clan_id, entity_type, entity_id, version, ciphertext, updated_at)`，写入 `sync_log` 并返回新 cursor。
- **Pull**：客户端 GET `/sync/pull?cursor=X`，服务端返回 `sync_log` 增量（ciphertext）。客户端**用 clan key 解密**，按 `(version, updated_at)` 做 LWW 合并写入本地。
- **冲突**：同行 `version` 相同但内容不同 → 取 `updated_at` 更新者；保留旧版本到 `sync_object_history`（N 份，可回滚）。
- **触发**：APP 前台时定时拉 + 写后即推；手动「立即同步」按钮；离线队列在恢复联网后回放。

### 边界
- L1（单人多端）：LWW + 版本历史足够，无真冲突。
- L2（多人共编）：仍是 LWW + 历史；当冲突率超阈值或进入 P2，再升级 CRDT（见 Part D）。

## A5. 鉴权与加密（轻量自建 + E2E）

### 身份
- **L0→L1 升级**：用户点「开启同步/创建家族」时，本地生成设备密钥对（X25519），派生 `user_id`，向云端注册 `User`+`Device(pubkey)`，换 JWT。**无邮箱/密码**（一键）。
- **可选绑定**：后续可绑邮箱/手机用于恢复（恢复码加密保管，非服务端可读）。
- **JWT**：短期 access + 长期 refresh；服务端只验签，不存明文密钥。

### E2E（零知识）
- **clan key**：每个家族一把对称密钥（AES-256-GCM），由 owner 创建家族时生成，存本地。
- **密钥分发**：clan key 用每位成员 `Device.pubkey` 包裹后存云端（wrapped_key）；成员设备用私钥解包得 clan key。服务端只见密文与包裹 key。
- **数据密文**：家族业务行（Person/Tomb/Photo 元数据/Relation/Reminder）用 clan key 加密后上云。照片二进制大文件单独加密分块存储（可走对象存储）。
- **注销**：删 Device + wrapped_key + 本设备 clan key 缓存；云端密文因无人可解而「事实销毁」。本机本地数据不动。

## A6. 家族共创与权限（RBAC + 只读令牌）

- **加入流程**：ID/邀请码/二维码三入口 → 发起申请 → 族长审核 → 通过后发 ShareToken(只读) 或升级 Membership(viewer/editor)。
- **角色**：`owner`（最高，可设多人防失联）/ `admin`（审核、设分支权限）/ `editor`（编辑本分支）/ `viewer`（只读）。
- **游客只读**：ShareToken 持有者可拉取家族密文视图，但无私钥解包 clan key → 实际只能看 owner 授权的「脱敏只读快照」（非密文流）。需在产品上明确：游客看的是 owner 主动发布的只读快照页，非实时密文。
- **修改审核**：editor 改动走「提案→admin 审核通过才落库」可选开关（默认开，防误改）。

## A7. 离线优先与联网降级

- L0 全离线可用，无任何服务端依赖。
- L1/L2 离线时：本地 CRUD 正常，写入同步队列；联网后回放。
- 查看他人共享家族：**必须联网**（拉密文）。离线仅显示本机已缓存快照。产品需显式标注「在线/离线」状态。

---

# Part B. 技术指南

## B1. 技术栈定版

| 层 | 技术 | 版本 | 备注 |
|----|------|------|------|
| 前端 | React + Vite + TS + Tailwind + RR6 + 高德 | 18/5/5.6/3/6 | 已有 |
| 桌面/移动壳 | Tauri v2 + geolocation + opener 插件 | 2 | 已有，按需补 local-notifications |
| 本地后端 | Rust + Axum 0.7 + rusqlite 0.31 | - | 已有 |
| 云端服务 | Rust + Axum + 复用 src-core | 0.7 | 新增 crate `src-cloud` |
| 云端 DB | PostgreSQL（生产）/ SQLite（起步） | - | 起步可 SQLite，扩容切 PG |
| 加密 | X25519 + AES-256-GCM（ring/argon2） | - | 设备密钥 + clan key |
| 鉴权 | JWT（jsonwebtoken crate） | - | 自签 |
| 迁移 | refinery | 0.8 | 替代手写 ALTER |

## B2. 目录结构演进

```
clantrail/
├── src-core/           # 共享模型 + DB（新增 sync 字段、User/Clan 模型）
├── src-server/         # 嵌入式后端 lib（现有，加 sync 适配器接口）
├── src-cloud/          # 【新】云端同步服务二进制（复用 src-core）
├── web/
│   ├── src/            # 前端（新增 account/sync/clan 页面）
│   └── src-tauri/      # Tauri 壳（新增 sync 后台模块、local-notifications 插件）
└── ...
```

## B3. DB 迁移框架落地（优先技术债）

1. 引入 `refinery`，建 `src-core/migrations/`。
2. `0001_initial.sql`：把现有 6 表 CREATE 迁入。
3. `0002_sync_columns.sql`：给 families/tombs/persons/photos/relations 加 `version`/`updated_at`/`deleted`。
4. `0003_relation_side.sql`：给 relations 加 `side`。
5. `0004_user_clan.sql`：建 users/devices/clans/memberships/invite_tokens/share_tokens/sync_cursors（云端表由 src-cloud 独立迁移）。
6. 建 `schema_version` 表（refinery 自管）。
7. 启用 WAL：`PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;` 在 `open()` 中固定。
8. **现有 `migrate_add_columns` 一次性 ALTER 逻辑废弃**，统一走 refinery。

## B4. 同步协议规约（核心端点）

```
POST /auth/register-device   { pubkey } -> { user_id, device_id, access_jwt, refresh_jwt }
POST /auth/refresh           { refresh_jwt } -> { access_jwt }

POST /clan                   { name, root_person_id? } -> { clan_id, share_code, invite_token }
POST /clan/join              { invite_token|share_code|qr } -> { membership_id, status=pending }
POST /clan/:id/approve       { membership_id } -> { membership_id, status=approved, wrapped_clan_key }

POST /sync/push              { clan_id, since_version, items:[{type,id,version,ciphertext,updated_at}] }
                             -> { cursor, applied:[{id,version}] }
GET  /sync/pull?clan_id=&cursor= -> { cursor, items:[{type,id,version,ciphertext,updated_at}] }

POST /share                  { clan_id, scope=read_only, expires_at } -> { share_token, qr_payload }
```

- 密文字段为 base64(AES-256-GCM(clan_key, payload_json))。
- `ciphertext` 内含原业务行 JSON（含 deleted 标志）。

## B5. 加密实现要点

- 设备密钥：X25519 私钥存本机安全区（Tauri 用 OS keychain / Android Keystore）；公钥上云。
- clan key：32 字节随机，AES-256-GCM。
- wrapped_clan_key = X25519(public=member_pubkey, private=owner_privkey) 派生 → AES 包裹 clan key。
- 大文件（照片）：分块 AES-GCM，密文存对象存储（如腾讯云 COS），URL + key 元数据上云。
- 服务端**永不持有** clan key 明文或设备私钥。

## B6. 移动端要点（Tauri v2）

- **后台同步**：Tauri 移动端后台受限；策略=前台定时拉 + 写后即推 + 手动同步；M3 再考虑前台服务（Android Foreground Service）原生插件。
- **通知**：现有提醒用本地通知；评估接入 `tauri-plugin-local-notifications` 保活。
- **网络**：`VITE_API_BASE` 注入本地 127.0.0.1:8080（内嵌）；云端 URL 由 sync 模块配置（环境变量，非前端直连）。
- **APK 体积**：debug 153MB 偏大，release 构建 + strip 可显著瘦身。

## B7. 安全与合规清单

- [ ] 所有云端 API 走 HTTPS + JWT 校验。
- [ ] 家族数据 E2E，服务端零知识（密文 + 包裹 key）。
- [ ] 调起外部地图只传坐标+墓地名，不传逝者信息（沿用约定）。
- [ ] 注销：删 Device/wrapped_key/本设备 clan key 缓存；不删本机本地数据。
- [ ] 邀请码/ShareToken 设有效期+次数；审核闭环防外人混入。
- [ ] 输入校验、请求体限制（现有 50MB）保留；同步接口加 rate limit。

## B8. 测试与 CI 策略

- **现状**：仅 7 个 db 单测，无 CI。
- **补**：
  - sync 引擎单测（LWW 合并、冲突回滚、加解密往返）。
  - src-cloud API 集成测试（auth/clan/sync 端到端）。
  - 前端关键流程组件测试。
- **CI**：建 `.github/workflows`，跑 `cargo test` + `npm run build` + lint；Android 构建走 release（按需触发）。

---

# Part C. PRD（L1+L2 核心闭环）

> L0（本地单机）已实现，本 PRD 聚焦「账号 + 云同步 + 家族共创」增量。

## C1. 目标与非目标

**目标**
- 让单人多设备数据不丢、可同步（L1）。
- 让同家族多人共编家谱与墓位，跨族姻亲可关联（L2）。
- 全程本地优先、E2E 加密、隐私克制。

**非目标**
- 不做强制账号；L0 永远可用。
- 不做 B 端陵园管理、公开社交广场。
- P0 不做多人实时共编（CRDT 留 P2）。

## C2. 用户故事（精选）

1. **US-1 开启同步**：作为单机用户，我想「一键开启同步」，换机不丢数据，且不用填邮箱密码。
2. **US-2 创建家族**：作为想邀请亲友的人，我想创建「张氏家族」并得到 ID/邀请码/二维码，分享给家人。
3. **US-3 加入家族**：作为家人，我想扫码或输 ID 加入，族长审核后我能查看；想编辑时一键升级身份。
4. **US-4 共编墓位**：作为 editor，我想给家族添人物、传照片、改关系，他人能看到我的改动。
5. **US-5 跨族关联**：作为李氏成员，我想通过母亲桥节点发起与陈氏家族的关联申请，对方同意后两图握手。
6. **US-6 注销无忧**：作为用户，我想注销云端身份后，本机数据原封不动，云端密文事实销毁。
7. **US-7 游客只读**：作为未登录访客，我想扫码查看某家族的只读快照，不能编辑。

## C3. 功能范围（分档）

### L1 账号与云同步
- F1.1 一键注册（设备密钥 + JWT）
- F1.2 多端 push/pull 同步（实体级 LWW + 版本历史）
- F1.3 同步状态指示（在线/离线/同步中/失败）
- F1.4 手动「立即同步」+ 冲突回滚入口
- F1.5 注销云端身份（不删本机）

### L2 家族共创
- F2.1 创建家族（ID + 邀请码 + 二维码三入口）
- F2.2 加入申请 + 族长审核
- F2.3 RBAC 角色（owner/admin/editor/viewer）
- F2.4 只读令牌（游客快照）
- F2.5 修改审核（提案→通过）
- F2.6 跨族姻亲关联（Relation.side + 关联申请）

## C4. 验收标准（关键）

- L0 全功能在飞行模式下 100% 可用（回归基线）。
- L1：A 设备改 → 联网 → B 设备拉到，延迟 < 30s；断网写入队列联网后回放无丢失。
- L1 注销：云端 sync_objects 无法被任何设备解密；本机数据完整。
- L2 加入：扫码 → 申请 → 族长通过 → 可查看；编辑需升级身份且留审计。
- 跨族关联：母亲节点在李氏/陈氏双视图均可见，墓位坐标单一来源。
- E2E：服务端 DB 落库为密文，无任何 clan key 明文。

## C5. 指标

| 类别 | 指标 | 目标 |
|------|------|------|
| 北极星 | 祭扫节点回流率 × 档案完整度 | 沿用战略 |
| 驱动 | 账号激活率 / 同步开启率 | 验证可选账号接受度 |
| 驱动 | 家族创建率 / 跨族关联数 | 共创冷启动 |
| 健康 | 纯本地用户占比 | >0，护城河报警 |
| 健康 | 同步失败率 / 冲突率 | <1% |
| 安全 | 隐私事件数 | 0 |

## C6. 风险

| 风险 | 等级 | 缓解 |
|------|------|------|
| E2E 密钥丢失=数据不可恢复 | 高 | 恢复码机制 + 多设备绑定 |
| LWW 多人冲突丢数据 | 中 | 版本历史回滚 + 冲突率监控，超阈值升级 CRDT |
| rusqlite 单连接争锁 | 中 | sync 走独立线程+channel；M3 评估 sqlx 池 |
| Tauri 后台同步受限 | 中 | 前台优先 + 手动同步；M3 原生前台服务 |
| 云端运维成本（个人开发者） | 中 | 起步 SQLite 单机，按量切 PG+对象存储 |

---

# Part D. 技术架构演进路线（多用户场景）

## D1. 阶段划分

| 里程碑 | 主题 | 能力 | 架构变化 |
|--------|------|------|---------|
| **M0** | 地基加固 | 迁移框架+WAL+schema_version+Relation.side+sync 列 | 纯本地，还技术债 |
| **M1** | 单人云同步 | L1 账号+多端同步+E2E | 新增 src-cloud + sync 引擎 |
| **M2** | 家族共创 | L2 clan/RBAC/invite/只读/跨族关联 | clan key 分发 + 审核流 |
| **M3** | 实时协作 | CRDT/OT + 后台同步 + 扩容 | sync 引擎升级 + 原生前台服务 + PG |

## D2. 每阶段架构变化

- **M0**：无新进程，只动 src-core（迁移+字段）。零功能风险。
- **M1**：引入 `src-cloud` 二进制 + 设备密钥 + sync_objects 密文存储。APP 侧加 sync 模块。云端 DB 用 SQLite 起步。
- **M2**：clan/membership/invite 表 + clan key 包裹分发 + 只读快照服务。前端加家族/审核 UI。
- **M3**：sync 引擎 LWW→CRDT（按实体类型选择性升级）；rusqlite→sqlx 连接池；云端 SQLite→PostgreSQL + 对象存储；Android 前台服务原生插件保活后台同步。

## D3. 关键演进点

1. **LWW → CRDT**：当 L2 多人共编冲突率 > 阈值（如 5%），对高冲突实体（Person/Relation）切换 CRDT，低冲突实体保留 LWW。混合模型，非一刀切。
2. **单连接 → 池**：M1 用 rusqlite 单连接 + 独立 sync 线程足够；M3 多人高频写入时迁 sqlx 连接池。
3. **加密升级**：M1 设备密钥+clan key；M3 可引入硬件密钥（Android StrongBox）可选。
4. **存储分层**：M1 元数据+密文同库；M2 照片密文移对象存储；M3 冷数据归档。

## D4. 容量与扩展预估（个人开发者量级）

| 量级 | 用户数 | 家族数 | 云端策略 |
|------|--------|--------|---------|
| 起步 | <1k | <200 | SQLite 单机 + 本地文件存密文 |
| 成长 | 1k–10k | 200–2k | PostgreSQL + 对象存储 |
| 规模 | >10k | >2k | PG 读写分离 + CDN + 监控告警 |

> 全程以「本地优先」降负载：大部分读在本地，云端只承担 sync + 共享，非热点读，扩容压力远小于传统 SaaS。

---

## 附：与产品战略文档的对应

| 本文档章节 | 对应战略文档 |
|-----------|-------------|
| A3 数据模型 | 战略 §4 图模型 |
| A5 鉴权加密 | 战略 §3.2/3.3 |
| A6 家族共创 | 战略 §3.4 加入机制 |
| C3 L1/L2 范围 | 战略 §5 P0-0/P1-1/P1-8/P1-9 |
| D1 M0–M3 | 战略 §5 Now/Next/Later |

> 本文档由技术架构与产品协同生成；E2E 密钥恢复、CRDT 切换阈值、云端运维成本三项需重点审定。
