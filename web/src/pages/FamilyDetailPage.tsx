import { useEffect, useState } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import type { Family, Tomb } from '../types';
import { api } from '../api/client';

export default function FamilyDetailPage() {
  const { id } = useParams<{ id: string }>();
  const [family, setFamily] = useState<Family | null>(null);
  const [tombs, setTombs] = useState<Tomb[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const navigate = useNavigate();

  useEffect(() => {
    if (!id) return;
    (async () => {
      try {
        const [f, ts] = await Promise.all([api.getFamily(id), api.listTombsByFamily(id)]);
        setFamily(f);
        setTombs(ts);
        setError(null);
      } catch (e) {
        setError(e instanceof Error ? e.message : '加载失败');
      } finally {
        setLoading(false);
      }
    })();
  }, [id]);

  if (loading) return <div className="page-container">加载中...</div>;

  return (
    <div className="page-container app-screen">
      <div className="app-header with-back">
        <button className="back-btn" onClick={() => navigate('/')} aria-label="返回">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.5} className="w-5 h-5">
            <path strokeLinecap="round" strokeLinejoin="round" d="M15 18l-6-6 6-6" />
          </svg>
        </button>
        <h1>{family?.name ?? '家族'}</h1>
        <div className="header-spacer" />
      </div>

      {error && <p className="empty-tip error-text">加载失败：{error}</p>}

      {family?.origin && (
        <div className="meta-line">
          <span>祖籍：{family.origin}</span>
        </div>
      )}
      {family?.description && <p className="tomb-desc">{family.description}</p>}

      <div className="section-title-row">
        <h2>墓地（{tombs.length}）</h2>
      </div>

      {tombs.length === 0 ? (
        <div className="empty-state">
          <p className="empty-sub">该家族还没有墓地记录</p>
        </div>
      ) : (
        <div className="list">
          {tombs.map((t) => (
            <div key={t.id} className="list-item" onClick={() => navigate(`/tomb/${t.id}`)}>
              <div className="list-item-main">
                <div className="list-item-title">{t.name}</div>
                <div className="list-item-sub">
                  {t.address ?? '暂无地址'} · {t.latitude.toFixed(4)}, {t.longitude.toFixed(4)}
                </div>
              </div>
              <svg className="chevron" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.5}>
                <path strokeLinecap="round" strokeLinejoin="round" d="M8.25 4.5l7.5 7.5-7.5 7.5" />
              </svg>
            </div>
          ))}
        </div>
      )}

      <button
        className="fab"
        onClick={() => navigate(`/tomb/new?family_id=${id}`)}
        aria-label="添加墓地"
      >
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2} className="w-6 h-6">
          <path strokeLinecap="round" d="M12 5v14M5 12h14" />
        </svg>
      </button>
    </div>
  );
}
