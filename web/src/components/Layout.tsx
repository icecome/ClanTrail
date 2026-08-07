import { NavLink, Outlet } from 'react-router-dom';
import { useEffect } from 'react';
import { api } from '../api/client';

export default function Layout() {
  useEffect(() => {
    // 空库时写入演示数据，方便联调
    api.seedDemoData().catch(() => {
      /* 后端未启动时静默失败 */
    });
  }, []);

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="sidebar-title">Tomb Keeper</div>
        <nav className="sidebar-nav">
          <NavLink to="/" end className={({ isActive }) => (isActive ? 'nav-item active' : 'nav-item')}>
            地图
          </NavLink>
          <NavLink to="/families" className={({ isActive }) => (isActive ? 'nav-item active' : 'nav-item')}>
            家族
          </NavLink>
        </nav>
        <div className="sidebar-footer">家族墓地数字档案</div>
      </aside>
      <main className="main-content">
        <Outlet />
      </main>
    </div>
  );
}
