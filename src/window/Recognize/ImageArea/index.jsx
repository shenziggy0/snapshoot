import { Copy, Image as PaneIcon } from 'lucide-react';
import { Card, CardBody, CardFooter, Button, Tooltip } from '@nextui-org/react';
import { appWindow } from '@tauri-apps/api/window';
import React, { useEffect, useRef } from 'react';
import { listen } from '@tauri-apps/api/event';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api';
import { atom, useAtom } from 'jotai';

import { useConfig } from '../../../hooks';

export const base64Atom = atom('');
let unlisten = null;

export default function ImageArea() {
    const [hideWindow] = useConfig('recognize_hide_window', false);
    const [base64, setBase64] = useAtom(base64Atom);
    const imgRef = useRef();
    const { t } = useTranslation();
    const load_img = () => {
        invoke('get_base64').then((v) => {
            setBase64(v);
            if (hideWindow) {
                appWindow.hide();
            } else {
                appWindow.show();
                appWindow.setFocus(true);
            }
        });
    };

    useEffect(() => {
        if (hideWindow !== null) {
            load_img();
            if (unlisten) {
                unlisten.then((f) => {
                    f();
                });
            }
            unlisten = listen('new_image', (_) => {
                load_img();
            });
        }
    }, [hideWindow]);

    return (
        <Card
            shadow='none'
            className='utility-card'
        >
            <h2 className='utility-card-heading'><PaneIcon aria-hidden='true' />{t('recognize.original_image')}</h2>
            <CardBody className='ocr-image-well min-h-0 p-0'>
                {base64 !== '' && (
                    <img
                        alt={t('recognize.original_image')}
                        ref={imgRef}
                        draggable={false}
                        className='object-contain h-full w-full'
                        src={'data:image/png;base64,' + base64}
                    />
                )}
            </CardBody>
            <CardFooter className='bg-content1 flex justify-start px-[12px]'>
                <Tooltip content={t('recognize.copy_img')}>
                    <Button
                        aria-label={t('recognize.copy_img')}
                        isDisabled={!base64}
                        isIconOnly
                        size='sm'
                        variant='light'
                        onPress={async () => {
                            await invoke('copy_img', {
                                width: imgRef.current.naturalWidth,
                                height: imgRef.current.naturalHeight,
                            });
                        }}
                    >
                        <Copy size={16} />
                    </Button>
                </Tooltip>
            </CardFooter>
        </Card>
    );
}
