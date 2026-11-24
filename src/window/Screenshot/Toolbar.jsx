import ToolButton from '../Annotate/ToolButton';
import {
    Blend,
    Captions,
    Circle,
    Copy,
    Download,
    GalleryVerticalEnd,
    Grid3X3,
    Highlighter,
    Languages,
    ListOrdered,
    Minus,
    MoveUpRight,
    Pin,
    Redo2,
    ScanText,
    ShieldCheck,
    Spline,
    Square,
    Type,
    Undo2,
    Video,
    X,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import React from 'react';

const COLORS = ['#e11d48', '#f97316', '#eab308', '#22c55e', '#0ea5e9', '#6366f1', '#111827', '#ffffff'];
const STROKES = [2, 4, 8, 12];
const FONTS = [16, 24, 36, 52];

const TOOLS = [
    { id: 'rect', icon: Square, label: 'annotate.rect' },
    { id: 'ellipse', icon: Circle, label: 'annotate.ellipse' },
    { id: 'line', icon: Minus, label: 'annotate.line' },
    { id: 'arrow', icon: MoveUpRight, label: 'annotate.arrow' },
    { id: 'pen', icon: Spline, label: 'annotate.pen' },
    { id: 'highlighter', icon: Highlighter, label: 'annotate.highlighter' },
    { id: 'text', icon: Type, label: 'annotate.text' },
    { id: 'mosaic', icon: Grid3X3, label: 'annotate.mosaic' },
    { id: 'blur', icon: Blend, label: 'annotate.blur' },
    { id: 'number', icon: ListOrdered, label: 'annotate.number' },
];

export default function CaptureToolbar(props) {
    const { t } = useTranslation();
    const {
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
        redactPending = false,
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

    const showProps = Boolean(tool) && !longCaptureActive;

    if (recordIntent) {
        return (
            <div className='snap-tools snap-tools-row snap-glass'>
                <button
                    type='button'
                    disabled={!onScreenRecord}
                    onClick={onScreenRecord}
                    className='snap-tool snap-tool-primary'
                >
                    <Video />
                    {t('screen_record.start')}
                </button>
                <ToolButton
                    title={t('annotate.close')}
                    onClick={onClose}
                >
                    <X size={18} />
                </ToolButton>
            </div>
        );
    }

    return (
        <div className='snap-tools flex flex-col items-center gap-1'>
            <div className='snap-tools-row snap-glass'>
                {TOOLS.map((item) => {
                    const Icon = item.icon;
                    return (
                        <ToolButton
                            key={item.id}
                            title={t(item.label)}
                            active={tool === item.id}
                            disabled={longCaptureActive}
                            onClick={() => setTool(tool === item.id ? '' : item.id)}
                        >
                            <Icon size={16} />
                        </ToolButton>
                    );
                })}
                <span className='snap-tools-divider' aria-hidden='true' />
                <ToolButton
                    title={t('annotate.undo')}
                    disabled={longCaptureActive || !canUndo}
                    onClick={onUndo}
                >
                    <Undo2 size={16} />
                </ToolButton>
                <ToolButton
                    title={t('annotate.redo')}
                    disabled={longCaptureActive || !canRedo}
                    onClick={onRedo}
                >
                    <Redo2 size={16} />
                </ToolButton>
                <span className='snap-tools-divider' aria-hidden='true' />
                {onLongScreenshot && !longCaptureActive && (
                    <ToolButton
                        title={t('annotate.long_screenshot')}
                        onClick={onLongScreenshot}
                    >
                        <GalleryVerticalEnd size={16} />
                    </ToolButton>
                )}
                {onScreenRecord && (
                    <ToolButton
                        title={t('screen_record.label')}
                        disabled={longCaptureActive}
                        onClick={onScreenRecord}
                    >
                        <Video size={18} />
                    </ToolButton>
                )}
                <ToolButton
                    title={t('annotate.ocr')}
                    disabled={longCaptureActive || redactPending}
                    onClick={onOcr}
                >
                    <ScanText size={16} />
                </ToolButton>
                <ToolButton
                    title={t('annotate.translate')}
                    disabled={longCaptureActive || redactPending}
                    onClick={onTranslate}
                >
                    <Languages size={16} />
                </ToolButton>
                {onInlineTranslate && (
                    <ToolButton
                        title={t('annotate.translate_overlay')}
                        disabled={longCaptureActive || redactPending}
                        onClick={onInlineTranslate}
                    >
                        <Captions size={16} />
                    </ToolButton>
                )}
                {onRedact && (
                    <ToolButton
                        title={t(redactPending ? 'annotate.redacting' : 'annotate.privacy')}
                        disabled={longCaptureActive || redactPending}
                        onClick={onRedact}
                    >
                        <ShieldCheck size={16} />
                    </ToolButton>
                )}
                <ToolButton
                    title={t('annotate.pin')}
                    disabled={longCaptureActive || redactPending}
                    onClick={onPin}
                >
                    <Pin size={16} />
                </ToolButton>
                <ToolButton
                    title={t('annotate.save')}
                    disabled={longCaptureActive || redactPending}
                    onClick={onSave}
                >
                    <Download size={16} />
                </ToolButton>
                <ToolButton
                    title={`${t('annotate.copy')} (Enter)`}
                    disabled={copyBusy || redactPending}
                    primary
                    onClick={onCopy}
                >
                    <Copy size={16} />
                    {t('annotate.copy')}
                </ToolButton>
                <ToolButton
                    title={`${t('annotate.close')} (Esc)`}
                    onClick={onClose}
                >
                    <X size={16} />
                </ToolButton>
            </div>
            {showProps && (
                <div className='snap-tools-options snap-glass'>
                    {COLORS.map((item) => (
                        <button
                            key={item}
                            type='button'
                            title={`${t('annotate.color')} · ${item}`}
                            aria-pressed={color === item}
                            onMouseDown={(event) => event.stopPropagation()}
                            onClick={() => setColor(item)}
                            className='snap-swatch'
                            style={{ '--swatch': item }}
                        />
                    ))}
                    <span className='snap-tools-divider' aria-hidden='true' />
                    {STROKES.map((item) => (
                        <button
                            key={item}
                            type='button'
                            title={`${t('annotate.stroke')} · ${item} px`}
                            aria-pressed={strokeWidth === item}
                            onMouseDown={(event) => event.stopPropagation()}
                            onClick={() => setStrokeWidth(item)}
                            className='snap-option'
                        >
                            {item}
                        </button>
                    ))}
                    {tool === 'text' && (
                        <>
                            <span className='snap-tools-divider' aria-hidden='true' />
                            {FONTS.map((item) => (
                                <button
                                    key={item}
                                    type='button'
                                    title={`${t('annotate.font')} · ${item} px`}
                                    aria-pressed={fontSize === item}
                                    onMouseDown={(event) => event.stopPropagation()}
                                    onClick={() => setFontSize(item)}
                                    className='snap-option'
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
