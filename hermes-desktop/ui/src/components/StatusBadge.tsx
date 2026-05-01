import './StatusBadge.css';

type Status = 'running' | 'stopped' | 'error' | 'loading';

interface StatusBadgeProps {
  status: Status;
  label: string;
}

const statusConfig: Record<Status, { color: string; text: string }> = {
  running: { color: '#22c55e', text: 'Running' },
  stopped: { color: '#94a3b8', text: 'Stopped' },
  error: { color: '#ef4444', text: 'Error' },
  loading: { color: '#f59e0b', text: 'Loading' },
};

export function StatusBadge({ status, label }: StatusBadgeProps) {
  const config = statusConfig[status];

  return (
    <div className="status-badge">
      <span
        className="status-dot"
        style={{ backgroundColor: config.color }}
      />
      <span className="status-label">{label}</span>
      <span className="status-text" style={{ color: config.color }}>
        {config.text}
      </span>
    </div>
  );
}
