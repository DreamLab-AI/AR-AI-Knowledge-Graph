/**
 * ControlPanel Component Exports
 *
 * Legacy shell (IntegratedControlPanel/UnifiedSettingsTabContent) deleted in the
 * control-center cutover (WP5) — superseded by features/control-center/registry.
 * This barrel now only re-exports the pieces still consumed elsewhere: shared
 * types and the status widgets reused as-is by the new ControlCenter.
 */

export * from './types';

// Status panels — reused as-is by features/control-center/status/StatusCluster
export { SpacePilotStatus } from './SpacePilotStatus';
export { BotsStatusPanel } from './BotsStatusPanel';
export { SystemHealthIndicator } from './SystemHealthIndicator';
