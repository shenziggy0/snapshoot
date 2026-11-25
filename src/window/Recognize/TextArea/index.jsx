import { Copy, ScanLine, Space, WrapText, Text as PaneIcon } from 'lucide-react';
import {
    Card,
    CardBody,
    CardFooter,
    Button,
    Skeleton,
    ButtonGroup,
    Tooltip,
    Dropdown,
    DropdownItem,
    DropdownMenu,
    DropdownTrigger,
} from '@nextui-org/react';
import { sendNotification } from '@tauri-apps/api/notification';
import { writeText } from '@tauri-apps/api/clipboard';
import { atom, useAtom, useAtomValue, useSetAtom } from 'jotai';
import React, { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { getServiceName, getServiceSouceType, ServiceSourceType } from '../../../utils/service_instance';
import { currentServiceInstanceKeyAtom, languageAtom, recognizeFlagAtom } from '../ControlArea';
import { invoke_plugin } from '../../../utils/invoke_plugin';
import * as builtinServices from '../../../services/recognize';
import { recognizeLayout } from '../../../services/recognize/system';
import { useConfig } from '../../../hooks';
import { osType } from '../../../utils/env';
import { base64Atom, focusedLineAtom } from '../ImageArea';
import { pluginListAtom } from '..';

export const textAtom = atom();

export default function TextArea(props) {
    const { serviceInstanceConfigMap } = props;
    const [autoCopy] = useConfig('recognize_auto_copy', false);
    const [deleteNewline] = useConfig('recognize_delete_newline', false);
    const [hideWindow] = useConfig('recognize_hide_window', false);
    const recognizeFlag = useAtomValue(recognizeFlagAtom);
    const currentServiceInstanceKey = useAtomValue(currentServiceInstanceKeyAtom);
    const language = useAtomValue(languageAtom);
    const base64 = useAtomValue(base64Atom);
    const [loading, setLoading] = useState(false);
    const [text, setText] = useAtom(textAtom);
    const [error, setError] = useState('');
    const [layout, setLayout] = useState(null);
    const setFocusedLine = useSetAtom(focusedLineAtom);
    const pluginList = useAtomValue(pluginListAtom);
    const { t } = useTranslation();

    useEffect(() => {
        setText('');
        setError('');
        setLayout(null);
        setFocusedLine(null);
        if (
            !base64 ||
            !currentServiceInstanceKey ||
            !language ||
            [autoCopy, deleteNewline, hideWindow].includes(null)
        ) {
            setLoading(false);
            return;
        }

        let current = true;
        setLoading(true);
        const read = async () => {
            const name = getServiceName(currentServiceInstanceKey);
            const config = serviceInstanceConfigMap[currentServiceInstanceKey] ?? {};
            const isPlugin = getServiceSouceType(currentServiceInstanceKey) === ServiceSourceType.PLUGIN;
            const languageMap = isPlugin ? pluginList[name]?.language : builtinServices[name]?.Language;
            if (!languageMap || !Object.prototype.hasOwnProperty.call(languageMap, language)) {
                throw new Error('recognize.language_unsupported');
            }
            const selectedLanguage = languageMap[language];
            if (isPlugin) {
                const [recognize, utils] = await invoke_plugin('recognize', name);
                return { text: await recognize(base64, selectedLanguage, { config, utils }) };
            }
            if (name === 'system' && osType === 'Darwin') {
                const layout = await recognizeLayout(base64, selectedLanguage);
                return { text: layout.text, layout };
            }
            return { text: await builtinServices[name].recognize(base64, selectedLanguage, { config }) };
        };

        void read()
            .then(({ text: raw, layout }) => {
                if (!current) return;
                const trimmed = raw.trim();
                const value = deleteNewline ? trimmed.replace(/-\s+/g, '').replace(/\s+/g, ' ') : trimmed;
                setText(value);
                setLayout(layout ?? null);
                setLoading(false);
                if (autoCopy && value) {
                    void writeText(value)
                        .then(() => {
                            if (current && hideWindow) {
                                return sendNotification({ title: t('common.write_clipboard'), body: value });
                            }
                        })
                        .catch(console.error);
                }
            })
            .catch((error) => {
                if (!current) return;
                setError(error?.message ?? String(error));
                setLoading(false);
            });
        return () => {
            current = false;
        };
    }, [
        base64,
        currentServiceInstanceKey,
        language,
        recognizeFlag,
        autoCopy,
        deleteNewline,
        hideWindow,
        pluginList,
        serviceInstanceConfigMap,
        setFocusedLine,
        setText,
        t,
    ]);

    return (
        <Card
            shadow='none'
            className='utility-card'
        >
            <h2 className='utility-card-heading'>
                <PaneIcon aria-hidden='true' />
                {t('recognize.result')}
            </h2>
            <CardBody className='bg-content1 p-0 h-full'>
                {loading ? (
                    <div className='space-y-3 m-[12px]'>
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
                    <>
                        {!error && (
                            <textarea
                                aria-label={t('recognize.result')}
                                value={text ?? ''}
                                className='bg-content1 h-full m-[12px] mb-0 resize-none focus:outline-none'
                                onChange={(e) => {
                                    setText(e.target.value);
                                }}
                            />
                        )}
                        {error && (
                            <textarea
                                aria-label={t('recognize.result')}
                                value={t(error)}
                                readOnly
                                className='bg-content1 h-full m-[12px] mb-0 resize-none focus:outline-none text-foreground font-medium'
                            />
                        )}
                    </>
                )}
            </CardBody>
            <CardFooter className='bg-content1 flex justify-start px-[12px]'>
                <ButtonGroup>
                    <Tooltip content={t('recognize.copy_text')}>
                        <Button
                            aria-label={t('recognize.copy_text')}
                            isIconOnly
                            isDisabled={!text}
                            size='sm'
                            variant='light'
                            onPress={() => {
                                writeText(text);
                            }}
                        >
                            <Copy size={16} />
                        </Button>
                    </Tooltip>
                    <Tooltip content={t('recognize.delete_newline')}>
                        <Button
                            aria-label={t('recognize.delete_newline')}
                            isIconOnly
                            isDisabled={!text}
                            variant='light'
                            size='sm'
                            onPress={() => {
                                setText(text.replace(/\-\s+/g, '').replace(/\s+/g, ' '));
                            }}
                        >
                            <WrapText size={16} />
                        </Button>
                    </Tooltip>
                    <Tooltip content={t('recognize.delete_space')}>
                        <Button
                            aria-label={t('recognize.delete_space')}
                            isIconOnly
                            isDisabled={!text}
                            variant='light'
                            size='sm'
                            onPress={() => {
                                setText(text.replaceAll(' ', ''));
                            }}
                        >
                            <Space size={16} />
                        </Button>
                    </Tooltip>
                </ButtonGroup>
                {!!layout?.lines?.length && (
                    <Dropdown>
                        <DropdownTrigger>
                            <Button
                                size='sm'
                                variant='light'
                                startContent={<ScanLine size={16} />}
                            >
                                {t('recognize.locate_lines')}
                            </Button>
                        </DropdownTrigger>
                        <DropdownMenu
                            aria-label={t('recognize.locate_lines')}
                            className='max-h-[50vh] max-w-[min(27rem,80vw)] overflow-auto'
                            onAction={(key) => setFocusedLine({ base64, layout, index: Number(key) })}
                        >
                            {layout.lines.map((line, index) => (
                                <DropdownItem
                                    key={index}
                                    textValue={line.text}
                                >
                                    <span className='whitespace-normal'>{line.text}</span>
                                </DropdownItem>
                            ))}
                        </DropdownMenu>
                    </Dropdown>
                )}
            </CardFooter>
        </Card>
    );
}
