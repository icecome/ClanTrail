import { useEffect, useRef, useState } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import type { Person, Photo, Relation, Tomb } from '../types';
import { api } from '../api/client';
import { openExternalMap } from '../utils/navigation';
import { useToast } from '../components/Toast';
import ConfirmSheet from '../components/ConfirmSheet';
import BottomSheet from '../components/BottomSheet';

export default function TombDetailPage() {
  const { id } = useParams();
  const [tomb, setTomb] = useState<Tomb | null>(null);
  const [persons, setPersons] = useState<Person[]>([]);
  const [photos, setPhotos] = useState<Photo[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [relations, setRelations] = useState<Record<string, Relation[]>>({});
  const [relatingPerson, setRelatingPerson] = useState<Person | null>(null);
  const [confirmDeletePerson, setConfirmDeletePerson] = useState<string | null>(null);
  const [confirmDeletePhoto, setConfirmDeletePhoto] = useState<string | null>(null);
  const navigate = useNavigate();
  const toast = useToast();

  useEffect(() => {
    if (!id) return;
    (async () => {
      try {
        const [t, ps, phs] = await Promise.all([
          api.getTomb(id),
          api.listPersonsByTomb(id),
          api.listPhotosByTomb(id),
        ]);
        if (!t) {
          setError('墓地不存在');
        } else {
          setTomb(t);
          setPersons(ps);
          setPhotos(phs);
        }
      } catch (e) {
        setError(e instanceof Error ? e.message : '加载失败');
      } finally {
        setLoading(false);
      }
    })();
  }, [id]);

  useEffect(() => {
    if (persons.length === 0) return;
    (async () => {
      const rels: Record<string, Relation[]> = {};
      for (const p of persons) {
        try {
          rels[p.id] = await api.listPersonRelations(p.id);
        } catch {
          /* 无关系时正常返回空数组 */
        }
      }
      setRelations(rels);
    })();
  }, [persons]);

  async function handleFiles(files: FileList | null) {
    if (!files || files.length === 0 || !id) return;
    const added: Photo[] = [];
    for (const f of Array.from(files)) {
      try {
        const fd = new FormData();
        fd.append('file', f);
        fd.append('entity_type', 'tomb');
        fd.append('entity_id', id);
        fd.append('caption', f.name);
        const rec = await api.uploadPhoto(fd);
        added.push(rec);
      } catch (e) {
        toast.show(`照片「${f.name}」上传失败：` + (e instanceof Error ? e.message : String(e)), 'error');
      }
    }
    if (added.length > 0) setPhotos((prev) => [...prev, ...added]);
  }

  async function setCover(photoId: string) {
    try {
      const others = photos.filter((p) => p.is_cover && p.id !== photoId);
      for (const p of others) {
        await api.updatePhotoCover(p.id, false);
      }
      await api.updatePhotoCover(photoId, true);
      setPhotos((prev) => prev.map((p) => ({ ...p, is_cover: p.id === photoId })));
    } catch (e) {
      toast.show('设置封面失败：' + (e instanceof Error ? e.message : String(e)), 'error');
    }
  }

  async function doDeletePhoto() {
    if (!confirmDeletePhoto) return;
    const pid = confirmDeletePhoto;
    setConfirmDeletePhoto(null);
    try {
      await api.deletePhoto(pid);
      setPhotos((prev) => prev.filter((p) => p.id !== pid));
    } catch (e) {
      toast.show('删除失败：' + (e instanceof Error ? e.message : String(e)), 'error');
    }
  }

  async function doDeletePerson() {
    if (!confirmDeletePerson) return;
    const pid = confirmDeletePerson;
    setConfirmDeletePerson(null);
    try {
      await api.deletePerson(pid);
      setPersons((prev) => prev.filter((p) => p.id !== pid));
    } catch (e) {
      toast.show('删除失败：' + (e instanceof Error ? e.message : String(e)), 'error');
    }
  }

  async function handleCreateRelation(
    personId: string,
    relatedPersonId: string,
    relationType: string,
  ) {
    try {
      const result = await api.createRelation(personId, {
        person_id: personId,
        related_person_id: relatedPersonId,
        relation_type: relationType as 'spouse' | 'parent' | 'child' | 'joint_burial',
      });
      setRelations((prev) => ({ ...prev, [personId]: result }));
      setRelatingPerson(null);
    } catch (e) {
      toast.show('关联失败：' + (e instanceof Error ? e.message : String(e)), 'error');
    }
  }

  const BackIcon = (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.5} className="w-5 h-5">
      <path strokeLinecap="round" strokeLinejoin="round" d="M15 18l-6-6 6-6" />
    </svg>
  );

  if (loading) {
    return (
      <div className="page-container app-screen">
        <div className="app-header with-back">
          <button className="back-btn" onClick={() => navigate(-1)} aria-label="返回">{BackIcon}</button>
          <h1>墓地</h1>
          <div className="header-spacer" />
        </div>
        <p className="empty-tip">加载中...</p>
      </div>
    );
  }

  if (error || !tomb) {
    return (
      <div className="page-container app-screen">
        <div className="app-header with-back">
          <button className="back-btn" onClick={() => navigate(-1)} aria-label="返回">{BackIcon}</button>
          <h1>墓地</h1>
          <div className="header-spacer" />
        </div>
        <p className="empty-tip">{error ?? '墓地不存在'}</p>
      </div>
    );
  }

  return (
    <div className="page-container app-screen">
      <div className="app-header with-back">
        <button className="back-btn" onClick={() => navigate(-1)} aria-label="返回">{BackIcon}</button>
        <h1>{tomb.name}</h1>
        <button className="header-action" onClick={() => navigate(`/tomb/${tomb.id}/edit`)}>
          编辑
        </button>
      </div>

      <div className="meta-line">
        <span>坐标：{tomb.latitude.toFixed(6)}, {tomb.longitude.toFixed(6)}</span>
        {tomb.address && <span>地址：{tomb.address}</span>}
      </div>
      {tomb.description && <p className="tomb-desc">{tomb.description}</p>}

      <div className="detail-actions">
        <button className="action-btn" onClick={() => openExternalMap(tomb)}>导航前往</button>
        <button
          className="action-btn"
          onClick={() => navigate(`/map?mode=view&tombId=${tomb.id}`)}
        >
          查看位置
        </button>
      </div>

      <div className="section-title-row">
        <h2>照片（{photos.length}）</h2>
        <button className="link-add" onClick={() => fileInputRef.current?.click()}>
          + 添加
        </button>
      </div>
      <input
        ref={fileInputRef}
        type="file"
        accept="image/*"
        multiple
        style={{ display: 'none' }}
        onChange={(e) => handleFiles(e.target.files)}
      />
      {photos.length === 0 ? (
        <p className="empty-tip">暂无照片</p>
      ) : (
        <div className="photo-grid">
          {photos.map((p) => (
            <div key={p.id} className={`photo-card ${p.is_cover ? 'is-cover' : ''}`}>
              <div className="photo-thumb">
                <img
                  src={api.photoUrl(p)}
                  alt={p.caption ?? '照片'}
                  loading="lazy"
                  onError={(e) => {
                    (e.currentTarget as HTMLImageElement).style.display = 'none';
                  }}
                />
                {p.is_cover && <span className="cover-badge">封面</span>}
              </div>
              <div className="photo-caption">{p.caption ?? p.file_path}</div>
              <div className="photo-actions">
                {!p.is_cover && (
                  <button className="link-btn" onClick={() => setCover(p.id)}>
                    设为封面
                  </button>
                )}
                <button className="link-btn danger" onClick={() => setConfirmDeletePhoto(p.id)}>
                  删除
                </button>
              </div>
            </div>
          ))}
        </div>
      )}

      <div className="section-title-row">
        <h2>安葬人物（{persons.length}）</h2>
        <button
          className="link-add"
          onClick={() => navigate(`/tomb/${tomb.id}/person/new`)}
        >
          + 添加
        </button>
      </div>
      {persons.length === 0 ? (
        <p className="empty-tip">暂无人物记录</p>
      ) : (
        <div className="person-list">
          {persons.map((p) => (
            <div
              key={p.id}
              className="card person-card"
              onClick={() => navigate(`/tomb/${tomb.id}/person/${p.id}`)}
            >
              <div className="card-title">
                {p.title ? `${p.title} · ` : ''}
                {p.name}
                {p.is_joint_burial && <span className="badge joint">合墓</span>}
                <span className="card-hint">点击编辑</span>
              </div>
              <div className="card-sub">
                {p.birth_date ?? '?'} ~ {p.death_date ?? '至今'}
              </div>
              <div className="person-meta">
                {p.spouse && (
                  <div>
                    <span className="meta-label">配偶</span>
                    <span className="meta-value">{p.spouse}</span>
                  </div>
                )}
                {p.children && (
                  <div>
                    <span className="meta-label">子女</span>
                    <span className="meta-value">{p.children}</span>
                  </div>
                )}
              </div>
              {relations[p.id] && relations[p.id].length > 0 && (
                <div className="relation-links">
                  {relations[p.id].map((r) => (
                    <span key={r.id} className="relation-tag">
                      <span className="meta-label">
                        {r.relation_type === 'spouse'
                          ? '配偶'
                          : r.relation_type === 'parent'
                            ? '父母'
                            : r.relation_type === 'child'
                              ? '子女'
                              : '合墓'}
                        :{' '}
                      </span>
                      {r.related_person_tomb_id ? (
                        <a
                          className="relation-link"
                          onClick={(e) => {
                            e.stopPropagation();
                            navigate(`/tomb/${r.related_person_tomb_id}`);
                          }}
                        >
                          {r.related_person_name ?? '未知'}
                        </a>
                      ) : (
                        <span className="meta-value">{r.related_person_name ?? '未知'}</span>
                      )}
                    </span>
                  ))}
                </div>
              )}
              {p.biography && <div className="card-desc">{p.biography}</div>}
              {p.epitaph && <div className="card-epitaph">墓志：{p.epitaph}</div>}
              <div className="card-actions" onClick={(e) => e.stopPropagation()}>
                <button className="link-btn" onClick={() => setRelatingPerson(p)}>
                  关联
                </button>
                <button className="link-btn danger" onClick={() => setConfirmDeletePerson(p.id)}>
                  删除
                </button>
              </div>
            </div>
          ))}
        </div>
      )}

      <BottomSheet
        open={relatingPerson !== null}
        title={relatingPerson ? `关联「${relatingPerson.name}」` : ''}
        onClose={() => setRelatingPerson(null)}
      >
        {relatingPerson && (
          <RelationPicker
            personId={relatingPerson.id}
            persons={persons}
            onConfirm={handleCreateRelation}
            onCancel={() => setRelatingPerson(null)}
          />
        )}
      </BottomSheet>

      <ConfirmSheet
        open={confirmDeletePerson !== null}
        title="删除人物"
        message="确定删除该人物记录？此操作不可撤销。"
        confirmText="删除"
        danger
        onConfirm={doDeletePerson}
        onCancel={() => setConfirmDeletePerson(null)}
      />
      <ConfirmSheet
        open={confirmDeletePhoto !== null}
        title="删除照片"
        message="确定删除这张照片？"
        confirmText="删除"
        danger
        onConfirm={doDeletePhoto}
        onCancel={() => setConfirmDeletePhoto(null)}
      />
    </div>
  );
}

