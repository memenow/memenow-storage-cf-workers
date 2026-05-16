# Deployment Guide

This guide explains how to deploy the MemeNow Storage service to Cloudflare Workers with D1 database integration.

## Overview

The MemeNow Storage service requires several Cloudflare resources:
- **Cloudflare Workers**: Edge compute runtime
- **D1 Database**: SQL database for upload metadata and state management
- **R2 Storage**: Object storage for file data
- **KV Storage**: Configuration and settings

## Prerequisites

- Cloudflare account with Workers Paid plan
- Rust toolchain (1.82 or later)
- Node.js and npm
- wrangler CLI

## Configuration Management

### Security-First Approach

All sensitive configuration (API keys, namespace IDs, database IDs) are managed through:
1. **Local Development**: Use `wrangler.toml.local` (git-ignored)
2. **CI/CD**: Use GitHub Actions secrets with `wrangler.toml.template`
3. **Production**: Never commit actual IDs to the repository

### File Structure

```
wrangler.toml          # Public config with placeholders
wrangler.toml.template # CI/CD template with variables
wrangler.toml.local    # Local dev config (git-ignored)
.env.example           # Example environment variables
schema.sql             # D1 database schema
```

## Initial Setup

### 1. Install Dependencies

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup target add wasm32-unknown-unknown

# Install Node.js dependencies
npm install -g wrangler

# Install worker-build
cargo install worker-build
```

### 2. Create Cloudflare Resources

#### Create D1 Database

```bash
# Development database
wrangler d1 create memenow-uploads-dev

# Production database
wrangler d1 create memenow-uploads-prod
```

Note the database IDs returned by these commands.

#### Create R2 Bucket

```bash
# Development bucket
wrangler r2 bucket create memenow-storage-dev

# Production bucket
wrangler r2 bucket create memenow-storage-prod
```

#### Create KV Namespace

```bash
# Development namespace
wrangler kv namespace create "STORAGE_CONFIG" --preview

# Production namespace
wrangler kv namespace create "STORAGE_CONFIG"
```

Note the namespace IDs returned by these commands.

### 3. Initialize Database Schema

Apply the schema to both development and production databases:

```bash
# Development
wrangler d1 execute memenow-uploads-dev --file=schema.sql

# Production
wrangler d1 execute memenow-uploads-prod --file=schema.sql
```

Verify the schema was applied:

```bash
# Check tables exist
wrangler d1 execute memenow-uploads-dev --command="SELECT name FROM sqlite_master WHERE type='table';"
```

### 4. Configure Local Development

Create local configuration file:

```bash
cp wrangler.toml.template wrangler.toml.local
cp .env.example .env
```

Edit `wrangler.toml.local` with your actual resource IDs:

```toml
name = "memenow-storage-cf-workers"
main = "build/worker/shim.mjs"
compatibility_date = "2025-08-15"
logpush = true

[placement]
mode = "smart"

[build]
command = "cargo install -q worker-build && worker-build --release"

[[r2_buckets]]
binding = "STORAGE_BUCKET"
bucket_name = "memenow-storage-dev"
preview_bucket_name = "memenow-storage-dev"

[[kv_namespaces]]
binding = "STORAGE_CONFIG"
id = "your_prod_kv_namespace_id"
preview_id = "your_dev_kv_namespace_id"

[[d1_databases]]
binding = "UPLOAD_DB"
database_name = "memenow-uploads-prod"
database_id = "your_prod_d1_database_id"
preview_database_id = "your_dev_d1_database_id"
```

## Local Development

### Start Development Server

```bash
wrangler dev --config wrangler.toml.local
```

### Test Basic Functionality

```bash
# Health check
curl http://localhost:8787/health

# Test upload initialization
curl -X POST http://localhost:8787/api/upload/init \
  -H "Content-Type: application/json" \
  -d '{
    "file_name": "test.txt",
    "total_size": 100,
    "user_role": "creator",
    "content_type": "text/plain",
    "user_id": "test_user"
  }'
