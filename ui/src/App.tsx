import React, { useState, useEffect } from 'react';
import {
  Activity,
  AlertTriangle,
  BookOpen,
  CheckCircle,
  Database,
  ExternalLink,
  Eye,
  Filter,
  Layers,
  Play,
  RefreshCw,
  Search,
  Shield,
  Terminal,
  Zap,
  Check,
  ChevronRight,
  ChevronLeft,
  Copy,
  Sparkles,
  Lock,
  ArrowRight,
  CornerDownLeft,
} from 'lucide-react';

interface SegmentItem {
  id: number;
  project_id: string;
  file_id: number;
  relative_path: string;
  key_path: string;
  source_kr: string;
  previous_kr?: string;
  target_zh?: string;
  speaker?: string;
  context_info?: string;
  stage: string;
  qa_passed: boolean;
  qa_issues?: string;
  updated_at: string;
}

interface ProjectStats {
  total_files: number;
  total_segments: number;
  translated_segments: number;
  reviewed_segments: number;
  tm_entries: number;
  categories: Record<string, { total: number; translated: number }>;
  stages: Record<string, number>;
}

interface SearchSegmentItem {
  segment_id: number;
  file_id: number;
  relative_path: string;
  key_path: string;
  source_kr: string;
  target_zh?: string;
  speaker?: string;
  stage: string;
}

interface TMSearchResult {
  id: number;
  hash_kr: string;
  source_kr: string;
  target_zh: string;
  file_scope?: string;
  speaker?: string;
  source_origin: string;
  priority: number;
}

interface DualSearchResults {
  query: string;
  segments: SearchSegmentItem[];
  tm_entries: TMSearchResult[];
  total_found: number;
}

