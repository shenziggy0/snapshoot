import { GripHorizontal, Pencil, Trash2 } from 'lucide-react';
import { Spacer, Button, Switch } from '@nextui-org/react';
import { useTranslation } from 'react-i18next';
import React from 'react';

import * as builtinServices from '../../../../../../services/translate';
import { useConfig } from '../../../../../../hooks';
import { INSTANCE_NAME_CONFIG_KEY, ServiceSourceType, getDisplayInstanceName, getServiceName, getServiceSouceType } from '../../../../../../utils/service_instance';

export default function ServiceItem(props) {
    const { serviceInstanceKey, pluginList, deleteServiceInstance, setCurrentConfigKey, onConfigOpen, ...drag } = props;
    const { t } = useTranslation();
    const [serviceInstanceConfig, setServiceInstanceConfig] = useConfig(serviceInstanceKey, {});

    const serviceSourceType = getServiceSouceType(serviceInstanceKey)
    const serviceName = getServiceName(serviceInstanceKey)

    return serviceSourceType === ServiceSourceType.PLUGIN && !(serviceName in pluginList) ? (
        <></>
    ) : (
        serviceInstanceConfig !== null && (
            <div className='settings-service-row'>
                <div className='flex'>
                    <div
                        {...drag}
                        className='service-grip'
                    >
                        <GripHorizontal size="1em" />
                    </div>

                    <Spacer x={2} />
                    {serviceSourceType === ServiceSourceType.BUILDIN && (
                        <>
                            <img
                                alt=''
                                src={`${builtinServices[serviceName].info.icon}`}
                                className='h-[24px] w-[24px] my-auto'
                                draggable={false}
                            />
                            <Spacer x={2} />
                            <h2 id={serviceInstanceKey} className='my-auto'>{getDisplayInstanceName(serviceInstanceConfig[INSTANCE_NAME_CONFIG_KEY], () => t(`services.translate.${serviceName}.title`))}</h2>
                        </>
                    )}
                    {serviceSourceType === ServiceSourceType.PLUGIN && (
                        <>
                            <img
                                alt=''
                                src={pluginList[serviceName].icon}
                                className='h-[24px] w-[24px] my-auto'
                                draggable={false}
                            />
                            <Spacer x={2} />
                            <h2 id={serviceInstanceKey} className='my-auto'>{getDisplayInstanceName(serviceInstanceConfig[INSTANCE_NAME_CONFIG_KEY], () => pluginList[serviceName].display) +  `[${t('common.plugin')}]`}</h2>
                        </>
                    )}
                </div>
                <div className='flex'>
                    <Switch
                        aria-labelledby={serviceInstanceKey}
                        size='sm'
                        isSelected={serviceInstanceConfig['enable'] ?? true}
                        onValueChange={(v) => {
                            setServiceInstanceConfig({ ...serviceInstanceConfig, enable: v });
                        }}
                    />
                    <Button
                        aria-label={t('quick_access.edit')}
                        isIconOnly
                        size='sm'
                        variant='light'
                        onPress={() => {
                            setCurrentConfigKey(serviceInstanceKey);
                            onConfigOpen();
                        }}
                    >
                        <Pencil size={17} aria-hidden='true' />
                    </Button>
                    <Spacer x={2} />
                    <Button
                        aria-label={t('quick_access.delete')}
                        isIconOnly
                        size='sm'
                        variant='light'
                        color='danger'
                        onPress={() => {
                            deleteServiceInstance(serviceInstanceKey);
                        }}
                    >
                        <Trash2 size={17} aria-hidden='true' />
                    </Button>
                </div>
            </div>
        )
    );
}
