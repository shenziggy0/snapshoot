import {
    Check,
    Copy,
    Download,
    History,
    Image as ImageIcon,
    Pencil,
    Pin,
    ScanText,
    Trash2,
    Upload,
    X,
} from 'lucide-react';
import './style.css';
import { convertFileSrc } from '@tauri-apps/api/tauri';
import { listen } from '@tauri-apps/api/event';
import { save } from '@tauri-apps/api/dialog';
import { writeText } from '@tauri-apps/api/clipboard';
import toast from 'react-hot-toast';
import Toaster from '../../components/Toaster';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api';
import React, { useCallback, useEffect, useRef, useState } from 'react';

import { useToastStyle } from '../../hooks';

const AUTO_HIDE_MS = 12000;

function formatTime(value) {
    if (!value) {
        return '';
    }
    return new Intl.DateTimeFormat(undefined, { hour: '2-digit', minute: '2-digit' }).format(new Date(value));
}

function CaptureThumb({ item }) {
    return (
        <img
            src={`${convertFileSrc(item.thumbPath)}?v=${item.createdAt}`}
            alt=''
            draggable={false}
        />
    );
}

const actions = [
    ['copy', 'annotate.copy', Copy],
    ['edit', 'quick_access.edit', Pencil],
    ['save', 'annotate.save', Download],
    ['pin', 'annotate.pin', Pin],
    ['ocr', 'annotate.ocr', ScanText],
    ['share', 'quick_access.share', Upload],
];

