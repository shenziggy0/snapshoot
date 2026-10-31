function normRect(x, y, w, h) {
    return {
        x: w < 0 ? x + w : x,
        y: h < 0 ? y + h : y,
        w: Math.abs(w),
        h: Math.abs(h),
    };
}

function clipRect(rect, width, height) {
    const x = Math.max(0, Math.floor(rect.x));
    const y = Math.max(0, Math.floor(rect.y));
    const right = Math.min(width, Math.ceil(rect.x + rect.w));
    const bottom = Math.min(height, Math.ceil(rect.y + rect.h));
    return { x, y, w: Math.max(0, right - x), h: Math.max(0, bottom - y) };
}

function drawRect(ctx, shape) {
    const rect = normRect(shape.x, shape.y, shape.w, shape.h);
    ctx.save();
    ctx.strokeStyle = shape.color;
    ctx.lineWidth = shape.strokeWidth;
    ctx.strokeRect(rect.x, rect.y, rect.w, rect.h);
    ctx.restore();
}

function drawEllipse(ctx, shape) {
    const rect = normRect(shape.x, shape.y, shape.w, shape.h);
    if (rect.w < 1 || rect.h < 1) {
        return;
    }
    ctx.save();
    ctx.strokeStyle = shape.color;
    ctx.lineWidth = shape.strokeWidth;
    ctx.beginPath();
    ctx.ellipse(rect.x + rect.w / 2, rect.y + rect.h / 2, rect.w / 2, rect.h / 2, 0, 0, Math.PI * 2);
    ctx.stroke();
    ctx.restore();
}

function drawArrow(ctx, shape) {
    const x1 = shape.x;
    const y1 = shape.y;
    const x2 = shape.x2;
    const y2 = shape.y2;
    const angle = Math.atan2(y2 - y1, x2 - x1);
    const head = Math.max(12, shape.strokeWidth * 4);
    ctx.save();
    ctx.strokeStyle = shape.color;
    ctx.fillStyle = shape.color;
    ctx.lineWidth = shape.strokeWidth;
    ctx.lineCap = 'round';
    ctx.beginPath();
    ctx.moveTo(x1, y1);
    ctx.lineTo(x2, y2);
    ctx.stroke();
    ctx.beginPath();
    ctx.moveTo(x2, y2);
    ctx.lineTo(x2 - head * Math.cos(angle - Math.PI / 6), y2 - head * Math.sin(angle - Math.PI / 6));
    ctx.lineTo(x2 - head * Math.cos(angle + Math.PI / 6), y2 - head * Math.sin(angle + Math.PI / 6));
    ctx.closePath();
    ctx.fill();
    ctx.restore();
}

function drawStroke(ctx, shape, highlighter) {
    const points = shape.points || [];
    if (!points.length) {
        return;
    }
    ctx.save();
    ctx.lineCap = 'round';
    ctx.lineJoin = 'round';
    ctx.strokeStyle = shape.color;
    ctx.fillStyle = shape.color;
    ctx.lineWidth = highlighter ? Math.max(12, shape.strokeWidth * 6) : shape.strokeWidth;
    if (highlighter) {
        ctx.globalAlpha = 0.45;
        ctx.globalCompositeOperation = 'multiply';
    }
    if (points.length === 1) {
        const radius = ctx.lineWidth / 2;
        ctx.beginPath();
        ctx.arc(points[0].x, points[0].y, radius, 0, Math.PI * 2);
        ctx.fill();
    } else {
        ctx.beginPath();
        ctx.moveTo(points[0].x, points[0].y);
        for (let i = 1; i < points.length; i += 1) {
            ctx.lineTo(points[i].x, points[i].y);
        }
        ctx.stroke();
    }
    ctx.restore();
}

function drawText(ctx, shape) {
    const text = shape.text || '';
    if (!text) {
        return;
    }
    ctx.save();
    ctx.fillStyle = shape.color;
    ctx.font = `${shape.fontSize || 24}px sans-serif`;
    ctx.textBaseline = 'top';
    ctx.fillText(text, shape.x, shape.y);
    ctx.restore();
}

function drawNumber(ctx, shape) {
    const radius = Math.max(14, (shape.strokeWidth || 2) * 4);
    ctx.save();
    ctx.beginPath();
    ctx.fillStyle = shape.color;
    ctx.arc(shape.x, shape.y, radius, 0, Math.PI * 2);
    ctx.fill();
    ctx.fillStyle = '#ffffff';
    ctx.font = `bold ${Math.round(radius)}px sans-serif`;
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.fillText(String(shape.n), shape.x, shape.y + 1);
    ctx.restore();
}

