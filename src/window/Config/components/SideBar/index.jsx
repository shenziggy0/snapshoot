import React from 'react';
import { NavLink, useLocation, useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { motion } from 'framer-motion';
import { AudioLines, ChevronDown, Cloud, History, Info, Keyboard, Languages, ScanText, Settings2, Video } from 'lucide-react';

const sections = [
    ['workspace', [
        ['general', Settings2, 'graphite'],
        ['record', Video, 'coral'],
        ['recognize', ScanText, 'blue'],
        ['translate', Languages, 'teal'],
        ['hotkey', Keyboard, 'violet'],
    ]],
    ['application', [
        ['service', AudioLines, 'indigo'],
        ['history', History, 'amber'],
        ['backup', Cloud, 'cyan'],
        ['about', Info, 'graphite'],
    ]],
];

export function CompactNavigation() {
    const { t } = useTranslation();
    const { pathname } = useLocation();
    const navigate = useNavigate();
    return (
        <label className='settings-compact-nav snap-glass'>
            <span>{t('config.shell.settings')}</span>
            <ChevronDown size={14} aria-hidden='true' />
            <select
                aria-label={t('config.shell.settings')}
                value={pathname === '/' ? '/general' : pathname}
                onChange={(event) => navigate(event.target.value)}
            >
                {sections.map(([section, pages]) => (
                    <optgroup key={section} label={t(`config.shell.${section}`)}>
                        {pages.map(([page]) => (
                            <option key={page} value={`/${page}`}>{t(`config.${page}.label`)}</option>
                        ))}
                    </optgroup>
                ))}
            </select>
        </label>
    );
}

export default function SideBar() {
    const { t } = useTranslation();
    return (
        <nav className='settings-navigation' aria-label={t('config.shell.settings')}>
            {sections.map(([section, pages]) => (
                <div className='settings-nav-group' key={section}>
                    <h2>{t(`config.shell.${section}`)}</h2>
                    {pages.map(([page, Icon, tint]) => (
                        <NavLink key={page} to={`/${page}`}>
                            {({ isActive }) => (
                                <>
                                    {isActive && (
                                        <motion.span
                                            aria-hidden='true'
                                            className='settings-selection'
                                            layoutId='settings-selection'
                                            transition={{ type: 'spring', stiffness: 520, damping: 42 }}
                                        />
                                    )}
                                    <span className={`settings-symbol tint-${tint}`} aria-hidden='true'><Icon size={16} /></span>
                                    <span className='settings-nav-label'>{t(`config.${page}.label`)}</span>
                                </>
                            )}
                        </NavLink>
                    ))}
                </div>
            ))}
        </nav>
    );
}
