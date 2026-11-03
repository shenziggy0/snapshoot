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
@interface SRRecorder : NSObject <SCStreamOutput, SCStreamDelegate> {
@public
    dispatch_queue_t queue;
    uint32_t state;
    int errorCode;
    uint64_t finalElapsed;
    SCStream *stream;
    AVAssetWriter *writer;
    AVAssetWriterInput *videoInput;
    AVAssetWriterInput *audioInput;
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
- (void)requestFinish:(BOOL)cancel completion:(void (^)(int, uint64_t))completion;
- (void)stopCapture;
- (void)finishWriter;
@end

@implementation SRRecorder
- (instancetype)init {
    if ((self = [super init])) {
        queue = dispatch_queue_create("top.shenziggy.snapshoot.screen-record",
            dispatch_queue_attr_make_with_autorelease_frequency(DISPATCH_QUEUE_SERIAL, DISPATCH_AUTORELEASE_FREQUENCY_WORK_ITEM));
        origin = pausedAt = intervalStart = finishAt = lastVideoPTS = lastAudioEnd = kCMTimeInvalid;
        pausedTotal = kCMTimeZero;
        completions = [NSMutableArray array];
    }
    return self;
}
- (void)dealloc { if (lastVideo) CFRelease(lastVideo); }
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
            if (audioInput) [stream removeStreamOutput:self type:SCStreamOutputTypeAudio error:NULL];
        }
    }
    stream = nil;
    writer = nil;
    videoInput = nil;
    audioInput = nil;
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
- (void)stream:(SCStream *)capture didOutputSampleBuffer:(CMSampleBufferRef)sample ofType:(SCStreamOutputType)type {
    (void)capture;
    @autoreleasepool {
        if (finished || finishing || errorCode || !CMSampleBufferIsValid(sample) || !CMSampleBufferDataIsReady(sample)) return;
        BOOL video = type == SCStreamOutputTypeScreen;
        if (!video) {
            if (@available(macOS 13.0, *)) { if (type != SCStreamOutputTypeAudio || !audioInput) return; }
            else return;
        }
        CMTime pts = CMSampleBufferGetPresentationTimeStamp(sample);
        if (!CMTIME_IS_NUMERIC(pts)) return;
        if (paused && (!CMTIME_IS_NUMERIC(origin) || CMTimeCompare(pts, pausedAt) >= 0)) return;
        if (video) {
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
        } else if (!CMTIME_IS_NUMERIC(origin)) return;

        CMSampleBufferRef clipped = NULL;
        if (!video) {
            const AudioStreamBasicDescription *format = CMAudioFormatDescriptionGetStreamBasicDescription(CMSampleBufferGetFormatDescription(sample));
            if (!format || format->mSampleRate <= 0) { [self fail:SREncoder]; return; }
            CMItemCount count = CMSampleBufferGetNumSamples(sample);
            CMTime lower = CMTIME_IS_NUMERIC(intervalStart) && CMTimeCompare(intervalStart, origin) > 0 ? intervalStart : origin;
            if (CMTIME_IS_NUMERIC(lastAudioEnd)) {
                CMTime previousEnd = CMTimeAdd(lastAudioEnd, pausedTotal);
                if (CMTimeCompare(previousEnd, lower) > 0) lower = previousEnd;
            }
            CMItemCount first = (CMItemCount)fmax(0, ceil(CMTimeGetSeconds(CMTimeSubtract(lower, pts)) * format->mSampleRate - 0.000001));
            CMItemCount end = count;
            if (paused) end = (CMItemCount)fmax(0, floor(CMTimeGetSeconds(CMTimeSubtract(pausedAt, pts)) * format->mSampleRate));
            if (end > count) end = count;
            if (first >= end) return;
            if (first || end != count) {
                clipped = SRCopyAudioRange(sample, first, end - first);
                if (!clipped) { [self fail:SREncoder]; return; }
                sample = clipped;
            }
        }
        CMSampleBufferRef adjusted = SRCopyTiming(sample, pausedTotal, video ? frameDuration : kCMTimeInvalid);
        if (clipped) CFRelease(clipped);
        if (!adjusted) { [self fail:SREncoder]; return; }
        CMTime adjustedPTS = CMSampleBufferGetPresentationTimeStamp(adjusted);
        if (video && CMTIME_IS_NUMERIC(lastVideoPTS) && CMTimeCompare(adjustedPTS, lastVideoPTS) <= 0) { CFRelease(adjusted); return; }
        AVAssetWriterInput *input = video ? videoInput : audioInput;
        if (!input.readyForMoreMediaData || ![input appendSampleBuffer:adjusted]) {
            CFRelease(adjusted);
            [self fail:SRError(writer.error, SREncoder)];
            return;
        }
        if (video) {
            if (lastVideo) CFRelease(lastVideo);
            lastVideo = adjusted;
            lastVideoPTS = adjustedPTS;
        } else {
            CMTime duration = CMSampleBufferGetDuration(adjusted);
            lastAudioEnd = CMTIME_IS_NUMERIC(duration) ? CMTimeAdd(adjustedPTS, duration) : adjustedPTS;
            CFRelease(adjusted);
        }
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

int32_t sr_start(uint32_t displayID, double scale, uint32_t x, uint32_t y, uint32_t width, uint32_t height,
                 uint32_t outputWidth, uint32_t outputHeight, uint32_t fps, uint32_t bitrate,
                 int32_t cursor, int32_t audio, const char *path, void **handle) {
    @autoreleasepool {
        if (!handle || !path) return SREncoder;
        *handle = NULL;
        if (@available(macOS 12.3, *)) {
            if (!sr_supported()) return SRUnsupported;
            if (!CGPreflightScreenCaptureAccess()) return SRPermission;
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
                        BOOL captureAudio = NO;
                        if (@available(macOS 13.0, *)) {
                            captureAudio = audio != 0;
                            configuration.capturesAudio = captureAudio;
                            configuration.excludesCurrentProcessAudio = YES;
                            configuration.sampleRate = 48000;
                            configuration.channelCount = 2;
                        }
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
                        if (captureAudio) {
                            NSDictionary *audioSettings = @{AVFormatIDKey: @(kAudioFormatMPEG4AAC),
                                AVSampleRateKey: @48000, AVNumberOfChannelsKey: @2, AVEncoderBitRateKey: @160000};
                            session->audioInput = [AVAssetWriterInput assetWriterInputWithMediaType:AVMediaTypeAudio outputSettings:audioSettings];
                            session->audioInput.expectsMediaDataInRealTime = YES;
                            if (![session->writer canAddInput:session->audioInput]) { [session fail:SREncoder]; return; }
                            [session->writer addInput:session->audioInput];
                        }
                        session->stream = [[SCStream alloc] initWithFilter:filter configuration:configuration delegate:session];
                        if (![session->stream addStreamOutput:session type:SCStreamOutputTypeScreen sampleHandlerQueue:session->queue error:&setupError]) { [session fail:SRError(setupError, SREncoder)]; return; }
                        if (@available(macOS 13.0, *)) {
                            if (captureAudio && ![session->stream addStreamOutput:session type:SCStreamOutputTypeAudio sampleHandlerQueue:session->queue error:&setupError]) { [session fail:SRError(setupError, SREncoder)]; return; }
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
                    });
                }];
                dispatch_after(dispatch_time(DISPATCH_TIME_NOW, 15 * NSEC_PER_SEC), session->queue, ^{
                    if (!session->finished && !session->finishing && !session->lastVideo) [session fail:SREncoder];
                });
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