function applyMosaic(ctx, shape) {
    const rect = clipRect(normRect(shape.x, shape.y, shape.w, shape.h), ctx.canvas.width, ctx.canvas.height);
    if (rect.w < 2 || rect.h < 2) {
        return;
    }
    const block = Math.max(4, shape.block || 12);
    const imageData = ctx.getImageData(rect.x, rect.y, rect.w, rect.h);
    const { data } = imageData;
    for (let by = 0; by < rect.h; by += block) {
        for (let bx = 0; bx < rect.w; bx += block) {
            const bw = Math.min(block, rect.w - bx);
            const bh = Math.min(block, rect.h - by);
            let r = 0;
            let g = 0;
            let b = 0;
            let a = 0;
            let count = 0;
            for (let yy = 0; yy < bh; yy += 1) {
                for (let xx = 0; xx < bw; xx += 1) {
                    const index = ((by + yy) * rect.w + (bx + xx)) * 4;
                    r += data[index];
                    g += data[index + 1];
                    b += data[index + 2];
                    a += data[index + 3];
                    count += 1;
                }
            }
            r /= count;
            g /= count;
            b /= count;
            a /= count;
            for (let yy = 0; yy < bh; yy += 1) {
                for (let xx = 0; xx < bw; xx += 1) {
                    const index = ((by + yy) * rect.w + (bx + xx)) * 4;
                    data[index] = r;
                    data[index + 1] = g;
                    data[index + 2] = b;
                    data[index + 3] = a;
                }
            }
        }
    }
    ctx.putImageData(imageData, rect.x, rect.y);
}

function boxBlur(imageData, radius) {
    const { width, height, data } = imageData;
    const src = new Uint8ClampedArray(data);
    const tmp = new Uint8ClampedArray(src.length);
    const r = Math.max(1, radius | 0);
    const div = r * 2 + 1;
    for (let y = 0; y < height; y += 1) {
        for (let x = 0; x < width; x += 1) {
            let rs = 0;
            let gs = 0;
            let bs = 0;
            let as = 0;
            for (let k = -r; k <= r; k += 1) {
                const xx = Math.min(width - 1, Math.max(0, x + k));
                const index = (y * width + xx) * 4;
                rs += src[index];
                gs += src[index + 1];
                bs += src[index + 2];
                as += src[index + 3];
            }
            const out = (y * width + x) * 4;
            tmp[out] = rs / div;
            tmp[out + 1] = gs / div;
            tmp[out + 2] = bs / div;
            tmp[out + 3] = as / div;
        }
    }
    for (let y = 0; y < height; y += 1) {
        for (let x = 0; x < width; x += 1) {
            let rs = 0;
            let gs = 0;
            let bs = 0;
            let as = 0;
            for (let k = -r; k <= r; k += 1) {
                const yy = Math.min(height - 1, Math.max(0, y + k));
                const index = (yy * width + x) * 4;
                rs += tmp[index];
                gs += tmp[index + 1];
                bs += tmp[index + 2];
                as += tmp[index + 3];
            }
            const out = (y * width + x) * 4;
            data[out] = rs / div;
            data[out + 1] = gs / div;
            data[out + 2] = bs / div;
            data[out + 3] = as / div;
        }
    }
    return imageData;
}

function applyBlur(ctx, shape) {
    const rect = clipRect(normRect(shape.x, shape.y, shape.w, shape.h), ctx.canvas.width, ctx.canvas.height);
    if (rect.w < 2 || rect.h < 2) {
        return;
    }
    const imageData = ctx.getImageData(rect.x, rect.y, rect.w, rect.h);
    const radius = Math.max(2, Math.min(8, shape.radius || 4));
    boxBlur(imageData, radius);
    ctx.putImageData(imageData, rect.x, rect.y);
}

function drawPreviewRect(ctx, shape) {
    const rect = normRect(shape.x, shape.y, shape.w, shape.h);
    ctx.save();
    ctx.strokeStyle = shape.color || '#ffffff';
    ctx.lineWidth = 1;
    ctx.setLineDash([6, 4]);
    ctx.strokeRect(rect.x, rect.y, rect.w, rect.h);
    ctx.restore();
}

export function drawShape(ctx, shape, preview) {
    if (!shape) {
        return;
    }
    if (preview && (shape.type === 'mosaic' || shape.type === 'blur')) {
        drawPreviewRect(ctx, shape);
        return;
    }
    switch (shape.type) {
        case 'rect':
            drawRect(ctx, shape);
            break;
        case 'ellipse':
            drawEllipse(ctx, shape);
            break;
        case 'arrow':
            drawArrow(ctx, shape);
            break;
        case 'pen':
            drawStroke(ctx, shape, false);
            break;
        case 'highlighter':
            drawStroke(ctx, shape, true);
            break;
        case 'text':
            drawText(ctx, shape);
            break;
        case 'number':
            drawNumber(ctx, shape);
            break;
        case 'mosaic':
            applyMosaic(ctx, shape);
            break;
        case 'blur':
            applyBlur(ctx, shape);
            break;
        default:
            break;
    }
}

export function renderScene(ctx, image, shapes, draft) {
    const { width, height } = ctx.canvas;
    ctx.clearRect(0, 0, width, height);
    if (image) {
        ctx.drawImage(image, 0, 0, width, height);
    }
    for (const shape of shapes) {
        drawShape(ctx, shape, false);
    }
    if (draft) {
        drawShape(ctx, draft, true);
    }
}
