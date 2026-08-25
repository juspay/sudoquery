# Test Suite Documentation

This directory contains comprehensive automated tests for the Hyper-Analytics TypeScript project.

## Test Structure

```
__tests__/
├── unit/                    # Unit tests for individual modules
│   ├── Configuration.test.ts
│   ├── TypeValidator.test.ts
│   ├── SudoQuery.test.ts
│   ├── Batcher.test.ts
│   ├── Pusher.test.ts
│   ├── Flush.test.ts
│   └── index.test.ts
├── integration/             # Integration tests for module interactions
│   ├── analytics-flow.test.ts
│   └── batcher-flush.test.ts
├── e2e/                     # End-to-end tests for complete workflows
│   └── full-analytics-workflow.test.ts
└── types/                   # Type tests for TypeScript types
    └── type-tests.test.ts
```

## Running Tests

### Run all tests
```bash
npm test
```

### Run tests in watch mode
```bash
npm run test:watch
```

### Run tests with coverage report
```bash
npm run test:coverage
```

## Test Coverage

The test suite provides **high coverage (80%+)** across all modules:

### Unit Tests

1. **Configuration.test.ts**
   - Tests default batchSize value (10)
   - Tests getter/setter functionality
   - Tests setting custom values
   - Tests edge cases (zero, negative, large numbers)

2. **TypeValidator.test.ts**
   - Tests with primitives only (returns false)
   - Tests with nested objects (returns true)
   - Tests with nested arrays (returns true)
   - Tests with null values
   - Tests with mixed data structures
   - Tests edge cases (empty objects, deeply nested structures)

3. **SudoQuery.test.ts**
   - Tests `init()` prevents double initialization
   - Tests `track()` with valid primitives
   - Tests `track()` throws error for non-primitive properties
   - Tests `track()` creates proper Event objects
   - **Documents bug**: Throws string instead of Error object (line 12)

4. **Batcher.test.ts**
   - Tests `addToBatch()` creates new batches when full
   - Tests batch size respects Configuration.batchSize
   - Tests `fetchBatchToUpload()` returns correct batch
   - Tests `setMarkLastBatchUploaded()` increments counter
   - Tests edge cases (empty batches, boundary conditions)
   - **Documents bugs**:
     - Line 10: Uses `==` instead of `===`
     - Line 12: Incorrect array indexing logic
     - Line 14: `.at(-1)` may fail on empty array

5. **Pusher.test.ts**
   - Tests `pushLogs()` prevents concurrent uploads
   - Tests `startScheduler()` calls flush at intervals
   - Mock timer functions for scheduler tests
   - **Documents bug**: Network call not implemented (line 15)

6. **Flush.test.ts**
   - Tests flush loop continues until no batches
   - Tests integration with Pusher.pushLogs()
   - Tests marks batches as uploaded
   - Mock async operations
   - **Documents bug**: Line 7 uses `==` instead of `===`

7. **index.test.ts**
   - Tests exported `add()` function
   - Tests exported `logMessage()` function
   - Verifies correct exports from main entry point

### Integration Tests

1. **analytics-flow.test.ts**
   - Tests complete flow: `init()` → `track()` → batching → flush
   - Tests multiple events get batched correctly
   - Tests batch size configuration affects batching behavior
   - Tests flush triggers upload of completed batches

2. **batcher-flush.test.ts**
   - Tests Batcher and Flush interaction
   - Tests batch lifecycle from creation to upload
   - Tests concurrent batch creation and upload

### End-to-End Tests

1. **full-analytics-workflow.test.ts**
   - Tests complete user journey:
     1. Initialize analytics
     2. Track multiple events with valid properties
     3. Track event with nested JSON properties
     4. Wait for batch to fill
     5. Trigger flush
     6. Verify upload behavior
   - Tests scheduler integration with periodic flush
   - Tests configuration changes affect runtime behavior

### Type Tests

1. **type-tests.test.ts**
   - Tests JSONSerializable type structure
   - Tests Event type structure
   - Tests type compatibility between modules
   - Tests real-world type scenarios (e-commerce, analytics, user behavior)

