import type {
  Family,
  NewFamily,
  NewRelation,
  Person,
  Photo,
  Relation,
  Reminder,
  Tomb,
  TombGroup,
} from '../types';

/**
 * 数据访问层 —— 通过 HTTP 调用 Rust (Axum) 后端。
 * Web 端走 vite proxy（/api → localhost:8080），
 * Tauri 移动端（Phase 3）会替换为 invoke 实现，接口签名保持不变。
 */

const BASE = import.meta.env.VITE_API_BASE ?? '/api';

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(`${BASE}${path}`, {
    headers: { 'Content-Type': 'application/json' },
    ...init,
  });
  if (!res.ok) {
    let detail = `HTTP ${res.status}`;
    try {
      const text = await res.text();
      if (text) {
        try {
          const body = JSON.parse(text);
          if (body && body.error) detail = body.error;
        } catch {
          // Axum 的 422 通常返回 text/plain，把原文截断展示更利于定位
          detail = text.length > 200 ? text.slice(0, 200) + '...' : text;
        }
      }
    } catch {
      /* 忽略读错误体失败 */
    }
    throw new Error(detail);
  }
  if (res.status === 204) return undefined as T;
  return res.json() as Promise<T>;
}

const json = (body: unknown): RequestInit => ({
  method: 'POST',
  body: JSON.stringify(body),
});

const putJson = (body: unknown): RequestInit => ({
  method: 'PUT',
  body: JSON.stringify(body),
});

export const api = {
  // ---- Family ----
  async listFamilies(): Promise<Family[]> {
    return request('/families');
  },
  async getFamily(id: string): Promise<Family> {
    return request(`/families/${id}`);
  },
  async createFamily(input: NewFamily): Promise<Family> {
    return request('/families', json(input));
  },
  async updateFamily(id: string, patch: Partial<NewFamily>): Promise<Family> {
    return request(`/families/${id}`, putJson(patch));
  },
  async deleteFamily(id: string): Promise<void> {
    return request(`/families/${id}`, { method: 'DELETE' });
  },

  // ---- TombGroup ----
  async listGroupsByFamily(familyId: string): Promise<TombGroup[]> {
    return request(`/families/${familyId}/groups`);
  },
  async createTombGroup(input: { family_id: string; name: string; description?: string | null }): Promise<TombGroup> {
    return request('/tomb-groups', json(input));
  },
  async deleteTombGroup(id: string): Promise<void> {
    return request(`/tomb-groups/${id}`, { method: 'DELETE' });
  },

  // ---- Tomb ----
  async listTombs(): Promise<Tomb[]> {
    return request('/tombs');
  },
  async listTombsByFamily(familyId: string): Promise<Tomb[]> {
    return request(`/families/${familyId}/tombs`);
  },
  async getTomb(id: string): Promise<Tomb | null> {
    try {
      return await request<Tomb>(`/tombs/${id}`);
    } catch (e) {
      if (e instanceof Error && e.message.includes('404')) return null;
      throw e;
    }
  },
  async createTomb(input: Omit<Tomb, 'id' | 'created_at' | 'updated_at'>): Promise<Tomb> {
    return request('/tombs', json(input));
  },
  async updateTomb(id: string, patch: Partial<Tomb>): Promise<Tomb> {
    return request(`/tombs/${id}`, putJson(patch));
  },
  async deleteTomb(id: string): Promise<void> {
    return request(`/tombs/${id}`, { method: 'DELETE' });
  },

  // ---- Person ----
  async listPersonsByTomb(tombId: string): Promise<Person[]> {
    return request(`/tombs/${tombId}/persons`);
  },
  async createPerson(input: Omit<Person, 'id' | 'created_at' | 'updated_at'>): Promise<Person> {
    return request('/persons', json(input));
  },
  async updatePerson(id: string, patch: Partial<Person>): Promise<Person> {
    return request(`/persons/${id}`, putJson(patch));
  },
  async deletePerson(id: string): Promise<void> {
    return request(`/persons/${id}`, { method: 'DELETE' });
  },

  // ---- Relation ----
  async listPersonRelations(personId: string): Promise<Relation[]> {
    return request(`/persons/${personId}/relations`);
  },
  async createRelation(personId: string, input: NewRelation): Promise<Relation[]> {
    return request(`/persons/${personId}/relations`, json(input));
  },
  async deleteRelation(id: string): Promise<void> {
    return request(`/relations/${id}`, { method: 'DELETE' });
  },

  // ---- Photo ----
  async listPhotosByTomb(tombId: string): Promise<Photo[]> {
    return request(`/tombs/${tombId}/photos`);
  },
  async createPhoto(input: { entity_type: string; entity_id: string; file_path: string; caption?: string | null; is_cover?: boolean }): Promise<Photo> {
    return request('/photos', json(input));
  },
  async uploadPhoto(formData: FormData): Promise<Photo> {
    const res = await fetch(`${BASE}/photos/upload`, { method: 'POST', body: formData });
    if (!res.ok) {
      let detail = `HTTP ${res.status}`;
      try {
        const text = await res.text();
        if (text) {
          try {
            const body = JSON.parse(text);
            if (body && body.error) detail = body.error;
          } catch {
            detail = text.length > 200 ? text.slice(0, 200) + '...' : text;
          }
        }
      } catch {
        /* 忽略读错误体失败 */
      }
      throw new Error(detail);
    }
    return res.json() as Promise<Photo>;
  },
  photoUrl(photo: Photo): string {
    return `${BASE}/photos/files/${photo.file_path}`;
  },
  async updatePhotoCover(id: string, is_cover: boolean): Promise<Photo> {
    return request(`/photos/${id}`, putJson({ is_cover }));
  },
  async deletePhoto(id: string): Promise<void> {
    return request(`/photos/${id}`, { method: 'DELETE' });
  },

  // ---- 数据导出/导入 ----
  async exportBackup(): Promise<Blob> {
    const res = await fetch(`${BASE}/export`, { method: 'GET' });
    if (!res.ok) throw new Error(`导出失败：HTTP ${res.status}`);
    return res.blob();
  },
  async importBackup(file: File): Promise<{ restored: boolean; schema_version: number; exported_at: string; backup_dir: string }> {
    const fd = new FormData();
    fd.append('file', file);
    const res = await fetch(`${BASE}/import`, { method: 'POST', body: fd });
    if (!res.ok) {
      let detail = `HTTP ${res.status}`;
      try {
        const body = await res.json();
        if (body && body.error) detail = body.error;
      } catch { /* ignore */ }
      throw new Error(detail);
    }
    return res.json();
  },

  // ---- 祭祀/忌日提醒 ----
  async listReminders(days: number = 365): Promise<Reminder[]> {
    return request(`/reminders?days=${days}`);
  },

  // ---- 搜索 ----
  async search(keyword: string): Promise<{ tombs: Tomb[]; persons: Tomb[]; families: Family[] }> {
    return request(`/search?keyword=${encodeURIComponent(keyword)}`);
  },

  // ---- 开发种子数据 ----
  async seedDemoData(): Promise<{ seeded: boolean }> {
    return request('/dev/seed', { method: 'POST' });
  },
};
