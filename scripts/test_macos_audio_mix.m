// Run from the repository root:
// clang -fobjc-arc -fblocks scripts/test_macos_audio_mix.m -framework AppKit \
//   -framework AVFoundation -framework AudioToolbox -framework CoreMedia \
//   -framework CoreGraphics -framework ScreenCaptureKit -o /tmp/snapshoot-audio-test
// /tmp/snapshoot-audio-test
#import "../src-tauri/src/screen_record/macos_record.m"
#include <assert.h>

static CMSampleBufferRef tone(double rate, AVAudioChannelCount channels, double frequency) {
    AVAudioFormat *format = [[AVAudioFormat alloc] initStandardFormatWithSampleRate:rate channels:channels];
    AVAudioPCMBuffer *pcm = [[AVAudioPCMBuffer alloc] initWithPCMFormat:format frameCapacity:(AVAudioFrameCount)(rate / 10)];
    pcm.frameLength = pcm.frameCapacity;
    for (AVAudioChannelCount channel = 0; channel < channels; channel++) {
        for (AVAudioFrameCount frame = 0; frame < pcm.frameLength; frame++) {
            pcm.floatChannelData[channel][frame] = 0.6 * sin(2 * M_PI * frequency * frame / rate);
        }
    }
    CMSampleBufferRef value = NULL;
    assert(CMAudioSampleBufferCreateWithPacketDescriptions(kCFAllocatorDefault, NULL, false, NULL,
        NULL, format.formatDescription, pcm.frameLength, kCMTimeZero, NULL, &value) == noErr);
    assert(CMSampleBufferSetDataBufferFromAudioBufferList(value, kCFAllocatorDefault, kCFAllocatorDefault,
        kCMSampleBufferFlag_AudioBufferList_Assure16ByteAlignment, pcm.audioBufferList) == noErr);
    assert(CMSampleBufferSetDataReady(value) == noErr);
    return value;
}

static double amplitude(const float *samples, int first, int count, double frequency) {
    double real = 0;
    double imaginary = 0;
    for (int index = first; index < first + count; index++) {
        double phase = 2 * M_PI * frequency * index / 48000;
        real += samples[index] * cos(phase);
        imaginary += samples[index] * sin(phase);
    }
    return 2 * hypot(real, imaginary) / count;
}

static void verifyAAC(CMSampleBufferRef first, CMSampleBufferRef tail) {
    NSURL *url = [NSURL fileURLWithPath:[NSTemporaryDirectory()
        stringByAppendingPathComponent:[NSUUID.UUID.UUIDString stringByAppendingPathExtension:@"mp4"]]];
    NSError *error = nil;
    AVAssetWriter *writer = [[AVAssetWriter alloc] initWithURL:url fileType:AVFileTypeMPEG4 error:&error];
    assert(writer);
    AVAssetWriterInput *input = [[AVAssetWriterInput alloc] initWithMediaType:AVMediaTypeAudio
        outputSettings:@{AVFormatIDKey: @(kAudioFormatMPEG4AAC), AVSampleRateKey: @48000,
                         AVNumberOfChannelsKey: @2, AVEncoderBitRateKey: @160000}];
    assert([writer canAddInput:input]);
    [writer addInput:input];
    assert([writer startWriting]);
    [writer startSessionAtSourceTime:CMSampleBufferGetPresentationTimeStamp(first)];
    assert([input appendSampleBuffer:first]);
    assert([input appendSampleBuffer:tail]);
    [input markAsFinished];
    dispatch_semaphore_t done = dispatch_semaphore_create(0);
    [writer finishWritingWithCompletionHandler:^{ dispatch_semaphore_signal(done); }];
    assert(dispatch_semaphore_wait(done, dispatch_time(DISPATCH_TIME_NOW, 5 * NSEC_PER_SEC)) == 0);
    if (writer.status != AVAssetWriterStatusCompleted) NSLog(@"AAC writer: %@", writer.error);
    assert(writer.status == AVAssetWriterStatusCompleted);

    AVURLAsset *asset = [AVURLAsset URLAssetWithURL:url options:nil];
    __block NSArray<AVAssetTrack *> *tracks = nil;
    [asset loadTracksWithMediaType:AVMediaTypeAudio completionHandler:^(NSArray<AVAssetTrack *> *value, NSError *failure) {
        if (failure) NSLog(@"AAC reader: %@", failure);
        tracks = value;
        dispatch_semaphore_signal(done);
    }];
    assert(dispatch_semaphore_wait(done, dispatch_time(DISPATCH_TIME_NOW, 5 * NSEC_PER_SEC)) == 0);
    assert(tracks.count == 1);

    AudioFileID audioFile = NULL;
    assert(AudioFileOpenURL((__bridge CFURLRef)url, kAudioFileReadPermission, 0, &audioFile) == noErr);
    AudioStreamBasicDescription info;
    UInt32 size = sizeof(info);
    assert(AudioFileGetProperty(audioFile, kAudioFilePropertyDataFormat, &size, &info) == noErr);
    assert(info.mFormatID == kAudioFormatMPEG4AAC && info.mSampleRate == 48000 && info.mChannelsPerFrame == 2);
    AudioFileClose(audioFile);

    ExtAudioFileRef decoder = NULL;
    assert(ExtAudioFileOpenURL((__bridge CFURLRef)url, &decoder) == noErr);
    AVAudioFormat *pcmFormat = [[AVAudioFormat alloc] initStandardFormatWithSampleRate:48000 channels:2];
    assert(ExtAudioFileSetProperty(decoder, kExtAudioFileProperty_ClientDataFormat,
        sizeof(AudioStreamBasicDescription), pcmFormat.streamDescription) == noErr);
    AVAudioPCMBuffer *decoded = [[AVAudioPCMBuffer alloc] initWithPCMFormat:pcmFormat frameCapacity:5280];
    decoded.frameLength = decoded.frameCapacity;
    UInt32 frames = decoded.frameLength;
    assert(ExtAudioFileRead(decoder, &frames, decoded.mutableAudioBufferList) == noErr && frames >= 4800);
    for (int channel = 0; channel < 2; channel++) {
        assert(amplitude(decoded.floatChannelData[channel], 1440, 2400, 500) > 0.2);
        assert(amplitude(decoded.floatChannelData[channel], 1440, 2400, 1000) > 0.2);
    }
    ExtAudioFileDispose(decoder);
    assert([NSFileManager.defaultManager removeItemAtURL:url error:&error]);
}

