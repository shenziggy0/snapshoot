import { appWindow, PhysicalPosition, PhysicalSize } from '@tauri-apps/api/window';
import { save } from '@tauri-apps/api/dialog';
import toast, { Toaster } from 'react-hot-toast';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api';
import React, { useEffect, useRef, useState } from 'react';

import { useToastStyle } from '../../hooks';
import { useSystemDark } from '../useSystemDark';

function clamp(value, min, max) {
    return Math.min(max, Math.max(min, value));
}

function base64ToObjectUrl(data) {
    const binary = atob(data);
    const bytes = new Uint8Array(binary.length);
    for (let i = 0; i < binary.length; i += 1) {
        bytes[i] = binary.charCodeAt(i);
    }
    return URL.createObjectURL(new Blob([bytes], { type: 'image/png' }));
}

export default function Pin() {
    const { t } = useTranslation();
    const toastStyle = useToastStyle();
    const dark = useSystemDark();
    const infoRef = useRef(null);
    const zoomRef = useRef(1);
    const [url, setUrl] = useState('');
    const [loadError, setLoadError] = useState('');
    const [opacity, setOpacity] = useState(1);
    const [menu, setMenu] = useState(null);
    const [hover, setHover] = useState(false);
    const [zoomText, setZoomText] = useState('');
    const zoomTimer = useRef(0);
    const blobUrlRef = useRef('');

    useEffect(() => {
        document.documentElement.style.borderRadius = '0px';
        document.documentElement.style.background = 'transparent';
        document.body.style.background = 'transparent';
        document.body.style.overflow = 'hidden';
        const root = document.getElementById('root');
        if (root) {
            root.style.background = 'transparent';
            root.style.height = '100%';
        }
    }, []);

    useEffect(() => {
        let cancelled = false;
        (async () => {
            const meta = await invoke('pin_window_info');
            const data = await invoke('pin_image_base64');
            if (cancelled) {
                return;
            }
            infoRef.current = meta;
            zoomRef.current = meta.zoom || 1;
            const objectUrl = base64ToObjectUrl(data);
            blobUrlRef.current = objectUrl;
            setUrl(objectUrl);
            await appWindow.show();
            await appWindow.setFocus();
            await appWindow.setAlwaysOnTop(true);
        })().catch(async (error) => {
            setLoadError(String(error));
            toast.error(String(error), { style: toastStyle });
            await appWindow.show();
        });
        return () => {
            cancelled = true;
            if (blobUrlRef.current) {
                URL.revokeObjectURL(blobUrlRef.current);
            }
        };
    }, []);

    const applyZoom = async (nextZoom, clientX, clientY) => {
        const info = infoRef.current;
        if (!info) {
            return;
        }
        const current = zoomRef.current;
        const image = document.getElementById('pin-image');
        const rect = image ? image.getBoundingClientRect() : null;
        const relX = rect && rect.width ? (clientX - rect.left) / rect.width : 0.5;
        const relY = rect && rect.height ? (clientY - rect.top) / rect.height : 0.5;
        const pos = await appWindow.outerPosition();
        const oldW = info.width * current;
        const oldH = info.height * current;
        const newW = Math.max(1, Math.round(info.width * nextZoom));
        const newH = Math.max(1, Math.round(info.height * nextZoom));
        const newX = Math.round(pos.x + relX * (oldW - newW));
        const newY = Math.round(pos.y + relY * (oldH - newH));
        zoomRef.current = nextZoom;
        await appWindow.setSize(new PhysicalSize(newW, newH));
        await appWindow.setPosition(new PhysicalPosition(newX, newY));
        setZoomText(`${Math.round(nextZoom * 100)}%`);
        window.clearTimeout(zoomTimer.current);
        zoomTimer.current = window.setTimeout(() => setZoomText(''), 700);
    };

    useEffect(() => {
        const onWheel = (event) => {
            event.preventDefault();
            if (!infoRef.current) {
                return;
            }
            if (event.ctrlKey || event.metaKey) {
                const delta = event.deltaY < 0 ? 0.05 : -0.05;
                setOpacity((value) => clamp(Number((value + delta).toFixed(2)), 0.15, 1));
                return;
            }
            const factor = event.deltaY < 0 ? 1.1 : 1 / 1.1;
            const next = clamp(zoomRef.current * factor, 0.05, 8);
            void applyZoom(next, event.clientX, event.clientY);
        };
        window.addEventListener('wheel', onWheel, { passive: false });
        return () => window.removeEventListener('wheel', onWheel);
    }, []);

    const run = async (action) => {
        setMenu(null);
        try {
            if (action === 'copy') {
                await invoke('copy_pin_image');
                toast.success(t('pin.copied'), { style: toastStyle });
            } else if (action === 'save') {
                const path = await save({
                    defaultPath: 'pot-pin.png',
                    filters: [{ name: 'PNG', extensions: ['png'] }],
                });
                if (!path) {
                    return;
                }
                await invoke('save_pin_image', { path });
                toast.success(t('pin.saved'), { style: toastStyle });
            } else if (action === 'ocr') {
                await invoke('ocr_pin_image');
            } else if (action === 'translate') {
                await invoke('translate_pin_image');
            } else if (action === 'close') {
                await appWindow.close();
            } else if (action === 'zoom-in') {
                await applyZoom(clamp(zoomRef.current * 1.1, 0.05, 8), window.innerWidth / 2, window.innerHeight / 2);
            } else if (action === 'zoom-out') {
                await applyZoom(clamp(zoomRef.current / 1.1, 0.05, 8), window.innerWidth / 2, window.innerHeight / 2);
            } else if (action === 'zoom-reset') {
                const initial = infoRef.current?.zoom || 1;
                await applyZoom(initial, window.innerWidth / 2, window.innerHeight / 2);
            } else if (action === 'annotate') {
                await invoke('annotate_pin');
            }
        } catch (error) {
            toast.error(String(error), { style: toastStyle });
        }
    };

    const menuItems = [
        ['copy', t('pin.copy')],
        ['save', t('pin.save')],
        ['ocr', t('pin.ocr')],
        ['translate', t('pin.translate')],
        ['annotate', t('pin.annotate')],
        ['sep', ''],
        ['zoom-in', t('pin.zoom_in')],
        ['zoom-out', t('pin.zoom_out')],
        ['zoom-reset', t('pin.zoom_reset')],
        ['sep2', ''],
        ['close', t('pin.close')],
    ];

    return (
        <div className='h-screen w-screen bg-transparent'>
            <Toaster />
            {loadError ? (
                <div className='fixed inset-0 flex items-center justify-center bg-neutral-900 p-3 text-center text-xs text-red-200'>
                    {loadError}
                </div>
            ) : null}
            <img
                id='pin-image'
                alt=''
                src={url}
                draggable={false}
                className='fixed inset-0 h-full w-full select-none'
                style={{
                    opacity,
                    display: loadError ? 'none' : 'block',
                    outline: hover ? (dark ? '1px solid rgba(255,255,255,0.8)' : '1px solid rgba(0,0,0,0.45)') : '1px solid transparent',
                    boxShadow: hover ? '0 10px 28px rgba(0,0,0,0.28)' : 'none',
                }}
                onMouseEnter={() => setHover(true)}
                onMouseLeave={() => setHover(false)}
                onError={() => setLoadError(t('pin.load_failed'))}
                onMouseDown={(event) => {
                    if (event.button !== 0) {
                        return;
                    }
                    setMenu(null);
                    if (event.detail >= 2) {
                        void appWindow.close();
                        return;
                    }
                    void appWindow.startDragging();
                }}
                onDoubleClick={(event) => {
                    event.preventDefault();
                    void appWindow.close();
                }}
                onContextMenu={(event) => {
                    event.preventDefault();
                    event.stopPropagation();
                    setMenu({ x: event.clientX, y: event.clientY });
                }}
            />
            {zoomText ? (
                <div className='pointer-events-none fixed left-1/2 top-3 z-40 -translate-x-1/2 rounded-full bg-black/70 px-2 py-0.5 text-xs text-white'>
                    {zoomText}
                </div>
            ) : null}
            {menu && (
                <div
                    className={`fixed z-30 min-w-[180px] rounded-md border p-1 text-sm shadow-lg ${
                        dark ? 'border-white/10 bg-neutral-900/95 text-white' : 'border-black/10 bg-white/95 text-neutral-800'
                    }`}
                    style={{
                        left: Math.max(4, Math.min(menu.x, window.innerWidth - 196)),
                        top: Math.max(4, Math.min(menu.y, window.innerHeight - 280)),
                    }}
                    onMouseDown={(event) => event.stopPropagation()}
                    onContextMenu={(event) => event.preventDefault()}
                >
                    {menuItems.map(([action, label]) =>
                        action.startsWith('sep') ? (
                            <div
                                key={action}
                                className={`my-1 h-px ${dark ? 'bg-white/15' : 'bg-black/10'}`}
                            />
                        ) : (
                            <button
                                key={action}
                                type='button'
                                className={`block w-full rounded px-2 py-1 text-left ${dark ? 'hover:bg-white/15' : 'hover:bg-black/5'}`}
                                onClick={() => {
                                    void run(action);
                                }}
                            >
                                {label}
                            </button>
                        )
                    )}
                    <label className={`mt-1 block px-2 py-1 text-xs ${dark ? 'text-white/80' : 'text-neutral-600'}`}>
                        {t('pin.opacity')} {Math.round(opacity * 100)}%
                        <input
                            type='range'
                            min='0.15'
                            max='1'
                            step='0.05'
                            value={opacity}
                            className='mt-1 w-full'
                            onChange={(event) => setOpacity(Number(event.target.value))}
                        />
                    </label>
                </div>
            )}
        </div>
    );
}
