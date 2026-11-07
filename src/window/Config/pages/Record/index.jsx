import React, { useEffect, useState } from 'react';
import { Card, CardBody, Select, SelectItem, Switch, Button, Input } from '@nextui-org/react';
import { invoke } from '@tauri-apps/api';
import { open } from '@tauri-apps/api/dialog';
import { useTranslation } from 'react-i18next';
import { useConfig } from '../../../../hooks';

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
    useEffect(() => {
        invoke('screen_record_support')
            .then(setSupport)
            .catch((e) => setError(String(e)));
    }, []);
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
            <CardBody className='gap-4'>
                {support && !support.supported && (
                    <p className='text-sm text-default-500'>{t('screen_record.unsupported')}</p>
                )}
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
                <Switch
                    isSelected={cursor === true}
                    onValueChange={setCursor}
                >
                    {t('config.record.cursor')}
                </Switch>
                {support?.system_audio && (
                    <Switch
                        isSelected={audio === true && microphone !== true}
                        isDisabled={microphone === true}
                        onValueChange={setAudio}
                    >
                        {t('config.record.system_audio')}
                    </Switch>
                )}
                {support?.microphone && (
                    <Switch
                        isSelected={microphone === true}
                        onValueChange={(value) => {
                            setMicrophone(value);
                            if (value) {
                                setAudio(false);
                            }
                        }}
                    >
                        {t('config.record.microphone')}
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
                {error && (
                    <p
                        role='alert'
                        className='text-sm text-danger'
                    >
                        {t(error)}
                    </p>
                )}
            </CardBody>
        </Card>
    );
}
