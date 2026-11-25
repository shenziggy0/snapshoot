import React from 'react';
import { NavLink, useLocation, useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import * as Select from '@radix-ui/react-select';
import { AudioLines, Check, ChevronDown, Cloud, History, Info, Keyboard, Languages, PanelLeft, ScanText, Settings2, Video } from 'lucide-react';

const sections = [
    ['workspace', [
        ['general', Settings2], ['record', Video], ['recognize', ScanText],
        ['translate', Languages], ['recent', History],
    ]],
    ['application', [
        ['hotkey', Keyboard], ['service', AudioLines], ['backup', Cloud], ['about', Info],
    ]],
];

export function CompactNavigation() {
    const { t } = useTranslation();
    const { pathname } = useLocation();
    const navigate = useNavigate();
    const pageName = pathname === '/history' ? 'recent' : pathname.slice(1) || 'general';
    return (
        <Select.Root value={pageName} onValueChange={(page) => navigate(`/${page}`)}>
            <Select.Trigger className='settings-compact-nav' aria-label={t('config.shell.settings')}>
                <PanelLeft size={16} /><Select.Value /><ChevronDown size={13} />
            </Select.Trigger>
            <Select.Portal>
                <Select.Content className='settings-nav-menu snap-glass' position='popper' align='start' sideOffset={5}>
                    <Select.Viewport>
                        {sections.map(([section, pages]) => (
                            <Select.Group key={section}>
                                <Select.Label>{t(`config.shell.${section}`)}</Select.Label>
                                {pages.map(([page, Icon]) => (
                                    <Select.Item key={page} value={page}>
                                        <Icon size={15} /><Select.ItemText>{t(`config.${page}.label`)}</Select.ItemText>
                                        <Select.ItemIndicator><Check size={14} /></Select.ItemIndicator>
                                    </Select.Item>
                                ))}
                            </Select.Group>
                        ))}
                    </Select.Viewport>
                </Select.Content>
            </Select.Portal>
        </Select.Root>
    );
}

export default function SideBar() {
    const { t } = useTranslation();
    return (
        <nav className='settings-navigation' aria-label={t('config.shell.settings')}>
            {sections.map(([section, pages]) => (
                <div className='settings-nav-group' key={section}>
                    <h2>{t(`config.shell.${section}`)}</h2>
                    {pages.map(([page, Icon]) => (
                        <NavLink key={page} to={`/${page}`}>
                            <Icon size={17} aria-hidden='true' />
                            {t(`config.${page}.label`)}
                        </NavLink>
                    ))}
                </div>
            ))}
        </nav>
    );
}
