import { appWindow } from '@tauri-apps/api/window';
import { BrowserRouter } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { useTheme } from 'next-themes';

import { invoke } from '@tauri-apps/api/tauri';
import React, { lazy, Suspense, useEffect } from 'react';
import { store, waitForThemeSave } from './utils/store';
import { useConfig } from './hooks';
import './style.css';
import './window/workspaces.css';
import './i18n';

// Each Tauri window only needs its own page. Loading every window up front
// made the hidden screenshot webview parse the config, OCR and translate UI
// before it could show a frozen frame.
const Screenshot = lazy(() => import('./window/Screenshot'));
const Translate = lazy(() => import('./window/Translate'));
const Recognize = lazy(() => import('./window/Recognize'));
const Annotate = lazy(() => import('./window/Annotate'));
const Updater = lazy(() => import('./window/Updater'));
const Pin = lazy(() => import('./window/Pin'));
const PinText = lazy(() => import('./window/PinText'));
const RecordResult = lazy(() => import('./window/RecordResult'));
const Config = lazy(() => import('./window/Config'));
const QuickAccess = lazy(() => import('./window/QuickAccess'));

const windowMap = {
    translate: <Translate />,
    screenshot: <Screenshot />,
    recognize: <Recognize />,
    annotate: <Annotate />,
    config: <Config />,
    updater: <Updater />,
    record_result: <RecordResult />,
    quick_access: <QuickAccess />,
};

export default function App() {
    const [devMode] = useConfig('dev_mode', false);
    const [appTheme] = useConfig('app_theme', 'system');
    const [transparent] = useConfig('transparent', true);
    const [appLanguage] = useConfig('app_language', 'en');
    const [appFont] = useConfig('app_font', 'default');
    const [appFallbackFont] = useConfig('app_fallback_font', 'default');
    const [appFontSize] = useConfig('app_font_size', 16);
    const { setTheme } = useTheme();
    const { i18n } = useTranslation();

    useEffect(() => {
        store.load();
    }, []);

    useEffect(() => {
        const onKeyDown = async (event) => {
            if (event.defaultPrevented) return;
            if (event.ctrlKey && !['c', 'v', 'x', 'a', 'z', 'y'].includes(event.key.toLowerCase())) {
                event.preventDefault();
            }
            if (event.key.startsWith('F') && event.key.length > 1) {
                event.preventDefault();
                if (devMode && event.key === 'F12') await invoke('open_devtools');
            }
            if (event.key !== 'Escape') return;
            // Open menus and editors handle Escape before the window does.
            if (document.querySelector('[aria-modal="true"], [role="menu"], [role="listbox"]')) return;
            if (appWindow.label === 'screenshot') {
                await invoke('screen_record_cancel');
                await invoke('long_capture_cancel');
                await invoke('hide_screenshot_window');
            } else if (appWindow.label === 'quick_access') {
                await invoke('hide_quick_access_window');
            } else {
                if (appWindow.label === 'config') {
                    try {
                        await waitForThemeSave();
                    } catch {
                        return; // The theme picker reports the save error.
                    }
                }
                await appWindow.close();
            }
        };
        window.addEventListener('keydown', onKeyDown);
        return () => window.removeEventListener('keydown', onKeyDown);
    }, [devMode]);

    useEffect(() => {
        if (appTheme !== null) setTheme(appTheme);
    }, [appTheme, setTheme]);

    useEffect(() => {
        document.documentElement.dataset.solid = String(transparent === false);
    }, [transparent]);

    useEffect(() => {
        if (appLanguage !== null) {
            i18n.changeLanguage(appLanguage);
        }
    }, [appLanguage]);

    useEffect(() => {
        if (appFont !== null && appFallbackFont !== null) {
            const customFonts = [appFont, appFallbackFont]
                .filter((font) => font && font !== 'default')
                .map((font) => JSON.stringify(font));
            document.documentElement.style.fontFamily = [...customFonts, '-apple-system', 'system-ui', 'sans-serif'].join(', ');
        }
        if (appFontSize !== null) {
            document.documentElement.style.fontSize = `${appFontSize}px`;
        }
    }, [appFont, appFallbackFont, appFontSize]);

    const page = appWindow.label.startsWith('pin_text_') ? (
        <PinText />
    ) : appWindow.label.startsWith('pin_') ? (
        <Pin />
    ) : (
        windowMap[appWindow.label]
    );
    return (
        <BrowserRouter>
            <Suspense fallback={null}>{page}</Suspense>
        </BrowserRouter>
    );
}
