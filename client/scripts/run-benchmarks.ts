#!/usr/bin/env node

/**
 * Benchmark Test Runner
 *
 * Orchestrates all performance and integration tests:
 * - Graph performance benchmarks
 * - Multi-user load tests
 * - VR performance validation
 * - Network resilience tests
 */

import * as fs from 'fs';
import * as path from 'path';
import { program } from 'commander';
import {
  GraphBenchmark,
  DEFAULT_BENCHMARK_CONFIG,
  BenchmarkResult
} from '../src/tests/performance/GraphBenchmark';
import {
  MultiUserLoadTest,
  DEFAULT_LOAD_TEST_CONFIG,
  LoadTestResult
} from '../src/tests/load/MultiUserTest';
import {
  VRPerformanceTest,
  DEFAULT_VR_CONFIG,
  VRPerformanceResult
} from '../src/tests/vr/VRPerformanceTest';
import {
  NetworkLatencyTest,
  DEFAULT_NETWORK_TEST_CONFIG,
  NetworkTestResult
} from '../src/tests/network/LatencyTest';

interface TestSuite {
  name: string;
  enabled: boolean;
  runner: () => Promise<any>;
}

interface BenchmarkReport {
  timestamp: Date;
  duration: number;
  results: {
    performance?: BenchmarkResult[];
    load?: LoadTestResult[];
    vr?: VRPerformanceResult;
    network?: NetworkTestResult[];
  };
  summary: {
    totalTests: number;
    passed: number;
    failed: number;
    warnings: number;
  };
}

class BenchmarkRunner {
  private outputDir: string;
  private suites: TestSuite[] = [];

  constructor(outputDir: string = './benchmark-results') {
    this.outputDir = outputDir;
    this.ensureOutputDir();
  }

  /**
   * Ensure output directory exists
   */
  private ensureOutputDir(): void {
    if (!fs.existsSync(this.outputDir)) {
      fs.mkdirSync(this.outputDir, { recursive: true });
    }
  }

  /**
   * Register test suite
   */
  registerSuite(name: string, runner: () => Promise<any>, enabled: boolean = true): void {
    this.suites.push({ name, runner, enabled });
  }

  /**
   * Run all enabled test suites
   */
  async runAll(): Promise<BenchmarkReport> {
    console.log('═══════════════════════════════════════════════');
    console.log('          VisionClaw Benchmark Suite          ');
    console.log('═══════════════════════════════════════════════\n');

    const startTime = Date.now();
    const results: BenchmarkReport['results'] = {};

    for (const suite of this.suites) {
      if (!suite.enabled) {
        console.log(`⏭️  Skipping ${suite.name}...\n`);
        continue;
      }

      console.log(`\n▶️  Running ${suite.name}...`);
      console.log('─────────────────────────────────────────────\n');

      try {
        const result = await suite.runner();
        results[suite.name.toLowerCase().replace(/\s+/g, '')] = result;
        console.log(`✅ ${suite.name} completed\n`);
      } catch (error) {
        console.error(`❌ ${suite.name} failed:`, error);
        console.log('');
      }
    }

    const duration = Date.now() - startTime;

    // Calculate summary
    const summary = this.calculateSummary(results);

    const report: BenchmarkReport = {
      timestamp: new Date(),
      duration,
      results,
      summary
    };

    // Save report
    this.saveReport(report);

    // Print summary
    this.printSummary(report);

    return report;
  }

  /**
   * Calculate test summary
   */
  private calculateSummary(results: BenchmarkReport['results']): BenchmarkReport['summary'] {
    let totalTests = 0;
    let passed = 0;
    let failed = 0;
    let warnings = 0;

    // Performance benchmarks
    if (results.performance) {
      totalTests += results.performance.length;
      results.performance.forEach(r => {
        if (r.avgFps >= 60) passed++;
        else failed++;
        if (r.gcPauses > 10) warnings++;
      });
    }

    // Load tests
    if (results.load) {
      totalTests += results.load.length;
      results.load.forEach(r => {
        if (r.successfulConnections === r.userCount && r.avgLatency < 200) passed++;
        else failed++;
        if (r.conflictsDetected > 0) warnings++;
      });
    }

    // VR test
    if (results.vr) {
      totalTests++;
      if (results.vr.passed) passed++;
      else failed++;
      if (results.vr.issues.length > 0) warnings++;
    }

    // Network tests
    if (results.network) {
      totalTests += results.network.length;
      results.network.forEach(r => {
        if (r.passed) passed++;
        else failed++;
        if (r.rubberBanding > 5) warnings++;
      });
    }


    return { totalTests, passed, failed, warnings };
  }

