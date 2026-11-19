import { invoke } from '@tauri-apps/api/tauri';
import { listen } from '@tauri-apps/api/event';
import { nanoid } from 'nanoid';

export async function translateStream(instanceKey, body, setResult) {
    const requestId = nanoid();
    let result = '';
    const unlisten = await listen('translate-stream-chunk', ({ payload }) => {
        if (payload.requestId !== requestId) return;
        result += payload.text;
        setResult?.(result + '_');
    });
    try {
        return await invoke('translate_stream', { requestId, instanceKey, body });
    } finally {
        unlisten();
    }
}
