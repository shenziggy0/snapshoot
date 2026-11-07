import { DEFAULT_TRANSLATE_SERVICES, migrateTranslateServices } from '../services/translate/defaults';
import { getServiceName, whetherPluginService } from './service_instance';
import { store } from './store';

const modules = import.meta.glob('../services/translate/*/index.jsx');
const cache = new Map();

function moduleKey(serviceName) {
    return `../services/translate/${serviceName}/index.jsx`;
}

function pickLanguage(language, preferred) {
    if (!language) {
        return null;
    }
    if (Object.prototype.hasOwnProperty.call(language, preferred)) {
        return preferred;
    }
    if (preferred !== 'zh_cn' && Object.prototype.hasOwnProperty.call(language, 'zh_cn')) {
        return 'zh_cn';
    }
    if (Object.prototype.hasOwnProperty.call(language, 'en')) {
        return 'en';
    }
    return null;
}

async function configuredServices() {
    const configured = await store.get('translate_service_list');
    const list = migrateTranslateServices(Array.isArray(configured) ? configured : [...DEFAULT_TRANSLATE_SERVICES]);
    const usable = list.filter((key) => !whetherPluginService(key) && modules[moduleKey(getServiceName(key))]);
    if (!usable.some((key) => getServiceName(key) === 'volcengine_free')) {
        usable.push('volcengine_free');
    }
    return usable;
}

export async function translateOnce(text, targetOverride) {
    const source = String(text || '').trim();
    if (!source) {
        return '';
    }
    const targetPreference = targetOverride || (await store.get('translate_target_language')) || 'zh_cn';
    const cacheKey = `${targetPreference}\n${source}`;
    if (cache.has(cacheKey)) {
        return cache.get(cacheKey);
    }

    let lastError = null;
    for (const instanceKey of await configuredServices()) {
        const serviceName = getServiceName(instanceKey);
        const loader = modules[moduleKey(serviceName)];
        if (!loader) {
            continue;
        }
        try {
            const service = await loader();
            const language = service.Language || {};
            const sourceLanguage = language.auto ?? 'auto';
            const targetLanguage = pickLanguage(language, targetPreference);
            if (!targetLanguage) {
                continue;
            }
            const config = (await store.get(instanceKey)) ?? {};
            const value = await service.translate(source, sourceLanguage, targetLanguage, { config });
            const translated = typeof value === 'string' ? value.trim() : String(value ?? '').trim();
            if (translated) {
                cache.set(cacheKey, translated);
                return translated;
            }
        } catch (error) {
            lastError = error;
        }
    }
    throw lastError || new Error('no available translate service');
}
