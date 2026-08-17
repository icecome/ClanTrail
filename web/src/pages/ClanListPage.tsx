import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import type { Family } from '../types';
import { api } from '../api/client';
import { useToast } from '../components/Toast';
import BottomSheet from '../components/BottomSheet';

export default function FamilyListPage() {
  const [families, setFamilies] = useState<Family[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [showAdd, setShowAdd] = useState(false);
  const [fName, setFName] = useState('');
  const [fOrigin, setFOrigin] = useState('');
  const [fDesc, setFDesc] = useState('');
  const navigate = useNavigate();
  const toast = useToast();

  useEffect(() => {
    (async () => {
      try {
        setFamilies(await api.listFamilies());
        setError(null);
      } catch (e) {
        setError(e instanceof Error ? e.message : '加载失败');
      } finally {
        setLoading(false);
      }
    })();
  }, []);

  async function submitAdd() {
    if (!fName.trim()) {
      toast.show('请输入家族名称', 'error');
      return;
    }
    try {
      const f = await api.createFamily({
        name: fName.trim(),
        origin: fOrigin.trim() || null,
        description: fDesc.trim() || null,
      });
      setShowAdd(false);
      setFName('');
      setFOrigin('');
      setFDesc('');
      navigate(`/families/${f.id}`);
    } catch (e) {
      toast.show('创建失败：' + (e instanceof Error ? e.message : String(e)), 'error');
    }
  }

  return (
    <div className="page-container app-screen">
      <div className="app-header">
        <h1>家族</h1>
        <button className="icon-btn" onClick={() => navigate('/settings')} aria-label="设置">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.5} className="w-5 h-5">
            <circle cx="12" cy="12" r="3" />
            <path strokeLinecap="round" strokeLinejoin="round" d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 11-2.83 2.83l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 11-4 0v-.09A1.65 1.65 0 009 19.4a1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 11-2.83-2.83l.06-.06a1.65 1.65 0 00.33-1.82 1.65 1.65 0 00-1.51-1H3a2 2 0 110-4h.09A1.65 1.65 0 004.6 9a1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 112.83-2.83l.06.06a1.65 1.65 0 001.82.33H9a1.65 1.65 0 001-1.51V3a2 2 0 114 0v.09a1.65 1.65 0 001 1.51 1.65 1.65 0 001.82-.33l.06-.06a2 2 0 112.83 2.83l-.06.06a1.65 1.65 0 00-.33 1.82V9a1.65 1.65 0 001.51 1H21a2 2 0 110 4h-.09a1.65 1.65 0 00-1.51 1z" />
          </svg>
        </button>
      </div>

      {loading && <p className="empty-tip">加载中...</p>}
      {error && <p className="empty-tip error-text">加载失败：{error}</p>}

      {!loading && !error && families.length === 0 && (
        <div className="empty-state">
          <div className="empty-icon">家</div>
          <p className="empty-title">还没有家族档案</p>
          <p className="empty-sub">点击右下角按钮创建第一个家族</p>
        </div>
      )}

      {families.length > 0 && (
        <div className="list">
          {families.map((f) => (
            <div key={f.id} className="list-item" onClick={() => navigate(`/families/${f.id}`)}>
              <div className="list-item-main">
                <div className="list-item-title">{f.name}</div>
                {f.origin && <div className="list-item-sub">祖籍：{f.origin}</div>}
              </div>
              <svg className="chevron" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.5}>
                <path strokeLinecap="round" strokeLinejoin="round" d="M8.25 4.5l7.5 7.5-7.5 7.5" />
              </svg>
            </div>
          ))}
        </div>
      )}

      <button className="fab" onClick={() => setShowAdd(true)} aria-label="添加家族">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2} className="w-6 h-6">
          <path strokeLinecap="round" d="M12 5v14M5 12h14" />
        </svg>
      </button>

      <BottomSheet open={showAdd} title="新建家族" onClose={() => setShowAdd(false)}>
        <div className="sheet-form">
          <label className="form-row">
            <span>名称 *</span>
            <input value={fName} onChange={(e) => setFName(e.target.value)} placeholder="如：张氏家族" />
          </label>
          <label className="form-row">
            <span>祖籍</span>
            <input value={fOrigin} onChange={(e) => setFOrigin(e.target.value)} placeholder="如：山西洪洞" />
          </label>
          <label className="form-row">
            <span>介绍</span>
            <textarea rows={2} value={fDesc} onChange={(e) => setFDesc(e.target.value)} placeholder="家族来历、堂号等" />
          </label>
          <div className="sheet-actions">
            <button className="sheet-btn" onClick={() => setShowAdd(false)}>取消</button>
            <button className="sheet-btn primary" onClick={submitAdd}>创建</button>
          </div>
        </div>
      </BottomSheet>
    </div>
  );
}
