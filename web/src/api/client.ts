import type {
  Clan,
  Edge,
  Grave,
  BurialGroup,
  Image,
  Member,
  NewBurialGroup,
  NewClan,
  NewEdge,
  Reminder,
} from '../types';
import { isTauri } from '@tauri-apps/api/core';

/**
 * 数据访问层 —— 通过 HTTP 调用 Rust (Axum) 后端。
 * Tauri 内嵌环境直连内嵌后端 127.0.0.1:8080；
 * 浏览器 dev 走 Vite 代理（/api → localhost:8080）。
 * 接口签名保持不变，供所有页面复用。
 */

const BASE = isTauri() ? 'http://127.0.0.1:8080/api' : (import.meta.env.VITE_API_BASE ?? '/api');

/** 携带 HTTP 状态码的请求错误，供调用方按状态码判断（如 404） */
class HttpError extends Error {
  status: number;
  constructor(status: number, message: string) {
    super(message);
    this.status = status;
    this.name = 'HttpError';
  }
}

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
    throw new HttpError(res.status, detail);
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

/** 将 File 读取为纯 base64 字符串（去掉 data:…;base64, 前缀） */
function fileToBase64(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const dataUrl = reader.result as string;
      const comma = dataUrl.indexOf(',');
      resolve(comma >= 0 ? dataUrl.slice(comma + 1) : dataUrl);
    };
    reader.onerror = () => reject(reader.error ?? new Error('读取文件失败'));
    reader.readAsDataURL(file);
  });
}

/** 统一解析非 2xx 响应并抛出 HttpError */
async function throwHttpError(res: Response): Promise<never> {
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
  throw new HttpError(res.status, detail);
}

