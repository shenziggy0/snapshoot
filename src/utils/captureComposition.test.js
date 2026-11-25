import test from 'node:test';
import assert from 'node:assert/strict';
import { planComposition } from './captureComposition.js';

test('before/after export preserves pixel size and position', () => {
    const plan = planComposition('compare', [
        { width: 800, height: 600 },
        { width: 800, height: 600 },
    ]);
    assert.deepEqual([plan.width, plan.height], [1600, 600]);
    assert.deepEqual(
        plan.frames.map(({ x, y }) => [x, y]),
        [
            [0, 0],
            [800, 0],
        ]
    );
});

test('tutorial frames stay in order and preserve each image aspect ratio', () => {
    const plan = planComposition('tutorial', [
        { width: 2400, height: 1200 },
        { width: 800, height: 200 },
        { width: 320, height: 640 },
    ]);
    assert.deepEqual(
        plan.frames.map(({ width, height }) => [width, height]),
        [
            [1600, 800],
            [800, 200],
            [320, 640],
        ]
    );
    assert.ok(plan.frames.every(({ y }, i) => i === 0 || y > plan.frames[i - 1].y + plan.frames[i - 1].height));
    assert.throws(
        () =>
            planComposition('compare', [
                { width: 8192, height: 8192 },
                { width: 800, height: 600 },
            ]),
        /recent.output_too_large/
    );
});
