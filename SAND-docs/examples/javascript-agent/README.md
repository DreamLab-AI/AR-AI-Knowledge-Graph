# JavaScript SAND Stack Agent Template

A complete, production-ready template for building autonomous agents on the SAND Stack using JavaScript.

## Features

- ✅ Full did:nostr identity management
- ✅ Encrypted Nostr messaging
- ✅ Service registry with MCP compatibility
- ✅ RESTful API with health checks
- ✅ Prometheus metrics integration
- ✅ Structured logging with Winston
- ✅ Graceful shutdown handling
- ✅ Docker support
- ✅ Comprehensive error handling

## Quick Start

### 1. Create Agent Identity

First, generate a new agent identity:

```bash
npm create agent@latest > agent.json
```

Save the private key displayed in the console - you'll need it for the next step.

### 2. Install Dependencies

```bash
npm install
```

### 3. Configure Environment

Copy the example environment file and add your private key:

```bash
cp .env.example .env
```

Edit `.env` and add your private key from step 1.

### 4. Run the Agent

```bash
npm start
```

Your agent is now running and connected to the Nostr network!

## Project Structure

```
├── src/
│   ├── index.js       # Main agent entry point
│   ├── config.js      # Configuration management
│   ├── identity.js    # Identity and authentication
│   ├── messages.js    # Message handling
│   ├── services.js    # Service registry
│   ├── logger.js      # Logging setup
│   ├── metrics.js     # Prometheus metrics
│   └── api.js         # REST API endpoints
├── tests/             # Test files
├── .env.example       # Environment template
├── Dockerfile         # Docker configuration
├── package.json       # Dependencies
└── README.md         # This file
```

## Available Services

The template includes three example services:

### Echo Service
Echoes back any input provided.

```bash
curl -X POST http://localhost:3000/mcp/execute \
  -H "Content-Type: application/json" \
  -d '{
    "capability": "echo",
    "input": "Hello, SAND Stack!"
  }'
```

### Time Service
Returns current time in multiple formats.

```bash
curl -X POST http://localhost:3000/mcp/execute \
  -H "Content-Type: application/json" \
  -d '{
    "capability": "time"
  }'
```

### Hash Service
Generates SHA256 hash of input.

```bash
curl -X POST http://localhost:3000/mcp/execute \
  -H "Content-Type: application/json" \
  -d '{
    "capability": "hash",
    "input": "Hash this text"
  }'
```

## API Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/health` | GET | Health check |
| `/ready` | GET | Readiness check |
| `/status` | GET | Agent status and stats |
| `/metrics` | GET | Prometheus metrics |
| `/.well-known/mcp` | GET | MCP service manifest |
| `/services` | GET | List all services |
| `/services/:id` | GET | Get service info |
| `/mcp/execute` | POST | Execute a service |
| `/messages/send` | POST | Send encrypted message |

## Adding Custom Services

To add your own service, register it in `src/index.js`:

```javascript
this.serviceRegistry.registerService({
  id: 'my-service',
  name: 'My Custom Service',
  description: 'Does something amazing',
  inputSchema: {
    type: 'object',
    required: true
  },
  pricing: {
    amount: 1000,
    currency: 'SAT'
  },
  handler: async (input) => {
    // Your service logic here
    return {
      result: 'success',
      data: processInput(input)
    };
  }
});
```

## Message Handling

The agent automatically handles several message types:

- `PING` - Responds with PONG
- `SERVICE_REQUEST` - Executes requested service
- `CAPABILITY_QUERY` - Returns available services

To add custom message handling:

```javascript
// In handleDirectMessage method
case 'MY_MESSAGE_TYPE':
  await this.handleMyMessage(event.pubkey, message);
  break;
```

## Docker Deployment

Build and run with Docker:

```bash
# Build image
docker build -t my-sand-agent .

# Run container
docker run -d \
  --name sand-agent \
  -p 3000:3000 \
  --env-file .env \
  my-sand-agent
```

## Monitoring

The agent exposes Prometheus metrics at `/metrics`:

- `agent_messages_processed_total` - Messages processed by type and status
- `agent_services_provided_total` - Services executed by type
- `agent_service_duration_seconds` - Service execution time histogram
- `agent_active_connections` - Active Nostr relay connections
- `agent_peers_discovered_total` - Peer agents discovered
- `agent_announcements_total` - Agent announcements sent

## Testing

Run the test suite:

```bash
# Run all tests
npm test

# Run tests in watch mode
npm run test:watch

# Run specific test file
npm test src/services.test.js
```

## Development

For development with auto-reload:

```bash
npm run dev
```

## Environment Variables

| Variable | Description | Required | Default |
|----------|-------------|----------|---------|
| `AGENT_PRIVATE_KEY` | Nostr private key | Yes | - |
| `AGENT_NAME` | Agent display name | No | "SAND Stack Agent" |
| `AGENT_DESCRIPTION` | Agent description | No | "A basic autonomous agent..." |
| `AGENT_VERSION` | Agent version | No | "1.0.0" |
| `NOSTR_RELAYS` | Comma-separated relay URLs | No | Default relays |
| `API_PORT` | API server port | No | 3000 |
| `API_HOST` | API server host | No | "0.0.0.0" |
| `LOG_LEVEL` | Logging level | No | "info" |
| `NODE_ENV` | Environment | No | "development" |

## Troubleshooting

### "Cannot connect to relays"
- Check your internet connection
- Try different relays in the `.env` file
- Ensure WebSocket connections are not blocked

### "Invalid private key"
- Ensure your private key is a valid hex string
- Check that it's correctly set in the `.env` file
- Try generating a new identity with `npm create agent`

### "Service not found"
- Check that the service is registered
- Verify the service ID matches exactly
- Look at logs for registration errors

## Security Considerations

1. **Private Key Protection**: Never commit your private key to version control
2. **Message Validation**: All messages are validated for age and structure
3. **Rate Limiting**: Consider implementing rate limiting for production
4. **Input Validation**: Services validate input against schemas
5. **CORS**: Configure CORS appropriately for production

## Next Steps

1. Add more services specific to your use case
2. Implement Lightning payments for paid services
3. Add Solid Pod integration for persistent storage
4. Deploy to production with proper monitoring
5. Join the Agentic Alliance community!

## Resources

- [SAND Stack Documentation](https://github.com/agentic-alliance/sand-stack)
- [Nostr Protocol NIPs](https://github.com/nostr-protocol/nips)
- [Model Context Protocol](https://modelcontextprotocol.io)
- [Lightning Network](https://lightning.network)

## License

MIT License - see LICENSE file for details.