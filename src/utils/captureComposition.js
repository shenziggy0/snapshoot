export function planComposition(kind, images) {
    const tutorial = kind === 'tutorial';
    const gutter = tutorial ? 24 : 0;
    let offset = gutter;
    const frames = images.map(({ width, height }) => {
        const frameWidth = tutorial ? Math.min(width, 1600) : width;
        const frameHeight = Math.max(1, Math.round((height * frameWidth) / width));
        const frame = tutorial
            ? { x: gutter, y: offset, width: frameWidth, height: frameHeight }
            : { x: offset, y: 0, width: frameWidth, height: frameHeight };
        offset += (tutorial ? frameHeight : frameWidth) + gutter;
        return frame;
    });
    const width = tutorial ? Math.max(...frames.map((frame) => frame.width)) + gutter * 2 : offset;
    const height = tutorial ? offset : Math.max(...frames.map((frame) => frame.height));
    if (width > 16384 || height > 16384 || width * height > 32_000_000) {
        throw new Error('recent.output_too_large');
    }
    return { width, height, frames };
}
