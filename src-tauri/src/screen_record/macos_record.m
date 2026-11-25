#import <AppKit/AppKit.h>
#import <AVFoundation/AVFoundation.h>
#import <AudioToolbox/AudioToolbox.h>
#import <CoreGraphics/CoreGraphics.h>
#import <CoreMedia/CoreMedia.h>
#import <CoreVideo/CoreVideo.h>
#import <ScreenCaptureKit/ScreenCaptureKit.h>
#import <dispatch/dispatch.h>
#include <math.h>
#include <stdint.h>
#include <stdlib.h>
#include <unistd.h>

// C ABI codes shared with macos.rs: 0 success, 1 unsupported, 2 permission,
// 3 encoder/capture/timeout, 4 filesystem. Never return borrowed NSString data.
enum { SROk = 0, SRUnsupported = 1, SRPermission = 2, SREncoder = 3, SRDisk = 4 };

static int SRError(NSError *error, int fallback) {
    if (error) NSLog(@"Snapshoot recording: %@", error);
    if ([error.domain isEqualToString:NSCocoaErrorDomain] &&
        (error.code == NSFileWriteOutOfSpaceError || error.code == NSFileWriteNoPermissionError ||
         error.code == NSFileWriteInvalidFileNameError || error.code == NSFileNoSuchFileError)) return SRDisk;
    return fallback;
}

static CMTime SRNow(void) { return CMClockGetTime(CMClockGetHostTimeClock()); }

// Preserve all timing entries, including audio's per-sample duration. PTS and
// valid DTS move by the same amount; the source clock is never replaced by an
// independently accumulated frame/sample count.
static CMSampleBufferRef SRCopyTiming(CMSampleBufferRef sample, CMTime offset, CMTime duration) {
    CMItemCount count = 0;
    if (CMSampleBufferGetSampleTimingInfoArray(sample, 0, NULL, &count) != noErr || count <= 0) return NULL;
    CMSampleTimingInfo *timing = calloc((size_t)count, sizeof(*timing));
    if (!timing) return NULL;
    CMSampleBufferRef copy = NULL;
    if (CMSampleBufferGetSampleTimingInfoArray(sample, count, timing, &count) == noErr) {
        for (CMItemCount i = 0; i < count; ++i) {
            if (CMTIME_IS_NUMERIC(timing[i].presentationTimeStamp))
                timing[i].presentationTimeStamp = CMTimeSubtract(timing[i].presentationTimeStamp, offset);
            if (CMTIME_IS_NUMERIC(timing[i].decodeTimeStamp))
                timing[i].decodeTimeStamp = CMTimeSubtract(timing[i].decodeTimeStamp, offset);
            if (CMTIME_IS_NUMERIC(duration)) timing[i].duration = duration;
        }
        if (CMSampleBufferCreateCopyWithNewTiming(kCFAllocatorDefault, sample, count, timing, &copy) != noErr)
            copy = NULL;
    }
    free(timing);
    return copy;
}

// CMSampleBufferCopySampleBufferForRange does not support non-interleaved
// audio. SCK can supply planar PCM, so copy boundary samples via AudioBufferList
// and let CoreMedia own a copy of all planes before this scratch buffer dies.
static CMSampleBufferRef SRCopyAudioRange(CMSampleBufferRef sample, CMItemCount first, CMItemCount count) {
    if (first < 0 || count <= 0 || first > INT32_MAX || count > INT32_MAX) return NULL;
    CMAudioFormatDescriptionRef description = CMSampleBufferGetFormatDescription(sample);
    const AudioStreamBasicDescription *asbd = CMAudioFormatDescriptionGetStreamBasicDescription(description);
    if (!asbd || asbd->mFormatID != kAudioFormatLinearPCM || asbd->mSampleRate <= 0) return NULL;
    AVAudioFormat *format = [[AVAudioFormat alloc] initWithStreamDescription:asbd];
    if (!format) return NULL;
    AVAudioPCMBuffer *pcm = [[AVAudioPCMBuffer alloc] initWithPCMFormat:format frameCapacity:(AVAudioFrameCount)count];
    if (!pcm) return NULL;
    pcm.frameLength = (AVAudioFrameCount)count;
    if (CMSampleBufferCopyPCMDataIntoAudioBufferList(sample, (int32_t)first, (int32_t)count, pcm.mutableAudioBufferList) != noErr) return NULL;
    CMTime step = CMTimeMake(1, (int32_t)llround(asbd->mSampleRate));
    CMSampleTimingInfo timing = { step,
        CMTimeAdd(CMSampleBufferGetPresentationTimeStamp(sample), CMTimeMake(first, step.timescale)), kCMTimeInvalid };
    CMSampleBufferRef copy = NULL;
    if (CMSampleBufferCreate(kCFAllocatorDefault, NULL, false, NULL, NULL, description,
        count, 1, &timing, 0, NULL, &copy) != noErr) return NULL;
    OSStatus result = CMSampleBufferSetDataBufferFromAudioBufferList(copy, kCFAllocatorDefault,
        kCFAllocatorDefault, kCMSampleBufferFlag_AudioBufferList_Assure16ByteAlignment, pcm.audioBufferList);
    if (result == noErr) result = CMSampleBufferSetDataReady(copy);
    if (result != noErr) { CFRelease(copy); return NULL; }
    return copy;
}

enum { SRMixRate = 48000, SRMixFrames = 4800, SRMixLatencyMs = 200 };

@interface SRAudioPacket : NSObject
@property(nonatomic) int64_t start;
@property(nonatomic, strong) AVAudioPCMBuffer *pcm;
@end
@implementation SRAudioPacket
@end

