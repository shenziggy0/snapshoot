import { appWindow } from '@tauri-apps/api/window';
import { listen } from '@tauri-apps/api/event';
import { save } from '@tauri-apps/api/dialog';
import { writeText } from '@tauri-apps/api/clipboard';
import { currentMonitor } from '@tauri-apps/api/window';
import toast, { Toaster } from 'react-hot-toast';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api';
import React, { useEffect, useRef, useState } from 'react';

import { drawShape } from '../Annotate/render';
import { useToastStyle } from '../../hooks';
import { useSystemDark } from '../useSystemDark';
import CaptureToolbar from './Toolbar';

const HANDLES = ['nw', 'n', 'ne', 'e', 'se', 's', 'sw', 'w'];

function clamp(value, min, max) {
    return Math.min(max, Math.max(min, value));
}

function normalize(rect) {
    const x = rect.w < 0 ? rect.x + rect.w : rect.x;
    const y = rect.h < 0 ? rect.y + rect.h : rect.y;
    return { x, y, w: Math.abs(rect.w), h: Math.abs(rect.h) };
}

function loadHtmlImage(src) {
    return new Promise((resolve, reject) => {
        const image = new Image();
        image.onload = () => resolve(image);
        image.onerror = () => reject(new Error('decode'));
        image.src = src;
    });
}

function base64ToObjectUrl(data) {
    const binary = atob(data);
    const bytes = new Uint8Array(binary.length);
    for (let index = 0; index < binary.length; index += 1) {
        bytes[index] = binary.charCodeAt(index);
    }
    return URL.createObjectURL(new Blob([bytes], { type: 'image/png' }));
}

function handleStyle(handle, rect) {
    const size = 8;
    const x = { w: rect.x, e: rect.x + rect.w, n: rect.x + rect.w / 2, s: rect.x + rect.w / 2 }[
        handle.includes('w') ? 'w' : handle.includes('e') ? 'e' : handle.includes('n') || handle.includes('s') ? (handle.includes('n') ? 'n' : 's') : 'n'
    ];
    let left = rect.x + rect.w / 2;
    let top = rect.y + rect.h / 2;
    if (handle.includes('w')) left = rect.x;
    if (handle.includes('e')) left = rect.x + rect.w;
    if (handle.includes('n')) top = rect.y;
    if (handle.includes('s')) top = rect.y + rect.h;
    if (handle === 'n' || handle === 's') left = rect.x + rect.w / 2;
    if (handle === 'w' || handle === 'e') top = rect.y + rect.h / 2;
    return {
        left: left - size / 2,
        top: top - size / 2,
        width: size,
        height: size,
        cursor: `${handle}-resize`,
    };
}

function resizeRect(origin, handle, dx, dy, bounds) {
    let { x, y, w, h } = origin;
    if (handle.includes('e')) w += dx;
    if (handle.includes('s')) h += dy;
    if (handle.includes('w')) {
        x += dx;
        w -= dx;
    }
    if (handle.includes('n')) {
        y += dy;
        h -= dy;
    }
    let next = normalize({ x, y, w, h });
    next.w = Math.max(8, next.w);
    next.h = Math.max(8, next.h);
    next.x = clamp(next.x, bounds.x, bounds.x + bounds.w - next.w);
    next.y = clamp(next.y, bounds.y, bounds.y + bounds.h - next.h);
    return next;
}

