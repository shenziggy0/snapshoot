import { Pause, Play, Square, X } from 'lucide-react';
import React from 'react';
import { useTranslation } from 'react-i18next';

export default function RecordControls({ status, busy, onPause, onStop, onCancel }) {
    const { t } = useTranslation();
    const seconds = Math.floor((status?.elapsed_ms || 0) / 1000);
    const time = `${String(Math.floor(seconds / 60)).padStart(2, '0')}:${String(seconds % 60).padStart(2, '0')}`;
    const paused = status?.state === 'paused';
    const failed = status?.state === 'error';
    return (
        <div
            className='snap-record snap-glass'
            data-paused={paused || failed}
            onMouseDown={(event) => event.stopPropagation()}
            aria-busy={busy}
        >
            <div className='snap-record-row'>
                <span
                    className='snap-record-dot'
                    aria-hidden='true'
                />
                <span className='min-w-[4.5rem] tabular-nums'>
                    {time}
                    {paused ? ` · ${t('screen_record.paused')}` : ''}
                </span>
                <button
                    type='button'
                    disabled={busy || failed}
                    onClick={onPause}
                >
                    {paused ? <Play /> : <Pause />}
                    {t(paused ? 'screen_record.resume' : 'screen_record.pause')}
                </button>
                <button
                    type='button'
                    disabled={busy || failed}
                    onClick={onStop}
                >
                    <Square />
                    {t(busy ? 'screen_record.processing' : 'screen_record.stop')}
                </button>
                <button
                    type='button'
                    disabled={busy}
                    onClick={onCancel}
                >
                    <X />
                    {t('screen_record.cancel')}
                </button>
            </div>
            {status?.message && (
                <p
                    role='alert'
                    className='max-w-[28rem] px-2 py-1 text-danger'
                >
                    {t(status.message)}
                </p>
            )}
        </div>
    );
}
