import { useState } from 'react';
import { z } from 'zod';
import { useResource } from '../useResource';
import { Feedback } from './Feedback';
import { TermAdvice, TermEvidence, termLinkSchema } from './TermAdvice';

const wikiUrl = z.url().refine((value) => {
  const url = new URL(value);
  return (
    url.protocol === 'https:' && url.hostname === 'limbuscompany.huijiwiki.com'
  );
});
const sourceSchema = z.object({
  title: z.string(),
  url: wikiUrl,
  revision_id: z.number().int().positive(),
  revision_at: z.string(),
  fetched_at: z.string(),
  attribution: z.string(),
  history_url: wikiUrl,
  license_url: z.literal('https://creativecommons.org/licenses/by-sa/4.0/'),
  license_basis: z.string(),
  notices: z.array(z.string()),
});
const matchSchema = z.object({
  kind: z.enum(['term_candidate', 'passage']),
  name: z.string(),
  text: z.string(),
  review_status: z.enum(['unreviewed', 'reference_only']),
  source: sourceSchema,
});
const resultsSchema = z.object({
  query: z.string(),
  name_links: z.array(termLinkSchema),
  terms: z.array(matchSchema),
  passages: z.array(matchSchema),
  warning: z.string(),
});
const statusSchema = z.object({
  configured: z.boolean(),
  captured_pages: z.number().int().nonnegative(),
  searchable_pages: z.number().int().nonnegative(),
  candidate_names: z.number().int().nonnegative(),
  passages: z.number().int().nonnegative(),
  incomplete_pages: z.array(z.string()),
});

export function ReferenceResults({ query, id }: { query: string; id: string }) {
  const status = useResource('/api/references/status', statusSchema, 0);
  const results = useResource(
    status.data?.configured && query.trim()
      ? `/api/references/search?q=${encodeURIComponent(query.trim())}&limit=8`
      : null,
    resultsSchema,
    0,
  );
  const data = query.trim() === results.data?.query ? results.data : null;
  return (
    <section
      className="reference-results"
      data-testid={`references-${id}`}
      aria-label="Wiki 翻译参考"
    >
      <div className="section-title">
        <h3>Wiki 翻译参考</h3>
        <span className="eyebrow">REFERENCE · NOT TM</span>
      </div>
      <p className="muted">
        社区用语与语境参考，未人工确认；仅单独列出的名称关联经过字段身份核对，不是双语
        TM。
      </p>
      <Feedback loading={status.loading} error={status.error} empty={null} />
      {status.data && !status.data.configured && (
        <p role="status">
          未配置参考库，请使用 --reference-db 与明确的 --project-id 启动 LCT。
        </p>
      )}
      {status.data?.configured && (
        <p className="muted" data-testid={`reference-coverage-${id}`}>
          {status.data.searchable_pages} 个可检索页面 ·{' '}
          {status.data.candidate_names} 个名称候选
          {status.data.incomplete_pages.length > 0 &&
            ` · 目录页未直接索引：${status.data.incomplete_pages.join('、')}`}
        </p>
      )}
      {status.data?.configured && query.trim() && (
        <Feedback
          loading={results.loading}
          error={results.error}
          empty={
            data &&
            data.name_links.length +
              data.terms.length +
              data.passages.length ===
              0
              ? 'Wiki 参考中没有匹配，尝试中文候选译名或更短的片段。'
              : null
          }
        />
      )}
      {data && !results.error && (
        <div data-testid={`reference-matches-${id}`}>
          {data.name_links.map((link) => (
            <article className="search-result" key={link.keyword_id}>
              <TermEvidence link={link} />
            </article>
          ))}
          {[...data.terms, ...data.passages].map((item, index) => (
            <article className="search-result" key={`${item.kind}-${index}`}>
              <div className="result-meta">
                <strong>{item.name}</strong>
                <span>
                  {item.kind === 'term_candidate'
                    ? '名称候选 · 未确认'
                    : '正文参考 · 非译文'}
                </span>
              </div>
              <p style={{ whiteSpace: 'pre-wrap' }}>{item.text}</p>
              <p>
                <a href={item.source.url} target="_blank" rel="noreferrer">
                  {item.source.title}
                </a>{' '}
                · 修订 {item.source.revision_id}
              </p>
              <p className="muted">
                {item.source.attribution} ·{' '}
                <a
                  href={item.source.history_url}
                  target="_blank"
                  rel="noreferrer"
                >
                  作者历史
                </a>{' '}
                ·{' '}
                <a
                  href={item.source.license_url}
                  target="_blank"
                  rel="noreferrer"
                >
                  CC BY-SA 4.0（另有声明除外）
                </a>
              </p>
              <small>
                采集：{item.source.fetched_at} · HTML
                转文本及分段；模板展开以采集时状态为准
              </small>
              {item.source.notices.map((notice, index) => (
                <p className="inline-message" key={index}>
                  {notice}
                </p>
              ))}
            </article>
          ))}
        </div>
      )}
    </section>
  );
}

export function ReferenceSearch({
  id,
  context,
  segmentId,
  targetText,
}: {
  id: string;
  context: string;
  segmentId: number;
  targetText: string;
}) {
  const [input, setInput] = useState('');
  const [query, setQuery] = useState('');
  return (
    <aside className="reference-panel" data-testid={`reference-panel-${id}`}>
      <p className="muted">当前词条：{context}</p>
      <form
        className="toolbar"
        onSubmit={(event) => {
          event.preventDefault();
          setQuery(input.trim());
        }}
      >
        <div className="search-field">
          <input
            data-testid={`reference-query-${id}`}
            aria-label="查询当前词条的 Wiki 参考"
            placeholder="中文候选译名或短语，例如：呼吸法"
            value={input}
            maxLength={120}
            onChange={(event) => setInput(event.target.value)}
          />
          <button
            data-testid={`reference-search-${id}`}
            disabled={!input.trim()}
          >
            检索参考
          </button>
        </div>
      </form>
      <ReferenceResults id={id} query={query} />
      <TermAdvice segmentId={segmentId} targetText={targetText} />
    </aside>
  );
}
