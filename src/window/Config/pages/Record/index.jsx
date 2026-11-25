import React, { useEffect, useState } from 'react';
import { Card, CardBody, Select, SelectItem, Switch, Button, Input } from '@nextui-org/react';
import { Video } from 'lucide-react';
import { invoke } from '@tauri-apps/api';
import { open } from '@tauri-apps/api/dialog';
import { useTranslation } from 'react-i18next';
import { useConfig } from '../../../../hooks';
import { osType } from '../../../../utils/env';
import { store } from '../../../../utils/store';

export default function Record() {
    const { t } = useTranslation();
    const [resolution, setResolution] = useConfig('record_resolution', 'original');
    const [fps, setFps] = useConfig('record_fps', 30);
    const [quality, setQuality] = useConfig('record_quality', 'standard');
    const [cursor, setCursor] = useConfig('record_cursor', true);
    const [audio, setAudio] = useConfig('record_system_audio', false);
    const [microphone, setMicrophone] = useConfig('record_microphone', false);
    const [directory, setDirectory] = useConfig('record_save_dir', '');
    const [support, setSupport] = useState(null);
    const [error, setError] = useState('');
    const [starting, setStarting] = useState(false);
    const microphoneUnavailable = microphone === true && support && !support.microphone;
    const audioUnavailable = microphoneUnavailable || (audio === true && support && !support.system_audio);
    const source =
        audio && microphone ? 'both_sources' : audio ? 'system_source' : microphone ? 'microphone_source' : 'no_audio';
    const settingsReady = [resolution, fps, quality, cursor, audio, microphone, directory].every(
        (value) => value !== null
    );

    useEffect(() => {
        let active = true;
        const refreshSupport = async () => {
            try {
                const capabilities = await invoke('screen_record_support');
                if (active) setSupport(capabilities);
            } catch (e) {
                if (active) setError(String(e));
            }
        };
        void refreshSupport();
        window.addEventListener('focus', refreshSupport);
        return () => {
            active = false;
            window.removeEventListener('focus', refreshSupport);
        };
    }, []);

    const beginRecording = async () => {
        setStarting(true);
        setError('');
        try {
            // The configuration fields debounce saves. Flush the values shown
            // next to Start so an immediate click uses the selected settings.
            await Promise.all(
                Object.entries({
                    record_resolution: resolution,
                    record_fps: fps,
                    record_quality: quality,
                    record_cursor: cursor,
                    record_system_audio: audio,
                    record_microphone: microphone,
                    record_save_dir: directory,
                }).map(([key, value]) => store.set(key, value))
            );
            await store.save();
            await invoke('reload_store');
            await invoke('start_capture', { mode: 'record' });
        } catch (e) {
            setError(String(e));
        } finally {
            setStarting(false);
        }
    };
    const openPermissions = async () => {
        try {
            await invoke('open_capture_privacy_settings', {
                source:
                    microphoneUnavailable || (microphone && error === 'screen_record.audio_unavailable')
                        ? 'microphone'
                        : 'screen',
            });
        } catch (e) {
            setError(String(e));
        }
    };
    const chooseDirectory = async () => {
        try {
            const selected = await open({ directory: true, multiple: false });
            if (selected) setDirectory(selected);
        } catch (e) {
            setError(String(e));
        }
    };
    return (
        <Card>
            <CardBody className='record-preferences'>
                {support?.supported && (
                    <div className='record-command'>
                        <div>
                            <Button
                                color='primary'
                                size='sm'
                                startContent={<Video size={15} />}
                                isLoading={starting}
                                isDisabled={!settingsReady || Boolean(audioUnavailable) || starting}
                                onPress={beginRecording}
                            >
                                {t('config.record.begin')}
                            </Button>
                        </div>
                        <p className='text-xs text-default-600'>
                            {t('config.record.source_summary', { source: t(`config.record.${source}`) })}
                        </p>
                        <p className='select-text break-all text-xs text-default-600'>
                            {t('config.record.destination_summary', {
                                path: directory || support.save_dir || t('config.record.default_dir'),
                            })}
                        </p>
                    </div>
                )}
                {support && !support.supported && (
                    <p className='text-sm text-default-500'>{t('screen_record.unsupported')}</p>
                )}
                <div className='record-format'>
                    {[
                        ['resolution', resolution, setResolution, ['original', '1080p', '720p']],
                        ['fps', fps, (v) => setFps(Number(v)), ['15', '30', '60']],
                        ['quality', quality, setQuality, ['high', 'standard', 'small']],
                    ].map(([key, value, setValue, choices]) => (
                        <Select
                            key={key}
                            label={t(`config.record.${key}`)}
                            selectedKeys={value === null ? [] : [String(value)]}
                            disallowEmptySelection
                            onSelectionChange={(keys) => {
                                const next = Array.from(keys)[0];
                                if (next) setValue(next);
                            }}
                        >
                            {choices.map((choice) => (
                                <SelectItem key={choice}>
                                    {key === 'fps' ? `${choice} fps` : t(`config.record.${choice}`)}
                                </SelectItem>
                            ))}
                        </Select>
                    ))}
                </div>
                <Switch
                    isSelected={cursor === true}
                    onValueChange={setCursor}
                >
                    {t('config.record.cursor')}
                </Switch>
                {(support?.system_audio || audio) && (
                    <Switch
                        isSelected={audio === true}
                        isDisabled={!support?.dual_audio && microphone === true && audio !== true}
                        onValueChange={setAudio}
                    >
                        {t('config.record.system_audio')}
                    </Switch>
                )}
                {(support?.microphone || microphone) && (
                    <Switch
                        isSelected={microphone === true}
                        onValueChange={(value) => {
                            setMicrophone(value);
                            if (value && !support?.dual_audio && audio === true) {
                                setAudio(false);
                            }
                        }}
                    >
                        {t(
                            support?.dual_audio || !support?.system_audio
                                ? 'config.record.microphone_independent'
                                : 'config.record.microphone'
                        )}
                    </Switch>
                )}
                <Input
                    label={t('config.record.save_dir')}
                    value={directory || ''}
                    placeholder={t('config.record.default_dir')}
                    onValueChange={setDirectory}
                />
                <div className='flex gap-2'>
                    <Button
                        variant='flat'
                        onPress={chooseDirectory}
                    >
                        {t('config.record.choose_dir')}
                    </Button>
                    <Button
                        variant='light'
                        onPress={() => setDirectory('')}
                    >
                        {t('config.record.reset_dir')}
                    </Button>
                </div>
                {(error || audioUnavailable) && (
                    <p
                        role='alert'
                        className='text-sm text-danger'
                    >
                        {t(error || 'screen_record.audio_unavailable')}
                    </p>
                )}
                {osType === 'Darwin' &&
                    (microphoneUnavailable ||
                        error === 'screen_record.permission_denied' ||
                        (microphone && error === 'screen_record.audio_unavailable')) && (
                        <div>
                            <Button
                                variant='flat'
                                onPress={openPermissions}
                            >
                                {t('capture.open_settings')}
                            </Button>
                        </div>
                    )}
            </CardBody>
        </Card>
    );
}
