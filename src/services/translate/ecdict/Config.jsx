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
                注意：使用 ECDICT 会把待查文本发送到上游 Pot 的服务器 pot-app.com/api/dict，不是 Snapshoot 自己的服务器。启用即表示你接受这一点。
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
