import { Routes, Route, NavLink } from 'react-router-dom';
import FamilyListPage from './pages/FamilyListPage';
import FamilyDetailPage from './pages/FamilyDetailPage';
import TombDetailPage from './pages/TombDetailPage';
import RemindersPage from './pages/RemindersPage';
import MapPage from './pages/MapPage';
import AddTombPage from './pages/AddTombPage';
import SettingsPage from './pages/SettingsPage';
import PersonFormPage from './pages/PersonFormPage';
import { ToastProvider } from './components/Toast';

function FamilyIcon() {
  return (
    <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
      <path strokeLinecap="round" strokeLinejoin="round" d="M15 19.128a9.38 9.38 0 002.625.372 9.337 9.337 0 004.121-.952 4.125 4.125 0 00-7.533-2.493M15 19.128v-.003c0-1.113-.285-2.16-.786-3.07M15 19.128v.106A12.318 12.318 0 018.624 21c-2.331 0-4.512-.645-6.374-1.766l-.001-.109a6.375 6.375 0 0111.964-3.07M12 6.375a3.375 3.375 0 11-6.75 0 3.375 3.375 0 016.75 0zm8.25 2.25a2.625 2.625 0 11-5.25 0 2.625 2.625 0 015.25 0z" />
    </svg>
  );
}

function BellIcon() {
  return (
    <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
      <path strokeLinecap="round" strokeLinejoin="round" d="M14.857 17.082a23.848 23.848 0 005.454-1.31A8.967 8.967 0 0118 9.75V9A6 6 0 006 9v.75a8.967 8.967 0 01-2.312 6.022c1.733.64 3.56 1.085 5.455 1.31m5.714 0a24.255 24.255 0 01-5.714 0m5.714 0a3 3 0 11-5.714 0" />
    </svg>
  );
}

export default function App() {
  const navLinkClass = ({ isActive }: { isActive: boolean }) =>
    `px-3.5 py-1.5 rounded-md text-sm no-underline transition-colors ${
      isActive ? 'bg-[#e8f3ff] text-[#165dff] font-medium' : 'text-[#4e5969] hover:bg-[#f2f3f5]'
    }`;

  const tabLinkClass = ({ isActive }: { isActive: boolean }) =>
    `flex flex-col items-center justify-center gap-0.5 py-1 px-4 no-underline ${
      isActive ? 'text-[#165dff]' : 'text-[#86909c]'
    }`;

  return (
    <ToastProvider>
      <div className="flex flex-col h-full">
        {/* 桌面端顶部导航 (md+) */}
        <nav className="hidden md:flex items-center justify-between h-[52px] px-5 bg-white border-b border-[#e5e6eb] shrink-0">
          <div className="text-base font-medium text-[#1f2329]">家族墓地档案</div>
          <div className="flex gap-2">
            <NavLink to="/" end className={navLinkClass}>
              家族
            </NavLink>
            <NavLink to="/reminders" className={navLinkClass}>
              提醒
            </NavLink>
            <NavLink to="/map" className={navLinkClass}>
              地图
            </NavLink>
          </div>
        </nav>

        {/* 主内容区 */}
        <main className="flex-1 overflow-auto max-md:pb-16">
          <Routes>
            <Route path="/" element={<FamilyListPage />} />
            <Route path="/families/:id" element={<FamilyDetailPage />} />
            <Route path="/tomb/new" element={<AddTombPage />} />
            <Route path="/tomb/:id/edit" element={<AddTombPage />} />
            <Route path="/tomb/:id" element={<TombDetailPage />} />
            <Route path="/tomb/:id/person/:pid" element={<PersonFormPage />} />
            <Route path="/reminders" element={<RemindersPage />} />
            <Route path="/settings" element={<SettingsPage />} />
            <Route path="/map" element={<MapPage />} />
          </Routes>
        </main>

        {/* 移动端底部 Tab Bar (max-md) */}
        <nav className="md:hidden fixed bottom-0 inset-x-0 z-50 bg-white border-t border-[#e5e6eb] flex items-center justify-around h-14 pb-[env(safe-area-inset-bottom,0)]">
          <NavLink to="/" end className={tabLinkClass}>
            <FamilyIcon />
            <span className="text-[10px]">家族</span>
          </NavLink>
          <NavLink to="/reminders" className={tabLinkClass}>
            <BellIcon />
            <span className="text-[10px]">提醒</span>
          </NavLink>
        </nav>
      </div>
    </ToastProvider>
  );
}
