import { useEffect, useRef, useState } from 'react';
import AMapLoader from '@amap/amap-jsapi-loader';
import { useNavigate, useSearchParams } from 'react-router-dom';
import type { Tomb } from '../types';
import { api } from '../api/client';
import { openExternalMap } from '../utils/navigation';

// 高德 2.0 必须在 loader.load 之前设置安全密钥
declare global {
  interface Window {
    _AMapSecurityConfig: { securityJsCode: string };
  }
}

window._AMapSecurityConfig = {
  securityJsCode: import.meta.env.VITE_AMAP_SECURITY_CODE,
};

// AMap loader 返回 any，用最小化的本地类型描述（官方 d.ts 过大）
interface AMapNamespace {
  Map: new (container: HTMLElement, opts: unknown) => AMapInstance;
  Marker: new (opts: unknown) => AMapMarker;
  ToolBar: new (opts?: unknown) => unknown;
  Scale: new () => unknown;
}

interface AMapInstance {
  addControl: (ctrl: unknown) => void;
  on: (event: string, cb: (e: { lnglat: { lng: number; lat: number } }) => void) => void;
  destroy: () => void;
}

interface AMapMarker {
  on: (
    event: string,
    cb: (e?: { target?: { getPosition: () => { lng: number; lat: number } } }) => void,
  ) => void;
  setMap: (map: AMapInstance | null) => void;
  setPosition: (pos: [number, number]) => void;
  setDraggable: (v: boolean) => void;
  getPosition: () => { lng: number; lat: number };
}

