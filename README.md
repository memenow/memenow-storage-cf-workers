# memenow-storage-cf-workers
Cloudflare Workers version of memenow storage service. High-performance, edge-based file storage and retrieval system

## Key Features

- Large file uploads with chunking support
- Upload state tracking using Durable Objects
- Upload progress querying and cancellation
- File storage using R2 buckets
- Integrated structured logging
- Health check endpoint

## Tech Stack

- Rust
- Cloudflare Workers
- Durable Objects
- R2 Storage
- KV Storage

## Project Structure

```
src/
├── config.rs       # Configuration management
├── errors.rs       # Error handling
├── models.rs       # Data models
├── handlers.rs     # Request handlers
├── durable_object.rs # Durable Object implementation
├── utils.rs        # Utility functions
├── logging.rs      # Logging
└── lib.rs          # Main entry point
```

## API Endpoints

- `POST /v1/uploads`: Initialize upload
- `GET /v1/uploads/:id`: Get upload progress
- `DELETE /v1/uploads/:id`: Cancel upload
- `GET /v1/health`: Health check

## Configuration

The project uses KV storage for configuration management. Key configuration items include:

- `DURABLE_OBJECT_NAME`: Name of the Durable Object
- `TRACKER_NAME`: Name of the tracker
- `BUCKET_NAME`: Name of the R2 bucket
- `MAX_FILE_SIZE`: Maximum file size
- `RATE_LIMIT`: Rate limit

## Deployment

1. Ensure the `wrangler` CLI tool is installed.
2. Update the `wrangler.toml` file with your configuration, including R2 bucket and KV namespace.
3. Run `wrangler publish` to deploy the project.

## Development

1. Clone the repository
2. Install dependencies: `cargo build`
3. Run locally: `wrangler dev`

## Notes

- Ensure proper configuration of Cloudflare R2 bucket and KV namespace.
- Set appropriate client timeout when uploading large files.
- Regularly check and clean up incomplete uploads to avoid wasting storage space.

## Contributing

Issues and pull requests are welcome to improve this project.

## License

This project is licensed under the Apache 2.0 License. For more details, please refer to the [LICENSE](./LICENSE) file.