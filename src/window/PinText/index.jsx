import { appWindow } from '@tauri-apps/api/window';
import { writeText } from '@tauri-apps/api/clipboard';
import toast, { Toaster } from 'react-hot-toast';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api';
import React, { useEffect, useState } from 'react';
import { MdClose, MdContentCopy } from 'react-icons/md';

import { useToastStyle } from '../../hooks';
import { useSystemDark } from '../useSystemDark';

export default function PinText() {
    const { t } = useTranslation();
    const toastStyle = useToastStyle();
    const dark = useSystemDark();
    const [text, setText] = useState('');
    const [error, setError] = useState('');

    useEffect(() => {
        document.documentElement.style.background = 'transparent';
        document.body.style.background = 'transparent';
        document.body.style.overflow = 'hidden';
        const root = document.getElementById('root');
        if (root) {
            root.style.height = '100%';
            root.style.background = 'transparent';
        }
        invoke('pin_text_content')
            .then(async (value) => {
                setText(value);
                await appWindow.show();
                await appWindow.setFocus();
                await appWindow.setAlwaysOnTop(true);
            })
            .catch((reason) => setError(String(reason)));
        const onKey = async (event) => {
            if (event.key === 'Escape') {
                await appWindow.close();
            }
        };
        window.addEventListener('keydown', onKey);
        return () => window.removeEventListener('keydown', onKey);
    }, []);

    return (
        <div className='fixed inset-0 overflow-hidden p-1.5'>
            <Toaster />
            <div
                className={`flex h-full flex-col overflow-hidden rounded-xl border shadow-2xl ${
                    dark
                        ? 'border-white/15 bg-neutral-900/95 text-white'
                        : 'border-black/10 bg-white/95 text-neutral-900'
                }`}
            >
                <div
                    className={`flex h-8 shrink-0 items-center justify-between border-b px-2.5 ${
                        dark ? 'border-white/10' : 'border-black/[0.06]'
                    }`}
                    data-tauri-drag-region='true'
                >
                    <span
                        className='text-[12px] font-medium'
                        data-tauri-drag-region='true'
                    >
                        {t('annotate.text')}
                    </span>
                    <div className='flex items-center gap-1'>
                        <button
                            type='button'
                            title={t('common.write_clipboard')}
                            onClick={async () => {
                                await writeText(text);
                                toast.success(t('annotate.copied'), { style: toastStyle });
                            }}
                            className='flex h-6 w-6 items-center justify-center rounded hover:bg-black/10'
                        >
                            <MdContentCopy size={14} />
                        </button>
                        <button
                            type='button'
                            title={t('annotate.close')}
                            onClick={() => appWindow.close()}
                            className='flex h-6 w-6 items-center justify-center rounded hover:bg-black/10'
                        >
                            <MdClose size={14} />
                        </button>
                    </div>
                </div>
                <div className='min-h-0 flex-1 select-text overflow-y-auto whitespace-pre-wrap break-words px-3 py-2.5 text-[13px] leading-5'>
                    {error || text}
                </div>
            </div>
        </div>
    );
}
