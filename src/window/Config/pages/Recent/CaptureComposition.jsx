import React, { useEffect, useState } from 'react';
import { Button, Modal, ModalBody, ModalContent, ModalFooter, ModalHeader } from '@nextui-org/react';
import { ArrowDown, ArrowUp } from 'lucide-react';
import { convertFileSrc, invoke } from '@tauri-apps/api/tauri';
import { useTranslation } from 'react-i18next';
import { planComposition } from '../../../../utils/captureComposition';
import { loadCaptureImage } from '../../../../utils/captureImage';
import { drawShape } from '../../../Annotate/render';
import { captureErrorKey, saveCapturePng } from '../../../QuickAccess/captureActions';

async function loadCapture(item) {
    const previewSrc = convertFileSrc(item.path);
    try {
        return { id: item.id, image: await loadCaptureImage(previewSrc, item.path), previewSrc };
    } catch {
        return { id: item.id, image: null, previewSrc };
    }
}

function encodeComposition(kind, steps, points) {
    const { width, height, frames } = planComposition(
        kind,
        steps.map(({ image }) => ({
            width: image.naturalWidth,
            height: image.naturalHeight,
        }))
    );
    const canvas = document.createElement('canvas');
    canvas.width = width;
    canvas.height = height;
    const context = canvas.getContext('2d');
    context.fillStyle = '#ffffff';
    context.fillRect(0, 0, width, height);
    frames.forEach((frame, index) => {
        const { id, image } = steps[index];
        context.drawImage(image, frame.x, frame.y, frame.width, frame.height);
        if (kind === 'tutorial') {
            const point = points[id];
            drawShape(context, {
                type: 'number',
                n: index + 1,
                color: '#243b84',
                strokeWidth: 4,
                x: point ? frame.x + frame.width * point.x : frame.x + 22,
                y: point ? frame.y + frame.height * point.y : frame.y + 22,
            });
        }
    });
    const data = canvas.toDataURL('image/png');
    if (!data.startsWith('data:image/png;base64,')) throw new Error('recent.output_too_large');
    return data;
}

function Comparison({ steps }) {
    const { t } = useTranslation();
    const [percent, setPercent] = useState(50);
    const [slider, setSlider] = useState(false);
    const [before, after] = steps;
    const aligned =
        before.image.naturalWidth === after.image.naturalWidth &&
        before.image.naturalHeight === after.image.naturalHeight;
    return (
        <div className='grid gap-3'>
            {aligned && (
                <Button
                    size='sm'
                    variant='flat'
                    className='justify-self-start'
                    onPress={() => setSlider(!slider)}
                >
                    {t(slider ? 'recent.side_by_side' : 'recent.slider_mode')}
                </Button>
            )}
            {aligned && slider ? (
                <>
                    <div className='relative mx-auto w-fit max-w-full overflow-hidden rounded-lg'>
                        <img
                            src={before.previewSrc}
                            className='block max-h-[48vh] max-w-full'
                            alt={t('recent.before')}
                        />
                        <img
                            src={after.previewSrc}
                            className='absolute inset-0 h-full w-full'
                            alt={t('recent.after')}
                            style={{ clipPath: `inset(0 ${100 - percent}% 0 0)` }}
                        />
                        <span
                            className='absolute inset-y-0 w-0.5 bg-primary'
                            style={{ left: `${percent}%` }}
                            aria-hidden='true'
                        />
                    </div>
                    <input
                        type='range'
                        min='0'
                        max='100'
                        value={percent}
                        onChange={(event) => setPercent(Number(event.target.value))}
                        aria-label={t('recent.slider_mode')}
                    />
                </>
            ) : (
                <div className='grid grid-cols-2 gap-2'>
                    {steps.map(({ id, previewSrc }, index) => (
                        <figure
                            key={id}
                            className='min-w-0'
                        >
                            <figcaption className='mb-1 text-sm'>
                                {t(index === 0 ? 'recent.before' : 'recent.after')}
                            </figcaption>
                            <img
                                src={previewSrc}
                                className='max-h-[48vh] max-w-full rounded-lg'
                                alt=''
                            />
                        </figure>
                    ))}
                </div>
            )}
        </div>
    );
}

