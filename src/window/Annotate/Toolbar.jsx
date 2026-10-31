import { useTranslation } from 'react-i18next';
import React from 'react';
import {
    MdArrowRightAlt,
    MdBlurOn,
    MdClose,
    MdContentCopy,
    MdCropSquare,
    MdDocumentScanner,
    MdFormatListNumbered,
    MdGesture,
    MdGridOn,
    MdPushPin,
    MdRadioButtonUnchecked,
    MdRedo,
    MdSaveAlt,
    MdTextFields,
    MdTranslate,
    MdUndo,
} from 'react-icons/md';
import { HiOutlinePencil } from 'react-icons/hi';

const COLORS = ['#ef4444', '#f97316', '#eab308', '#22c55e', '#0ea5e9', '#6366f1', '#ffffff', '#111827'];
const WIDTHS = [2, 4, 8, 12];

const TOOLS = [
    { id: 'rect', icon: MdCropSquare, label: 'annotate.rect' },
    { id: 'ellipse', icon: MdRadioButtonUnchecked, label: 'annotate.ellipse' },
    { id: 'arrow', icon: MdArrowRightAlt, label: 'annotate.arrow' },
    { id: 'pen', icon: MdGesture, label: 'annotate.pen' },
    { id: 'highlighter', icon: HiOutlinePencil, label: 'annotate.highlighter' },
    { id: 'text', icon: MdTextFields, label: 'annotate.text' },
    { id: 'mosaic', icon: MdGridOn, label: 'annotate.mosaic' },
    { id: 'blur', icon: MdBlurOn, label: 'annotate.blur' },
    { id: 'number', icon: MdFormatListNumbered, label: 'annotate.number' },
];

function IconButton({ title, active, disabled, onClick, children }) {
    return (
        <button
            type='button'
            title={title}
            disabled={disabled}
            onClick={onClick}
            className={`flex h-8 w-8 items-center justify-center rounded text-white hover:bg-white/20 disabled:opacity-30 ${
                active ? 'bg-white/25' : ''
            }`}
        >
            {children}
        </button>
    );
}

export default function Toolbar(props) {
    const { t } = useTranslation();
    const {
        tool,
        setTool,
        color,
        setColor,
        strokeWidth,
        setStrokeWidth,
        includeAnnotations,
        setIncludeAnnotations,
        canUndo,
        canRedo,
        onUndo,
        onRedo,
        onCopy,
        onSave,
        onPin,
        onOcr,
        onTranslate,
        onClose,
        disabled,
    } = props;

    return (
        <div className='pointer-events-auto flex w-[680px] max-w-[calc(100vw-12px)] flex-col gap-1 rounded-xl border border-white/10 bg-neutral-900/95 p-1.5 text-white shadow-lg'>
            <div className='flex flex-wrap items-center gap-0.5'>
                {TOOLS.map((item) => {
                    const Icon = item.icon;
                    return (
                        <IconButton
                            key={item.id}
                            title={t(item.label)}
                            active={tool === item.id}
                            onClick={() => setTool(item.id)}
                        >
                            <Icon size={18} />
                        </IconButton>
                    );
                })}
                <span className='mx-1 h-5 w-px bg-white/20' />
                <IconButton
                    title={t('annotate.undo')}
                    disabled={!canUndo}
                    onClick={onUndo}
                >
                    <MdUndo size={18} />
                </IconButton>
                <IconButton
                    title={t('annotate.redo')}
                    disabled={!canRedo}
                    onClick={onRedo}
                >
                    <MdRedo size={18} />
                </IconButton>
                <span className='mx-1 h-5 w-px bg-white/20' />
                <IconButton
                    title={t('annotate.copy')}
                    disabled={disabled}
                    onClick={onCopy}
                >
                    <MdContentCopy size={18} />
                </IconButton>
                <IconButton
                    title={t('annotate.save')}
                    disabled={disabled}
                    onClick={onSave}
                >
                    <MdSaveAlt size={18} />
                </IconButton>
                <IconButton
                    title={t('annotate.pin')}
                    disabled={disabled}
                    onClick={onPin}
                >
                    <MdPushPin size={18} />
                </IconButton>
                <IconButton
                    title={t('annotate.ocr')}
                    disabled={disabled}
                    onClick={onOcr}
                >
                    <MdDocumentScanner size={18} />
                </IconButton>
                <IconButton
                    title={t('annotate.translate')}
                    disabled={disabled}
                    onClick={onTranslate}
                >
                    <MdTranslate size={18} />
                </IconButton>
                <IconButton
                    title={t('annotate.close')}
                    onClick={onClose}
                >
                    <MdClose size={18} />
                </IconButton>
            </div>
            <div className='flex flex-wrap items-center gap-1 px-1'>
                {COLORS.map((item) => (
                    <button
                        key={item}
                        type='button'
                        title={t('annotate.color')}
                        onClick={() => setColor(item)}
                        className={`h-5 w-5 rounded-full border ${color === item ? 'border-white' : 'border-black/40'}`}
                        style={{ background: item }}
                    />
                ))}
                <input
                    aria-label={t('annotate.color')}
                    type='color'
                    value={color}
                    onChange={(event) => setColor(event.target.value)}
                    className='h-6 w-8 cursor-pointer bg-transparent'
                />
                <span className='mx-1 h-4 w-px bg-white/20' />
                {WIDTHS.map((item) => (
                    <button
                        key={item}
                        type='button'
                        title={t('annotate.stroke')}
                        onClick={() => setStrokeWidth(item)}
                        className={`rounded px-1.5 text-xs ${strokeWidth === item ? 'bg-white/25' : 'hover:bg-white/10'}`}
                    >
                        {item}
                    </button>
                ))}
                <button
                    type='button'
                    onClick={() => setIncludeAnnotations(!includeAnnotations)}
                    className={`ml-auto rounded px-2 py-0.5 text-xs ${
                        includeAnnotations ? 'bg-sky-600' : 'bg-white/10'
                    }`}
                >
                    {includeAnnotations ? t('annotate.include_annotations') : t('annotate.raw_image')}
                </button>
            </div>
        </div>
    );
}
