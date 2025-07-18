import promClient from 'prom-client';

export function setupMetrics() {
  // Create registry
  const register = new promClient.Registry();

  // Add default metrics
  promClient.collectDefaultMetrics({ register });

  // Custom metrics
  const messagesProcessed = new promClient.Counter({
    name: 'agent_messages_processed_total',
    help: 'Total number of messages processed',
    labelNames: ['type', 'status'],
    registers: [register]
  });

  const servicesProvided = new promClient.Counter({
    name: 'agent_services_provided_total',
    help: 'Total number of services provided',
    labelNames: ['service'],
    registers: [register]
  });

  const serviceLatency = new promClient.Histogram({
    name: 'agent_service_duration_seconds',
    help: 'Service execution duration',
    labelNames: ['service', 'status'],
    buckets: [0.01, 0.05, 0.1, 0.5, 1, 2, 5, 10],
    registers: [register]
  });

  const activeConnections = new promClient.Gauge({
    name: 'agent_active_connections',
    help: 'Number of active Nostr relay connections',
    registers: [register]
  });

  const agentsDiscovered = new promClient.Counter({
    name: 'agent_peers_discovered_total',
    help: 'Total number of peer agents discovered',
    registers: [register]
  });

  const announcements = new promClient.Counter({
    name: 'agent_announcements_total',
    help: 'Total number of agent announcements sent',
    registers: [register]
  });

  const earnings = new promClient.Counter({
    name: 'agent_earnings_satoshis',
    help: 'Total earnings in satoshis',
    registers: [register]
  });

  return {
    register,
    messagesProcessed,
    servicesProvided,
    serviceLatency,
    activeConnections,
    agentsDiscovered,
    announcements,
    earnings
  };
}