// ---------------------------------------------------------------------------
// 关联人物选择器（底部 Sheet 内）
// ---------------------------------------------------------------------------
function RelationPicker({
  personId,
  persons,
  onConfirm,
  onCancel,
}: {
  personId: string;
  persons: Person[];
  onConfirm: (pid: string, rpid: string, rtype: string) => void;
  onCancel: () => void;
}) {
  const [selectedPerson, setSelectedPerson] = useState('');
  const [relationType, setRelationType] = useState<'spouse' | 'parent' | 'child' | 'joint_burial'>(
    'spouse',
  );
  const others = persons.filter((p) => p.id !== personId);

  const submit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!selectedPerson) return;
    onConfirm(personId, selectedPerson, relationType);
  };

  return (
    <form onSubmit={submit} className="form sheet-form">
      <label className="form-row">
        <span>关联人物</span>
        <select value={selectedPerson} onChange={(e) => setSelectedPerson(e.target.value)}>
          <option value="">-- 请选择 --</option>
          {others.map((p) => (
            <option key={p.id} value={p.id}>
              {p.title ? `${p.title} · ` : ''}
              {p.name}
            </option>
          ))}
        </select>
      </label>
      <label className="form-row">
        <span>关系类型</span>
        <select
          value={relationType}
          onChange={(e) => setRelationType(e.target.value as 'spouse' | 'parent' | 'child' | 'joint_burial')}
        >
          <option value="spouse">配偶</option>
          <option value="parent">父母</option>
          <option value="child">子女</option>
          <option value="joint_burial">合墓</option>
        </select>
      </label>
      <div className="sheet-actions">
        <button type="button" className="sheet-btn" onClick={onCancel}>
          取消
        </button>
        <button type="submit" className="sheet-btn primary" disabled={!selectedPerson}>
          确认关联
        </button>
      </div>
    </form>
  );
}
