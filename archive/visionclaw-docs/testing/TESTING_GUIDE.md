# VisionClaw Testing & Validation Guide

## Overview

VisionClaw includes comprehensive testing and benchmarking suites to ensure performance, reliability, and compatibility across all platforms.

## Test Suites

### Gap Remediation Tests (ADR-031 Sprint)

The following test suites were added during the VisionClaw gap remediation waves 1-4:

**Backend Integration Tests**

- **`tests/orchestration_improvements_test.rs`**: 15 integration tests covering ADR-031 agent orchestration improvements — supervisor AllForOne strategy, ActorFactory respawning, graceful shutdown draining, escalation wiring, health endpoint validation, CQRS bus timeouts, and ontology handler error honesty.

**Client Unit Tests (94 new tests)**

| Test File | Test Count | Coverage |
|-----------|-----------|----------|
| `settingsStore` | 31 | Settings state management, persistence, validation, defaults |
| `graphDataManager` | 22 | Graph data loading, caching, incremental updates, error handling |
| `graphComputations` | 41 | Layout algorithms, force calculations, position computations, edge cases |

**Re-enabled Test Suites**

- GPU test files are being re-enabled after CUDA path fixes
- CQRS bus tests (previously commented out) are now uncommented and passing — these validate handler registration, dispatch routing, timeout behaviour, and error propagation

---

### 1. Performance Benchmarks

**Location**: `client/src/tests/performance/GraphBenchmark.ts`

Measures rendering performance at various node counts:
- **100 nodes**: Baseline performance
- **500 nodes**: Typical usage
- **1000 nodes**: Heavy usage
- **5000 nodes**: Stress test

**Metrics**:
- Average/Min/Max FPS
- Frame time (P99)
- Memory usage
- GC pauses
- Render/update time breakdown

**Run**:
```bash
npm run benchmark:performance
```

**Pass Criteria**:
- 60+ FPS at all node counts
- P99 frame time < 16.67ms
- Memory usage stable
- GC pauses < 10 per test

### 2. Multi-User Load Testing

**Location**: `client/src/tests/load/MultiUserTest.ts`

Simulates concurrent users interacting with the graph:
- **10 users**: Light load
- **50 users**: Moderate load
- **100 users**: Heavy load

**Metrics**:
- Connection success rate
- Average latency
- Position convergence time
- Conflict detection/resolution
- Messages per second

**Run**:
```bash
npm run benchmark:load
```

**Pass Criteria**:
- 100% connection success
- Latency < 200ms
- Convergence time < 1s
- Conflict resolution rate > 95%

### 3. VR Performance Validation

**Location**: `client/src/tests/vr/VRPerformanceTest.ts`

Tests VR-specific performance for Quest 3:
- 72fps minimum framerate
- Hand tracking latency < 50ms
- Zero reprojection
- Comfort score > 80

**Metrics**:
- FPS (avg/min/max)
- Frame time variance
- Hand tracking latency
- Reprojection rate
- Comfort score

**Run**:
```bash
npm run benchmark:vr
```

**Requirements**:
- VR headset (Quest 3 recommended)
- WebXR-compatible browser

**Pass Criteria**:
- Avg FPS ≥ 72
- Min FPS ≥ 65
- Hand tracking latency < 50ms
- Comfort score > 80

### 4. Network Resilience Testing

**Location**: `client/src/tests/network/LatencyTest.ts`

Tests behavior under various network conditions:
- **Good**: 50ms latency, 0% loss
- **Average**: 100ms latency, 1% loss
- **Poor**: 500ms latency, 5% loss
- **Very Poor**: 1000ms latency, 10% loss

**Metrics**:
- Actual vs expected latency
- Interpolation smoothness
- Rubber-banding occurrences
- Reconnection count
- Message loss rate

**Run**:
```bash
npm run benchmark:network
```

**Pass Criteria**:
- Interpolation smoothness > 70
- Rubber-banding < 5 occurrences
- Zero reconnections
- Graceful degradation

### 5. Vircadia Integration Testing

**Location**: `client/src/tests/integration/VircadiaTest.ts`

Verifies Vircadia features work with Three.js:
- Avatar synchronization
- Presence indicators
- Collaboration features
- Audio/video integration
- Domain coordination

**Metrics**:
- Avatar update latency
- Presence update frequency
- Collaboration events
- Audio packet reception
- Three.js compatibility

**Run**:
```bash
npm run benchmark:integration
```

**Requirements**:
- Vircadia domain server running
- Test user account

