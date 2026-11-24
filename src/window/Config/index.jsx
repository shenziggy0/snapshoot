import { useLocation, useRoutes } from 'react-router-dom';
import React, { useEffect, useRef, useState } from 'react';
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
    const page = useRoutes(routes);
    const pageName = location.pathname.slice(1) || 'general';
    const title = t([`config.${pageName}.title`, `config.${pageName}.label`]);
    const scrollArea = useRef(null);
    const heading = useRef(null);
    const [collapsedPage, setCollapsedPage] = useState('');

    useEffect(() => {
        const observer = new IntersectionObserver(([entry]) => {
            setCollapsedPage(entry.isIntersecting ? '' : pageName);
        }, { root: scrollArea.current, rootMargin: '-38px 0px 0px' });
        observer.observe(heading.current);
        return () => observer.disconnect();
    }, [pageName]);

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
            <aside className='settings-rail snap-glass'>
                <div className='settings-top-space' data-tauri-drag-region='true' />
                <div className='settings-brand' data-tauri-drag-region='true'>
                    <img src='/icon.svg' draggable={false} alt='' />
                    <strong>Snapshoot</strong>
                </div>
                <SideBar />
                {appVersion && <small className='settings-version'>Snapshoot <span>v{appVersion}</span></small>}
            </aside>
            <main className='settings-main'>
                <div className='settings-titlebar' data-collapsed={collapsedPage === pageName} data-tauri-drag-region='true'>
                    <span className='settings-compact-title' aria-hidden='true'>{title}</span>
                    <CompactNavigation />
                    {osType !== 'Darwin' && <WindowControl onClose={closeSettings} />}
                </div>
                <div key={pageName} ref={scrollArea} className='settings-scroll'>
                    <div className='settings-content'>
                        <h1 className='settings-heading' ref={heading}>{title}</h1>
                        {page}
                    </div>
                </div>
            </main>
        </div>
    );
}
