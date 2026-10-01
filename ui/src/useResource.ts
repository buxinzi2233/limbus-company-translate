import { useEffect, useState } from 'react';
import type { z } from 'zod';
import { request } from './api';

export function useResource<T>(
  url: string | null,
  schema: z.ZodType<T>,
  revision: number,
) {
  const key = JSON.stringify([url, revision]);
  const [state, setState] = useState<{
    key: string;
    data: T | null;
    loading: boolean;
    error: string | null;
  }>({ key, data: null, loading: url !== null, error: null });
  useEffect(() => {
    if (url === null) return;
    const controller = new AbortController();
    setState({ key, data: null, loading: true, error: null });
    request(url, schema, { signal: controller.signal }).then(
      (data) => {
        if (!controller.signal.aborted)
          setState({ key, data, loading: false, error: null });
      },
      (error: Error) => {
        if (!controller.signal.aborted)
          setState({ key, data: null, loading: false, error: error.message });
      },
    );
    return () => controller.abort();
  }, [url, schema, key]);
  return state.key === key
    ? state
    : { key, data: null, loading: url !== null, error: null };
}
