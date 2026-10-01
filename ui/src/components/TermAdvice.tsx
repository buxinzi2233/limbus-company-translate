import { useState } from 'react';
import { z } from 'zod';
import { request } from '../api';

const httpsUrl = z
  .url()
  .refine((value) => new URL(value).protocol === 'https:');

export const termLinkSchema = z.object({
  keyword_id: z.string(),
  source_kr: z.string(),
  target_zh: z.string(),
  wiki_url: httpsUrl,
  wiki_revision: z.number().int().positive(),
  kr_path: z.string(),
  kr_pointer: z.string(),
  kr_sha256: z.string().regex(/^[a-f0-9]{64}$/),
  zh_path: z.string(),
  zh_pointer: z.string(),
  zh_sha256: z.string().regex(/^[a-f0-9]{64}$/),
  zero_release_url: httpsUrl,
  source_manifest_url: httpsUrl,
  zero_license_url: z.literal(
    'https://creativecommons.org/licenses/by-nc-sa/4.0/',
  ),
  wiki_license_url: z.literal(
    'https://creativecommons.org/licenses/by-sa/4.0/',
  ),
  attribution: z.string(),
  verification: z.literal('exact_unique_id_and_wiki_name_checked'),
  review_status: z.literal('unreviewed'),
});

export function TermEvidence({
  link,
}: {
  link: z.infer<typeof termLinkSchema>;
}) {
  return (
    <details>
      <summary>
        {link.source_kr} → {link.target_zh} · {link.keyword_id} · 未人工确认
      </summary>
      <p>{link.attribution}</p>
      <p>
        <a href={link.wiki_url} target="_blank" rel="noreferrer">
          Wiki 修订 {link.wiki_revision}
        </a>{' '}
        ·{' '}
        <a href={link.wiki_license_url} target="_blank" rel="noreferrer">
          Wiki CC BY-SA 4.0
        </a>
      </p>
      <p>
        <a href={link.zero_release_url} target="_blank" rel="noreferrer">
          零协会来源版本
        </a>{' '}
        ·{' '}
        <a href={link.zero_license_url} target="_blank" rel="noreferrer">
          零协会 CC BY-NC-SA 4.0
        </a>
      </p>
      <p>
        韩文：{link.kr_path} {link.kr_pointer}
        <br />
        中文：{link.zh_path} {link.zh_pointer}
      </p>
      <small>
        仅名称字段身份核对，不是描述配对或翻译记忆；许可分别保留，不合并重新授权。
      </small>
    </details>
  );
}

const reportSchema = z.object({
  applicable: z.boolean(),
  available_links: z.number().int().nonnegative(),
  matched_terms: z.number().int().nonnegative(),
  blocking: z.literal(false),
  advice: z.array(
    z.object({
      rule_id: z.string(),
      message: z.string(),
      evidence: termLinkSchema,
    }),
  ),
});

export function TermAdvice({
  segmentId,
  targetText,
}: {
  segmentId: number;
  targetText: string;
}) {
  const [result, setResult] = useState<{
    text: string;
    id: number;
    report: z.infer<typeof reportSchema>;
  } | null>(null);
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);
  const current =
    result?.text === targetText && result.id === segmentId
      ? result.report
      : null;
  async function check() {
    setLoading(true);
    setError('');
    setResult(null);
    try {
      const report = await request('/api/qa/terminology', reportSchema, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ id: segmentId, target_zh: targetText }),
      });
      setResult({ text: targetText, id: segmentId, report });
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setLoading(false);
    }
  }
  return (
    <section data-testid={`term-advice-${segmentId}`}>
      <button
        type="button"
        data-testid={`term-check-${segmentId}`}
        onClick={check}
        disabled={loading || !targetText.trim()}
      >
        检查术语提示（不保存）
      </button>
      <p className="muted">
        仅战斗描述的已索引名称；提示不阻断
        QA，不代表人工确认，也不替代结构或语义审校。
      </p>
      {loading && <p role="status">检查中…</p>}
      {error && <p role="alert">{error}</p>}
      {current && (
        <div role="status">
          <p>
            {!current.applicable
              ? '当前字段不适用战斗名称格式检查。'
              : current.available_links === 0
                ? '未配置名称关联或关联库为空；没有执行术语检查。'
                : `关联库 ${current.available_links} 项，当前命中 ${current.matched_terms} 项，提示 ${current.advice.length} 项；未命中不代表译文正确。`}
          </p>
          {current.advice.map((item, index) => (
            <article className="search-result" key={index}>
              <p>
                {item.rule_id} · {item.message}
              </p>
              <TermEvidence link={item.evidence} />
            </article>
          ))}
        </div>
      )}
    </section>
  );
}
