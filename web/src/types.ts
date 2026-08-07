// 与 Rust 端 src-core/src/models.rs 保持一致

export interface Family {
  id: string;
  name: string;
  description: string | null;
  origin: string | null;
  created_at: string;
  updated_at: string;
}

export interface NewFamily {
  name: string;
  description: string | null;
  origin: string | null;
}

export interface TombGroup {
  id: string;
  family_id: string;
  name: string;
  description: string | null;
  created_at: string;
  updated_at: string;
}

export interface NewTombGroup {
  family_id: string;
  name: string;
  description: string | null;
}

export interface Tomb {
  id: string;
  name: string;
  latitude: number;
  longitude: number;
  address: string | null;
  description: string | null;
  group_id: string | null;
  family_id: string | null;
  created_at: string;
  updated_at: string;
}

export interface NewTomb {
  name: string;
  latitude: number;
  longitude: number;
  address: string | null;
  description: string | null;
  group_id: string | null;
  family_id: string | null;
}

export interface Person {
  id: string;
  tomb_id: string;
  name: string;
  title: string | null;
  birth_date: string | null;
  death_date: string | null;
  biography: string | null;
  epitaph: string | null;
  spouse: string | null;
  is_joint_burial: boolean;
  children: string | null;
  order_index: number;
  created_at: string;
  updated_at: string;
}

export interface NewPerson {
  tomb_id: string;
  name: string;
  title: string | null;
  birth_date: string | null;
  death_date: string | null;
  biography: string | null;
  epitaph: string | null;
  spouse: string | null;
  is_joint_burial: boolean;
  children: string | null;
  order_index: number;
}

export type EntityType = 'tomb' | 'person' | 'family';

export interface Photo {
  id: string;
  entity_type: string;
  entity_id: string;
  file_path: string;
  caption: string | null;
  is_cover: boolean;
  created_at: string;
}

export interface NewPhoto {
  entity_type: EntityType;
  entity_id: string;
  file_path: string;
  caption: string | null;
  is_cover: boolean;
}

export type RelationType = 'spouse' | 'parent' | 'child' | 'joint_burial';

export interface Relation {
  id: string;
  person_id: string;
  related_person_id: string;
  relation_type: string;
  created_at: string;
  related_person_name: string | null;
  related_person_tomb_id: string | null;
}

export interface NewRelation {
  person_id: string;
  related_person_id: string;
  relation_type: RelationType;
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
  person_id: string | null;
  /** 关联墓地 ID（忌日时） */
  tomb_id: string | null;
  /** 距离今天的天数 */
  days_until: number;
}
