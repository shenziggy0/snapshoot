import { Copy, Minus, Square, X } from 'lucide-react';
import React, { useEffect, useState } from 'react';
import { appWindow } from '@tauri-apps/api/window';
import { listen } from '@tauri-apps/api/event';
import { Button } from '@nextui-org/react';

import { useTranslation } from 'react-i18next';
import './style.css';

export default function WindowControl({ onClose = () => appWindow.close() }) {
    const { t } = useTranslation();
    const [isMax, setIsMax] = useState(false);

    useEffect(() => {
        const update = () => appWindow.isMaximized().then(setIsMax);
        void update();
        const listener = listen('tauri://resize', update);
        return () => { listener.then((unlisten) => unlisten()); };
    }, []);

    return (
        <div className='snap-window-controls'>
            <Button
                isIconOnly
                variant='light'
                className='window-action'
                aria-label={t('common.minimize')}
                onPress={() => appWindow.minimize()}
            >
                <Minus size={16} />
            </Button>
            <Button
                isIconOnly
                variant='light'
                className='window-action'
                aria-label={t(isMax ? 'common.restore_window' : 'common.maximize')}
                onPress={() => {
                    if (isMax) {
                        appWindow.unmaximize();
                    } else {
                        appWindow.maximize();
                    }
                }}
            >
                {isMax ? <Copy size={16} /> : <Square size={16} />}
            </Button>
            <Button
                isIconOnly
                variant='light'
                className='window-action close-button'
                aria-label={t('annotate.close')}
                onPress={onClose}
            >
                <X size={16} />
            </Button>
        </div>
    );
}
