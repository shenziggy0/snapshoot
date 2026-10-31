import { appWindow } from '@tauri-apps/api/window';
import { listen } from '@tauri-apps/api/event';
import { save } from '@tauri-apps/api/dialog';
import toast, { Toaster } from 'react-hot-toast';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api';
import React, { useEffect, useRef, useState } from 'react';

import { useToastStyle } from '../../hooks';
import { renderScene } from './render';
import Toolbar from './Toolbar';

const TOOLBAR_PX = 108;

function fitDisplay(width, height) {
    const dpr = window.devicePixelRatio || 1;
    const maxW = Math.max(1, window.innerWidth);
    const maxH = Math.max(1, window.innerHeight - TOOLBAR_PX);
    const cssW = width / dpr;
    const cssH = height / dpr;
    const fit = Math.min(maxW / cssW, maxH / cssH, 1);
    return {
        w: Math.max(1, Math.round(cssW * fit)),
        h: Math.max(1, Math.round(cssH * fit)),
    };
}

function decodeBase64(data) {
    const binary = atob(data);
    const bytes = new Uint8Array(binary.length);
    for (let i = 0; i < binary.length; i += 1) {
        bytes[i] = binary.charCodeAt(i);
    }
    return bytes;
}

function loadHtmlImage(src) {
    return new Promise((resolve, reject) => {
        const img = new Image();
        img.onload = () => resolve(img);
        img.onerror = () => reject(new Error('decode'));
        img.src = src;
    });
}

function eventPoint(event, canvas) {
    const rect = canvas.getBoundingClientRect();
    const scaleX = rect.width ? canvas.width / rect.width : 1;
    const scaleY = rect.height ? canvas.height / rect.height : 1;
    return {
        x: (event.clientX - rect.left) * scaleX,
        y: (event.clientY - rect.top) * scaleY,
    };
}

