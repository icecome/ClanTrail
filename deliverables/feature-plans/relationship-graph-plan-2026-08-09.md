# 关系图功能方案（以墓为主 + 可扩展族谱）

**日期**：2026-08-09
**类型**：功能设计方案
**决策基线**：以墓为主，同时为未来的家族/族谱扩展打下基础；人物关系仅用「配偶/儿子/女儿」三种基础边，其余辈分由系统自动推导。

---

## 0. 方案摘要（TL;DR）

- **核心思路**：`Member` 从「墓主」升级为「人物」——墓只是人的一个可选属性，允许录入没有墓的在世亲属。
- **关系模型**：只存三条基础边 `spouse(配偶) / son(儿子) / daughter(女儿)`，**孙子/孙女/外孙/同辈/姻亲等全部由系统自动推导**，不手动标注。
- **此方案同时满足**：现在以墓为主（墓碑登记不受影响）+ 未来扩展族谱（活人入谱、多代人物、自动算辈分）。
- **改动量**：一次 DB 迁移（V4 重建 members 表）+ 关系选择器改造 + 家族详情页「在世成员」区 + 关系图页面。**不破坏现有任何墓地功能。**

---

## 1. 为什么这样设计

### 1.1 兼顾「墓」与「人」两种模式

| 使用场景 | 现在（以墓为主） | 未来（族谱扩展） |
|---------|----------------|----------------|
| 核心对象 | 墓（有墓才有人） | 人（有墓只是人的一个属性） |
| 关系边 | 手动标记 | 基础边 + 自动推导 |
| 活人 | 无法表达 | 可入谱 |

**矛盾**：两种模式相反的。解决方案是让 `Member` 同时扮演「墓主」和「人」两个角色——**有墓时是墓主，无墓时是人**。这样现在不用重写，未来也不用重写。

### 1.2 为什么只用三种基础边

| 关系 | 是否存储 | 理由 |
|------|---------|------|
| 配偶（spouse） | ✅ 存储 | 基础边，双向对称 |
| 儿子（son） | ✅ 存储 | 基础边，父/母 → 子，带性别 |
| 女儿（daughter） | ✅ 存储 | 基础边，父/母 → 女，带性别 |
| 孙子/孙女/外孙 | ❌ 自动推导 | 儿子/女儿的儿子/女儿 |
| 兄弟/姐妹 | ❌ 自动推导 | 同一父母的子女 |
| 父母 | ❌ 自动推导 | 儿子/女儿的逆边 |

**优势**：
- 录入极简——用户只需标「他是谁的配偶/儿子/女儿」
- 数据无冗余——辈分、同辈、姻亲都是算出来的，不会出现「父子关系标了但孙子没标」的漏洞
- 天然带性别——儿子/女儿区分了性别，派生关系（孙子 vs 外孙）自动正确

---

## 2. 数据模型改动

### 2.1 V4 迁移：重建 members 表

SQLite 不支持直接 `DROP NOT NULL`，需重建表。`V4__person_model.sql`：

```sql
CREATE TABLE members_new (
    id              TEXT PRIMARY KEY,
    grave_id        TEXT REFERENCES graves(id) ON DELETE SET NULL,  -- 可空：在世者无墓
    clan_id         TEXT REFERENCES clans(id) ON DELETE CASCADE,     -- 新增：在世者归族
    name            TEXT NOT NULL,
    title           TEXT,
    birth_date      TEXT,
    death_date      TEXT,
    biography       TEXT,
    epitaph         TEXT,
    spouse          TEXT,        -- 保留：旧数据迁移
    is_joint_burial INTEGER NOT NULL DEFAULT 0,
    children        TEXT,        -- 保留：旧数据迁移
    is_alive        INTEGER NOT NULL DEFAULT 0,  -- 新增：1=在世 0=已故
    order_index     INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    version         INTEGER NOT NULL DEFAULT 0,
    deleted         INTEGER NOT NULL DEFAULT 0
);

-- 数据迁移：旧墓主默认 is_alive=0，clan_id 从 grave 反查
INSERT INTO members_new (id, grave_id, clan_id, name, title, birth_date, death_date,
    biography, epitaph, spouse, is_joint_burial, children, is_alive, order_index,
    created_at, updated_at, version, deleted)
SELECT m.id, m.grave_id, g.clan_id, m.name, m.title, m.birth_date, m.death_date,
    m.biography, m.epitaph, m.spouse, m.is_joint_burial, m.children, 0, m.order_index,
    m.created_at, m.updated_at, COALESCE(m.version, 0), COALESCE(m.deleted, 0)
FROM members m
LEFT JOIN graves g ON g.id = m.grave_id;

DROP TABLE members;
ALTER TABLE members_new RENAME TO members;

-- 重建索引
CREATE INDEX IF NOT EXISTS idx_members_tomb ON members(grave_id);
CREATE INDEX IF NOT EXISTS idx_members_clan ON members(clan_id);
CREATE INDEX IF NOT EXISTS idx_members_alive ON members(is_alive);
```

