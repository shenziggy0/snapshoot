import { useTranslation } from 'react-i18next';
import { Button } from '@nextui-org/react';
import React from 'react';

export function Config(props) {
    const { updateServiceList, onClose } = props;
    const { t } = useTranslation();

    return (
        <>
            <div>{t('services.no_need')}</div>
            <div style={{ color: '#e67e22', margin: '8px 0' }}>
                {t('services.upstream_notice', { name: 'ECDICT', host: 'pot-app.com/api/dict' })}
            </div>
            <div>
                <Button
                    fullWidth
                    color='primary'
                    onPress={() => {
                        updateServiceList('ecdict');
                        onClose();
                    }}
                >
                    {t('common.save')}
                </Button>
            </div>
        </>
    );
}
