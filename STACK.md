
# Frontend

Use:
- Vite+
- OXC
- preact

## Project structure
- web/static - Files out of the bundle copyed directly (vite publicDir)
- web/assets
- web/components - this components can never import services or components from another folder than this
- web/services - stores with asynchronous methods
- web/views - view components, can use src/components and src/services to compound views
- index.html - web entrypoint
