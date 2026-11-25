import { Copy, Image as PaneIcon, Maximize2 } from 'lucide-react';
import { Card, CardBody, CardFooter, Button, Tooltip } from '@nextui-org/react';
import { appWindow } from '@tauri-apps/api/window';
import React, { useEffect, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api';
import { atom, useAtom } from 'jotai';

import { useConfig } from '../../../hooks';

export const base64Atom = atom('');
export const focusedLineAtom = atom(null);

function lineViewBox(line, image) {
    const padding = Math.max(16, line.height);
    const width = Math.min(image.width, line.width + padding * 2);
    const height = Math.min(image.height, line.height + padding * 2);
    const x = Math.max(0, Math.min(line.x - padding, image.width - width));
    const y = Math.max(0, Math.min(line.y - padding, image.height - height));
    return `${x} ${y} ${width} ${height}`;
}

export default function ImageArea() {
    const [hideWindow] = useConfig('recognize_hide_window', false);
    const [base64, setBase64] = useAtom(base64Atom);
    const [focusedLine, setFocusedLine] = useAtom(focusedLineAtom);
    const [error, setError] = useState('');
    const { t } = useTranslation();
    const imageSrc = `data:image/png;base64,${base64}`;
    const line = focusedLine?.base64 === base64 ? focusedLine.layout.lines[focusedLine.index] : null;
    const layout = focusedLine?.layout;

    useEffect(() => {
        if (hideWindow === null) return;
        let active = true;
        let request = 0;
        const loadImage = async () => {
            const current = ++request;
            try {
                const data = await invoke('get_base64');
                if (!active || current !== request) return;
                setBase64(data);
                setError('');
                if (hideWindow) {
                    await appWindow.hide();
                } else {
                    await appWindow.show();
                    await appWindow.setFocus();
                }
            } catch (error) {
                if (active && current === request) setError(String(error));
            }
        };
        const subscription = listen('new_image', loadImage);
        void subscription
            .then(() => {
                if (active) return loadImage();
            })
            .catch((error) => {
                if (active) setError(String(error));
            });
        return () => {
            active = false;
            void subscription.then(
                (dispose) => dispose(),
                () => {}
            );
        };
    }, [hideWindow, setBase64]);

    return (
        <Card
            shadow='none'
            className='utility-card'
        >
            <h2 className='utility-card-heading'>
                <PaneIcon aria-hidden='true' />
                {t('recognize.original_image')}
            </h2>
            <CardBody className='ocr-image-well min-h-0 p-0'>
                {line ? (
                    <svg
                        role='img'
                        aria-label={t('recognize.original_image')}
                        className='h-full w-full'
                        viewBox={lineViewBox(line, layout)}
                    >
                        <image
                            href={imageSrc}
                            width={layout.width}
                            height={layout.height}
                        />
                        <rect
                            x={line.x}
                            y={line.y}
                            width={line.width}
                            height={line.height}
                            fill='none'
                            stroke='var(--snap-accent)'
                            strokeWidth='2'
                            vectorEffect='non-scaling-stroke'
                        />
                    </svg>
                ) : (
                    base64 && (
                        <img
                            alt={t('recognize.original_image')}
                            draggable={false}
                            className='object-contain h-full w-full'
                            src={imageSrc}
                        />
                    )
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
                            try {
                                await invoke('copy_png_base64', { data: base64 });
                                setError('');
                            } catch (error) {
                                setError(String(error));
                            }
                        }}
                    >
                        <Copy size={16} />
                    </Button>
                </Tooltip>
                {line && (
                    <Button
                        size='sm'
                        variant='light'
                        startContent={<Maximize2 size={16} />}
                        onPress={() => setFocusedLine(null)}
                    >
                        {t('recognize.reset_image_zoom')}
                    </Button>
                )}
                {error && (
                    <span
                        role='alert'
                        className='text-danger text-small'
                    >
                        {error}
                    </span>
                )}
            </CardFooter>
        </Card>
    );
}