**关键点**：
- `grave_id` 从 `NOT NULL` → 可空，`ON DELETE CASCADE` → `SET NULL`（删除墓不影响关联在世成员）
- 新增 `clan_id`（在世者直接归族，不再依赖墓反查）
- 新增 `is_alive`（在世标记）
- 旧墓主数据自动迁移：`is_alive=0`，`clan_id` 从所属墓反查

### 2.2 Rust 模型层（`src-core/src/models.rs`）

```rust
pub struct Member {
    // ... 现有字段
    pub grave_id: Option<String>,  // 改：从 String 变 Option
    pub clan_id: Option<String>,    // 新增：在世者归族
    pub is_alive: bool,             // 新增：在世标记
    // ...
}

pub struct NewMember {
    // grave_id 改为 Optional
    pub grave_id: Option<String>,
    pub clan_id: Option<String>,
    pub is_alive: bool,
    // ...
}
```

### 2.3 关系类型改造（`EdgeType`）

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeType {
    Spouse,   // 配偶（双向对称）
    Son,      // 儿子（父/母 → 男）
    Daughter, // 女儿（父/母 → 女）
}
```

**移除**：`Parent`、`Child`、`JointBurial`。
- `Parent/Child` → 由 `Son/Daughter` 的逆边自动推导
- `JointBurial` → 由 `Member.is_joint_burial` 字段表达（同墓标记），不再作为关系边

**兼容旧数据**：已有 `parent/child/joint_burial` 边保留在库中，前端展示时映射为通用「关联」；新录入只允许三种基础边。可选做一次性迁移脚本，把旧边按性别映射为 son/daughter。

---

## 3. 关系自动推导规则

这是本方案的核心。系统只存三条基础边，**所有派生关系在渲染时实时计算**。

### 3.1 单向边存储

- `spouse`：A→B 存一条，B→A 自动互为配偶
- `son`：父/母 → 儿子，存一条有向边
- `daughter`：父/母 → 女儿，存一条有向边

### 3.2 推导算法（前端图渲染时执行）

```
从任意人物 X 出发：

1. 配偶（spouse）：找 X 的所有 spouse 边 → 配偶
2. 子女（children）：找所有指向 X 的 son/daughter 边 → 儿子/女儿
3. 父母（parents）：X 的所有 son/daughter 边指向的人 → 父/母
4. 孙辈（grandchildren）：X 的子女的子女 → 孙/孙女/外孙/外孙女
   - 儿子 → 儿子 = 孙子
   - 儿子 → 女儿 = 孙女
   - 女儿 → 儿子 = 外孙
   - 女儿 → 女儿 = 外孙女
