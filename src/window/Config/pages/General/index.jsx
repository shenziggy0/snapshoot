import { ChevronRight, Eye, EyeOff, History, Keyboard, MousePointer2, Scan, Video } from 'lucide-react';
import AppearancePicker from './AppearancePicker';
import { Link } from 'react-router-dom';
import { enable, isEnabled, disable } from 'tauri-plugin-autostart-api';
import { DropdownTrigger } from '@nextui-org/react';
import React, { useState, useEffect, useRef } from 'react';
import toast from 'react-hot-toast';
import Toaster from '../../../../components/Toaster';
import { DropdownMenu } from '@nextui-org/react';
import { DropdownItem } from '@nextui-org/react';
import { useTranslation } from 'react-i18next';
import { CardBody } from '@nextui-org/react';
import { Dropdown } from '@nextui-org/react';
import { info } from 'tauri-plugin-log-api';
import { Button } from '@nextui-org/react';
import { Switch } from '@nextui-org/react';
import 'flag-icons/css/flag-icons.min.css';
import { Input } from '@nextui-org/react';
import { Card } from '@nextui-org/react';
import { invoke } from '@tauri-apps/api';
import { saveAppTheme } from '../../../../utils/store';

import { useConfig } from '../../../../hooks/useConfig';
import { LanguageFlag } from '../../../../utils/language';
import { useToastStyle } from '../../../../hooks';
import { osType } from '../../../../utils/env';

const MAC_KEYS = { Command: '⌘', Control: '⌃', Ctrl: '⌃', Shift: '⇧', Alt: '⌥' };

let timer = null;