export default function QuickAccess() {
    const { t } = useTranslation();
    const toastStyle = useToastStyle();
    const [items, setItems] = useState([]);
    const [current, setCurrent] = useState(null);
    const [showHistory, setShowHistory] = useState(false);
    const hideTimer = useRef(null);
    const historyButton = useRef(null);
    const hovering = useRef(false);
    const focusing = useRef(false);

    useEffect(() => {
        document.documentElement.style.background = 'transparent';
        document.body.style.background = 'transparent';
        document.body.style.overflow = 'hidden';
        const root = document.getElementById('root');
        if (root) {
            root.style.height = '100%';
            root.style.background = 'transparent';
        }
    }, []);

    const hide = useCallback(() => {
        invoke('hide_quick_access_window').catch(() => {});
    }, []);

    const scheduleHide = useCallback(() => {
        clearTimeout(hideTimer.current);
        if (hovering.current || focusing.current || showHistory) {
            return;
        }
        hideTimer.current = setTimeout(hide, AUTO_HIDE_MS);
    }, [hide, showHistory]);

    const refresh = useCallback(async () => {
        try {
            const next = await invoke('capture_history_list');
            setItems(next);
            setCurrent((previous) => next.find((item) => item.id === previous?.id) || next[0] || null);
            scheduleHide();
        } catch (error) {
            console.error('load capture history failed', error);
        }
    }, [scheduleHide]);

    useEffect(() => {
        void refresh();
        const unlisten = listen('quick-access-update', (event) => {
            const item = event.payload;
            setItems((previous) => [item, ...previous.filter((candidate) => candidate.id !== item.id)].slice(0, 60));
            setCurrent(item);
            setShowHistory(false);
            scheduleHide();
        });
        scheduleHide();
        return () => {
            clearTimeout(hideTimer.current);
            unlisten.then((fn) => fn());
        };
    }, [hide, refresh, scheduleHide]);

    const run = async (action) => {
        if (!current) {
            return;
        }
        try {
            if (action === 'copy') {
                await invoke('capture_history_copy', { id: current.id });
                toast.success(t('annotate.copied'), { style: toastStyle });
            } else if (action === 'save') {
                const path = await save({
                    defaultPath: 'snapshoot-screenshot.png',
                    filters: [{ name: 'PNG', extensions: ['png'] }],
                });
                if (!path) {
                    return;
                }
                await invoke('capture_history_save', { id: current.id, path });
                toast.success(t('annotate.saved'), { style: toastStyle });
            } else if (action === 'share') {
                const url = await invoke('capture_history_share', { id: current.id });
                await writeText(url);
                toast.success(t('quick_access.share_copied'), { style: toastStyle });
            } else if (action === 'pin') {
                await invoke('capture_history_pin', { id: current.id });
            } else if (action === 'ocr') {
                await invoke('capture_history_ocr', { id: current.id });
            } else if (action === 'edit') {
                await invoke('capture_history_edit', { id: current.id });
            }
        } catch (error) {
            const message = String(error);
            toast.error(message.startsWith('quick_access.') ? t(message) : message, { style: toastStyle });
        }
    };

    const remove = async (id) => {
        try {
            await invoke('capture_history_delete', { id });
            const next = items.filter((item) => item.id !== id);
            setItems(next);
            if (current?.id === id) {
                setCurrent(next[0] || null);
            }
        } catch (error) {
            toast.error(String(error), { style: toastStyle });
        }
    };

    const status = current
        ? current.action === 'save'
            ? t('annotate.saved')
            : current.action === 'pin'
              ? t('annotate.pin')
              : t('annotate.copied')
        : t('quick_access.empty');

    const describe = (item) => `${item.width} × ${item.height} · ${formatTime(item.createdAt)}`;

    return (
        <div className='quick-window'>
            <Toaster />
            <section
                className='quick-card snap-glass'
                onMouseEnter={() => {
                    hovering.current = true;
                    clearTimeout(hideTimer.current);
                }}
                onMouseLeave={() => {
                    hovering.current = false;
                    scheduleHide();
                }}
                onFocusCapture={(event) => {
                    focusing.current = event.target.matches(':focus-visible');
                    clearTimeout(hideTimer.current);
                }}
                onBlurCapture={(event) => {
                    if (!event.currentTarget.contains(event.relatedTarget)) {
                        focusing.current = false;
                        scheduleHide();
                    }
                }}
            >
                <header
                    className='quick-heading'
                    data-tauri-drag-region='true'
                >
                    <span
                        className='quick-status'
                        role='status'
                        data-success={Boolean(current) && !showHistory}
                        data-tauri-drag-region='true'
                    >
                        {showHistory ? <History /> : current ? <Check /> : <ImageIcon />}
                        {showHistory ? t('quick_access.history') : status}
                    </span>
                    {current && !showHistory && <time>{formatTime(current.createdAt)}</time>}
                    <button
                        className='quick-icon'
                        type='button'
                        ref={historyButton}
                        title={t('quick_access.history')}
                        aria-label={t('quick_access.history')}
                        aria-expanded={showHistory}
                        aria-pressed={showHistory}
                        onClick={() => setShowHistory(!showHistory)}
                    >
                        <History />
                    </button>
                    <button
                        className='quick-icon'
                        type='button'
                        title={t('annotate.close')}
                        onClick={hide}
                    >
                        <X />
                    </button>
                </header>
                {showHistory && items.length > 0 ? (
                    <div className='quick-history'>
                        {items.map((item) => (
                            <div
                                className='quick-tile'
                                key={item.id}
                            >
                                <button
                                    className='quick-tile-select'
                                    title={describe(item)}
                                    aria-pressed={current?.id === item.id}
                                    onClick={(event) => {
                                        setCurrent(item);
                                        setShowHistory(false);
                                        if (event.detail === 0) historyButton.current?.focus();
                                    }}
                                >
                                    <CaptureThumb item={item} />
                                    <span>{formatTime(item.createdAt)}</span>
                                </button>
                                <button
                                    type='button'
                                    className='quick-delete'
                                    title={t('quick_access.delete')}
                                    onClick={() => remove(item.id)}
                                >
                                    <Trash2 size={14} />
                                </button>
                            </div>
                        ))}
                    </div>
                ) : current ? (
                    <>
                        <button
                            className='quick-preview'
                            type='button'
                            title={t('quick_access.click_edit')}
                            onClick={() => run('edit')}
                        >
                            <CaptureThumb key={current.id} item={current} />
                            <span className='quick-dimensions'>
                                {current.width} × {current.height}
                            </span>
                        </button>
                        <div className='quick-actions'>
                            {actions.map(([action, key, Icon]) => (
                                <button
                                    type='button'
                                    key={action}
                                    title={t(key)}
                                    aria-label={t(key)}
                                    className={action === 'copy' ? 'quick-primary' : 'quick-icon'}
                                    onClick={() => run(action)}
                                >
                                    <Icon
                                        size={16}
                                        aria-hidden='true'
                                    />
                                    {action === 'copy' && t(key)}
                                </button>
                            ))}
                        </div>
                        <footer className='quick-strip'>
                            <span>{t('quick_access.recent')}</span>
                            {items.slice(0, 6).map((item) => (
                                <button
                                    type='button'
                                    key={item.id}
                                    title={describe(item)}
                                    aria-pressed={current.id === item.id}
                                    onClick={() => setCurrent(item)}
                                >
                                    <CaptureThumb item={item} />
                                </button>
                            ))}
                        </footer>
                    </>
                ) : (
                    <div className='quick-empty'>
                        <ImageIcon size={28} />
                        <p>{t('quick_access.empty_hint')}</p>
                    </div>
                )}
            </section>
        </div>
    );
}
