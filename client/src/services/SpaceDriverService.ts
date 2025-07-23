/**
 * SpaceDriver Service for WebHID 6DOF Controllers
 * Tested with Spaceball 5000 (USB) and SpaceExplorer
 * 
 * This service handles WebHID communication with 3Dconnexion SpaceMouse/SpacePilot devices
 * Provides events for translation (x,y,z), rotation (rx,ry,rz), and button presses
 */

import { createLogger } from '../utils/logger';

const logger = createLogger('SpaceDriverService');

// Device configuration
const VENDOR_ID = 0x046d; // Logitech/3Dconnexion
const DEVICE_FILTER = { vendorId: VENDOR_ID };
const REQUEST_PARAMS = { filters: [DEVICE_FILTER] };

// Report IDs
const REPORT_ID_TRANSLATION = 1;
const REPORT_ID_ROTATION = 2;
const REPORT_ID_BUTTONS = 3;

// Event types
export interface TranslateEvent extends CustomEvent {
  detail: {
    x: number;
    y: number;
    z: number;
  };
}

export interface RotateEvent extends CustomEvent {
  detail: {
    rx: number;
    ry: number;
    rz: number;
  };
}

export interface ButtonsEvent extends CustomEvent {
  detail: {
    buttons: string[];
  };
}

export interface ConnectEvent extends CustomEvent {
  detail: {
    device: HIDDevice;
  };
}

/**
 * SpaceDriver singleton service for managing WebHID SpaceMouse/SpacePilot devices
 */
class SpaceDriverService extends EventTarget {
  private device: HIDDevice | undefined;
  private isInitialized = false;

  constructor() {
    super();
    this.handleInputReport = this.handleInputReport.bind(this);
    this.handleDisconnect = this.handleDisconnect.bind(this);
  }

  /**
   * Initialize the service and check for already connected devices
   */
  async initialize(): Promise<void> {
    if (this.isInitialized) return;

    try {
      // Check if WebHID is available
      if (!navigator.hid) {
        logger.warn('WebHID API not available. This could be due to:');
        logger.warn('1. Not using HTTPS or localhost');
        logger.warn('2. Browser doesn\'t support WebHID (use Chrome or Edge)');
        logger.warn('3. WebHID is disabled in browser flags');
        logger.warn('4. Running in an insecure context');
        
        // Check if we're in a secure context
        if (window.isSecureContext === false) {
          logger.warn('Not in a secure context. WebHID requires HTTPS or localhost.');
          logger.warn('');
          logger.warn('TO FIX - Choose one option:');
          logger.warn('1. Access via http://localhost:3000 instead of IP');
          logger.warn('2. Use HTTPS (https://192.168.0.51:3000)');
          logger.warn('3. In Chrome: chrome://flags → "Insecure origins treated as secure" → Add http://192.168.0.51:3000');
          logger.warn('');
        }
        
        // Dispatch a custom event to notify UI components
        this.dispatchEvent(new CustomEvent('webhid-unavailable', {
          detail: {
            isSecureContext: window.isSecureContext,
            hostname: window.location.hostname,
            protocol: window.location.protocol
          }
        }));
        
        // Don't initialize demo mode - let user fix the issue
        return;
      }

      // Check for already paired devices
      const devices = await navigator.hid.getDevices();
      const spacePilotDevices = devices.filter(d => d.vendorId === VENDOR_ID);
      
      if (spacePilotDevices.length > 0) {
        logger.info(`Found ${spacePilotDevices.length} paired SpacePilot device(s)`);
        await this.openDevice(spacePilotDevices[0]);
      }

      // Listen for device disconnection
      navigator.hid.addEventListener('disconnect', this.handleDisconnect);

      // Listen for new device connections (Note: this may not work reliably on all platforms)
      navigator.hid.addEventListener('connect', (evt: HIDConnectionEvent) => {
        logger.info('Device connected:', evt.device.productName);
        if (evt.device.vendorId === VENDOR_ID && !this.device) {
          this.openDevice(evt.device);
        }
      });

      this.isInitialized = true;
      logger.info('SpaceDriver service initialized');
    } catch (error) {
      logger.error('Failed to initialize SpaceDriver:', error);
    }
  }