export default function General() {
    const [autoStart, setAutoStart] = useState(false);
    const [fontList, setFontList] = useState(null);
    const [checkUpdate, setCheckUpdate] = useConfig('check_update', true);
    const [quickAccess, setQuickAccess] = useConfig('quick_access_enabled', true);
    const [serverPort, setServerPort] = useConfig('server_port', 60828);
    const [captureShortcut] = useConfig('hotkey_screenshot_annotate', osType === 'Darwin' ? 'Command+Shift+A' : 'F1');
    const [captureError, setCaptureError] = useState('');
    const [startingCapture, setStartingCapture] = useState(false);
    const [showToken, setShowToken] = useState(false);
    const [serverToken] = useConfig('server_token', '');
    const [appLanguage, setAppLanguage] = useConfig('app_language', 'en');
    const [appTheme] = useConfig('app_theme', 'system');
    const [pendingTheme, setPendingTheme] = useState(null);
    const themeRequest = useRef(null);
    const [appFont, setAppFont] = useConfig('app_font', 'default');
    const [appFallbackFont, setAppFallbackFont] = useConfig('app_fallback_font', 'default');
    const [appFontSize, setAppFontSize] = useConfig('app_font_size', 16);
    const [transparent, setTransparent] = useConfig('transparent', true);
    const [devMode, setDevMode] = useConfig('dev_mode', false);
    const [trayClickEvent, setTrayClickEvent] = useConfig('tray_click_event', 'config');
    const [proxyEnable, setProxyEnable] = useConfig('proxy_enable', false);
    const [proxyHost, setProxyHost] = useConfig('proxy_host', '');
    const [proxyPort, setProxyPort] = useConfig('proxy_port', '');
    const [proxyUsername, setProxyUsername] = useConfig('proxy_username', '');
    const [proxyPassword, setProxyPassword] = useConfig('proxy_password', '');
    const [noProxy, setNoProxy] = useConfig('no_proxy', 'localhost,127.0.0.1');
    const { t, i18n } = useTranslation();
    const toastStyle = useToastStyle();

    const languageName = {
        zh_cn: '简体中文',
        zh_tw: '繁體中文',
        en: 'English',
        ja: '日本語',
        ko: '한국어',
        fr: 'Français',
        es: 'Español',
        ru: 'Русский',
        de: 'Deutsch',
        it: 'Italiano',
        tr: 'Türkçe',
        pt_pt: 'Português',
        pt_br: 'Português (Brasil)',
        nb_no: 'Norsk Bokmål',
        nn_no: 'Norsk Nynorsk',
        fa: 'فارسی',
        uk: 'Українська',
        ar: 'العربية',
        he: 'עִבְרִית',
    };

    useEffect(() => {
        isEnabled().then((v) => {
            setAutoStart(v);
        });
        invoke('font_list').then((v) => {
            setFontList(v);
        });
    }, []);

    return (
        <>
            <Toaster />
            <section className='capture-launcher' aria-labelledby='capture-title'>
                <div className='launcher-body'>
                    <div className='launcher-command'>
                        <h2 id='capture-title'>{t('config.hotkey.screenshot_annotate')}</h2>
                        <Link to='/hotkey' className='launcher-shortcut' title={t('capture.change_shortcut')} aria-label={t('capture.change_shortcut')}>
                            <Keyboard size={15} />
                            <span className='settings-keycaps'>
                                {captureShortcut
                                    ? captureShortcut.split('+').map((key, index) => (
                                        <kbd key={index}>{osType === 'Darwin' ? MAC_KEYS[key] || key : key}</kbd>
                                    ))
                                    : t('config.shell.unassigned')}
                            </span>
                            <ChevronRight size={12} />
                        </Link>
                        <button type='button' className='launcher-start' disabled={startingCapture} onClick={async () => {
                            setStartingCapture(true);
                            setCaptureError('');
                            try {
                                await invoke('start_capture', { mode: 'annotate' });
                            } catch (error) {
                                setCaptureError(String(error));
                            } finally {
                                setStartingCapture(false);
                            }
                        }}>
                            <Scan size={16} />{t('capture.start')}
                        </button>
                    </div>
                    <div className='launcher-preview' aria-hidden='true'>
                        <span className='launcher-selection'><i /><i /><i /><i /><MousePointer2 /></span>
                    </div>
                </div>
                <div className='launcher-links'>
                    <Link to='/record'><Video />{t('config.record.label')}<ChevronRight /></Link>
                    <Link to='/recent'><History />{t('config.recent.label')}<ChevronRight /></Link>
                </div>
            </section>
            {captureError && (
                <div role='alert' className='mt-2 flex flex-wrap items-center gap-2 text-sm text-danger'>
                    {t(captureError)}
                    {captureError === 'screen_record.permission_denied' && osType === 'Darwin' && (
                        <Button size='sm' variant='flat' onPress={() => invoke('open_capture_privacy_settings', { source: 'screen' })}>
                            {t('capture.open_settings')}
                        </Button>
                    )}
                </div>
            )}
            {appTheme !== null && (
                <AppearancePicker
                    value={pendingTheme ?? appTheme}
                    onChange={(mode) => {
                        setPendingTheme(mode);
                        const request = saveAppTheme(mode);
                        themeRequest.current = request;
                        void request.catch(() => {
                            if (themeRequest.current === request) {
                                toast.error(t('config.general.theme_save_failed'), { style: toastStyle });
                            }
                        }).finally(() => {
                            if (themeRequest.current === request) setPendingTheme(null);
                        });
                    }}
                />
            )}
            <h2 className='settings-section-title'>{t('config.shell.startup')}</h2>
            <Card className='mb-[10px]'>
                <CardBody>
                    <div className='config-item'>
                        <h3>{t('config.general.auto_start')}</h3>
                        <Switch
                            aria-label={t('config.general.auto_start')}
                            isSelected={autoStart}
                            onValueChange={(v) => {
                                setAutoStart(v);
                                if (v) {
                                    enable().then(() => {
                                        info('Auto start enabled');
                                    });
                                } else {
                                    disable().then(() => {
                                        info('Auto start disabled');
                                    });
                                }
                            }}
                        />
                    </div>
                    <div className='config-item'>
                        <h3>{t('config.general.check_update')}</h3>
                        {checkUpdate !== null && (
                            <Switch
                                aria-label={t('config.general.check_update')}
                                isSelected={checkUpdate}
                                onValueChange={(v) => {
                                    setCheckUpdate(v);
                                }}
                            />
                        )}
                    </div>
                    <div className='config-item'>
                        <h3>{t('config.general.quick_access')}</h3>
                        {quickAccess !== null && (
                            <Switch
                                aria-label={t('config.general.quick_access')}
                                isSelected={quickAccess}
                                onValueChange={(v) => {
                                    setQuickAccess(v);
                                    if (!v) {
                                        invoke('hide_quick_access_window');
                                    }
                                }}
                            />
                        )}
                    </div>
                </CardBody>
            </Card>
            <h2 className='settings-section-title'>{t('config.shell.appearance')}</h2>
            <Card className='mb-[10px]'>
                <CardBody>
                    <div className='config-item'>
                        <h3 className='my-auto'>{t('config.general.app_language')}</h3>
                        {appLanguage !== null && (
                            <Dropdown>
                                <DropdownTrigger>
                                    <Button
                                        variant='bordered'
                                        startContent={<span className={`fi fi-${LanguageFlag[appLanguage]}`} />}
                                    >
                                        {languageName[appLanguage]}
                                    </Button>
                                </DropdownTrigger>
                                <DropdownMenu
                                    aria-label='app language'
                                    className='max-h-[40vh] overflow-y-auto'
                                    onAction={(key) => {
                                        setAppLanguage(key);
                                        i18n.changeLanguage(key);
                                        invoke('update_tray', { language: key, copyMode: '' });
                                    }}
                                >
                                    <DropdownItem
                                        key='zh_cn'
                                        startContent={<span className={`fi fi-${LanguageFlag.zh_cn}`} />}
                                    >
                                        简体中文
                                    </DropdownItem>
                                    <DropdownItem
                                        key='zh_tw'
                                        startContent={<span className={`fi fi-${LanguageFlag.zh_cn}`} />}
                                    >
                                        繁體中文
                                    </DropdownItem>
                                    <DropdownItem
                                        key='en'
                                        startContent={<span className={`fi fi-${LanguageFlag.en}`} />}
                                    >
                                        English
                                    </DropdownItem>
                                    <DropdownItem
                                        key='ja'
                                        startContent={<span className={`fi fi-${LanguageFlag.ja}`} />}
                                    >
                                        日本語
                                    </DropdownItem>
                                    <DropdownItem
                                        key='ko'
                                        startContent={<span className={`fi fi-${LanguageFlag.ko}`} />}
                                    >
                                        한국어
                                    </DropdownItem>
                                    <DropdownItem
                                        key='fr'
                                        startContent={<span className={`fi fi-${LanguageFlag.fr}`} />}
                                    >
                                        Français
                                    </DropdownItem>
                                    <DropdownItem
                                        key='de'
                                        startContent={<span className={`fi fi-${LanguageFlag.de}`} />}
                                    >
                                        Deutsch
                                    </DropdownItem>
                                    <DropdownItem
                                        key='es'
                                        startContent={<span className={`fi fi-${LanguageFlag.es}`} />}
                                    >
                                        Español
                                    </DropdownItem>
                                    <DropdownItem
                                        key='ru'
                                        startContent={<span className={`fi fi-${LanguageFlag.ru}`} />}
                                    >
                                        Русский
                                    </DropdownItem>
                                    <DropdownItem
                                        key='it'
                                        startContent={<span className={`fi fi-${LanguageFlag.it}`} />}
                                    >
                                        Italiano
                                    </DropdownItem>
                                    <DropdownItem
                                        key='tr'
                                        startContent={<span className={`fi fi-${LanguageFlag.tr}`} />}
                                    >
                                        Türkçe
                                    </DropdownItem>
                                    <DropdownItem
                                        key='pt_pt'
                                        startContent={<span className={`fi fi-${LanguageFlag.pt_pt}`} />}
                                    >
                                        Português
                                    </DropdownItem>
                                    <DropdownItem
                                        key='pt_br'
                                        startContent={<span className={`fi fi-${LanguageFlag.pt_br}`} />}
                                    >
                                        Português (Brasil)
                                    </DropdownItem>
                                    <DropdownItem
                                        key='nb_no'
                                        startContent={<span className={`fi fi-${LanguageFlag.nb_no}`} />}
                                    >
                                        Norsk Bokmål
                                    </DropdownItem>
                                    <DropdownItem
                                        key='nn_no'
                                        startContent={<span className={`fi fi-${LanguageFlag.nn_no}`} />}
                                    >
                                        Norsk Nynorsk
                                    </DropdownItem>
                                    <DropdownItem
                                        key='fa'
                                        startContent={<span className={`fi fi-${LanguageFlag.fa}`} />}
                                    >
                                        فارسی
                                    </DropdownItem>
                                    <DropdownItem
                                        key='uk'
                                        startContent={<span className={`fi fi-${LanguageFlag.uk}`} />}
                                    >
                                        Українська
                                    </DropdownItem>
                                    <DropdownItem
                                        key='ar'
                                        startContent={<span className={`fi fi-${LanguageFlag.ar}`} />}
                                    >
                                        العربية
                                    </DropdownItem>
                                    <DropdownItem
                                        key='he'
                                        startContent={<span className={`fi fi-${LanguageFlag.he}`} />}
                                    >
                                        עִבְרִית
                                    </DropdownItem>
                                </DropdownMenu>
                            </Dropdown>
                        )}
                    </div>
                    <div className='config-item'>
                        <h3 className='my-auto'>{t('config.general.app_font')}</h3>
                        {appFont !== null && fontList !== null && (
                            <Dropdown>
                                <DropdownTrigger>
                                    <Button
                                        variant='bordered'
                                        style={{
                                            fontFamily: appFont === 'default' ? 'sans-serif' : appFont,
                                        }}
                                    >
                                        {appFont === 'default' ? t('config.general.default_font') : appFont}
                                    </Button>
                                </DropdownTrigger>
                                <DropdownMenu
                                    aria-label='app font'
                                    className='max-h-[50vh] overflow-y-auto'
                                    onAction={(key) => {
                                        document.documentElement.style.fontFamily = `"${
                                            key === 'default' ? 'sans-serif' : key
                                        }","${appFallbackFont === 'default' ? 'sans-serif' : appFallbackFont}"`;
                                        setAppFont(key);
                                    }}
                                >
                                    <DropdownItem
                                        style={{ fontFamily: 'sans-serif' }}
                                        key='default'
                                    >
                                        {t('config.general.default_font')}
                                    </DropdownItem>
                                    {fontList.map((x) => {
                                        return (
                                            <DropdownItem
                                                style={{ fontFamily: x }}
                                                key={x}
                                            >
                                                {x}
                                            </DropdownItem>
                                        );
                                    })}
                                </DropdownMenu>
                            </Dropdown>
                        )}
                    </div>
                    <div className='config-item'>
                        <h3 className='my-auto'>{t('config.general.app_fallback_font')}</h3>
                        {appFallbackFont !== null && fontList !== null && (
                            <Dropdown>
                                <DropdownTrigger>
                                    <Button
                                        variant='bordered'
                                        style={{
                                            fontFamily: appFallbackFont === 'default' ? 'sans-serif' : appFallbackFont,
                                        }}
                                    >
                                        {appFallbackFont === 'default'
                                            ? t('config.general.default_font')
                                            : appFallbackFont}
                                    </Button>
                                </DropdownTrigger>
                                <DropdownMenu
                                    aria-label='app font'
                                    className='max-h-[50vh] overflow-y-auto'
                                    onAction={(key) => {
                                        document.documentElement.style.fontFamily = `"${
                                            appFont === 'default' ? 'sans-serif' : appFont
                                        }","${key === 'default' ? 'sans-serif' : key}"`;
                                        setAppFallbackFont(key);
                                    }}
                                >
                                    <DropdownItem
                                        style={{ fontFamily: 'sans-serif' }}
                                        key='default'
                                    >
                                        {t('config.general.default_font')}
                                    </DropdownItem>
                                    {fontList.map((x) => {
                                        return (
                                            <DropdownItem
                                                style={{ fontFamily: x }}
                                                key={x}
                                            >
                                                {x}
                                            </DropdownItem>
                                        );
                                    })}
                                </DropdownMenu>
                            </Dropdown>
                        )}
                    </div>
                    <div className='config-item'>
                        <h3 className='my-auto mx-0'>{t('config.general.font_size.title')}</h3>
                        {appFontSize !== null && (
                            <Dropdown>
                                <DropdownTrigger>
                                    <Button variant='bordered'>{t(`config.general.font_size.${appFontSize}`)}</Button>
                                </DropdownTrigger>
                                <DropdownMenu
                                    aria-label='window position'
                                    className='max-h-[50vh] overflow-y-auto'
                                    onAction={(key) => {
                                        document.documentElement.style.fontSize = `${key}px`;
                                        setAppFontSize(key);
                                    }}
                                >
                                    <DropdownItem key={10}>{t(`config.general.font_size.10`)}</DropdownItem>
                                    <DropdownItem key={12}>{t(`config.general.font_size.12`)}</DropdownItem>
                                    <DropdownItem key={14}>{t(`config.general.font_size.14`)}</DropdownItem>
                                    <DropdownItem key={16}>{t(`config.general.font_size.16`)}</DropdownItem>
                                    <DropdownItem key={18}>{t(`config.general.font_size.18`)}</DropdownItem>
                                    <DropdownItem key={20}>{t(`config.general.font_size.20`)}</DropdownItem>
                                    <DropdownItem key={24}>{t(`config.general.font_size.24`)}</DropdownItem>
                                </DropdownMenu>
                            </Dropdown>
                        )}
                    </div>
                    <div className={`config-item ${osType !== 'Windows_NT' && 'hidden'}`}>
                        <h3 className='my-auto'>{t('config.general.tray_click_event')}</h3>
                        {trayClickEvent !== null && (
                            <Dropdown>
                                <DropdownTrigger>
                                    <Button variant='bordered'>{t(`config.general.event.${trayClickEvent}`)}</Button>
                                </DropdownTrigger>
                                <DropdownMenu
                                    aria-label='tray click event'
                                    onAction={(key) => {
                                        setTrayClickEvent(key);
                                    }}
                                >
                                    <DropdownItem key='config'>{t('config.general.event.config')}</DropdownItem>
                                    <DropdownItem key='translate'>{t('config.general.event.translate')}</DropdownItem>
                                    <DropdownItem key='ocr_recognize'>
                                        {t('config.general.event.ocr_recognize')}
                                    </DropdownItem>
                                    <DropdownItem key='ocr_translate'>
                                        {t('config.general.event.ocr_translate')}
                                    </DropdownItem>
                                    <DropdownItem key='disable'>{t('config.general.event.disable')}</DropdownItem>
                                </DropdownMenu>
                            </Dropdown>
                        )}
                    </div>
                    <div className={`config-item ${osType === 'Darwin' && 'hidden'}`}>
                        <h3>{t('config.general.transparent')}</h3>
                        {transparent !== null && (
                            <Switch
                                aria-label={t('config.general.transparent')}
                                isSelected={transparent}
                                onValueChange={(v) => {
                                    setTransparent(v);
                                }}
                            />
                        )}
                    </div>
                    <div className='config-item'>
                        <h3>{t('config.general.dev_mode')}</h3>
                        {devMode !== null && (
                            <Switch
                                aria-label={t('config.general.dev_mode')}
                                isSelected={devMode}
                                onValueChange={(v) => {
                                    setDevMode(v);
                                }}
                            />
                        )}
                    </div>
                </CardBody>
            </Card>
            <h2 className='settings-section-title'>{t('config.shell.connection')}</h2>
            <Card className='mb-[10px]'>
                <CardBody>
                    <div className='config-item'>
                        <div>
                            <h3 className='my-auto'>{t('config.general.server_token')}</h3>
                            <p className='max-w-[420px] text-sm text-default-500'>
                                {t('config.general.server_token_hint')}
                            </p>
                        </div>
                        {serverToken !== null && (
                            <Input
                                aria-label={t('config.general.server_token')}
                                type={showToken ? 'text' : 'password'}
                                endContent={
                                    <button
                                        type='button'
                                        aria-label={t(
                                            showToken ? 'config.shell.hide_token' : 'config.shell.show_token'
                                        )}
                                        onClick={() => setShowToken(!showToken)}
                                    >
                                        {showToken ? <EyeOff size={16} /> : <Eye size={16} />}
                                    </button>
                                }
                                isReadOnly
                                variant='bordered'
                                value={serverToken || ''}
                                className='max-w-[360px]'
                            />
                        )}
                    </div>
                    <div className='config-item'>
                        <h3 className='my-auto'>{t('config.general.server_port')}</h3>
                        {serverPort !== null && (
                            <Input
                                type='number'
                                variant='bordered'
                                value={serverPort}
                                labelPlacement='outside-left'
                                onValueChange={(v) => {
                                    if (parseInt(v) !== serverPort) {
                                        if (timer) {
                                            clearTimeout(timer);
                                        }
                                        timer = setTimeout(() => {
                                            toast.success(t('config.general.server_port_change'), {
                                                duration: 3000,
                                                style: toastStyle,
                                            });
                                        }, 1000);
                                    }
                                    if (v === '') {
                                        setServerPort(0);
                                    } else if (parseInt(v) > 65535) {
                                        setServerPort(65535);
                                    } else if (parseInt(v) < 0) {
                                        setServerPort(0);
                                    } else {
                                        setServerPort(parseInt(v));
                                    }
                                }}
                                className='max-w-[100px]'
                            />
                        )}
                    </div>
                </CardBody>
            </Card>
            <Card>
                <CardBody>
                    <div className='config-item'>
                        <h3>{t('config.general.proxy.title')}</h3>
                        {proxyEnable !== null && (
                            <Switch
                                aria-label={t('config.general.proxy.title')}
                                isSelected={proxyEnable}
                                onValueChange={async (v) => {
                                    if (v) {
                                        if (proxyHost === '' || proxyPort === '') {
                                            setProxyEnable(false);
                                            toast.error(t('config.general.proxy_error'), {
                                                duration: 3000,
                                                style: toastStyle,
                                            });
                                            return;
                                        } else {
                                            setProxyEnable(v);
                                        }
                                    } else {
                                        setProxyEnable(v);
                                    }
                                    toast.success(t('config.general.proxy_change'), {
                                        duration: 1000,
                                        style: toastStyle,
                                    });
                                }}
                            />
                        )}
                    </div>
                    <div className='config-item'>
                        {proxyHost !== null && (
                            <Input
                                type='url'
                                variant='bordered'
                                isRequired
                                label={t('config.general.proxy.host')}
                                startContent={<span>http://</span>}
                                value={proxyHost}
                                onValueChange={(v) => {
                                    setProxyHost(v);
                                }}
                                className='mr-2'
                            />
                        )}
                        {proxyPort !== null && (
                            <Input
                                type='number'
                                variant='bordered'
                                isRequired
                                label={t('config.general.proxy.port')}
                                value={proxyPort}
                                onValueChange={(v) => {
                                    if (parseInt(v) > 65535) {
                                        setProxyPort(65535);
                                    } else if (parseInt(v) < 0) {
                                        setProxyPort('');
                                    } else {
                                        setProxyPort(parseInt(v));
                                    }
                                }}
                                className='ml-2'
                            />
                        )}
                    </div>
                    <div className='config-item'>
                        {proxyUsername !== null && (
                            <Input
                                type='text'
                                variant='bordered'
                                isDisabled
                                label={t('config.general.proxy.username')}
                                value={proxyUsername}
                                onValueChange={(v) => {
                                    setProxyUsername(v);
                                }}
                                className='mr-2'
                            />
                        )}
                        {proxyPassword !== null && (
                            <Input
                                type='password'
                                variant='bordered'
                                isDisabled
                                label={t('config.general.proxy.password')}
                                value={proxyPassword}
                                onValueChange={(v) => {
                                    setProxyPassword(v);
                                }}
                                className='ml-2'
                            />
                        )}
                    </div>
                    <div className='config-item'>
                        {noProxy !== null && (
                            <Input
                                variant='bordered'
                                label={t('config.general.proxy.no_proxy')}
                                value={noProxy}
                                onValueChange={(v) => {
                                    setNoProxy(v);
                                }}
                            />
                        )}
                    </div>
                </CardBody>
            </Card>
        </>
    );
}
