import { Body } from '@tauri-apps/api/http';

import { translateFetch } from '../../../utils/translate_fetch';

const URL = 'https://translate.volcengine.com/crx/translate/v1/';

export async function translate(text, from, to) {
    const source = from || 'auto';
    const target = to || 'zh';
    const res = await translateFetch(URL, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: Body.json({
            source_language: source,
            target_language: target,
            text,
        }),
    });
    const result = res.data || {};
    const status = result.base_resp?.status_code;
    if (!res.ok || (status !== undefined && status !== 0)) {
        const message = result.base_resp?.status_message || JSON.stringify(result);
        throw `Http Request Error\nHttp Status: ${res.status}\n${message}`;
    }
    const translation = result.translation;
    if (!translation) {
        throw 'Empty translation';
    }
    return String(translation).trim();
}

export * from './Config';
export * from './info';