// Both capture sources are timed on the host clock. Hold a short window so
// asynchronous microphone and screen callbacks can land in the same samples.
@interface SRAudioMixer : NSObject
@property(nonatomic, readonly) int64_t nextFrame;
@property(nonatomic, readonly) int64_t latestFrame;
- (BOOL)enqueueSample:(CMSampleBufferRef)sample microphone:(BOOL)microphone atFrame:(int64_t)frame
                first:(CMItemCount)first count:(CMItemCount)count;
- (CMSampleBufferRef)copyNextBufferThrough:(int64_t)end origin:(CMTime)origin CF_RETURNS_RETAINED;
@end

@implementation SRAudioMixer {
    AVAudioFormat *_format;
    NSMutableArray<SRAudioPacket *> *_system;
    NSMutableArray<SRAudioPacket *> *_mic;
    AVAudioConverter *_systemConverter;
    AVAudioConverter *_micConverter;
}
- (instancetype)init {
    if ((self = [super init])) {
        _format = [[AVAudioFormat alloc] initStandardFormatWithSampleRate:SRMixRate channels:2];
        _system = [NSMutableArray array];
        _mic = [NSMutableArray array];
    }
    return self;
}
- (BOOL)enqueueSample:(CMSampleBufferRef)sample microphone:(BOOL)microphone atFrame:(int64_t)frame
                first:(CMItemCount)first count:(CMItemCount)count {
    const AudioStreamBasicDescription *format =
        CMAudioFormatDescriptionGetStreamBasicDescription(CMSampleBufferGetFormatDescription(sample));
    if (!format || format->mFormatID != kAudioFormatLinearPCM || format->mSampleRate <= 0 ||
        first < 0 || first > INT32_MAX || count <= 0 || count > INT32_MAX) return NO;
    AVAudioFormat *inputFormat = [[AVAudioFormat alloc] initWithStreamDescription:format];
    if (!inputFormat) return NO;
    AVAudioPCMBuffer *input = [[AVAudioPCMBuffer alloc] initWithPCMFormat:inputFormat frameCapacity:(AVAudioFrameCount)count];
    if (!input) return NO;
    input.frameLength = (AVAudioFrameCount)count;
    if (CMSampleBufferCopyPCMDataIntoAudioBufferList(sample, (int32_t)first, (int32_t)count,
        input.mutableAudioBufferList) != noErr) return NO;

    AVAudioPCMBuffer *pcm = input;
    if (![_format isEqual:inputFormat]) {
        AVAudioConverter *converter = microphone ? _micConverter : _systemConverter;
        if (![converter.inputFormat isEqual:inputFormat]) {
            converter = [[AVAudioConverter alloc] initFromFormat:inputFormat toFormat:_format];
            if (!converter) return NO;
            converter.primeMethod = AVAudioConverterPrimeMethod_None;
            if (inputFormat.channelCount == 1) converter.channelMap = @[@0, @0];
            if (microphone) _micConverter = converter;
            else _systemConverter = converter;
        }
        double frames = ceil(count * _format.sampleRate / inputFormat.sampleRate);
        if (frames > UINT32_MAX - 16) return NO;
        pcm = [[AVAudioPCMBuffer alloc] initWithPCMFormat:_format frameCapacity:(AVAudioFrameCount)frames + 16];
        if (!pcm) return NO;
        NSError *error = nil;
        __block BOOL supplied = NO;
        AVAudioConverterOutputStatus status = [converter convertToBuffer:pcm error:&error
            withInputFromBlock:^AVAudioBuffer *(AVAudioPacketCount requested, AVAudioConverterInputStatus *inputStatus) {
                (void)requested;
                *inputStatus = supplied ? AVAudioConverterInputStatus_NoDataNow : AVAudioConverterInputStatus_HaveData;
                if (supplied) return nil;
                supplied = YES;
                return input;
            }];
        if (status == AVAudioConverterOutputStatus_Error) {
            NSLog(@"Snapshoot audio conversion: %@", error);
            return NO;
        }
    }

    if (pcm.frameLength == 0 || frame + pcm.frameLength <= _nextFrame) return YES;
    SRAudioPacket *packet = [SRAudioPacket new];
    packet.start = frame;
    packet.pcm = pcm;
    NSMutableArray<SRAudioPacket *> *packets = microphone ? _mic : _system;
    // AVFoundation delivers each source in order. Ignore a duplicate packet
    // rather than accumulating that source twice in the overlap.
    if (packets.lastObject && frame < packets.lastObject.start) return YES;
    [packets addObject:packet];
    _latestFrame = MAX(_latestFrame, frame + pcm.frameLength);
    return YES;
}
- (void)mixPackets:(NSMutableArray<SRAudioPacket *> *)packets into:(AVAudioPCMBuffer *)output {
    int64_t end = _nextFrame + output.frameLength;
    int64_t mixedThrough = _nextFrame;
    for (SRAudioPacket *packet in packets) {
        if (packet.start >= end) break;
        int64_t start = MAX(mixedThrough, packet.start);
        int64_t limit = MIN(end, packet.start + packet.pcm.frameLength);
        if (limit <= start) continue;
        for (AVAudioChannelCount channel = 0; channel < _format.channelCount; channel++) {
            float *out = output.floatChannelData[channel] + (start - _nextFrame);
            const float *data = packet.pcm.floatChannelData[channel] + (start - packet.start);
            for (int64_t index = 0; index < limit - start; index++) out[index] += 0.5f * data[index];
        }
        mixedThrough = limit;
    }
}
- (CMSampleBufferRef)copyNextBufferThrough:(int64_t)end origin:(CMTime)origin {
    if (end <= _nextFrame) return NULL;
    AVAudioFrameCount count = (AVAudioFrameCount)MIN(end - _nextFrame, SRMixFrames);
    AVAudioPCMBuffer *output = [[AVAudioPCMBuffer alloc] initWithPCMFormat:_format frameCapacity:count];
    if (!output) return NULL;
    output.frameLength = count;
    for (AVAudioChannelCount channel = 0; channel < _format.channelCount; channel++) {
        memset(output.floatChannelData[channel], 0, count * sizeof(float));
    }
    [self mixPackets:_system into:output];
    [self mixPackets:_mic into:output];
    CMTime pts = CMTimeAdd(origin, CMTimeMake(_nextFrame, SRMixRate));
    CMSampleBufferRef result = NULL;
    if (CMAudioSampleBufferCreateWithPacketDescriptions(kCFAllocatorDefault, NULL, false, NULL, NULL,
        _format.formatDescription, count, pts, NULL, &result) != noErr) return NULL;
    OSStatus status = CMSampleBufferSetDataBufferFromAudioBufferList(result, kCFAllocatorDefault,
        kCFAllocatorDefault, kCMSampleBufferFlag_AudioBufferList_Assure16ByteAlignment, output.audioBufferList);
    if (status == noErr) status = CMSampleBufferSetDataReady(result);
    if (status != noErr) { CFRelease(result); return NULL; }
    _nextFrame += count;
    for (NSMutableArray<SRAudioPacket *> *packets in @[_system, _mic]) {
        while (packets.firstObject && packets.firstObject.start + packets.firstObject.pcm.frameLength <= _nextFrame)
            [packets removeObjectAtIndex:0];
    }
    return result;
}
@end

