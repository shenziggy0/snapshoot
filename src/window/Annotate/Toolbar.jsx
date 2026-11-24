import ToolButton from './ToolButton';
import {
    Blend,
    Circle,
    Copy,
    Download,
    Grid3X3,
    Highlighter,
    Languages,
    ListOrdered,
    MoveUpRight,
    Pin,
    Redo2,
    ScanText,
    Spline,
    Square,
    Type,
    Undo2,
    X,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import React from 'react';

const COLORS = ['#ef4444', '#f97316', '#eab308', '#22c55e', '#0ea5e9', '#6366f1', '#ffffff', '#111827'];
const WIDTHS = [2, 4, 8, 12];

const TOOLS = [
    { id: 'rect', icon: Square, label: 'annotate.rect' },
    { id: 'ellipse', icon: Circle, label: 'annotate.ellipse' },
    { id: 'arrow', icon: MoveUpRight, label: 'annotate.arrow' },
    { id: 'pen', icon: Spline, label: 'annotate.pen' },
    { id: 'highlighter', icon: Highlighter, label: 'annotate.highlighter' },
    { id: 'text', icon: Type, label: 'annotate.text' },
    { id: 'mosaic', icon: Grid3X3, label: 'annotate.mosaic' },
    { id: 'blur', icon: Blend, label: 'annotate.blur' },
    { id: 'number', icon: ListOrdered, label: 'annotate.number' },
];

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
        <div className='snap-tools snap-editor-tools'>
            <div className='snap-tools-row snap-glass'>
                {TOOLS.map((item) => {
                    const Icon = item.icon;
                    return (
                        <ToolButton
                            key={item.id}
                            title={t(item.label)}
                            active={tool === item.id}
                            onClick={() => setTool(item.id)}
                        >
                            <Icon size={18} />
                        </ToolButton>
                    );
                })}
                <span className='snap-tools-divider' aria-hidden='true' />
                <ToolButton
                    title={t('annotate.undo')}
                    disabled={!canUndo}
                    onClick={onUndo}
                >
                    <Undo2 size={18} />
                </ToolButton>
                <ToolButton
                    title={t('annotate.redo')}
                    disabled={!canRedo}
                    onClick={onRedo}
                >
                    <Redo2 size={18} />
                </ToolButton>
                <span className='snap-tools-divider' aria-hidden='true' />
                <ToolButton
                    title={t('annotate.copy')}
                    disabled={disabled}
                    primary
                    onClick={onCopy}
                >
                    <Copy size={18} />
                    {t('annotate.copy')}
                </ToolButton>
                <ToolButton
                    title={t('annotate.save')}
                    disabled={disabled}
                    onClick={onSave}
                >
                    <Download size={18} />
                </ToolButton>
                <ToolButton
                    title={t('annotate.pin')}
                    disabled={disabled}
                    onClick={onPin}
                >
                    <Pin size={18} />
                </ToolButton>
                <ToolButton
                    title={t('annotate.ocr')}
                    disabled={disabled}
                    onClick={onOcr}
                >
                    <ScanText size={18} />
                </ToolButton>
                <ToolButton
                    title={t('annotate.translate')}
                    disabled={disabled}
                    onClick={onTranslate}
                >
                    <Languages size={18} />
                </ToolButton>
                <ToolButton
                    title={t('annotate.close')}
                    onClick={onClose}
                >
                    <X size={18} />
                </ToolButton>
            </div>
            <div className='snap-tools-options snap-glass'>
                {COLORS.map((item) => (
                    <button
                        key={item}
                        type='button'
                        title={`${t('annotate.color')} · ${item}`}
                        aria-pressed={color === item}
                        onClick={() => setColor(item)}
                        className='snap-swatch'
                        style={{ '--swatch': item }}
                    />
                ))}
                <input
                    aria-label={t('annotate.color')}
                    type='color'
                    value={color}
                    onChange={(event) => setColor(event.target.value)}
                    className='h-6 w-8 cursor-pointer bg-transparent'
                />
                <span className='snap-tools-divider' aria-hidden='true' />
                {WIDTHS.map((item) => (
                    <button
                        key={item}
                        type='button'
                        title={`${t('annotate.stroke')} · ${item} px`}
                        aria-pressed={strokeWidth === item}
                        onClick={() => setStrokeWidth(item)}
                        className='snap-option'
                    >
                        {item}
                    </button>
                ))}
                <button
                    type='button'
                    onClick={() => setIncludeAnnotations(!includeAnnotations)}
                    aria-pressed={includeAnnotations}
                    className='snap-editor-mode'
                >
                    {includeAnnotations ? t('annotate.include_annotations') : t('annotate.raw_image')}
                </button>
            </div>
        </div>
    );
}
