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
    MdGTranslate,
    MdGridOn,
    MdHorizontalRule,
    MdPrivacyTip,
    MdPushPin,
    MdPhotoSizeSelectActual,
    MdRadioButtonUnchecked,
    MdRedo,
    MdSaveAlt,
    MdTextFields,
    MdTranslate,
    MdUndo,
    MdVideocam,
} from 'react-icons/md';
import { HiOutlinePencil } from 'react-icons/hi';

const COLORS = ['#e11d48', '#f97316', '#eab308', '#22c55e', '#0ea5e9', '#6366f1', '#111827', '#ffffff'];
const STROKES = [2, 4, 8, 12];
const FONTS = [16, 24, 36, 52];

const TOOLS = [
    { id: 'rect', icon: MdCropSquare, label: 'annotate.rect' },
    { id: 'ellipse', icon: MdRadioButtonUnchecked, label: 'annotate.ellipse' },
    { id: 'line', icon: MdHorizontalRule, label: 'annotate.line' },
    { id: 'arrow', icon: MdArrowRightAlt, label: 'annotate.arrow' },
    { id: 'pen', icon: MdGesture, label: 'annotate.pen' },
    { id: 'highlighter', icon: HiOutlinePencil, label: 'annotate.highlighter' },
    { id: 'text', icon: MdTextFields, label: 'annotate.text' },
    { id: 'mosaic', icon: MdGridOn, label: 'annotate.mosaic' },
    { id: 'blur', icon: MdBlurOn, label: 'annotate.blur' },
    { id: 'number', icon: MdFormatListNumbered, label: 'annotate.number' },
];

function IconButton({ title, active, disabled, onClick, dark, children }) {
    return (
        <button
            type='button'
            title={title}
            aria-label={title}
            disabled={disabled}
            onMouseDown={(event) => event.stopPropagation()}
            onClick={onClick}
            className={`flex h-7 w-7 items-center justify-center rounded-md disabled:opacity-30 ${
                active
                    ? 'bg-[#1E63F0] text-white'
                    : dark
                      ? 'text-white hover:bg-white/15'
                      : 'text-neutral-800 hover:bg-black/10'
            }`}
        >
            {children}
        </button>
    );
}