@interface SRReply : NSObject {
@public
    dispatch_semaphore_t gate;
    int code;
    uint32_t state;
    uint64_t elapsed;
}
@end
@implementation SRReply
- (instancetype)init {
    if ((self = [super init])) gate = dispatch_semaphore_create(0);
    return self;
}
@end

API_AVAILABLE(macos(12.3))
@interface SRRecorder : NSObject <SCStreamOutput, SCStreamDelegate, AVCaptureAudioDataOutputSampleBufferDelegate> {
@public
    dispatch_queue_t queue;
    dispatch_queue_t audioQueue;
    uint32_t state;
    int errorCode;
    uint64_t finalElapsed;
    SCStream *stream;
    AVCaptureSession *micSession;
    AVCaptureAudioDataOutput *micOutput;
    CMClockRef micClock;
    AVAssetWriter *writer;
    AVAssetWriterInput *videoInput;
    AVAssetWriterInput *audioInput;
    SRAudioMixer *audioMixer;
    BOOL systemAudio;
    NSURL *url;
    BOOL ownsOutput;
    BOOL starting;
    BOOL captureStarted;
    BOOL finishing;
    BOOL finishingWriter;
    BOOL finished;
    BOOL cancelled;
    BOOL paused;
    CMTime origin;
    CMTime pausedAt;
    CMTime pausedTotal;
    CMTime intervalStart;
    CMTime finishAt;
    CMTime lastVideoPTS;
    CMTime lastAudioEnd;
    CMTime frameDuration;
    CMSampleBufferRef lastVideo;
    NSMutableArray *completions;
}
- (uint64_t)elapsed;
- (void)fail:(int)code;
- (void)complete;
- (void)appendAudioSampleBuffer:(CMSampleBufferRef)sample microphone:(BOOL)microphone;
- (BOOL)appendMixedAudioThrough:(CMTime)time final:(BOOL)final;
- (void)requestFinish:(BOOL)cancel completion:(void (^)(int, uint64_t))completion;
- (void)stopCapture;
- (void)finishWriter;
@end

