#import <Foundation/Foundation.h>
#import <Vision/Vision.h>
#import <ImageIO/ImageIO.h>
#include <stdlib.h>
#include <string.h>

static char *copy_utf8_json(id object) {
    NSError *error = nil;
    NSData *data = [NSJSONSerialization dataWithJSONObject:object options:0 error:&error];
    if (data == nil) {
        return NULL;
    }
    char *result = malloc(data.length + 1);
    if (result == NULL) {
        return NULL;
    }
    memcpy(result, data.bytes, data.length);
    result[data.length] = '\0';
    return result;
}

static NSDictionary *box_json(CGRect box, size_t width, size_t height) {
    // Vision uses a bottom-left, normalized coordinate space.
    double x = box.origin.x * width;
    double y = (1.0 - box.origin.y - box.size.height) * height;
    return @{
        @"x": @(x),
        @"y": @(y),
        @"width": @(box.size.width * width),
        @"height": @(box.size.height * height),
    };
}

static const char *recognize(const unsigned char *bytes, size_t length, const char *language, int *status)
    API_AVAILABLE(macos(10.15)) {
    if (bytes == NULL || length == 0) {
        return NULL;
    }
    @autoreleasepool {
        NSData *image_data = [NSData dataWithBytes:bytes length:length];
        CGImageSourceRef source = CGImageSourceCreateWithData((__bridge CFDataRef)image_data, NULL);
        if (source == NULL) {
            return NULL;
        }
        CGImageRef image = CGImageSourceCreateImageAtIndex(source, 0, NULL);
        CFRelease(source);
        if (image == NULL) {
            return NULL;
        }

        size_t width = CGImageGetWidth(image);
        size_t height = CGImageGetHeight(image);
        VNImageRequestHandler *handler =
            [[VNImageRequestHandler alloc] initWithCGImage:image options:@{}];
        CFRelease(image);

        VNRecognizeTextRequest *text_request = [[VNRecognizeTextRequest alloc] init];
        text_request.recognitionLevel = VNRequestTextRecognitionLevelAccurate;
        text_request.usesLanguageCorrection = YES;
        BOOL auto_language = language == NULL || strcmp(language, "auto") == 0;
        if (@available(macOS 13.0, *)) {
            text_request.automaticallyDetectsLanguage = auto_language;
        }
        if (!auto_language) {
            NSString *requested = [NSString stringWithUTF8String:language];
            NSArray<NSString *> *supported;
            if (@available(macOS 12.0, *)) {
                supported = [text_request supportedRecognitionLanguagesAndReturnError:NULL];
            } else {
                // Older Vision versions use the class method for the same query.
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wdeprecated-declarations"
                supported = [VNRecognizeTextRequest
                    supportedRecognitionLanguagesForTextRecognitionLevel:text_request.recognitionLevel
                    revision:text_request.revision
                    error:NULL];
#pragma clang diagnostic pop
            }
            if (requested == nil || ![supported containsObject:requested]) {
                *status = 1;
                return NULL;
            }
            text_request.recognitionLanguages = @[requested];
        }

        VNDetectBarcodesRequest *barcode_request = [[VNDetectBarcodesRequest alloc] init];
        NSError *error = nil;
        if (![handler performRequests:@[ text_request, barcode_request ] error:&error]) {
            return NULL;
        }

        NSMutableArray *lines = [NSMutableArray array];
        NSMutableArray *text_parts = [NSMutableArray array];
        for (VNRecognizedTextObservation *observation in text_request.results) {
            VNRecognizedText *candidate = [[observation topCandidates:1] firstObject];
            if (candidate == nil || candidate.string.length == 0) {
                continue;
            }
            NSMutableDictionary *line = [box_json(observation.boundingBox, width, height) mutableCopy];
            line[@"text"] = candidate.string;
            line[@"confidence"] = @(candidate.confidence);
            [lines addObject:line];
            [text_parts addObject:candidate.string];
        }

        NSMutableArray *barcodes = [NSMutableArray array];
        for (VNBarcodeObservation *observation in barcode_request.results) {
            NSString *payload = observation.payloadStringValue;
            if (payload.length == 0) {
                continue;
            }
            NSMutableDictionary *barcode = [box_json(observation.boundingBox, width, height) mutableCopy];
            barcode[@"text"] = payload;
            barcode[@"symbology"] = observation.symbology ?: @"";
            [barcodes addObject:barcode];
        }

        NSDictionary *result = @{
            @"width": @(width),
            @"height": @(height),
            @"text": [text_parts componentsJoinedByString:@"\n"],
            @"lines": lines,
            @"barcodes": barcodes,
        };
        return copy_utf8_json(result);
    }
}

const char *snapshoot_vision_ocr(const unsigned char *bytes, size_t length, const char *language, int *status) {
    *status = 0;
    if (@available(macOS 10.15, *)) {
        return recognize(bytes, length, language, status);
    }
    return NULL;
}

void snapshoot_vision_ocr_free(char *value) {
    if (value != NULL) {
        free(value);
    }
}
