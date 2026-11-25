import { useTranslation } from 'react-i18next';
import { Pin, ScanText } from 'lucide-react';
import { readDir, BaseDirectory, readTextFile, exists } from '@tauri-apps/api/fs';
import { appConfigDir, join } from '@tauri-apps/api/path';
import { convertFileSrc } from '@tauri-apps/api/tauri';
import { appWindow } from '@tauri-apps/api/window';
import React, { useState, useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import { Button } from '@nextui-org/react';
import { atom, useAtom } from 'jotai';

import WindowControl from '../../components/WindowControl';
import { store } from '../../utils/store';
import { osType } from '../../utils/env';
import { useConfig } from '../../hooks';
import ControlArea from './ControlArea';
import ImageArea from './ImageArea';
import TextArea from './TextArea';

export const pluginListAtom = atom();

export default function Recognize() {
    const { t } = useTranslation();
    const [pluginList, setPluginList] = useAtom(pluginListAtom);
    const [closeOnBlur] = useConfig('recognize_close_on_blur', false);
    const [pined, setPined] = useState(false);
    const [noteOpen, setNoteOpen] = useState(false);
    const [serviceInstanceList] = useConfig('recognize_service_list', ['system', 'tesseract']);
    const [serviceInstanceConfigMap, setServiceInstanceConfigMap] = useState(null);

    const loadPluginList = async () => {
        let temp = {};
        if (await exists(`plugins/recognize`, { dir: BaseDirectory.AppConfig })) {
            const plugins = await readDir(`plugins/recognize`, { dir: BaseDirectory.AppConfig });
            for (const plugin of plugins) {
                const infoStr = await readTextFile(`plugins/recognize/${plugin.name}/info.json`, {
                    dir: BaseDirectory.AppConfig,
                });
                let pluginInfo = JSON.parse(infoStr);
                if ('icon' in pluginInfo) {
                    const appConfigDirPath = await appConfigDir();
                    const iconPath = await join(
                        appConfigDirPath,
                        `/plugins/recognize/${plugin.name}/${pluginInfo.icon}`
                    );
                    pluginInfo.icon = convertFileSrc(iconPath);
                }
                temp[plugin.name] = pluginInfo;
            }
        }
        setPluginList({ ...temp });
    };
    const loadServiceInstanceConfigMap = async () => {
        const config = {};
        for (const serviceInstanceKey of serviceInstanceList) {
            config[serviceInstanceKey] = (await store.get(serviceInstanceKey)) ?? {};
        }
        setServiceInstanceConfigMap({ ...config });
    };
    useEffect(() => {
        if (serviceInstanceList !== null) {
            loadServiceInstanceConfigMap();
        }
    }, [serviceInstanceList]);

    useEffect(() => {
        loadPluginList();
    }, []);
    useEffect(() => {
        if (!closeOnBlur || pined || noteOpen) return;
        let active = true;
        let timer;
        const cancelClose = () => clearTimeout(timer);
        const subscriptions = [
            listen('tauri://blur', () => {
                if (!active) return;
                cancelClose();
                // Windows may briefly lose focus while dragging the window.
                timer = setTimeout(() => { void appWindow.close(); }, 50);
            }),
            listen('tauri://focus', cancelClose),
        ];
        return () => {
            active = false;
            cancelClose();
            for (const subscription of subscriptions) void subscription.then((dispose) => dispose()).catch(console.error);
        };
    }, [closeOnBlur, pined, noteOpen]);

    return (
        pluginList &&
        serviceInstanceConfigMap !== null && (
            <div className='utility-window'>
                <header className='utility-bar' data-tauri-drag-region='true'>
                    <h1><ScanText aria-hidden='true' />{t('config.hotkey.ocr_recognize')}</h1>
                    <Button
                        isIconOnly
                        size='sm'
                        variant='flat'
                        className='utility-pin'
                        aria-label={t('common.pin_window')}
                        aria-pressed={pined}
                        onPress={async () => {
                            await appWindow.setAlwaysOnTop(!pined);
                            setPined(!pined);
                        }}
                    >
                        <Pin size={16} aria-hidden='true' />
                    </Button>
                    {osType !== 'Darwin' && <WindowControl />}
                </header>
                <div className='ocr-panes'>
                    <ImageArea />
                    <TextArea serviceInstanceConfigMap={serviceInstanceConfigMap} />
                </div>
                <ControlArea
                    serviceInstanceList={serviceInstanceList}
                    serviceInstanceConfigMap={serviceInstanceConfigMap}
                    onNoteOpenChange={setNoteOpen}
                />
            </div>
        )
    );
}