export default function Screenshot() {
    const { t } = useTranslation();
    const toastStyle = useToastStyle();
    const dark = useSystemDark();
    const canvasRef = useRef(null);
    const sampleRef = useRef(null);
    const loupeRef = useRef(null);
    const imageRef = useRef(null);
    const selectionRef = useRef(null);
    const dragRef = useRef(null);
    const hoverRef = useRef(null);
    const draftRef = useRef(null);
    const shapesRef = useRef([]);
    const redoRef = useRef([]);
    const intentRef = useRef('annotate');
    const layoutRef = useRef(null);
    const framesRef = useRef([]);
    const originRef = useRef({ x: 0, y: 0 });
    const blobRef = useRef('');
    const colorRef = useRef('#e11d48');
    const strokeRef = useRef(4);
    const fontRef = useRef(24);
    const toolRef = useRef('');

    const [image, setImage] = useState(null);
    const [layout, setLayout] = useState(null);
    const [selection, setSelection] = useState(null);
    const [hover, setHover] = useState(null);
    const [tool, setTool] = useState('');
    const [shapes, setShapes] = useState([]);
    const [redoCount, setRedoCount] = useState(0);
    const [draft, setDraft] = useState(null);
    const [color, setColor] = useState('#e11d48');
    const [strokeWidth, setStrokeWidth] = useState(4);
    const [fontSize, setFontSize] = useState(24);
    const [textDraft, setTextDraft] = useState(null);
    const [loadError, setLoadError] = useState('');
    const [cursor, setCursor] = useState({ x: 0, y: 0 });
    const [loupe, setLoupe] = useState(null);

    selectionRef.current = selection;
    hoverRef.current = hover;
    draftRef.current = draft;
    shapesRef.current = shapes;
    colorRef.current = color;
    strokeRef.current = strokeWidth;
    fontRef.current = fontSize;
    toolRef.current = tool;
    layoutRef.current = layout;

    useEffect(() => {
        document.documentElement.style.borderRadius = '0px';
        document.documentElement.style.background = '#000';
        document.body.style.background = '#000';
        document.body.style.overflow = 'hidden';
        const root = document.getElementById('root');
        if (root) {
            root.style.height = '100%';
            root.style.background = '#000';
        }
    }, []);

    const publish = (nextShapes, nextRedo) => {
        shapesRef.current = nextShapes;
        redoRef.current = nextRedo;
        setShapes(nextShapes);
        setRedoCount(nextRedo.length);
    };

    const commitShape = (shape) => {
        publish([...shapesRef.current, shape], []);
    };

    const paint = () => {
        const canvas = canvasRef.current;
        const current = imageRef.current;
        if (!canvas || !current) {
            return;
        }
        const ctx = canvas.getContext('2d');
        ctx.clearRect(0, 0, canvas.width, canvas.height);
        ctx.drawImage(current, 0, 0);
        for (const shape of shapesRef.current) {
            drawShape(ctx, shape, false);
        }
        if (draft) {
            drawShape(ctx, draft, true);
        }
    };

    useEffect(() => {
        paint();
    });

    const boot = async () => {
        setLoadError('');
        setSelection(null);
        setHover(null);
        setTool('');
        publish([], []);
        setDraft(null);
        if (blobRef.current) {
            URL.revokeObjectURL(blobRef.current);
            blobRef.current = '';
        }
        try {
            const intent = await invoke('screenshot_intent');
            intentRef.current = intent || 'annotate';
            if (intentRef.current !== 'edit') {
                const monitor = await currentMonitor();
                await invoke('screenshot', { x: monitor.position.x, y: monitor.position.y });
                const inner = await appWindow.innerPosition();
                originRef.current = { x: inner.x, y: inner.y };
                framesRef.current = await invoke('list_capture_windows');
            } else {
                framesRef.current = [];
            }
            const captured = await invoke('read_capture_image');
            const objectUrl = base64ToObjectUrl(captured.data);
            blobRef.current = objectUrl;
            const next = await loadHtmlImage(objectUrl);
            imageRef.current = next;
            await appWindow.show();
            await appWindow.setFocus();
            await appWindow.setAlwaysOnTop(true);
            const placed = placeImage(next, intentRef.current);
            layoutRef.current = placed.layout;
            setLayout(placed.layout);
            setImage(next);
            if (placed.selection) {
                selectionRef.current = placed.selection;
                setSelection(placed.selection);
            }
        } catch (error) {
            setLoadError(String(error));
            await appWindow.show();
        }
    };

    useEffect(() => {
        void boot();
        const unlisten = listen('screenshot-reload', () => {
            void boot();
        });
        return () => {
            unlisten.then((fn) => fn());
            if (blobRef.current) {
                URL.revokeObjectURL(blobRef.current);
            }
        };
    }, []);

    const imagePoint = (clientX, clientY) => {
        const canvas = canvasRef.current;
        if (!canvas) {
            return { x: 0, y: 0 };
        }
        const bounds = canvas.getBoundingClientRect();
        const scaleX = bounds.width ? canvas.width / bounds.width : 1;
        const scaleY = bounds.height ? canvas.height / bounds.height : 1;
        return {
            x: (clientX - bounds.left) * scaleX,
            y: (clientY - bounds.top) * scaleY,
        };
    };

    const updateLoupe = (clientX, clientY) => {
        const current = imageRef.current;
        const canvas = canvasRef.current;
        if (!current || !canvas || selectionRef.current) {
            setLoupe(null);
            return;
        }
        const point = imagePoint(clientX, clientY);
        const sample = sampleRef.current?.getContext('2d', { willReadFrequently: true });
        if (!sample) {
            return;
        }
        const ix = clamp(Math.round(point.x), 0, current.width - 1);
        const iy = clamp(Math.round(point.y), 0, current.height - 1);
        sample.drawImage(current, ix, iy, 1, 1, 0, 0, 1, 1);
        const pixel = sample.getImageData(0, 0, 1, 1).data;
        const hex = `#${[pixel[0], pixel[1], pixel[2]].map((part) => part.toString(16).padStart(2, '0')).join('')}`;
        const loupeCanvas = loupeRef.current;
        if (loupeCanvas) {
            const zoom = 8;
            const ctx = loupeCanvas.getContext('2d');
            const src = loupeCanvas.width / zoom;
            ctx.imageSmoothingEnabled = false;
            ctx.drawImage(current, point.x - src / 2, point.y - src / 2, src, src, 0, 0, loupeCanvas.width, loupeCanvas.height);
            ctx.strokeStyle = '#ffffff';
            ctx.lineWidth = 1;
            ctx.beginPath();
            ctx.moveTo(loupeCanvas.width / 2, 0);
            ctx.lineTo(loupeCanvas.width / 2, loupeCanvas.height);
            ctx.moveTo(0, loupeCanvas.height / 2);
            ctx.lineTo(loupeCanvas.width, loupeCanvas.height / 2);
            ctx.stroke();
        }
        const box = 132;
        let left = clientX + 24;
        let top = clientY + 24;
        if (left + box > window.innerWidth) left = clientX - box - 16;
        if (top + box + 36 > window.innerHeight) top = clientY - box - 40;
        setLoupe({ left, top, hex, x: ix, y: iy });
        setCursor({ x: clientX, y: clientY });
    };

    const hoverAt = (clientX, clientY) => {
        if (intentRef.current === 'edit' || selectionRef.current || !layoutRef.current) {
            setHover(null);
            return;
        }
        const canvas = canvasRef.current;
        if (!canvas) {
            return;
        }
        const bounds = canvas.getBoundingClientRect();
        const scaleX = bounds.width ? canvas.width / bounds.width : 1;
        const scaleY = bounds.height ? canvas.height / bounds.height : 1;
        const px = originRef.current.x + (clientX - bounds.left) * scaleX;
        const py = originRef.current.y + (clientY - bounds.top) * scaleY;
        let best = null;
        let bestArea = Infinity;
        for (const frame of framesRef.current) {
            if (px < frame.x || py < frame.y || px > frame.x + frame.width || py > frame.y + frame.height) {
                continue;
            }
            const area = frame.width * frame.height;
            if (area < bestArea) {
                best = frame;
                bestArea = area;
            }
        }
        if (!best) {
            setHover(null);
            return;
        }
        setHover({
            x: bounds.left + (best.x - originRef.current.x) / scaleX,
            y: bounds.top + (best.y - originRef.current.y) / scaleY,
            w: best.width / scaleX,
            h: best.height / scaleY,
        });
    };

    useEffect(() => {
        const onMove = (event) => {
            const drag = dragRef.current;
            if (!drag) {
                hoverAt(event.clientX, event.clientY);
                updateLoupe(event.clientX, event.clientY);
                return;
            }
            const dx = event.clientX - drag.x;
            const dy = event.clientY - drag.y;
            if (drag.kind === 'new') {
                const next = normalize({
                    x: drag.origin.x,
                    y: drag.origin.y,
                    w: dx,
                    h: dy,
                });
                selectionRef.current = next;
                setSelection(next);
                updateLoupe(event.clientX, event.clientY);
                return;
            }
            const bounds = layoutRef.current || { x: 0, y: 0, w: window.innerWidth, h: window.innerHeight };
            if (drag.kind === 'move') {
                const next = {
                    ...drag.origin,
                    x: clamp(drag.origin.x + dx, bounds.x ?? bounds.left ?? 0, (bounds.x ?? 0) + (bounds.w ?? bounds.width) - drag.origin.w),
                    y: clamp(drag.origin.y + dy, bounds.y ?? bounds.top ?? 0, (bounds.y ?? 0) + (bounds.h ?? bounds.height) - drag.origin.h),
                };
                const limit = imageBounds();
                next.x = clamp(next.x, limit.x, limit.x + limit.w - next.w);
                next.y = clamp(next.y, limit.y, limit.y + limit.h - next.h);
                selectionRef.current = next;
                setSelection(next);
                return;
            }
            if (drag.kind === 'resize') {
                const next = resizeRect(drag.origin, drag.handle, dx, dy, imageBounds());
                selectionRef.current = next;
                setSelection(next);
            }
        };
        const onUp = (event) => {
            const drag = dragRef.current;
            if (!drag) {
                return;
            }
            dragRef.current = null;
            if (drag.kind === 'draw') {
                const shape = draftRef.current;
                draftRef.current = null;
                setDraft(null);
                if (!shape) {
                    return;
                }
                if ((shape.type === 'pen' || shape.type === 'highlighter') && shape.points?.length) {
                    commitShape(shape);
                } else if ((shape.type === 'line' || shape.type === 'arrow') && Math.hypot(shape.x2 - shape.x, shape.y2 - shape.y) >= 2) {
                    commitShape(shape);
                } else if (Math.abs(shape.w || 0) >= 2 || Math.abs(shape.h || 0) >= 2) {
                    commitShape(shape);
                }
                return;
            }
            if (drag.kind !== 'new') {
                return;
            }
            const moved = Math.hypot(event.clientX - drag.x, event.clientY - drag.y);
            let next = selectionRef.current;
            if (moved < 4 && hoverRef.current) {
                next = hoverRef.current;
                selectionRef.current = next;
                setSelection(next);
            }
            if (!next || next.w < 4 || next.h < 4) {
                setSelection(null);
                return;
            }
            const intent = intentRef.current;
            if (intent === 'ocr' || intent === 'translate') {
                void finish(intent, next);
            }
        };
        window.addEventListener('mousemove', onMove);
        window.addEventListener('mouseup', onUp);
        return () => {
            window.removeEventListener('mousemove', onMove);
            window.removeEventListener('mouseup', onUp);
        };
    });

    const imageBounds = () => {
        const current = layoutRef.current;
        if (!current) {
            return { x: 0, y: 0, w: window.innerWidth, h: window.innerHeight };
        }
        return { x: current.left, y: current.top, w: current.width, h: current.height };
    };

    const exportBase64 = (rect) => {
        const canvas = canvasRef.current;
        if (!canvas || !rect) {
            return '';
        }
        const bounds = canvas.getBoundingClientRect();
        const scaleX = bounds.width ? canvas.width / bounds.width : 1;
        const scaleY = bounds.height ? canvas.height / bounds.height : 1;
        const sx = clamp((rect.x - bounds.left) * scaleX, 0, canvas.width - 1);
        const sy = clamp((rect.y - bounds.top) * scaleY, 0, canvas.height - 1);
        const sw = clamp(Math.round(rect.w * scaleX), 1, canvas.width - sx);
        const sh = clamp(Math.round(rect.h * scaleY), 1, canvas.height - sy);
        const out = document.createElement('canvas');
        out.width = sw;
        out.height = sh;
        out.getContext('2d').drawImage(canvas, sx, sy, sw, sh, 0, 0, sw, sh);
        return out.toDataURL('image/png').split(',')[1];
    };

    const finish = async (action, rect) => {
        const data = exportBase64(rect || selectionRef.current);
        if (!data) {
            return;
        }
        try {
            if (action === 'copy') {
                await invoke('copy_png_base64', { data });
            } else if (action === 'save') {
                const path = await save({
                    defaultPath: 'snapshoot-screenshot.png',
                    filters: [{ name: 'PNG', extensions: ['png'] }],
                });
                if (!path) {
                    return;
                }
                await invoke('save_png_base64', { path, data });
                toast.success(t('annotate.saved'), { style: toastStyle });
                return;
            } else if (action === 'pin') {
                await invoke('pin_png_base64', { data });
            } else if (action === 'ocr') {
                await invoke('open_ocr_from_png_base64', { data });
            } else if (action === 'translate') {
                await invoke('open_translate_from_png_base64', { data });
            }
            await appWindow.close();
        } catch (error) {
            toast.error(String(error), { style: toastStyle });
        }
    };

    useEffect(() => {
        const onKey = async (event) => {
            if (document.activeElement?.tagName === 'INPUT') {
                return;
            }
            if (event.key === 'Enter' && selectionRef.current) {
                event.preventDefault();
                await finish('copy');
            } else if (event.key.toLowerCase() === 'c' && loupe && !event.metaKey && !event.ctrlKey) {
                event.preventDefault();
                await writeText(loupe.hex);
                toast.success(loupe.hex, { style: toastStyle });
            } else if (event.key.startsWith('Arrow') && selectionRef.current) {
                event.preventDefault();
                const step = event.shiftKey ? 10 : 1;
                const delta = { ArrowLeft: [-step, 0], ArrowRight: [step, 0], ArrowUp: [0, -step], ArrowDown: [0, step] }[
                    event.key
                ];
                const limit = imageBounds();
                const current = selectionRef.current;
                setSelection({
                    ...current,
                    x: clamp(current.x + delta[0], limit.x, limit.x + limit.w - current.w),
                    y: clamp(current.y + delta[1], limit.y, limit.y + limit.h - current.h),
                });
            } else if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'z') {
                event.preventDefault();
                event.stopPropagation();
                if (event.shiftKey) {
                    redo();
                } else {
                    undo();
                }
            }
        };
        window.addEventListener('keydown', onKey, true);
        return () => window.removeEventListener('keydown', onKey, true);
    });

    const undo = () => {
        const current = shapesRef.current;
        if (!current.length) {
            return;
        }
        publish(current.slice(0, -1), [...redoRef.current, current[current.length - 1]]);
    };
    const redo = () => {
        const stack = redoRef.current;
        if (!stack.length) {
            return;
        }
        publish([...shapesRef.current, stack[stack.length - 1]], stack.slice(0, -1));
    };

    const onBackgroundDown = (event) => {
        if (event.button !== 0 || selectionRef.current) {
            return;
        }
        dragRef.current = { kind: 'new', x: event.clientX, y: event.clientY, origin: { x: event.clientX, y: event.clientY } };
        setSelection({ x: event.clientX, y: event.clientY, w: 0, h: 0 });
    };

    const startDraw = (event) => {
        if (event.button !== 0) {
            return;
        }
        event.stopPropagation();
        const active = toolRef.current;
        const point = imagePoint(event.clientX, event.clientY);
        if (!active) {
            dragRef.current = {
                kind: 'move',
                x: event.clientX,
                y: event.clientY,
                origin: { ...selectionRef.current },
            };
            return;
        }
        if (active === 'number') {
            const n = shapesRef.current.reduce((max, shape) => (shape.type === 'number' ? Math.max(max, shape.n) : max), 0) + 1;
            commitShape({ type: 'number', x: point.x, y: point.y, n, color: colorRef.current, strokeWidth: strokeRef.current });
            return;
        }
        if (active === 'text') {
            setTextDraft({
                x: point.x,
                y: point.y,
                cssX: event.clientX,
                cssY: event.clientY,
                value: '',
            });
            return;
        }
        if (active === 'pen' || active === 'highlighter') {
            setDraft({ type: active, points: [point], color: colorRef.current, strokeWidth: strokeRef.current });
            dragRef.current = { kind: 'draw' };
            return;
        }
        setDraft({
            type: active,
            x: point.x,
            y: point.y,
            w: 0,
            h: 0,
            x2: point.x,
            y2: point.y,
            color: colorRef.current,
            strokeWidth: strokeRef.current,
        });
        dragRef.current = { kind: 'draw', x: point.x, y: point.y };
    };

        const onDrawMove = (event) => {
        const drag = dragRef.current;
        const currentDraft = draftRef.current;
        if (!drag || drag.kind !== 'draw' || !currentDraft) {
            return;
        }
        const point = imagePoint(event.clientX, event.clientY);
        let next = currentDraft;
        if (currentDraft.type === 'pen' || currentDraft.type === 'highlighter') {
            next = { ...currentDraft, points: [...currentDraft.points, point] };
        } else if (currentDraft.type === 'line' || currentDraft.type === 'arrow') {
            next = { ...currentDraft, x2: point.x, y2: point.y };
        } else {
            next = { ...currentDraft, w: point.x - currentDraft.x, h: point.y - currentDraft.y };
        }
        draftRef.current = next;
        setDraft(next);
    };

    const onDrawUp = () => {
        const shape = draftRef.current;
        if (!shape) {
            return;
        }
        draftRef.current = null;
        setDraft(null);
        if (dragRef.current?.kind === 'draw') {
            dragRef.current = null;
        }
        if (shape.type === 'pen' || shape.type === 'highlighter') {
            if (shape.points.length) {
                commitShape(shape);
            }
            return;
        }
        if (shape.type === 'line' || shape.type === 'arrow') {
            if (Math.hypot(shape.x2 - shape.x, shape.y2 - shape.y) >= 2) {
                commitShape(shape);
            }
            return;
        }
        if (Math.abs(shape.w) >= 2 || Math.abs(shape.h) >= 2) {
            commitShape(shape);
        }
    };

    const scaleLabel = () => {
        const canvas = canvasRef.current;
        if (!canvas || !selection) {
            return '';
        }
        const bounds = canvas.getBoundingClientRect();
        const scaleX = bounds.width ? canvas.width / bounds.width : 1;
        const scaleY = bounds.height ? canvas.height / bounds.height : 1;
        return `${Math.max(1, Math.round(selection.w * scaleX))} × ${Math.max(1, Math.round(selection.h * scaleY))}`;
    };

    const toolbarTop = () => {
        if (!selection) {
            return 0;
        }
        const below = selection.y + selection.h + 10;
        if (below + 78 < window.innerHeight) {
            return below;
        }
        return Math.max(8, selection.y - 78);
    };

    const accent = '#1E63F0';
    const editing = Boolean(selection) && intentRef.current !== 'ocr' && intentRef.current !== 'translate';

    return (
        <div className='fixed inset-0 overflow-hidden bg-black text-white'>
            <Toaster />
            <canvas
                ref={sampleRef}
                width={1}
                height={1}
                className='hidden'
            />
            {loadError ? (
                <div className='absolute inset-0 z-50 flex items-center justify-center p-8 text-center text-sm text-red-200'>
                    {loadError}
                </div>
            ) : null}
            <canvas
                ref={canvasRef}
                width={image?.naturalWidth || image?.width || 1}
                height={image?.naturalHeight || image?.height || 1}
                className='absolute'
                style={
                    layout
                        ? { left: layout.left, top: layout.top, width: layout.width, height: layout.height }
                        : { inset: 0, width: '100%', height: '100%' }
                }
            />
            {layout && (
                <>
                    <div
                        className='pointer-events-none absolute bg-black/45'
                        style={{ left: 0, top: 0, right: 0, height: selection ? selection.y : hover ? hover.y : '100%' }}
                    />
                    {(selection || hover) && (
                        <>
                            <div
                                className='pointer-events-none absolute bg-black/45'
                                style={{
                                    left: 0,
                                    top: (selection || hover).y,
                                    width: (selection || hover).x,
                                    height: (selection || hover).h,
                                }}
                            />
                            <div
                                className='pointer-events-none absolute bg-black/45'
                                style={{
                                    left: (selection || hover).x + (selection || hover).w,
                                    top: (selection || hover).y,
                                    right: 0,
                                    height: (selection || hover).h,
                                }}
                            />
                            <div
                                className='pointer-events-none absolute bg-black/45'
                                style={{
                                    left: 0,
                                    top: (selection || hover).y + (selection || hover).h,
                                    right: 0,
                                    bottom: 0,
                                }}
                            />
                        </>
                    )}
                </>
            )}
            {!selection && (
                <div
                    className='absolute inset-0 z-10 cursor-crosshair'
                    onMouseDown={onBackgroundDown}
                />
            )}
            {(hover && !selection) || selection ? (
                <div
                    className='pointer-events-none absolute z-20 border-2'
                    style={{
                        left: (selection || hover).x,
                        top: (selection || hover).y,
                        width: (selection || hover).w,
                        height: (selection || hover).h,
                        borderColor: accent,
                    }}
                />
            ) : null}
            {selection && (
                <div
                    className='pointer-events-none absolute z-20 rounded bg-black/70 px-1.5 py-0.5 text-[11px]'
                    style={{ left: selection.x, top: Math.max(4, selection.y - 22) }}
                >
                    {scaleLabel()}
                </div>
            )}
            {selection && editing && (
                <div
                    className='absolute z-30'
                    style={{ left: selection.x, top: selection.y, width: selection.w, height: selection.h }}
                    onMouseDown={startDraw}
                    onMouseMove={onDrawMove}
                    onMouseUp={onDrawUp}
                >
                    {HANDLES.map((handle) => (
                        <div
                            key={handle}
                            className='absolute z-40 border border-sky-500 bg-white'
                            style={handleStyle(handle, { x: 0, y: 0, w: selection.w, h: selection.h })}
                            onMouseDown={(event) => {
                                event.stopPropagation();
                                dragRef.current = {
                                    kind: 'resize',
                                    handle,
                                    x: event.clientX,
                                    y: event.clientY,
                                    origin: { ...selection },
                                };
                            }}
                        />
                    ))}
                    {textDraft && (
                        <input
                            autoFocus
                            value={textDraft.value}
                            className='absolute z-50 min-w-[4rem] border border-white/80 bg-black/40 px-1 text-white outline-none'
                            style={{
                                left: textDraft.cssX - selection.x,
                                top: textDraft.cssY - selection.y,
                                color,
                                fontSize,
                            }}
                            onChange={(event) => setTextDraft({ ...textDraft, value: event.target.value })}
                            onMouseDown={(event) => event.stopPropagation()}
                            onKeyDown={(event) => {
                                event.stopPropagation();
                                if (event.key === 'Enter') {
                                    event.preventDefault();
                                    const value = event.target.value.trim();
                                    if (value) {
                                        const canvas = canvasRef.current;
                                        const bounds = canvas.getBoundingClientRect();
                                        const scale = bounds.width ? canvas.width / bounds.width : 1;
                                        commitShape({
                                            type: 'text',
                                            x: textDraft.x,
                                            y: textDraft.y,
                                            text: value,
                                            color,
                                            fontSize: fontSize * scale,
                                            strokeWidth,
                                        });
                                    }
                                    setTextDraft(null);
                                } else if (event.key === 'Escape') {
                                    event.preventDefault();
                                    setTextDraft(null);
                                }
                            }}
                        />
                    )}
                </div>
            )}
            {selection && editing && (
                <div
                    className='absolute z-40'
                    style={{
                        left: clamp(selection.x + selection.w / 2, 180, window.innerWidth - 180),
                        top: toolbarTop(),
                        transform: 'translateX(-50%)',
                    }}
                >
                    <CaptureToolbar
                        dark={dark}
                        tool={tool}
                        setTool={setTool}
                        color={color}
                        setColor={setColor}
                        strokeWidth={strokeWidth}
                        setStrokeWidth={setStrokeWidth}
                        fontSize={fontSize}
                        setFontSize={setFontSize}
                        canUndo={shapes.length > 0}
                        canRedo={redoCount > 0}
                        onUndo={undo}
                        onRedo={redo}
                        onOcr={() => finish('ocr')}
                        onTranslate={() => finish('translate')}
                        onPin={() => finish('pin')}
                        onSave={() => finish('save')}
                        onCopy={() => finish('copy')}
                        onClose={() => appWindow.close()}
                    />
                </div>
            )}
            {loupe && !selection && (
                <div
                    className='pointer-events-none absolute z-50 overflow-hidden rounded-md border border-white/80 bg-black shadow-lg'
                    style={{ left: loupe.left, top: loupe.top, width: 132 }}
                >
                    <canvas
                        ref={loupeRef}
                        width={120}
                        height={120}
                        className='h-[120px] w-[132px]'
                    />
                    <div className='px-1.5 py-1 text-[11px] leading-4'>
                        <div>{loupe.hex}</div>
                        <div>
                            {loupe.x}, {loupe.y}
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
}

function placeImage(image, intent) {
    if (intent !== 'edit') {
        return {
            layout: { left: 0, top: 0, width: window.innerWidth, height: window.innerHeight },
            selection: null,
        };
    }
    const dpr = window.devicePixelRatio || 1;
    const cssW = image.width / dpr;
    const cssH = image.height / dpr;
    const fit = Math.min((window.innerWidth * 0.9) / cssW, (window.innerHeight * 0.8) / cssH, 1);
    const width = Math.max(1, cssW * fit);
    const height = Math.max(1, cssH * fit);
    const left = (window.innerWidth - width) / 2;
    const top = (window.innerHeight - height) / 2;
    return {
        layout: { left, top, width, height },
        selection: { x: left, y: top, w: width, h: height },
    };
}
