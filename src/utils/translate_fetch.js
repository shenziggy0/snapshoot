import { fetch as tauriFetch } from '@tauri-apps/api/http';

export const TRANSLATE_TIMEOUT_SECS = 8;

function classify(detail) {
    if (/timed?\s*out|timeout|deadline/i.test(detail)) {
        return 'timeout';
    }
    if (/connection|send request|network|dns|refused|reset|closed|unreachable|os error/i.test(detail)) {
        return 'network';
    }
    return '';
}

export async function translateFetch(url, options = {}) {
    try {
        return await tauriFetch(url, {
            ...options,
            timeout: options.timeout ?? TRANSLATE_TIMEOUT_SECS,
        });
    } catch (error) {
        const detail = String(error?.message || error || '');
        const wrapped = new Error(detail);
        wrapped.kind = classify(detail);
        wrapped.detail = detail;
        throw wrapped;
    }
}
