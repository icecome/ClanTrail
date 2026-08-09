use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// 宗族
// ---------------------------------------------------------------------------
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Clan {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    /// 祖籍 / 起源地
    pub origin: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    /// 同步版本号（随每次更新自增）
    #[serde(default)]
    pub version: u32,
    /// 软删除标记（同步预留）
    #[serde(default)]
    pub deleted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewClan {
    pub name: String,
    pub description: Option<String>,
    pub origin: Option<String>,
}

// ---------------------------------------------------------------------------
// 墓组 —— 一个宗族下的墓地区划（如"祖坟区""新墓区"）
// ---------------------------------------------------------------------------
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BurialGroup {
    pub id: String,
    pub clan_id: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewBurialGroup {
    pub clan_id: String,
    pub name: String,
    pub description: Option<String>,
}

// ---------------------------------------------------------------------------
// 单墓地
// ---------------------------------------------------------------------------
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Grave {
    pub id: String,
    /// 墓地名称 / 编号（如"XX公墓 A区-12排-3号"）
    pub name: String,
    pub latitude: f64,
    pub longitude: f64,
    pub address: Option<String>,
    pub description: Option<String>,
    /// 所属墓组（可选）
    pub burial_group_id: Option<String>,
    /// 所属宗族（可选）
    pub clan_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub deleted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewGrave {
    pub name: String,
    pub latitude: f64,
    pub longitude: f64,
    pub address: Option<String>,
    pub description: Option<String>,
    pub burial_group_id: Option<String>,
    pub clan_id: Option<String>,
}

// ---------------------------------------------------------------------------
// 人物 —— 有墓时是墓主（安葬人物），无墓时是在世亲属
// ---------------------------------------------------------------------------
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Member {
    pub id: String,
    /// 所属墓地（可空：在世者无墓）
    pub grave_id: Option<String>,
    /// 所属宗族（在世者直接归族；已故者从墓地归族）
    pub clan_id: Option<String>,
    pub name: String,
    /// 称谓（如"先祖""祖父""慈母"）
    pub title: Option<String>,
    pub birth_date: Option<String>,
    pub death_date: Option<String>,
    /// 生平介绍
    pub biography: Option<String>,
    /// 墓志铭
    pub epitaph: Option<String>,
    /// 配偶姓名（旧文本字段，关系图用 Edge）
    pub spouse: Option<String>,
    /// 是否合墓（夫妻或家族同穴）
    pub is_joint_burial: bool,
    /// 子女（旧文本字段，关系图用 Edge）
    pub children: Option<String>,
    /// 在世标记（true=在世无墓，false=已安葬）
    #[serde(default)]
    pub is_alive: bool,
    /// 排序（合葬墓中区分先后）
    pub order_index: i32,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub deleted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewMember {
    pub grave_id: Option<String>,
    pub clan_id: Option<String>,
    pub name: String,
    pub title: Option<String>,
    pub birth_date: Option<String>,
    pub death_date: Option<String>,
    pub biography: Option<String>,
    pub epitaph: Option<String>,
    pub spouse: Option<String>,
    pub is_joint_burial: bool,
    pub children: Option<String>,
    #[serde(default)]
    pub is_alive: bool,
    pub order_index: i32,
}

// ---------------------------------------------------------------------------
// 图片 —— 多态关联到 Grave / Member / Clan
// ---------------------------------------------------------------------------
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EntityType {
    Grave,
    Member,
    Clan,
}

impl EntityType {
    pub fn as_str(&self) -> &'static str {
        match self {
            EntityType::Grave => "grave",
            EntityType::Member => "member",
            EntityType::Clan => "clan",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "grave" => Some(EntityType::Grave),
            "member" => Some(EntityType::Member),
            "clan" => Some(EntityType::Clan),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Image {
    pub id: String,
    pub entity_type: String,
    pub entity_id: String,
    pub file_path: String,
    pub caption: Option<String>,
    pub is_cover: bool,
    pub created_at: String,
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub deleted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewImage {
    pub entity_type: EntityType,
    pub entity_id: String,
    pub file_path: String,
    pub caption: Option<String>,
    pub is_cover: bool,
}

// ---------------------------------------------------------------------------
// 人物关系（结构化关联，替代纯文本 spouse/children）
// 只存三种基础边：spouse(配偶)/son(儿子)/daughter(女儿)，其余辈分自动推导。
// ---------------------------------------------------------------------------
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeType {
    Spouse,
    Son,
    Daughter,
}

impl EdgeType {
    pub fn as_str(&self) -> &'static str {
        match self {
            EdgeType::Spouse => "spouse",
            EdgeType::Son => "son",
            EdgeType::Daughter => "daughter",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "spouse" => Some(EdgeType::Spouse),
            "son" => Some(EdgeType::Son),
            "daughter" => Some(EdgeType::Daughter),
            _ => None,
        }
    }

    /// 反向关系类型（spouse <-> spouse, son/daughter 的逆为 parent，存为 son/daughter）
    /// spouse 双向对称；son/daughter 的反向边在前端推导时使用，存储时仍存 son/daughter
    pub fn reverse(&self) -> EdgeType {
        match self {
            EdgeType::Spouse => EdgeType::Spouse,
            EdgeType::Son => EdgeType::Son,
            EdgeType::Daughter => EdgeType::Daughter,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub id: String,
    pub member_id: String,
    pub related_member_id: String,
    pub relation_type: String,
    pub created_at: String,
    /// 关系归属侧：born 原生家庭 / married 姻亲家庭（跨族关联用）
    #[serde(default)]
    pub side: Option<String>,
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub deleted: bool,
    /// 关联人物的姓名（JOIN 查询填充，前端展示用）
    pub related_member_name: Option<String>,
    /// 关联人物所在墓的 ID
    pub related_member_grave_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewEdge {
    pub member_id: String,
    pub related_member_id: String,
    pub relation_type: EdgeType,
    /// 关系归属侧：born / married（跨族关联用）
    #[serde(default)]
    pub side: Option<String>,
}

// ---------------------------------------------------------------------------
// 辅助：生成时间戳
// ---------------------------------------------------------------------------
pub fn now_iso() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

pub fn new_uuid() -> String {
    Uuid::new_v4().to_string()
}

// ---------------------------------------------------------------------------
// 祭祀/忌日提醒
// ---------------------------------------------------------------------------
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReminderType {
    /// 人物忌日（按农历）
    DeathAnniversary,
    /// 传统节日
    Festival,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reminder {
    pub id: String,
    pub reminder_type: ReminderType,
    pub title: String,
    /// 公历日期（YYYY-MM-DD）
    pub date: String,
    /// 农历日期描述（如 农历七月初三）
    pub lunar_date: String,
    /// 关联人物 ID（忌日时）
    pub member_id: Option<String>,
    /// 关联墓地 ID（忌日时）
    pub grave_id: Option<String>,
    /// 距离今天的天数
    pub days_until: i64,
}
