import { useCallback } from 'react';
let audioContext = null;
let source = null;

export const useVoice = () => {
    const playOrStop = useCallback((data) => {
        if (source) {
            // 如果正在播放，停止播放
            source.stop();
            source.disconnect();
            source = null;
        } else {
            // 如果没在播放，开始播放。首次真正朗读时才创建 AudioContext，
            // 避免仅 import 模块就占用音频输出、阻止系统休眠。
            if (!audioContext) {
                audioContext = new (window.AudioContext || window.webkitAudioContext)();
            }
            audioContext.decodeAudioData(new Uint8Array(data).buffer, (buffer) => {
                source = audioContext.createBufferSource();
                source.buffer = buffer;
                source.connect(audioContext.destination);
                source.start();
                source.onended = () => {
                    source.disconnect();
                    source = null;
                };
            });
        }
    });

    return playOrStop;
};