export default function CaptureToolbar(props) {
    const { t } = useTranslation();
    const {
        dark,
        tool,
        setTool,
        color,
        setColor,
        strokeWidth,
        setStrokeWidth,
        fontSize,
        setFontSize,
        canUndo,
        canRedo,
        onUndo,
        onRedo,
        onOcr,
        onTranslate,
        onInlineTranslate,
        onRedact,
        onLongScreenshot,
        onScreenRecord,
        recordIntent = false,
        longCaptureActive = false,
        copyBusy = false,
        onPin,
        onSave,
        onCopy,
        onClose,
    } = props;
    const bar = dark ? 'border-white/10 bg-neutral-900/95 text-white' : 'border-black/10 bg-white/95 text-neutral-800';
    const showProps = Boolean(tool) && !longCaptureActive;

    if (recordIntent) {
        return (
            <div className={`flex items-center gap-2 rounded-xl border p-1.5 shadow-lg ${bar}`}>
                <button
                    type='button'
                    disabled={!onScreenRecord}
                    onClick={onScreenRecord}
                    className='flex h-9 items-center gap-1 rounded-md bg-red-600 px-3 text-sm text-white disabled:opacity-40'
                >
                    <MdVideocam />
                    {t('screen_record.start')}
                </button>
                <IconButton
                    title={t('annotate.close')}
                    dark={dark}
                    onClick={onClose}
                >
                    <MdClose size={18} />
                </IconButton>
            </div>
        );
    }

    return (
        <div className='flex flex-col items-center gap-1'>
            <div className={`flex items-center gap-0.5 rounded-xl border px-1 py-1 shadow-lg backdrop-blur ${bar}`}>
                {TOOLS.map((item) => {
                    const Icon = item.icon;
                    return (
                        <IconButton
                            key={item.id}
                            title={t(item.label)}
                            active={tool === item.id}
                            disabled={longCaptureActive}
                            dark={dark}
                            onClick={() => setTool(tool === item.id ? '' : item.id)}
                        >
                            <Icon size={16} />
                        </IconButton>
                    );
                })}
                <span className={`mx-1 h-4 w-px ${dark ? 'bg-white/20' : 'bg-black/15'}`} />
                <IconButton
                    title={t('annotate.undo')}
                    disabled={longCaptureActive || !canUndo}
                    dark={dark}
                    onClick={onUndo}
                >
                    <MdUndo size={16} />
                </IconButton>
                <IconButton
                    title={t('annotate.redo')}
                    disabled={longCaptureActive || !canRedo}
                    dark={dark}
                    onClick={onRedo}
                >
                    <MdRedo size={16} />
                </IconButton>
                <span className={`mx-1 h-4 w-px ${dark ? 'bg-white/20' : 'bg-black/15'}`} />
                {onLongScreenshot && !longCaptureActive && (
                    <IconButton
                        title={t('annotate.long_screenshot')}
                        dark={dark}
                        onClick={onLongScreenshot}
                    >
                        <MdPhotoSizeSelectActual size={16} />
                    </IconButton>
                )}
                {onScreenRecord && (
                    <IconButton
                        title={t('screen_record.label')}
                        disabled={longCaptureActive}
                        dark={dark}
                        onClick={onScreenRecord}
                    >
                        <MdVideocam size={18} />
                    </IconButton>
                )}
                <IconButton
                    title={t('annotate.ocr')}
                    disabled={longCaptureActive}
                    dark={dark}
                    onClick={onOcr}
                >
                    <MdDocumentScanner size={16} />
                </IconButton>
                <IconButton
                    title={t('annotate.translate')}
                    disabled={longCaptureActive}
                    dark={dark}
                    onClick={onTranslate}
                >
                    <MdTranslate size={16} />
                </IconButton>
                <IconButton
                    title={t('annotate.translate_overlay')}
                    disabled={longCaptureActive}
                    dark={dark}
                    onClick={onInlineTranslate}
                >
                    <MdGTranslate size={16} />
                </IconButton>
                <IconButton
                    title={t('annotate.privacy')}
                    disabled={longCaptureActive}
                    dark={dark}
                    onClick={onRedact}
                >
                    <MdPrivacyTip size={16} />
                </IconButton>
                <IconButton
                    title={t('annotate.pin')}
                    disabled={longCaptureActive}
                    dark={dark}
                    onClick={onPin}
                >
                    <MdPushPin size={16} />
                </IconButton>
                <IconButton
                    title={t('annotate.save')}
                    disabled={longCaptureActive}
                    dark={dark}
                    onClick={onSave}
                >
                    <MdSaveAlt size={16} />
                </IconButton>
                <IconButton
                    title={`${t('annotate.copy')} (Enter)`}
                    disabled={copyBusy}
                    dark={dark}
                    onClick={onCopy}
                >
                    <MdContentCopy size={16} />
                </IconButton>
                <IconButton
                    title={`${t('annotate.close')} (Esc)`}
                    dark={dark}
                    onClick={onClose}
                >
                    <MdClose size={16} />
                </IconButton>
            </div>
            {showProps && (
                <div className={`flex items-center gap-1 rounded-lg border px-2 py-1 shadow-md ${bar}`}>
                    {COLORS.map((item) => (
                        <button
                            key={item}
                            type='button'
                            title={t('annotate.color')}
                            onMouseDown={(event) => event.stopPropagation()}
                            onClick={() => setColor(item)}
                            className={`h-4 w-4 rounded-full border ${
                                color === item ? 'border-sky-500' : 'border-black/20'
                            }`}
                            style={{ background: item }}
                        />
                    ))}
                    <span className={`mx-1 h-3 w-px ${dark ? 'bg-white/20' : 'bg-black/15'}`} />
                    {STROKES.map((item) => (
                        <button
                            key={item}
                            type='button'
                            title={t('annotate.stroke')}
                            onMouseDown={(event) => event.stopPropagation()}
                            onClick={() => setStrokeWidth(item)}
                            className={`rounded px-1 text-[11px] ${
                                strokeWidth === item ? (dark ? 'bg-white/20' : 'bg-black/10') : ''
                            }`}
                        >
                            {item}
                        </button>
                    ))}
                    {tool === 'text' && (
                        <>
                            <span className={`mx-1 h-3 w-px ${dark ? 'bg-white/20' : 'bg-black/15'}`} />
                            {FONTS.map((item) => (
                                <button
                                    key={item}
                                    type='button'
                                    title={t('annotate.font')}
                                    onMouseDown={(event) => event.stopPropagation()}
                                    onClick={() => setFontSize(item)}
                                    className={`rounded px-1 text-[11px] ${
                                        fontSize === item ? (dark ? 'bg-white/20' : 'bg-black/10') : ''
                                    }`}
                                >
                                    {item}
                                </button>
                            ))}
                        </>
                    )}
                </div>
            )}
        </div>
    );
}
