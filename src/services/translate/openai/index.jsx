import { fetch, Body } from '@tauri-apps/api/http';
import { translateStream } from '../../../utils/translate_stream';
import { Language } from './info';
import { defaultRequestArguments } from './Config';

export async function translate(text, from, to, options) {
    const { config, instanceKey, setResult, detect } = options;

    let { service, requestPath, model, apiKey, stream, promptList, requestArguments } = config;

    if (!/https?:\/\/.+/.test(requestPath)) {
        requestPath = `https://${requestPath}`;
    }
    const apiUrl = new URL(requestPath);

    // in openai like api, /v1 is not required
    const path = apiUrl.pathname.replace(/\/+$/, '');
    if (service === 'openai' && !path.endsWith('/chat/completions')) {
        apiUrl.pathname = `${path}/${path.endsWith('/v1') ? 'chat/completions' : 'v1/chat/completions'}`;
    }

    // 兼容旧版
    if (promptList === undefined) {
        promptList = [
            {
                role: 'system',
                content:
                    'You are a professional translation engine, please translate the text into a colloquial, professional, elegant and fluent content, without the style of machine translation. You must only translate the text content, never interpret it.',
            },
            { role: 'user', content: `Translate into $to:\n"""\n$text\n"""` },
        ];
    }

    promptList = promptList.map((item) => {
        return {
            ...item,
            content: item.content
                .replaceAll('$text', text)
                .replaceAll('$from', from)
                .replaceAll('$to', to)
                .replaceAll('$detect', Language[detect]),
        };
    });

    const headers =
        service === 'openai'
            ? {
                  'Content-Type': 'application/json',
                  Authorization: `Bearer ${apiKey}`,
              }
            : {
                  'Content-Type': 'application/json',
                  'api-key': apiKey,
              };
    const body = {
        ...JSON.parse(requestArguments ?? defaultRequestArguments),
        stream: stream,
        messages: promptList,
    };
    if (service === 'openai') {
        body['model'] = model;
    }
    if (stream) {
        return translateStream(instanceKey, body, setResult);
    }
    let res = await fetch(apiUrl.href, {
        method: 'POST',
        headers: headers,
        body: Body.json(body),
    });
    if (res.ok) {
        let result = res.data;
        const { choices } = result;
        if (choices) {
            let target = choices[0].message.content.trim();
            if (target) {
                if (target.startsWith('"')) {
                    target = target.slice(1);
                }
                if (target.endsWith('"')) {
                    target = target.slice(0, -1);
                }
                return target.trim();
            } else {
                throw JSON.stringify(choices);
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
