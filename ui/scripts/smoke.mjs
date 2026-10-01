/** Exercise real API responses without changing project translations. Requires Node 22.18+. */
import assert from 'node:assert/strict';
import {
  request,
  statusSchema,
  segmentsSchema,
  searchSchema,
  fixSchema,
  auditSchema,
} from '../src/api.ts';

const origin = process.argv[2];
if (!origin)
  throw new TypeError('Usage: pnpm test:smoke http://127.0.0.1:3000');
const base = new URL(origin);
assert.ok(
  ['127.0.0.1', 'localhost', '[::1]'].includes(base.hostname),
  'Smoke tests must use a local server.',
);
const status = await request(
  new URL('/api/status', base).href,
  statusSchema,
  {},
);
const first = await request(
  new URL('/api/segments?page=1&page_size=25', base).href,
  segmentsSchema,
  {},
);
assert.equal(first.page, 1);
assert.ok(first.items.length <= 25);
const second = await request(
  new URL('/api/segments?page=2&page_size=25', base).href,
  segmentsSchema,
  {},
);
assert.equal(second.page, 2);
assert.ok(
  !second.items.some((item) =>
    first.items.some((previous) => previous.id === item.id),
  ),
);
const pending = await request(
  new URL('/api/segments?stage=0&page=1&page_size=25', base).href,
  segmentsSchema,
  {},
);
assert.ok(pending.items.every((item) => item.stage === 'Untranslated'));
const search = await request(
  new URL('/api/search?q=%E6%AD%A3%E4%B9%89&limit=3', base).href,
  searchSchema,
  {},
);
assert.equal(
  search.total_found,
  search.segments.length + search.tm_entries.length,
);
const empty = await request(
  new URL('/api/segments?search=lct-no-match-5f732a&page=1&page_size=25', base)
    .href,
  segmentsSchema,
  {},
);
assert.equal(empty.total, 0);
const fixed = await request(new URL('/api/qa/fix', base).href, fixSchema, {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({ text: '测试范围1～3' }),
});
assert.equal(fixed.fixed, '测试范围1~3');
const audit = await request(new URL('/api/audit', base).href, auditSchema, {});
assert.equal(audit.project.id, status.project?.id);
assert.ok(audit.events.length <= 20 && audit.operations.length <= 20);
const reviewQueue = await request(
  new URL('/api/review-queue', base).href,
  segmentsSchema,
  {},
);
assert.ok(
  reviewQueue.items.every(
    (item) =>
      ['TmExact', 'OutdatedDiff', 'AiDrafted', 'Imported'].includes(
        item.stage,
      ) ||
      (item.stage === 'Reviewed' &&
        (!item.review_confirmed || !item.qa_passed)),
  ),
);
console.log('live_api_smoke_passed', {
  project: status.project?.id,
  total: first.total,
  pending: pending.total,
  searchResults: search.total_found,
});
