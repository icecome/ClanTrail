import type { Tomb } from '../types';
import BottomSheet from './BottomSheet';
import { useToast } from './Toast';
import { MAP_PROVIDERS, type MapProvider, openExternalMap } from '../utils/navigation';

interface Props {
  open: boolean;
  tomb: Pick<Tomb, 'name' | 'latitude' | 'longitude'> | null;
  onClose: () => void;
}

// 导航地图选择：适配高德 / 百度 / 腾讯，唤起对应 App（未安装时由系统兜底网页版）
export default function MapNavSheet({ open, tomb, onClose }: Props) {
  const toast = useToast();
  if (!tomb) return null;

  async function handlePick(provider: MapProvider) {
    if (!tomb) return;
    try {
      await openExternalMap(tomb, provider);
      onClose();
    } catch (e) {
      toast.show('唤起地图失败：' + (e instanceof Error ? e.message : String(e)), 'error');
    }
  }

  return (
    <BottomSheet open={open} title="选择导航地图" onClose={onClose}>
      <div className="map-nav-list">
        {MAP_PROVIDERS.map((p) => (
          <button key={p.id} className="map-nav-item" onClick={() => handlePick(p.id)}>
            {p.name}
          </button>
        ))}
      </div>
    </BottomSheet>
  );
}
