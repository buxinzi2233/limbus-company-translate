import {
  CategoryFilter,
  categoryLabels,
  type Category,
} from '../components/CategoryFilter';
import { useEffect, useRef, useState } from 'react';
import { ArrowLeft, ArrowRight } from 'lucide-react';
import { segmentsSchema } from '../api';
import { useResource } from '../useResource';
import { Feedback } from '../components/Feedback';
import { TranslationEditor } from '../components/TranslationEditor';
import type { DraftProps } from './Workbench';

export function Focus({
  drafts,
  onDraft,
  onDiscard,
  onSaved,
  revision,
  hasTaskScope,
}: DraftProps & { hasTaskScope: boolean }) {
  const [category, setCategory] = useState<Category>('all');
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const restoreFocus = useRef(false);
  const [batch, setBatch] = useState(0);
  const [index, setIndex] = useState(0);
  const [completed, setCompleted] = useState<Readonly<Record<number, string>>>(
    {},
  );
  const params = new URLSearchParams({
    selection: hasTaskScope ? 'task' : 'inventory',
  });
  if (category !== 'all') params.set('category', category);
  const { data, loading, error } = useResource(
    `/api/review-queue?${params}`,
    segmentsSchema,
    revision + batch,
  );
  const items =
    data?.items.filter((item) => completed[item.id] !== item.updated_at) ?? [];
  const position = Math.min(index, Math.max(0, items.length - 1));
  const item = items[position];
  useEffect(() => {
    if (restoreFocus.current && !loading && inputRef.current) {
      inputRef.current.focus();
      restoreFocus.current = false;
    }
  }, [item?.id, loading]);
  return (
    <div className="page-enter focus-page">
      <div className="page-intro">
        <span className="eyebrow">FOCUSED REVIEW</span>
        <p data-testid="review-scope-label">
          {hasTaskScope ? '仅本次任务范围' : '全量原文库存'}
        </p>
        <h2>一次，专注一句。</h2>
        <p>核对 AI 草稿、参考译文及原文变更；保存并审校后进入下一条。</p>
      </div>
      <CategoryFilter
        id="focus"
        value={category}
        onChange={(next) => {
          setCategory(next);
          setIndex(0);
          restoreFocus.current = false;
        }}
      />
      <div className="focus-status">
        <span>
          {categoryLabels[category]} · 待审 {data?.total ?? '—'} 条 /
          当前批次最多 100 条
        </span>
        <span>
          {item ? `${position + 1} / ${items.length}` : '—'}{' '}
          <span className="muted">
            · 本次完成 {Object.keys(completed).length} 条
          </span>
        </span>
      </div>
      <div
        className="batch-progress"
        role="progressbar"
        aria-label="当前批次审校位置"
        aria-valuemin={0}
        aria-valuemax={Math.max(1, items.length)}
        aria-valuenow={item ? position + 1 : 0}
      >
        <i
          style={{
            width: `${item ? ((position + 1) / Math.max(1, items.length)) * 100 : 0}%`,
          }}
        />
      </div>
      <Feedback
        loading={loading}
        error={error}
        empty={
          !loading && data && !item
            ? '当前没有待审译文，尚未翻译的词条仍可在双语工作台编辑'
            : null
        }
      />
      {item && (
        <TranslationEditor
          editorId={`focus-${item.id}`}
          key={item.id}
          item={item}
          draftValue={drafts[item.id]}
          inputRef={inputRef}
          onDiscard={onDiscard}
          onDraft={onDraft}
          onSaved={(result) => {
            setCompleted((previous) => ({
              ...previous,
              [result.id]: item.updated_at,
            }));
            restoreFocus.current = true;
            onSaved(result);
          }}
        />
      )}
      <div className="pagination">
        <button
          data-testid="focus-prev"
          className="outline-button"
          disabled={!item || position === 0}
          onClick={() => setIndex(position - 1)}
        >
          <ArrowLeft size={16} />
          上一条
        </button>
        <button
          data-testid="focus-reload"
          className="text-button"
          disabled={loading}
          onClick={() => {
            setIndex(0);
            setBatch(batch + 1);
          }}
        >
          重新读取待审队列
        </button>
        <button
          data-testid="focus-next"
          className="outline-button"
          disabled={!item || position >= items.length - 1}
          onClick={() => setIndex(position + 1)}
        >
          暂跳此条
          <ArrowRight size={16} />
        </button>
      </div>
      <aside className="review-note">
        <span className="eyebrow">REVIEW PROTOCOL</span>
        <h3>准确，先于流畅。</h3>
        <p>
          核对数字、占位符和关键词，保留角色语气与上下文；自动格式修复仅修改当前草稿，不会代替人工审校。
        </p>
      </aside>
    </div>
  );
}