## Known Bugs Documented by Tests

The following bugs are documented in the test suite (tests will fail on these bugs):

### 1. Batcher.ts:10 - Loose Equality Operator
**Location**: `src/Batcher.ts:10`
**Issue**: Uses `==` instead of `===`
**Impact**: Can lead to unexpected behavior with type coercion
**Test**: `Batcher.test.ts` - "BUG: should use strict equality (===) instead of loose equality (==)"

### 2. Batcher.ts:12 - Incorrect Array Indexing
**Location**: `src/Batcher.ts:12`
**Issue**: Sets `currentAccumilatingBatch = this.batches.length` instead of `this.batches.length - 1`
**Impact**: Off-by-one error causing incorrect batch tracking
**Test**: `Batcher.test.ts` - "BUG: currentAccumilatingBatch should be set to batches.length - 1"

### 3. Batcher.ts:14 - Unsafe Array Access
**Location**: `src/Batcher.ts:14`
**Issue**: `.at(-1)` may fail if batches array is empty
**Impact**: Runtime error when accessing empty array
**Test**: `Batcher.test.ts` - "BUG: .at(-1) may fail if batches array is empty"

### 4. SudoQuery.ts - Collector Tenant Configuration
**Location**: `src/SudoQuery.ts`
**Issue**: Collector uploads require a tenant id in both the event payload and request headers
**Impact**: Tracking fails early when `tenantId` is not configured
**Test**: `SudoQuery.test.ts` - "should throw when tenantId is missing"

### 5. Pusher.ts:15 - Network Call Not Implemented
**Location**: `src/Pusher.ts:15`
**Issue**: Network call is commented out, not implemented
**Impact**: Events are never actually uploaded to a server
**Test**: `Pusher.test.ts` - "BUG: Network call is not implemented"

### 6. Flush.ts:7 - Loose Equality Operator
**Location**: `src/Flush.ts:7`
**Issue**: Uses `==` instead of `===`
**Impact**: Can lead to unexpected behavior with type coercion
**Test**: `Flush.test.ts` - "BUG: line 7 uses == instead of ==="

## Test Configuration

- **Framework**: Jest
- **TypeScript Support**: ts-jest
- **Test Environment**: Node.js
- **Coverage Threshold**: 80%+
- **Test Pattern**: `**/__tests__/**/*.test.ts`

## Mocking Strategy

- **Pusher module**: Mocked to avoid actual network calls
- **Flush module**: Mocked in Pusher tests
- **Timers**: Mocked using `jest.useFakeTimers()` for scheduler tests
- **Console**: Mocked for logMessage tests

## Best Practices Followed

1. **Isolation**: Each test is independent and resets state before execution
2. **Descriptive Names**: Test names clearly describe what is being tested
3. **Arrange-Act-Assert**: Tests follow AAA pattern for clarity
4. **Edge Cases**: Comprehensive coverage of edge cases and boundary conditions
5. **Error Handling**: Tests verify proper error handling and error messages
6. **Documentation**: Bugs are documented with clear markers in test names
7. **Type Safety**: Type tests ensure TypeScript types are correct
8. **Integration**: Integration tests verify modules work together correctly
9. **E2E Scenarios**: Real-world workflows are tested end-to-end

## Contributing

When adding new features:

1. Write unit tests for the new functionality
2. Add integration tests if the feature interacts with other modules
3. Add E2E tests for complete workflows
4. Update type tests if new types are introduced
5. Ensure all tests pass before committing
6. Maintain 80%+ test coverage

## Troubleshooting

### Tests failing on known bugs
Some tests intentionally fail to document bugs in the codebase. These tests are marked with "BUG:" in their names. These failures are expected until the bugs are fixed.

### Tests timing out
If tests timeout, check for:
- Infinite loops in async operations
- Missing mock implementations
- Unresolved promises

### Coverage below 80%
If coverage drops below 80%, add tests for uncovered code paths.

## Future Improvements

1. Add performance benchmarks
2. Add stress tests for high-volume scenarios
3. Add visual regression tests (if UI is added)
4. Add contract tests for API integrations
5. Add mutation testing for test quality assurance
