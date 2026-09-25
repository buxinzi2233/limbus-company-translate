import React, { useState } from 'react';
import {
  Activity,
  AlertTriangle,
  BookOpen,
  CheckCircle,
  Database,
  Download,
  Eye,
  Layers,
  Play,
  RefreshCw,
  Search,
  Shield,
  Zap,
} from 'lucide-react';

interface SegmentItem {
  id: number;
  pointer: string;
  source_kr: string;
  target_zh: string;
  speaker?: string;
  stage: 'Untranslated' | 'TmExact' | 'OutdatedDiff' | 'AiDrafted' | 'Reviewed' | 'UpstreamFrozen';
  qa_passed: boolean;
}

export default function App() {
  const [activeTab, setActiveTab] = useState<'dashboard' | 'editor' | 'audit' | 'inbox'>('dashboard');
  const [searchQuery, setSearchQuery] = useState('');
  const [filterMode, setFilterMode] = useState<'all' | 'untranslated' | 'review'>('all');

  // 模拟数据展示真实体验
  const [segments, setSegments] = useState<SegmentItem[]>([
    {
      id: 10101,
      pointer: '/dataList/0/content',
      source_kr: '균열이 마침내 완전히 찢어지고, 푸이였을 존재가 그 틈새에서 모습을 드러낸다.',
      target_zh: '裂隙终于彻底撕裂，本应是普伊的存在从中缓缓走出。',
      speaker: '旁白',
      stage: 'Reviewed',
      qa_passed: true,
    },
    {
      id: 10102,
      pointer: '/dataList/1/content',
      source_kr: '그대는 변하였구료, 푸이 군. 이번 잠은… 평안히 주무셨소?',
      target_zh: '……汝变了呢，普伊君。这一觉……睡得可还安稳？',
      speaker: '李箱',
      stage: 'Reviewed',
      qa_passed: true,
    },
    {
      id: 10103,
      pointer: '/dataList/2/content',
      source_kr: '몸 상태가 처음 계약했을 때처럼, 신체 능력이 약해진 것 때문이지?',
      target_zh: '<是因为身体能力衰退得宛如最初签订契约时那般，对吧？>',
      speaker: '但丁',
      stage: 'TmExact',
      qa_passed: true,
    },
    {
      id: 10104,
      pointer: '/skills/0/desc',
      source_kr: '체력이 25% 이하일 때, 3턴 동안 피해량 50% 증가 및 [호흡] 2단계 획득',
      target_zh: '体力在25%以下时，在3回合内伤害量增加50%并自身获得2层[呼吸法] 状态',
      speaker: '技能机制',
      stage: 'AiDrafted',
      qa_passed: true,
    },
  ]);

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100vh', background: '#0d1117' }}>
      {/* 顶部主导航栏 */}
      <header
        style={{
          height: '56px',
          background: '#161b22',
          borderBottom: '1px solid #30363d',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          padding: '0 20px',
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
          <div
            style={{
              width: '32px',
              height: '32px',
              background: '#f8c200',
              borderRadius: '6px',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              fontWeight: 'bold',
              color: '#000',
            }}
          >
            L
          </div>
          <div>
            <div style={{ fontSize: '15px', fontWeight: 600, color: '#f0f6fc' }}>
              边狱公司本地化工程平台 <span style={{ fontSize: '11px', color: '#8b949e', fontWeight: 400 }}>v0.1.0 (Rust Core)</span>
            </div>
            <div style={{ fontSize: '11px', color: '#8b949e' }}>
              活跃工程: <strong style={{ color: '#58a6ff' }}>limbus-s8-p2 (第8赛季下半开荒)</strong> | 零协会基准: <span style={{ color: '#3fb950' }}>LLC-2026092102 (已融合 Part 1)</span>
            </div>
          </div>
        </div>

        {/* 顶部反向台词快速检索框 */}
        <div style={{ position: 'relative', width: '380px' }}>
          <Search size={16} style={{ position: 'absolute', left: '10px', top: '9px', color: '#8b949e' }} />
          <input
            type="text"
            placeholder="毫秒级反向搜实机台词/角色名/关卡 (Ctrl+K)..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            style={{
              width: '100%',
              height: '34px',
              background: '#0d1117',
              border: '1px solid #30363d',
              borderRadius: '6px',
              paddingLeft: '34px',
              paddingRight: '12px',
              color: '#f0f6fc',
              fontSize: '13px',
              outline: 'none',
            }}
          />
        </div>

        {/* 顶部一键工程操作按钮 */}
        <div style={{ display: 'flex', gap: '8px' }}>
          <button
            style={{
              background: '#21262d',
              border: '1px solid #30363d',
              color: '#c9d1d9',
              borderRadius: '6px',
              padding: '6px 12px',
              fontSize: '12px',
              cursor: 'pointer',
              display: 'flex',
              alignItems: 'center',
              gap: '6px',
            }}
          >
            <RefreshCw size={14} /> 检查客户端更新
          </button>
          <button
            style={{
              background: '#238636',
              border: 'none',
              color: '#fff',
              borderRadius: '6px',
              padding: '6px 12px',
              fontSize: '12px',
              fontWeight: 600,
              cursor: 'pointer',
              display: 'flex',
              alignItems: 'center',
              gap: '6px',
            }}
          >
            <Zap size={14} /> 热注入 Steam 游戏
          </button>
          <button
            style={{
              background: '#1f6feb',
              border: 'none',
              color: '#fff',
              borderRadius: '6px',
              padding: '6px 12px',
              fontSize: '12px',
              fontWeight: 600,
              cursor: 'pointer',
              display: 'flex',
              alignItems: 'center',
              gap: '6px',
            }}
          >
            <Download size={14} /> 发布分发包
          </button>
        </div>
      </header>

      {/* 二级功能 Tab 切换栏 */}
      <nav
        style={{
          height: '42px',
          background: '#161b22',
          borderBottom: '1px solid #30363d',
          display: 'flex',
          alignItems: 'center',
          padding: '0 20px',
          gap: '24px',
        }}
      >
        <button
          onClick={() => setActiveTab('dashboard')}
          style={{
            background: 'none',
            border: 'none',
            color: activeTab === 'dashboard' ? '#58a6ff' : '#8b949e',
            borderBottom: activeTab === 'dashboard' ? '2px solid #58a6ff' : 'none',
            height: '100%',
            fontSize: '13px',
            fontWeight: 600,
            cursor: 'pointer',
            display: 'flex',
            alignItems: 'center',
            gap: '6px',
          }}
        >
          <Activity size={16} /> 全局汉化态势大盘
        </button>
        <button
          onClick={() => setActiveTab('editor')}
          style={{
            background: 'none',
            border: 'none',
            color: activeTab === 'editor' ? '#58a6ff' : '#8b949e',
            borderBottom: activeTab === 'editor' ? '2px solid #58a6ff' : 'none',
            height: '100%',
            fontSize: '13px',
            fontWeight: 600,
            cursor: 'pointer',
            display: 'flex',
            alignItems: 'center',
            gap: '6px',
          }}
        >
          <BookOpen size={16} /> 双语对照工作台
        </button>
        <button
          onClick={() => setActiveTab('inbox')}
          style={{
            background: 'none',
            border: 'none',
            color: activeTab === 'inbox' ? '#58a6ff' : '#8b949e',
            borderBottom: activeTab === 'inbox' ? '2px solid #58a6ff' : 'none',
            height: '100%',
            fontSize: '13px',
            fontWeight: 600,
            cursor: 'pointer',
            display: 'flex',
            alignItems: 'center',
            gap: '6px',
          }}
        >
          <AlertTriangle size={16} /> 人工重点审查箱 (1% 裁决)
          <span style={{ background: '#f85149', color: '#fff', borderRadius: '10px', padding: '1px 6px', fontSize: '10px' }}>2</span>
        </button>
        <button
          onClick={() => setActiveTab('audit')}
          style={{
            background: 'none',
            border: 'none',
            color: activeTab === 'audit' ? '#58a6ff' : '#8b949e',
            borderBottom: activeTab === 'audit' ? '2px solid #58a6ff' : 'none',
            height: '100%',
            fontSize: '13px',
            fontWeight: 600,
            cursor: 'pointer',
            display: 'flex',
            alignItems: 'center',
            gap: '6px',
          }}
        >
          <Database size={16} /> AI 审计流水线
        </button>
      </nav>

      {/* 主工作视口 */}
      <main style={{ flex: 1, overflow: 'auto', padding: '20px' }}>
        {activeTab === 'dashboard' && (
          <div>
            {/* 核心指标卡片群 */}
            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: '16px', marginBottom: '24px' }}>
              <div style={{ background: '#161b22', border: '1px solid #30363d', borderRadius: '8px', padding: '16px' }}>
                <div style={{ fontSize: '12px', color: '#8b949e', marginBottom: '4px' }}>总汉化完成度</div>
                <div style={{ fontSize: '28px', fontWeight: 'bold', color: '#3fb950' }}>89.4%</div>
                <div style={{ fontSize: '11px', color: '#8b949e', marginTop: '6px' }}>
                  已完成 12,410 / 总待译 13,880 句段
                </div>
              </div>

              <div style={{ background: '#161b22', border: '1px solid #30363d', borderRadius: '8px', padding: '16px' }}>
                <div style={{ fontSize: '12px', color: '#8b949e', marginBottom: '4px' }}>零协会上游状态</div>
                <div style={{ fontSize: '20px', fontWeight: 'bold', color: '#58a6ff' }}>LLC-2026092102</div>
                <div style={{ fontSize: '11px', color: '#3fb950', marginTop: '8px' }}>
                  ✓ 7,414 权威语料已反哺吸纳为 TM
                </div>
              </div>

              <div style={{ background: '#161b22', border: '1px solid #30363d', borderRadius: '8px', padding: '16px' }}>
                <div style={{ fontSize: '12px', color: '#8b949e', marginBottom: '4px' }}>门禁与健康状况 (QA)</div>
                <div style={{ fontSize: '28px', fontWeight: 'bold', color: '#3fb950' }}>0 FATAL</div>
                <div style={{ fontSize: '11px', color: '#8b949e', marginTop: '6px' }}>
                  0 标签未闭合 · 0 全角波浪号 · 0 极性反转
                </div>
              </div>

              <div style={{ background: '#161b22', border: '1px solid #30363d', borderRadius: '8px', padding: '16px' }}>
                <div style={{ fontSize: '12px', color: '#8b949e', marginBottom: '4px' }}>TM 记忆秒杀率</div>
                <div style={{ fontSize: '28px', fontWeight: 'bold', color: '#f8c200' }}>74.2%</div>
                <div style={{ fontSize: '11px', color: '#8b949e', marginTop: '6px' }}>
                  海量重复 RPG 与通用词条无需人工重翻
                </div>
              </div>
            </div>

            {/* 分模块进度矩阵 */}
            <div style={{ background: '#161b22', border: '1px solid #30363d', borderRadius: '8px', padding: '20px', marginBottom: '24px' }}>
              <div style={{ fontSize: '15px', fontWeight: 600, color: '#f0f6fc', marginBottom: '16px' }}>
                各模块开荒缺口全景
              </div>
              <div style={{ display: 'flex', flexDirection: 'column', gap: '14px' }}>
                <div>
                  <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '12px', marginBottom: '4px' }}>
                    <span>主线剧情 (StoryData: S1017B ~ S1029B)</span>
                    <strong style={{ color: '#3fb950' }}>100.0% (已完全封板)</strong>
                  </div>
                  <div style={{ height: '8px', background: '#21262d', borderRadius: '4px', overflow: 'hidden' }}>
                    <div style={{ width: '100%', height: '100%', background: '#3fb950' }} />
                  </div>
                </div>

                <div>
                  <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '12px', marginBottom: '4px' }}>
                    <span>战斗与被动机制 (Skills & Passives a1c10p2)</span>
                    <strong style={{ color: '#3fb950' }}>98.2% (剩余 4 处微调)</strong>
                  </div>
                  <div style={{ height: '8px', background: '#21262d', borderRadius: '4px', overflow: 'hidden' }}>
                    <div style={{ width: '98.2%', height: '100%', background: '#3fb950' }} />
                  </div>
                </div>

                <div>
                  <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '12px', marginBottom: '4px' }}>
                    <span>RPG 探索系统 (floor-1-b ~ floor-b3-b)</span>
                    <strong style={{ color: '#f8c200' }}>82.5% (TM 自动秒杀 75%，AI 剩余批处理中)</strong>
                  </div>
                  <div style={{ height: '8px', background: '#21262d', borderRadius: '4px', overflow: 'hidden' }}>
                    <div style={{ width: '82.5%', height: '100%', background: '#f8c200' }} />
                  </div>
                </div>
              </div>
            </div>
          </div>
        )}

        {activeTab === 'editor' && (
          <div style={{ display: 'flex', gap: '20px', height: '100%' }}>
            {/* 左侧文件树 */}
            <div
              style={{
                width: '280px',
                background: '#161b22',
                border: '1px solid #30363d',
                borderRadius: '8px',
                padding: '12px',
                display: 'flex',
                flexDirection: 'column',
                gap: '8px',
              }}
            >
              <div style={{ fontSize: '13px', fontWeight: 600, color: '#f0f6fc', marginBottom: '8px' }}>
                待办资产文件列表 (93 个新增)
              </div>
              <div style={{ fontSize: '12px', padding: '6px 10px', background: '#21262d', borderRadius: '4px', cursor: 'pointer', color: '#58a6ff' }}>
                📄 StoryData/S1029B.json (100%)
              </div>
              <div style={{ fontSize: '12px', padding: '6px 10px', borderRadius: '4px', cursor: 'pointer', color: '#c9d1d9' }}>
                📄 RPGSystem/rpg-loc-dialogue-floor-1-b.json (84%)
              </div>
              <div style={{ fontSize: '12px', padding: '6px 10px', borderRadius: '4px', cursor: 'pointer', color: '#c9d1d9' }}>
                📄 Skills_Abnormality-a1c10p2.json (98%)
              </div>
            </div>

            {/* 右侧条目列表 */}
            <div style={{ flex: 1, display: 'flex', flexDirection: 'column', gap: '12px' }}>
              {segments.map((seg) => (
                <div
                  key={seg.id}
                  style={{
                    background: '#161b22',
                    border: '1px solid #30363d',
                    borderRadius: '8px',
                    padding: '14px',
                    display: 'flex',
                    flexDirection: 'column',
                    gap: '8px',
                  }}
                >
                  <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', fontSize: '11px', color: '#8b949e' }}>
                    <span>#{seg.id} 路径: {seg.pointer}</span>
                    <span
                      style={{
                        padding: '2px 8px',
                        borderRadius: '4px',
                        background: seg.stage === 'Reviewed' ? '#238636' : seg.stage === 'TmExact' ? '#1f6feb' : '#8957e5',
                        color: '#fff',
                        fontWeight: 600,
                      }}
                    >
                      {seg.stage}
                    </span>
                  </div>

                  {seg.speaker && (
                    <div style={{ fontSize: '12px', color: '#f8c200', fontWeight: 600 }}>
                      说话人: {seg.speaker}
                    </div>
                  )}

                  <div style={{ fontSize: '13px', color: '#8b949e', background: '#0d1117', padding: '8px 12px', borderRadius: '4px' }}>
                    <strong>KR:</strong> {seg.source_kr}
                  </div>

                  <div style={{ fontSize: '13px', color: '#f0f6fc', background: '#1c2128', padding: '8px 12px', borderRadius: '4px', border: '1px solid #30363d' }}>
                    <strong>ZH:</strong> {seg.target_zh}
                  </div>
                </div>
              ))}
            </div>
          </div>
        )}

        {activeTab === 'inbox' && (
          <div style={{ background: '#161b22', border: '1px solid #30363d', borderRadius: '8px', padding: '20px' }}>
            <div style={{ fontSize: '15px', fontWeight: 600, color: '#f85149', marginBottom: '12px', display: 'flex', alignItems: 'center', gap: '8px' }}>
              <AlertTriangle size={18} /> 人工裁决箱 (仅聚焦争议与潜在分歧，无需翻阅 99% 的普通文本)
            </div>
            <div style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
              <div style={{ background: '#0d1117', border: '1px solid #f85149', borderRadius: '6px', padding: '14px' }}>
                <div style={{ fontSize: '12px', color: '#f85149', fontWeight: 600, marginBottom: '6px' }}>
                  【良秀台词缩写争议】同形异义词预警
                </div>
                <div style={{ fontSize: '13px', color: '#8b949e', marginBottom: '4px' }}>
                  原文: 시. 대. 네놈의 기대와 달리… 나는 어떤 기억도 가지고 있지 않다.
                </div>
                <div style={{ fontSize: '13px', color: '#f0f6fc', marginBottom: '8px' }}>
                  当前译文: 呼……钟·头。与你的期待相反……我没有任何记忆。
                </div>
                <div style={{ display: 'flex', gap: '8px' }}>
                  <button style={{ background: '#238636', border: 'none', color: '#fff', borderRadius: '4px', padding: '4px 10px', fontSize: '12px', cursor: 'pointer' }}>
                    确认裁决并锁定
                  </button>
                  <button style={{ background: '#21262d', border: '1px solid #30363d', color: '#c9d1d9', borderRadius: '4px', padding: '4px 10px', fontSize: '12px', cursor: 'pointer' }}>
                    人工修改
                  </button>
                </div>
              </div>
            </div>
          </div>
        )}

        {activeTab === 'audit' && (
          <div style={{ background: '#161b22', border: '1px solid #30363d', borderRadius: '8px', padding: '20px' }}>
            <div style={{ fontSize: '15px', fontWeight: 600, color: '#f0f6fc', marginBottom: '14px' }}>
              AI 智能体操作审计流水 (Audit Trail)
            </div>
            <div style={{ display: 'flex', flexDirection: 'column', gap: '10px', fontSize: '12px' }}>
              <div style={{ padding: '8px 12px', background: '#0d1117', borderRadius: '4px', display: 'flex', justifyContent: 'space-between' }}>
                <span>🤖 AI 批量完成了 S1029B.json (24 句，耗时 12s)</span>
                <span style={{ color: '#3fb950' }}>100% 通过门禁</span>
              </div>
              <div style={{ padding: '8px 12px', background: '#0d1117', borderRadius: '4px', display: 'flex', justifyContent: 'space-between' }}>
                <span>⚡ TM 引擎自动秒杀 floor-1-b 320 条相同 RPG 文本</span>
                <span style={{ color: '#58a6ff' }}>100% 自动对齐</span>
              </div>
              <div style={{ padding: '8px 12px', background: '#0d1117', borderRadius: '4px', display: 'flex', justifyContent: 'space-between' }}>
                <span>📦 零协会 Part 1 官方库知识反哺完成 (7,414 项)</span>
                <span style={{ color: '#f8c200' }}>已设为 Gold 优先级</span>
              </div>
            </div>
          </div>
        )}
      </main>
    </div>
  );
}
