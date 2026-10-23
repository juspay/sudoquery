#import <Foundation/Foundation.h>

NS_ASSUME_NONNULL_BEGIN

/// Uses every Objective-C method of the SDK, so building the sample checks the generated header,
/// and tracks an `objc_smoke` event. The results are logged with NSLog.
void SQSampleRunObjCSmokeTest(void);

NS_ASSUME_NONNULL_END
