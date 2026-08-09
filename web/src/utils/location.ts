import { isTauri } from '@tauri-apps/api/core';
import {
  checkPermissions,
  clearWatch,
  getCurrentPosition,
  requestPermissions,
  watchPosition,
} from '@tauri-apps/plugin-geolocation';

export interface Coord {
  lat: number;
  lng: number;
}

export interface CoordWithHeading extends Coord {
  heading: number | null;
  accuracy: number;
}

/**
 * Tauri 环境统一定位权限获取：检查 → 请求 → 校验，未授权则抛错。
 * 供 getLocation / startWatching 复用，避免权限逻辑重复。
 */
async function ensureTauriLocationPermission(): Promise<void> {
  let perm = await checkPermissions();
  if (perm.location !== 'granted') {
    perm = await requestPermissions(['location']);
  }
  if (perm.location !== 'granted') {
    throw new Error('未授权定位权限');
  }
}

/**
 * 获取当前经纬度，跨端统一入口。
 *
 * - Tauri 移动端：走原生 geolocation 插件。首次调用会自动弹出系统定位授权框
 *   （Android 危险权限运行时请求），用户授权后方可定位。
 * - 桌面 / 非 Tauri 环境：回退到 Web 标准 navigator.geolocation。
 */
export async function getLocation(): Promise<Coord> {
  if (isTauri()) {
    await ensureTauriLocationPermission();
    const pos = await getCurrentPosition();
    return { lat: pos.coords.latitude, lng: pos.coords.longitude };
  }

  return new Promise<Coord>((resolve, reject) => {
    if (!navigator.geolocation) {
      reject(new Error('当前设备不支持定位'));
      return;
    }
    navigator.geolocation.getCurrentPosition(
      (pos) => resolve({ lat: pos.coords.latitude, lng: pos.coords.longitude }),
      (err) => reject(new Error(err.message)),
      { enableHighAccuracy: true, timeout: 10000 },
    );
  });
}

/**
 * 持续监听设备位置与方向（GPS heading）。
 * 返回 cleanup 函数，组件卸载时调用。
 */
export async function startWatching(
  onUpdate: (c: CoordWithHeading) => void,
  onError: (e: string) => void,
): Promise<() => void> {
  if (isTauri()) {
    await ensureTauriLocationPermission();
    try {
      const channelId = await watchPosition(
        { enableHighAccuracy: true, timeout: 5000, maximumAge: 0 },
        (pos, err) => {
          if (err) {
            onError(err);
          } else if (pos) {
            onUpdate({
              lat: pos.coords.latitude,
              lng: pos.coords.longitude,
              heading: pos.coords.heading,
              accuracy: pos.coords.accuracy,
            });
          }
        },
      );
      return () => { clearWatch(channelId); };
    } catch (e) {
      onError(e instanceof Error ? e.message : String(e));
      return () => {};
    }
  }

  // 非 Tauri：用 Web Geolocation API 的 watchPosition
  const watchId = navigator.geolocation.watchPosition(
    (pos) => {
      onUpdate({
        lat: pos.coords.latitude,
        lng: pos.coords.longitude,
        heading: pos.coords.heading,
        accuracy: pos.coords.accuracy,
      });
    },
    (err) => onError(err.message),
    { enableHighAccuracy: true, timeout: 5000, maximumAge: 0 },
  );
  return () => navigator.geolocation.clearWatch(watchId);
}
