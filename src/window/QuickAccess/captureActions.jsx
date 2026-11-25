import { Copy, Download, Pencil, Pin, ScanText, Upload } from 'lucide-react';
import { convertFileSrc } from '@tauri-apps/api/tauri';
import { save } from '@tauri-apps/api/dialog';
import { writeText } from '@tauri-apps/api/clipboard';
import { invoke } from '@tauri-apps/api';
import React from 'react';

export const captureActions = [
    ['copy', 'annotate.copy', Copy],
    ['edit', 'quick_access.edit', Pencil],
    ['save', 'annotate.save', Download],
    ['pin', 'annotate.pin', Pin],
    ['ocr', 'annotate.ocr', ScanText],
    ['share', 'quick_access.share', Upload],
];

export function CaptureThumb({ item }) {
    return (
        <img
            src={`${convertFileSrc(item.thumbPath)}?v=${item.createdAt}`}
            alt=''
            draggable={false}
        />
    );
}

export async function saveCapturePng(defaultPath, write) {
    const path = await save({ defaultPath, filters: [{ name: 'PNG', extensions: ['png'] }] });
    if (!path) return false;
    await write(path);
    return true;
}

export function captureErrorKey(error) {
    return error instanceof Error ? error.message : String(error);
}

export async function runCaptureAction(item, action) {
    const { id } = item;
    if (action === 'save') {
        const saved = await saveCapturePng('snapshoot-screenshot.png', (path) =>
            invoke('capture_history_save', { id, path })
        );
        return saved ? 'annotate.saved' : null;
    }
    if (action === 'share') {
        await writeText(await invoke('capture_history_share', { id }));
        return 'quick_access.share_copied';
    }
    await invoke(`capture_history_${action}`, { id });
    return action === 'copy' ? 'annotate.copied' : null;
}
