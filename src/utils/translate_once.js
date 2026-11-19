import { getServiceName, whetherPluginService } from './service_instance';
import { store } from './store';

const modules = import.meta.glob('../services/translate/*/index.jsx');

export async function translateOnce(text, targetOverride) {
    const source = String(text || '').trim();
    if (!source) {
        return '';
    }
    const targetPreference = targetOverride || (await store.get('translate_target_language')) || 'zh_cn';
    const configured = await store.get('translate_service_list');
    const instances = Array.isArray(configured) ? configured : [];

    let hasEnabledService = false;
    let lastError = null;
    for (const instanceKey of instances) {
        if (typeof instanceKey !== 'string' || whetherPluginService(instanceKey)) {
            continue;
        }
        const serviceName = getServiceName(instanceKey);
        const loader = modules[`../services/translate/${serviceName}/index.jsx`];
        if (!loader) {
            continue;
        }
        const config = (await store.get(instanceKey)) ?? {};
        if (!(config.enable ?? true)) {
            continue;
        }
        hasEnabledService = true;
        try {
            const service = await loader();
            const language = service.Language || {};
            const sourceLanguage = language.auto ?? 'auto';
            if (!Object.prototype.hasOwnProperty.call(language, targetPreference)) {
                continue;
            }
            const value = await service.translate(source, sourceLanguage, language[targetPreference], {
                config,
                instanceKey,
                detect: 'auto',
                setResult: () => {},
            });
            const translated = typeof value === 'string' ? value.trim() : '';
            if (translated && translated !== '[STREAM]') {
                return translated;
            }
            lastError = new Error('translate.inline_empty_result');
        } catch (error) {
            lastError = error;
        }
    }
    throw lastError || new Error(hasEnabledService ? 'translate.target_not_supported' : 'translate.inline_no_service');
}
