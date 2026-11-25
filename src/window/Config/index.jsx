import { useLocation, useNavigate, useRoutes } from 'react-router-dom';
import React, { useEffect } from 'react';
import { appWindow } from '@tauri-apps/api/window';
import { useTranslation } from 'react-i18next';
import WindowControl from '../../components/WindowControl';
import SideBar, { CompactNavigation } from './components/SideBar';
import { appVersion, osType } from '../../utils/env';
import { waitForThemeSave } from '../../utils/store';
import routes from './routes';
import './style.css';

export default function Config() {
    const { t } = useTranslation();
    const location = useLocation();
    const navigate = useNavigate();
    const page = useRoutes(routes);
    const pageName = location.pathname.slice(1) || 'general';
    const title = t([`config.${pageName}.title`, `config.${pageName}.label`]);
    useEffect(() => {
        let mounted = true;
        const closeListener = appWindow.onCloseRequested(async (event) => {
            try {
                await waitForThemeSave();
            } catch {
                event.preventDefault();
            }
        });

        void closeListener
            .catch((error) => console.error('Could not subscribe to settings close requests', error))
            .then(() => {
                if (mounted) return appWindow.show();
            })
            .catch((error) => console.error('Could not show the settings window', error));

        return () => {
            mounted = false;
            void closeListener.then((unlisten) => unlisten(), () => {});
        };
    }, []);

    useEffect(() => {
        const unsubscribe = appWindow.listen('open-recent', () => navigate('/recent'));
        return () => { void unsubscribe.then((remove) => remove()); };
    }, [navigate]);

    const closeSettings = async () => {
        try {
            await waitForThemeSave();
        } catch {
            return; // The theme picker reports the save error.
        }
        await appWindow.close();
    };

    return (
        <div className='settings-shell' data-platform={osType}>
            <aside className='settings-rail'>
                <div className='settings-top-space' data-tauri-drag-region='true' />
                <div className='settings-brand' data-tauri-drag-region='true'>
                    <img src='/icon.svg' draggable={false} alt='' />
                    <div><strong>Snapshoot</strong>{appVersion && <small className='settings-version'>v{appVersion}</small>}</div>
                </div>
                <SideBar />
            </aside>
            <main className='settings-main'>
                <div className='settings-titlebar' data-tauri-drag-region='true'>
                    <h1 className='settings-heading'>{title}</h1>
                    <CompactNavigation />
                    {osType !== 'Darwin' && <WindowControl onClose={closeSettings} />}
                </div>
                <div key={pageName} className='settings-scroll'>
                    <div className='settings-content'>
                        {page}
                    </div>
                </div>
            </main>
        </div>
    );
}
