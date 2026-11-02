export function placeToolbar({ selection, size, viewport, margin = 8, gap = 10 }) {
    const left = Math.max(margin, Math.min(
        selection.x + selection.w / 2 - size.w / 2,
        viewport.w - size.w - margin
    ));
    const below = selection.y + selection.h + gap;
    if (below + size.h <= viewport.h - margin) {
        return { left, top: below };
    }
    const above = selection.y - gap - size.h;
    if (above >= margin) {
        return { left, top: above };
    }
    const top = Math.max(margin, Math.min(
        selection.y + selection.h - gap - size.h,
        viewport.h - size.h - margin
    ));
    return { left, top };
}
