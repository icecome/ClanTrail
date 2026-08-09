import { useEffect, useRef, useState } from 'react';
import { useLocation, useNavigate, useParams, useSearchParams } from 'react-router-dom';
import { api } from '../api/client';
import { useToast } from '../components/Toast';

export default function AddTombPage() {
  const { id: editId } = useParams<{ id: string }>();
  const [params] = useSearchParams();
  const location = useLocation();
  const familyId = params.get('family_id') ?? undefined;
  const navigate = useNavigate();
  const toast = useToast();
  const isEdit = !!editId;
  // 防止从地图选点返回后，用户手动修改坐标又被旧 state 重复覆盖
  const coordConsumed = useRef(false);

  const [name, setName] = useState('');
  const [address, setAddress] = useState('');
  const [desc, setDesc] = useState('');
  const [latStr, setLatStr] = useState('');
  const [lngStr, setLngStr] = useState('');
  const [locating, setLocating] = useState(false);
  const [loading, setLoading] = useState(isEdit);

  useEffect(() => {
    if (!isEdit || !editId) return;
    (async () => {
      try {
        const t = await api.getTomb(editId);
        if (t) {
          setName(t.name);
          setAddress(t.address ?? '');
          setDesc(t.description ?? '');
          setLatStr(t.latitude.toFixed(6));
          setLngStr(t.longitude.toFixed(6));
        }
      } catch (e) {
        toast.show('加载墓地失败：' + (e instanceof Error ? e.message : String(e)), 'error');
      } finally {
        setLoading(false);
      }
    })();
  }, [editId, isEdit]);

  // 从地图选点返回时，自动填入坐标（仅消费一次）
  useEffect(() => {
    if (coordConsumed.current) return;
    const sc = (location.state as { selectedCoord?: { lat: number; lng: number } } | null)?.selectedCoord;
    if (sc && typeof sc.lat === 'number' && typeof sc.lng === 'number') {
      coordConsumed.current = true;
      setLatStr(sc.lat.toFixed(6));
      setLngStr(sc.lng.toFixed(6));
      toast.show('已从地图选取坐标', 'success');
    }
  }, [location.state, toast]);

  function openMapPick() {
    const returnTo = location.pathname + location.search;
    navigate(`/map?mode=select&returnTo=${encodeURIComponent(returnTo)}`);
  }

  function useCurrentLocation() {
    if (!navigator.geolocation) {
      toast.show('当前设备不支持定位', 'error');
      return;
    }
    setLocating(true);
    navigator.geolocation.getCurrentPosition(
      (pos) => {
        setLatStr(pos.coords.latitude.toFixed(6));
        setLngStr(pos.coords.longitude.toFixed(6));
        setLocating(false);
        toast.show('已获取当前经纬度', 'success');
      },
      (err) => {
        setLocating(false);
        toast.show('定位失败：' + err.message, 'error');
      },
      { enableHighAccuracy: true, timeout: 10000 },
    );
  }

  function coordPreview(): string | null {
    const lat = parseFloat(latStr);
    const lng = parseFloat(lngStr);
    if (!isNaN(lat) && !isNaN(lng)) return `${lat.toFixed(6)}, ${lng.toFixed(6)}`;
    return null;
  }

  async function submit() {
    if (!name.trim()) {
      toast.show('请输入墓地名称', 'error');
      return;
    }
    const lat = parseFloat(latStr);
    const lng = parseFloat(lngStr);
    if (isNaN(lat) || isNaN(lng) || lat < -90 || lat > 90 || lng < -180 || lng > 180) {
      toast.show('请输入有效的经度（-180~180）与纬度（-90~90）', 'error');
      return;
    }
    try {
      const payload = {
        name: name.trim(),
        latitude: lat,
        longitude: lng,
        address: address.trim() || null,
        description: desc.trim() || null,
        group_id: null,
        family_id: familyId ?? null,
      };
      if (isEdit && editId) {
        await api.updateTomb(editId, payload);
        toast.show('已保存', 'success');
        navigate(`/tomb/${editId}`);
      } else {
        const created = await api.createTomb(payload);
        toast.show('墓地已创建', 'success');
        navigate(`/tomb/${created.id}`);
      }
    } catch (e) {
      toast.show('保存失败：' + (e instanceof Error ? e.message : String(e)), 'error');
    }
  }

  if (loading) return <div className="page-container">加载中...</div>;

  return (
    <div className="page-container app-screen">
      <div className="app-header with-back">
        <button className="back-btn" onClick={() => navigate(-1)} aria-label="返回">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.5} className="w-5 h-5">
            <path strokeLinecap="round" strokeLinejoin="round" d="M15 18l-6-6 6-6" />
          </svg>
        </button>
        <h1>{isEdit ? '编辑墓地' : '添加墓地'}</h1>
        <div className="header-spacer" />
      </div>

      <div className="form-page">
        <label className="form-row">
          <span>名称 *</span>
          <input value={name} onChange={(e) => setName(e.target.value)} placeholder="如：祖父之墓" />
        </label>

        <div className="form-row">
          <span>位置</span>
          <button className="locate-btn" onClick={useCurrentLocation} disabled={locating}>
            {locating ? '定位中...' : '使用当前经纬度'}
          </button>
          <button className="locate-btn locate-btn-ghost" onClick={openMapPick}>
            在地图中选点
          </button>
          <p className="locate-hint">
            {coordPreview() ? `已获取：${coordPreview()}` : '点击上方按钮自动获取，或在地图中选点，也可手动填写下方经纬度'}
          </p>
        </div>

        <div className="form-row form-row-grid">
          <label>
            <span>经度</span>
            <input
              type="number"
              step="0.000001"
              inputMode="decimal"
              placeholder="-180 ~ 180"
              value={lngStr}
              onChange={(e) => setLngStr(e.target.value)}
            />
          </label>
          <label>
            <span>纬度</span>
            <input
              type="number"
              step="0.000001"
              inputMode="decimal"
              placeholder="-90 ~ 90"
              value={latStr}
              onChange={(e) => setLatStr(e.target.value)}
            />
          </label>
        </div>

        <label className="form-row">
          <span>地址</span>
          <input value={address} onChange={(e) => setAddress(e.target.value)} placeholder="详细地址（可选）" />
        </label>
        <label className="form-row">
          <span>介绍</span>
          <textarea rows={3} value={desc} onChange={(e) => setDesc(e.target.value)} placeholder="墓地说明（可选）" />
        </label>

        <button className="btn-primary-block" onClick={submit}>
          保存墓地
        </button>
      </div>
    </div>
  );
}
