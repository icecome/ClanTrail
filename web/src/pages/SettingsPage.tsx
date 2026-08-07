import { useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { api } from '../api/client';
import { useToast } from '../components/Toast';
import ConfirmSheet from '../components/ConfirmSheet';

export default function SettingsPage() {
  const navigate = useNavigate();
  const toast = useToast();
  const [showImportConfirm, setShowImportConfirm] = useState(false);
  const [importFile, setImportFile] = useState<File | null>(null);
  const importInputRef = useRef<HTMLInputElement>(null);

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
      toast.show('已导出备份', 'success');
    } catch (e) {
      toast.show('导出失败：' + (e instanceof Error ? e.message : String(e)), 'error');
    }
  }

  function pickImport(file: File | undefined) {
    if (!file) return;
    setImportFile(file);
    setShowImportConfirm(true);
  }

  async function doImport() {
    if (!importFile) return;
    setShowImportConfirm(false);
    try {
      const r = await api.importBackup(importFile);
      toast.show(`导入成功，备份已存至 ${r.backup_dir}`, 'success');
    } catch (e) {
      toast.show('导入失败：' + (e instanceof Error ? e.message : String(e)), 'error');
    } finally {
      setImportFile(null);
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
        <h1>设置</h1>
        <div className="header-spacer" />
      </div>

      <div className="form-page">
        <div className="section-title-row">
          <span>数据管理</span>
        </div>
        <div className="list">
          <div className="list-item" onClick={handleExport}>
            <div className="list-item-main">
              <div className="list-item-title">导出备份</div>
              <div className="list-item-sub">导出数据库与照片为 zip 文件</div>
            </div>
            <svg className="chevron" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.5}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M8.25 4.5l7.5 7.5-7.5 7.5" />
            </svg>
          </div>
          <div className="list-item" onClick={() => importInputRef.current?.click()}>
            <div className="list-item-main">
              <div className="list-item-title">导入备份</div>
              <div className="list-item-sub">从 zip 恢复（将覆盖当前数据）</div>
            </div>
            <svg className="chevron" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.5}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M8.25 4.5l7.5 7.5-7.5 7.5" />
            </svg>
          </div>
          <input
            ref={importInputRef}
            type="file"
            accept=".zip,application/zip"
            style={{ display: 'none' }}
            onChange={(e) => pickImport(e.target.files?.[0])}
          />
        </div>

        <div className="section-title-row">
          <span>关于</span>
        </div>
        <div className="list">
          <div className="list-item" style={{ cursor: 'default' }}>
            <div className="list-item-main">
              <div className="list-item-title">家族墓地档案</div>
              <div className="list-item-sub">版本 0.1.0 · 记录家族墓地、墓主忌日与祭祀提醒</div>
            </div>
          </div>
        </div>

        <div className="section-title-row">
          <span>定位说明</span>
        </div>
        <p className="locate-hint" style={{ padding: '0 16px' }}>
          添加墓地时，可点击「使用当前经纬度」自动获取 GPS 坐标，也可手动输入经度与纬度。
        </p>
      </div>

      <ConfirmSheet
        open={showImportConfirm}
        title="导入备份"
        message="导入会覆盖当前数据，请确认已导出最新备份。是否继续？"
        confirmText="继续导入"
        danger
        onConfirm={doImport}
        onCancel={() => { setShowImportConfirm(false); setImportFile(null); }}
      />
    </div>
  );
}
