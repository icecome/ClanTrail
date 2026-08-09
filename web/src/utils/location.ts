import { isTauri } from '@tauri-apps/api/core';
import {
  checkPermissions,
  getCurrentPosition,
  requestPermissions,
} from '@tauri-apps/plugin-geolocation';

export interface Coord {
  lat: number;
  lng: number;
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
    let perm = await checkPermissions();
    if (perm.location !== 'granted') {
      perm = await requestPermissions(['location']);
    }
    if (perm.location !== 'granted') {
      throw new Error('未授权定位权限');
    }
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
