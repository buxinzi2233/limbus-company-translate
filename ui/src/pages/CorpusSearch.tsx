import { useState } from 'react';
import { ReferenceResults } from '../components/ReferenceSearch';
import { ArrowRight, Search, Copy } from 'lucide-react';
import { searchSchema, stageLabels } from '../api';
import { useResource } from '../useResource';
import { Feedback } from '../components/Feedback';

export function CorpusSearch({ revision }: { revision: number }) {
  const [query, setQuery] = useState('');
  const [submitted, setSubmitted] = useState('');
  const [attempt, setAttempt] = useState(0);
  const [notice, setNotice] = useState('');
  const { data, loading, error } = useResource(
    submitted
      ? `/api/search?q=${encodeURIComponent(submitted)}&limit=50`
      : null,
    searchSchema,
    revision + attempt,
  );
  const copy = async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setNotice('译文已复制，可返回工作台粘贴。');
    } catch (error) {
      setNotice(`复制失败，请手动选择译文：${String(error)}`);
    }
  };
  return (
    <div className="page-enter">
      <div className="page-intro">
        <span className="eyebrow">CORPUS & TRANSLATION MEMORY</span>
        <h2>找到语言留下的线索。</h2>
        <p>
          统一检索项目词条、翻译记忆与 Wiki
          参考，区分已存译文和未确认的社区用语。
        </p>
      </div>
      <form
        className="corpus-form"
        onSubmit={(event) => {
          event.preventDefault();
          if (query.trim()) {
            setSubmitted(query.trim());
            setAttempt(attempt + 1);
          }
        }}
      >
        <Search size={24} />
        <input
          data-testid="corpus-query"
          aria-label="检索语料与翻译记忆"
          placeholder="输入中文、韩文或角色名…"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
        />
        <button
          className="solid-button"
          data-testid="corpus-search"
          disabled={loading || !query.trim()}
        >
          检索 <ArrowRight size={18} />
        </button>
      </form>
      <div className="search-hint">
        <span>项目词条 + TM 翻译记忆 + Wiki 参考</span>
        <span>项目/TM 各 50 条；Wiki 名称/正文各 8 条</span>
      </div>
      {notice && (
        <p role="status" className="inline-message">
          {notice}
        </p>
      )}
      <Feedback
        loading={loading}
        error={error}
        empty={
          !submitted
            ? '从一个词、一句台词开始'
            : data?.total_found === 0
              ? '项目词条与 TM 中没有匹配；Wiki 参考见下方'
              : null
        }
      />
      <ReferenceResults query={submitted} id="corpus" />
      {data && data.total_found > 0 && (
        <div className="search-columns">
          <section>
            <div className="section-title">
              <h3>
                项目词条 <small>{data.segments.length}</small>
              </h3>
              <span className="eyebrow">PROJECT</span>
            </div>
            {data.segments.map((item) => (
              <article className="search-result" key={item.segment_id}>
                <div className="result-meta">
                  <span>
                    #{item.segment_id} · {item.speaker ?? '未标注'}
                  </span>
                  <span>
                    {item.stage === 'Reviewed' && !item.review_confirmed
                      ? '历史译文待复核'
                      : stageLabels[item.stage]}
                  </span>
                </div>
                <p lang="ko">{item.source_kr}</p>
                <p className="result-translation">
                  {item.target_zh ?? '尚未翻译'}
                </p>
                <code>
                  {item.relative_path}
                  <br />
                  {item.key_path}
                </code>
              </article>
            ))}
            {data.segments.length === 0 && (
              <p className="muted">项目中暂无匹配。</p>
            )}
          </section>
          <section>
            <div className="section-title">
              <h3>
                翻译记忆 <small>{data.tm_entries.length}</small>
              </h3>
              <span className="eyebrow">MEMORY</span>
            </div>
            {data.tm_entries.map((item) => (
              <article className="search-result" key={item.id}>
                <div className="result-meta">
                  <span>{item.source_origin}</span>
                  <button
                    className="icon-button"
                    data-testid={`copy-tm-${item.id}`}
                    aria-label="复制记忆库译文"
                    title="复制译文"
                    onClick={() => copy(item.target_zh)}
                  >
                    <Copy size={16} />
                  </button>
                </div>
                <p lang="ko">{item.source_kr}</p>
                <p className="result-translation">{item.target_zh}</p>
                <code>
                  {item.file_scope ?? '全局语料'} · 优先级 {item.priority}
                </code>
              </article>
            ))}
            {data.tm_entries.length === 0 && (
              <p className="muted">翻译记忆中暂无匹配。</p>
            )}
          </section>
        </div>
      )}
    </div>
  );
}
