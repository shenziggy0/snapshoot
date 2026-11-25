import React, { useCallback, useEffect, useState } from 'react';
import { Button, Tab, Tabs } from '@nextui-org/react';
import { invoke } from '@tauri-apps/api';
import { listen } from '@tauri-apps/api/event';
import { appWindow } from '@tauri-apps/api/window';
import toast from 'react-hot-toast';
import { useTranslation } from 'react-i18next';
import Toaster from '../../../../components/Toaster';
import { useToastStyle } from '../../../../hooks';
import { captureActions, captureErrorKey, CaptureThumb, runCaptureAction } from '../../../QuickAccess/captureActions';
import { useCaptureDrag } from '../../../Screenshot/useCaptureDrag';
import History from '../History';
import CaptureComposition from './CaptureComposition';

function useHistoryList(command, event) {
    const [state, setState] = useState({ items: [], error: '', loading: true });
    useEffect(() => {
        let mounted = true;
        let request = 0;
        const refresh = async () => {
            const current = ++request;
            try {
                const items = await invoke(command);
                if (mounted && current === request) setState({ items, error: '', loading: false });
            } catch (error) {
                if (mounted && current === request) {
                    setState((previous) => ({ ...previous, error: captureErrorKey(error), loading: false }));
                }
            }
        };
        const changeSubscription = listen(event, refresh);
        const focusSubscription = appWindow.onFocusChanged(({ payload }) => {
            if (payload) void refresh();
        });
        void changeSubscription.then(refresh, refresh);
        return () => {
            mounted = false;
            for (const subscription of [changeSubscription, focusSubscription]) {
                void subscription.then(
                    (unlisten) => unlisten(),
                    () => {}
                );
            }
        };
    }, [command, event]);
    return state;
}

function ScreenshotTile({ item, selected, checked, onSelect, onCheck, onError }) {
    const { t } = useTranslation();
    const drag = useCaptureDrag(item.id, onError);
    const date = new Date(item.createdAt).toLocaleString();
    const description = `${item.width} × ${item.height} · ${date}`;
    return (
        <li
            className={`overflow-hidden rounded-xl border bg-content1 ${selected ? 'border-primary' : 'border-divider'}`}
        >
            <button
                type='button'
                className='relative flex h-24 w-full cursor-grab items-center justify-center bg-default-100 [&_img]:max-h-full [&_img]:max-w-full'
                title={t('quick_access.drag_label')}
                aria-label={description}
                aria-pressed={selected}
                {...drag}
                onClick={onSelect}
            >
                <CaptureThumb item={item} />
            </button>
            <label className='flex items-center gap-2 p-2 text-xs'>
                <input
                    type='checkbox'
                    checked={checked}
                    onChange={onCheck}
                />
                {description}
            </label>
        </li>
    );
}

function Screenshots({ reportError }) {
    const { t } = useTranslation();
    const toastStyle = useToastStyle();
    const { items, error, loading } = useHistoryList('capture_history_list', 'capture-history-changed');
    const [currentId, setCurrentId] = useState(null);
    const [checkedIds, setCheckedIds] = useState([]);
    const [composition, setComposition] = useState(null);
    const current = items.find((item) => item.id === currentId) ?? items[0];
    const checked = checkedIds.map((id) => items.find((item) => item.id === id)).filter(Boolean);

    const run = async (action) => {
        try {
            const message =
                action === 'recapture'
                    ? await invoke('recapture_region', { id: current.id })
                    : await runCaptureAction(current, action);
            if (typeof message === 'string') toast.success(t(message), { style: toastStyle });
        } catch (cause) {
            reportError(cause);
        }
    };
    const startComposition = (kind) => setComposition({ kind, items: checked });

    if (loading) return <p role='status'>{t('recent.loading')}</p>;
    if (error) return <p role='alert'>{t(error)}</p>;
    if (!current) return <p>{t('quick_access.empty_hint')}</p>;
    return (
        <div className='grid gap-4'>
            <div
                className='flex flex-wrap gap-2'
                aria-label={t('recent.screenshot_actions')}
            >
                {captureActions.map(([action, key, Icon]) => (
                    <Button
                        key={action}
                        size='sm'
                        variant='flat'
                        startContent={<Icon size={14} />}
                        onPress={() => run(action)}
                    >
                        {t(key)}
                    </Button>
                ))}
                {current.region && (
                    <Button
                        size='sm'
                        variant='flat'
                        onPress={() => run('recapture')}
                    >
                        {t('recent.recapture')}
                    </Button>
                )}
            </div>
            <div className='flex flex-wrap gap-2'>
                <Button
                    size='sm'
                    color='primary'
                    isDisabled={checked.length !== 2}
                    onPress={() => startComposition('compare')}
                >
                    {t('recent.compare')}
                </Button>
                <Button
                    size='sm'
                    variant='flat'
                    isDisabled={!checked.length}
                    onPress={() => startComposition('tutorial')}
                >
                    {t('recent.tutorial')}
                </Button>
            </div>
            <ul className='grid grid-cols-[repeat(auto-fill,minmax(125px,1fr))] gap-3'>
                {items.map((item) => (
                    <ScreenshotTile
                        key={item.id}
                        item={item}
                        selected={item.id === current.id}
                        checked={checkedIds.includes(item.id)}
                        onSelect={() => setCurrentId(item.id)}
                        onCheck={(event) => {
                            const enabled = event.currentTarget.checked;
                            setCheckedIds((ids) => (enabled ? [...ids, item.id] : ids.filter((id) => id !== item.id)));
                        }}
                        onError={reportError}
                    />
                ))}
            </ul>
            {composition && (
                <CaptureComposition
                    {...composition}
                    onClose={() => setComposition(null)}
                />
            )}
        </div>
    );
}

