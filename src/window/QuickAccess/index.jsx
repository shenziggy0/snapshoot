import {
    Check,
    History,
    Image as ImageIcon,
    Trash2,
    X,
} from 'lucide-react';
import './style.css';
import { listen } from '@tauri-apps/api/event';
import toast from 'react-hot-toast';
import Toaster from '../../components/Toaster';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api';
import React, { useCallback, useEffect, useRef, useState } from 'react';

import { useToastStyle } from '../../hooks';
import { captureActions, captureErrorKey, CaptureThumb, runCaptureAction } from './captureActions';
import { useCaptureDrag } from '../Screenshot/useCaptureDrag';

const AUTO_HIDE_MS = 12000;

function formatTime(value) {
    if (!value) {
        return '';
    }
    return new Intl.DateTimeFormat(undefined, { hour: '2-digit', minute: '2-digit' }).format(new Date(value));
}

function DragCaptureButton({ item, onDragError, className, children, ...props }) {
    const drag = useCaptureDrag(item.id, onDragError);
    return (
        <button
            type='button'
            className={`${className}${drag.onPointerDown ? ' cursor-grab' : ''}`}
            {...props}
            {...drag}
        >
            <CaptureThumb key={item.id} item={item} />
            {children}
        </button>
    );
}

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
            const message = await runCaptureAction(current, action);
            if (message) toast.success(t(message), { style: toastStyle });
        } catch (error) {
            toast.error(t(captureErrorKey(error)), { style: toastStyle });
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
    const reportDragError = (error) => toast.error(t(captureErrorKey(error)), { style: toastStyle });

    return (
        <div className='quick-window'>
            <Toaster />
            <section
                className='quick-card'
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
                    className='quick-heading snap-glass'
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
                                <DragCaptureButton
                                    item={item}
                                    onDragError={reportDragError}
                                    className='quick-tile-select'
                                    title={`${describe(item)} · ${t('quick_access.drag_label')}`}
                                    aria-label={describe(item)}
                                    aria-pressed={current?.id === item.id}
                                    onClick={(event) => {
                                        setCurrent(item);
                                        setShowHistory(false);
                                        if (event.detail === 0) historyButton.current?.focus();
                                    }}
                                >
                                    <span>{formatTime(item.createdAt)}</span>
                                </DragCaptureButton>
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
                        <DragCaptureButton
                            item={current}
                            onDragError={reportDragError}
                            className='quick-preview'
                            title={`${t('quick_access.click_edit')} · ${t('quick_access.drag_label')}`}
                            aria-label={t('quick_access.click_edit')}
                            onClick={() => run('edit')}
                        >
                            <span className='quick-dimensions'>
                                {current.width} × {current.height}
                            </span>
                        </DragCaptureButton>
                        <div className='quick-actions snap-glass'>
                            {captureActions.map(([action, key, Icon]) => (
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
