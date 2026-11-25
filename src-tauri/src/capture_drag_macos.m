#import <AppKit/AppKit.h>

@interface SnapshootFileDragSource : NSObject <NSDraggingSource>
@end

@implementation SnapshootFileDragSource
- (NSDragOperation)draggingSession:(NSDraggingSession *)session
    sourceOperationMaskForDraggingContext:(NSDraggingContext)context {
    (void)session;
    (void)context;
    return NSDragOperationCopy;
}
@end

int snapshoot_drag_file(void *rawWindow, const char *path);

int snapshoot_drag_file(void *rawWindow, const char *path) {
    if (!NSThread.isMainThread || !rawWindow || !path || !(NSEvent.pressedMouseButtons & 1)) {
        return 0;
    }
    NSWindow *window = (__bridge NSWindow *)rawWindow;
    NSView *view = window.contentView;
    if (!view || !window.isVisible) return 0;
    NSString *name = [NSString stringWithUTF8String:path];
    if (!name) return 0;

    NSPoint location = window.mouseLocationOutsideOfEventStream;
    // The webview's asynchronous IPC can arrive after the NSEvent that moved
    // the pointer. Start from the live pointer in the source window instead.
    NSEvent *event = NSApp.currentEvent;
    if (event.window != window || (event.type != NSEventTypeLeftMouseDown &&
        event.type != NSEventTypeLeftMouseDragged)) {
        event = [NSEvent mouseEventWithType:NSEventTypeLeftMouseDragged
            location:location modifierFlags:NSEvent.modifierFlags
            timestamp:NSProcessInfo.processInfo.systemUptime windowNumber:window.windowNumber
            context:nil eventNumber:0 clickCount:1 pressure:1];
    }
    if (!event) return 0;
    NSPoint origin = [view convertPoint:location fromView:nil];
    NSDraggingItem *item = [[NSDraggingItem alloc] initWithPasteboardWriter:[NSURL fileURLWithPath:name]];
    NSImage *icon = [NSWorkspace.sharedWorkspace iconForFile:name];
    [item setDraggingFrame:NSMakeRect(origin.x - 18, origin.y - 18, 36, 36) contents:icon];
    static SnapshootFileDragSource *source;
    if (!source) source = [SnapshootFileDragSource new];
    NSDraggingSession *session = [view beginDraggingSessionWithItems:@[item] event:event source:source];
    return session != nil;
}
