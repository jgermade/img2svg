
# Frontend

Use:
- Vite+
- OXC
- preact

## Project structure
- static - Files out of the bundle copyed directly (vite publicDir)
- src/assets
- src/components - this components can never import services or components from another folder than this
- src/services - stores with asynchronous methods
- src/views - view components, can use src/components and src/services to compound views
- index.html - web entrypoint