export default function MapPage() {
  const [params] = useSearchParams();
  const mode = params.get('mode') ?? 'view';
  const returnTo = params.get('returnTo') ?? '/';
  const tombId = params.get('tombId') ?? null;
  const mapContainer = useRef<HTMLDivElement>(null);
  const [tombs, setTombs] = useState<Tomb[]>([]);
  const [error, setError] = useState<string | null>(null);
  // 选点模式下已选取的坐标（点击地图放置标记后填充，确认前可拖动微调）
  const [selectedCoord, setSelectedCoord] = useState<{ lat: number; lng: number } | null>(null);
  const pickMarkerRef = useRef<AMapMarker | null>(null);
  // 离线薄兜底：高德 JS API 加载失败时（无网络/被拦截），不白屏，降级为墓地清单
  const [mapFailed, setMapFailed] = useState(false);
  const navigate = useNavigate();

  useEffect(() => {
    let destroyed = false;
    let map: AMapInstance | null = null;
    const markers: AMapMarker[] = [];
    (async () => {
      let list: Tomb[] = [];
      try {
        if (tombId) {
          const t = await api.getTomb(tombId);
          if (t) list = [t];
        } else {
          list = await api.listTombs();
        }
      } catch {
        setError('加载数据失败');
      }
      if (destroyed) return;
      setTombs(list);

      try {
        const loaded: unknown = await AMapLoader.load({
          key: import.meta.env.VITE_AMAP_KEY,
          version: '2.0',
          plugins: ['AMap.ToolBar', 'AMap.Scale'],
        });
        if (destroyed || !mapContainer.current) return;
        const AMap = loaded as AMapNamespace;
        const center =
          mode === 'view' && list.length > 0
            ? [list[0].longitude, list[0].latitude]
            : [108.0, 34.0];
        map = new AMap.Map(mapContainer.current, {
          center,
          zoom: mode === 'view' && list.length > 0 ? 15 : 4,
          viewMode: '2D',
        });
        map.addControl(new AMap.ToolBar({ position: 'RT' }));
        map.addControl(new AMap.Scale());
        if (mode === 'view') {
          for (const t of list) {
            const marker = new AMap.Marker({
              position: [t.longitude, t.latitude],
              title: t.name,
              extData: { tombId: t.id },
            });
            marker.on('click', () => navigate(`/tomb/${t.id}`));
            marker.setMap(map);
            markers.push(marker);
          }
        } else if (mode === 'select') {
          // 选点模式：点击地图放置一个可拖动标记，确认前不跳转，避免误触已有墓地
          map.on('click', (e) => {
            const { lng, lat } = e.lnglat;
            let m = pickMarkerRef.current;
            if (!m) {
              m = new AMap.Marker({ position: [lng, lat], draggable: true });
              m.on('dragend', () => {
                const p = pickMarkerRef.current?.getPosition();
                if (p) setSelectedCoord({ lat: p.lat, lng: p.lng });
              });
              m.setMap(map);
              pickMarkerRef.current = m;
            } else {
              m.setPosition([lng, lat]);
            }
            setSelectedCoord({ lat, lng });
          });
        }
      } catch (err) {
        if (destroyed) return;
        // 离线/被拦截时不白屏，降级为清单视图
        setMapFailed(true);
        setError('地图组件加载失败，已切换为离线清单模式');
        console.warn('高德地图加载失败：', err);
      }
    })();

    return () => {
      destroyed = true;
      markers.forEach((m) => m.setMap(null));
      pickMarkerRef.current?.setMap(null);
      map?.destroy();
      map = null;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [mode, returnTo, tombId, navigate]);

  const title = mode === 'select' ? '选择位置' : '查看位置';

  function confirmPick() {
    if (selectedCoord) navigate(returnTo, { state: { selectedCoord } });
  }

  function resetPick() {
    setSelectedCoord(null);
    pickMarkerRef.current?.setMap(null);
    pickMarkerRef.current = null;
  }

  return (
    <div className="page-container app-screen map-tool-screen">
      <div className="app-header with-back">
        <button className="back-btn" onClick={() => navigate(-1)} aria-label="返回">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.5} className="w-5 h-5">
            <path strokeLinecap="round" strokeLinejoin="round" d="M15 18l-6-6 6-6" />
          </svg>
        </button>
        <h1>{title}</h1>
        <div className="header-spacer" />
      </div>

      {mode === 'select' && <p className="map-hint">点击地图任意位置，选择墓地坐标</p>}

      {mode === 'view' && tombs.length === 1 && (
        <div className="map-target-info">
          <div className="map-target-main">
            <div className="list-item-title">{tombs[0].name}</div>
            <div className="list-item-sub">
              {tombs[0].latitude.toFixed(6)}, {tombs[0].longitude.toFixed(6)}
            </div>
          </div>
          <button className="btn btn-small" onClick={() => openExternalMap(tombs[0])}>
            导航前往
          </button>
        </div>
      )}

      {mapFailed ? (
        <div className="offline-list">
          <p className="offline-tip">{error ?? '当前为离线清单模式（地图组件不可用）。'}</p>
          {tombs.map((t) => (
            <div key={t.id} className="card offline-item" onClick={() => navigate(`/tomb/${t.id}`)}>
              <div className="offline-item-main">
                <div className="card-title">{t.name}</div>
                {t.address && <div className="card-sub">{t.address}</div>}
                <div className="card-desc">
                  {t.latitude.toFixed(6)}, {t.longitude.toFixed(6)}
                </div>
              </div>
              <button
                className="btn btn-ghost"
                onClick={(e) => {
                  e.stopPropagation();
                  openExternalMap(t);
                }}
              >
                导航
              </button>
            </div>
          ))}
          {tombs.length === 0 && <p className="empty-tip">暂无可显示的位置</p>}
        </div>
      ) : (
        <div ref={mapContainer} className="map-container-full" />
      )}

      {mode === 'select' && selectedCoord && (
        <div className="map-pick-bar">
          <div className="map-pick-info">
            <span className="map-pick-label">已选位置</span>
            <span className="map-pick-coord">
              {selectedCoord.lat.toFixed(6)}, {selectedCoord.lng.toFixed(6)}
            </span>
          </div>
          <div className="map-pick-actions">
            <button className="pick-btn ghost" onClick={resetPick}>
              重选
            </button>
            <button className="pick-btn primary" onClick={confirmPick}>
              确认选择
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
