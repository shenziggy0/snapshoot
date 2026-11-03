import React from 'react';
import { useTranslation } from 'react-i18next';
import { MdPause, MdPlayArrow, MdStop, MdClose } from 'react-icons/md';

export default function RecordControls({ status, busy, onPause, onStop, onCancel }) {
    const { t } = useTranslation();
    const seconds = Math.floor((status?.elapsed_ms || 0) / 1000);
    const time = `${String(Math.floor(seconds / 60)).padStart(2, '0')}:${String(seconds % 60).padStart(2, '0')}`;
    const paused = status?.state === 'paused';
    const failed = status?.state === 'error';
    const button =
        'flex h-9 items-center gap-1 rounded-md px-2 hover:bg-white/15 focus-visible:outline focus-visible:outline-2 focus-visible:outline-white disabled:opacity-40';
    return (
        <div
            className='rounded-xl border border-white/20 bg-neutral-900/95 p-1.5 text-sm text-white shadow-lg'
            onMouseDown={(event) => event.stopPropagation()}
            aria-busy={busy}
        >
            <div className='flex items-center gap-2'>
                <span
                    className='ml-2 h-2 w-2 rounded-full bg-red-500'
                    aria-hidden='true'
                />
                <span className='min-w-[4.5rem] tabular-nums'>
                    {time}
                    {paused ? ` · ${t('screen_record.paused')}` : ''}
                </span>
                <button
                    type='button'
                    className={button}
                    disabled={busy || failed}
                    onClick={onPause}
                >
                    {paused ? <MdPlayArrow /> : <MdPause />}
                    {t(paused ? 'screen_record.resume' : 'screen_record.pause')}
                </button>
                <button
                    type='button'
                    className={button}
                    disabled={busy || failed}
                    onClick={onStop}
                >
                    <MdStop />
                    {t(busy ? 'screen_record.processing' : 'screen_record.stop')}
                </button>
                <button
                    type='button'
                    className={button}
                    disabled={busy}
                    onClick={onCancel}
                >
                    <MdClose />
                    {t('screen_record.cancel')}
                </button>
            </div>
            {status?.message && (
                <p
                    role='alert'
                    className='max-w-[28rem] px-2 py-1 text-red-200'
                >
                    {t(status.message)}
                </p>
            )}
        </div>
    );
}