function Recordings({ reportError }) {
    const { t } = useTranslation();
    const toastStyle = useToastStyle();
    const { items, error, loading } = useHistoryList('record_history_list', 'record-history-changed');
    const [forgotten, setForgotten] = useState([]);
    const run = async (id, action) => {
        try {
            await invoke('record_history_action', { id, action });
            if (action === 'forget') setForgotten((ids) => [...ids, id]);
            if (action === 'copy') toast.success(t('screen_record.copied'), { style: toastStyle });
        } catch (cause) {
            reportError(cause);
        }
    };
    if (loading) return <p role='status'>{t('recent.loading')}</p>;
    if (error) return <p role='alert'>{t(error)}</p>;
    const visible = items.filter(({ id }) => !forgotten.includes(id));
    if (!visible.length) return <p>{t('record_history.empty')}</p>;
    return (
        <ul className='grid gap-2'>
            {visible.map((item) => (
                <li
                    key={item.id}
                    className='grid gap-2 rounded-xl border border-divider bg-content1 p-3'
                >
                    <strong className='break-all text-sm'>{item.path.split(/[\\/]/).pop()}</strong>
                    <span className='text-xs text-default-600'>
                        {new Date(item.createdAt).toLocaleString()} · {item.width} × {item.height} ·{' '}
                        {(item.durationMs / 1000).toFixed(1)} s
                    </span>
                    {!item.exists && <span className='text-sm text-warning'>{t('record_history.file_missing')}</span>}
                    <div className='flex flex-wrap gap-2'>
                        {item.exists &&
                            [
                                ['open', 'record_history.open'],
                                ['copy', 'record_history.copy'],
                                ['reveal', 'screen_record.reveal'],
                            ].map(([action, key]) => (
                                <Button
                                    key={action}
                                    size='sm'
                                    variant='flat'
                                    onPress={() => run(item.id, action)}
                                >
                                    {t(key)}
                                </Button>
                            ))}
                        <Button
                            size='sm'
                            variant='light'
                            onPress={() => run(item.id, 'forget')}
                        >
                            {t('record_history.forget')}
                        </Button>
                    </div>
                </li>
            ))}
        </ul>
    );
}

export default function Recent() {
    const { t } = useTranslation();
    const toastStyle = useToastStyle();
    const reportError = useCallback(
        (error) => toast.error(t(captureErrorKey(error)), { style: toastStyle }),
        [t, toastStyle]
    );
    return (
        <>
            <Toaster />
            <Tabs aria-label={t('config.recent.label')}>
                <Tab
                    key='captures'
                    title={t('recent.screenshots')}
                >
                    <Screenshots reportError={reportError} />
                </Tab>
                <Tab
                    key='recordings'
                    title={t('config.record.label')}
                >
                    <Recordings reportError={reportError} />
                </Tab>
                <Tab
                    key='translations'
                    title={t('recent.translations')}
                >
                    <History embedded />
                </Tab>
            </Tabs>
        </>
    );
}
