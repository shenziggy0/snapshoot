import {
    MdClose,
    MdCloudUpload,
    MdContentCopy,
    MdDeleteOutline,
    MdDocumentScanner,
    MdEdit,
    MdHistory,
    MdPushPin,
    MdSaveAlt,
} from 'react-icons/md';
import { convertFileSrc } from '@tauri-apps/api/tauri';
import { listen } from '@tauri-apps/api/event';
import { save } from '@tauri-apps/api/dialog';
import { writeText } from '@tauri-apps/api/clipboard';
import toast, { Toaster } from 'react-hot-toast';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api';
import React, { useCallback, useEffect, useRef, useState } from 'react';

import { useToastStyle } from '../../hooks';
import { useSystemDark } from '../useSystemDark';

const AUTO_HIDE_MS = 12000;

function formatTime(value) {
    if (!value) {
        return '';
    }
    return new Intl.DateTimeFormat(undefined, { hour: '2-digit', minute: '2-digit' }).format(new Date(value));
}

function ActionButton({ title, disabled, onClick, dark, children }) {
    return (
        <button
            type='button'
            title={title}
            aria-label={title}
            disabled={disabled}
            onClick={onClick}
            className={`flex h-8 flex-1 items-center justify-center rounded-md border text-[13px] transition-colors disabled:opacity-35 ${
                dark
                    ? 'border-white/10 text-white hover:bg-white/10'
                    : 'border-black/10 text-neutral-800 hover:bg-black/5'
            }`}
        >
            {children}
        </button>
    );
}

