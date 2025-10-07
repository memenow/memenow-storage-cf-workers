# Flowchart

_Last Updated: 2025-10-07_

## Description

End-to-end request processing with decisions and data stores.

<!--@auto:diagram:flow:start-->

```mermaid
flowchart TD
    A[Client Request] --> B{Route Match?}
    B -->|Yes| C[Apply Middleware: CORS & Validation]
    C --> D{Validation Pass?}
    D -->|No| E[Error Response]
    D -->|Yes| F[Handler: Upload/Other]
    F --> G[Database Operations: Create/Get/Update]
    G --> H[R2 Storage: Put/Get/Delete]
    H --> I[Response with CORS Headers]
    B -->|No| J[404 Not Found]
    E --> I
    J --> I
    I --> A
```

<!--@auto:diagram:flow:end-->
