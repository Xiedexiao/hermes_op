import { useEffect, useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { notificationsList, type NotificationItem } from '../lib/tauri';
import { useMissionStore } from '../store/missionStore';
import './NotificationsPage.css';

const allKinds = 'all';

export function NotificationsPage() {
  const navigate = useNavigate();
  const selectMission = useMissionStore((state) => state.selectMission);
  const [items, setItems] = useState<NotificationItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [kindFilter, setKindFilter] = useState<string>(allKinds);

  useEffect(() => {
    void loadNotifications();
  }, []);

  const kinds = useMemo(
    () => Array.from(new Set(items.map((item) => item.kind))).sort(),
    [items],
  );
  const filteredItems = useMemo(
    () => items.filter((item) => kindFilter === allKinds || item.kind === kindFilter),
    [items, kindFilter],
  );

  async function loadNotifications() {
    setLoading(true);
    setError(null);
    try {
      setItems(await notificationsList());
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }

  return (
    <div className="notifications-page">
      <div className="notifications-page-header">
        <div>
          <h2>Notifications</h2>
          <p>集中查看待审批、失败 run 和已完成 run 的提醒，并直接跳回对应工作面。</p>
        </div>
        <div className="notifications-page-actions">
          <select value={kindFilter} onChange={(event) => setKindFilter(event.target.value)}>
            <option value={allKinds}>All kinds</option>
            {kinds.map((kind) => (
              <option key={kind} value={kind}>
                {kind}
              </option>
            ))}
          </select>
          <button type="button" className="notifications-button" onClick={() => void loadNotifications()}>
            刷新
          </button>
        </div>
      </div>

      {error ? <div className="notifications-banner notifications-banner-error">{error}</div> : null}
      {loading ? <div className="notifications-card">加载通知中...</div> : null}

      {!loading && filteredItems.length === 0 ? (
        <div className="notifications-card notifications-empty">当前没有匹配的通知。</div>
      ) : null}

      {!loading && filteredItems.length > 0 ? (
        <div className="notifications-list">
          {filteredItems.map((item) => (
            <article className="notifications-card" key={item.id}>
              <div className="notifications-item-header">
                <div>
                  <div className="notifications-kind">{item.kind}</div>
                  <h3>{item.title}</h3>
                </div>
                <time>{item.created_at}</time>
              </div>
              <p className="notifications-message">{item.message}</p>
              <div className="notifications-meta">
                <span>route: {item.route}</span>
                <span>mission: {item.mission_id ?? '-'}</span>
              </div>
              <div className="notifications-item-actions">
                <button
                  type="button"
                  className="notifications-button notifications-button-primary"
                  onClick={() => {
                    if (item.mission_id) {
                      selectMission(item.mission_id);
                    }
                    navigate(item.route);
                  }}
                >
                  打开工作面
                </button>
              </div>
            </article>
          ))}
        </div>
      ) : null}
    </div>
  );
}
