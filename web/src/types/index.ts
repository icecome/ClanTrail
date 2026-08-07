// 与 Rust 核心库模型保持一致的类型定义

export interface Family {
  id: string;
  name: string;
  description: string | null;
  origin: string | null;
  created_at: string;
  updated_at: string;
}

export interface TombGroup {
  id: string;
  family_id: string;
  name: string;
  description: string | null;
  created_at: string;
  updated_at: string;
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

export type { EntityType as EntityTypeName };