@implementation SRRecorder
- (instancetype)init {
    if ((self = [super init])) {
        queue = dispatch_queue_create("top.shenziggy.snapshoot.screen-record",
            dispatch_queue_attr_make_with_autorelease_frequency(DISPATCH_QUEUE_SERIAL, DISPATCH_AUTORELEASE_FREQUENCY_WORK_ITEM));
        // Sample callbacks must not share the queue that calls startRunning.
        audioQueue = dispatch_queue_create("top.shenziggy.snapshoot.screen-record.audio",
            dispatch_queue_attr_make_with_autorelease_frequency(DISPATCH_QUEUE_SERIAL, DISPATCH_AUTORELEASE_FREQUENCY_WORK_ITEM));
        origin = pausedAt = intervalStart = finishAt = lastVideoPTS = lastAudioEnd = kCMTimeInvalid;
        pausedTotal = kCMTimeZero;
        completions = [NSMutableArray array];
    }
    return self;
}
- (void)dealloc {
    if (lastVideo) CFRelease(lastVideo);
    if (micClock) CFRelease(micClock);
}
- (uint64_t)elapsed {
    if (finished) return finalElapsed;
    if (!CMTIME_IS_NUMERIC(origin)) return 0;
    CMTime end = finishing ? finishAt : (paused ? pausedAt : SRNow());
    double seconds = CMTimeGetSeconds(CMTimeSubtract(CMTimeSubtract(end, pausedTotal), origin));
    return isfinite(seconds) && seconds > 0 ? (uint64_t)(seconds * 1000.0) : 0;
}
- (void)fail:(int)code {
    if (finished) return;
    if (!errorCode) errorCode = code;
    state = 3;
    [self requestFinish:YES completion:nil];
}
- (void)complete {
    if (finished) return;
    finalElapsed = [self elapsed];
    finished = YES;
    if (stream) {
        [stream removeStreamOutput:self type:SCStreamOutputTypeScreen error:NULL];
        if (@available(macOS 13.0, *)) {
            if (systemAudio) [stream removeStreamOutput:self type:SCStreamOutputTypeAudio error:NULL];
        }
    }
    stream = nil;
    if (micSession) {
        [micSession stopRunning];
        micSession = nil;
    }
    micOutput = nil;
    if (micClock) { CFRelease(micClock); micClock = NULL; }
    writer = nil;
    videoInput = nil;
    audioInput = nil;
    audioMixer = nil;
    if (lastVideo) { CFRelease(lastVideo); lastVideo = NULL; }
    if ((cancelled || errorCode) && ownsOutput && [[NSFileManager defaultManager] fileExistsAtPath:url.path]) {
        NSError *error = nil;
        if (![[NSFileManager defaultManager] removeItemAtURL:url error:&error]) errorCode = SRError(error, SRDisk);
    }
    if (errorCode) state = 3;
    NSArray *callbacks = [completions copy];
    [completions removeAllObjects];
    for (void (^callback)(int, uint64_t) in callbacks) callback(errorCode, finalElapsed);
}
- (void)requestFinish:(BOOL)cancel completion:(void (^)(int, uint64_t))completion {
    if (finished) { if (completion) completion(errorCode, finalElapsed); return; }
    if (completion) [completions addObject:[completion copy]];
    cancelled = cancelled || cancel;
    if (finishing) return;
    finishing = YES;
    finishAt = paused ? pausedAt : SRNow();
    if (!errorCode) state = 2;
    // The watchdog runs on the same queue as all state mutation. Late platform
    // callbacks retain the object but cannot revive a completed recording.
    dispatch_after(dispatch_time(DISPATCH_TIME_NOW, 30 * NSEC_PER_SEC), queue, ^{
        if (!self->finished) {
            self->errorCode = SREncoder;
            self->cancelled = YES;
            [self->stream stopCaptureWithCompletionHandler:^(NSError *error) { (void)error; }];
            [self->writer cancelWriting];
            [self complete];
        }
    });
    if (!starting) [self stopCapture];
}
- (void)stopCapture {
    if (!stream || !captureStarted) { [self finishWriter]; return; }
    captureStarted = NO;
    [stream stopCaptureWithCompletionHandler:^(NSError *error) {
        dispatch_async(self->queue, ^{
            if (self->finished) return;
            if (error && !self->cancelled) self->errorCode = SRError(error, SREncoder);
            if (self->audioMixer && self->micSession && !self->cancelled && !self->errorCode) {
                [self->micSession stopRunning];
                // The microphone delegate forwards packets to the recorder queue.
                // Let both queues deliver their final packets before closing AAC.
                dispatch_async(self->audioQueue, ^{
                    dispatch_async(self->queue, ^{ [self finishWriter]; });
                });
                return;
            }
            [self finishWriter];
        });
    }];
}
- (void)finishWriter {
    if (finished || finishingWriter) return;
    if (cancelled || errorCode || !CMTIME_IS_NUMERIC(origin) || !lastVideo) {
        if (!cancelled && !errorCode) errorCode = SREncoder;
        [writer cancelWriting];
        [self complete];
        return;
    }
    CMTime end = CMTimeSubtract(finishAt, pausedTotal);
    if (audioMixer && ![self appendMixedAudioThrough:finishAt final:YES]) {
        dispatch_after(dispatch_time(DISPATCH_TIME_NOW, 10 * NSEC_PER_MSEC), queue, ^{ [self finishWriter]; });
        return;
    }
    if (errorCode) {
        [writer cancelWriting];
        [self complete];
        return;
    }
    // ScreenCaptureKit may stop delivering complete frames on a static screen.
    // Extend the last image to the stop time instead of producing a tiny movie.
    CMTime tailPTS = CMTimeSubtract(end, frameDuration);
    if (lastVideo && CMTimeCompare(tailPTS, lastVideoPTS) > 0) {
        if (!videoInput.readyForMoreMediaData) {
            dispatch_after(dispatch_time(DISPATCH_TIME_NOW, 10 * NSEC_PER_MSEC), queue, ^{ [self finishWriter]; });
            return;
        }
        CMSampleBufferRef tail = SRCopyTiming(lastVideo, CMTimeSubtract(lastVideoPTS, tailPTS), frameDuration);
        if (!tail || ![videoInput appendSampleBuffer:tail]) {
            if (tail) CFRelease(tail);
            errorCode = SRError(writer.error, SREncoder);
            [writer cancelWriting];
            [self complete];
            return;
        }
        CFRelease(tail);
    }
    finishingWriter = YES;
    [writer endSessionAtSourceTime:end];
    [videoInput markAsFinished];
    [audioInput markAsFinished];
    [writer finishWritingWithCompletionHandler:^{
        dispatch_async(self->queue, ^{
            if (self->finished) return;
            if (self->writer.status != AVAssetWriterStatusCompleted)
                self->errorCode = SRError(self->writer.error, SREncoder);
            [self complete];
        });
    }];
}
- (void)stream:(SCStream *)capture didStopWithError:(NSError *)error {
    (void)capture;
    dispatch_async(queue, ^{
        if (!self->finishing && !self->finished) [self fail:SRError(error, SREncoder)];
    });
}
- (BOOL)appendMixedAudioThrough:(CMTime)time final:(BOOL)final {
    CMTime active = CMTimeSubtract(CMTimeSubtract(time, pausedTotal), origin);
    if (!CMTIME_IS_NUMERIC(active)) return YES;
    int64_t end = CMTimeConvertScale(active, SRMixRate, kCMTimeRoundingMethod_RoundTowardZero).value;
    if (!final) end = MIN(end, audioMixer.latestFrame);
    while (audioMixer.nextFrame < end) {
        if (!audioInput.readyForMoreMediaData) return NO;
        CMSampleBufferRef block = [audioMixer copyNextBufferThrough:end origin:origin];
        BOOL appended = block && [audioInput appendSampleBuffer:block];
        if (block) CFRelease(block);
        if (!appended) {
            if (final) errorCode = SRError(writer.error, SREncoder);
            else [self fail:SRError(writer.error, SREncoder)];
            break;
        }
    }
    return YES;
}
- (void)appendAudioSampleBuffer:(CMSampleBufferRef)sample microphone:(BOOL)microphone {
    @autoreleasepool {
        if (finished || (finishing && (!audioMixer || finishingWriter || cancelled)) ||
            errorCode || !audioInput || !CMSampleBufferIsValid(sample) ||
            !CMSampleBufferDataIsReady(sample) || !CMTIME_IS_NUMERIC(origin)) return;
        CMTime pts = CMSampleBufferGetPresentationTimeStamp(sample);
        if (microphone && micClock) pts = CMSyncConvertTime(pts, micClock, CMClockGetHostTimeClock());
        if (!CMTIME_IS_NUMERIC(pts)) return;
        CMTime upper = finishing ? finishAt : (paused ? pausedAt : kCMTimeInvalid);
        if (CMTIME_IS_NUMERIC(upper) && CMTimeCompare(pts, upper) >= 0) return;
        const AudioStreamBasicDescription *format =
            CMAudioFormatDescriptionGetStreamBasicDescription(CMSampleBufferGetFormatDescription(sample));
        if (!format || format->mSampleRate <= 0) { [self fail:SREncoder]; return; }
        CMItemCount count = CMSampleBufferGetNumSamples(sample);
        CMTime lower = CMTIME_IS_NUMERIC(intervalStart) && CMTimeCompare(intervalStart, origin) > 0 ? intervalStart : origin;
        if (!audioMixer && CMTIME_IS_NUMERIC(lastAudioEnd)) {
            CMTime previousEnd = CMTimeAdd(lastAudioEnd, pausedTotal);
            if (CMTimeCompare(previousEnd, lower) > 0) lower = previousEnd;
        }
        CMItemCount first = (CMItemCount)fmax(0, ceil(CMTimeGetSeconds(CMTimeSubtract(lower, pts)) * format->mSampleRate - 0.000001));
        CMItemCount end = count;
        if (CMTIME_IS_NUMERIC(upper))
            end = (CMItemCount)fmax(0, floor(CMTimeGetSeconds(CMTimeSubtract(upper, pts)) * format->mSampleRate));
        if (end > count) end = count;
        if (first >= end) return;
        if (audioMixer) {
            CMTime firstPTS = CMTimeAdd(pts, CMTimeMake(first, (int32_t)llround(format->mSampleRate)));
            CMTime active = CMTimeSubtract(CMTimeSubtract(firstPTS, pausedTotal), origin);
            if (!CMTIME_IS_NUMERIC(active)) return;
            int64_t frame = CMTimeConvertScale(active, SRMixRate, kCMTimeRoundingMethod_Default).value;
            if (![audioMixer enqueueSample:sample microphone:microphone atFrame:frame first:first count:end-first]) {
                [self fail:SREncoder];
                return;
            }
            if (!finishing) {
                CMTime safe = CMTimeSubtract(SRNow(), CMTimeMake(SRMixLatencyMs, 1000));
                if (paused && CMTimeCompare(safe, pausedAt) > 0) safe = pausedAt;
                [self appendMixedAudioThrough:safe final:NO];
                if (audioMixer.latestFrame - audioMixer.nextFrame > (int64_t)SRMixRate * 2) [self fail:SREncoder];
            }
            return;
        }
        CMSampleBufferRef clipped = NULL;
        if (first || end != count) {
            clipped = SRCopyAudioRange(sample, first, end - first);
            if (!clipped) { [self fail:SREncoder]; return; }
            sample = clipped;
        }
        CMSampleBufferRef adjusted = SRCopyTiming(sample, pausedTotal, kCMTimeInvalid);
        if (clipped) CFRelease(clipped);
        if (!adjusted) { [self fail:SREncoder]; return; }
        CMTime adjustedPTS = CMSampleBufferGetPresentationTimeStamp(adjusted);
        if (!audioInput.readyForMoreMediaData || ![audioInput appendSampleBuffer:adjusted]) {
            CFRelease(adjusted);
            [self fail:SRError(writer.error, SREncoder)];
            return;
        }
        CMTime duration = CMSampleBufferGetDuration(adjusted);
        lastAudioEnd = CMTIME_IS_NUMERIC(duration) ? CMTimeAdd(adjustedPTS, duration) : adjustedPTS;
        CFRelease(adjusted);
    }
}

