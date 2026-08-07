import { useEffect, useState } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import type { Person } from '../types';
import { api } from '../api/client';
import { useToast } from '../components/Toast';

function emptyPerson(tombId: string): Person {
  return {
    id: '',
    tomb_id: tombId,
    name: '',
    title: null,
    birth_date: null,
    death_date: null,
    biography: null,
    epitaph: null,
    spouse: null,
    is_joint_burial: false,
    children: null,
    order_index: 0,
    created_at: '',
    updated_at: '',
  };
}

export default function PersonFormPage() {
  const { id: tombId, pid } = useParams<{ id: string; pid: string }>();
  const navigate = useNavigate();
  const toast = useToast();
  const isNew = !pid || pid === 'new';
  const [form, setForm] = useState<Person | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    if (!tombId) return;
    if (isNew) {
      setForm(emptyPerson(tombId));
      setLoading(false);
      return;
    }
    (async () => {
      try {
        const persons = await api.listPersonsByTomb(tombId);
        const found = persons.find((p) => p.id === pid);
        setForm(found ?? emptyPerson(tombId));
      } catch (e) {
        toast.show('加载人物失败：' + (e instanceof Error ? e.message : String(e)), 'error');
        setForm(emptyPerson(tombId));
      } finally {
        setLoading(false);
      }
    })();
  }, [tombId, pid, isNew]);

  if (loading || !form) return <div className="page-container">加载中...</div>;

  async function submit() {
    if (!form) return;
    if (!form.name.trim()) {
      toast.show('请输入姓名', 'error');
      return;
    }
    try {
      if (isNew) {
        const { id, created_at, updated_at, ...input } = form;
        void id;
        void created_at;
        void updated_at;
        await api.createPerson({ ...input, tomb_id: tombId! });
      } else {
        const { id, created_at, updated_at, tomb_id, ...patch } = form;
        void id;
        void created_at;
        void updated_at;
        void tomb_id;
        await api.updatePerson(form.id, patch);
      }
      toast.show('已保存', 'success');
      navigate(`/tomb/${tombId}`);
    } catch (e) {
      toast.show('保存失败：' + (e instanceof Error ? e.message : String(e)), 'error');
    }
  }

  return (
    <div className="page-container app-screen">
      <div className="app-header with-back">
        <button className="back-btn" onClick={() => navigate(-1)} aria-label="返回">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.5} className="w-5 h-5">
            <path strokeLinecap="round" strokeLinejoin="round" d="M15 18l-6-6 6-6" />
          </svg>
        </button>
        <h1>{isNew ? '添加人物' : '编辑人物'}</h1>
        <button className="header-action" onClick={submit}>保存</button>
      </div>

      <div className="form-page">
        <div className="form-row form-row-grid">
          <label>
            <span>姓名 *</span>
            <input
              required
              value={form.name}
              onChange={(e) => setForm({ ...form, name: e.target.value })}
            />
          </label>
          <label>
            <span>称谓</span>
            <input
              placeholder="如：一世祖、祖父"
              value={form.title ?? ''}
              onChange={(e) => setForm({ ...form, title: e.target.value || null })}
            />
          </label>
        </div>

        <div className="form-row form-row-grid">
          <label>
            <span>生年</span>
            <input
              type="date"
              value={form.birth_date ?? ''}
              onChange={(e) => setForm({ ...form, birth_date: e.target.value || null })}
            />
          </label>
          <label>
            <span>殁年</span>
            <input
              type="date"
              value={form.death_date ?? ''}
              onChange={(e) => setForm({ ...form, death_date: e.target.value || null })}
            />
          </label>
        </div>

        <div className="form-row form-row-grid">
          <label>
            <span>配偶</span>
            <input
              value={form.spouse ?? ''}
              onChange={(e) => setForm({ ...form, spouse: e.target.value || null })}
            />
          </label>
          <label>
            <span>子女</span>
            <input
              placeholder="顿号分隔"
              value={form.children ?? ''}
              onChange={(e) => setForm({ ...form, children: e.target.value || null })}
            />
          </label>
        </div>

        <label className="form-row form-row-checkbox">
          <input
            type="checkbox"
            checked={form.is_joint_burial}
            onChange={(e) => setForm({ ...form, is_joint_burial: e.target.checked })}
          />
          <span>合墓（夫妻或家族同穴）</span>
        </label>

        <label className="form-row">
          <span>生平</span>
          <textarea
            rows={3}
            value={form.biography ?? ''}
            onChange={(e) => setForm({ ...form, biography: e.target.value || null })}
          />
        </label>

        <label className="form-row">
          <span>墓志铭</span>
          <textarea
            rows={2}
            value={form.epitaph ?? ''}
            onChange={(e) => setForm({ ...form, epitaph: e.target.value || null })}
          />
        </label>
      </div>
    </div>
  );
}
