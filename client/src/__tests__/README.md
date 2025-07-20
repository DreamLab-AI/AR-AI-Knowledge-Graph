# Settings Integration Tests

This directory contains comprehensive tests for the settings system integration, including type alignment, store functionality, selective hooks, and performance optimizations.

## Test Structure

```
__tests__/
├── settings/
│   ├── typeAlignment.test.ts     # Tests for type consistency between settings.ts and defaultSettings.ts
│   ├── performance.test.ts       # Performance benchmarks and optimization tests
│   └── integration.test.ts       # Full integration tests for the settings system
├── store/
│   └── settingsStore.test.ts     # Tests for the settings store with Immer updates
├── hooks/
│   └── useSelectiveSettingsStore.test.ts  # Tests for selective subscription hooks
└── setup.ts                      # Test environment setup
```

## Running Tests

```bash
# Run all tests
npm test

# Run tests with UI
npm run test:ui

# Run tests with coverage
npm run test:coverage

# Run specific test file
npm test typeAlignment

# Run tests in watch mode
npm test -- --watch
```

## Test Coverage

The test suite covers:

### 1. Type Alignment (`typeAlignment.test.ts`)
- Verifies defaultSettings conforms to Settings interface
- Checks all required properties exist
- Validates correct types for all settings
- Ensures number ranges are valid (opacity, metalness, etc.)
- Tests optional AI service settings

### 2. Settings Store (`settingsStore.test.ts`)
- Initialization from server and local storage
- Get/set operations with path navigation
- Immer-based updateSettings with immutability
- Subscription management and notifications
- Real-time viewport updates
- Authentication integration
- Error handling and recovery
- Persistence and server sync

### 3. Selective Hooks (`useSelectiveSettingsStore.test.ts`)
- `useSelectiveSetting` - Single path subscription
- `useSelectiveSettings` - Multiple path subscriptions
- `useSettingSetter` - Batched updates
- `useSettingsSubscription` - Callback-based subscriptions
- `useSettingsSelector` - Derived state with selectors
- Performance optimizations and re-render prevention
- Edge case handling

### 4. Performance Tests (`performance.test.ts`)
- Debounced update batching
- Selective re-render optimization
- Batched operations efficiency
- Memory management and cleanup
- Immer performance validation
- Real-time update prioritization

### 5. Integration Tests (`integration.test.ts`)
- Full settings lifecycle flow
- Multi-component coordination
- Authentication integration
- Real-time update coordination
- Error recovery scenarios
- Cross-session persistence
- High-frequency update handling

## Key Features Tested

### Settings Alignment
- Type safety between TypeScript interfaces and runtime values
- Automatic generation of defaultSettings from YAML maintains consistency
- All properties have correct types and valid ranges

### Immer Integration
- Immutable updates with Immer's produce function
- Complex nested updates handled efficiently
- Maintains referential equality for unchanged objects
- Prevents accidental mutations

### Performance Optimizations
- Debounced saves prevent server spam
- Selective subscriptions reduce re-renders
- Batched updates improve efficiency
- Real-time viewport updates bypass debouncing
- Memory cleanup prevents leaks

### Authentication Support
- Nostr authentication headers included when available
- Power user state properly propagated
- Graceful fallback for unauthenticated users

### Error Resilience
- Server failures don't crash the UI
- Subscriber errors are isolated
- Local state remains consistent
- Automatic recovery mechanisms

## Writing New Tests

When adding new settings or features:

1. **Type Tests**: Add to `typeAlignment.test.ts` to verify new properties
2. **Store Tests**: Add to `settingsStore.test.ts` for new store methods
3. **Hook Tests**: Add to `useSelectiveSettingsStore.test.ts` for new hooks
4. **Integration Tests**: Add scenarios to `integration.test.ts`

## Performance Benchmarks

Current benchmarks (on typical hardware):
- Single update: < 1ms
- Batch of 10 updates: < 5ms
- Batch of 100 updates: < 20ms
- Complex nested update: < 10ms
- Subscription notification: < 1ms per subscriber

## Troubleshooting

### Common Issues

1. **Test Timeouts**: Increase timeout in `vitest.config.ts`
2. **Mock Failures**: Check mock implementations in test files
3. **Type Errors**: Ensure settings.ts and defaultSettings.ts are in sync
4. **Performance Tests**: May vary based on hardware, adjust thresholds

### Debug Mode

Enable debug logging in tests:
```typescript
import { debugState } from '@/utils/debugState';
debugState.enable();
```

## CI/CD Integration

These tests are designed to run in CI pipelines:
- Fast execution (< 30s for full suite)
- No external dependencies
- Deterministic results
- Coverage reports generated

Add to your CI config:
```yaml
- name: Run Tests
  run: npm test -- --coverage --reporter=json
```