export const CHINA_KEYLESS_SERVICES = ['volcengine_free'];

const UNREACHABLE_DEFAULTS = new Set(['deepl', 'bing', 'lingva', 'yandex', 'google', 'ecdict']);

export const DEFAULT_TRANSLATE_SERVICES = ['volcengine_free'];

function serviceName(instanceKey) {
    return String(instanceKey).split('@')[0];
}

export function migrateTranslateServices(list) {
    if (!Array.isArray(list) || list.length === 0) {
        return [...DEFAULT_TRANSLATE_SERVICES];
    }
    const names = list.map(serviceName);
    const onlyUnreachable = names.every((name) => UNREACHABLE_DEFAULTS.has(name));
    if (!onlyUnreachable) {
        return list;
    }
    const missing = CHINA_KEYLESS_SERVICES.filter((name) => !names.includes(name));
    if (missing.length === 0) {
        return list;
    }
    return [...missing, ...list];
}
