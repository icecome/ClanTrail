import type { Tomb } from '../types';

/**
 * 调起外部地图 App 前往墓地（导航伴行）。
 *
 * 隐私克制：仅传递坐标 + 墓地名称（地名），不携带任何逝者/人物信息。
 * Web 端用高德 URI API：手机上唤起高德 App，桌面端打开高德网页版。
 */
export function openExternalMap(tomb: Pick<Tomb, 'name' | 'latitude' | 'longitude'>): void {
  const name = encodeURIComponent(tomb.name || '墓地');
  const url =
    `https://uri.amap.com/marker?position=${tomb.longitude},${tomb.latitude}` +
    `&name=${name}&src=tombkeeper&coordinate=gaode&callnative=1`;
  window.open(url, '_blank', 'noopener');
}