- (void)captureOutput:(AVCaptureOutput *)output didOutputSampleBuffer:(CMSampleBufferRef)sampleBuffer
       fromConnection:(AVCaptureConnection *)connection {
    (void)output;
    (void)connection;
    CFRetain(sampleBuffer);
    dispatch_async(queue, ^{
        [self appendAudioSampleBuffer:sampleBuffer microphone:YES];
        CFRelease(sampleBuffer);
    });
}

- (void)stream:(SCStream *)capture didOutputSampleBuffer:(CMSampleBufferRef)sample ofType:(SCStreamOutputType)type {
    (void)capture;
    @autoreleasepool {
        if (type != SCStreamOutputTypeScreen) {
            if (@available(macOS 13.0, *)) {
                if (type == SCStreamOutputTypeAudio) [self appendAudioSampleBuffer:sample microphone:NO];
            }
            return;
        }
        if (finished || finishing || errorCode || !CMSampleBufferIsValid(sample) || !CMSampleBufferDataIsReady(sample)) return;
        CMTime pts = CMSampleBufferGetPresentationTimeStamp(sample);
        if (!CMTIME_IS_NUMERIC(pts)) return;
        if (paused && (!CMTIME_IS_NUMERIC(origin) || CMTimeCompare(pts, pausedAt) >= 0)) return;
        CFArrayRef attachments = CMSampleBufferGetSampleAttachmentsArray(sample, false);
        if (!attachments || CFArrayGetCount(attachments) == 0) return;
        NSDictionary *info = (__bridge NSDictionary *)CFArrayGetValueAtIndex(attachments, 0);
        NSNumber *status = info[SCStreamFrameInfoStatus];
        if (!status || status.integerValue != SCFrameStatusComplete || !CMSampleBufferGetImageBuffer(sample)) return;
        if (CMTIME_IS_NUMERIC(intervalStart) && CMTimeCompare(pts, intervalStart) < 0) return;
        if (!CMTIME_IS_NUMERIC(origin)) {
            if (![writer startWriting]) { [self fail:SRError(writer.error, SREncoder)]; return; }
            origin = pts;
            [writer startSessionAtSourceTime:origin];
        }
        if (!videoInput.readyForMoreMediaData) return;
        CMSampleBufferRef adjusted = SRCopyTiming(sample, pausedTotal, frameDuration);
        if (!adjusted) { [self fail:SREncoder]; return; }
        CMTime adjustedPTS = CMSampleBufferGetPresentationTimeStamp(adjusted);
        if (CMTIME_IS_NUMERIC(lastVideoPTS) && CMTimeCompare(adjustedPTS, lastVideoPTS) <= 0) {
            CFRelease(adjusted);
            return;
        }
        if (!videoInput.readyForMoreMediaData || ![videoInput appendSampleBuffer:adjusted]) {
            CFRelease(adjusted);
            [self fail:SRError(writer.error, SREncoder)];
            return;
        }
        if (lastVideo) CFRelease(lastVideo);
        lastVideo = adjusted;
        lastVideoPTS = adjustedPTS;
    }
}
@end