**Pass Criteria**:
- All features functional
- Avatar latency < 100ms
- Zero compatibility issues

## Running Tests

### Run All Tests
```bash
npm run benchmark
```

### Run Specific Test Suite
```bash
npm run benchmark:performance
npm run benchmark:load
npm run benchmark:vr
npm run benchmark:network
npm run benchmark:integration
```

### CI Mode
```bash
npm run benchmark:ci
```

### Custom Output Directory
```bash
ts-node scripts/run-benchmarks.ts --all --output ./my-results
```

## Test Results

Results are saved in `benchmark-results/` directory:

```
benchmark-results/
├── benchmark-YYYY-MM-DD-HH-mm-ss.json  # Complete results
├── performance-YYYY-MM-DD-HH-mm-ss.md  # Performance report
├── load-YYYY-MM-DD-HH-mm-ss.md         # Load test report
├── vr-YYYY-MM-DD-HH-mm-ss.md           # VR report
├── network-YYYY-MM-DD-HH-mm-ss.md      # Network report
└── vircadia-YYYY-MM-DD-HH-mm-ss.md     # Integration report
```

### Result Format

**JSON**:
```json
{
  "timestamp": "2025-01-15T10:30:00.000Z",
  "duration": 125000,
  "results": {
    "performance": [...],
    "load": [...],
    "vr": {...},
    "network": [...],
    "vircadia": {...}
  },
  "summary": {
    "totalTests": 15,
    "passed": 14,
    "failed": 1,
    "warnings": 2
  }
}
```

**Markdown Reports**: Human-readable with tables and issue summaries

## CI Integration

Tests run automatically on:
- Push to `main` or `develop`
- Pull requests
- Daily at 2 AM UTC
- Manual trigger

See `.github/workflows/benchmarks.yml`

## Performance Regression Detection

Benchmarks are compared against baseline:

```bash
node scripts/compare-benchmarks.js \
  baseline-results.json \
  benchmark-results/benchmark-latest.json
```

Alerts on:
- FPS drop > 10%
- Latency increase > 20%
- Memory usage increase > 30%
- Any test failures

## Best Practices

### Before Committing
```bash
# Run quick performance check
npm run benchmark:performance

# Check no regressions
git diff baseline-results.json
```

### Before Release
```bash
# Full test suite
npm run benchmark

# Check all reports
ls -la benchmark-results/
```

### Continuous Monitoring
- Review daily benchmark reports
- Track performance trends
- Set up alerts for regressions

## Troubleshooting

### Tests Fail to Connect
- Check server is running: `npm run dev:server`
- Verify WebSocket URL in config
- Check firewall/network settings

### VR Tests Don't Start
- Enable WebXR in browser flags
- Connect VR headset
- Grant permissions when prompted

### High Memory Usage
- Check for memory leaks: `node --expose-gc`
- Profile with Chrome DevTools
- Review disposal patterns

### Inconsistent Results
- Close other applications
- Run multiple times and average
- Check system resources
- Disable browser extensions

## Writing New Tests

### Performance Test Template
```typescript
import { GraphBenchmark } from './GraphBenchmark';

const customConfig = {
  duration: 20,
  nodeCounts: [100, 1000, 10000],
  edgeDensity: 5,
  warmupFrames: 120,
  collectGCMetrics: true
};

const benchmark = new GraphBenchmark(customConfig);
const results = await benchmark.run();
```

### Integration Test Template
```typescript
export class MyIntegrationTest {
  async run(): Promise<TestResult> {
    // Setup
    await this.initialize();

    // Execute tests
    const feature1Works = await this.testFeature1();
    const feature2Works = await this.testFeature2();

    // Cleanup
    this.cleanup();

    return {
      passed: feature1Works && feature2Works,
      issues: [...this.issues],
      timestamp: new Date()
    };
  }
}
```

## Performance Targets

| Metric | Target | Critical |
|--------|--------|----------|
| FPS (desktop) | 60+ | 45+ |
| FPS (VR) | 72+ | 65+ |
| Frame time | <16.67ms | <22ms |
| Latency | <100ms | <200ms |
| Hand tracking | <50ms | <100ms |
| Memory growth | <1MB/min | <5MB/min |
| GC frequency | <1/sec | <5/sec |

## Support

For issues with tests:
1. Check test documentation
2. Review recent changes
3. Run in isolation
4. File issue with logs

---

**Last Updated**: 2026-04-10
**Maintainer**: VisionClaw QA Team
