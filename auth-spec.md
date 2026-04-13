# Auth Service Technical Specification

Complete technical specification for the centralized authentication service.

## Principles

- **Stateless**: No database, all information is in the JWT/cookie
- **Decentralized**: Apps validate tokens locally via public key (JWKS)
- **Flexible**: Apps decide expiration policy based on `iat` (issued at)
- **Dual-mode**: Supports cookies (subdomains) and tokens (external apps)

## Architecture Overview

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│    Your App     │     │  Auth Service   │     │  Google OAuth   │
└────────┬────────┘     └────────┬────────┘     └────────┬────────┘
         │                       │                       │
         │  1. /login?redirect=  │                       │
         │ ───────────────────►  │                       │
         │                       │  2. OAuth redirect    │
         │                       │ ───────────────────►  │
         │                       │                       │
         │                       │  3. code + user info  │
         │                       │ ◄───────────────────  │
         │                       │                       │
         │  4. redirect + JWT    │                       │
         │ ◄───────────────────  │                       │
         │    (cookie + token)   │                       │
         │                       │                       │
         │  5. Validate JWT      │                       │
         │     (uses JWKS)       │                       │
```

## Authentication Flow

1. **User Initiates Login**: Your app redirects to `/login` with a callback URL
2. **OAuth Redirect**: Auth service redirects user to Google OAuth
3. **Google Authentication**: User authenticates with Google
4. **Code Exchange**: Google redirects back to `/callback` with authorization code
5. **Token Generation**: Auth service exchanges code for user info and creates JWT
6. **Response**: User is redirected to your app with JWT (token + optional cookie)
7. **Local Validation**: Your app validates JWT using public key from JWKS

## Endpoints

### `GET /login`

Initiates OAuth flow with Google.

**Query Parameters:**

| Parameter | Required | Description |
|-----------|----------|-------------|
| `redirect` | Yes | Callback URL after authentication |
| `mode` | No | `cookie` (default), `token`, or `both` |
| `state` | No | Opaque value returned in callback (recommended for CSRF protection) |

**Validations:**
- `redirect` must be HTTPS (except localhost for development)
- `redirect` must be in allowed domain (configurable via `ALLOWED_DOMAINS`)
- `mode` must be one of: `cookie`, `token`, `both`

**Example:**
```
GET https://auth.yourdomain.com/login?redirect=https://app.yourdomain.com/callback&mode=both
```

### `GET /callback`

Receives Google OAuth response. Internal endpoint, not called directly by applications.

**Actions:**
1. Validate `state` parameter (if provided)
2. Exchange `code` for access token with Google
3. Get user information (`email`, `name`, `picture`)
4. Generate JWT signed with RS256
5. If `mode` includes `cookie`: set `__auth` cookie with `domain=`
6. Redirect to original `redirect` with token in query string

**Success redirect:**
```
https://app.yourdomain.com/callback?token=eyJhbGc...&state=xyz
```

**Error redirect:**
```
https://app.yourdomain.com/callback?error=access_denied&error_description=User+denied&state=xyz
```

### `GET /logout`

Removes authentication cookie.

**Query Parameters:**

| Parameter | Required | Description |
|-----------|----------|-------------|
| `redirect` | No | URL to redirect after logout |

**Actions:**
1. Remove `__auth` cookie by setting `maxAge=0`
2. Redirect to `redirect` or `/` if not specified

**Example:**
```
GET https://auth.yourdomain.com/logout?redirect=https://app.yourdomain.com
```

### `GET /.well-known/jwks.json`

Returns public keys for JWT validation (JWKS - JSON Web Key Set).

**Response:**
```json
{
  "keys": [
    {
      "kty": "RSA",
      "kid": "key-2024-01",
      "use": "sig",
      "alg": "RS256",
      "n": "0vx7agoebGc...",
      "e": "AQAB"
    }
  ]
}
```

**Headers:**
- `Cache-Control: public, max-age=86400` (24 hours)

**Cache Strategy:**
- Apps should cache this response for 24 hours
- Key rotation should use a new `kid` value

### `GET /userinfo`

Returns authenticated user information. Convenient for debug/testing.

**Headers:**
```
Authorization: Bearer <token>
```
or
```
Cookie: __auth=<token>
```

**Response (200):**
```json
{
  "sub": "google-oauth2|123456789",
  "email": "user@gmail.com",
  "name": "User Name",
  "picture": "https://lh3.googleusercontent.com/..."
}
```

**Response (401):**
```json
{
  "error": "invalid_token",
  "error_description": "Token is missing or invalid"
}
```

### `POST /refresh`

Issues a new token with updated `iat`. Useful for extending sessions without re-authenticating.

**Headers:**
```
Authorization: Bearer <token>
```
or
```
Cookie: __auth=<token>
```

**Validations:**
- Token must have valid RS256 signature
- Token must not have been issued more than 30 days ago

**Response (200):**
```json
{
  "token": "eyJhbGc..."
}
```

Also updates `__auth` cookie if present.

**Response (401):**
```json
{
  "error": "invalid_token",
  "error_description": "Token is missing or invalid"
}
```

or

```json
{
  "error": "token_expired",
  "error_description": "Token is too old to refresh"
}
```

## JWT (JSON Web Token)

### Header

```json
{
  "alg": "RS256",
  "typ": "JWT",
  "kid": "key-2024-01"
}
```

### Payload

```json
{
  "iss": "https://auth.yourdomain.com",
  "sub": "google-oauth2|123456789",
  "iat": 1706097600,
  "email": "user@gmail.com",
  "name": "User Name",
  "picture": "https://lh3.googleusercontent.com/..."
}
```

| Claim | Type | Description |
|-------|------|-------------|
| `iss` | string | Issuer (always your auth service domain) |
| `sub` | string | Unique user ID (`provider|id`) |
| `iat` | number | Unix timestamp of issuance |
| `email` | string | User email address |
| `name` | string | User's full name |
| `picture` | string | Profile photo URL |

**Important Notes:**
- No `exp` (expiration) field. Each app decides validity based on `iat`
- Use `kid` to select the correct public key from JWKS

## Cookie

| Attribute | Value |
|-----------|-------|
| Name | `__auth` |
| Value | Complete JWT |
| Domain | Configurable via `COOKIE_DOMAIN` (e.g., `.yourdomain.com`) |
| Path | `/` |
| HttpOnly | `true` |
| Secure | `true` (in production) |
| SameSite | `Lax` |
| MaxAge | 30 days (2592000 seconds) |

## Configuration

### Environment Variables

```env
# Google OAuth (Required)
GOOGLE_CLIENT_ID=xxx.apps.googleusercontent.com
GOOGLE_CLIENT_SECRET=GOCSPX-xxx

