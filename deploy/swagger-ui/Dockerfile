FROM node:22-alpine AS definition
WORKDIR /build
COPY deploy/swagger-ui/package.json deploy/swagger-ui/package-lock.json ./
RUN npm ci --ignore-scripts --no-audit --no-fund
COPY deploy/swagger-ui/bundle.mjs ./
COPY docs/API/front_back.yaml ./api/front_back.yaml
COPY docs/API/paths/ ./api/paths/
COPY docs/API/schemas/ ./api/schemas/
COPY docs/API/components/ ./api/components/
RUN node bundle.mjs ./api/front_back.yaml /build/openapi.json

FROM swaggerapi/swagger-ui:v5.32.0
COPY --from=definition /build/openapi.json /openapi.json
ENV SWAGGER_JSON=/openapi.json
