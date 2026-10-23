#import "ObjCSmokeTest.h"

@import SudoQuery;

void SQSampleRunObjCSmokeTest(void) {
    // An invalid config is reported as an NSError.
    SQSudoQueryConfig *invalid = [[SQSudoQueryConfig alloc] initWithOrgId:@"acme-k3x9qa"
                                                              workspaceId:@"ios-demo1"
                                                                 endpoint:@"ftp://nowhere"];
    NSError *error = nil;
    BOOL started = [SQSudoQuery initializeWithConfig:invalid error:&error];
    NSLog(@"SudoQuery ObjC smoke: invalid config started=%d domain=%@ code=%ld (expected %ld)",
          started, error.domain, (long)error.code, (long)SQSudoQueryConfigErrorCodeInvalidEndpoint);

    // Every option. The app already initialized the SDK from Swift, so this call is ignored.
    SQSudoQueryConfig *config = [[SQSudoQueryConfig alloc] initWithOrgId:@"acme-k3x9qa"
                                                             workspaceId:@"ios-demo1"
                                                                endpoint:@"http://localhost:3001/v1/events/batch"];
    config.token = nil;
    config.headers = @{};
    config.source = @"ios";
    config.sessionId = nil;
    config.batchSize = 20;
    config.flushInterval = 10;
    config.sessionTimeout = 30 * 60;
    config.debugLogging = YES;
    if (![SQSudoQuery initializeWithConfig:config error:&error]) {
        NSLog(@"SudoQuery ObjC smoke: %@", error.localizedDescription);
    }

    [SQSudoQuery setSuperProperty:@"objc" value:@YES];
    [SQSudoQuery track:@"objc_smoke"
            properties:@{@"flag": @YES, @"count": @1, @"ratio": @0.5, @"none": NSNull.null, @2: @"numeric key"}];
    [SQSudoQuery track:@"objc_smoke_plain"];
    [SQSudoQuery setUser:@"objc_user"];
    NSLog(@"SudoQuery ObjC smoke: user=%@ initialized=%d version=%@ errorDomain=%@",
          SQSudoQuery.user, SQSudoQuery.isInitialized, SQSudoQuery.sdkVersion, SQSudoQueryConfig.errorDomain);
    [SQSudoQuery removeUser];
    NSLog(@"SudoQuery ObjC smoke: superProperties=%@", SQSudoQuery.superProperties);
    [SQSudoQuery clearSuperProperties];
    [SQSudoQuery flush];

    // Compiled but not run: a smoke test shouldn't log the app out.
    void (^logout)(void) = ^{
        [SQSudoQuery reset];
    };
    (void)logout;
}