```

## GitHub Actions Setup

### Required Secrets

Set these in your GitHub repository settings (Settings → Secrets and variables → Actions):

| Secret Name | Description |
|------------|-------------|
| `CLOUDFLARE_API_TOKEN` | Cloudflare API token with Workers permissions |
| `CLOUDFLARE_ACCOUNT_ID` | Cloudflare account ID |
| `PROD_KV_NAMESPACE_ID` | Production KV namespace ID |
| `DEV_KV_NAMESPACE_ID` | Development KV namespace ID |
| `PROD_D1_DATABASE_NAME` | Production D1 database name |
| `PROD_D1_DATABASE_ID` | Production D1 database ID |
| `DEV_D1_DATABASE_ID` | Development D1 database ID |
| `PROD_R2_BUCKET` | Production R2 bucket name |
| `DEV_R2_BUCKET` | Development R2 bucket name |

### Deployment Workflow

The canonical workflow lives in `.github/workflows/deploy.yml`. Its behavior:

1. **On push to `main`**: deploys to the preview environment via
   `cloudflare/wrangler-action@v3` with `deploy --env preview`, then applies
   `schema.sql` against the dev D1 binding (`--remote`).
2. **On GitHub release**: deploys to production with `deploy`, then applies
   `schema.sql` against the production D1 binding (`--remote`).
3. **Configuration assembly**: copies `wrangler.toml.template` to
   `wrangler.toml` and substitutes the secrets listed above via `sed`.

`schema.sql` is idempotent (`CREATE TABLE IF NOT EXISTS`, `CREATE INDEX IF
NOT EXISTS`, `CREATE VIEW IF NOT EXISTS`), so the schema step is safe to run
on every deploy.

## Manual Deployment

### Deploy to Preview

```bash
# Using local config
wrangler deploy --env preview --config wrangler.toml.local

# Or with environment variables
export PROD_KV_NAMESPACE_ID=your_prod_kv_id
export DEV_KV_NAMESPACE_ID=your_dev_kv_id
export PROD_D1_DATABASE_ID=your_prod_d1_id
export DEV_D1_DATABASE_ID=your_dev_d1_id
envsubst < wrangler.toml.template > wrangler.toml
wrangler deploy --env preview
```

### Deploy to Production

```bash
# Using local config
wrangler deploy --config wrangler.toml.local

# Or create a release on GitHub to trigger automatic deployment
git tag v1.0.0
git push origin v1.0.0
```

## Database Management

### Schema Updates

When updating the database schema:

1. Update `schema.sql` with new changes
2. Apply migrations manually:

```bash
# Development
wrangler d1 execute memenow-uploads-dev --file=migrations/001_add_new_column.sql

# Production (be careful!)
wrangler d1 execute memenow-uploads-prod --file=migrations/001_add_new_column.sql
```

### Backup and Recovery

```bash
# Export database
wrangler d1 export memenow-uploads-prod --output=backup.sql

# Import database
wrangler d1 execute memenow-uploads-dev --file=backup.sql
```

### Monitor Database Usage

```bash
# Check database size and query stats
wrangler d1 info memenow-uploads-prod
```

## Configuration Management

### KV Configuration

Set initial configuration in KV storage:

```bash
# Development
echo '{"database_name":"UPLOAD_DB","max_file_size":10737418240,"chunk_size":99614720}' | \
wrangler kv key put "config" --binding=STORAGE_CONFIG --env=preview

