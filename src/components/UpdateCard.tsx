import { useUpdate } from '../lib/useUpdate';
import Button from './ui/Button';
import Icon from './ui/Icon';

/** Sidebar card shown when a newer uttr is available, and while it downloads and installs. */
export default function UpdateCard() {
  const u = useUpdate();
  if (!u.info || !['available', 'downloading', 'installing'].includes(u.phase)) return null;
  const busy = u.phase !== 'available';
  return (
    <section className="update-card" aria-label="Update available" aria-live="polite">
      <div className="update-card-head">
        <span className="update-card-icon" aria-hidden="true"><Icon name="download" size={14} /></span>
        <span className="update-card-text">
          <strong>{busy ? (u.phase === 'installing' ? 'Installing…' : 'Downloading…') : 'Update available'}</strong>
          <span>Version {u.info.version}</span>
        </span>
      </div>
      {busy ? (
        <div className="pb" role="progressbar" aria-label="Update progress" aria-valuemin={0} aria-valuemax={100} aria-valuenow={u.progress === null ? undefined : Math.round(u.progress * 100)}>
          <div className={`pb-fill${u.progress === null ? ' pb-indeterminate' : ''}`} style={{ width: u.progress === null ? '35%' : `${u.progress * 100}%` }} />
        </div>
      ) : (
        <Button variant="primary" onClick={u.install}>Update and restart</Button>
      )}
      {u.error && <p className="update-card-error" role="alert">{u.error}</p>}
    </section>
  );
}
