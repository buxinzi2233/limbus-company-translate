import { useEffect, useState } from 'react';
import {
  ArrowUpRight,
  BookOpen,
  CircleDot,
  Files,
  ScanLine,
  RefreshCw,
  History,
  SlidersHorizontal,
} from 'lucide-react';
import { statusSchema, type Draft, type Segment } from './api';
import { useResource } from './useResource';
import { Feedback } from './components/Feedback';
import { Overview } from './pages/Overview';
import { Workbench } from './pages/Workbench';
import { Focus } from './pages/Focus';
import { CorpusSearch } from './pages/CorpusSearch';
import { Audit } from './pages/Audit';
import './styles.css';

const pages = [
  { id: 'overview', name: '项目总览', english: 'OVERVIEW', icon: CircleDot },
  { id: 'workbench', name: '双语工作台', english: 'WORKSPACE', icon: BookOpen },
  { id: 'search', name: '语料检索', english: 'CORPUS SEARCH', icon: Files },
  {
    id: 'audit',
    name: '来源与审计',
    english: 'PROVENANCE & AUDIT',
    icon: History,
  },
  { id: 'focus', name: '待审队列', english: 'FOCUSED REVIEW', icon: ScanLine },
] as const;
function readPage(): string {
  return window.location.hash.slice(1) || 'overview';
}

export default function App() {
  const [page, setPage] = useState(readPage);
  const [reducedMotion, setReducedMotion] = useState(false);
  const [revision, setRevision] = useState(0);
  const [drafts, setDrafts] = useState<Readonly<Record<number, Draft>>>({});
  const { data, loading, error } = useResource(
    '/api/status',
    statusSchema,
    revision,
  );
  useEffect(() => {
    if (Object.keys(drafts).length === 0) return;
    const warn = (event: BeforeUnloadEvent) => {
      event.preventDefault();
      event.returnValue = '';
    };
    window.addEventListener('beforeunload', warn);
    return () => window.removeEventListener('beforeunload', warn);
  }, [drafts]);
  const current = pages.find((item) => item.id === page);
  useEffect(() => {
    const navigate = () => {
      if (window.location.hash !== '#main-content') {
        setPage(readPage());
        window.scrollTo({ top: 0 });
      }
    };
    window.addEventListener('hashchange', navigate);
    return () => window.removeEventListener('hashchange', navigate);
  }, []);
  const editorProps = {
    drafts,
    revision,
    onDraft: (id: number, text: string, updatedAt: string) =>
      setDrafts((previous) => ({
        ...previous,
        [id]: { text, updatedAt: previous[id]?.updatedAt ?? updatedAt },
      })),
    onDiscard: (id: number) =>
      setDrafts((previous) =>
        Object.fromEntries(
          Object.entries(previous).filter(([key]) => Number(key) !== id),
        ),
      ),
    onSaved: (saved: Segment) => {
      setDrafts((previous) =>
        Object.fromEntries(
          Object.entries(previous).filter(
            ([key, draft]) =>
              Number(key) !== saved.id || draft.text !== saved.target_zh,
          ),
        ),
      );
      setRevision((previous) => previous + 1);
    },
  };
  return (
    <div className="app-shell" data-motion={reducedMotion ? 'reduced' : 'full'}>
      <a className="skip-link" href="#main-content">
        跳至主要内容
      </a>
      <aside className="sidebar">
        <a className="brand" href="#overview" aria-label="LCT 项目总览">
          <span className="brand-mark" aria-hidden="true">
            <i />
            <i />
            <i />
          </span>
          <span>
            LCT<span className="brand-sub">LOCALIZATION ENGINE</span>
          </span>
        </a>
        <div className="sidebar-rule" />
        <div className="sidebar-label">工作区 / WORKSPACE</div>
        <nav aria-label="主要导航">
          {pages.map((item, index) => (
            <a
              key={item.id}
              href={`#${item.id}`}
              data-testid={`nav-${item.id}`}
              aria-label={item.name}
              className={page === item.id ? 'nav-item active' : 'nav-item'}
              aria-current={page === item.id ? 'page' : undefined}
            >
              <item.icon size={19} />
              <span>
                {item.name}
                <small>{item.english}</small>
              </span>
              <span className="nav-index">0{index + 1}</span>
            </a>
          ))}
        </nav>
        <div className="sidebar-bottom">
          <div className="sidebar-art" aria-hidden="true">
            <span />
            <span />
            <span />
          </div>
          <span className="eyebrow">PRECISION IN EVERY WORD.</span>
          <p>让语言，延续意义。</p>
          <div className="edition">
            莱茵生命美学研究 · 原创界面
            <br />
            非官方作品 / LOCAL EDITION
          </div>
        </div>
      </aside>
      <div className="workspace">
        <header className="topbar">
          <div className="breadcrumb">
            工作区 <span>/</span>{' '}
            <strong>{current?.name ?? '页面未找到'}</strong>
          </div>
          <div className="topbar-controls">
            <button
              className="motion-toggle"
              data-testid="motion-toggle"
              aria-pressed={reducedMotion}
              title="本次使用减少动态效果；始终尊重系统减少动态效果设置"
              onClick={() => setReducedMotion((previous) => !previous)}
            >
              <SlidersHorizontal size={14} />
              减少动态效果
            </button>
            <div className="connection">
              <span
                className={`connection-dot ${error ? 'offline' : loading ? 'pending' : ''}`}
              />
              {error ? '连接异常' : loading ? '正在连接' : '本地服务已连接'}
            </div>
          </div>
        </header>
        <main id="main-content" tabIndex={-1}>
          <div className="project-heading">
            <div>
              <span className="eyebrow">
                {data?.project?.id ?? 'LOCAL WORKSPACE'}
              </span>
              <h1>
                {data?.project?.name ??
                  (loading ? '正在连接工作区' : '本地化工作区')}
              </h1>
            </div>
            <button
              className="refresh-button"
              data-testid="refresh"
              onClick={() => setRevision((previous) => previous + 1)}
              disabled={loading}
            >
              <RefreshCw size={15} className={loading ? 'spin' : ''} />
              <span>刷新数据</span>
            </button>
          </div>
          {page === 'overview' && (
            <>
              <Feedback loading={loading} error={error} empty={null} />
              {data && <Overview data={data} />}
            </>
          )}
          <div hidden={page !== 'workbench'}>
            {data && (
              <Workbench
                {...editorProps}
                hasTaskScope={data.task_scope !== null}
              />
            )}
          </div>
          <div hidden={page !== 'search'}>
            <CorpusSearch revision={revision} />
          </div>
          <div hidden={page !== 'focus'}>
            {data && (
              <Focus {...editorProps} hasTaskScope={data.task_scope !== null} />
            )}
          </div>
          <div hidden={page !== 'audit'}>
            <Audit revision={revision} />
          </div>
          {!current && (
            <div className="feedback">
              <h2>没有找到这个页面</h2>
              <a href="#overview">
                返回项目总览 <ArrowUpRight size={16} />
              </a>
            </div>
          )}
        </main>
        <footer className="site-footer">
          <span>LCT / 本地化工程工作站</span>
          <span>本地资料 · 人工审校 · 精确表达</span>
          <span>{data?.version ?? '连接待确认'}</span>
        </footer>
      </div>
    </div>
  );
}
