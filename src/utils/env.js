import { type, arch as archFn, version } from '@tauri-apps/api/os';
import { getVersion } from '@tauri-apps/api/app';

export let osType = '';
export let arch = '';
export let osVersion = '';
export let appVersion = '';

export async function initEnv() {
    const [nextType, nextArch, nextVersion, nextAppVersion] = await Promise.all([
        type(),
        archFn(),
        version(),
        getVersion(),
    ]);
    osType = nextType;
    arch = nextArch;
    osVersion = nextVersion;
    appVersion = nextAppVersion;
}
