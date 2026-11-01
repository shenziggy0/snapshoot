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
                注意：使用 Lingva 会把待翻译文本发送到上游 Pot 的服务器 lingva.pot-app.com，不是 Snapshoot 自己的服务器。启用即表示你接受这一点。
            </div>
            <div>
                <Button
                    fullWidth
                    color='primary'
                    onPress={() => {
                        updateServiceList('lingva');
                        onClose();
                    }}
                >
                    {t('common.save')}
                </Button>
            </div>
        </>
    );
}
