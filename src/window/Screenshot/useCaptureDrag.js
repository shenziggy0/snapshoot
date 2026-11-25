import { useRef } from 'react';
import { invoke } from '@tauri-apps/api';
import { osType } from '../../utils/env';

export function useCaptureDrag(id, onError) {
    const pointer = useRef(null);
    const dragged = useRef(false);
    if (!id || (osType !== 'Darwin' && osType !== 'Windows_NT')) return {};

    const end = () => {
        pointer.current = null;
    };
    return {
        onPointerDown(event) {
            if (event.button !== 0 || event.pointerType !== 'mouse') return;
            pointer.current = { id: event.pointerId, x: event.clientX, y: event.clientY };
            dragged.current = false;
            event.currentTarget.setPointerCapture(event.pointerId);
        },
        onPointerMove(event) {
            const start = pointer.current;
            if (!start || event.pointerId !== start.id) return;
            if ((event.buttons & 1) === 0) {
                end();
                return;
            }
            if (Math.hypot(event.clientX - start.x, event.clientY - start.y) < 5) return;
            dragged.current = true;
            end();
            // Let AppKit or OLE own the pointer once the native gesture starts.
            if (event.currentTarget.hasPointerCapture(start.id)) {
                event.currentTarget.releasePointerCapture(start.id);
            }
            void invoke('capture_history_drag', { id }).catch(onError);
        },
        onPointerUp: end,
        onPointerCancel: end,
        onLostPointerCapture: end,
        onDragStart(event) {
            event.preventDefault();
        },
        onClickCapture(event) {
            if (dragged.current && event.detail !== 0) {
                dragged.current = false;
                event.stopPropagation();
                event.preventDefault();
            }
        },
    };
}