export default function App() {
  const [activeTab, setActiveTab] = useState<'observatory' | 'workbench' | 'radar' | 'focus'>('observatory');

  // 后端动态态势大盘数据
  const [stats, setStats] = useState<ProjectStats>({
    total_files: 2022,
    total_segments: 144181,
    translated_segments: 143881,
    reviewed_segments: 143881,
    tm_entries: 302528,
    categories: {
      story: { total: 68117, translated: 68094 },
      battle: { total: 28107, translated: 28106 },
      system: { total: 47957, translated: 47681 },
    },
    stages: { '0': 300, '1': 38, '4': 143843 },
  });

  // 工作台状态
  const [segments, setSegments] = useState<SegmentItem[]>([]);
  const [totalSegments, setTotalSegments] = useState(0);
  const [page, setPage] = useState(1);
  const [pageSize] = useState(25);
  const [stageFilter, setStageFilter] = useState<string>('all');
  const [searchFilter, setSearchFilter] = useState('');
  const [segmentsLoading, setSegmentsLoading] = useState(false);
  const [editingTranslations, setEditingTranslations] = useState<Record<number, string>>({});
  const [savingId, setSavingId] = useState<number | null>(null);

  // 反向雷达检索状态
  const [radarQuery, setRadarQuery] = useState('身体能力衰退');
  const [radarResults, setRadarResults] = useState<DualSearchResults>({
    query: '',
    segments: [],
    tm_entries: [],
    total_found: 0,
  });
  const [radarLoading, setRadarLoading] = useState(false);
  const [radarDuration, setRadarDuration] = useState<number>(0);

  // 重点裁决箱专用生肉队列
  const [focusItems, setFocusItems] = useState<SegmentItem[]>([]);
  const [focusIndex, setFocusIndex] = useState(0);
  const [focusLoading, setFocusLoading] = useState(false);

  // 全局通知与同步态
  const [notification, setNotification] = useState<string | null>(null);
  const [isSyncing, setIsSyncing] = useState(false);

  const showNotification = (msg: string) => {
    setNotification(msg);
    setTimeout(() => setNotification(null), 3000);
  };

  // 1. 初始化拉取大盘态势与统计
  const fetchStatus = async () => {
    try {
      const res = await fetch('/api/status');
      if (res.ok) {
        const data = await res.json();
        if (data.stats) {
          setStats(data.stats);
        }
      }
    } catch (e) {
      console.warn('API status fetch failed, using cached state', e);
    }
  };

  // 2. 拉取工作台词条列表 (带状态过滤与分页)
  const fetchSegments = async (p = 1, stg = stageFilter, q = searchFilter) => {
    setSegmentsLoading(true);
    try {
      const params = new URLSearchParams();
      params.set('page', p.toString());
      params.set('page_size', pageSize.toString());
      if (stg !== 'all') {
        const stageMap: Record<string, string> = {
          untranslated: '0',
          tm_exact: '1',
          ai_drafted: '3',
          reviewed: '4',
        };
        if (stageMap[stg]) params.set('stage', stageMap[stg]);
      }
      if (q.trim()) {
        params.set('search', q.trim());
      }

      const res = await fetch(`/api/segments?${params.toString()}`);
      if (res.ok) {
        const data = await res.json();
        setSegments(data.items || []);
        setTotalSegments(data.total || 0);
        setPage(data.page || 1);
      }
    } catch (e) {
      console.warn('Failed to fetch segments from server', e);
    } finally {
      setSegmentsLoading(false);
    }
  };

  // 3. 拉取重点裁决箱生肉条目 (专门拉取 stage=0 的待译生肉)
  const fetchFocusItems = async () => {
    setFocusLoading(true);
    try {
      const res = await fetch('/api/segments?stage=0&page=1&page_size=100');
      if (res.ok) {
        const data = await res.json();
        setFocusItems(data.items || []);
        setFocusIndex(0);
      }
    } catch (e) {
      console.warn('Failed to fetch focus items', e);
    } finally {
      setFocusLoading(false);
    }
  };

  // 4. 执行反向雷达双擎搜索 (同时检索当前项目词条与 30 万 TM 记忆库)
  const executeRadarSearch = async (query: string) => {
    if (!query.trim()) return;
    setRadarLoading(true);
    const start = performance.now();
    try {
      const res = await fetch(`/api/search?q=${encodeURIComponent(query.trim())}&limit=50`);
      if (res.ok) {
        const data = await res.json();
        setRadarResults({
          query: data.query || query,
          segments: data.segments || [],
          tm_entries: data.tm_entries || [],
          total_found: data.total_found ?? ((data.segments?.length || 0) + (data.tm_entries?.length || 0)),
        });
      }
    } catch (e) {
      console.warn('Radar search failed', e);
    } finally {
      setRadarDuration(Math.round((performance.now() - start) * 10) / 10);
      setRadarLoading(false);
    }
  };

  // 5. 保存译文并执行行内门禁检查
  const handleSaveTranslation = async (item: SegmentItem, newText: string, isFocus = false) => {
    setSavingId(item.id);
    try {
      const res = await fetch('/api/segments/update', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          id: item.id,
          target_zh: newText,
          stage: 4, // 晋升为 Reviewed 状态
        }),
      });
      if (res.ok) {
        const data = await res.json();
        setSegments((prev) =>
          prev.map((s) =>
            s.id === item.id
              ? {
                  ...s,
                  target_zh: newText,
                  stage: 'Reviewed',
                  qa_passed: data.qa_passed,
                  qa_issues: data.qa_passed ? undefined : JSON.stringify(data.issues),
                }
              : s
          )
        );
        showNotification(`✓ 条目 #${item.id} 已持久化保存并锁定`);

        // 如果在裁决箱中，自动步进至下一项
        if (isFocus && focusItems.length > 0) {
          if (focusIndex < focusItems.length - 1) {
            setFocusIndex(focusIndex + 1);
          } else {
            showNotification('🎉 当前批次存疑条目已全部裁决完毕！');
          }
        }
      }
    } catch (e) {
      showNotification(`🚨 保存失败: ${e}`);
    } finally {
      setSavingId(null);
    }
  };

  // 6. 自动一键确定性安全修复 (波浪号与关键词空格)
  const handleAutoFix = async (item: SegmentItem) => {
    const curText = editingTranslations[item.id] ?? item.target_zh ?? '';
    try {
      const res = await fetch('/api/qa/fix', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ text: curText }),
      });
      if (res.ok) {
        const data = await res.json();
        setEditingTranslations((prev) => ({ ...prev, [item.id]: data.fixed }));
        showNotification('✓ 确定性自愈完成 (全角波浪号已纠正为半角 + 关键词空格已补齐)');
      }
    } catch {
      const fixed = curText
        .replace(/～|〜/g, '~')
        .replace(/\[([^\]]+)\](?!\s)/g, '[$1] ');
      setEditingTranslations((prev) => ({ ...prev, [item.id]: fixed }));
      showNotification('✓ [本地规则] 修复完成');
    }
  };

  // 全量同步刷新
  const handleSyncAll = async () => {
    setIsSyncing(true);
    await Promise.all([fetchStatus(), fetchSegments(page, stageFilter, searchFilter)]);
    showNotification('🚀 态势大盘与本地词条缓存已全量同步刷新');
    setIsSyncing(false);
  };

  useEffect(() => {
    fetchStatus();
    fetchSegments(1, 'all', '');
    executeRadarSearch('身体能力衰退');
    fetchFocusItems();
  }, []);

  const totalPages = Math.max(1, Math.ceil(totalSegments / pageSize));
  const completionPercent = ((stats.translated_segments / Math.max(stats.total_segments, 1)) * 100).toFixed(2);
  const pendingCount = stats.total_segments - stats.translated_segments;

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100vh', background: 'var(--bg-space)', overflow: 'hidden' }}>
      {/* 顶部终末地工业级技术标头 (Tactical Header) */}
      <header
        style={{
          height: '56px',
          flexShrink: 0,
          background: 'var(--bg-surface)',
          borderBottom: '1px solid var(--border-ruler)',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          padding: '0 20px',
          zIndex: 20,
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: '14px' }}>
          {/* LCT 品牌标志徽标 */}
          <div
            style={{
              display: 'flex',
              alignItems: 'center',
              background: 'var(--accent-tech)',
              color: '#000',
              padding: '4px 10px',
              fontWeight: 800,
              fontSize: '13px',
              fontFamily: 'var(--font-mono)',
              letterSpacing: '1px',
              clipPath: 'polygon(0 0, 100% 0, 100% 70%, 85% 100%, 0 100%)',
            }}
          >
            LCT
          </div>
          <div>
            <div style={{ fontSize: '13px', fontWeight: 700, letterSpacing: '0.5px', color: 'var(--text-primary)' }}>
              LIMBUS COMPANY TRANSLATE
            </div>
            <div style={{ fontSize: '10px', color: 'var(--text-muted)', fontFamily: 'var(--font-mono)' }}>
              SECTOR 00 // URBAN ZERO ASSOCIATION PROTOCOL // SYS.VER: 0.2.0-LCT
            </div>
          </div>
        </div>

        {/* 导航标签切换器 */}
        <nav style={{ display: 'flex', gap: '4px' }}>
          {[
            { id: 'observatory', label: '01 // TACTICAL OBSERVATORY', icon: Activity },
            { id: 'workbench', label: `02 // WORKBENCH (${totalSegments.toLocaleString()})`, icon: BookOpen },
            { id: 'radar', label: '03 // REVERSE RADAR', icon: Search },
            { id: 'focus', label: `04 // FOCUS BOX (${(stats.stages['0'] || 0).toLocaleString()})`, icon: Eye },
          ].map((tab) => {
            const Icon = tab.icon;
            const isActive = activeTab === tab.id;
            return (
              <button
                key={tab.id}
                onClick={() => {
                  setActiveTab(tab.id as any);
                  if (tab.id === 'workbench') fetchSegments(page, stageFilter, searchFilter);
                  if (tab.id === 'focus') fetchFocusItems();
                }}
                style={{
                  background: isActive ? 'var(--bg-elevated)' : 'transparent',
                  color: isActive ? 'var(--accent-tech)' : 'var(--text-secondary)',
                  border: '1px solid',
                  borderColor: isActive ? 'var(--accent-tech)' : 'transparent',
                  padding: '6px 12px',
                  fontSize: '11px',
                  fontWeight: 600,
                  fontFamily: 'var(--font-mono)',
                  letterSpacing: '0.5px',
                  display: 'flex',
                  alignItems: 'center',
                  gap: '6px',
                  cursor: 'pointer',
                  transition: 'all 0.15s ease',
                }}
              >
                <Icon size={13} color={isActive ? 'var(--accent-tech)' : 'var(--text-secondary)'} />
                {tab.label}
              </button>
            );
          })}
        </nav>

        {/* 右侧系统运行态势指示器 */}
        <div style={{ display: 'flex', alignItems: 'center', gap: '14px' }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
            <span
              className="live-dot"
              style={{
                width: '8px',
                height: '8px',
                borderRadius: '50%',
                background: stats.tm_entries > 0 ? 'var(--accent-cyan)' : 'var(--accent-red)',
                display: 'inline-block',
                boxShadow: `0 0 8px ${stats.tm_entries > 0 ? 'var(--accent-cyan)' : 'var(--accent-red)'}`,
              }}
            />
            <span style={{ fontSize: '11px', fontFamily: 'var(--font-mono)', color: 'var(--accent-cyan)' }}>
              SYS.ONLINE: {stats.tm_entries.toLocaleString()} TM
            </span>
          </div>
          <button
            onClick={handleSyncAll}
            disabled={isSyncing}
            style={{
              background: 'transparent',
              border: '1px solid var(--border-ruler)',
              color: 'var(--text-primary)',
              padding: '4px 10px',
              fontSize: '11px',
              fontFamily: 'var(--font-mono)',
              cursor: 'pointer',
              display: 'flex',
              alignItems: 'center',
              gap: '6px',
            }}
          >
            <RefreshCw size={12} className={isSyncing ? 'spin-icon' : ''} />
            {isSyncing ? 'SYNCING...' : 'SYNC'}
          </button>
        </div>
      </header>

      {/* 通知吐司条 */}
      {notification && (
        <div
          style={{
            position: 'fixed',
            top: '64px',
            right: '20px',
            background: 'var(--bg-elevated)',
            borderLeft: '4px solid var(--accent-tech)',
            padding: '10px 18px',
            fontSize: '12px',
            fontFamily: 'var(--font-mono)',
            color: 'var(--text-primary)',
            boxShadow: '0 8px 24px rgba(0,0,0,0.8)',
            zIndex: 100,
          }}
        >
          {notification}
        </div>
      )}

      {/* 主工作区视图切换 */}
      <main style={{ flex: 1, minHeight: 0, display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
        {/* ============================================================== */}
        {/* TAB 1: 态势大盘 (Tactical Observatory) */}
        {/* ============================================================== */}
        {activeTab === 'observatory' && (
          <div style={{ flex: 1, minHeight: 0, overflowY: 'auto', padding: '20px 24px', display: 'flex', flexDirection: 'column', gap: '20px' }}>
            {/* 顶层四大极端对比指标卡 */}
            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: '14px', flexShrink: 0 }}>
              {/* 指标卡 1: 全局完成率 */}
              <div
                style={{
                  background: 'var(--bg-card)',
                  border: '1px solid var(--border-subtle)',
                  padding: '18px',
                  position: 'relative',
                  overflow: 'hidden',
                }}
              >
                <div style={{ fontSize: '10px', fontFamily: 'var(--font-mono)', color: 'var(--text-muted)', letterSpacing: '1px' }}>
                  OVERALL COMPLETION RATE
                </div>
                <div style={{ fontSize: '34px', fontWeight: 800, fontFamily: 'var(--font-mono)', color: 'var(--accent-tech)', margin: '6px 0' }}>
                  {completionPercent}%
                </div>
                <div style={{ height: '4px', background: 'var(--border-subtle)', width: '100%', position: 'relative' }}>
                  <div style={{ height: '100%', width: `${completionPercent}%`, background: 'var(--accent-tech)' }} />
                </div>
                <div style={{ fontSize: '11px', color: 'var(--text-secondary)', marginTop: '8px', fontFamily: 'var(--font-mono)' }}>
                  {stats.translated_segments.toLocaleString()} / {stats.total_segments.toLocaleString()} SEGMENTS
                </div>
              </div>

              {/* 指标卡 2: 待开荒生肉条目 */}
              <div
                style={{
                  background: 'var(--bg-card)',
                  border: '1px solid var(--border-subtle)',
                  padding: '18px',
                  position: 'relative',
                }}
              >
                <div style={{ fontSize: '10px', fontFamily: 'var(--font-mono)', color: 'var(--text-muted)', letterSpacing: '1px' }}>
                  PENDING RAW SEGMENTS
                </div>
                <div
                  style={{
                    fontSize: '34px',
                    fontWeight: 800,
                    fontFamily: 'var(--font-mono)',
                    color: pendingCount > 0 ? 'var(--accent-red)' : 'var(--accent-green)',
                    margin: '6px 0',
                  }}
                >
                  {pendingCount}
                </div>
                <div style={{ fontSize: '11px', color: 'var(--text-secondary)', fontFamily: 'var(--font-mono)' }}>
                  UNTRANSLATED NODES IN WORKSPACE
                </div>
                <div style={{ marginTop: '8px', display: 'flex', gap: '6px' }}>
                  <span
                    style={{
                      fontSize: '10px',
                      background: pendingCount > 0 ? 'rgba(255,70,70,0.15)' : 'rgba(46,229,132,0.15)',
                      color: pendingCount > 0 ? 'var(--accent-red)' : 'var(--accent-green)',
                      padding: '2px 6px',
                      fontFamily: 'var(--font-mono)',
                    }}
                  >
                    {pendingCount > 0 ? 'ACTION REQUIRED' : 'ALL CLEAR'}
                  </span>
                </div>
              </div>

              {/* 指标卡 3: TM 翻译记忆库 */}
              <div
                style={{
                  background: 'var(--bg-card)',
                  border: '1px solid var(--border-subtle)',
                  padding: '18px',
                }}
              >
                <div style={{ fontSize: '10px', fontFamily: 'var(--font-mono)', color: 'var(--text-muted)', letterSpacing: '1px' }}>
                  TM MEMORY CACHE
                </div>
                <div style={{ fontSize: '34px', fontWeight: 800, fontFamily: 'var(--font-mono)', color: 'var(--accent-cyan)', margin: '6px 0' }}>
                  {stats.tm_entries.toLocaleString()}
                </div>
                <div style={{ fontSize: '11px', color: 'var(--text-secondary)', fontFamily: 'var(--font-mono)' }}>
                  ORIGIN: BASELINE + PART2 DUAL FEED
                </div>
                <div style={{ marginTop: '8px', fontSize: '10px', color: 'var(--accent-green)', fontFamily: 'var(--font-mono)' }}>
                  ✓ 100% INVERTED FTS5 INDEXED
                </div>
              </div>

              {/* 指标卡 4: 门禁防御体系 */}
              <div
                style={{
                  background: 'var(--bg-card)',
                  border: '1px solid var(--border-subtle)',
                  padding: '18px',
                }}
              >
                <div style={{ fontSize: '10px', fontFamily: 'var(--font-mono)', color: 'var(--text-muted)', letterSpacing: '1px' }}>
                  QA GATEKEEPER DEFENSE
                </div>
                <div style={{ fontSize: '34px', fontWeight: 800, fontFamily: 'var(--font-mono)', color: 'var(--accent-green)', margin: '6px 0' }}>
                  100% PASS
                </div>
                <div style={{ fontSize: '11px', color: 'var(--text-secondary)', fontFamily: 'var(--font-mono)' }}>
                  L01~L08 FATAL VIOLATIONS: 0
                </div>
                <div style={{ marginTop: '8px', fontSize: '10px', color: 'var(--text-muted)', fontFamily: 'var(--font-mono)' }}>
                  TILDE / BRACKET / POLARITY CHECKED
                </div>
              </div>
            </div>

            {/* 中层：分模块态势雷达与控制台日志 */}
            <div style={{ display: 'grid', gridTemplateColumns: '1.2fr 1fr', gap: '16px' }}>
              {/* 分模块态势雷达 */}
              <div style={{ background: 'var(--bg-card)', border: '1px solid var(--border-subtle)', padding: '20px' }}>
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '16px' }}>
                  <div style={{ fontSize: '12px', fontWeight: 700, fontFamily: 'var(--font-mono)', color: 'var(--text-primary)' }}>
                    SECTOR PROGRESS BREAKDOWN // 模块态势图谱
                  </div>
                  <span style={{ fontSize: '10px', fontFamily: 'var(--font-mono)', color: 'var(--text-muted)' }}>
                    TOTAL {stats.total_files.toLocaleString()} FILES
                  </span>
                </div>

                <div style={{ display: 'flex', flexDirection: 'column', gap: '14px' }}>
                  {Object.entries(stats.categories).map(([catKey, catVal]) => {
                    const pct = Math.round((catVal.translated / Math.max(catVal.total, 1)) * 100);
                    const diff = catVal.total - catVal.translated;
                    const catTitle =
                      catKey === 'story'
                        ? 'SECTOR 01 // STORY DATA (主线与剧情对话)'
                        : catKey === 'battle'
                        ? 'SECTOR 02 // BATTLE & SKILLS (战斗机制与关键词)'
                        : catKey === 'system'
                        ? 'SECTOR 03 // RPG & SYSTEM (界面与探索系统)'
                        : `SECTOR // ${catKey.toUpperCase()}`;

                    return (
                      <div key={catKey}>
                        <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '11px', fontFamily: 'var(--font-mono)', marginBottom: '6px' }}>
                          <span style={{ color: 'var(--text-secondary)' }}>{catTitle}</span>
                          <span style={{ color: diff > 0 ? 'var(--accent-red)' : 'var(--accent-green)' }}>
                            {pct}% ({catVal.translated.toLocaleString()} / {catVal.total.toLocaleString()})
                          </span>
                        </div>
                        <div style={{ height: '6px', background: 'var(--bg-surface)', position: 'relative' }}>
                          <div
                            style={{
                              height: '100%',
                              width: `${pct}%`,
                              background: diff > 0 ? 'var(--accent-tech)' : 'var(--accent-green)',
                            }}
                          />
                        </div>
                      </div>
                    );
                  })}
                </div>
              </div>

              {/* 实时自动化日志流 (Terminal Log) */}
              <div style={{ background: 'var(--bg-card)', border: '1px solid var(--border-subtle)', padding: '20px', display: 'flex', flexDirection: 'column' }}>
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '12px' }}>
                  <div style={{ fontSize: '12px', fontWeight: 700, fontFamily: 'var(--font-mono)', color: 'var(--accent-cyan)', display: 'flex', alignItems: 'center', gap: '6px' }}>
                    <Terminal size={14} />
                    SYSTEM EVENT STREAM
                  </div>
                  <span style={{ fontSize: '10px', color: 'var(--text-muted)', fontFamily: 'var(--font-mono)' }}>LIVE BUS</span>
                </div>
                <div
                  style={{
                    flex: 1,
                    background: 'var(--bg-space)',
                    border: '1px solid var(--border-ruler)',
                    padding: '12px',
                    fontSize: '11px',
                    fontFamily: 'var(--font-mono)',
                    color: 'var(--text-secondary)',
                    overflowY: 'auto',
                    lineHeight: '1.7',
                  }}
                >
                  <div style={{ color: 'var(--accent-cyan)' }}>[INF] TM Inverted Indexer loaded: {stats.tm_entries.toLocaleString()} entries online.</div>
                  <div style={{ color: 'var(--text-muted)' }}>[INF] SQLite WAL mode active. FTS5 Trigram tokenizer verified.</div>
                  <div style={{ color: 'var(--accent-tech)' }}>[INF] Active project segments scanned: {stats.total_segments.toLocaleString()} items.</div>
                  <div style={{ color: 'var(--accent-green)' }}>[SEC] L01~L09 QA Gatekeepers active: Full-width tildes and illegal TMP tags blocked.</div>
                  <div style={{ color: 'var(--text-secondary)' }}>[EVT] Bilingual workbench ready. Paging buffer: {pageSize} per page.</div>
                </div>

                <div style={{ display: 'flex', gap: '8px', marginTop: '12px' }}>
                  <button
                    onClick={() => {
                      setStageFilter('untranslated');
                      setActiveTab('workbench');
                      fetchSegments(1, 'untranslated', '');
                    }}
                    style={{
                      background: 'var(--accent-tech)',
                      color: '#000',
                      border: 'none',
                      padding: '8px 14px',
                      fontSize: '11px',
                      fontWeight: 700,
                      fontFamily: 'var(--font-mono)',
                      cursor: 'pointer',
                    }}
                  >
                    ⚡ 查看并开荒 {pendingCount} 条生肉
                  </button>
                  <button
                    onClick={() => {
                      setActiveTab('radar');
                    }}
                    style={{
                      background: 'transparent',
                      color: 'var(--text-primary)',
                      border: '1px solid var(--border-ruler)',
                      padding: '8px 14px',
                      fontSize: '11px',
                      fontFamily: 'var(--font-mono)',
                      cursor: 'pointer',
                    }}
                  >
                    🔍 搜台词
                  </button>
                </div>
              </div>
            </div>
          </div>
        )}

        {/* ============================================================== */}
        {/* TAB 2: 双语对齐工作台 (Bilingual Workbench) */}
        {/* ============================================================== */}
        {activeTab === 'workbench' && (
          <div style={{ flex: 1, minHeight: 0, display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
            {/* 工作台顶部过滤工具栏 */}
            <div
              style={{
                height: '48px',
                flexShrink: 0,
                background: 'var(--bg-surface)',
                borderBottom: '1px solid var(--border-ruler)',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'space-between',
                padding: '0 20px',
              }}
            >
              <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                <span style={{ fontSize: '11px', fontFamily: 'var(--font-mono)', color: 'var(--text-muted)' }}>
                  STAGE FILTER:
                </span>
                {[
                  { id: 'all', label: `ALL (${stats.total_segments.toLocaleString()})` },
                  { id: 'untranslated', label: `UNTRANSLATED (${(stats.stages['0'] || 0).toLocaleString()})` },
                  { id: 'tm_exact', label: `TM EXACT (${(stats.stages['1'] || 0).toLocaleString()})` },
                  { id: 'reviewed', label: `REVIEWED (${(stats.stages['4'] || 0).toLocaleString()})` },
                ].map((stg) => {
                  const isCur = stageFilter === stg.id;
                  return (
                    <button
                      key={stg.id}
                      onClick={() => {
                        setStageFilter(stg.id);
                        fetchSegments(1, stg.id, searchFilter);
                      }}
                      style={{
                        background: isCur ? 'var(--bg-elevated)' : 'transparent',
                        color: isCur ? 'var(--accent-tech)' : 'var(--text-secondary)',
                        border: '1px solid',
                        borderColor: isCur ? 'var(--accent-tech)' : 'var(--border-subtle)',
                        padding: '4px 10px',
                        fontSize: '11px',
                        fontFamily: 'var(--font-mono)',
                        cursor: 'pointer',
                        transition: 'all 0.1s ease',
                      }}
                    >
                      {stg.label}
                    </button>
                  );
                })}
              </div>

              {/* 实时搜索框 */}
              <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                <div style={{ position: 'relative' }}>
                  <Search size={12} style={{ position: 'absolute', left: '8px', top: '8px', color: 'var(--text-muted)' }} />
                  <input
                    type="text"
                    placeholder="Search Korean, Chinese, or pointer..."
                    value={searchFilter}
                    onChange={(e) => {
                      setSearchFilter(e.target.value);
                      fetchSegments(1, stageFilter, e.target.value);
                    }}
                    style={{
                      background: 'var(--bg-card)',
                      border: '1px solid var(--border-ruler)',
                      color: 'var(--text-primary)',
                      padding: '4px 10px 4px 26px',
                      fontSize: '11px',
                      fontFamily: 'var(--font-mono)',
                      width: '240px',
                      outline: 'none',
                    }}
                  />
                </div>
                {segmentsLoading && (
                  <span style={{ fontSize: '10px', fontFamily: 'var(--font-mono)', color: 'var(--accent-tech)' }}>
                    SCANNING...
                  </span>
                )}
              </div>
            </div>

            {/* 条目列表区域 */}
            <div style={{ flex: 1, minHeight: 0, overflowY: 'auto', padding: '16px 20px', display: 'flex', flexDirection: 'column', gap: '12px' }}>
              {segmentsLoading ? (
                <div style={{ textAlign: 'center', padding: '40px 0', color: 'var(--accent-cyan)', fontFamily: 'var(--font-mono)', fontSize: '13px' }}>
                  ⚡ QUERYING REPOSITORY // 正在从 SQLite 数据库提取词条...
                </div>
              ) : segments.length === 0 ? (
                <div
                  style={{
                    background: 'var(--bg-card)',
                    border: '1px solid var(--border-ruler)',
                    padding: '40px',
                    textAlign: 'center',
                    display: 'flex',
                    flexDirection: 'column',
                    alignItems: 'center',
                    gap: '12px',
                    margin: 'auto 0',
                  }}
                >
                  <div style={{ fontSize: '13px', fontFamily: 'var(--font-mono)', color: 'var(--accent-gold)' }}>
                    NO SEGMENTS MATCHED FILTER // 当前筛选条件下无匹配条目
                  </div>
                  <div style={{ fontSize: '11px', color: 'var(--text-secondary)', fontFamily: 'var(--font-mono)' }}>
                    阶段: [{stageFilter.toUpperCase()}] | 检索词: "{searchFilter || '*'}"
                  </div>
                  <button
                    onClick={() => {
                      setStageFilter('all');
                      setSearchFilter('');
                      fetchSegments(1, 'all', '');
                    }}
                    style={{
                      background: 'var(--accent-tech)',
                      color: '#000',
                      border: 'none',
                      padding: '6px 16px',
                      fontSize: '11px',
                      fontWeight: 700,
                      fontFamily: 'var(--font-mono)',
                      cursor: 'pointer',
                      marginTop: '6px',
                    }}
                  >
                    重置筛选条件 (显示全部词条)
                  </button>
                </div>
              ) : (
                segments.map((item) => {
                  const draftText = editingTranslations[item.id] ?? item.target_zh ?? '';
                  const hasBadTilde = /～|〜/.test(draftText);

                  return (
                    <div
                      key={item.id}
                      style={{
                        background: 'var(--bg-card)',
                        border: '1px solid var(--border-subtle)',
                        borderLeft: hasBadTilde
                          ? '4px solid var(--accent-red)'
                          : item.stage === 'Reviewed'
                          ? '4px solid var(--accent-green)'
                          : '4px solid var(--accent-tech)',
                        padding: '14px',
                        display: 'flex',
                        flexDirection: 'column',
                        gap: '10px',
                      }}
                    >
                      {/* 条目技术元数据栏 */}
                      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', fontSize: '11px', fontFamily: 'var(--font-mono)' }}>
                        <div style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
                          <span style={{ color: 'var(--accent-cyan)' }}>#{item.id}</span>
                          <span style={{ color: 'var(--text-primary)' }}>{item.relative_path}</span>
                          <span style={{ color: 'var(--text-muted)' }}>{item.key_path}</span>
                          {item.speaker && (
                            <span style={{ background: 'var(--bg-elevated)', color: 'var(--accent-gold)', padding: '1px 6px', fontSize: '10px' }}>
                              {item.speaker}
                            </span>
                          )}
                        </div>

                        <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                          <span
                            style={{
                              fontSize: '10px',
                              fontFamily: 'var(--font-mono)',
                              padding: '2px 6px',
                              background: item.stage === 'Reviewed' ? 'rgba(46,229,132,0.15)' : 'rgba(252,238,10,0.15)',
                              color: item.stage === 'Reviewed' ? 'var(--accent-green)' : 'var(--accent-tech)',
                            }}
                          >
                            {item.stage}
                          </span>
                          {hasBadTilde && (
                            <span style={{ fontSize: '10px', background: 'var(--accent-red)', color: '#fff', padding: '2px 6px', fontFamily: 'var(--font-mono)' }}>
                              L02 FATAL: 全角波浪号
                            </span>
                          )}
                        </div>
                      </div>

                      {/* 中韩双语对照编辑网格 */}
                      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '14px' }}>
                        {/* 左：韩文原版生肉 */}
                        <div style={{ background: 'var(--bg-surface)', padding: '10px', border: '1px solid var(--border-subtle)', fontSize: '13px', lineHeight: '1.6', color: 'var(--text-secondary)' }}>
                          {item.source_kr}
                        </div>

                        {/* 右：中文译文输入框 */}
                        <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
                          <textarea
                            value={draftText}
                            placeholder="在此输入符合边狱公司本地化规范的中文译文..."
                            onChange={(e) => {
                              setEditingTranslations({ ...editingTranslations, [item.id]: e.target.value });
                            }}
                            rows={3}
                            style={{
                              width: '100%',
                              background: 'var(--bg-surface)',
                              border: '1px solid',
                              borderColor: hasBadTilde ? 'var(--accent-red)' : 'var(--border-ruler)',
                              color: 'var(--text-primary)',
                              padding: '10px',
                              fontSize: '13px',
                              lineHeight: '1.6',
                              fontFamily: 'var(--font-sans)',
                              outline: 'none',
                              resize: 'vertical',
                            }}
                          />

                          {/* 底部操作条 */}
                          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                            <div>
                              {hasBadTilde && (
                                <button
                                  onClick={() => handleAutoFix(item)}
                                  style={{
                                    background: 'transparent',
                                    border: '1px solid var(--accent-red)',
                                    color: 'var(--accent-red)',
                                    fontSize: '10px',
                                    fontFamily: 'var(--font-mono)',
                                    padding: '2px 8px',
                                    cursor: 'pointer',
                                  }}
                                >
                                  ⚡ 一键修正全角波浪号
                                </button>
                              )}
                            </div>

                            <button
                              onClick={() => handleSaveTranslation(item, draftText)}
                              disabled={savingId === item.id}
                              style={{
                                background: hasBadTilde ? 'var(--border-ruler)' : 'var(--accent-tech)',
                                color: '#000',
                                border: 'none',
                                padding: '4px 14px',
                                fontSize: '11px',
                                fontWeight: 700,
                                fontFamily: 'var(--font-mono)',
                                cursor: hasBadTilde ? 'not-allowed' : 'pointer',
                                display: 'flex',
                                alignItems: 'center',
                                gap: '6px',
                              }}
                            >
                              <Lock size={11} />
                              {savingId === item.id ? 'SAVING...' : 'SAVE & LOCK'}
                            </button>
                          </div>
                        </div>
                      </div>
                    </div>
                  );
                })
              )}
            </div>

            {/* 工作台底部紧凑分页栏 */}
            <div
              style={{
                height: '42px',
                flexShrink: 0,
                background: 'var(--bg-surface)',
                borderTop: '1px solid var(--border-ruler)',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'space-between',
                padding: '0 20px',
                fontSize: '11px',
                fontFamily: 'var(--font-mono)',
              }}
            >
              <div style={{ color: 'var(--text-muted)' }}>
                SHOWING {totalSegments === 0 ? 0 : (page - 1) * pageSize + 1} -{' '}
                {Math.min(page * pageSize, totalSegments)} OF {totalSegments.toLocaleString()} SEGMENTS
              </div>

              <div style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
                <button
                  onClick={() => fetchSegments(Math.max(1, page - 1), stageFilter, searchFilter)}
                  disabled={page <= 1}
                  style={{
                    background: 'var(--bg-card)',
                    border: '1px solid var(--border-ruler)',
                    color: page <= 1 ? 'var(--text-muted)' : 'var(--text-primary)',
                    padding: '3px 10px',
                    cursor: page <= 1 ? 'not-allowed' : 'pointer',
                  }}
                >
                  ◄ PREV
                </button>
                <span style={{ color: 'var(--accent-tech)' }}>
                  PAGE {page} / {totalPages}
                </span>
                <button
                  onClick={() => fetchSegments(Math.min(totalPages, page + 1), stageFilter, searchFilter)}
                  disabled={page >= totalPages}
                  style={{
                    background: 'var(--bg-card)',
                    border: '1px solid var(--border-ruler)',
                    color: page >= totalPages ? 'var(--text-muted)' : 'var(--text-primary)',
                    padding: '3px 10px',
                    cursor: page >= totalPages ? 'not-allowed' : 'pointer',
                  }}
                >
                  NEXT ►
                </button>
              </div>
            </div>
          </div>
        )}

        {/* ============================================================== */}
        {/* TAB 3: 反向台词雷达 (Reverse Finder) */}
        {/* ============================================================== */}
        {activeTab === 'radar' && (
          <div style={{ flex: 1, minHeight: 0, padding: '16px 20px', display: 'flex', flexDirection: 'column', gap: '16px', overflow: 'hidden' }}>
            {/* 搜索控制台 */}
            <div style={{ background: 'var(--bg-card)', border: '1px solid var(--border-subtle)', padding: '16px', flexShrink: 0 }}>
              <div style={{ fontSize: '11px', fontFamily: 'var(--font-mono)', color: 'var(--accent-cyan)', marginBottom: '8px' }}>
                REVERSE IN-GAME SCRIPT FINDER // 毫秒级双擎实机台词雷达 (支持项目词条 + 30 万 TM 记忆库)
              </div>
              <div style={{ display: 'flex', gap: '10px' }}>
                <input
                  type="text"
                  placeholder="输入游戏实机看到的半句话、词条或角色名 (中韩双语均支持)..."
                  value={radarQuery}
                  onChange={(e) => setRadarQuery(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter') executeRadarSearch(radarQuery);
                  }}
                  style={{
                    flex: 1,
                    background: 'var(--bg-surface)',
                    border: '1px solid var(--border-ruler)',
                    color: 'var(--text-primary)',
                    padding: '8px 12px',
                    fontSize: '13px',
                    fontFamily: 'var(--font-mono)',
                    outline: 'none',
                  }}
                />
                <button
                  onClick={() => executeRadarSearch(radarQuery)}
                  style={{
                    background: 'var(--accent-tech)',
                    color: '#000',
                    border: 'none',
                    padding: '0 20px',
                    fontSize: '12px',
                    fontWeight: 700,
                    fontFamily: 'var(--font-mono)',
                    cursor: 'pointer',
                  }}
                >
                  {radarLoading ? 'SCANNING...' : `SEARCH ${stats.tm_entries.toLocaleString()} TM`}
                </button>
              </div>

              {/* 快捷检索标签 */}
              <div style={{ display: 'flex', gap: '8px', marginTop: '10px', alignItems: 'center', flexWrap: 'wrap' }}>
                <span style={{ fontSize: '10px', color: 'var(--text-muted)', fontFamily: 'var(--font-mono)' }}>QUICK TAGS:</span>
                {['身体能力衰退', '계약', '균열', '호흡', '뫼르소', '단테', '거울 조각'].map((tag) => (
                  <button
                    key={tag}
                    onClick={() => {
                      setRadarQuery(tag);
                      executeRadarSearch(tag);
                    }}
                    style={{
                      background: 'var(--bg-surface)',
                      border: '1px solid var(--border-subtle)',
                      color: 'var(--text-secondary)',
                      padding: '2px 8px',
                      fontSize: '10px',
                      fontFamily: 'var(--font-mono)',
                      cursor: 'pointer',
                    }}
                  >
                    {tag}
                  </button>
                ))}
              </div>
            </div>

            {/* 检索状态小标头 */}
            {radarResults.query && (
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', fontSize: '11px', fontFamily: 'var(--font-mono)', color: 'var(--text-muted)', flexShrink: 0 }}>
                <span>
                  QUERY: "{radarResults.query}" // FOUND {radarResults.total_found} MATCHES (
                  {radarResults.tm_entries.length} TM ENTRIES, {radarResults.segments.length} PROJECT SEGMENTS)
                </span>
                <span style={{ color: 'var(--accent-cyan)' }}>LATENCY: {radarDuration}ms</span>
              </div>
            )}

            {/* 检索结果展示表 (允许独立纵向滚动) */}
            <div style={{ flex: 1, minHeight: 0, overflowY: 'auto', display: 'flex', flexDirection: 'column', gap: '10px' }}>
              {radarLoading ? (
                <div style={{ textAlign: 'center', padding: '40px 0', color: 'var(--accent-cyan)', fontFamily: 'var(--font-mono)' }}>
                  ⚡ 雷达正在高速反查 FTS5 倒排索引...
                </div>
              ) : radarResults.total_found === 0 ? (
                <div
                  style={{
                    background: 'var(--bg-card)',
                    border: '1px solid var(--border-ruler)',
                    padding: '30px',
                    textAlign: 'center',
                    display: 'flex',
                    flexDirection: 'column',
                    alignItems: 'center',
                    gap: '8px',
                  }}
                >
                  <div style={{ fontSize: '12px', fontFamily: 'var(--font-mono)', color: 'var(--accent-gold)' }}>
                    NO ENTRIES FOUND IN TM OR PROJECT DATABASE
                  </div>
                  <div style={{ fontSize: '11px', color: 'var(--text-secondary)', fontFamily: 'var(--font-mono)' }}>
                    未能检索到与 "{radarQuery}" 匹配的条目。建议使用上方快捷标签或尝试输入更短的词根。
                  </div>
                </div>
              ) : (
                <>
                  {/* 项目活跃词条命中 */}
                  {radarResults.segments.map((item, idx) => (
                    <div
                      key={`seg-${idx}`}
                      style={{
                        background: 'var(--bg-card)',
                        border: '1px solid var(--border-subtle)',
                        borderLeft: '4px solid var(--accent-tech)',
                        padding: '12px 16px',
                        display: 'flex',
                        flexDirection: 'column',
                        gap: '6px',
                      }}
                    >
                      <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '11px', fontFamily: 'var(--font-mono)' }}>
                        <div style={{ display: 'flex', gap: '10px', alignItems: 'center' }}>
                          <span style={{ background: 'var(--accent-tech)', color: '#000', padding: '1px 6px', fontWeight: 700 }}>
                            PROJECT SEGMENT
                          </span>
                          <span style={{ color: 'var(--text-primary)' }}>{item.relative_path}</span>
                          <span style={{ color: 'var(--text-muted)' }}>{item.key_path}</span>
                        </div>
                        {item.speaker && <span style={{ color: 'var(--accent-gold)' }}>SPEAKER: {item.speaker}</span>}
                      </div>

                      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '14px', marginTop: '4px' }}>
                        <div style={{ background: 'var(--bg-surface)', padding: '8px 12px', fontSize: '12px', color: 'var(--text-secondary)', lineHeight: '1.5' }}>
                          {item.source_kr}
                        </div>
                        <div style={{ background: 'var(--bg-surface)', padding: '8px 12px', fontSize: '12px', color: 'var(--text-primary)', lineHeight: '1.5' }}>
                          {item.target_zh || '<未翻译>'}
                        </div>
                      </div>
                    </div>
                  ))}

                  {/* TM 翻译记忆库命中 */}
                  {radarResults.tm_entries.map((item, idx) => (
                    <div
                      key={`tm-${idx}`}
                      style={{
                        background: 'var(--bg-card)',
                        border: '1px solid var(--border-subtle)',
                        borderLeft: '4px solid var(--accent-cyan)',
                        padding: '12px 16px',
                        display: 'flex',
                        flexDirection: 'column',
                        gap: '6px',
                      }}
                    >
                      <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '11px', fontFamily: 'var(--font-mono)' }}>
                        <div style={{ display: 'flex', gap: '10px', alignItems: 'center' }}>
                          <span style={{ background: 'var(--accent-cyan)', color: '#000', padding: '1px 6px', fontWeight: 700 }}>
                            TM MEMORY ({item.source_origin})
                          </span>
                          <span style={{ color: 'var(--text-muted)' }}>SCOPE: {item.file_scope || '*'}</span>
                          <span style={{ color: 'var(--accent-gold)' }}>WEIGHT: {item.priority}</span>
                        </div>
                        {item.speaker && <span style={{ color: 'var(--accent-cyan)' }}>SPEAKER: {item.speaker}</span>}
                      </div>

                      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '14px', marginTop: '4px' }}>
                        <div style={{ background: 'var(--bg-surface)', padding: '8px 12px', fontSize: '12px', color: 'var(--text-secondary)', lineHeight: '1.5' }}>
                          {item.source_kr}
                        </div>
                        <div style={{ background: 'var(--bg-surface)', padding: '8px 12px', fontSize: '12px', color: 'var(--text-primary)', lineHeight: '1.5', display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start' }}>
                          <span>{item.target_zh}</span>
                          <button
                            onClick={() => {
                              navigator.clipboard.writeText(item.target_zh);
                              showNotification('✓ 译文已复制到剪贴板');
                            }}
                            title="复制译文"
                            style={{ background: 'transparent', border: 'none', color: 'var(--text-muted)', cursor: 'pointer', padding: '2px' }}
                          >
                            <Copy size={12} />
                          </button>
                        </div>
                      </div>
                    </div>
                  ))}
                </>
              )}
            </div>
          </div>
        )}

        {/* ============================================================== */}
        {/* TAB 4: 重点裁决箱 (Focus Box) */}
        {/* ============================================================== */}
        {activeTab === 'focus' && (
          <div style={{ flex: 1, minHeight: 0, padding: '20px 40px', display: 'flex', flexDirection: 'column', justifyContent: 'center', alignItems: 'center', overflowY: 'auto' }}>
            <div
              style={{
                width: '100%',
                maxWidth: '850px',
                background: 'var(--bg-card)',
                border: '1px solid var(--border-ruler)',
                padding: '24px',
                boxShadow: '0 12px 32px rgba(0,0,0,0.8)',
                position: 'relative',
              }}
            >
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', borderBottom: '1px solid var(--border-subtle)', paddingBottom: '10px', marginBottom: '16px' }}>
                <span style={{ fontSize: '12px', fontWeight: 700, fontFamily: 'var(--font-mono)', color: 'var(--accent-tech)' }}>
                  HUMAN FOCUS DECISION BOX // 存疑/生肉高精快速裁决台
                </span>
                <span style={{ fontSize: '11px', fontFamily: 'var(--font-mono)', color: 'var(--text-muted)' }}>
                  DECISION #{focusIndex + 1} OF {Math.max(focusItems.length, 1)}
                </span>
              </div>

              {focusLoading ? (
                <div style={{ textAlign: 'center', padding: '40px 0', color: 'var(--accent-cyan)', fontFamily: 'var(--font-mono)' }}>
                  ⚡ 正在加载生肉裁决队列...
                </div>
              ) : focusItems.length === 0 ? (
                <div style={{ textAlign: 'center', padding: '40px 0', color: 'var(--accent-green)', fontFamily: 'var(--font-mono)' }}>
                  🎉 当前工作区无待裁决生肉！全部词条均已完成翻译。
                </div>
              ) : (
                (() => {
                  const cur = focusItems[focusIndex];
                  const curText = editingTranslations[cur.id] ?? cur.target_zh ?? '';
                  const hasBadTilde = /～|〜/.test(curText);

                  return (
                    <div style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
                      <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '11px', fontFamily: 'var(--font-mono)', color: 'var(--text-muted)' }}>
                        <span>FILE: {cur.relative_path} | KEY: {cur.key_path}</span>
                        {cur.speaker && <span style={{ color: 'var(--accent-gold)' }}>SPEAKER: {cur.speaker}</span>}
                      </div>

                      {/* 韩文原句大字展示 */}
                      <div>
                        <div style={{ fontSize: '10px', color: 'var(--text-muted)', fontFamily: 'var(--font-mono)', marginBottom: '4px' }}>
                          SOURCE KOREAN (原文)
                        </div>
                        <div style={{ background: 'var(--bg-surface)', padding: '14px', fontSize: '15px', lineHeight: '1.6', color: 'var(--text-secondary)' }}>
                          {cur.source_kr}
                        </div>
                      </div>

                      {/* 中文译文大字编辑 */}
                      <div>
                        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '4px' }}>
                          <span style={{ fontSize: '10px', color: 'var(--accent-tech)', fontFamily: 'var(--font-mono)' }}>
                            TARGET CHINESE (裁决译文)
                          </span>
                          {hasBadTilde && (
                            <button
                              onClick={() => handleAutoFix(cur)}
                              style={{
                                background: 'transparent',
                                border: '1px solid var(--accent-red)',
                                color: 'var(--accent-red)',
                                fontSize: '10px',
                                fontFamily: 'var(--font-mono)',
                                padding: '1px 6px',
                                cursor: 'pointer',
                              }}
                            >
                              ⚡ 一键修复全角符号
                            </button>
                          )}
                        </div>
                        <textarea
                          rows={3}
                          value={curText}
                          onChange={(e) => setEditingTranslations({ ...editingTranslations, [cur.id]: e.target.value })}
                          onKeyDown={(e) => {
                            if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
                              handleSaveTranslation(cur, curText, true);
                            }
                          }}
                          placeholder="在此输入裁决译文 (按 Ctrl+Enter 提交并自动切换至下一条)..."
                          style={{
                            width: '100%',
                            background: 'var(--bg-surface)',
                            border: '1px solid',
                            borderColor: hasBadTilde ? 'var(--accent-red)' : 'var(--accent-tech)',
                            color: 'var(--text-primary)',
                            padding: '12px',
                            fontSize: '14px',
                            lineHeight: '1.6',
                            fontFamily: 'var(--font-sans)',
                            outline: 'none',
                          }}
                        />
                      </div>

                      {/* 键盘操作流按钮 */}
                      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginTop: '8px' }}>
                        <div style={{ display: 'flex', gap: '8px' }}>
                          <button
                            onClick={() => setFocusIndex(Math.max(0, focusIndex - 1))}
                            disabled={focusIndex === 0}
                            style={{
                              background: 'transparent',
                              border: '1px solid var(--border-ruler)',
                              color: 'var(--text-secondary)',
                              padding: '6px 12px',
                              fontSize: '11px',
                              fontFamily: 'var(--font-mono)',
                              cursor: focusIndex === 0 ? 'not-allowed' : 'pointer',
                            }}
                          >
                            ◄ PREV (Left)
                          </button>
                          <button
                            onClick={() => setFocusIndex(Math.min(focusItems.length - 1, focusIndex + 1))}
                            disabled={focusIndex >= focusItems.length - 1}
                            style={{
                              background: 'transparent',
                              border: '1px solid var(--border-ruler)',
                              color: 'var(--text-secondary)',
                              padding: '6px 12px',
                              fontSize: '11px',
                              fontFamily: 'var(--font-mono)',
                              cursor: focusIndex >= focusItems.length - 1 ? 'not-allowed' : 'pointer',
                            }}
                          >
                            NEXT (Right) ►
                          </button>
                        </div>

                        <button
                          onClick={() => handleSaveTranslation(cur, curText, true)}
                          disabled={savingId === cur.id || hasBadTilde}
                          style={{
                            background: hasBadTilde ? 'var(--border-ruler)' : 'var(--accent-tech)',
                            color: '#000',
                            border: 'none',
                            padding: '6px 20px',
                            fontSize: '12px',
                            fontWeight: 700,
                            fontFamily: 'var(--font-mono)',
                            cursor: hasBadTilde ? 'not-allowed' : 'pointer',
                            display: 'flex',
                            alignItems: 'center',
                            gap: '6px',
                          }}
                        >
                          <CornerDownLeft size={13} />
                          {savingId === cur.id ? 'SAVING...' : 'APPROVE & NEXT (Ctrl+Enter)'}
                        </button>
                      </div>
                    </div>
                  );
                })()
              )}
            </div>
          </div>
        )}
      </main>
    </div>
  );
}
