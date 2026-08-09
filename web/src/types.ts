// 与 Rust 端 src-core/src/models.rs 保持一致

export interface Clan {
  id: string;
  name: string;
  description: string | null;
  origin: string | null;
  created_at: string;
  updated_at: string;
  /** 同步版本号（随每次更新自增） */
  version?: number;
  /** 软删除标记 */
  deleted?: boolean;
}

export interface NewClan {
  name: string;
  description: string | null;
  origin: string | null;
}

export interface BurialGroup {
  id: string;
  clan_id: string;
  name: string;
  description: string | null;
  created_at: string;
  updated_at: string;
}

export interface NewBurialGroup {
  clan_id: string;
  name: string;
  description: string | null;
}

export interface Grave {
  id: string;
  name: string;
  latitude: number;
  longitude: number;
  address: string | null;
  description: string | null;
  /** 所属墓组（可选） */
  burial_group_id: string | null;
  /** 所属宗族（可选） */
  clan_id: string | null;
  created_at: string;
  updated_at: string;
  version?: number;
  deleted?: boolean;
}

export interface NewGrave {
  name: string;
  latitude: number;
  longitude: number;
  address: string | null;
  description: string | null;
  burial_group_id: string | null;
  clan_id: string | null;
}

export interface Member {
  id: string;
  /** 所属墓地（可空：在世者无墓） */
  grave_id: string | null;
  /** 所属宗族（在世者直接归族） */
  clan_id: string | null;
  name: string;
  title: string | null;
  birth_date: string | null;
  death_date: string | null;
  biography: string | null;
  epitaph: string | null;
  spouse: string | null;
  is_joint_burial: boolean;
  children: string | null;
  /** 在世标记（true=在世无墓） */
  is_alive: boolean;
  order_index: number;
  created_at: string;
  updated_at: string;
  version?: number;
  deleted?: boolean;
}

export interface NewMember {
  grave_id: string | null;
  clan_id: string | null;
  name: string;
  title: string | null;
  birth_date: string | null;
  death_date: string | null;
  biography: string | null;
  epitaph: string | null;
  spouse: string | null;
  is_joint_burial: boolean;
  children: string | null;
  is_alive: boolean;
  order_index: number;
}

export type EntityType = 'grave' | 'member' | 'clan';

export interface Image {
  id: string;
  entity_type: string;
  entity_id: string;
  file_path: string;
  caption: string | null;
  is_cover: boolean;
  created_at: string;
  version?: number;
  deleted?: boolean;
}

export interface NewImage {
  entity_type: EntityType;
  entity_id: string;
  file_path: string;
  caption: string | null;
  is_cover: boolean;
}

export type EdgeType = 'spouse' | 'son' | 'daughter';

export interface Edge {
  id: string;
  member_id: string;
  related_member_id: string;
  relation_type: string;
  created_at: string;
  /** 关系归属侧：born 原生家庭 / married 姻亲家庭 */
  side?: string | null;
  version?: number;
  deleted?: boolean;
  related_member_name: string | null;
  related_member_grave_id: string | null;
}

export interface NewEdge {
  member_id: string;
  related_member_id: string;
  relation_type: EdgeType;
  /** 关系归属侧：born / married（跨族关联用） */
  side?: string | null;
}

export type ReminderType = 'death_anniversary' | 'festival';

export interface Reminder {
  id: string;
  reminder_type: ReminderType;
  title: string;
  /** 公历日期（YYYY-MM-DD） */
  date: string;
  /** 农历日期描述（如 农历七月初三） */
  lunar_date: string;
  /** 关联人物 ID（忌日时） */
  member_id: string | null;
  /** 关联墓地 ID（忌日时） */
  grave_id: string | null;
  /** 距离今天的天数 */
  days_until: number;
}
