import type { ReactNode } from 'react';
import { Routes, Route, useLocation, useNavigate } from 'react-router-dom';
import ClanListPage from './pages/ClanListPage';
import ClanDetailPage from './pages/ClanDetailPage';
import GraveDetailPage from './pages/GraveDetailPage';
import RemindersPage from './pages/RemindersPage';
import MapPage from './pages/MapPage';
import AddGravePage from './pages/AddGravePage';
import SettingsPage from './pages/SettingsPage';
import MemberFormPage from './pages/MemberFormPage';
import { ToastProvider } from './components/Toast';

// ---------- 线性图标（设计系统 24 图标集子集） ----------
function HomeIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.6} strokeLinecap="round" strokeLinejoin="round">
      <path d="M4 11l8-7 8 7" />
      <path d="M6 10v9h12v-9" />
      <path d="M10 19v-5h4v5" />
    </svg>
  );
}

function TimeIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.6} strokeLinecap="round" strokeLinejoin="round">
      <circle cx="12" cy="13" r="7" />
      <path d="M12 9v4l3 2" />
      <path d="M5 4h14" />
    </svg>
  );
}

function MapIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.6} strokeLinecap="round" strokeLinejoin="round">
      <path d="M9 4l6 2 6-2v14l-6 2-6-2-6 2V6l6-2z" />
      <path d="M9 4v14M15 6v14" />
    </svg>
  );
}

function UserIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.6} strokeLinecap="round" strokeLinejoin="round">
      <circle cx="12" cy="8" r="3.4" />
      <path d="M5.5 19a6.5 6.5 0 0113 0" />
    </svg>
  );
}

interface TabDef {
  path: string;
  label: string;
  icon: () => ReactNode;
  // 该 Tab 处于激活态时匹配的路径前缀
  activePrefixes: string[];
}

const TABS: TabDef[] = [
  { path: '/', label: '家族', icon: HomeIcon, activePrefixes: ['/', '/clans', '/grave'] },
  { path: '/reminders', label: '时序', icon: TimeIcon, activePrefixes: ['/reminders'] },
  { path: '/map', label: '地图', icon: MapIcon, activePrefixes: ['/map'] },
  { path: '/settings', label: '我的', icon: UserIcon, activePrefixes: ['/settings'] },
];

export default function App() {
  const location = useLocation();
  const navigate = useNavigate();
  // 子流程（新建/编辑墓地、人物表单、选点、地图全屏预览）不显示底栏
  const isSubFlow =
    location.pathname === '/grave/new' ||
    /^\/grave\/[^/]+\/edit$/.test(location.pathname) ||
    /^\/grave\/[^/]+\/member\//.test(location.pathname) ||
    /^\/member\//.test(location.pathname) ||
    /^\/clans\/[^/]+\/(members|graph)$/.test(location.pathname) ||
    (location.pathname === '/map' && location.search.includes('mode=select')) ||
    /^\/settings\/(backup|privacy|gps|about)$/.test(location.pathname) ||
    (location.pathname === '/map' && (location.search.includes('graveId=') || location.search.includes('view=full')));

  const activeTab = (() => {
    const path = location.pathname;
    if (path.startsWith('/reminders')) return 1;
    if (path.startsWith('/map')) return 2;
    if (path.startsWith('/settings')) return 3;
    return 0;
  })();

  return (
    <ToastProvider>
      <div className="app-shell">
        <main className="main-content">
          <Routes>
            <Route path="/" element={<ClanListPage />} />
            <Route path="/clans/:id" element={<ClanDetailPage />} />
            <Route path="/member/:pid/edit" element={<MemberFormPage />} />
            <Route path="/grave/new" element={<AddGravePage />} />
            <Route path="/grave/:id/edit" element={<AddGravePage />} />
            <Route path="/grave/:id" element={<GraveDetailPage />} />
            <Route path="/grave/:id/member/:pid" element={<MemberFormPage />} />
            <Route path="/reminders" element={<RemindersPage />} />
            <Route path="/settings" element={<SettingsPage />} />
            <Route path="/map" element={<MapPage />} />
          </Routes>
        </main>

        {/* 移动端玻璃态底栏：家族 / 时序 / 地图 / 我的 */}
        {!isSubFlow && (
          <nav className="tab-bar">
            {TABS.map((t, i) => {
              const Icon = t.icon;
              const active = activeTab === i;
              return (
                <div
                  key={t.path}
                  className={`tab-item ${active ? 'active' : ''}`}
                  onClick={() => navigate(t.path, { replace: true })}
                  role="button"
                  tabIndex={0}
                >
                  <Icon />
                  <span>{t.label}</span>
                </div>
              );
            })}
          </nav>
        )}
      </div>
    </ToastProvider>
  );
}