export const api = {
  // ---- Clan ----
  async listClans(): Promise<Clan[]> {
    return request('/clans');
  },
  async getClan(id: string): Promise<Clan> {
    return request(`/clans/${id}`);
  },
  async createClan(input: NewClan): Promise<Clan> {
    return request('/clans', json(input));
  },
  async updateClan(id: string, patch: Partial<NewClan>): Promise<Clan> {
    return request(`/clans/${id}`, putJson(patch));
  },
  async deleteClan(id: string): Promise<void> {
    return request(`/clans/${id}`, { method: 'DELETE' });
  },

  // ---- BurialGroup ----
  async listGroupsByClan(clanId: string): Promise<BurialGroup[]> {
    return request(`/clans/${clanId}/groups`);
  },
  async createBurialGroup(input: NewBurialGroup): Promise<BurialGroup> {
    return request('/burial-groups', json(input));
  },
  async deleteBurialGroup(id: string): Promise<void> {
    return request(`/burial-groups/${id}`, { method: 'DELETE' });
  },

  // ---- Grave ----
  async listGraves(): Promise<Grave[]> {
    return request('/graves');
  },
  async listGravesByClan(clanId: string): Promise<Grave[]> {
    return request(`/clans/${clanId}/graves`);
  },
  async getGrave(id: string): Promise<Grave | null> {
    try {
      return await request<Grave>(`/graves/${id}`);
    } catch (e) {
      if (e instanceof HttpError && e.status === 404) return null;
      throw e;
    }
  },
  async createGrave(input: Omit<Grave, 'id' | 'created_at' | 'updated_at'>): Promise<Grave> {
    return request('/graves', json(input));
  },
  async updateGrave(id: string, patch: Partial<Grave>): Promise<Grave> {
    return request(`/graves/${id}`, putJson(patch));
  },
  async deleteGrave(id: string): Promise<void> {
    return request(`/graves/${id}`, { method: 'DELETE' });
  },

  // ---- Member ----
  async listMembersByGrave(graveId: string): Promise<Member[]> {
    return request(`/graves/${graveId}/members`);
  },
  async getMember(id: string): Promise<Member> {
    return request(`/members/${id}`);
  },
  async createMember(input: Omit<Member, 'id' | 'created_at' | 'updated_at'>): Promise<Member> {
    return request('/members', json(input));
  },
  async updateMember(id: string, patch: Partial<Member>): Promise<Member> {
    return request(`/members/${id}`, putJson(patch));
  },
  async deleteMember(id: string): Promise<void> {
    return request(`/members/${id}`, { method: 'DELETE' });
  },

  // ---- Edge ----
  async listMemberEdges(memberId: string): Promise<Edge[]> {
    return request(`/members/${memberId}/edges`);
  },
  async createEdge(memberId: string, input: NewEdge): Promise<Edge[]> {
    return request(`/members/${memberId}/edges`, json(input));
  },
  async deleteEdge(id: string): Promise<void> {
    return request(`/edges/${id}`, { method: 'DELETE' });
  },

  // ---- 关系图 ----
  async listMembersByClan(clanId: string, alive?: boolean): Promise<Member[]> {
    const qs = alive !== undefined ? `?alive=${alive}` : '';
    return request(`/clans/${clanId}/members${qs}`);
  },
  async clanGraph(clanId: string): Promise<{ members: Member[]; edges: Edge[] }> {
    return request(`/clans/${clanId}/graph`);
  },
  async memberEgograph(memberId: string): Promise<{ members: Member[]; edges: Edge[] }> {
    return request(`/members/${memberId}/egograph`);
  },

  // ---- Image ----
  async listImagesByGrave(graveId: string): Promise<Image[]> {
    return request(`/graves/${graveId}/images`);
  },
  async listMemberImages(memberId: string): Promise<Image[]> {
    return request(`/members/${memberId}/images`);
  },
  async createImage(input: { entity_type: string; entity_id: string; file_path: string; caption?: string | null; is_cover?: boolean }): Promise<Image> {
    return request('/images', json(input));
  },
  async uploadImage(file: File, entityType: string, entityId: string, caption?: string, is_cover?: boolean): Promise<Image> {
    if (isTauri()) {
      // Tauri 环境：base64 JSON 上传（避开 WebView fetch multipart 缺陷）
      const fileData = await fileToBase64(file);
      const res = await fetch(`${BASE}/images/upload64`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          entity_type: entityType,
          entity_id: entityId,
          caption: caption ?? null,
          file_name: file.name,
          file_data: fileData,
          is_cover: is_cover ?? false,
        }),
      });
      if (!res.ok) return throwHttpError(res);
      return res.json() as Promise<Image>;
    }
    // 非 Tauri：multipart/form-data
    const fd = new FormData();
    fd.append('file', file);
    fd.append('entity_type', entityType);
    fd.append('entity_id', entityId);
    if (caption) fd.append('caption', caption);
    const res = await fetch(`${BASE}/images/upload`, { method: 'POST', body: fd });
    if (!res.ok) return throwHttpError(res);
    return res.json() as Promise<Image>;
  },
  imageUrl(image: Image): string {
    return `${BASE}/images/files/${image.file_path}`;
  },
  async updateImageCover(id: string, is_cover: boolean): Promise<Image> {
    return request(`/images/${id}`, putJson({ is_cover }));
  },
  async deleteImage(id: string): Promise<void> {
    return request(`/images/${id}`, { method: 'DELETE' });
  },

  // ---- 数据导出/导入与备份管理 ----
  async exportBackup(): Promise<{ path: string; filename: string; size_bytes: number }> {
    const res = await fetch(`${BASE}/export`, { method: 'POST' });
    if (!res.ok) {
      let detail = `HTTP ${res.status}`;
      try {
        const body = await res.json();
        if (body && body.error) detail = body.error;
      } catch { /* ignore */ }
      throw new HttpError(res.status, detail);
    }
    return res.json();
  },
  async listBackups(): Promise<{ filename: string; size_bytes: number; modified_at: string }[]> {
    return request('/backups');
  },
  backupDownloadUrl(filename: string): string {
    return `${BASE}/backups/${encodeURIComponent(filename)}`;
  },
  async deleteBackup(filename: string): Promise<void> {
    return request(`/backups/${encodeURIComponent(filename)}`, { method: 'DELETE' });
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
      throw new HttpError(res.status, detail);
    }
    return res.json();
  },

  // ---- 祭祀/忌日提醒 ----
  async listReminders(days: number = 365): Promise<Reminder[]> {
    return request(`/reminders?days=${days}`);
  },

  // ---- 搜索 ----
  async search(keyword: string): Promise<{ graves: Grave[]; members: Grave[]; clans: Clan[] }> {
    return request(`/search?keyword=${encodeURIComponent(keyword)}`);
  },

  // ---- 开发种子数据 ----
  async seedDemoData(): Promise<{ seeded: boolean }> {
    return request('/dev/seed', { method: 'POST' });
  },
};
