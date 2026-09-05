// @ts-ignore - vitest types may not be available in all environments
import { describe, it, expect, beforeEach, vi } from 'vitest';

// --- Mock all external dependencies before importing the module under test ---

vi.mock('../../../utils/loggerConfig', () => ({
  createLogger: () => ({
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
    debug: vi.fn(),
  }),
  createErrorMetadata: vi.fn((e: unknown) => e),
}));

vi.mock('../../../utils/clientDebugState', () => ({
  debugState: {
    isEnabled: () => false,
    isDataDebugEnabled: () => false,
  },
}));

vi.mock('../../../features/graph/managers/graphDataManager', () => ({
  graphDataManager: {
    nodeIdMap: new Map(),
    fetchInitialData: vi.fn().mockResolvedValue(undefined),
    setGraphData: vi.fn().mockResolvedValue(undefined),
  },
}));

vi.mock('../../../features/ontology/store/useInferredEdgesStore', () => ({
  useInferredEdgesStore: {
    getState: () => ({
      refresh: vi.fn().mockResolvedValue(undefined),
    }),
  },
}));

const mockSet = vi.fn();
const mockGetSectionPaths = vi.fn();
vi.mock('../../settingsStore', () => ({
  useSettingsStore: {
    getState: () => ({
      settings: {},
      set: mockSet,
    }),
  },
  settingsStoreUtils: {
    getSectionPaths: (...args: unknown[]) => mockGetSectionPaths(...args),
  },
}));

const mockGetCurrentUser = vi.fn();
vi.mock('../../../services/nostrAuthService', () => ({
  nostrAuth: {
    getCurrentUser: (...args: unknown[]) => mockGetCurrentUser(...args),
  },
}));

const mockGetSettingsByPaths = vi.fn();
vi.mock('../../../api/settingsApi', () => ({
  settingsApi: {
    getSettingsByPaths: (...args: unknown[]) => mockGetSettingsByPaths(...args),
  },
}));

const mockEmit = vi.fn();
const mockNotifyMessageHandlers = vi.fn();
vi.mock('../connectionManager', () => ({
  emit: (...args: unknown[]) => mockEmit(...args),
  notifyMessageHandlers: (...args: unknown[]) => mockNotifyMessageHandlers(...args),
}));

vi.mock('../binaryProtocol', () => ({
  handleErrorFrame: vi.fn(),
}));

vi.mock('../filterSync', () => ({
  isFilterResponseExpected: vi.fn(() => false),
  clearFilterResponseExpectation: vi.fn(),
}));

// Need to import AFTER mocks are set up
import { handleTextMessage } from '../textMessageHandler';

const noopGet = () => ({ forceReconnect: vi.fn() });
const noopSet = vi.fn();
const noopProcessQueue = vi.fn();

const CURRENT_USER_PUBKEY = 'aaaa000000000000000000000000000000000000000000000000000000aa';
const OTHER_PUBKEY = 'bbbb111111111111111111111111111111111111111111111111111111bb';

function dispatch(message: Record<string, unknown>) {
  handleTextMessage(
    message as never,
    noopGet as never,
    noopSet as never,
    noopProcessQueue as never,
  );
}

describe('handleTextMessage — settingsUpdated (ADR-2047)', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockGetCurrentUser.mockReturnValue({ pubkey: CURRENT_USER_PUBKEY });
    mockGetSectionPaths.mockImplementation((section: string) => {
      if (section === 'physics') return ['visualisation.graphs.knowledge.physics'];
      if (section === 'rendering') return ['visualisation.rendering.ambientLightIntensity'];
      if (section === 'nodeFilter') return ['nodeFilter'];
      return [];
    });
  });

  it('applies the supplied settings object directly for the nodeFilter category', () => {
    const nodeFilterSettings = {
      enabled: true,
      qualityThreshold: 0.5,
      authorityThreshold: 0.3,
      filterByQuality: true,
      filterByAuthority: false,
      filterMode: 'and',
      includeLinkedPages: true,
    };

    dispatch({
      type: 'settingsUpdated',
      category: 'nodeFilter',
      updatedBy: OTHER_PUBKEY,
      timestamp: 1000,
      settings: nodeFilterSettings,
    });

    expect(mockSet).toHaveBeenCalledWith('nodeFilter', nodeFilterSettings, true);
    // No follow-up re-read should happen for nodeFilter.
    expect(mockGetSettingsByPaths).not.toHaveBeenCalled();
  });

  it('re-reads the category from the settings API for a physics update', async () => {
    mockGetSettingsByPaths.mockResolvedValue({
      visualisation: { graphs: { knowledge: { physics: { springK: 0.42 } } } },
    });

    dispatch({
      type: 'settingsUpdated',
      category: 'physics',
      updatedBy: OTHER_PUBKEY,
      timestamp: 1000,
    });

    expect(mockGetSectionPaths).toHaveBeenCalledWith('physics');
    expect(mockGetSettingsByPaths).toHaveBeenCalledWith(['visualisation.graphs.knowledge.physics']);

    // Allow the settingsApi promise to resolve.
    await Promise.resolve();
    await Promise.resolve();

    expect(mockSet).toHaveBeenCalledWith(
      'visualisation.graphs.knowledge.physics',
      { springK: 0.42 },
      true,
    );
  });

  it('ignores the echo of the current user\'s own write', () => {
    dispatch({
      type: 'settingsUpdated',
      category: 'rendering',
      updatedBy: CURRENT_USER_PUBKEY,
      timestamp: 1000,
    });

    expect(mockGetSettingsByPaths).not.toHaveBeenCalled();
    expect(mockSet).not.toHaveBeenCalled();
  });

  it('ignores a stale settingsUpdated message (older timestamp than last applied)', async () => {
    mockGetSettingsByPaths.mockResolvedValue({
      visualisation: { rendering: { ambientLightIntensity: 1.5 } },
    });

    // First message establishes the "last applied" timestamp for 'rendering'.
    dispatch({
      type: 'settingsUpdated',
      category: 'rendering',
      updatedBy: OTHER_PUBKEY,
      timestamp: 2000,
    });
    await Promise.resolve();
    await Promise.resolve();
    expect(mockGetSettingsByPaths).toHaveBeenCalledTimes(1);

    mockGetSettingsByPaths.mockClear();
    mockSet.mockClear();

    // A second message with an older timestamp must be dropped.
    dispatch({
      type: 'settingsUpdated',
      category: 'rendering',
      updatedBy: OTHER_PUBKEY,
      timestamp: 1000,
    });

    expect(mockGetSettingsByPaths).not.toHaveBeenCalled();
    expect(mockSet).not.toHaveBeenCalled();
  });
});
