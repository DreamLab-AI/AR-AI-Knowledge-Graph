import { logger } from './logger.js';

export class ServiceRegistry {
  constructor() {
    this.services = new Map();
  }

  registerService(service) {
    if (!service.id || !service.name || !service.handler) {
      throw new Error('Service must have id, name, and handler');
    }

    if (typeof service.handler !== 'function') {
      throw new Error('Service handler must be a function');
    }

    this.services.set(service.id, {
      id: service.id,
      name: service.name,
      description: service.description || '',
      inputSchema: service.inputSchema || null,
      outputSchema: service.outputSchema || null,
      pricing: service.pricing || { amount: 0, currency: 'SAT' },
      handler: service.handler
    });

    logger.info(`Registered service: ${service.id} - ${service.name}`);
  }

  unregisterService(serviceId) {
    if (this.services.delete(serviceId)) {
      logger.info(`Unregistered service: ${serviceId}`);
      return true;
    }
    return false;
  }

  getService(serviceId) {
    const service = this.services.get(serviceId);
    return service ? service.handler : null;
  }

  getServiceInfo(serviceId) {
    const service = this.services.get(serviceId);
    if (!service) return null;

    // Return service info without the handler function
    const { handler, ...info } = service;
    return info;
  }

  listCapabilities() {
    return Array.from(this.services.values()).map(service => ({
      id: service.id,
      name: service.name,
      description: service.description,
      pricing: service.pricing,
      inputSchema: service.inputSchema,
      outputSchema: service.outputSchema
    }));
  }

  // Execute service with validation
  async executeService(serviceId, input) {
    const service = this.services.get(serviceId);

    if (!service) {
      throw new Error(`Service ${serviceId} not found`);
    }

    // Validate input if schema provided
    if (service.inputSchema) {
      this.validateInput(input, service.inputSchema);
    }

    // Execute service
    const startTime = Date.now();

    try {
      const result = await service.handler(input);

      // Validate output if schema provided
      if (service.outputSchema) {
        this.validateOutput(result, service.outputSchema);
      }

      const executionTime = Date.now() - startTime;
      logger.info(`Service ${serviceId} executed in ${executionTime}ms`);

      return result;
    } catch (error) {
      const executionTime = Date.now() - startTime;
      logger.error(`Service ${serviceId} failed after ${executionTime}ms:`, error);
      throw error;
    }
  }

  // Basic schema validation (can be extended with JSON Schema)
  validateInput(input, schema) {
    if (schema.required && input === undefined) {
      throw new Error('Input is required');
    }

    if (schema.type) {
      const inputType = Array.isArray(input) ? 'array' : typeof input;
      if (inputType !== schema.type) {
        throw new Error(`Input must be of type ${schema.type}`);
      }
    }
  }

  validateOutput(output, schema) {
    if (schema.required && output === undefined) {
      throw new Error('Output is required');
    }

    if (schema.type) {
      const outputType = Array.isArray(output) ? 'array' : typeof output;
      if (outputType !== schema.type) {
        throw new Error(`Output must be of type ${schema.type}`);
      }
    }
  }

  // Get MCP-compatible manifest
  getMCPManifest() {
    return {
      name: 'SAND Stack Agent Services',
      version: '1.0.0',
      capabilities: this.listCapabilities()
    };
  }
}