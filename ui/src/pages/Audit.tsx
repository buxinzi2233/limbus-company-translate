import {
  CategoryFilter,
  categoryLabels,
  type Category,
} from '../components/CategoryFilter';
import { useState } from 'react';
import { auditSchema } from '../api';
import { useResource } from '../useResource';
import { Feedback } from '../components/Feedback';

export function Audit({ revision }: { revision: number }) {
  const [category, setCategory] = useState<Category>('all');
  const [operationsBefore, setOperationsBefore] = useState<number | null>(null);
  const [before, setBefore] = useState<number | null>(null);
  const [importsBefore, setImportsBefore] = useState<number | null>(null);
  const [corpusOffset, setCorpusOffset] = useState(0);
  const params = new URLSearchParams({ corpus_offset: String(corpusOffset) });
  if (category !== 'all') params.set('category', category);
  if (operationsBefore !== null)
    params.set('operations_before', String(operationsBefore));
  if (before !== null) params.set('before', String(before));
  if (importsBefore !== null)
    params.set('imports_before', String(importsBefore));
  const { data, loading, error } = useResource(
    `/api/audit?${params}`,
    auditSchema,
    revision,
  );
  return (
    <div className="page-enter audit-page" data-testid="audit-page">
      <div className="page-intro">
        <span className="eyebrow">PROVENANCE & REVIEW HISTORY</span>
        <h2>每一次改写，有迹可循。</h2>
        <p>
          AI 草稿通过 QA 不等于人工审校；记录说明操作入口，不证明操作者身份。
        </p>
      </div>
      <CategoryFilter
        id="audit"
        value={category}
        onChange={(next) => {
          setCategory(next);
          setBefore(null);
        }}
      />
      <p className="muted" data-testid="audit-category-note">
        分区筛选适用于译文提交记录，按关联资源的当前分类查询；原文快照、共享 TM
        来源与整批文件操作保持跨分区展示。
      </p>
      <Feedback loading={loading} error={error} empty={null} />
      {data && (
        <>
          <section className="search-result">
            <h3>当前项目 · {data.project.name}</h3>
            <p>
              韩文来源：
              <code>
                {data.project.live_client_kr_dir ??
                  '尚未记录，历史数据不能据此推断来源'}
              </code>
            </p>
            <p>
              中文参考目录：
              <code>{data.project.workspace_zh_dir ?? '尚未记录'}</code>
            </p>
            <p className="muted">
              CLI 执行导入；此页只读。历史版本未记录的操作不会被补造。
            </p>
          </section>
          <section>
            <div className="section-title">
              <h3>原文快照导入</h3>
              <span>当前项目 · 每页最多 20 次</span>
            </div>
            {!data.imports.length && <p className="muted">尚无导入审计记录</p>}
            {data.imports.map((item) => (
              <article className="search-result" key={item.id}>
                <p>
                  <time>{item.created_at}</time>
                </p>
                <p>
                  韩文：<code>{item.source_directory}</code>
                </p>
                <p>
                  中文：<code>{item.translation_directory}</code>
                </p>
                <p>
                  当前 {item.active_segments} 条 · 新增或恢复{' '}
                  {item.added_or_reactivated} 条 · 原文变化 {item.changed} 条 ·
                  退出 {item.retired} 条
                </p>
              </article>
            ))}
            <div className="pagination">
              <button
                className="text-button"
                data-testid="audit-imports-first"
                disabled={loading || importsBefore === null}
                onClick={() => setImportsBefore(null)}
              >
                最新导入
              </button>
              <button
                className="outline-button"
                data-testid="audit-imports-next"
                disabled={loading || data.next_imports === null}
                onClick={() => setImportsBefore(data.next_imports)}
              >
                更早导入
              </button>
            </div>
          </section>
          <section>
            <div className="section-title">
              <h3>翻译记忆来源</h3>
              <span>数据库内共享，非本项目专属</span>
            </div>
            {!data.corpora.length && <p className="muted">尚无翻译记忆来源</p>}
            {data.corpora.map((source) => (
              <article className="search-result" key={source.source_origin}>
                <h4>{source.source_origin}</h4>
                <p>{source.entries} 条语料 · 来源名称由导入方声明</p>
              </article>
            ))}
            <div className="pagination">
              <button
                className="text-button"
                data-testid="audit-corpus-first"
                disabled={loading || corpusOffset === 0}
                onClick={() => setCorpusOffset(0)}
              >
                首批来源
              </button>
              <button
                className="outline-button"
                data-testid="audit-corpus-next"
                disabled={loading || data.next_corpora === null}
                onClick={() => {
                  if (data.next_corpora !== null)
                    setCorpusOffset(data.next_corpora);
                }}
              >
                更多来源
              </button>
            </div>
          </section>
          <section>
            <div className="section-title">
              <h3>译文提交记录 · {categoryLabels[category]}</h3>
              <span>当前项目 · 每页最多 20 次</span>
            </div>
            {!data.events.length && (
              <p className="muted">尚无译文提交审计记录</p>
            )}
            {data.events.map((event) => (
              <article
                className="search-result"
                data-testid={`audit-event-${event.id}`}
                key={event.id}
              >
                <h4>
                  词条 #{event.segment_id} ·{' '}
                  {event.kind === 'ai_draft'
                    ? 'AI 草稿，尚未审校'
                    : '审校入口提交'}
                </h4>
                <p className="muted">
                  {event.created_at} ·{' '}
                  {event.kind === 'ai_draft'
                    ? `声明的生产者：${event.producer}`
                    : '本机审校接口；不构成人员身份认证'}
                </p>
                <details>
                  <summary data-testid={`audit-details-${event.id}`}>
                    查看该次修改
                  </summary>
                  <p lang="ko">原文：{event.source_kr}</p>
                  <p>修改前：{event.previous_target ?? '无译文'}</p>
                  <p>提交内容：{event.target_zh}</p>
                </details>
              </article>
            ))}
            <div className="pagination">
              <button
                className="text-button"
                data-testid="audit-events-first"
                disabled={loading || before === null}
                onClick={() => setBefore(null)}
              >
                最新修改
              </button>
              <button
                className="outline-button"
                data-testid="audit-events-next"
                disabled={loading || data.next_events === null}
                onClick={() => setBefore(data.next_events)}
              >
                更早修改
              </button>
            </div>
          </section>
          <section>
            <div className="section-title">
              <h3>文件输出与恢复</h3>
              <span>成功操作回执 · 每页最多 20 次</span>
            </div>
            {!data.operations.length && (
              <p className="muted">尚无文件操作回执</p>
            )}
            {data.operations.map((operation) => (
              <article className="search-result" key={operation.id}>
                <h4>
                  {operation.kind === 'inject' ? '注入完成' : '恢复完成'} ·{' '}
                  {operation.affected_files} 个文件
                </h4>
                <p>
                  {operation.created_at} ·{' '}
                  {operation.project_id === null
                    ? '未绑定项目：外部工单或历史备份（数据库内共享记录）'
                    : `项目：${operation.project_id}`}
                </p>
                <p>
                  输出目录：<code>{operation.workspace}</code>
                </p>
                <p>
                  备份清单目录：<code>{operation.backup_directory}</code>
                </p>
              </article>
            ))}
            <div className="pagination">
              <button
                className="text-button"
                data-testid="audit-operations-first"
                disabled={loading || operationsBefore === null}
                onClick={() => setOperationsBefore(null)}
              >
                最新文件操作
              </button>
              <button
                className="outline-button"
                data-testid="audit-operations-next"
                disabled={loading || data.next_operations === null}
                onClick={() => setOperationsBefore(data.next_operations)}
              >
                更早文件操作
              </button>
            </div>
          </section>
          <p className="review-note">
            回执只证明某次操作已成功，不证明文件之后未被外部修改；中断或回执记录失败时，以命令错误、操作锁和备份清单为准。
          </p>
        </>
      )}
    </div>
  );
}