int main(void) {
    @autoreleasepool {
        SRAudioMixer *mixer = [SRAudioMixer new];
        CMSampleBufferRef system = tone(48000, 2, 500);
        CMSampleBufferRef mic = tone(44100, 1, 1000);
        assert([mixer enqueueSample:system microphone:NO atFrame:0 first:0 count:4800]);
        // Place a clipped/normalized mono packet 20 ms after the screen audio.
        assert([mixer enqueueSample:mic microphone:YES atFrame:960 first:441 count:3528]);
        CFRelease(system);
        CFRelease(mic);

        CMTime origin = CMTimeMake(100, 1);
        CMSampleBufferRef mixed = [mixer copyNextBufferThrough:4800 origin:origin];
        assert(mixed && CMTimeCompare(CMSampleBufferGetPresentationTimeStamp(mixed), origin) == 0);
        assert(CMSampleBufferGetNumSamples(mixed) == 4800);
        AVAudioFormat *format = [[AVAudioFormat alloc] initWithStreamDescription:
            CMAudioFormatDescriptionGetStreamBasicDescription(CMSampleBufferGetFormatDescription(mixed))];
        assert(format.sampleRate == 48000 && format.channelCount == 2);
        AVAudioPCMBuffer *decoded = [[AVAudioPCMBuffer alloc] initWithPCMFormat:format frameCapacity:4800];
        decoded.frameLength = 4800;
        assert(CMSampleBufferCopyPCMDataIntoAudioBufferList(mixed, 0, 4800, decoded.mutableAudioBufferList) == noErr);

        for (int channel = 0; channel < 2; channel++) {
            const float *samples = decoded.floatChannelData[channel];
            assert(amplitude(samples, 0, 960, 500) > 0.24);
            assert(amplitude(samples, 0, 960, 1000) < 0.01);
            assert(amplitude(samples, 1440, 2400, 500) > 0.24);
            assert(amplitude(samples, 1440, 2400, 1000) > 0.24);
            assert(amplitude(samples, 0, 480, 1000) < 0.01);
        }
        CMSampleBufferRef tail = [mixer copyNextBufferThrough:5280 origin:origin];
        assert(tail && CMSampleBufferGetNumSamples(tail) == 480);
        assert(CMTimeCompare(CMSampleBufferGetPresentationTimeStamp(tail), CMTimeAdd(origin, CMTimeMake(4800, 48000))) == 0);
        verifyAAC(mixed, tail);
        CFRelease(mixed);
        CFRelease(tail);
        assert([mixer copyNextBufferThrough:5280 origin:origin] == NULL);
        puts("dual audio mix: stereo system + 44.1 kHz mono microphone, contiguous timestamps and a single 48 kHz stereo AAC track");
    }
    return 0;
}
