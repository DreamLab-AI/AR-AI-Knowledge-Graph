/**
 * Control Center — public barrel. design-spec.md §2.
 * The MainLayout swap (WP5) imports `ControlCenter` from here.
 */

export { ControlCenter, default } from './ControlCenter';
export { useControlCenterUI } from './state/useControlCenterUI';
export type { ControlCenterUIState } from './state/useControlCenterUI';
export { StatusSurface } from './status/StatusSurface';
export type { RevealDetail } from './hooks/useRevealSetting';
export { REVEAL_EVENT } from './hooks/useRevealSetting';
