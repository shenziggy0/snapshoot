import { Code, Card, CardBody, Button, Skeleton } from '@nextui-org/react';
import React, { useEffect, useState } from 'react';
import { appWindow } from '@tauri-apps/api/window';
import { invoke } from '@tauri-apps/api/tauri';
import { open } from '@tauri-apps/api/shell';
import { useTranslation } from 'react-i18next';
import ReactMarkdown from 'react-markdown';

import { useConfig } from '../../hooks';
import { osType } from '../../utils/env';

const RELEASES_PAGE = 'https://github.com/shenziggy0/snapshoot/releases';

// Snapshoot only tells the user about a new release and opens the installer
// download; installers are unsigned, so nothing is installed automatically.
export default function Updater() {
    const [transparent] = useConfig('transparent', true);
    const [release, setRelease] = useState(null);
    const [error, setError] = useState('');
    const { t } = useTranslation();

    useEffect(() => {
        if (appWindow.label === 'updater') {
            appWindow.show();
        }
        invoke('check_latest_release').then(setRelease, (e) => {
            setError(e.toString());
        });
    }, []);

    let body = '';
    if (error !== '') {
        body = `${t('updater.failed')}\n\n${error}`;
    } else if (release !== null) {
        body = release.has_update
            ? `${t('updater.new_version', { version: release.latest })}\n\n${release.notes}`
            : t('updater.latest');
    }
    if (body !== '' && osType === 'Darwin') {
        body += `\n\n${t('updater.macos_install_help')}`;
    }
    let downloadUrl = null;
    if (error !== '') {
        downloadUrl = RELEASES_PAGE;
    } else if (release !== null && release.has_update) {
        downloadUrl = release.download_url ?? release.page_url;
    }

    return (
        <div
            className={`${transparent ? 'bg-background/90' : 'bg-background'} h-screen ${
                osType === 'Linux' && 'rounded-[10px] border-1 border-default-100'
            }`}
        >
            <div className='p-[5px] h-[35px] w-full select-none cursor-default'>
                <div
                    data-tauri-drag-region='true'
                    className={`h-full w-full flex ${osType === 'Darwin' ? 'justify-end' : 'justify-start'}`}
                >
                    <img
                        src='icon.png'
                        className='h-[25px] w-[25px] mr-[10px]'
                        draggable={false}
                    />
                    <h2>{t('updater.title')}</h2>
                </div>
            </div>
            <Card className='mx-[80px] mt-[10px] overscroll-auto h-[calc(100vh-150px)]'>
                <CardBody>
                    {body === '' ? (
                        <div className='space-y-3'>
                            <Skeleton className='w-3/5 rounded-lg'>
                                <div className='h-3 w-3/5 rounded-lg bg-default-200'></div>
                            </Skeleton>
                            <Skeleton className='w-4/5 rounded-lg'>
                                <div className='h-3 w-4/5 rounded-lg bg-default-200'></div>
                            </Skeleton>
                            <Skeleton className='w-2/5 rounded-lg'>
                                <div className='h-3 w-2/5 rounded-lg bg-default-300'></div>
                            </Skeleton>
                        </div>
                    ) : (
                        <ReactMarkdown
                            className='markdown-body select-text'
                            components={{
                                code: ({ node, ...props }) => {
                                    const { children } = props;
                                    return <Code size='sm'>{children}</Code>;
                                },
                                h2: ({ node, ...props }) => (
                                    <b>
                                        <h2
                                            className='text-[24px]'
                                            {...props}
                                        />
                                        <hr />
                                        <br />
                                    </b>
                                ),
                                h3: ({ node, ...props }) => (
                                    <b>
                                        <br />
                                        <h3
                                            className='text-[18px]'
                                            {...props}
                                        />
                                        <br />
                                    </b>
                                ),
                                li: ({ node, ...props }) => {
                                    const { children } = props;
                                    return (
                                        <li
                                            className='list-disc list-inside'
                                            children={children}
                                        />
                                    );
                                },
                            }}
                        >
                            {body}
                        </ReactMarkdown>
                    )}
                </CardBody>
            </Card>
            <div className='grid gap-4 grid-cols-2 h-[50px] my-[10px] mx-[80px]'>
                <Button
                    variant='flat'
                    isDisabled={downloadUrl === null}
                    color='primary'
                    onPress={() => {
                        open(downloadUrl);
                    }}
                >
                    {t('updater.download')}
                </Button>
                <Button
                    variant='flat'
                    color='danger'
                    onPress={() => {
                        appWindow.close();
                    }}
                >
                    {t('updater.cancel')}
                </Button>
            </div>
        </div>
    );
}
