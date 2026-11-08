import { Blob } from 'node:buffer'
import { vi } from 'vitest'

// jsdom lacks Blob.stream(), which Shiki uses to decompress its bundled WASM.
vi.stubGlobal('Blob', Blob)
