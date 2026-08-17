import type { Tomb } from '../types';
import { isTauri } from '@tauri-apps/api/core';
import { openUrl } from '@tauri-apps/plugin-opener';
import { wgs84togcj02 } from './coord';

export type MapProvider = 'amap' | 'baidu' | 'tencent';

export const MAP_PROVIDERS: { id: MapProvider; name: string }[] = [
  { id: 'amap', name: '高德地图' },
  { id: 'baidu', name: '百度地图' },
  { id: 'tencent', name: '腾讯地图' },
];

/**
 * 构造各地图厂商的导航 URI（均仅传递坐标 + 地名，不携带逝者信息）。
 */
function buildMapUri(provider: MapProvider, grave: Pick<Tomb, 'name' | 'latitude' | 'longitude'>): string {
  const name = encodeURIComponent(grave.name || '墓地');
  // 数据库存 WGS84，高德/腾讯/百度调起统一转 GCJ-02（百度 coord_type=gcj02 由其内部转 BD-09）
  const [lng, lat] = wgs84togcj02(grave.longitude, grave.latitude);
  switch (provider) {
    case 'baidu':
      return `https://api.map.baidu.com/marker?location=${lat},${lng}&title=${name}&content=${name}&output=html&coord_type=gcj02&src=clantrail`;
    case 'tencent':
      return `https://apis.map.qq.com/uri/v1/marker?marker=coord:${lat},${lng};title:${name}&referer=clantrail`;
    case 'amap':
    default:
      return `https://uri.amap.com/marker?position=${lng},${lat}&name=${name}&src=clantrail&coordinate=gaode&callnative=1`;
  }
}

/**
 * 调起外部地图 App 前往墓地（导航伴行）。
 *
 * Tauri 环境用 opener 插件的 openUrl -> 系统 Intent.ACTION_VIEW，按 URL scheme
 * 唤起对应地图 App（未安装时由系统/浏览器兜底）。非 Tauri 环境回退 window.open。
 */
export async function openExternalMap(
  grave: Pick<Tomb, 'name' | 'latitude' | 'longitude'>,
  provider: MapProvider = 'amap',
): Promise<void> {
  const url = buildMapUri(provider, grave);
  if (isTauri()) {
    await openUrl(url);
    return;
  }
  window.open(url, '_blank', 'noopener');
}
