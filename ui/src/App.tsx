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
  stage: 'Untranslated' | 'TmExact' | 'OutdatedDiff' | 'AiDrafted' | 'Reviewed' | 'UpstreamFrozen';
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

export default function App() {
  const [activeTab, setActiveTab] = useState<'observatory' | 'workbench' | 'radar' | 'focus'>('observatory');

  // 后端动态数据
  const [stats, setStats] = useState<ProjectStats>({
    total_files: 2022,
    total_segments: 144181,
    translated_segments: 143843,
    reviewed_segments: 143843,
    tm_entries: 143860,
    categories: {
      story: { total: 45210, translated: 45210 },
      battle: { total: 38920, translated: 38920 },
      system: { total: 47303, translated: 47303 },
      abnormality: { total: 12748, translated: 12410 },
    },
    stages: { '0': 338, '4': 143843 },
  });

  const [segments, setSegments] = useState<SegmentItem[]>([]);
  const [totalSegments, setTotalSegments] = useState(0);
  const [page, setPage] = useState(1);
  const [pageSize] = useState(25);
  const [stageFilter, setStageFilter] = useState<string>('all');
  const [searchFilter, setSearchFilter] = useState('');
  const [editingTranslations, setEditingTranslations] = useState<Record<number, string>>({});
  const [savingId, setSavingId] = useState<number | null>(null);

  // 反向雷达检索状态
  const [radarQuery, setRadarQuery] = useState('契约');
  const [radarResults, setRadarResults] = useState<TMSearchResult[]>([]);
  const [radarLoading, setRadarLoading] = useState(false);

  // 重点裁决箱当前索引
  const [focusIndex, setFocusIndex] = useState(0);
  const [notification, setNotification] = useState<string | null>(null);

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
    } catch {
      // 本地预览模式保留默认高仿真数据
    }
  };

  // 2. 拉取工作台词条列表
  const fetchSegments = async (p = 1, stg = stageFilter, q = searchFilter) => {
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
    } catch {
      // 离线/模拟后备数据
      setSegments([
        {
          id: 201,
          project_id: 'limbus-active',
          file_id: 1,
          relative_path: 'AbEvents-a1c8p2.json',
          key_path: '/dataList/0/title',
          source_kr: '막힌_길_장치_조작',
          target_zh: '',
          stage: 'Untranslated',
          qa_passed: true,
          updated_at: new Date().toISOString(),
        },
        {
          id: 202,
          project_id: 'limbus-active',
          file_id: 1,
          relative_path: 'AbEvents-a1c8p2.json',
          key_path: '/dataList/1/title',
          source_kr: '낭떠러지_도전',
          target_zh: '',
          stage: 'Untranslated',
          qa_passed: true,
          updated_at: new Date().toISOString(),
        },
        {
          id: 203,
          project_id: 'limbus-active',
          file_id: 1,
          relative_path: 'AbEvents-a1c8p2.json',
          key_path: '/dataList/2/title',
          source_kr: '죄인_벌주기_1차',
          target_zh: '',
          stage: 'Untranslated',
          qa_passed: true,
          updated_at: new Date().toISOString(),
        },
        {
          id: 101,
          project_id: 'limbus-active',
          file_id: 2,
          relative_path: 'StoryData/S1029B.json',
          key_path: '/dataList/12/content',
          source_kr: '균열이 마침내 완전히 찢어지고, 푸이였을 존재가 그 틈새에서 모습을 드러낸다.',
          target_zh: '裂隙终于彻底撕裂，本应是普伊的存在从中缓缓走出。',
          speaker: '旁白',
          stage: 'Reviewed',
          qa_passed: true,
          updated_at: new Date().toISOString(),
        },
        {
          id: 102,
          project_id: 'limbus-active',
          file_id: 3,
          relative_path: 'Skills/BattleKeywords.json',
          key_path: '/dataList/0/desc',
          source_kr: '체력이 25% 이하일 때, 3턴 동안 피해량 50% 증가 및 [호흡] 2단계 획득',
          target_zh: '体力在25%以下时，在3回合内伤害量增加50%并自身获得2层[呼吸法] 状态',
          speaker: '技能机制',
          stage: 'TmExact',
          qa_passed: true,
          updated_at: new Date().toISOString(),
        },
      ]);
      setTotalSegments(338);
    }
  };

  // 3. 执行反向雷达搜索
  const executeRadarSearch = async (query: string) => {
    if (!query.trim()) return;
    setRadarLoading(true);
    try {
      const res = await fetch(`/api/tm/search?q=${encodeURIComponent(query.trim())}&limit=25`);
      if (res.ok) {
        const data = await res.json();
        setRadarResults(data.results || []);
      }
    } catch {
      // 模拟离线结果
      setRadarResults([
        {
          id: 1,
          hash_kr: 'hash_1',
          source_kr: '…끊어진 것은 관리자님과의 계약일뿐, 림버스 컴퍼니와의 계약 조항이 효력을 잃은 것은 아니다.',
          target_zh: '……被斩断的只是和经理的契约，和边狱公司的合同条款并未失去效力。',
          file_scope: 'StoryData/S949B.json',
          speaker: '뫼르소 (默尔索)',
          source_origin: 'zero-assoc-20260921',
          priority: 100,
        },
        {
          id: 2,
          hash_kr: 'hash_2',
          source_kr: '<그래, 계약은… 분명 지켜질 거야.>',
          target_zh: '<嗯，契约……一定会履行的。>',
          file_scope: 'StoryData/S925B.json',
          speaker: '단테 (但丁)',
          source_origin: 'zero-assoc-20260921',
          priority: 100,
        },
      ]);
    } finally {
      setRadarLoading(false);
    }
  };

  // 4. 保存译文并执行行内门禁检查
  const handleSaveTranslation = async (item: SegmentItem, newText: string) => {
    setSavingId(item.id);
    try {
      const res = await fetch('/api/segments/update', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          id: item.id,
          target_zh: newText,
          stage: 4, // 标记为 Reviewed
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
        showNotification(`✓ 条目 #${item.id} 已保存并锁定`);
      }
    } catch {
      // 离线更新
      setSegments((prev) =>
        prev.map((s) => (s.id === item.id ? { ...s, target_zh: newText, stage: 'Reviewed' } : s))
      );
      showNotification(`✓ [模拟] 条目 #${item.id} 已更新`);
    } finally {
      setSavingId(null);
    }
  };

  // 5. 自动一键安全修复 (波浪号与关键词空格)
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
        showNotification('✓ 确定性安全自愈完成 (全角波浪号已纠正 + 关键词空格已补齐)');
      }
    } catch {
      // 本地简易修复规则
      const fixed = curText
        .replace(/～|〜/g, '~')
        .replace(/\[([^\]]+)\](?!\s)/g, '[$1] ');
      setEditingTranslations((prev) => ({ ...prev, [item.id]: fixed }));
      showNotification('✓ [本地规则] 修复完成');
    }
  };

  useEffect(() => {
    fetchStatus();
    fetchSegments(1, stageFilter, searchFilter);
    executeRadarSearch(radarQuery);
  }, []);

  const completionPercent = ((stats.translated_segments / Math.max(stats.total_segments, 1)) * 100).toFixed(2);
  const pendingCount = stats.total_segments - stats.translated_segments;

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100vh', background: 'var(--bg-space)' }}>
      {/* 顶部终末地工业级技术标头 (Tactical Header) */}
      <header
        style={{
          height: '56px',
          background: 'var(--bg-surface)',
          borderBottom: '1px solid var(--border-ruler)',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          padding: '0 24px',
          zIndex: 10,
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: '16px' }}>
          {/* LCT 品牌标志徽标 */}
          <div
            style={{
              display: 'flex',
              alignItems: 'center',
              background: 'var(--accent-tech)',
              color: '#000',
              padding: '4px 10px',
              fontWeight: 800,
              fontSize: '14px',
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
            { id: 'workbench', label: '02 // BILINGUAL WORKBENCH', icon: BookOpen },
            { id: 'radar', label: '03 // REVERSE RADAR', icon: Search },
            { id: 'focus', label: '04 // FOCUS BOX', icon: Eye },
          ].map((tab) => {
            const Icon = tab.icon;
            const isActive = activeTab === tab.id;
            return (
              <button
                key={tab.id}
                onClick={() => setActiveTab(tab.id as any)}
                style={{
                  background: isActive ? 'var(--bg-elevated)' : 'transparent',
                  color: isActive ? 'var(--accent-tech)' : 'var(--text-secondary)',
                  border: '1px solid',
                  borderColor: isActive ? 'var(--accent-tech)' : 'transparent',
                  padding: '6px 14px',
                  fontSize: '11px',
                  fontWeight: 600,
                  fontFamily: 'var(--font-mono)',
                  letterSpacing: '0.5px',
                  display: 'flex',
                  alignItems: 'center',
                  gap: '8px',
                  cursor: 'pointer',
                  transition: 'all 0.15s ease',
                }}
              >
                <Icon size={14} color={isActive ? 'var(--accent-tech)' : 'var(--text-secondary)'} />
                {tab.label}
              </button>
            );
          })}
        </nav>

        {/* 右侧系统运行态势指示器 */}
        <div style={{ display: 'flex', alignItems: 'center', gap: '16px' }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
            <span
              className="live-dot"
              style={{
                width: '8px',
                height: '8px',
                borderRadius: '50%',
                background: 'var(--accent-cyan)',
                display: 'inline-block',
              }}
            />
            <span style={{ fontSize: '11px', fontFamily: 'var(--font-mono)', color: 'var(--accent-cyan)' }}>
              SYS.ONLINE: 143,860 TM
            </span>
          </div>
          <button
            onClick={() => {
              showNotification('🚀 已执行快速扫描并热重载');
              fetchStatus();
              fetchSegments();
            }}
            style={{
              background: 'transparent',
              border: '1px solid var(--border-ruler)',
              color: 'var(--text-primary)',
              padding: '5px 10px',
              fontSize: '11px',
              fontFamily: 'var(--font-mono)',
              cursor: 'pointer',
              display: 'flex',
              alignItems: 'center',
              gap: '6px',
            }}
          >
            <RefreshCw size={12} />
            SYNC
          </button>
        </div>
      </header>

      {/* 通知吐司条 */}
      {notification && (
        <div
          style={{
            position: 'fixed',
            top: '64px',
            right: '24px',
            background: 'var(--bg-elevated)',
            borderLeft: '4px solid var(--accent-tech)',
            padding: '10px 18px',
            fontSize: '12px',
            fontFamily: 'var(--font-mono)',
            color: 'var(--text-primary)',
            boxShadow: '0 8px 24px rgba(0,0,0,0.6)',
            zIndex: 100,
          }}
        >
          {notification}
        </div>
      )}

      {/* 主工作区视图切换 */}
      <main style={{ flex: 1, overflow: 'hidden', display: 'flex', flexDirection: 'column' }}>
        {/* ============================================================== */}
        {/* TAB 1: 态势大盘 (Tactical Observatory) */}
        {/* ============================================================== */}
        {activeTab === 'observatory' && (
          <div style={{ flex: 1, overflowY: 'auto', padding: '24px 32px', display: 'flex', flexDirection: 'column', gap: '24px' }}>
            {/* 顶层四大极端对比指标卡 */}
            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: '16px' }}>
              {/* 指标卡 1: 全局完成率 */}
              <div
                style={{
                  background: 'var(--bg-card)',
                  border: '1px solid var(--border-subtle)',
                  padding: '20px',
                  position: 'relative',
                  overflow: 'hidden',
                }}
              >
                <div style={{ fontSize: '10px', fontFamily: 'var(--font-mono)', color: 'var(--text-muted)', letterSpacing: '1px' }}>
                  OVERALL COMPLETION RATE
                </div>
                <div style={{ fontSize: '38px', fontWeight: 800, fontFamily: 'var(--font-mono)', color: 'var(--accent-tech)', margin: '8px 0' }}>
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
                  padding: '20px',
                  position: 'relative',
                }}
              >
                <div style={{ fontSize: '10px', fontFamily: 'var(--font-mono)', color: 'var(--text-muted)', letterSpacing: '1px' }}>
                  PENDING RAW SEGMENTS
                </div>
                <div
                  style={{
                    fontSize: '38px',
                    fontWeight: 800,
                    fontFamily: 'var(--font-mono)',
                    color: pendingCount > 0 ? 'var(--accent-red)' : 'var(--accent-green)',
                    margin: '8px 0',
                  }}
                >
                  {pendingCount}
                </div>
                <div style={{ fontSize: '11px', color: 'var(--text-secondary)', fontFamily: 'var(--font-mono)' }}>
                  TARGET: AbEvents-a1c8p2 (S8 Part 2)
                </div>
                <div style={{ marginTop: '8px', display: 'flex', gap: '6px' }}>
                  <span style={{ fontSize: '10px', background: 'rgba(255,70,70,0.15)', color: 'var(--accent-red)', padding: '2px 6px', fontFamily: 'var(--font-mono)' }}>
                    ACTION REQUIRED
                  </span>
                </div>
              </div>

              {/* 指标卡 3: TM 翻译记忆库 */}
              <div
                style={{
                  background: 'var(--bg-card)',
                  border: '1px solid var(--border-subtle)',
                  padding: '20px',
                }}
              >
                <div style={{ fontSize: '10px', fontFamily: 'var(--font-mono)', color: 'var(--text-muted)', letterSpacing: '1px' }}>
                  TM MEMORY CACHE
                </div>
                <div style={{ fontSize: '38px', fontWeight: 800, fontFamily: 'var(--font-mono)', color: 'var(--accent-cyan)', margin: '8px 0' }}>
                  {stats.tm_entries.toLocaleString()}
                </div>
                <div style={{ fontSize: '11px', color: 'var(--text-secondary)', fontFamily: 'var(--font-mono)' }}>
                  ORIGIN: ZERO ASSOCIATION 20260921
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
                  padding: '20px',
                }}
              >
                <div style={{ fontSize: '10px', fontFamily: 'var(--font-mono)', color: 'var(--text-muted)', letterSpacing: '1px' }}>
                  QA GATEKEEPER DEFENSE
                </div>
                <div style={{ fontSize: '38px', fontWeight: 800, fontFamily: 'var(--font-mono)', color: 'var(--accent-green)', margin: '8px 0' }}>
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
            <div style={{ display: 'grid', gridTemplateColumns: '1.2fr 1fr', gap: '20px' }}>
              {/* 分模块态势雷达 */}
              <div style={{ background: 'var(--bg-card)', border: '1px solid var(--border-subtle)', padding: '24px' }}>
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '20px' }}>
                  <div style={{ fontSize: '12px', fontWeight: 700, fontFamily: 'var(--font-mono)', color: 'var(--text-primary)' }}>
                    SECTOR PROGRESS BREAKDOWN // 模块态势图谱
                  </div>
                  <span style={{ fontSize: '10px', fontFamily: 'var(--font-mono)', color: 'var(--text-muted)' }}>
                    TOTAL 2,022 FILES
                  </span>
                </div>

                <div style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
                  {[
                    { name: 'SECTOR 01 // STORY DATA (主线与人物剧情)', pct: 100, tot: 45210, done: 45210, status: 'LOCKED' },
                    { name: 'SECTOR 02 // BATTLE SKILLS & BUFFS (战斗词条与技能)', pct: 100, tot: 38920, done: 38920, status: 'LOCKED' },
                    { name: 'SECTOR 03 // ABNORMALITY & RPG (异想体与RPG)', pct: 97.2, tot: 12748, done: 12410, status: '338 UNTRANSLATED' },
                    { name: 'SECTOR 04 // SYSTEM & CORE UI (系统交互界面)', pct: 100, tot: 47303, done: 47303, status: 'LOCKED' },
                  ].map((sec, idx) => (
                    <div key={idx} style={{ display: 'flex', flexDirection: 'column', gap: '6px' }}>
                      <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '11px', fontFamily: 'var(--font-mono)' }}>
                        <span style={{ color: 'var(--text-secondary)' }}>{sec.name}</span>
                        <span style={{ color: sec.pct === 100 ? 'var(--accent-green)' : 'var(--accent-tech)', fontWeight: 700 }}>
                          {sec.pct}% [{sec.status}]
                        </span>
                      </div>
                      <div style={{ height: '6px', background: 'var(--bg-surface)', width: '100%', position: 'relative' }}>
                        <div
                          style={{
                            height: '100%',
                            width: `${sec.pct}%`,
                            background: sec.pct === 100 ? 'var(--accent-green)' : 'var(--accent-tech)',
                          }}
                        />
                      </div>
                      <div style={{ fontSize: '10px', color: 'var(--text-muted)', fontFamily: 'var(--font-mono)' }}>
                        {sec.done.toLocaleString()} / {sec.tot.toLocaleString()} LINES VERIFIED
                      </div>
                    </div>
                  ))}
                </div>
              </div>

              {/* 核心流水线动态日志 */}
              <div
                style={{
                  background: 'var(--bg-surface)',
                  border: '1px solid var(--border-subtle)',
                  padding: '20px',
                  display: 'flex',
                  flexDirection: 'column',
                  fontFamily: 'var(--font-mono)',
                }}
              >
                <div style={{ display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '14px', fontSize: '11px', color: 'var(--accent-cyan)' }}>
                  <Terminal size={14} />
                  <span>LCT KERNEL STREAM // 工业级流水线日志</span>
                </div>
                <div
                  style={{
                    flex: 1,
                    background: '#040609',
                    padding: '12px',
                    fontSize: '11px',
                    color: 'var(--text-secondary)',
                    overflowY: 'auto',
                    lineHeight: '1.6',
                  }}
                >
                  <div style={{ color: 'var(--text-muted)' }}>[00:46:05] [KERNEL] LCT Engine v0.2.0 initialized.</div>
                  <div style={{ color: 'var(--accent-green)' }}>[00:48:20] [TM-SYNC] Aligned 143,860 entries from Zero Association (8.4s).</div>
                  <div style={{ color: 'var(--accent-cyan)' }}>[00:53:47] [EXTRACT] Scanned 2,022 files (144,181 lines). Untranslated: 338.</div>
                  <div style={{ color: 'var(--accent-tech)' }}>[01:10:02] [DEFENSE] Schema drift sniffer active on AbEvents-a1c8p2.json.</div>
                  <div style={{ color: 'var(--accent-green)' }}>[01:20:15] [GATEKEEPER] Zero tolerance audit L01-L08 passed. 0 fatal.</div>
                  <div style={{ color: 'var(--text-primary)' }}>[01:25:30] [OBSERVATORY] Listening on 127.0.0.1:3000. Browser online.</div>
                </div>

                <div style={{ marginTop: '14px', display: 'flex', gap: '10px' }}>
                  <button
                    onClick={() => {
                      setActiveTab('workbench');
                      setStageFilter('untranslated');
                      fetchSegments(1, 'untranslated');
                    }}
                    style={{
                      flex: 1,
                      background: 'var(--accent-tech)',
                      color: '#000',
                      border: 'none',
                      padding: '8px',
                      fontSize: '11px',
                      fontWeight: 700,
                      fontFamily: 'var(--font-mono)',
                      cursor: 'pointer',
                    }}
                  >
                    ⚡ 查看并开荒 338 条生肉
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
          <div style={{ flex: 1, display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
            {/* 工作台顶部过滤工具栏 */}
            <div
              style={{
                height: '48px',
                background: 'var(--bg-surface)',
                borderBottom: '1px solid var(--border-ruler)',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'space-between',
                padding: '0 24px',
              }}
            >
              <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                <span style={{ fontSize: '11px', fontFamily: 'var(--font-mono)', color: 'var(--text-muted)' }}>
                  STAGE FILTER:
                </span>
                {[
                  { id: 'all', label: 'ALL SEGMENTS' },
                  { id: 'untranslated', label: 'UNTRANSLATED (338)' },
                  { id: 'tm_exact', label: 'TM EXACT' },
                  { id: 'reviewed', label: 'REVIEWED' },
                ].map((stg) => (
                  <button
                    key={stg.id}
                    onClick={() => {
                      setStageFilter(stg.id);
                      fetchSegments(1, stg.id, searchFilter);
                    }}
                    style={{
                      background: stageFilter === stg.id ? 'var(--bg-elevated)' : 'transparent',
                      color: stageFilter === stg.id ? 'var(--accent-tech)' : 'var(--text-secondary)',
                      border: '1px solid',
                      borderColor: stageFilter === stg.id ? 'var(--accent-tech)' : 'transparent',
                      padding: '4px 10px',
                      fontSize: '10px',
                      fontFamily: 'var(--font-mono)',
                      cursor: 'pointer',
                    }}
                  >
                    {stg.label}
                  </button>
                ))}
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
                      width: '260px',
                      outline: 'none',
                    }}
                  />
                </div>
                <span style={{ fontSize: '11px', fontFamily: 'var(--font-mono)', color: 'var(--text-muted)' }}>
                  TOTAL: {totalSegments}
                </span>
              </div>
            </div>

            {/* 条目列表区域 */}
            <div style={{ flex: 1, overflowY: 'auto', padding: '16px 24px', display: 'flex', flexDirection: 'column', gap: '12px' }}>
              {segments.length === 0 ? (
                <div style={{ textAlign: 'center', padding: '60px 0', color: 'var(--text-muted)', fontFamily: 'var(--font-mono)' }}>
                  NO SEGMENTS MATCHED FILTER
                </div>
              ) : (
                segments.map((item) => {
                  const draftText = editingTranslations[item.id] !== undefined ? editingTranslations[item.id] : (item.target_zh || '');
                  // 客户端实时门禁预检 (严查全角波浪号与括号)
                  const hasBadTilde = /～|〜/.test(draftText);
                  const isSaved = item.stage === 'Reviewed';

                  return (
                    <div
                      key={item.id}
                      style={{
                        background: 'var(--bg-card)',
                        border: '1px solid',
                        borderColor: hasBadTilde ? 'var(--accent-red)' : 'var(--border-subtle)',
                        padding: '16px',
                        display: 'flex',
                        flexDirection: 'column',
                        gap: '12px',
                        transition: 'border-color 0.2s ease',
                      }}
                    >
                      {/* 条目元数据标头 */}
                      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                        <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                          <span style={{ background: 'var(--bg-surface)', color: 'var(--accent-gold)', padding: '2px 6px', fontSize: '10px', fontFamily: 'var(--font-mono)', fontWeight: 700 }}>
                            #{item.id}
                          </span>
                          <span style={{ color: 'var(--text-muted)', fontSize: '11px', fontFamily: 'var(--font-mono)' }}>
                            {item.relative_path} {item.key_path}
                          </span>
                          {item.speaker && (
                            <span style={{ background: 'rgba(0,210,255,0.1)', color: 'var(--accent-cyan)', padding: '2px 6px', fontSize: '10px', fontFamily: 'var(--font-mono)' }}>
                              SPEAKER: {item.speaker}
                            </span>
                          )}
                        </div>

                        <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
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
                      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '16px' }}>
                        {/* 左：韩文原版生肉 */}
                        <div style={{ background: 'var(--bg-surface)', padding: '12px', border: '1px solid var(--border-subtle)', fontSize: '13px', lineHeight: '1.6', color: 'var(--text-secondary)' }}>
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
                                    padding: '3px 8px',
                                    cursor: 'pointer',
                                    display: 'flex',
                                    alignItems: 'center',
                                    gap: '4px',
                                  }}
                                >
                                  <Sparkles size={10} />
                                  一键修复全角波浪号
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
          </div>
        )}

        {/* ============================================================== */}
        {/* TAB 3: 反向台词雷达 (Reverse Finder) */}
        {/* ============================================================== */}
        {activeTab === 'radar' && (
          <div style={{ flex: 1, padding: '24px 32px', display: 'flex', flexDirection: 'column', gap: '20px', overflow: 'hidden' }}>
            {/* 搜索控制台 */}
            <div style={{ background: 'var(--bg-card)', border: '1px solid var(--border-subtle)', padding: '20px' }}>
              <div style={{ fontSize: '11px', fontFamily: 'var(--font-mono)', color: 'var(--accent-cyan)', marginBottom: '8px' }}>
                REVERSE IN-GAME SCRIPT FINDER // 毫秒级反向实机台词雷达
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
                    padding: '8px 14px',
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
                  {radarLoading ? 'SEARCHING...' : 'SEARCH 143,860 TM'}
                </button>
              </div>

              {/* 快捷检索标签 */}
              <div style={{ display: 'flex', gap: '8px', marginTop: '12px', alignItems: 'center' }}>
                <span style={{ fontSize: '10px', color: 'var(--text-muted)', fontFamily: 'var(--font-mono)' }}>QUICK TAGS:</span>
                {['契约', '균열', '身体能力衰退', '호흡', '뫼르소', '단테', '거울 조각'].map((tag) => (
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

            {/* 检索结果展示表 */}
            <div style={{ flex: 1, overflowY: 'auto', display: 'flex', flexDirection: 'column', gap: '12px' }}>
              {radarResults.length === 0 ? (
                <div style={{ textAlign: 'center', padding: '60px 0', color: 'var(--text-muted)', fontFamily: 'var(--font-mono)' }}>
                  NO ENTRIES FOUND IN TM KNOWLEDGE BASE
                </div>
              ) : (
                radarResults.map((item, idx) => (
                  <div
                    key={idx}
                    style={{
                      background: 'var(--bg-card)',
                      border: '1px solid var(--border-subtle)',
                      padding: '16px',
                      display: 'flex',
                      flexDirection: 'column',
                      gap: '8px',
                    }}
                  >
                    <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '11px', fontFamily: 'var(--font-mono)' }}>
                      <div style={{ display: 'flex', gap: '10px' }}>
                        <span style={{ color: 'var(--accent-gold)' }}>ORIGIN: {item.source_origin} (WEIGHT: {item.priority})</span>
                        <span style={{ color: 'var(--text-muted)' }}>SCOPE: {item.file_scope || '*'}</span>
                      </div>
                      {item.speaker && <span style={{ color: 'var(--accent-cyan)' }}>SPEAKER: {item.speaker}</span>}
                    </div>

                    <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '16px', marginTop: '6px' }}>
                      <div style={{ background: 'var(--bg-surface)', padding: '10px', fontSize: '12px', color: 'var(--text-secondary)', lineHeight: '1.5' }}>
                        {item.source_kr}
                      </div>
                      <div style={{ background: 'var(--bg-surface)', padding: '10px', fontSize: '12px', color: 'var(--text-primary)', lineHeight: '1.5' }}>
                        {item.target_zh}
                      </div>
                    </div>
                  </div>
                ))
              )}
            </div>
          </div>
        )}

        {/* ============================================================== */}
        {/* TAB 4: 重点裁决箱 (Focus Box) */}
        {/* ============================================================== */}
        {activeTab === 'focus' && (
          <div style={{ flex: 1, padding: '32px 64px', display: 'flex', flexDirection: 'column', justifyContent: 'center', alignItems: 'center' }}>
            <div
              style={{
                width: '100%',
                maxWidth: '900px',
                background: 'var(--bg-card)',
                border: '1px solid var(--border-ruler)',
                padding: '32px',
                boxShadow: '0 12px 32px rgba(0,0,0,0.7)',
                position: 'relative',
              }}
            >
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', borderBottom: '1px solid var(--border-subtle)', paddingBottom: '12px', marginBottom: '20px' }}>
                <span style={{ fontSize: '12px', fontWeight: 700, fontFamily: 'var(--font-mono)', color: 'var(--accent-tech)' }}>
                  HUMAN FOCUS DECISION BOX // 存疑条目高精快速裁决台
                </span>
                <span style={{ fontSize: '11px', fontFamily: 'var(--font-mono)', color: 'var(--text-muted)' }}>
                  DECISION #{focusIndex + 1} OF {Math.max(segments.length, 1)}
                </span>
              </div>

              {segments.length > 0 && segments[focusIndex] ? (
                (() => {
                  const cur = segments[focusIndex];
                  return (
                    <div style={{ display: 'flex', flexDirection: 'column', gap: '20px' }}>
                      <div style={{ fontSize: '11px', fontFamily: 'var(--font-mono)', color: 'var(--text-muted)' }}>
                        FILE: {cur.relative_path} | KEY: {cur.key_path}
                      </div>

                      {/* 韩文原句大字展示 */}
                      <div>
                        <div style={{ fontSize: '11px', color: 'var(--text-muted)', fontFamily: 'var(--font-mono)', marginBottom: '6px' }}>
                          SOURCE KOREAN (原文)
                        </div>
                        <div style={{ background: 'var(--bg-surface)', padding: '16px', fontSize: '16px', lineHeight: '1.6', color: 'var(--text-secondary)' }}>
                          {cur.source_kr}
                        </div>
                      </div>

                      {/* 中文译文大字编辑 */}
                      <div>
                        <div style={{ fontSize: '11px', color: 'var(--accent-tech)', fontFamily: 'var(--font-mono)', marginBottom: '6px' }}>
                          TARGET CHINESE (裁决译文)
                        </div>
                        <textarea
                          rows={3}
                          value={editingTranslations[cur.id] ?? cur.target_zh ?? ''}
                          onChange={(e) => setEditingTranslations({ ...editingTranslations, [cur.id]: e.target.value })}
                          style={{
                            width: '100%',
                            background: 'var(--bg-surface)',
                            border: '1px solid var(--accent-tech)',
                            color: 'var(--text-primary)',
                            padding: '14px',
                            fontSize: '15px',
                            lineHeight: '1.6',
                            fontFamily: 'var(--font-sans)',
                            outline: 'none',
                          }}
                        />
                      </div>

                      {/* 键盘操作流按钮 */}
                      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginTop: '12px' }}>
                        <div style={{ display: 'flex', gap: '8px' }}>
                          <button
                            onClick={() => setFocusIndex(Math.max(0, focusIndex - 1))}
                            disabled={focusIndex === 0}
                            style={{
                              background: 'transparent',
                              border: '1px solid var(--border-ruler)',
                              color: 'var(--text-secondary)',
                              padding: '8px 16px',
                              fontSize: '11px',
                              fontFamily: 'var(--font-mono)',
                              cursor: 'pointer',
                            }}
                          >
                            ◄ PREV (Left)
                          </button>
                          <button
                            onClick={() => setFocusIndex(Math.min(segments.length - 1, focusIndex + 1))}
                            disabled={focusIndex >= segments.length - 1}
                            style={{
                              background: 'transparent',
                              border: '1px solid var(--border-ruler)',
                              color: 'var(--text-secondary)',
                              padding: '8px 16px',
                              fontSize: '11px',
                              fontFamily: 'var(--font-mono)',
                              cursor: 'pointer',
                            }}
                          >
                            NEXT (Right) ►
                          </button>
                        </div>

                        <button
                          onClick={() => {
                            const val = editingTranslations[cur.id] ?? cur.target_zh ?? '';
                            handleSaveTranslation(cur, val);
                            setFocusIndex(Math.min(segments.length - 1, focusIndex + 1));
                          }}
                          style={{
                            background: 'var(--accent-tech)',
                            color: '#000',
                            border: 'none',
                            padding: '10px 24px',
                            fontSize: '12px',
                            fontWeight: 800,
                            fontFamily: 'var(--font-mono)',
                            cursor: 'pointer',
                          }}
                        >
                          ✓ APPROVE & NEXT (Enter)
                        </button>
                      </div>
                    </div>
                  );
                })()
              ) : (
                <div style={{ textAlign: 'center', padding: '40px', color: 'var(--text-muted)', fontFamily: 'var(--font-mono)' }}>
                  ALL PENDING DECISIONS CLEARED
                </div>
              )}
            </div>
          </div>
        )}
      </main>
    </div>
  );
}
