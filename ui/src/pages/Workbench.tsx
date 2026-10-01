import {
  CategoryFilter,
  categoryLabels,
  type Category,
} from '../components/CategoryFilter';
import { Fragment, useEffect, useState } from 'react';
import { ReferenceSearch } from '../components/ReferenceSearch';
import { ArrowLeft, ArrowRight, Search } from 'lucide-react';
import { formatCount, segmentsSchema, type Draft, type Segment } from '../api';
import { useResource } from '../useResource';
import { Feedback } from '../components/Feedback';
import { TranslationEditor } from '../components/TranslationEditor';

export type DraftProps = {
  drafts: Readonly<Record<number, Draft>>;
  onDraft: (id: number, text: string, updatedAt: string) => void;
  onDiscard: (id: number) => void;
  onSaved: (item: Segment) => void;
  revision: number;
};
export function Workbench({
  drafts,
  onDraft,
  onDiscard,
  onSaved,
  revision,
  hasTaskScope,
}: DraftProps & { hasTaskScope: boolean }) {
  const [category, setCategory] = useState<Category>('all');
  const [referenceId, setReferenceId] = useState<number | null>(null);
  const [page, setPage] = useState(1);
  const [selection, setSelection] = useState(
    hasTaskScope ? 'task' : 'inventory',
  );
  const [stage, setStage] = useState('all');
  const [query, setQuery] = useState('');
  const [search, setSearch] = useState('');
  const params = new URLSearchParams({
    page: String(page),
    page_size: '25',
    selection,
  });
  if (category !== 'all') params.set('category', category);
  if (stage !== 'all') params.set('stage', stage);
  if (search) params.set('search', search);
  const { data, loading, error } = useResource(
    `/api/segments?${params}`,
    segmentsSchema,
    revision,
  );
  const pages = Math.max(1, Math.ceil((data?.total ?? 0) / 25));
  useEffect(() => {
    if (data && !loading && page > pages) setPage(pages);
  }, [data, loading, page, pages]);
  return (
    <div className="page-enter">
      <div className="page-intro">
        <span className="eyebrow">BILINGUAL WORKSPACE</span>
        <h2>在原文与译文之间。</h2>
        <p>逐条对照、保留语境，让每一次审校有据可循。</p>
      </div>
      <CategoryFilter
        id="workbench"
        value={category}
        onChange={(next) => {
          setCategory(next);
          setPage(1);
          setReferenceId(null);
        }}
      />
      <form
        className="toolbar"
        onSubmit={(event) => {
          event.preventDefault();
          setSearch(query.trim());
          setPage(1);
        }}
      >
        <select
          data-testid="scope-selection"
          aria-label="词条范围"
          value={selection}
          onChange={(event) => {
            setSelection(event.target.value);
            setPage(1);
            setReferenceId(null);
          }}
        >
          <option value="task" disabled={!hasTaskScope}>
            本次任务范围
          </option>
          <option value="inventory">全量原文库存</option>
        </select>
        <div className="search-field">
          <Search size={18} />
          <input
            data-testid="workbench-query"
            aria-label="搜索项目词条"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="搜索原文或译文…"
          />
          <button data-testid="workbench-search" type="submit">
            查找 <ArrowRight size={16} />
          </button>
        </div>
        <select
          data-testid="stage-filter"
          aria-label="词条阶段"
          value={stage}
          onChange={(event) => {
            setStage(event.target.value);
            setPage(1);
          }}
        >
          <option value="all">全部阶段</option>
          <option value="0">待翻译</option>
          <option value="1">记忆库匹配</option>
          <option value="2">原文变更</option>
          <option value="3">AI 草稿</option>
          <option value="4">已审校</option>
          <option value="5">导入译文待审</option>
          <option value="9">上游锁定</option>
        </select>
      </form>
      <div className="results-heading">
        <span>
          {!loading && data
            ? `${selection === 'task' ? '本次任务' : '全量库存'} · ${categoryLabels[category]} · ${formatCount(data.total)} 条记录`
            : '项目词条'}
        </span>
        <span className="eyebrow">SOURCE / TRANSLATION</span>
      </div>
      <Feedback
        loading={loading}
        error={error}
        empty={data?.items.length === 0 ? '没有符合条件的词条' : null}
      />
      {!loading &&
        data?.items.map((item) => {
          return (
            <Fragment key={item.id}>
              <TranslationEditor
                editorId={`workbench-${item.id}`}
                item={item}
                draftValue={drafts[item.id]}
                inputRef={null}
                onDiscard={onDiscard}
                onDraft={onDraft}
                onSaved={onSaved}
              />
              <button
                className="outline-button"
                data-testid={`reference-toggle-${item.id}`}
                aria-expanded={referenceId === item.id}
                onClick={() =>
                  setReferenceId(referenceId === item.id ? null : item.id)
                }
              >
                {referenceId === item.id ? '收起 Wiki 参考' : '查看 Wiki 参考'}
              </button>
              {referenceId === item.id && (
                <ReferenceSearch
                  key={item.id}
                  id={`segment-${item.id}`}
                  segmentId={item.id}
                  targetText={drafts[item.id]?.text ?? item.target_zh ?? ''}
                  context={`#${item.id} · ${item.relative_path} · ${item.key_path}`}
                />
              )}
            </Fragment>
          );
        })}
      <div className="pagination">
        <button
          data-testid="page-prev"
          className="outline-button"
          disabled={loading || page <= 1}
          onClick={() => setPage(page - 1)}
        >
          <ArrowLeft size={16} />
          上一页
        </button>
        <span>
          第 {page} / {pages} 页
        </span>
        <button
          data-testid="page-next"
          className="outline-button"
          disabled={loading || page >= pages}
          onClick={() => setPage(page + 1)}
        >
          下一页
          <ArrowRight size={16} />
        </button>
      </div>
    </div>
  );
}
