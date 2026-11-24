import { ThemeProvider as NextThemesProvider } from 'next-themes';
import { NextUIProvider } from '@nextui-org/react';
import { MotionConfig } from 'framer-motion';
import ReactDOM from 'react-dom/client';
import React, { useSyncExternalStore } from 'react';

import { initStore } from './utils/store';
import { initEnv } from './utils/env';
import App from './App';

if (import.meta.env.PROD) {
    document.addEventListener('contextmenu', (e) => {
        e.preventDefault();
    });
}

const rootElement = document.getElementById('root');
const motionPreference = window.matchMedia('(prefers-reduced-motion: reduce)');
const listenForMotion = (notify) => {
    if (typeof motionPreference.addEventListener === 'function') {
        motionPreference.addEventListener('change', notify);
        return () => motionPreference.removeEventListener('change', notify);
    }
    motionPreference.addListener(notify);
    return () => motionPreference.removeListener(notify);
};

function Providers() {
    const reduceMotion = useSyncExternalStore(listenForMotion, () => motionPreference.matches);
    return (
        <MotionConfig reducedMotion={reduceMotion ? 'always' : 'never'}>
            <NextUIProvider disableAnimation={reduceMotion} skipFramerMotionAnimations={false}>
                <NextThemesProvider attribute='class'>
                    <App />
                </NextThemesProvider>
            </NextUIProvider>
        </MotionConfig>
    );
}

Promise.all([initStore(), initEnv()])
    .catch((error) => {
        // A slow or failed watcher must not leave a hidden capture webview
        // mounted with no React tree at all.
        console.error('app bootstrap failed', error);
    })
    .finally(() => ReactDOM.createRoot(rootElement).render(<Providers />));
