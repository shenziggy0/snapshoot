import { FolderOpen, Trash2, Video, X } from 'lucide-react';
import React, { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api';
import { listen } from '@tauri-apps/api/event';
import { appWindow } from '@tauri-apps/api/window';
import { useTranslation } from 'react-i18next';
import { Button } from '@nextui-org/react';
import { osType } from '../../utils/env';

export default function RecordResult() {
    const { t } = useTranslation();
    const [result, setResult] = useState(null);
    const [error, setError] = useState('');
    const [busy, setBusy] = useState(false);
    const timer = useRef(null);
    useEffect(() => {
        let alive = true;
        const reload = async () => {
            try {
                const result = await invoke('screen_record_last_result');
                if (!alive) return;
                setResult(result);
                setError('');
                clearTimeout(timer.current);
                timer.current = setTimeout(() => appWindow.close(), 8000);
            } catch (e) {
                if (alive) setError(String(e));
            }
        };
        const unlisten = listen('record-result-ready', reload);
        reload();
        return () => {
            alive = false;
            clearTimeout(timer.current);
            unlisten.then((f) => f());
        };
    }, []);
    const action = async (name) => {
        if (busy) return;
        clearTimeout(timer.current);
        setBusy(true);
        try {
            await invoke(`screen_record_${name}_last`);
            if (name === 'delete') await appWindow.close();
            else timer.current = setTimeout(() => appWindow.close(), 8000);
        } catch (e) {
            setError(String(e));
        } finally {
            setBusy(false);
        }
    };
    return (
        <div className='record-result' data-platform={osType}>
            <header data-tauri-drag-region='true'>
                <h1 className='text-base font-semibold'>{t('screen_record.saved')}</h1>
                <Button
                    isIconOnly
                    size='sm'
                    variant='light'
                    aria-label={t('annotate.close')}
                    onPress={() => appWindow.close()}
                >
                    <X size={18} />
                </Button>
            </header>
            {result && (
                <>
                    <div className='record-result-details'>
                        <div className='record-result-icon'>
                            <Video
                                size={30}
                                aria-hidden='true'
                            />
                        </div>
                        <div className='min-w-0 text-sm'>
                            <p>
                                {result.width} × {result.height} · {(result.duration_ms / 1000).toFixed(1)} s
                            </p>
                            <p>
                                {(result.size_bytes / 1024 / 1024).toFixed(2)} MB · {result.fps} fps
                            </p>
                            <p
                                className='truncate text-xs text-default-500'
                                title={result.path}
                            >
                                {result.path.split(/[\\/]/).pop()}
                            </p>
                        </div>
                    </div>
                    <p className='mb-2 text-xs text-default-500'>
                        {t(result.clipboard_copied ? 'screen_record.copied' : 'screen_record.clipboard_failed')}
                    </p>
                    <div className='flex gap-2'>
                        <Button
                            size='sm'
                            variant='flat'
                            color='primary'
                            isDisabled={busy}
                            onPress={() => action('reveal')}
                            startContent={<FolderOpen size="1em" />}
                        >
                            {t('screen_record.reveal')}
                        </Button>
                        <Button
                            size='sm'
                            variant='light'
                            color='danger'
                            isDisabled={busy}
                            onPress={() => action('delete')}
                            startContent={<Trash2 size="1em" />}
                        >
                            {t('screen_record.delete')}
                        </Button>
                    </div>
                </>
            )}
            {error && (
                <p
                    role='alert'
                    className='mt-2 text-xs text-danger'
                >
                    {t(error)}
                </p>
            )}
        </div>
    );
}
