import { fetch, Body } from '@tauri-apps/api/http';
import { translateStream } from '../../../utils/translate_stream';
import { Language } from './info';

export async function translate(text, from, to, options = {}) {
    const { config, instanceKey, setResult, detect } = options;

    let { apiKey, stream, promptList, requestPath } = config;
    if (!requestPath) {
        requestPath = 'https://generativelanguage.googleapis.com/v1beta/models/gemini-pro';
    }
    if (!/https?:\/\/.+/.test(requestPath)) {
        requestPath = `https://${requestPath}`;
    }

    promptList = promptList.map((item) => {
        return {
            ...item,
            parts: [
                {
                    text: item.parts[0].text
                        .replaceAll('$text', text)
                        .replaceAll('$from', from)
                        .replaceAll('$to', to)
                        .replaceAll('$detect', Language[detect]),
                },
            ],
        };
    });

    const headers = {
        'Content-Type': 'application/json',
    };
    let body = {
        contents: promptList,
        safetySettings: [
            {
                category: 'HARM_CATEGORY_HARASSMENT',
                threshold: 'BLOCK_NONE',
            },
            {
                category: 'HARM_CATEGORY_HATE_SPEECH',
                threshold: 'BLOCK_NONE',
            },
            {
                category: 'HARM_CATEGORY_SEXUALLY_EXPLICIT',
                threshold: 'BLOCK_NONE',
            },
            {
                category: 'HARM_CATEGORY_DANGEROUS_CONTENT',
                threshold: 'BLOCK_NONE',
            },
        ],
    };

    if (stream) {
        return translateStream(instanceKey, body, setResult);
    }
    const apiUrl = new URL(requestPath);
    const modelPath = apiUrl.pathname.replace(/\/+$/, '').replace(/:(streamGenerateContent|generateContent)$/, '');
    apiUrl.pathname = `${modelPath}:generateContent`;
    apiUrl.searchParams.set('key', apiKey);
    const res = await fetch(apiUrl.href, {
        method: 'POST',
        headers: headers,
        body: Body.json(body),
    });

    if (res.ok) {
        let result = res.data;
        const { candidates } = result;
        if (candidates) {
            let target = candidates[0].content.parts[0].text.trim();
            if (target) {
                if (target.startsWith('"')) {
                    target = target.slice(1);
                }
                if (target.endsWith('"')) {
                    target = target.slice(0, -1);
                }
                return target.trim();
            } else {
                throw JSON.stringify(candidates);
            }
        } else {
            throw JSON.stringify(result);
        }
    } else {
        throw `Http Request Error\nHttp Status: ${res.status}\n${JSON.stringify(res.data)}`;
    }
}

export * from './Config';
export * from './info';