# Production
echo '{"database_name":"UPLOAD_DB","max_file_size":10737418240,"chunk_size":99614720}' | \
wrangler kv key put "config" --binding=STORAGE_CONFIG
```

### Update Configuration

```bash
# Update max file size to 5GB
echo '{"database_name":"UPLOAD_DB","max_file_size":5368709120,"chunk_size":99614720}' | \
wrangler kv key put "config" --binding=STORAGE_CONFIG
```

## Resource Naming Convention

### Recommended resource names

| Environment | D1 database | R2 bucket |
|-------------|-------------|-----------|
| Development | `memenow-uploads-dev` | `memenow-storage-dev` |
| Production  | `memenow-uploads-prod` | `memenow-storage-prod` |

KV namespaces are referenced by ID from secrets; only the binding name
appears in `wrangler.toml`.

### Binding Names (in code)
- **KV Binding**: `STORAGE_CONFIG`
- **D1 Binding**: `UPLOAD_DB`
- **R2 Binding**: `STORAGE_BUCKET`

## Monitoring and Observability

### Enable Logging

```bash
# Enable logpush for detailed logs
wrangler logpush create --compatibility-date=2025-08-15
```

### Monitor Performance

1. **Cloudflare Dashboard**: Monitor request volume, error rates, and response times
2. **D1 Analytics**: Track database query performance and usage
3. **R2 Metrics**: Monitor storage operations and bandwidth usage

### Built-in Observability

- Worker observability is enabled in `wrangler.toml` (`[observability.logs]`).
- Each request is logged on entry (method + path) by the worker.
- Error responses include a UTC timestamp and machine-readable error code
  for correlation with logs.

## Troubleshooting

### Common Issues

**"Namespace not found" error**
- Verify KV namespace IDs in secrets/config
- Ensure namespace exists in Cloudflare dashboard
- Check binding names match wrangler.toml

**"Database not found" error**
- Create D1 databases first: `wrangler d1 create <name>`
- Update database IDs in configuration
- Verify database schema is applied

**"Schema not applied" error**
```bash
# Check if tables exist
wrangler d1 execute your-db --command="SELECT name FROM sqlite_master WHERE type='table';"

# Reapply schema if needed
wrangler d1 execute your-db --file=schema.sql
```

**Build failures**
- Ensure Rust toolchain is installed: `rustup update`
- Install wasm32 target: `rustup target add wasm32-unknown-unknown`
- Install worker-build: `cargo install worker-build`
- Check for compilation errors: `cargo check --target wasm32-unknown-unknown`

**Performance Issues**
- Monitor D1 database query performance
- Check R2 operation latency
- Verify edge caching is working
- Review Workers CPU time usage

### Verify Configuration

```bash
# Check current bindings
wrangler whoami
wrangler kv namespace list
wrangler d1 list
wrangler r2 bucket list

# Test deployment without publishing
wrangler deploy --dry-run --outdir dist

# Validate wrangler.toml
wrangler deploy --dry-run
```

### Debug Database Issues

```bash
# Check table structure
wrangler d1 execute your-db --command="PRAGMA table_info(uploads);"

# Check data
wrangler d1 execute your-db --command="SELECT COUNT(*) FROM uploads;"

# Check recent uploads
wrangler d1 execute your-db --command="SELECT upload_id, status, created_at FROM uploads ORDER BY created_at DESC LIMIT 5;"
```

## Security Best Practices

1. **Never commit sensitive IDs** to the repository
2. **Use environment-specific resources** (dev/staging/prod)
3. **Rotate API tokens regularly** (every 90 days)
4. **Limit token permissions** to minimum required scopes
5. **Use GitHub environments** for deployment protection rules
6. **Enable audit logging** for all administrative actions
7. **Monitor unusual upload patterns** for abuse detection
8. **Implement rate limiting** to prevent service abuse

## Performance Optimization

### Database Optimization

- Monitor D1 query performance in Cloudflare dashboard
- Use indexed queries for upload lookups
- Implement query result caching where appropriate
- Consider database cleanup for old/cancelled uploads

### Storage Optimization

- Use appropriate chunk sizes for network conditions
- Implement client-side retry logic
- Monitor R2 operation costs and optimize access patterns
- Consider implementing CDN caching for frequently accessed files

### Worker Optimization

- Minimize cold start latency
- Use appropriate CPU time allocation
- Implement efficient error handling
- Monitor memory usage patterns

## Support and Maintenance

### Regular Maintenance Tasks

1. **Monthly**: Review usage metrics and costs
2. **Quarterly**: Rotate API tokens and review access permissions
3. **As needed**: Apply database schema updates
4. **On issues**: Check error logs and performance metrics

### Getting Help

For issues or questions:
1. Check the [Architecture Documentation](./ARCHITECTURE.md)
2. Review the [API Documentation](./API.md)  
3. Consult Cloudflare Workers documentation
4. Open an issue on GitHub with relevant logs and configuration