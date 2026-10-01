import { useEffect, useRef } from 'react';
import { categoryLabels } from '../components/CategoryFilter';
import { ArrowUpRight, ArrowRight } from 'lucide-react';
import { formatCount, percent, type Status } from '../api';

/**
 * Gyroscope drive: pointer position -> lerped CSS variables.
 * Only runs on fine pointers with motion allowed; eases back to level otherwise.
 */
function useGyroscope() {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const stage = ref.current;
    if (!stage) return;
    if (!window.matchMedia('(hover: hover) and (pointer: fine)').matches)
      return;
    let raf = 0;
    const target = { x: 0, y: 0 };
    /* Three inertia channels: fast (frame/readout), mid (calibration ring),
       slow (trajectory/inner gimbal) — layers settle at different times,
       which reads as mechanical assembly rather than one rigid block. */
    const fast = { x: 0, y: 0 };
    const mid = { x: 0, y: 0 };
    const slow = { x: 0, y: 0 };
    let active = false;
    const reduced = () =>
      (stage.closest('.app-shell') as HTMLElement | null)?.dataset.motion ===
        'reduced' ||
      window.matchMedia('(prefers-reduced-motion: reduce)').matches;
    const apply = () => {
      stage.style.setProperty('--ax', fast.x.toFixed(4));
      stage.style.setProperty('--ay', fast.y.toFixed(4));
      stage.style.setProperty('--bx', mid.x.toFixed(4));
      stage.style.setProperty('--by', mid.y.toFixed(4));
      stage.style.setProperty('--cx', slow.x.toFixed(4));
      stage.style.setProperty('--cy', slow.y.toFixed(4));
      stage.style.setProperty('--ga', Math.hypot(fast.x, fast.y).toFixed(4));
    };
    const settled = (c: { x: number; y: number }) =>
      Math.abs(target.x - c.x) < 0.0005 && Math.abs(target.y - c.y) < 0.0005;
    const tick = () => {
      fast.x += (target.x - fast.x) * 0.16;
      fast.y += (target.y - fast.y) * 0.16;
      mid.x += (target.x - mid.x) * 0.095;
      mid.y += (target.y - mid.y) * 0.095;
      slow.x += (target.x - slow.x) * 0.055;
      slow.y += (target.y - slow.y) * 0.055;
      apply();
      if (settled(fast) && settled(mid) && settled(slow)) {
        fast.x = mid.x = slow.x = target.x;
        fast.y = mid.y = slow.y = target.y;
        apply();
        active = false;
        return;
      }
      raf = requestAnimationFrame(tick);
    };
    const wake = () => {
      if (!active) {
        active = true;
        raf = requestAnimationFrame(tick);
      }
    };
    const onMove = (event: Event) => {
      const pointer = event as PointerEvent;
      if (reduced()) {
        target.x = 0;
        target.y = 0;
      } else {
        const rect = stage.getBoundingClientRect();
        const nx = ((pointer.clientX - rect.left) / rect.width) * 2 - 1;
        const ny = ((pointer.clientY - rect.top) / rect.height) * 2 - 1;
        target.x = Math.max(-1, Math.min(1, nx));
        target.y = Math.max(-1, Math.min(1, ny));
      }
      wake();
    };
    const onLeave = () => {
      target.x = 0;
      target.y = 0;
      wake();
    };
    /* Track the pointer across the whole window: the instrument is an eye
       that keeps facing the cursor wherever it goes; only leaving the
       window (or reduced motion) eases it back to level. */
    window.addEventListener('pointermove', onMove, { passive: true });
    document.addEventListener('pointerleave', onLeave);
    return () => {
      cancelAnimationFrame(raf);
      window.removeEventListener('pointermove', onMove);
      document.removeEventListener('pointerleave', onLeave);
    };
  }, []);
  return ref;
}