export default function CaptureComposition({ kind, items, onClose }) {
    const { t } = useTranslation();
    const [steps, setSteps] = useState(null);
    const [points, setPoints] = useState({});
    const [error, setError] = useState('');
    const [saving, setSaving] = useState(false);
    const tutorial = kind === 'tutorial';
    useEffect(() => {
        let active = true;
        void Promise.all(items.map(loadCapture)).then((loaded) => {
            if (active) setSteps(loaded);
        });
        return () => {
            active = false;
        };
    }, [items]);
    const move = (index, destination) =>
        setSteps((current) => {
            const ordered = [...current];
            const [step] = ordered.splice(index, 1);
            ordered.splice(destination, 0, step);
            return ordered;
        });
    const exportPng = async () => {
        setSaving(true);
        setError('');
        try {
            const data = encodeComposition(kind, steps, points);
            const saved = await saveCapturePng(`snapshoot-${kind}.png`, (path) =>
                invoke('save_png_base64', { path, data })
            );
            if (saved) onClose();
        } catch (cause) {
            setError(captureErrorKey(cause));
        } finally {
            setSaving(false);
        }
    };
    const valid = steps?.length > 0 && steps.every(({ image }) => image) && (tutorial || steps.length === 2);
    return (
        <Modal
            isOpen
            onClose={onClose}
            isDismissable={!saving}
            size='3xl'
            scrollBehavior='inside'
        >
            <ModalContent>
                <ModalHeader>{t(tutorial ? 'recent.tutorial' : 'recent.compare')}</ModalHeader>
                <ModalBody>
                    {!steps && <p role='status'>{t('recent.loading')}</p>}
                    {!tutorial && valid && <Comparison steps={steps} />}
                    {!tutorial && steps && !valid && <p role='alert'>{t('recent.capture_missing')}</p>}
                    {tutorial && steps?.length > 0 && (
                        <>
                            <p className='text-sm text-default-600'>{t('recent.tutorial_hint')}</p>
                            <ol className='grid gap-3'>
                                {steps.map(({ id, image, previewSrc }, index) => {
                                    const point = points[id];
                                    return (
                                        <li
                                            key={id}
                                            className='rounded-xl border border-divider p-3'
                                        >
                                            <div className='mb-2 flex items-center gap-2'>
                                                <span className='text-sm'>
                                                    {t('recent.step', { number: index + 1 })}
                                                </span>
                                                <Button
                                                    isIconOnly
                                                    size='sm'
                                                    variant='flat'
                                                    aria-label={t('recent.move_up')}
                                                    isDisabled={index === 0}
                                                    onPress={() => move(index, index - 1)}
                                                >
                                                    <ArrowUp size={15} />
                                                </Button>
                                                <Button
                                                    isIconOnly
                                                    size='sm'
                                                    variant='flat'
                                                    aria-label={t('recent.move_down')}
                                                    isDisabled={index === steps.length - 1}
                                                    onPress={() => move(index, index + 1)}
                                                >
                                                    <ArrowDown size={15} />
                                                </Button>
                                                <Button
                                                    size='sm'
                                                    variant='light'
                                                    onPress={() =>
                                                        setSteps((current) => current.filter((step) => step.id !== id))
                                                    }
                                                >
                                                    {t('recent.remove_step')}
                                                </Button>
                                            </div>
                                            {image ? (
                                                <button
                                                    type='button'
                                                    className='relative block max-w-full cursor-crosshair'
                                                    aria-label={t('recent.mark_step', { number: index + 1 })}
                                                    onClick={(event) => {
                                                        const { left, top, width, height } =
                                                            event.currentTarget.getBoundingClientRect();
                                                        const next =
                                                            event.detail === 0
                                                                ? { x: 0.5, y: 0.5 }
                                                                : {
                                                                      x: (event.clientX - left) / width,
                                                                      y: (event.clientY - top) / height,
                                                                  };
                                                        setPoints((current) => ({ ...current, [id]: next }));
                                                    }}
                                                >
                                                    <img
                                                        src={previewSrc}
                                                        alt=''
                                                        className='max-h-[34vh] max-w-full rounded-lg'
                                                        draggable={false}
                                                    />
                                                    <span
                                                        className='pointer-events-none absolute flex size-6 items-center justify-center rounded-full bg-primary text-xs font-bold text-white'
                                                        style={
                                                            point
                                                                ? {
                                                                      left: `${point.x * 100}%`,
                                                                      top: `${point.y * 100}%`,
                                                                      transform: 'translate(-50%, -50%)',
                                                                  }
                                                                : { left: 8, top: 8 }
                                                        }
                                                    >
                                                        {index + 1}
                                                    </span>
                                                </button>
                                            ) : (
                                                <p role='alert'>{t('recent.capture_missing')}</p>
                                            )}
                                        </li>
                                    );
                                })}
                            </ol>
                        </>
                    )}
                    {error && (
                        <p
                            role='alert'
                            className='text-danger'
                        >
                            {t(error)}
                        </p>
                    )}
                </ModalBody>
                <ModalFooter>
                    <Button
                        variant='light'
                        onPress={onClose}
                        isDisabled={saving}
                    >
                        {t('annotate.close')}
                    </Button>
                    <Button
                        color='primary'
                        onPress={exportPng}
                        isDisabled={!valid}
                        isLoading={saving}
                    >
                        {t('recent.export_png')}
                    </Button>
                </ModalFooter>
            </ModalContent>
        </Modal>
    );
}
