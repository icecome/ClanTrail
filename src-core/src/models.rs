use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// 家族
// ---------------------------------------------------------------------------
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Family {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    /// 祖籍 / 起源地
    pub origin: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewFamily {
    pub name: String,
    pub description: Option<String>,
    pub origin: Option<String>,
}

// ---------------------------------------------------------------------------
// 墓组 —— 一个家族下的墓地区划（如"祖坟区""新墓区"）
// ---------------------------------------------------------------------------
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TombGroup {
    pub id: String,
    pub family_id: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewTombGroup {
    pub family_id: String,
    pub name: String,
    pub description: Option<String>,
}

// ---------------------------------------------------------------------------
// 单墓地
// ---------------------------------------------------------------------------
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tomb {
    pub id: String,
    /// 墓地名称 / 编号（如"XX公墓 A区-12排-3号"）
    pub name: String,
    pub latitude: f64,
    pub longitude: f64,
    pub address: Option<String>,
    pub description: Option<String>,
    /// 所属墓组（可选）
    pub group_id: Option<String>,
    /// 所属家族（可选）
    pub family_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewTomb {
    pub name: String,
    pub latitude: f64,
    pub longitude: f64,
    pub address: Option<String>,
    pub description: Option<String>,
    pub group_id: Option<String>,
    pub family_id: Option<String>,
}

// ---------------------------------------------------------------------------
// 墓主 / 人物 —— 一个墓地可能合葬多人
// ---------------------------------------------------------------------------
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Person {
    pub id: String,
    pub tomb_id: String,
    pub name: String,
    /// 称谓（如"先祖""祖父""慈母"）
    pub title: Option<String>,
    pub birth_date: Option<String>,
    pub death_date: Option<String>,
    /// 生平介绍
    pub biography: Option<String>,
    /// 墓志铭
    pub epitaph: Option<String>,
    /// 配偶姓名
    pub spouse: Option<String>,
    /// 是否合墓（夫妻或家族同穴）
    pub is_joint_burial: bool,
    /// 子女（多个用顿号/逗号分隔，简化处理）
    pub children: Option<String>,
    /// 排序（合葬墓中区分先后）
    pub order_index: i32,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewPerson {
    pub tomb_id: String,
    pub name: String,
    pub title: Option<String>,
    pub birth_date: Option<String>,
    pub death_date: Option<String>,
    pub biography: Option<String>,
    pub epitaph: Option<String>,
    pub spouse: Option<String>,
    pub is_joint_burial: bool,
    pub children: Option<String>,
    pub order_index: i32,
}

// ---------------------------------------------------------------------------
// 照片 —— 多态关联到 Tomb / Person / Family
// ---------------------------------------------------------------------------
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EntityType {
    Tomb,
    Person,
    Family,
}

impl EntityType {
    pub fn as_str(&self) -> &'static str {
        match self {
            EntityType::Tomb => "tomb",
            EntityType::Person => "person",
            EntityType::Family => "family",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "tomb" => Some(EntityType::Tomb),
            "person" => Some(EntityType::Person),
            "family" => Some(EntityType::Family),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Photo {
    pub id: String,
    pub entity_type: String,
    pub entity_id: String,
    pub file_path: String,
    pub caption: Option<String>,
    pub is_cover: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewPhoto {
    pub entity_type: EntityType,
    pub entity_id: String,
    pub file_path: String,
    pub caption: Option<String>,
    pub is_cover: bool,
}

// ---------------------------------------------------------------------------
// 人物关系（结构化关联，替代纯文本 spouse/children）
// ---------------------------------------------------------------------------
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationType {
    Spouse,
    Parent,
    Child,
    JointBurial,
}

impl RelationType {
    pub fn as_str(&self) -> &'static str {
        match self {
            RelationType::Spouse => "spouse",
            RelationType::Parent => "parent",
            RelationType::Child => "child",
            RelationType::JointBurial => "joint_burial",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "spouse" => Some(RelationType::Spouse),
            "parent" => Some(RelationType::Parent),
            "child" => Some(RelationType::Child),
            "joint_burial" => Some(RelationType::JointBurial),
            _ => None,
        }
    }

    /// 反向关系类型（parent ↔ child, spouse ↔ spouse, joint_burial ↔ joint_burial）
    pub fn reverse(&self) -> RelationType {
        match self {
            RelationType::Spouse => RelationType::Spouse,
            RelationType::Parent => RelationType::Child,
            RelationType::Child => RelationType::Parent,
            RelationType::JointBurial => RelationType::JointBurial,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relation {
    pub id: String,
    pub person_id: String,
    pub related_person_id: String,
    pub relation_type: String,
    pub created_at: String,
    /// 关联人物的姓名（JOIN 查询填充，前端展示用）
    pub related_person_name: Option<String>,
    /// 关联人物所在墓的 ID
    pub related_person_tomb_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewRelation {
    pub person_id: String,
    pub related_person_id: String,
    pub relation_type: RelationType,
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
    pub person_id: Option<String>,
    /// 关联墓地 ID（忌日时）
    pub tomb_id: Option<String>,
    /// 距离今天的天数
    pub days_until: i64,
}