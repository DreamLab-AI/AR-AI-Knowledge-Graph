import { Settings } from '../features/settings/config/settings';
import { apiService } from './api';
import { createLogger, createErrorMetadata } from '../utils/logger';
import { convertSnakeToCamelCase, convertCamelToSnakeCase } from '../utils/caseConversion';
import { debugState } from '../utils/debugState';

const logger = createLogger('SettingsService');

/**
 * Service for managing settings API interactions
 */
class SettingsService {
  private static instance: SettingsService;

  private constructor() {}

  public static getInstance(): SettingsService {
    if (!SettingsService.instance) {
      SettingsService.instance = new SettingsService();
    }
    return SettingsService.instance;
  }

  /**
   * Fetch settings from the server
   * @returns The settings from the server, converted to camelCase
   */
  public async fetchSettings(): Promise<Settings | null> {
    try {
      // Fetch settings from the server
      const rawSettings = await apiService.get<Record<string, any>>('/user-settings');
      
      // Server sends camelCase for this endpoint, so no conversion needed here.
      const settings = rawSettings as Settings;
      
      if (debugState.isEnabled()) {
        logger.info('Fetched settings from server (already camelCase):', { settings });
      }
      
      return settings;
    } catch (error) {
      logger.error('Failed to fetch settings:', createErrorMetadata(error));
      return null;
    }
  }

  /**
   * Save settings to the server
   * @param settings The settings to save, in camelCase
   * @param authHeaders Optional authentication headers
   * @returns The updated settings from the server, converted to camelCase
   */
  public async saveSettings(
    settings: Settings,
    authHeaders: Record<string, string> = {}
  ): Promise<Settings | null> {
    try {
      // Convert settings to snake_case for the server
      const settingsToSend = convertCamelToSnakeCase(settings); // Convert to snake_case for the server
      
      if (debugState.isEnabled()) {
        logger.info('Saving settings to server (camelCase):', { settingsToSend });
      }
      
      // Send settings to the server
      const rawUpdatedSettings = await apiService.post<Record<string, any>>(
        '/user-settings/sync',
        settingsToSend,
        authHeaders
      );
      
      // Server sends camelCase for this endpoint's response, so no conversion needed here.
      const updatedSettings = rawUpdatedSettings as Settings;
      
      if (debugState.isEnabled()) {
        logger.info('Settings saved successfully (response already camelCase):', { updatedSettings });
      }
      
      return updatedSettings;
    } catch (error) {
      logger.error('Failed to save settings:', createErrorMetadata(error));
      return null;
    }
  }

  /**
   * Clear the settings cache on the server
   * @param authHeaders Authentication headers
   * @returns Whether the operation was successful
   */
  public async clearSettingsCache(authHeaders: Record<string, string>): Promise<boolean> {
    try {
      await apiService.post('/user-settings/clear-cache', {}, authHeaders);
      logger.info('Settings cache cleared successfully');
      return true;
    } catch (error) {
      logger.error('Failed to clear settings cache:', createErrorMetadata(error));
      return false;
    }
  }
}

export const settingsService = SettingsService.getInstance();