int32_t sr_supported(void) {
    if (@available(macOS 12.3, *)) return NSClassFromString(@"SCStream") != nil;
    return 0;
}
int32_t sr_audio_supported(void) {
    if (@available(macOS 13.0, *)) return sr_supported();
    return 0;
}

int32_t sr_microphone_supported(void) {
    if (@available(macOS 10.14, *)) {
        if (!sr_supported()) return 0;
        AVAuthorizationStatus status = [AVCaptureDevice authorizationStatusForMediaType:AVMediaTypeAudio];
        return status != AVAuthorizationStatusDenied && status != AVAuthorizationStatusRestricted &&
            [AVCaptureDevice defaultDeviceWithMediaType:AVMediaTypeAudio] != nil;
    }
    return 0;
}

int32_t sr_start(uint32_t displayID, double scale, uint32_t x, uint32_t y, uint32_t width, uint32_t height,
                 uint32_t outputWidth, uint32_t outputHeight, uint32_t fps, uint32_t bitrate,
                 int32_t cursor, int32_t audio, int32_t microphone, const char *path, void **handle) {
    @autoreleasepool {
        if (!handle || !path) return SREncoder;
        *handle = NULL;
        if (@available(macOS 12.3, *)) {
            if (!sr_supported()) return SRUnsupported;
            if (!CGPreflightScreenCaptureAccess()) return SRPermission;
            if ((audio && !sr_audio_supported()) || (microphone && !sr_microphone_supported())) return SRPermission;
            if (!isfinite(scale) || scale <= 0 || !width || !height || outputWidth < 2 || outputHeight < 2 ||
                (outputWidth & 1) || (outputHeight & 1) || !fps || !bitrate) return SREncoder;
            NSString *name = [[NSString alloc] initWithUTF8String:path];
            if (!name) return SRDisk;
            SRRecorder *session = [SRRecorder new];
            session->url = [NSURL fileURLWithPath:name];
            session->frameDuration = CMTimeMake(1, (int32_t)fps);
            *handle = (__bridge_retained void *)session;
            dispatch_async(session->queue, ^{
                [SCShareableContent getShareableContentExcludingDesktopWindows:NO onScreenWindowsOnly:NO completionHandler:^(SCShareableContent *content, NSError *error) {
                    dispatch_async(session->queue, ^{
                        if (session->finished || session->finishing) return;
                        if (error || !content) { [session fail:SRError(error, SRPermission)]; return; }
                        SCDisplay *display = nil;
                        for (SCDisplay *candidate in content.displays) if (candidate.displayID == displayID) { display = candidate; break; }
                        if (!display) { [session fail:SREncoder]; return; }
                        CGRect source = CGRectMake(x / scale, y / scale, width / scale, height / scale);
                        if (CGRectGetMaxX(source) > display.frame.size.width + 0.5 || CGRectGetMaxY(source) > display.frame.size.height + 0.5) { [session fail:SREncoder]; return; }
                        NSMutableArray<SCRunningApplication *> *excluded = [NSMutableArray array];
                        for (SCRunningApplication *app in content.applications) if (app.processID == getpid()) [excluded addObject:app];
                        // Fail closed: never accidentally capture our own red border/controls.
                        if (excluded.count == 0) { [session fail:SREncoder]; return; }
                        SCContentFilter *filter = [[SCContentFilter alloc] initWithDisplay:display excludingApplications:excluded exceptingWindows:@[]];
                        SCStreamConfiguration *configuration = [SCStreamConfiguration new];
                        configuration.sourceRect = source;
                        configuration.width = outputWidth;
                        configuration.height = outputHeight;
                        configuration.minimumFrameInterval = session->frameDuration;
                        configuration.showsCursor = cursor != 0;
                        configuration.pixelFormat = kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange;
                        configuration.queueDepth = 5;
                        BOOL captureSystemAudio = NO;
                        BOOL captureMicrophone = microphone != 0;
                        if (@available(macOS 13.0, *)) {
                            captureSystemAudio = audio != 0;
                            configuration.capturesAudio = captureSystemAudio;
                            configuration.excludesCurrentProcessAudio = YES;
                            configuration.sampleRate = 48000;
                            configuration.channelCount = 2;
                        }
                        void (^build)(void) = ^{
                        if (session->finished || session->finishing) return;
                        NSError *setupError = nil;
                        session->writer = [[AVAssetWriter alloc] initWithURL:session->url fileType:AVFileTypeMPEG4 error:&setupError];
                        if (!session->writer) { [session fail:SRError(setupError, SRDisk)]; return; }
                        session->ownsOutput = YES;
                        NSDictionary *videoSettings = @{AVVideoCodecKey: AVVideoCodecTypeH264,
                            AVVideoWidthKey: @(outputWidth), AVVideoHeightKey: @(outputHeight),
                            AVVideoCompressionPropertiesKey: @{AVVideoAverageBitRateKey: @(bitrate),
                                AVVideoProfileLevelKey: AVVideoProfileLevelH264HighAutoLevel,
                                AVVideoExpectedSourceFrameRateKey: @(fps)}};
                        session->videoInput = [AVAssetWriterInput assetWriterInputWithMediaType:AVMediaTypeVideo outputSettings:videoSettings];
                        session->videoInput.expectsMediaDataInRealTime = YES;
                        if (![session->writer canAddInput:session->videoInput]) { [session fail:SREncoder]; return; }
                        [session->writer addInput:session->videoInput];
                        if (captureSystemAudio || captureMicrophone) {
                            NSDictionary *audioSettings = @{AVFormatIDKey: @(kAudioFormatMPEG4AAC),
                                AVSampleRateKey: @48000, AVNumberOfChannelsKey: @2, AVEncoderBitRateKey: @160000};
                            session->audioInput = [AVAssetWriterInput assetWriterInputWithMediaType:AVMediaTypeAudio outputSettings:audioSettings];
                            session->audioInput.expectsMediaDataInRealTime = YES;
                            if (![session->writer canAddInput:session->audioInput]) { [session fail:SREncoder]; return; }
                            [session->writer addInput:session->audioInput];
                            session->systemAudio = captureSystemAudio;
                            if (captureSystemAudio && captureMicrophone) session->audioMixer = [SRAudioMixer new];
                        }
                        if (captureMicrophone) {
                            AVCaptureDevice *device = [AVCaptureDevice defaultDeviceWithMediaType:AVMediaTypeAudio];
                            if (!device) { [session fail:SRPermission]; return; }
                            AVCaptureDeviceInput *input = [AVCaptureDeviceInput deviceInputWithDevice:device error:&setupError];
                            if (!input) { [session fail:SRError(setupError, SRPermission)]; return; }
                            session->micSession = [AVCaptureSession new];
                            if ([session->micSession canAddInput:input]) [session->micSession addInput:input];
                            else { [session fail:SREncoder]; return; }
                            session->micOutput = [AVCaptureAudioDataOutput new];
                            [session->micOutput setSampleBufferDelegate:session queue:session->audioQueue];
                            if ([session->micSession canAddOutput:session->micOutput]) [session->micSession addOutput:session->micOutput];
                            else { [session fail:SREncoder]; return; }
                            [session->micSession startRunning];
                            if (!session->micSession.isRunning) { [session fail:SREncoder]; return; }
                            if (session->audioMixer) {
                                CMClockRef clock = session->micSession.synchronizationClock;
                                if (clock) session->micClock = (CMClockRef)CFRetain(clock);
                            }
                        }
                        session->stream = [[SCStream alloc] initWithFilter:filter configuration:configuration delegate:session];
                        if (![session->stream addStreamOutput:session type:SCStreamOutputTypeScreen sampleHandlerQueue:session->queue error:&setupError]) { [session fail:SRError(setupError, SREncoder)]; return; }
                        if (@available(macOS 13.0, *)) {
                            if (captureSystemAudio && ![session->stream addStreamOutput:session type:SCStreamOutputTypeAudio sampleHandlerQueue:session->queue error:&setupError]) { [session fail:SRError(setupError, SREncoder)]; return; }
                        }
                        session->starting = YES;
                        SCStream *startingStream = session->stream;
                        [startingStream startCaptureWithCompletionHandler:^(NSError *startError) {
                            dispatch_async(session->queue, ^{
                                session->starting = NO;
                                if (session->finished) { [startingStream stopCaptureWithCompletionHandler:^(NSError *ignored) { (void)ignored; }]; return; }
                                session->captureStarted = startError == nil;
                                if (startError) [session fail:SRError(startError, SREncoder)];
                                if (session->finishing) [session stopCapture];
                            });
                        }];
                        // Count the first-frame timeout from the moment capture
                        // is asked to start, not from permission or enumeration.
                        dispatch_after(dispatch_time(DISPATCH_TIME_NOW, 15 * NSEC_PER_SEC), session->queue, ^{
                            if (!session->finished && !session->finishing && !session->lastVideo) [session fail:SREncoder];
                        });
                        };
                        if (captureMicrophone) {
                            AVAuthorizationStatus status = [AVCaptureDevice authorizationStatusForMediaType:AVMediaTypeAudio];
                            if (status == AVAuthorizationStatusDenied || status == AVAuthorizationStatusRestricted) {
                                [session fail:SRPermission];
                                return;
                            }
                            if (status != AVAuthorizationStatusAuthorized) {
                                // Ask off this queue so pause/stop/status are not stuck behind the dialog.
                                [AVCaptureDevice requestAccessForMediaType:AVMediaTypeAudio completionHandler:^(BOOL allowed) {
                                    dispatch_async(session->queue, ^{
                                        if (session->finished || session->finishing) return;
                                        if (!allowed) { [session fail:SRPermission]; return; }
                                        build();
                                    });
                                }];
                                return;
                            }
                        }
                        build();
                    });
                }];
            });
            return SROk;
        }
        return SRUnsupported;
    }
}

