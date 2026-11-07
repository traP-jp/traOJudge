import { writeFile } from 'node:fs/promises'
import SwaggerParser from '@apidevtools/swagger-parser'

const [input, output] = process.argv.slice(2)
const definition = await SwaggerParser.bundle(input)
await writeFile(output, JSON.stringify(definition))
