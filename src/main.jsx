import { ThemeProvider as NextThemesProvider } from 'next-themes';
import { appWindow } from '@tauri-apps/api/window';
import { NextUIProvider } from '@nextui-org/react';
import ReactDOM from 'react-dom/client';
import React from 'react';

import { initStore } from './utils/store';
import { initEnv } from './utils/env';
import App from './App';

if (import.meta.env.PROD) {
    document.addEventListener('contextmenu', (e) => {
        e.preventDefault();
    });
}

const rootElement = document.getElementById('root');

function renderApp() {
    const root = ReactDOM.createRoot(rootElement);
    root.render(
        <NextUIProvider>
            <NextThemesProvider attribute='class'>
                <App />
            </NextThemesProvider>
        </NextUIProvider>
    );
}

Promise.all([initStore(), initEnv()])
    .catch((error) => {
        // A slow or failed watcher must not leave a hidden capture webview
        // mounted with no React tree at all.
        console.error('app bootstrap failed', error);
    })
    .finally(renderApp);
