import React from 'react';
import { Check } from 'lucide-react';
import { useTranslation } from 'react-i18next';

export default function AppearancePicker({ value, onChange }) {
    const { t } = useTranslation();
    return (
        <fieldset className='appearance-picker'>
            <legend className='settings-section-title'>{t('config.general.app_theme')}</legend>
            <div className='appearance-options'>
                {['light', 'dark', 'system'].map((mode) => (
                    <label key={mode} className='appearance-choice' data-mode={mode}>
                        <input
                            type='radio'
                            name='appearance'
                            className='sr-only'
                            checked={value === mode}
                            onChange={() => onChange(mode)}
                        />
                        <span className='appearance-preview' aria-hidden='true'>
                            <span className='appearance-miniature'><i /><i /><i /></span>
                        </span>
                        <span className='appearance-caption'>
                            {t(`config.general.theme.${mode}`)}
                            <span className='appearance-check' aria-hidden='true'><Check size={10} strokeWidth={3} /></span>
                        </span>
                    </label>
                ))}
            </div>
        </fieldset>
    );
}
