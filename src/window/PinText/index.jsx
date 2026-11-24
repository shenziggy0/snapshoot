import { Copy, X } from 'lucide-react';
import { appWindow } from '@tauri-apps/api/window';
import { writeText } from '@tauri-apps/api/clipboard';
import toast from 'react-hot-toast';
import Toaster from '../../components/Toaster';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api';
import React, { useEffect, useState } from 'react';

import { useToastStyle } from '../../hooks';

export default function PinText() {
    const { t } = useTranslation();
    const toastStyle = useToastStyle();
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
    }, []);

    return (
        <div className='fixed inset-0 overflow-hidden p-1.5'>
            <Toaster />
            <div className='snap-note'>
                <div
                    className='snap-note-heading'
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
                        >
                            <Copy size={14} />
                        </button>
                        <button
                            type='button'
                            title={t('annotate.close')}
                            onClick={() => appWindow.close()}
                        >
                            <X size={14} />
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