/** Original progress instrument, not a reproduction of a faction trademark. */
function ProgressInstrument({ value }: { value: number }) {
  const stageRef = useGyroscope();
  const ticks = Array.from({ length: 72 }, (_, index) => {
    const major = index % 6 === 0;
    const angle = (index / 72) * 360;
    const outer = 178;
    const inner = major ? 167 : 172.5;
    const rad = ((angle - 90) * Math.PI) / 180;
    return {
      key: index,
      major,
      x1: 220 + Math.cos(rad) * inner,
      y1: 190 + Math.sin(rad) * inner,
      x2: 220 + Math.cos(rad) * outer,
      y2: 190 + Math.sin(rad) * outer,
    };
  });
  const labels = [25, 50, 75, 100].map((label) => {
    const rad = ((label * 3.6 - 90) * Math.PI) / 180;
    return {
      label,
      x: 220 + Math.cos(rad) * 149,
      y: 190 + Math.sin(rad) * 149,
    };
  });
  return (
    <div className="instrument" aria-label={`翻译完成率 ${value.toFixed(2)}%`}>
      <div className="instrument-stage" ref={stageRef}>
        <span className="instrument-shadow" aria-hidden="true" />
        {/* eye socket: tick frame follows the pointer most strongly */}
        <svg
          className="gimbal gimbal-frame"
          viewBox="0 0 440 380"
          fill="none"
          aria-hidden="true"
        >
          <path
            className="instrument-crosshair"
            d="M220 0V380M0 190H440"
            stroke="currentColor"
            opacity=".17"
          />
          <g className="instrument-ticks dial-spin-slow">
            {ticks.map((tick) => (
              <line
                key={tick.key}
                x1={tick.x1}
                y1={tick.y1}
                x2={tick.x2}
                y2={tick.y2}
                stroke="currentColor"
                strokeWidth={tick.major ? 1.5 : 1}
                opacity={tick.major ? 0.55 : 0.28}
              />
            ))}
          </g>
          <g className="instrument-scale">
            {labels.map(({ label, x, y }) => (
              <text
                key={label}
                x={x}
                y={y}
                textAnchor="middle"
                dominantBaseline="central"
              >
                {label}
              </text>
            ))}
          </g>
          <circle
            cx="220"
            cy="190"
            r="151"
            stroke="currentColor"
            opacity=".15"
          />
          <path
            className="corner-mark"
            d="M357 60h14m-7-7v14"
            stroke="currentColor"
          />
          <path
            className="corner-mark corner-mark-alt"
            d="M67 320h14m-7-7v14"
            stroke="currentColor"
          />
        </svg>
        {/* calibration ring: mid-speed channel, slow continuous precession */}
        <svg
          className="gimbal gimbal-calibration"
          viewBox="0 0 440 380"
          fill="none"
          aria-hidden="true"
        >
          <circle
            className="dial-precess"
            cx="220"
            cy="190"
            r="135"
            stroke="currentColor"
            strokeWidth="1"
            strokeDasharray="1 7"
          />
          <circle
            className="instrument-sweep"
            cx="220"
            cy="190"
            r="135"
            pathLength="100"
            strokeDasharray="11 89"
            stroke="#8aa96b"
            strokeWidth="2.5"
            transform="rotate(-90 220 190)"
          />
        </svg>
        {/* orbit plane: slow channel, counter-tilted on its own axis */}
        <svg
          className="gimbal gimbal-trajectory"
          viewBox="0 0 440 380"
          fill="none"
          aria-hidden="true"
        >
          <g className="orbit-precess">
            <ellipse
              className="instrument-trajectory"
              pathLength="1"
              cx="220"
              cy="190"
              rx="192"
              ry="58"
              transform="rotate(-38 220 190)"
              stroke="currentColor"
              opacity=".55"
            />
            <circle
              className="instrument-beacon"
              cx="73"
              cy="292"
              r="5"
              fill="#e7b663"
            />
            <circle
              className="instrument-ping"
              cx="73"
              cy="292"
              r="5"
              stroke="#e7b663"
              strokeWidth="1.5"
            />
          </g>
        </svg>
        {/* inner gimbal ring: resists hardest, stays level like a gyroscope */}
        <svg
          className="gimbal gimbal-ring"
          viewBox="0 0 440 380"
          fill="none"
          aria-hidden="true"
        >
          <circle
            cx="220"
            cy="190"
            r="115"
            stroke="currentColor"
            opacity=".17"
            strokeWidth="16"
          />
          <circle
            className="progress-orbit"
            cx="220"
            cy="190"
            r="115"
            stroke="currentColor"
            strokeWidth="16"
            pathLength="100"
            strokeDasharray={`${value} 100`}
            transform="rotate(-90 220 190)"
          />
        </svg>
        <div className="instrument-value">
          <span className="eyebrow">TRANSLATION COVERAGE</span>
          <strong>
            {value.toFixed(2)}
            <small>%</small>
          </strong>
          <span>翻译完成率</span>
        </div>
      </div>
      <div className="instrument-caption">
        <span>KR → ZH-CN</span>
        <span>语言的边界，不是理解的边界。</span>
      </div>
    </div>
  );
}

