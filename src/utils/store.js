import { Store } from 'tauri-plugin-store-api';
import { appConfigDir, join } from '@tauri-apps/api/path';
import { watch } from 'tauri-plugin-fs-watch-api';
import { emit } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api';

export let store = new Store();
let pendingThemeSave;

export function saveAppTheme(mode) {
    const save = (pendingThemeSave?.catch(() => {}) ?? Promise.resolve()).then(async () => {
        const previousTheme = await store.get('app_theme');
        await store.set('app_theme', mode);
        try {
            await store.save();
        } catch (error) {
            // The plugin cache is also used by later saves in other windows.
            try {
                if (previousTheme == null) await store.delete('app_theme');
                else await store.set('app_theme', previousTheme);
            } catch (restoreError) {
                console.error('Could not restore the theme in the store cache', restoreError);
            }
            throw error;
        }

        await emit('app_theme_changed', mode).catch((error) => {
            console.error('Could not broadcast the saved theme', error);
        });
    });

    pendingThemeSave = save;
    const clear = () => {
        if (pendingThemeSave === save) pendingThemeSave = undefined;
    };
    void save.then(clear, clear);
    return save;
}

export async function waitForThemeSave() {
    let save = pendingThemeSave;
    while (save) {
        try {
            await save;
        } catch (error) {
            if (!pendingThemeSave || pendingThemeSave === save) throw error;
        }
        if (pendingThemeSave === save) return;
        save = pendingThemeSave;
    }
}

export async function initStore() {
    const appConfigDirPath = await appConfigDir();
    const appConfigPath = await join(appConfigDirPath, 'config.json');
    store = new Store(appConfigPath);
    // File watching is only needed for external config edits. Do not hold the
    // first render behind another IPC round-trip.
    void watch(appConfigPath, async () => {
        await store.load();
        await invoke('reload_store');
    }).catch((error) => console.error('config watch failed', error));
}