int32_t sr_status(void *handle, uint32_t *state, uint64_t *elapsed, int32_t *error) {
    @autoreleasepool {
        if (!handle || !state || !elapsed || !error) return SREncoder;
        if (@available(macOS 12.3, *)) {
            SRRecorder *session = (__bridge SRRecorder *)handle;
            SRReply *reply = [SRReply new];
            dispatch_async(session->queue, ^{
                reply->state = session->state; reply->elapsed = [session elapsed]; reply->code = session->errorCode;
                dispatch_semaphore_signal(reply->gate);
            });
            if (dispatch_semaphore_wait(reply->gate, dispatch_time(DISPATCH_TIME_NOW, 5 * NSEC_PER_SEC))) return SREncoder;
            *state = reply->state; *elapsed = reply->elapsed; *error = reply->code;
            return SROk;
        }
        return SRUnsupported;
    }
}

int32_t sr_pause(void *handle, int32_t paused) {
    @autoreleasepool {
        if (!handle) return SREncoder;
        if (@available(macOS 12.3, *)) {
            SRRecorder *session = (__bridge SRRecorder *)handle;
            SRReply *reply = [SRReply new];
            dispatch_async(session->queue, ^{
                if (session->finished || session->finishing || session->errorCode || !session->lastVideo) reply->code = session->errorCode ?: SREncoder;
                else if (session->paused != (paused != 0)) {
                    CMTime now = SRNow();
                    if (paused) session->pausedAt = now;
                    else {
                        if (CMTIME_IS_NUMERIC(session->origin)) session->pausedTotal = CMTimeAdd(session->pausedTotal, CMTimeSubtract(now, session->pausedAt));
                        session->intervalStart = now;
                    }
                    session->paused = paused != 0;
                    session->state = paused ? 1 : 0;
                }
                dispatch_semaphore_signal(reply->gate);
            });
            if (dispatch_semaphore_wait(reply->gate, dispatch_time(DISPATCH_TIME_NOW, 5 * NSEC_PER_SEC))) return SREncoder;
            return reply->code;
        }
        return SRUnsupported;
    }
}

