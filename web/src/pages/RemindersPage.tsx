import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import type { Reminder } from '../types';
import { api } from '../api/client';

function daysLabel(d: number): string {
  if (d <= 0) return '今天';
  if (d === 1) return '明天';
  if (d === 2) return '后天';
  return `${d} 天后`;
}

function badgeClass(t: Reminder['reminder_type']): string {
  return t === 'death_anniversary'
    ? 'reminder-badge death'
    : 'reminder-badge festival';
}

export default function RemindersPage() {
  const [reminders, setReminders] = useState<Reminder[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const navigate = useNavigate();

  useEffect(() => {
    (async () => {
      try {
        setReminders(await api.listReminders(365));
      } catch (e) {
        setError(e instanceof Error ? e.message : '加载失败');
      } finally {
        setLoading(false);
      }
    })();
  }, []);

  const deathList = reminders.filter((r) => r.reminder_type === 'death_anniversary');
  const festivalList = reminders.filter((r) => r.reminder_type === 'festival');

  return (
    <div className="page-container">
      <h1>祭祀提醒</h1>
      <p className="page-sub">按农历推算的忌日与重要祭扫节日</p>

      {loading && <p className="empty-tip">加载中...</p>}
      {error && <p className="empty-tip error-text">加载失败：{error}</p>}
      {!loading && !error && reminders.length === 0 && (
        <p className="empty-tip">未来一年内暂无忌日或节日提醒。可在人物信息中填写安葬日期。</p>
      )}

      {deathList.length > 0 && (
        <section className="reminder-section">
          <h2 className="reminder-section-title">逝者忌日</h2>
          <div className="reminder-list">
            {deathList.map((r) => (
              <div
                key={r.id}
                className="card reminder-card"
                onClick={() => r.tomb_id && navigate(`/tomb/${r.tomb_id}`)}
                role="button"
              >
                <div className="reminder-main">
                  <div className="card-title">{r.title}</div>
                  <div className="card-sub">{r.lunar_date}</div>
                  <div className="card-desc">{r.date}</div>
                </div>
                <span className={badgeClass(r.reminder_type)}>{daysLabel(r.days_until)}</span>
              </div>
            ))}
          </div>
        </section>
      )}

      {festivalList.length > 0 && (
        <section className="reminder-section">
          <h2 className="reminder-section-title">传统节日</h2>
          <div className="reminder-list">
            {festivalList.map((r) => (
              <div key={r.id} className="card reminder-card">
                <div className="reminder-main">
                  <div className="card-title">{r.title}</div>
                  <div className="card-sub">{r.lunar_date}</div>
                  <div className="card-desc">{r.date}</div>
                </div>
                <span className={badgeClass(r.reminder_type)}>{daysLabel(r.days_until)}</span>
              </div>
            ))}
          </div>
        </section>
      )}
    </div>
  );
}