export function Overview({ data }: { data: Status }) {
  const stats = { ...data.stats, ...data.task_scope };
  const pending = stats.pending_review_segments;
  const categories: Readonly<Record<string, string>> = categoryLabels;
  return (
    <div className="overview page-enter">
      <div className="feedback" data-testid="task-scope-summary">
        <strong>
          {data.task_scope
            ? `本次任务 · ${data.task_scope.name}`
            : '未设置任务范围 · 当前显示全量库存'}
        </strong>
        <p data-testid="inventory-summary">
          全量原文库存：{formatCount(data.stats.total_segments)} 条 /{' '}
          {formatCount(data.stats.total_files)} 文件；下方进度按
          {data.task_scope ? '本次任务' : '全量库存'}计算。
        </p>
      </div>
      <section className="hero">
        <div className="hero-ambience" aria-hidden="true">
          <span className="scanline" />
          {Array.from({ length: 14 }, (_, index) => (
            <i key={index} />
          ))}
        </div>
        <div className="hero-copy">
          <span className="eyebrow">LOCALIZATION / RESEARCH OVERVIEW</span>
          <h2>
            语言之间，
            <br />
            精确抵达<span className="square-stop">.</span>
          </h2>
          <p>让原文的每一层含义，在另一种语言中完整延续。</p>
          <a
            className="primary-link"
            href="#workbench"
            data-testid="open-workbench"
          >
            进入双语工作台 <ArrowUpRight size={20} />
          </a>
          <div className="hero-foot">
            <span className="spectrum" />
            <span>LCT · 本地化工程工作站</span>
          </div>
        </div>
        <ProgressInstrument
          value={percent(stats.translated_segments, stats.total_segments)}
        />
      </section>
      <section className="metrics" aria-label="项目统计">
        {[
          [
            '01',
            data.task_scope ? '本次任务词条' : '项目词条',
            stats.total_segments,
            'SEGMENTS',
          ],
          ['02', '已翻译', stats.translated_segments, 'TRANSLATED'],
          ['03', '资源文件', stats.total_files, 'FILES'],
          ['04', '翻译记忆', stats.tm_entries, 'TM ENTRIES'],
        ].map(([id, label, value, unit]) => (
          <div className="metric" key={id}>
            <span className="metric-label">
              <small>{id}</small>
              {label}
            </span>
            <strong>{formatCount(Number(value))}</strong>
            <span className="eyebrow">{unit}</span>
          </div>
        ))}
      </section>
      <div className="overview-bottom">
        <section className="ledger">
          <div className="section-title">
            <h3>分域进度</h3>
            <span className="eyebrow">CORPUS DISTRIBUTION</span>
          </div>
          <p className="muted" data-testid="category-count-basis">
            按资源用途分域：RPG
            对话、选项、旁白及任务归入剧情；条数包含正文、名称、称号等字段，不等于句数或字数。
          </p>
          {Object.entries(stats.categories).map(([key, category], index) => (
            <div className="category" key={key} data-testid={`category-${key}`}>
              <span className="category-index">0{index + 1}</span>
              <div>
                <div className="category-heading">
                  <strong>{categories[key] ?? key}</strong>
                  <span>
                    {formatCount(category.translated)}{' '}
                    <small>/ {formatCount(category.total)}</small>
                  </span>
                </div>
                <progress
                  aria-label={`${categories[key] ?? key}翻译进度`}
                  value={category.translated}
                  max={Math.max(1, category.total)}
                />
              </div>
              <span className="category-percent">
                {(
                  Math.floor(
                    percent(category.translated, category.total) * 100,
                  ) / 100
                ).toFixed(2)}
                <small>%</small>
              </span>
            </div>
          ))}
          {Object.keys(stats.categories).length === 0 && (
            <p className="muted">项目尚未导入资源文件。</p>
          )}
        </section>
        <aside className="next-action">
          <div className="section-title">
            <span className="eyebrow">NEXT IN YOUR WORKFLOW</span>
            <ArrowUpRight size={20} />
          </div>
          <h3>等待人工审校</h3>
          <strong className="pending-count">
            {formatCount(pending)}
            <small> 条</small>
          </strong>
          <p>从一个词条开始，推进下一段理解。</p>
          <a href="#focus" data-testid="open-focus">
            打开待审队列 <ArrowRight size={18} />
          </a>
        </aside>
      </div>
      <div className="overview-note">
        <span>
          审校记录 <b>{formatCount(stats.reviewed_segments)}</b> 条
        </span>
        <span>审校数量不代表全项目 QA 通过，质量检查结果以单条记录为准。</span>
      </div>
    </div>
  );
}
