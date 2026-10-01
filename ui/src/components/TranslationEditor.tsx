import { useState, type Ref } from 'react';
import {
  Check,
  Copy,
  WandSparkles,
  LockKeyhole,
  LoaderCircle,
} from 'lucide-react';
import {
  fixSchema,
  request,
  stageLabels,
  updateSchema,
  type Segment,
  type Draft,
} from '../api';

type EditorProps = {
  editorId: string;
  item: Segment;
  draftValue: Draft | undefined;
  inputRef: Ref<HTMLTextAreaElement>;
  onDiscard: (id: number) => void;
  onDraft: (id: number, text: string, updatedAt: string) => void;
  onSaved: (item: Segment) => void;
};
export function TranslationEditor({
  editorId,
  item: loadedItem,
  draftValue,
  inputRef,
  onDiscard,
  onDraft,
  onSaved,
}: EditorProps) {
  const [confirmedSave, setConfirmedSave] = useState<{
    previousUpdatedAt: string;
    item: Segment;
  } | null>(null);
  const item =
    confirmedSave?.previousUpdatedAt === loadedItem.updated_at
      ? confirmedSave.item
      : loadedItem;
  const draft = draftValue?.text ?? item.target_zh ?? '';
  const expectedUpdatedAt = draftValue?.updatedAt ?? item.updated_at;
  const stale = expectedUpdatedAt !== item.updated_at;
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<{
    text: string;
    error: boolean;
  } | null>(null);
  const locked = item.is_locked || item.stage === 'UpstreamFrozen';
  const invalid = /[～〜]/.test(draft);
  const reviewedUnchanged =
    item.stage === 'Reviewed' &&
    item.review_confirmed &&
    item.qa_passed &&
    draft === item.target_zh;
  const saveDisabled =
    busy || locked || stale || invalid || !draft.trim() || reviewedUnchanged;
  const save = async () => {
    if (saveDisabled) return;
    setBusy(true);
    setMessage(null);
    try {
      const data = await request('/api/segments/update', updateSchema, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          project_id: item.project_id,
          id: item.id,
          target_zh: draft,
          stage: 4,
          expected_updated_at: expectedUpdatedAt,
        }),
      });
      setConfirmedSave({
        previousUpdatedAt: loadedItem.updated_at,
        item: data.item,
      });
      onSaved(data.item);
      setMessage({ error: false, text: '译文已保存，单条 QA 检查通过。' });
    } catch (error) {
      setMessage({
        error: true,
        text: error instanceof Error ? error.message : String(error),
      });
    } finally {
      setBusy(false);
    }
  };
  const fix = async () => {
    setBusy(true);
    setMessage(null);
    try {
      const data = await request('/api/qa/fix', fixSchema, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ text: draft }),
      });
      onDraft(item.id, data.fixed, expectedUpdatedAt);
      setMessage({
        error: false,
        text:
          data.fixed === draft
            ? '未发现可自动修复的格式问题。'
            : '格式已修复，尚未保存，请确认译文。',
      });
    } catch (error) {
      setMessage({
        error: true,
        text: error instanceof Error ? error.message : String(error),
      });
    } finally {
      setBusy(false);
    }
  };
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(item.source_kr);
      setMessage({ error: false, text: '原文已复制。' });
    } catch (error) {
      setMessage({
        error: true,
        text: `复制失败，请手动选择原文：${String(error)}`,
      });
    }
  };
  return (
    <article
      className="translation-editor"
      data-testid={`editor-${editorId}`}
      aria-busy={busy}
      data-confirmed={reviewedUnchanged && !stale}
    >
      <span className="editor-frame" aria-hidden="true">
        <i />
        <i />
        <i />
        <i />
      </span>
      <header className="editor-heading">
        <div>
          <span className="eyebrow">
            SEGMENT / {String(item.id).padStart(6, '0')}
          </span>
          <h3>{item.speaker ?? '未标注说话人'}</h3>
        </div>
        <span className={`stage stage-${item.stage}`}>
          {locked && <LockKeyhole size={12} />}
          {item.stage === 'Reviewed' && !item.review_confirmed
            ? '历史译文待复核'
            : stageLabels[item.stage]}
        </span>
      </header>
      <div className="file-reference">
        <span>{item.relative_path}</span>
        <code>{item.key_path}</code>
      </div>
      <div className="language-pair">
        <section className="source-pane">
          <div className="pane-label">
            <span>
              <b>KR</b> 原文
            </span>
            <button
              className="icon-button"
              data-testid={`copy-${editorId}`}
              title="复制原文"
              aria-label="复制原文"
              onClick={copy}
            >
              <Copy size={15} />
            </button>
          </div>
          <p lang="ko">{item.source_kr}</p>
          {item.previous_kr && (
            <details>
              <summary>查看变更前原文</summary>
              <p lang="ko">{item.previous_kr}</p>
            </details>
          )}
          {item.context_info && (
            <div className="context">上下文 / {item.context_info}</div>
          )}
        </section>
        <section className="target-pane">
          <label className="pane-label" htmlFor={`translation-${editorId}`}>
            <span>
              <b>ZH</b> 中文译文
            </span>
            <span>{draft.length} 字符</span>
          </label>
          <textarea
            ref={inputRef}
            id={`translation-${editorId}`}
            data-testid={`translation-${editorId}`}
            value={draft}
            readOnly={locked}
            disabled={busy}
            placeholder="在这里写下准确的中文译文…"
            onChange={(event) => {
              setMessage(null);
              onDraft(item.id, event.target.value, expectedUpdatedAt);
            }}
            onKeyDown={(event) => {
              if (event.key === 'Enter' && (event.ctrlKey || event.metaKey)) {
                event.preventDefault();
                void save();
              }
            }}
          />
        </section>
      </div>
      {stale && (
        <p className="inline-message error" role="alert">
          服务器记录已更新，草稿仍保留；请先复制草稿并对照最新原文，再放弃旧草稿重新编辑。
          <button
            data-testid={`discard-${editorId}`}
            onClick={() => onDiscard(item.id)}
          >
            放弃旧草稿，读取最新译文
          </button>
        </p>
      )}
      {item.qa_issues && (
        <p className="inline-message error">已有 QA 记录：{item.qa_issues}</p>
      )}
      {invalid && (
        <p className="inline-message error">
          发现全角波浪号，请先修复格式再保存。
        </p>
      )}
      {message && (
        <p
          className={`inline-message ${message.error ? 'error' : ''}`}
          role={message.error ? 'alert' : 'status'}
        >
          {message.text}
        </p>
      )}
      <footer className="editor-actions">
        <span className="muted">
          {locked ? '上游基准只读，不允许覆盖' : 'Ctrl / ⌘ + Enter 保存并审校'}
        </span>
        <div>
          <button
            data-testid={`fix-${editorId}`}
            className="text-button"
            disabled={busy || locked}
            onClick={fix}
          >
            <WandSparkles size={16} />
            修复格式
          </button>
          <button
            data-testid={`save-${editorId}`}
            className="solid-button"
            disabled={saveDisabled}
            onClick={save}
          >
            {busy ? (
              <LoaderCircle size={16} className="spin" />
            ) : (
              <Check size={16} />
            )}
            {busy
              ? '处理中…'
              : reviewedUnchanged && !stale
                ? '已保存并审校'
                : '保存并审校'}
          </button>
        </div>
      </footer>
    </article>
  );
}