  /**
   * Open and connect to a specific HID device
   */
  private async openDevice(device: HIDDevice): Promise<void> {
    try {
      // Disconnect from current device if any
      if (this.device) {
        await this.disconnect();
      }

      // Open the new device
      await device.open();
      logger.info('Opened device:', device.productName);

      // Set up event listener
      device.addEventListener('inputreport', this.handleInputReport);
      
      this.device = device;
      
      // Dispatch connect event
      this.dispatchEvent(new CustomEvent('connect', {
        detail: { device }
      }) as ConnectEvent);
    } catch (error) {
      logger.error('Failed to open device:', error);
    }
  }

  /**
   * Disconnect from the current device
   */
  async disconnect(): Promise<void> {
    if (this.device) {
      try {
        this.device.removeEventListener('inputreport', this.handleInputReport);
        await this.device.close();
        logger.info('Disconnected from device');
      } catch (error) {
        logger.error('Error closing device:', error);
      }
      
      this.device = undefined;
      this.dispatchEvent(new Event('disconnect'));
    }
  }

  /**
   * Check if a device is currently connected
   */
  isConnected(): boolean {
    return this.device !== undefined;
  }

  /**
   * Get the currently connected device info
   */
  getDevice(): HIDDevice | undefined {
    return this.device;
  }

  /**
   * Handle device disconnection event
   */
  private handleDisconnect(evt: HIDConnectionEvent): void {
    if (evt.device === this.device) {
      logger.info('Device disconnected:', evt.device.productName);
      this.disconnect();
    }
  }

  /**
   * Handle input reports from the device
   */
  private handleInputReport(evt: HIDInputReportEvent): void {
    switch (evt.reportId) {
      case REPORT_ID_TRANSLATION:
        this.handleTranslation(new Int16Array(evt.data.buffer));
        break;
      case REPORT_ID_ROTATION:
        this.handleRotation(new Int16Array(evt.data.buffer));
        break;
      case REPORT_ID_BUTTONS:
        this.handleButtons(new Uint16Array(evt.data.buffer)[0]);
        break;
      default:
        logger.warn('Unknown report ID:', evt.reportId);
    }
  }

  /**
   * Handle translation data (x, y, z movement)
   */
  private handleTranslation(values: Int16Array): void {
    this.dispatchEvent(new CustomEvent('translate', {
      detail: {
        x: values[0],
        y: values[1],
        z: values[2]
      }
    }) as TranslateEvent);
  }

  /**
   * Handle rotation data (rx, ry, rz rotation)
   */
  private handleRotation(values: Int16Array): void {
    // Note: rx and ry are negated based on the original implementation
    this.dispatchEvent(new CustomEvent('rotate', {
      detail: {
        rx: -values[0],
        ry: -values[1],
        rz: values[2]
      }
    }) as RotateEvent);
  }

  /**
   * Handle button press data
   */
  private handleButtons(buttonBits: number): void {
    const buttons: string[] = [];
    
    // Check each bit for button state
    for (let i = 0; i < 16; i++) {
      if (buttonBits & (1 << i)) {
        // Use hex notation for consistency with original implementation
        buttons.push(`[${(i + 1).toString(16).toUpperCase()}]`);
      }
    }

    this.dispatchEvent(new CustomEvent('buttons', {
      detail: { buttons }
    }) as ButtonsEvent);
  }

  /**
   * Request permission and connect to a SpacePilot device
   */
  async scan(): Promise<boolean> {
    try {
      const devices = await navigator.hid.requestDevice(REQUEST_PARAMS);
      
      if (devices.length === 0) {
        logger.info('No device selected');
        return false;
      }

      await this.openDevice(devices[0]);
      return true;
    } catch (error) {
      logger.error('Failed to scan for devices:', error);
      return false;
    }
  }

  /**
   * Clean up resources
   */
  async destroy(): Promise<void> {
    await this.disconnect();
    
    if (this.isInitialized) {
      navigator.hid.removeEventListener('disconnect', this.handleDisconnect);
      this.isInitialized = false;
    }
  }
}

// Export singleton instance
export const SpaceDriver = new SpaceDriverService();

// Auto-initialize when imported
SpaceDriver.initialize().catch(error => {
  logger.error('Failed to auto-initialize SpaceDriver:', error);
});