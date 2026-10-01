import { AlertCircle } from 'lucide-react';

/** Instrument-style calibration loader; spins only during real requests. */
function Calibrating() {
  return (
    <span className="calibrating" aria-hidden="true">
      <svg viewBox="0 0 40 40" fill="none">
        <circle
          className="calibrating-outer"
          cx="20"
          cy="20"
          r="16"
          pathLength="100"
          stroke="#537158"
          strokeWidth="2"
          strokeDasharray="26 74"
        />
        <circle
          className="calibrating-inner"
          cx="20"
          cy="20"
          r="9.5"
          pathLength="100"
          stroke="#a9b97f"
          strokeWidth="2"
          strokeDasharray="18 82"
        />
        <circle cx="20" cy="20" r="2.4" fill="#537158" />
      </svg>
    </span>
  );
}

export function Feedback({
  loading,
  error,
  empty,
}: {
  loading: boolean;
  error: string | null;
  empty: string | null;
}) {
  if (loading)
    return (
      <div className="feedback" role="status">
        <Calibrating />
        <h3>正在读取本地资料</h3>
        <p>等待工作区响应，请稍候。</p>
      </div>
    );
  if (error)
    return (
      <div className="feedback error" role="alert">
        <AlertCircle size={24} />
        <h3>资料读取失败</h3>
        <p>{error}</p>
        <p>请确认本地服务正常运行，然后点击「刷新数据」重试。</p>
      </div>
    );
  if (empty)
    return (
      <div className="feedback">
        <span className="empty-symbol">∅</span>
        <h3>{empty}</h3>
        <p>这里仅展示工作区的真实数据。</p>
      </div>
    );
  return null;
}
