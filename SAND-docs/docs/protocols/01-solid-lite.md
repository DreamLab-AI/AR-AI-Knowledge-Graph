# 1. Solid-Lite Specification

**Status**: DRAFT

This document specifies `solid-lite`, a minimal subset of the Solid Protocol. The goal of `solid-lite` is to provide the core benefits of Solid—namely, user-owned, permissioned data storage—with a greatly reduced implementation complexity, making it suitable for resource-constrained agents and simple clients.

## 1. Conformance

A `solid-lite` server MUST conform to the following core principles:
- It MUST expose resources (data) via HTTP.
- It MUST use Web Access Control (WAC) for authorization.
- It MUST support a subset of Linked Data Platform (LDP) for resource interaction.

A `solid-lite` server is NOT REQUIRED to implement the full Solid specification, including:
- WebSockets-based notifications.
- Advanced LDP features (e.g., complex container management).
- Multiple identity providers beyond a simple WebID-based mechanism.

## 2. Authentication

Authentication is handled via the client presenting a verifiable credential, typically a JWT signed by the key associated with their `did:nostr` identity. The server verifies this signature to authenticate the client.

## 3. Authorization (Web Access Control)

Authorization is managed using `.acl` files, as specified by Web Access Control (WAC). Each resource can have an associated `.acl` file that defines which agents (identified by their WebID) have `Read`, `Write`, and `Control` access.

### Example `.acl` file:

```turtle
# ACL for /data/profile.ttl

@prefix acl: <http://www.w3.org/ns/auth/acl#>.
@prefix foaf: <http://xmlns.com/foaf/0.1/>.

# The owner has full control over their profile
<#owner>
    a acl:Authorization;
    acl:agent <https://example.com/owner/profile/card#me>;
    acl:accessTo <./profile.ttl>;
    acl:mode acl:Read, acl:Write, acl:Control.

# A specific agent has read-only access
<#friend>
    a acl:Authorization;
    acl:agent <did:nostr:npub1...>;
    acl:accessTo <./profile.ttl>;
    acl:mode acl:Read.
```

- `acl:agent`: Specifies the agent (WebID or DID) being granted permission.
- `acl:accessTo`: The resource the permissions apply to.
- `acl:mode`: The level of access (`Read`, `Write`, `Control`).

## 4. Resource Interaction (LDP Subset)

A `solid-lite` server MUST support the following HTTP methods for interacting with resources.

### `GET`
- **Purpose**: Retrieve a resource.
- **Headers**: `Accept` header should be used to request a specific RDF serialization (e.g., `text/turtle`).
- **Response**: `200 OK` with the resource in the requested format.

### `PUT`
- **Purpose**: Create or replace a resource at a specific URI.
- **Headers**: `Content-Type` must specify the format of the resource being sent.
- **Response**: `201 Created` if new, `204 No Content` if replaced.

### `DELETE`
- **Purpose**: Delete a resource.
- **Response**: `204 No Content` on success.

### `PATCH`
- **Purpose**: Partially update a resource.
- **Headers**: `Content-Type` MUST be `application/sparql-update`.
- **Body**: A SPARQL UPDATE query.
- **Response**: `204 No Content` on success.

## 5. Server Structure

A `solid-lite` server exposes a simple hierarchical structure of containers (directories) and resources (files). The root of the server is the user's storage space.

- **Containers**: Act like directories. Represented as LDP Basic Containers.
- **Resources**: The actual data files, typically stored as RDF (e.g., Turtle files).

This minimal specification provides a clear path for developers to implement lightweight, interoperable data pods for their agents, fostering a rich ecosystem of decentralized data.

---
**Next:** [2. Model Context Protocol (MCP)](./02-model-context-protocol.md)