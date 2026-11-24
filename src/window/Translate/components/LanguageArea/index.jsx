import { ArrowRightLeft } from 'lucide-react';
import { Card, Button, CardFooter, Dropdown, DropdownMenu, DropdownTrigger, DropdownItem } from '@nextui-org/react';
import { useTranslation } from 'react-i18next';
import React, { useEffect, useRef } from 'react';
import { atom, useAtom, useAtomValue } from 'jotai';

import { languageList } from '../../../../utils/language';
import { detectLanguageAtom } from '../SourceArea';
import { useConfig } from '../../../../hooks';

export const sourceLanguageAtom = atom();
export const targetLanguageAtom = atom();

export default function LanguageArea() {
    const [rememberLanguage] = useConfig('translate_remember_language', false);
    const [translateSourceLanguage, setTranslateSourceLanguage] = useConfig('translate_source_language', 'auto');
    const [translateTargetLanguage, setTranslateTargetLanguage] = useConfig('translate_target_language', 'zh_cn');
    const [translateSecondLanguage] = useConfig('translate_second_language', 'en');

    const [sourceLanguage, setSourceLanguage] = useAtom(sourceLanguageAtom);
    const [targetLanguage, setTargetLanguage] = useAtom(targetLanguageAtom);
    const detectLanguage = useAtomValue(detectLanguageAtom);
    const { t } = useTranslation();
    const previousTarget = useRef(null);

    useEffect(() => {
        if (translateSourceLanguage) {
            setSourceLanguage(translateSourceLanguage);
        }
        if (translateTargetLanguage) {
            setTargetLanguage(translateTargetLanguage);
        }
    }, [translateSourceLanguage, translateTargetLanguage]);

    useEffect(() => {
        if (rememberLanguage && sourceLanguage && targetLanguage) {
            setTranslateSourceLanguage(sourceLanguage);
            setTranslateTargetLanguage(targetLanguage);
        }
    }, [sourceLanguage, targetLanguage, rememberLanguage]);

    return (
        <Card
            shadow='none'
            className='translation-languages snap-glass'
        >
            <CardFooter className='flex items-center justify-between'>
                <div className='flex'>
                    <Dropdown>
                        <DropdownTrigger>
                            <Button
                                radius='sm'
                                variant='light'
                            >
                                {t(`languages.${sourceLanguage}`)}
                            </Button>
                        </DropdownTrigger>
                        <DropdownMenu
                            aria-label='Source Language'
                            className='max-h-[50vh] overflow-y-auto'
                            onAction={(key) => {
                                setSourceLanguage(key);
                            }}
                        >
                            <DropdownItem key='auto'>{t('languages.auto')}</DropdownItem>
                            {languageList.map((x) => {
                                return <DropdownItem key={x}>{t(`languages.${x}`)}</DropdownItem>;
                            })}
                        </DropdownMenu>
                    </Dropdown>
                </div>
                <div className='flex'>
                    <Button
                        aria-label={t('common.switch_languages')}
                        isIconOnly
                        size='sm'
                        variant='light'
                        className='text-[20px]'
                        isDisabled={!sourceLanguage || !targetLanguage || !translateTargetLanguage || !translateSecondLanguage}
                        onPress={() => {
                            if (sourceLanguage !== 'auto') {
                                setSourceLanguage(targetLanguage);
                                setTargetLanguage(sourceLanguage);
                            } else if (detectLanguage) {
                                setTargetLanguage(targetLanguage === translateTargetLanguage ? detectLanguage : translateTargetLanguage);
                            } else if (targetLanguage !== translateSecondLanguage) {
                                previousTarget.current = targetLanguage;
                                setTargetLanguage(translateSecondLanguage);
                            } else {
                                setTargetLanguage(previousTarget.current ?? translateTargetLanguage);
                            }
                        }}
                    >
                        <ArrowRightLeft size="1em" />
                    </Button>
                </div>
                <div className='flex'>
                    <Dropdown>
                        <DropdownTrigger>
                            <Button
                                radius='sm'
                                variant='light'
                            >
                                {t(`languages.${targetLanguage}`)}
                            </Button>
                        </DropdownTrigger>
                        <DropdownMenu
                            aria-label='Target Language'
                            className='max-h-[50vh] overflow-y-auto'
                            onAction={(key) => {
                                setTargetLanguage(key);
                            }}
                        >
                            {languageList.map((x) => {
                                return <DropdownItem key={x}>{t(`languages.${x}`)}</DropdownItem>;
                            })}
                        </DropdownMenu>
                    </Dropdown>
                </div>
            </CardFooter>
        </Card>
    );
}