export default function Annotate() {
    const { t } = useTranslation();
    const toastStyle = useToastStyle();
    const canvasRef = useRef(null);
    const stageRef = useRef(null);
    const imageRef = useRef(null);
    const rawRef = useRef('');
    const historyRef = useRef({ shapes: [], redo: [] });
    const textDraftRef = useRef(null);

    const [tool, setTool] = useState('rect');
    const [color, setColor] = useState('#ef4444');
    const [strokeWidth, setStrokeWidth] = useState(4);
    const [includeAnnotations, setIncludeAnnotations] = useState(true);
    const [shapes, setShapes] = useState([]);
    const [redoCount, setRedoCount] = useState(0);
    const [draft, setDraft] = useState(null);
    const [image, setImage] = useState(null);
    const [box, setBox] = useState({ w: 1, h: 1 });
    const [textDraft, setTextDraft] = useState(null);
    const [loadError, setLoadError] = useState('');
    const blobUrlRef = useRef('');

    useEffect(() => {
        document.documentElement.style.borderRadius = '0px';
        document.documentElement.style.background = 'transparent';
        document.documentElement.style.height = '100%';
        document.body.style.background = '#141414';
        document.body.style.height = '100%';
        document.body.style.overflow = 'hidden';
        const root = document.getElementById('root');
        if (root) {
            root.style.height = '100%';
            root.style.background = '#141414';
        }
    }, []);

    const publish = (next) => {
        historyRef.current = next;
        setShapes(next.shapes);
        setRedoCount(next.redo.length);
    };

    const commit = (shape) => {
        const stored = { ...shape };
        if (shape.type === 'mosaic') {
            stored.block = Math.max(6, strokeWidth * 3);
        }
        if (shape.type === 'blur') {
            stored.radius = Math.max(2, Math.min(8, strokeWidth));
        }
        publish({ shapes: [...historyRef.current.shapes, stored], redo: [] });
    };

    const undo = () => {
        const { shapes: current, redo } = historyRef.current;
        if (!current.length) {
            return;
        }
        publish({
            shapes: current.slice(0, -1),
            redo: [...redo, current[current.length - 1]],
        });
    };

    const redo = () => {
        const { shapes: current, redo: stack } = historyRef.current;
        if (!stack.length) {
            return;
        }
        publish({
            shapes: [...current, stack[stack.length - 1]],
            redo: stack.slice(0, -1),
        });
    };

    const showEditor = async () => {
        await appWindow.show();
        await appWindow.setFocus();
        await appWindow.setAlwaysOnTop(true);
    };

    const loadImage = async () => {
        setLoadError('');
        if (blobUrlRef.current) {
            URL.revokeObjectURL(blobUrlRef.current);
            blobUrlRef.current = '';
        }
        let payload;
        try {
            payload = await invoke('annotate_source_image');
        } catch (error) {
            setLoadError(String(error));
            await showEditor();
            return;
        }
        const base64 = payload?.data || '';
        const width = payload?.width || 0;
        const height = payload?.height || 0;
        if (!base64 || width < 1 || height < 1) {
            setLoadError(t('annotate.empty'));
            await showEditor();
            return;
        }
        rawRef.current = base64;
        try {
            const bytes = decodeBase64(base64);
            const blobUrl = URL.createObjectURL(new Blob([bytes], { type: 'image/png' }));
            blobUrlRef.current = blobUrl;
            let next;
            try {
                next = await loadHtmlImage(blobUrl);
            } catch (blobError) {
                URL.revokeObjectURL(blobUrl);
                blobUrlRef.current = '';
                next = await loadHtmlImage(`data:image/png;base64,${base64}`);
            }
            if (!next.naturalWidth || !next.naturalHeight) {
                throw new Error('empty bitmap');
            }
            imageRef.current = next;
            setImage(next);
            setBox(fitDisplay(next.naturalWidth, next.naturalHeight));
            publish({ shapes: [], redo: [] });
            setDraft(null);
            setTextDraft(null);
            await showEditor();
            setBox(fitDisplay(next.naturalWidth, next.naturalHeight));
        } catch (error) {
            setImage(null);
            setLoadError(t('annotate.load_failed'));
            await showEditor();
        }
    };

    useEffect(() => {
        void loadImage();
        const unlisten = listen('annotate-reload', () => {
            void loadImage();
        });
        return () => {
            unlisten.then((fn) => fn());
        };
    }, []);

    useEffect(() => {
        const measure = () => {
            const current = imageRef.current;
            if (!current) {
                return;
            }
            setBox(fitDisplay(current.naturalWidth || current.width, current.naturalHeight || current.height));
        };
        measure();
        window.addEventListener('resize', measure);
        return () => window.removeEventListener('resize', measure);
    }, [image]);

    useEffect(() => {
        const canvas = canvasRef.current;
        if (!canvas || !image) {
            return;
        }
        renderScene(canvas.getContext('2d'), image, shapes, draft);
    }, [image, shapes, draft]);

    const payload = () => {
        if (!includeAnnotations) {
            return rawRef.current;
        }
        const canvas = canvasRef.current;
        if (!canvas) {
            return rawRef.current;
        }
        return canvas.toDataURL('image/png').split(',')[1];
    };

    const onCopy = async () => {
        try {
            await invoke('copy_png_base64', { data: payload() });
            toast.success(t('annotate.copied'), { style: toastStyle });
        } catch (error) {
            toast.error(String(error), { style: toastStyle });
        }
    };

    const onSave = async () => {
        try {
            const path = await save({
                defaultPath: 'snapshoot-screenshot.png',
                filters: [{ name: 'PNG', extensions: ['png'] }],
            });
            if (!path) {
                return;
            }
            await invoke('save_png_base64', { path, data: payload() });
            toast.success(t('annotate.saved'), { style: toastStyle });
        } catch (error) {
            toast.error(String(error), { style: toastStyle });
        }
    };

    const onPin = async () => {
        try {
            await invoke('pin_png_base64', { data: payload() });
            await appWindow.close();
        } catch (error) {
            toast.error(String(error), { style: toastStyle });
        }
    };

    const onOcr = async () => {
        try {
            await invoke('open_ocr_from_png_base64', { data: payload() });
            await appWindow.close();
        } catch (error) {
            toast.error(String(error), { style: toastStyle });
        }
    };

    const onTranslate = async () => {
        try {
            await invoke('open_translate_from_png_base64', { data: payload() });
            await appWindow.close();
        } catch (error) {
            toast.error(String(error), { style: toastStyle });
        }
    };

    const finishText = () => {
        const draftValue = textDraftRef.current;
        if (!draftValue) {
            return;
        }
        textDraftRef.current = null;
        setTextDraft(null);
        const value = (draftValue.value || '').trim();
        if (!value) {
            return;
        }
        commit({
            type: 'text',
            x: draftValue.x,
            y: draftValue.y,
            text: value,
            color: draftValue.color,
            fontSize: Math.max(22, draftValue.strokeWidth * 7),
            strokeWidth: draftValue.strokeWidth,
        });
    };

    useEffect(() => {
        const onKey = (event) => {
            const tag = document.activeElement?.tagName;
            if (tag === 'INPUT' || tag === 'TEXTAREA') {
                return;
            }
            const meta = event.ctrlKey || event.metaKey;
            const key = event.key.toLowerCase();
            if (meta && key === 'z') {
                event.preventDefault();
                event.stopPropagation();
                if (event.shiftKey) {
                    redo();
                } else {
                    undo();
                }
            } else if (meta && key === 'y') {
                event.preventDefault();
                event.stopPropagation();
                redo();
            } else if (meta && key === 's') {
                event.preventDefault();
                event.stopPropagation();
                void onSave();
            }
        };
        window.addEventListener('keydown', onKey, true);
        return () => window.removeEventListener('keydown', onKey, true);
    });

    const onPointerDown = (event) => {
        if (event.button !== 0 || !canvasRef.current || !image) {
            return;
        }
        if (textDraftRef.current) {
            finishText();
        }
        const point = eventPoint(event, canvasRef.current);
        if (tool === 'number') {
            const n =
                historyRef.current.shapes.reduce(
                    (max, shape) => (shape.type === 'number' ? Math.max(max, shape.n) : max),
                    0
                ) + 1;
            commit({ type: 'number', x: point.x, y: point.y, n, color, strokeWidth });
            return;
        }
        if (tool === 'text') {
            const rect = canvasRef.current.getBoundingClientRect();
            const wrapper = canvasRef.current.parentElement.getBoundingClientRect();
            const nextDraft = {
                x: point.x,
                y: point.y,
                cssX: event.clientX - wrapper.left,
                cssY: event.clientY - wrapper.top,
                scale: rect.width ? rect.width / canvasRef.current.width : 1,
                value: '',
                color,
                strokeWidth,
            };
            textDraftRef.current = nextDraft;
            setTextDraft(nextDraft);
            return;
        }
        if (tool === 'pen' || tool === 'highlighter') {
            setDraft({ type: tool, points: [point], color, strokeWidth });
            return;
        }
        setDraft({
            type: tool,
            x: point.x,
            y: point.y,
            w: 0,
            h: 0,
            x2: point.x,
            y2: point.y,
            color,
            strokeWidth,
        });
    };

    const onPointerMove = (event) => {
        if (!draft || !canvasRef.current) {
            return;
        }
        const point = eventPoint(event, canvasRef.current);
        if (draft.type === 'pen' || draft.type === 'highlighter') {
            setDraft({ ...draft, points: [...draft.points, point] });
            return;
        }
        if (draft.type === 'arrow') {
            setDraft({ ...draft, x2: point.x, y2: point.y });
            return;
        }
        setDraft({ ...draft, w: point.x - draft.x, h: point.y - draft.y });
    };

    const onPointerUp = () => {
        if (!draft) {
            return;
        }
        const shape = draft;
        setDraft(null);
        if (shape.type === 'pen' || shape.type === 'highlighter') {
            if (shape.points.length) {
                commit(shape);
            }
            return;
        }
        if (shape.type === 'arrow') {
            if (Math.hypot(shape.x2 - shape.x, shape.y2 - shape.y) >= 2) {
                commit(shape);
            }
            return;
        }
        if (Math.abs(shape.w) >= 2 || Math.abs(shape.h) >= 2) {
            commit(shape);
        }
    };

    return (
        <div className='fixed inset-0 flex flex-col bg-neutral-900 text-white'>
            <Toaster />
            <div
                ref={stageRef}
                className='relative flex min-h-0 flex-1 items-center justify-center overflow-hidden'
                onMouseDown={(event) => {
                    if (event.target === event.currentTarget) {
                        void appWindow.startDragging();
                    }
                }}
            >
                {loadError ? (
                    <div className='max-w-[90%] px-4 text-center text-sm text-red-200'>{loadError}</div>
                ) : (
                    <div
                        className='relative'
                        style={{ width: box.w, height: box.h }}
                    >
                        <canvas
                            ref={canvasRef}
                            width={image?.naturalWidth || image?.width || 1}
                            height={image?.naturalHeight || image?.height || 1}
                            className='cursor-crosshair'
                            style={{ width: box.w, height: box.h, display: 'block' }}
                            onMouseDown={onPointerDown}
                            onMouseMove={onPointerMove}
                            onMouseUp={onPointerUp}
                            onMouseLeave={onPointerUp}
                            onContextMenu={(event) => event.preventDefault()}
                        />
                        {textDraft && (
                            <input
                                autoFocus
                                value={textDraft.value}
                                placeholder={t('annotate.text_placeholder')}
                                onChange={(event) => {
                                    const nextDraft = { ...textDraft, value: event.target.value };
                                    textDraftRef.current = nextDraft;
                                    setTextDraft(nextDraft);
                                }}
                                onBlur={() => finishText()}
                                onKeyDown={(event) => {
                                    event.stopPropagation();
                                    if (event.key === 'Enter') {
                                        event.preventDefault();
                                        const nextDraft = { ...textDraft, value: event.target.value };
                                        textDraftRef.current = nextDraft;
                                        finishText();
                                    } else if (event.key === 'Escape') {
                                        event.preventDefault();
                                        textDraftRef.current = null;
                                        setTextDraft(null);
                                    }
                                }}
                                className='absolute z-20 min-w-[80px] border border-white/70 bg-black/40 px-1 text-white outline-none'
                                style={{
                                    left: textDraft.cssX,
                                    top: textDraft.cssY,
                                    color: textDraft.color,
                                    fontSize: Math.max(14, Math.max(22, textDraft.strokeWidth * 7) * textDraft.scale),
                                }}
                            />
                        )}
                    </div>
                )}
            </div>
            <div
                className='flex items-start justify-center px-1 pb-2'
                style={{ height: TOOLBAR_PX }}
            >
                <Toolbar
                    tool={tool}
                    setTool={setTool}
                    color={color}
                    setColor={setColor}
                    strokeWidth={strokeWidth}
                    setStrokeWidth={setStrokeWidth}
                    includeAnnotations={includeAnnotations}
                    setIncludeAnnotations={setIncludeAnnotations}
                    canUndo={shapes.length > 0}
                    canRedo={redoCount > 0}
                    onUndo={undo}
                    onRedo={redo}
                    onCopy={onCopy}
                    onSave={onSave}
                    onPin={onPin}
                    onOcr={onOcr}
                    onTranslate={onTranslate}
                    onClose={() => {
                        void appWindow.close();
                    }}
                    disabled={!image}
                />
            </div>
        </div>
    );
}