5. 同辈（siblings）：与 X 有共同父母的其他人 → 兄/弟/姐/妹
6. 姻亲（in-laws）：配偶的父母/子女 → 岳/公/婆/媳/婿等（可选扩展）
```

### 3.3 约束与环检测

- **唯一配偶**：一个人最多一个 `spouse` 边（复婚/继室场景暂不处理，未来可扩展）
- **无自我循环**：`son/daughter` 边禁止指向自己、禁止形成父子环（A 是 B 的儿子，B 不能又是 A 的儿子）
- **方向唯一**：`son` 边方向固定为父/母 → 子，录入时校验目标性别

---

## 4. 后端 API 改造

| 接口 | 改动 |
|------|------|
| `POST /api/members` | 允许 `grave_id` 为空（创建在世成员）；新增 `clan_id`、`is_alive` |
| `PUT /api/members/:id` | 支持更新 `is_alive`、`clan_id` |
| `POST /api/members/:id/edges` | 只接受 `spouse/son/daughter`；增加性别校验与环检测 |
| `GET /api/clans/:id/living-members` | **新增**：查该族所有在世成员（`clan_id` 匹配且 `is_alive=1`） |
| `GET /api/clans/:id/graph` | **新增**：返回该族全部人物（在世+已故）+ 全部关系边，供关系图渲染 |
| `GET /api/members/:id/egograph` | **新增**：返回以该人为中心 2-3 跳的子图 |
| `GET /api/search?q=` | 扩展：返回在世成员（供关系选择器搜全库） |

---

## 5. 前端页面改造

### 5.1 家族详情页（`ClanDetailPage`）

- 在「墓地」列表下方新增「**在世成员**」区
- 显示在世成员数量 + 列表（姓名/称谓/生年）
- 「+ 添加在世成员」按钮 → 进入在世成员表单（无墓）
- 在世成员卡片样式与逝者区分（如浅色边框 + 「在世」徽标）

### 5.2 人物表单页（`MemberFormPage`）

- **新增模式**：`is_alive` 时显示「家族」选择器（代替墓选择器），`grave_id` 置空
- **关系选择器改造**：从「只选同墓人」改为「**全局搜索**」——输入姓名搜全库（含在世成员），也支持「+ 新建在世人物」即时创建
- 关系类型下拉：只保留 `spouse / son / daughter` 三选项
- 移除 `joint_burial` 关系边（改由 `is_joint_burial` 勾选表达）

### 5.3 人物详情页（`MemberDetailPage`）

- **新增「查看关系图」按钮** → 跳到关系图页面（以该人为中心）
- 「关联人物」列表改为展示推导后的关系（如「父亲：李二」「孙子：李三」），而非只列直接边

### 5.4 新增关系图页面（`GraphPage`）

- 路由：`/graph?clanId=xxx`（全族谱）或 `/graph?memberId=xxx`（人物 Ego 图）
- 渲染：**d3-force 力导向 SVG**（离线可用、体积小、能表达网状关系）
- 节点样式：在世=实心彩色，已故=半透明/灰；Spouse 边=红色实线同层并排，Son/Daughter 边=蓝色箭头纵向
- 点击节点 → 跳转人物详情页
- 可选：侧边面板显示选中人物的推导关系（配偶/子女/父母/孙辈/同辈）

---

## 6. 实施步骤

| 阶段 | 工作 | 工作量 |
|------|------|--------|
| **P0-1** | V4 迁移（重建 members 表）+ Rust 模型/DB 改造 | 1 天 |
| **P0-2** | 后端 API：living-members / graph / egograph / 搜索扩展 + 边约束 | 1 天 |
| **P0-3** | 前端：家族详情页「在世成员」区 + 人物表单支持无墓 + 关系选择器全局搜索 | 1-2 天 |
| **P0-4** | 关系图页面（d3-force 渲染 + 关系推导 + 节点跳转） | 2-3 天 |
| **P1** | 人物详情页「查看关系图」+ 关系列表显示推导关系 | 1 天 |
| **P2** | 旧 parent/child/joint_burial 边迁移脚本 + 跨族姻亲（side）渲染 | 视需要 |

---

## 7. 未来扩展（当真有族谱需求时）

本方案已为族谱扩展铺好路，届时只需一次**纯后端数据提升**：

```
现在（本方案后）          未来（族谱扩展）
─────────────────        ──────────────────
Member（人，可带墓）        Person（人，独立实体） ← 从 Member 迁移
                           GraveMember（墓位记录）← 只有逝者
Edge（人间关系）            Edge 改挂 Person ID
```

因为本方案已把 Member 当「人」用，未来只需把「人」拆成 `Person` 表、把「墓位信息」留在 `GraveMember`。**前端不用动**，数据零丢失，过渡顺滑。

---

## 8. 风险与待确认

1. **旧边迁移**：已有 `parent/child` 边无性别信息，无法自动映射为 son/daughter。方案：保留为通用「关联」边，或由用户手动重新标注。**需确认是否做一次性迁移脚本。**
2. **唯一配偶**：复婚/继室不在本方案范围（一个人只一个配偶）。未来扩展时再放宽。
3. **性别字段**：`son/daughter` 依赖目标性别，但当前 Member 无 `gender` 字段。方案：关系录入时由边类型隐含（选 son 即目标为男），不单独加 gender 字段。**若未来需要**，可加 `gender` 字段。
4. **图渲染库**：d3-force 手绘 SVG（推荐，离线可用）vs react-force-graph（开箱即用但体积大）。需确认。

---

> 本方案由关系图功能设计生成，重要决策（旧边迁移、性别字段、图库选型）请由主理人审定。