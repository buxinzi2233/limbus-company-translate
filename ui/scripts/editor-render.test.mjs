import assert from 'node:assert/strict';
import { readFile, writeFile, unlink } from 'node:fs/promises';
import { randomUUID } from 'node:crypto';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import ts from 'typescript';

const source = new URL(
  '../src/components/TranslationEditor.tsx',
  import.meta.url,
);
const compiled = new URL(`./.editor-render-${randomUUID()}.mjs`, source);
const output = ts
  .transpileModule(await readFile(source, 'utf8'), {
    compilerOptions: {
      module: ts.ModuleKind.ESNext,
      jsx: ts.JsxEmit.ReactJSX,
      target: ts.ScriptTarget.ES2022,
    },
  })
  .outputText.replace("from '../api'", "from '../api.ts'");

await writeFile(compiled, output, { flag: 'wx' });
try {
  const { TranslationEditor } = await import(compiled.href);
  const item = {
    id: 1,
    project_id: 'render-check',
    file_id: 1,
    relative_path: 'Battle/Skills.json',
    key_path: '/dataList/0/desc',
    source_kr: '효과 1: {0}에게 20% 피해를 입힙니다.',
    previous_kr: null,
    target_zh: '效果1：对{0}造成20%伤害。',
    speaker: null,
    context_info: null,
    stage: 'Reviewed',
    qa_passed: true,
    qa_issues: null,
    updated_at: '2026-01-01T00:00:00Z',
    is_locked: false,
    review_confirmed: true,
  };
  const unexpectedEvent = () => {
    throw new Error('Server rendering must not perform an edit');
  };
  const cases = [
    { name: 'saved review', item, draft: undefined, disabled: true },
    {
      name: 'edited review',
      item,
      draft: { text: '效果1：向{0}造成20%伤害。', updatedAt: item.updated_at },
      disabled: false,
    },
    {
      name: 'reverted draft',
      item,
      draft: { text: item.target_zh, updatedAt: item.updated_at },
      disabled: true,
    },
    {
      name: 'source changed',
      item: { ...item, stage: 'OutdatedDiff', qa_passed: false },
      draft: undefined,
      disabled: false,
    },
    {
      name: 'unreviewed existing target',
      item: { ...item, stage: 'TmExact' },
      draft: undefined,
      disabled: false,
    },
    {
      name: 'failed QA',
      item: { ...item, qa_passed: false },
      draft: undefined,
      disabled: false,
    },
    {
      name: 'stale edit',
      item,
      draft: { text: '待复核草稿', updatedAt: '2025-12-31T00:00:00Z' },
      disabled: true,
    },
    {
      name: 'locked file',
      item: { ...item, is_locked: true },
      draft: { text: '新草稿', updatedAt: item.updated_at },
      disabled: true,
    },
    {
      name: 'empty translation',
      item,
      draft: { text: ' ', updatedAt: item.updated_at },
      disabled: true,
    },
  ];
  cases.push({
    name: 'imported translation',
    item: { ...item, stage: 'Imported' },
    draft: undefined,
    disabled: false,
  });
  cases.push({
    name: 'AI draft',
    item: { ...item, stage: 'AiDrafted' },
    draft: undefined,
    disabled: false,
  });
  cases.push({
    name: 'legacy review without evidence',
    item: { ...item, review_confirmed: false },
    draft: undefined,
    disabled: false,
  });
  for (const scenario of cases) {
    const html = renderToStaticMarkup(
      createElement(TranslationEditor, {
        editorId: 'render-check',
        item: scenario.item,
        draftValue: scenario.draft,
        inputRef: null,
        onDiscard: unexpectedEvent,
        onDraft: unexpectedEvent,
        onSaved: unexpectedEvent,
      }),
    );
    const button = html.match(
      /<button\b[^>]*data-testid="save-render-check"[^>]*>/,
    )?.[0];
    assert.ok(button, `${scenario.name}: save button missing`);
    assert.equal(
      /\bdisabled=""/.test(button),
      scenario.disabled,
      scenario.name,
    );
  }
  console.log(
    `Editor component rendering passed: ${cases.length} save-state scenarios`,
  );
} finally {
  await unlink(compiled);
}