  /**
   * Save report to file
   */
  private saveReport(report: BenchmarkReport): void {
    const timestamp = report.timestamp.toISOString().replace(/:/g, '-');
    const filename = `benchmark-${timestamp}.json`;
    const filepath = path.join(this.outputDir, filename);

    fs.writeFileSync(filepath, JSON.stringify(report, null, 2));
    console.log(`\n📁 Report saved to: ${filepath}`);

    // Also save markdown reports
    this.saveMarkdownReports(report);
  }

  /**
   * Save individual markdown reports
   */
  private saveMarkdownReports(report: BenchmarkReport): void {
    const timestamp = report.timestamp.toISOString().replace(/:/g, '-');

    if (report.results.performance) {
      const md = GraphBenchmark.generateReport(report.results.performance);
      fs.writeFileSync(
        path.join(this.outputDir, `performance-${timestamp}.md`),
        md
      );
    }

    if (report.results.load) {
      const md = MultiUserLoadTest.generateReport(report.results.load);
      fs.writeFileSync(
        path.join(this.outputDir, `load-${timestamp}.md`),
        md
      );
    }

    if (report.results.vr) {
      const md = VRPerformanceTest.generateReport(report.results.vr);
      fs.writeFileSync(
        path.join(this.outputDir, `vr-${timestamp}.md`),
        md
      );
    }

    if (report.results.network) {
      const md = NetworkLatencyTest.generateReport(report.results.network);
      fs.writeFileSync(
        path.join(this.outputDir, `network-${timestamp}.md`),
        md
      );
    }

  }

  /**
   * Print summary to console
   */
  private printSummary(report: BenchmarkReport): void {
    console.log('\n═══════════════════════════════════════════════');
    console.log('              Test Summary                     ');
    console.log('═══════════════════════════════════════════════\n');

    console.log(`Total Tests:    ${report.summary.totalTests}`);
    console.log(`✅ Passed:      ${report.summary.passed}`);
    console.log(`❌ Failed:      ${report.summary.failed}`);
    console.log(`⚠️  Warnings:    ${report.summary.warnings}`);
    console.log(`⏱️  Duration:    ${(report.duration / 1000).toFixed(2)}s\n`);

    const successRate = (report.summary.passed / report.summary.totalTests) * 100;
    console.log(`Success Rate:  ${successRate.toFixed(1)}%\n`);

    if (report.summary.failed === 0) {
      console.log('🎉 All tests passed!\n');
    } else {
      console.log('⚠️  Some tests failed. Check reports for details.\n');
    }
  }
}

// CLI Interface
program
  .name('run-benchmarks')
  .description('Run VisionClaw benchmark and test suite')
  .version('1.0.0');

program
  .option('-p, --performance', 'Run performance benchmarks')
  .option('-l, --load', 'Run load tests')
  .option('-v, --vr', 'Run VR performance tests')
  .option('-n, --network', 'Run network resilience tests')
  .option('-a, --all', 'Run all tests (default)', true)
  .option('-o, --output <dir>', 'Output directory', './benchmark-results')
  .parse(process.argv);

const options = program.opts();

// Determine which tests to run
const runAll = options.all || (!options.performance && !options.load && !options.vr && !options.network);
const runPerformance = runAll || options.performance;
const runLoad = runAll || options.load;
const runVr = runAll || options.vr;
const runNetwork = runAll || options.network;

// Create runner
const runner = new BenchmarkRunner(options.output);

// Register suites
runner.registerSuite('Performance Benchmarks', async () => {
  const benchmark = new GraphBenchmark(DEFAULT_BENCHMARK_CONFIG);
  return await benchmark.run();
}, runPerformance);

runner.registerSuite('Load Tests', async () => {
  const loadTest = new MultiUserLoadTest(DEFAULT_LOAD_TEST_CONFIG);
  return await loadTest.run();
}, runLoad);

runner.registerSuite('VR Performance', async () => {
  const vrTest = new VRPerformanceTest(DEFAULT_VR_CONFIG);
  return await vrTest.run();
}, runVr);

runner.registerSuite('Network Resilience', async () => {
  const networkTest = new NetworkLatencyTest(DEFAULT_NETWORK_TEST_CONFIG);
  return await networkTest.run();
}, runNetwork);

// Run benchmarks
runner.runAll()
  .then(() => process.exit(0))
  .catch((error) => {
    console.error('Benchmark suite failed:', error);
    process.exit(1);
  });