int32_t sr_finish(void *handle, int32_t cancel, uint64_t *elapsed) {
    @autoreleasepool {
        if (!handle || !elapsed || [NSThread isMainThread]) return SREncoder;
        if (@available(macOS 12.3, *)) {
            SRRecorder *session = (__bridge SRRecorder *)handle;
            SRReply *reply = [SRReply new];
            dispatch_async(session->queue, ^{
                [session requestFinish:cancel != 0 completion:^(int code, uint64_t duration) {
                    reply->code = cancel && code != SRDisk ? SROk : code;
                    reply->elapsed = duration;
                    dispatch_semaphore_signal(reply->gate);
                }];
            });
            if (dispatch_semaphore_wait(reply->gate, dispatch_time(DISPATCH_TIME_NOW, 35 * NSEC_PER_SEC))) return SREncoder;
            *elapsed = reply->elapsed;
            return reply->code;
        }
        return SRUnsupported;
    }
}

void sr_release(void *handle) {
    @autoreleasepool {
        if (!handle) return;
        if (@available(macOS 12.3, *)) {
            SRRecorder *session = (__bridge_transfer SRRecorder *)handle;
            dispatch_async(session->queue, ^{ [session requestFinish:YES completion:nil]; });
        }
    }
}

int32_t sr_copy_file(const char *path) {
    @autoreleasepool {
        if (!path || ![NSThread isMainThread]) return SREncoder;
        NSString *name = [[NSString alloc] initWithUTF8String:path];
        if (!name || ![[NSFileManager defaultManager] fileExistsAtPath:name]) return SRDisk;
        NSPasteboard *pasteboard = [NSPasteboard generalPasteboard];
        [pasteboard clearContents];
        return [pasteboard writeObjects:@[[NSURL fileURLWithPath:name]]] ? SROk : SREncoder;
    }
}
int32_t sr_reveal(const char *path) {
    @autoreleasepool {
        if (!path || ![NSThread isMainThread]) return SREncoder;
        NSString *name = [[NSString alloc] initWithUTF8String:path];
        if (!name || ![[NSFileManager defaultManager] fileExistsAtPath:name]) return SRDisk;
        [[NSWorkspace sharedWorkspace] activateFileViewerSelectingURLs:@[[NSURL fileURLWithPath:name]]];
        return SROk;
    }
}
