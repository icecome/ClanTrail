import { useEffect, useRef, useState } from 'react';
import { Link } from 'react-router-dom';
import { api } from '../api/client';
import type { Family } from '../types';

export default function FamiliesPage() {
  const [families, setFamilies] = useState<Family[]>([]);
  const [loading, setLoading] = useState(true);
  const importInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    api.listFamilies().then((data) => {
      setFamilies(data);
      setLoading(false);
    });
  }, []);

  async function handleExport() {
    try {
      const blob = await api.exportBackup();
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `tomb-keeper-backup-${new Date().toISOString().slice(0, 10)}.zip`;
      document.body.appendChild(a);
      a.click();
      a.remove();
      URL.revokeObjectURL(url);
    } catch (e) {
      alert('导出失败：' + (e instanceof Error ? e.message : String(e)));
    }
  }

  async function handleImport(file: File | undefined) {
    if (!file) return;
    if (!confirm('导入备份会覆盖当前数据，请先确认已导出最新备份。是否继续？')) return;
    try {
      const result = await api.importBackup(file);
      alert(`导入成功（备份已保存到：${result.backup_dir}）。请刷新页面查看最新数据。`);
      window.location.reload();
    } catch (e) {
      alert('导入失败：' + (e instanceof Error ? e.message : String(e)));
    }
  }

  if (loading) return <div className="page-hint">加载中...</div>;

  if (families.length === 0) {
    return <div className="page-hint">暂无家族档案，请先在地图上创建墓地。</div>;
  }

  return (
    <div className="page">
      <div className="page-title-row">
        <h1 className="page-title">家族档案</h1>
        <div className="page-actions">
          <button className="btn btn-small" onClick={handleExport}>导出备份</button>
          <button className="btn btn-small" onClick={() => importInputRef.current?.click()}>导入备份</button>
          <input
            ref={importInputRef}
            type="file"
            accept=".zip,application/zip"
            style={{ display: 'none' }}
            onChange={(e) => handleImport(e.target.files?.[0])}
          />
        </div>
      </div>
      <div className="card-list">
        {families.map((f) => (
          <Link key={f.id} to={`/families/${f.id}`} className="card">
            <div className="card-title">{f.name}</div>
            {f.origin && <div className="card-subtitle">祖籍：{f.origin}</div>}
            {f.description && <div className="card-desc">{f.description}</div>}
          </Link>
        ))}
      </div>
    </div>
  );
}
