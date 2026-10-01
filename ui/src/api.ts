import { z } from 'zod';

const count = z.number().int().nonnegative();
const stage = z.enum([
  'Untranslated',
  'TmExact',
  'OutdatedDiff',
  'AiDrafted',
  'Reviewed',
  'Imported',
  'UpstreamFrozen',
]);
export const segmentSchema = z.object({
  id: count,
  is_locked: z.boolean(),
  review_confirmed: z.boolean(),
  file_id: count,
  project_id: z.string(),
  relative_path: z.string(),
  key_path: z.string(),
  source_kr: z.string(),
  previous_kr: z.string().nullable(),
  target_zh: z.string().nullable(),
  speaker: z.string().nullable(),
  context_info: z.string().nullable(),
  stage,
  qa_passed: z.boolean(),
  qa_issues: z.string().nullable(),
  updated_at: z.string(),
});
export type Segment = z.infer<typeof segmentSchema>;
export type Draft = { text: string; updatedAt: string };
const statsSchema = z.object({
  total_files: count,
  total_segments: count,
  translated_segments: count,
  reviewed_segments: count,
  tm_entries: count,
  pending_review_segments: count,
  categories: z.record(
    z.string(),
    z.object({ total: count, translated: count }),
  ),
  stages: z.record(z.string(), count),
});
export const taskScopeSchema = statsSchema
  .omit({ tm_entries: true, stages: true })
  .extend({
    name: z.string(),
    manifest_sha256: z.string(),
    source_import_id: count,
  });
export const statusSchema = z.object({
  status: z.literal('ok'),
  version: z.string(),
  project: z.object({ id: z.string(), name: z.string() }).nullable(),
  stats: statsSchema,
  task_scope: taskScopeSchema.nullable(),
});
export type Status = z.infer<typeof statusSchema>;
export const segmentsSchema = z.object({
  status: z.literal('ok'),
  total: count,
  page: count,
  page_size: count,
  items: z.array(segmentSchema),
});
export const searchSchema = z.object({
  status: z.literal('ok'),
  query: z.string(),
  total_found: count,
  segments: z.array(
    segmentSchema
      .pick({
        file_id: true,
        relative_path: true,
        key_path: true,
        source_kr: true,
        target_zh: true,
        speaker: true,
        stage: true,
        review_confirmed: true,
      })
      .extend({ segment_id: count }),
  ),
  tm_entries: z.array(
    z.object({
      id: count,
      source_kr: z.string(),
      target_zh: z.string(),
      source_origin: z.string(),
      file_scope: z.string().nullable(),
      speaker: z.string().nullable(),
      priority: z.number(),
    }),
  ),
});
export const updateSchema = z.object({
  item: segmentSchema,
  status: z.literal('ok'),
  id: count,
  qa_passed: z.boolean(),
  issues: z.array(
    z.object({
      rule_id: z.string(),
      severity: z.enum(['Fatal', 'Error', 'Warn']),
      message: z.string(),
    }),
  ),
});
export const fixSchema = z.object({ fixed: z.string(), original: z.string() });
export const stageLabels: Record<Segment['stage'], string> = {
  Untranslated: '待翻译',
  TmExact: '记忆库匹配',
  OutdatedDiff: '原文变更',
  AiDrafted: 'AI 草稿',
  Reviewed: '已审校',
  Imported: '导入译文待审',
  UpstreamFrozen: '上游锁定',
};

/** Validate server responses; retry transient failures without masking the final error. */
export async function request<T>(
  url: string,
  schema: z.ZodType<T>,
  init: RequestInit,
): Promise<T> {
  for (let attempt = 1; attempt <= 2; attempt += 1) {
    try {
      const response = await fetch(url, {
        ...init,
        signal: AbortSignal.any([
          ...(init.signal ? [init.signal] : []),
          AbortSignal.timeout(15000),
        ]),
      });
      const body = await response.text();
      if (!response.ok) {
        const error = new Error(
          `${init.method ?? 'GET'} ${url} · HTTP ${response.status} · ${body}`,
        );
        if (
          response.status >= 500 &&
          attempt === 1 &&
          (!init.method || init.method === 'GET')
        ) {
          console.warn('api_retry', {
            url,
            attempt,
            status: response.status,
            body,
          });
          continue;
        }
        throw error;
      }
      let parsed: z.infer<ReturnType<typeof z.json>>;
      try {
        parsed = JSON.parse(body);
      } catch (error) {
        throw new SyntaxError(
          `${url} · HTTP ${response.status} · 响应不是有效 JSON: ${String(error)} · ${body}`,
        );
      }
      const result = schema.safeParse(parsed);
      if (!result.success)
        throw new TypeError(
          `${url} · HTTP ${response.status} · 响应不符合接口契约: ${result.error.message} · ${body}`,
        );
      return result.data;
    } catch (error) {
      if (
        error instanceof TypeError &&
        error.message === 'Failed to fetch' &&
        (!init.method || init.method === 'GET') &&
        attempt === 1 &&
        !init.signal?.aborted
      ) {
        console.warn('api_retry', { url, attempt, error });
        continue;
      }
      if (error instanceof TypeError && error.message === 'Failed to fetch') {
        throw new TypeError(
          `${init.method ?? 'GET'} ${url} · 无法连接本地服务，请检查服务进程；写入请求的结果可能未知，请刷新核对后重试。原因: ${error.message}`,
        );
      }
      if (error instanceof DOMException && error.name === 'TimeoutError') {
        throw new DOMException(
          `${url} · 请求超过 15 秒，请检查本地服务是否繁忙。`,
          'TimeoutError',
        );
      }
      throw error;
    }
  }
  throw new Error(`${url} · 请求重试次数耗尽`);
}
export const formatCount = (value: number): string =>
  value.toLocaleString('en-US');
export const percent = (part: number, total: number): number =>
  total === 0 ? 0 : Math.min(100, (part / total) * 100);

export const auditSchema = z.object({
  operations: z.array(
    z.object({
      id: count,
      project_id: z.string().nullable(),
      kind: z.enum(['inject', 'restore']),
      workspace: z.string(),
      backup_directory: z.string(),
      affected_files: count,
      created_at: z.string(),
    }),
  ),
  next_operations: count.nullable(),
  status: z.literal('ok'),
  project: z.object({
    id: z.string(),
    name: z.string(),
    live_client_kr_dir: z.string().nullable(),
    workspace_zh_dir: z.string().nullable(),
  }),
  events: z.array(
    z.object({
      id: count,
      segment_id: count,
      kind: z.enum(['ai_draft', 'review']),
      producer: z.string(),
      source_kr: z.string(),
      previous_target: z.string().nullable(),
      target_zh: z.string(),
      previous_stage: z.number().int(),
      resulting_stage: z.number().int(),
      created_at: z.string(),
    }),
  ),
  imports: z.array(
    z.object({
      id: count,
      source_directory: z.string(),
      translation_directory: z.string(),
      active_segments: count,
      added_or_reactivated: count,
      changed: count,
      retired: count,
      created_at: z.string(),
    }),
  ),
  corpora: z.array(z.object({ source_origin: z.string(), entries: count })),
  next_events: count.nullable(),
  next_imports: count.nullable(),
  next_corpora: count.nullable(),
});
