import { appWindow } from '@tauri-apps/api/window';
import { BrowserRouter } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { warn } from 'tauri-plugin-log-api';
import { useTheme } from 'next-themes';

import { invoke } from '@tauri-apps/api/tauri';
import React, { lazy, Suspense, useEffect } from 'react';
import { store } from './utils/store';
import { useConfig } from './hooks';
import './style.css';
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
        if (devMode !== null && devMode) {
            document.addEventListener('keydown', async (e) => {
                let allowKeys = ['c', 'v', 'x', 'a', 'z', 'y'];
                if (e.ctrlKey && !allowKeys.includes(e.key.toLowerCase())) {
                    e.preventDefault();
                }
                if (e.key === 'F12') {
                    await invoke('open_devtools');
                }
                if (e.key.startsWith('F') && e.key.length > 1) {
                    e.preventDefault();
                }
                if (e.key === 'Escape') {
                    if (appWindow.label === 'screenshot') {
                        await invoke('screen_record_cancel');
                        await invoke('long_capture_cancel');
                        await invoke('hide_screenshot_window');
                    } else if (appWindow.label === 'quick_access') {
                        await invoke('hide_quick_access_window');
                    } else {
                        await appWindow.close();
                    }
                }
            });
        } else {
            document.addEventListener('keydown', async (e) => {
                let allowKeys = ['c', 'v', 'x', 'a', 'z', 'y'];
                if (e.ctrlKey && !allowKeys.includes(e.key.toLowerCase())) {
                    e.preventDefault();
                }
                if (e.key.startsWith('F') && e.key.length > 1) {
                    e.preventDefault();
                }
                if (e.key === 'Escape') {
                    if (appWindow.label === 'screenshot') {
                        await invoke('screen_record_cancel');
                        await invoke('long_capture_cancel');
                        await invoke('hide_screenshot_window');
                    } else if (appWindow.label === 'quick_access') {
                        await invoke('hide_quick_access_window');
                    } else {
                        await appWindow.close();
                    }
                }
            });
        }
    }, [devMode]);

    useEffect(() => {
        if (appTheme !== null) {
            if (appTheme !== 'system') {
                setTheme(appTheme);
            } else {
                try {
                    if (window.matchMedia('(prefers-color-scheme: dark)').matches) {
                        setTheme('dark');
                    } else {
                        setTheme('light');
                    }
                    window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', (e) => {
                        if (e.matches) {
                            setTheme('dark');
                        } else {
                            setTheme('light');
                        }
                    });
                } catch {
                    warn("Can't detect system theme.");
                }
            }
        }
    }, [appTheme]);

    useEffect(() => {
        if (appLanguage !== null) {
            i18n.changeLanguage(appLanguage);
        }
    }, [appLanguage]);

    useEffect(() => {
        if (appFont !== null && appFallbackFont !== null) {
            document.documentElement.style.fontFamily = `"${appFont === 'default' ? 'sans-serif' : appFont}","${
                appFallbackFont === 'default' ? 'sans-serif' : appFallbackFont
            }"`;
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