export default function QuickAccess() {
    const { t } = useTranslation();
    const dark = useSystemDark();
    const toastStyle = useToastStyle();
    const [items, setItems] = useState([]);
    const [current, setCurrent] = useState(null);
    const [showHistory, setShowHistory] = useState(false);
    const hideTimer = useRef(null);
    const hovering = useRef(false);

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
        if (hovering.current || showHistory) {
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
        const onKey = (event) => {
            if (event.key === 'Escape') {
                hide();
            }
        };
        window.addEventListener('keydown', onKey);
        scheduleHide();
        return () => {
            clearTimeout(hideTimer.current);
            unlisten.then((fn) => fn());
            window.removeEventListener('keydown', onKey);
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

    const card = dark ? 'border-white/10 bg-neutral-900/95 text-white' : 'border-black/10 bg-white/95 text-neutral-900';
    const muted = dark ? 'text-white/55' : 'text-neutral-500';
    const status = current
        ? current.action === 'save'
            ? t('annotate.saved')
            : current.action === 'pin'
              ? t('annotate.pin')
              : t('annotate.copied')
        : t('quick_access.empty');

    return (
        <div className='fixed inset-0 overflow-hidden bg-transparent p-2'>
            <Toaster />
            <div
                className={`relative flex h-full flex-col overflow-hidden rounded-xl border shadow-2xl backdrop-blur ${card}`}
                onMouseEnter={() => {
                    hovering.current = true;
                    clearTimeout(hideTimer.current);
                }}
                onMouseLeave={() => {
                    hovering.current = false;
                    scheduleHide();
                }}
            >
                <div
                    className={`flex h-8 shrink-0 items-center justify-between border-b px-3 ${
                        dark ? 'border-white/10' : 'border-black/[0.06]'
                    }`}
                    data-tauri-drag-region='true'
                >
                    <div
                        className='flex items-center gap-2 text-[12px] font-medium'
                        data-tauri-drag-region='true'
                    >
                        <span className={`h-1.5 w-1.5 rounded-full ${current ? 'bg-emerald-500' : 'bg-neutral-400'}`} />
                        {status}
                        {current && <span className={`font-normal ${muted}`}>{formatTime(current.createdAt)}</span>}
                    </div>
                    <div className='flex items-center gap-1'>
                        <button
                            type='button'
                            title={t('quick_access.history')}
                            onClick={() => {
                                setShowHistory((value) => !value);
                                clearTimeout(hideTimer.current);
                            }}
                            className={`flex h-6 w-6 items-center justify-center rounded-md ${
                                showHistory ? 'bg-sky-500 text-white' : dark ? 'hover:bg-white/10' : 'hover:bg-black/5'
                            }`}
                        >
                            <MdHistory size={15} />
                        </button>
                        <button
                            type='button'
                            title={t('annotate.close')}
                            onClick={hide}
                            className={`flex h-6 w-6 items-center justify-center rounded-md ${
                                dark ? 'hover:bg-white/10' : 'hover:bg-black/5'
                            }`}
                        >
                            <MdClose size={15} />
                        </button>
                    </div>
                </div>

                {showHistory ? (
                    <div className='flex min-h-0 flex-1 flex-col p-3'>
                        <div className='mb-2 flex items-center justify-between'>
                            <span className={`text-[12px] ${muted}`}>{t('quick_access.recent')}</span>
                            <span className={`text-[11px] ${muted}`}>{items.length}</span>
                        </div>
                        <div className='grid min-h-0 flex-1 grid-cols-3 gap-2 overflow-y-auto pr-1'>
                            {items.map((item) => (
                                <div
                                    key={item.id}
                                    className='group relative'
                                >
                                    <button
                                        type='button'
                                        onClick={() => {
                                            setCurrent(item);
                                            setShowHistory(false);
                                        }}
                                        className={`block h-[62px] w-full overflow-hidden rounded-md border ${
                                            current?.id === item.id
                                                ? 'border-sky-500'
                                                : dark
                                                  ? 'border-white/10'
                                                  : 'border-black/10'
                                        }`}
                                    >
                                        <img
                                            src={`${convertFileSrc(item.thumbPath)}?v=${item.createdAt}`}
                                            alt=''
                                            className='h-full w-full object-cover'
                                        />
                                    </button>
                                    <button
                                        type='button'
                                        title={t('quick_access.delete')}
                                        onClick={() => void remove(item.id)}
                                        className='absolute right-1 top-1 hidden h-5 w-5 items-center justify-center rounded bg-black/60 text-white group-hover:flex'
                                    >
                                        <MdDeleteOutline size={13} />
                                    </button>
                                </div>
                            ))}
                        </div>
                    </div>
                ) : (
                    <>
                        <div className='flex min-h-0 flex-1 gap-3 p-3'>
                            <button
                                type='button'
                                title={t('quick_access.edit')}
                                onClick={() => void run('edit')}
                                disabled={!current}
                                className={`h-[78px] w-[118px] shrink-0 overflow-hidden rounded-md border ${
                                    dark ? 'border-white/10 bg-white/5' : 'border-black/10 bg-black/[0.03]'
                                }`}
                            >
                                {current ? (
                                    <img
                                        src={`${convertFileSrc(current.thumbPath)}?v=${current.createdAt}`}
                                        alt=''
                                        className='h-full w-full object-cover'
                                    />
                                ) : null}
                            </button>
                            <div className='flex min-w-0 flex-1 flex-col justify-between py-0.5'>
                                <div className='min-w-0'>
                                    <div className='truncate text-[13px] font-medium'>
                                        {current
                                            ? `${current.width} × ${current.height}`
                                            : t('quick_access.no_capture')}
                                    </div>
                                    <div className={`mt-1 truncate text-[11px] ${muted}`}>
                                        {current ? t('quick_access.click_edit') : t('quick_access.empty_hint')}
                                    </div>
                                </div>
                                <div className='flex gap-1.5'>
                                    <ActionButton
                                        title={t('annotate.copy')}
                                        disabled={!current}
                                        onClick={() => run('copy')}
                                        dark={dark}
                                    >
                                        <MdContentCopy size={15} />
                                    </ActionButton>
                                    <ActionButton
                                        title={t('annotate.save')}
                                        disabled={!current}
                                        onClick={() => run('save')}
                                        dark={dark}
                                    >
                                        <MdSaveAlt size={16} />
                                    </ActionButton>
                                    <ActionButton
                                        title={t('quick_access.share')}
                                        disabled={!current}
                                        onClick={() => run('share')}
                                        dark={dark}
                                    >
                                        <MdCloudUpload size={16} />
                                    </ActionButton>
                                    <ActionButton
                                        title={t('annotate.pin')}
                                        disabled={!current}
                                        onClick={() => run('pin')}
                                        dark={dark}
                                    >
                                        <MdPushPin size={15} />
                                    </ActionButton>
                                    <ActionButton
                                        title={t('annotate.ocr')}
                                        disabled={!current}
                                        onClick={() => run('ocr')}
                                        dark={dark}
                                    >
                                        <MdDocumentScanner size={15} />
                                    </ActionButton>
                                    <ActionButton
                                        title={t('quick_access.edit')}
                                        disabled={!current}
                                        onClick={() => run('edit')}
                                        dark={dark}
                                    >
                                        <MdEdit size={15} />
                                    </ActionButton>
                                </div>
                            </div>
                        </div>
                        <div
                            className={`flex h-9 shrink-0 items-center gap-1.5 border-t px-3 ${dark ? 'border-white/10' : 'border-black/[0.06]'}`}
                        >
                            {items.slice(0, 7).map((item) => (
                                <button
                                    key={item.id}
                                    type='button'
                                    title={formatTime(item.createdAt)}
                                    onClick={() => setCurrent(item)}
                                    className={`h-7 w-10 shrink-0 overflow-hidden rounded border ${
                                        current?.id === item.id
                                            ? 'border-sky-500'
                                            : dark
                                              ? 'border-white/10'
                                              : 'border-black/10'
                                    }`}
                                >
                                    <img
                                        src={`${convertFileSrc(item.thumbPath)}?v=${item.createdAt}`}
                                        alt=''
                                        className='h-full w-full object-cover'
                                    />
                                </button>
                            ))}
                            <span className={`ml-auto text-[11px] ${muted}`}>{t('quick_access.recent')}</span>
                        </div>
                    </>
                )}
            </div>
        </div>
    );
}
