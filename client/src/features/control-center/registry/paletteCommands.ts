/**
 * Registers one "reveal setting" command per field into the EXISTING command
 * palette registry. Cmd+K → fuzzy-search by label OR dot-path (via keywords) →
 * Enter dispatches `controlcenter:reveal`, which the Control Center listens for
 * (WP2's useRevealSetting) to open the group, ensureLoad it, scroll + flash the row.
 *
 * Call once from App.tsx::initializeCommandPalette() (WP3 wires the call).
 */
import { commandRegistry } from '../../command-palette';
import { PALETTE_INDEX } from './paletteIndex';

let registered = false;

export function registerSettingsCommands(): void {
  if (registered) return;
  registered = true;

  for (const e of PALETTE_INDEX) {
    commandRegistry.registerCommand({
      id: e.id,
      title: `${e.groupLabel} › ${e.label}`,
      description: e.path ?? e.localKey ?? e.action ?? '',
      category: 'settings',
      keywords: e.keywords,
      handler: () => {
        window.dispatchEvent(
          new CustomEvent('controlcenter:reveal', {
            detail: { group: e.groupId, testid: e.testid },
          }),
        );
      },
    });
  }
}