# JWT Keys (Required)
JWT_PRIVATE_KEY_PATH=/etc/auth.fbmac/private.pem
JWT_PUBLIC_KEY_PATH=/etc/auth.fbmac/public.pem
JWT_KEY_ID=key-2024-01

# Domains (Optional, with defaults)
ALLOWED_DOMAINS=yourdomain.com,localhost
BASE_URL=https://auth.yourdomain.com
COOKIE_DOMAIN=.yourdomain.com

# Port (Optional, default: 3000)
PORT=3000
```

### RSA Key Generation

```bash
# Generate private key (2048-bit RSA)
openssl genrsa -out private.pem 2048

# Extract public key
openssl rsa -in private.pem -pubout -out public.pem

# Set proper permissions
chmod 600 private.pem public.pem
```

## Validation Algorithm (Client-side)

### Pseudocode

```python
# 1. Get JWKS (cache for 24h)
jwks = fetch("https://auth.yourdomain.com/.well-known/jwks.json")

# 2. Extract token from cookie or header
token = request.cookies["__auth"] or request.headers["Authorization"].replace("Bearer ", "")

# 3. Decode header to get kid
header = jwt.decode_header(token)
key = jwks.find_key(header["kid"])

# 4. Validate signature
payload = jwt.verify(token, key, algorithms=["RS256"])

# 5. Validate issuer
assert payload["iss"] == "https://auth.yourdomain.com"

# 6. Validate age (app policy)
max_age = 7 * 24 * 60 * 60  # 7 days, for example
assert time.now - payload["iat"] < max_age

# 7. Authenticated user
user = payload
```

## Security Considerations

### Required Validations

1. **Redirect URI**: Only whitelisted domains
2. **HTTPS**: Required in production (except localhost)
3. **State parameter**: Recommended to prevent CSRF
4. **Cookie flags**: HttpOnly, Secure, SameSite=Lax
5. **RSA key size**: Minimum 2048 bits
6. **Algorithm**: RS256 (asymmetric)

### Rate Limiting (Recommended)

| Endpoint | Suggested Limit |
|----------|----------------|
| `/login` | 10/min per IP |
| `/refresh` | 30/min per IP |
| `/jwks.json` | 100/min per IP |

## OAuth State Management

The auth service uses an in-memory Map to store OAuth state:

- **Storage**: In-memory Map
- **Expiry**: 10 minutes
- **Cleanup**: Automatic on state access

**For production scaling**: Consider replacing with Redis or similar.

## Technology Stack

### Current Implementation

- **Runtime**: Bun
- **Framework**: Hono
- **JWT**: `jose` library
- **Deployment**: Docker Compose

### Alternative Stacks

For reimplementation:

| Component | Recommended Options |
|-----------|------------------|
| Runtime | Bun, Node.js, Deno |
| Framework | Hono, Express, Fastify |
| JWT | `jose`, `jsonwebtoken` |
| Deployment | Docker, Kubernetes, Cloudflare Workers, Vercel |

## Performance Considerations

### Caching

- **JWKS**: Cache for 24 hours (`Cache-Control: public, max-age=86400`)
- **OAuth state**: In-memory with 10-minute expiry

### Scalability

- **Stateless design** allows horizontal scaling
- **No database** required
- **Token validation** is client-side (reduces server load)

**For high traffic**: Use Redis for OAuth state management.

## Error Responses

All error responses follow this format:

```json
{
  "error": "error_code",
  "error_description": "Human-readable error message"
}
```

### Common Error Codes

| Error Code | Description |
|-------------|-------------|
| `missing_redirect` | `redirect` parameter is required |
| `invalid_redirect` | Redirect URL is not allowed |
| `invalid_mode` | Mode must be cookie, token, or both |
| `invalid_state` | State is invalid or expired |
| `missing_code` | Authorization code is missing |
| `server_error` | Failed to complete authentication |
| `invalid_token` | Token is missing or invalid |
| `token_expired` | Token is too old to refresh |

## Related Documentation

- [Development Guide](DEVELOPMENT.md) - Local development setup
- [Integration Guide](INTEGRATION.md) - Client application integration
