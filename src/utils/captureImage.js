import { readBinaryFile } from '@tauri-apps/api/fs';

function loadHtmlImage(src, withCors = false) {
    return new Promise((resolve, reject) => {
        const image = new Image();
        if (withCors) {
            // The asset response allows the webview origin. Requesting CORS
            // keeps the canvas untainted so copy/save can still export it.
            image.crossOrigin = 'anonymous';
        }
        image.onload = () => resolve(image);
        image.onerror = () => reject(new Error('decode'));
        image.src = src;
    });
}

export async function loadCaptureImage(src, localPath) {
    try {
        return await loadHtmlImage(src, true);
    } catch (corsError) {
        // Some WKWebView versions do not apply CORS to custom schemes. Fetching
        // the same local asset and decoding a blob keeps the canvas exportable.
        let blob;
        if (localPath) {
            blob = new Blob([await readBinaryFile(localPath)], { type: 'image/png' });
        } else {
            const response = await fetch(src);
            if (!response.ok) throw corsError;
            blob = await response.blob();
        }
        const url = URL.createObjectURL(blob);
        try {
            return await loadHtmlImage(url);
        } finally {
            URL.revokeObjectURL(url);
        }
    }
}
