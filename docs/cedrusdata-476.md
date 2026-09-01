# CedrusData 476-5 deployment

## Compatibility

The plugin is compiled against `io.trino:trino-spi:476` and uses no
CedrusData-specific API. CedrusData's `476-5` version is treated as upstream
476 plus a distribution revision. Install the plugin in a staging cluster and
run the smoke queries below before production rollout; public CedrusData source
confirming binary identity of every 476-5 build was not available during
implementation.

```text
Trino coordinator + workers
        |  open_sdbl plugin on every node
        v
Kubernetes Service :8088
        v
open-sdbl-trino pods
        v
1C PostgreSQL (read-only credentials)
```

The Java plugin directory from `trino-open-sdbl/target/plugin` must be copied
to `${TRINO_HOME}/plugin/open-sdbl` on the coordinator and every worker.

## Catalog

Create `etc/catalog/onec.properties` on every Trino node:

```properties
connector.name=open_sdbl
open-sdbl.uri=http://open-sdbl-trino:8088
open-sdbl.request-timeout-ms=65000
```

These are properties implemented by this repository's connector; they are not
invented stock Thrift properties.

## Kubernetes resources

```yaml
apiVersion: v1
kind: Secret
metadata:
  name: open-sdbl-postgres
type: Opaque
stringData:
  username: readonly_onec
  password: replace-me
  refresh-token: replace-with-a-random-token
---
apiVersion: v1
kind: ConfigMap
metadata:
  name: open-sdbl-trino
data:
  OPEN_SDBL_POSTGRES_HOST: postgres.example.internal
  OPEN_SDBL_POSTGRES_PORT: "5432"
  OPEN_SDBL_POSTGRES_DATABASE: onec
  OPEN_SDBL_POSTGRES_TLS_MODE: require
  OPEN_SDBL_METADATA_CACHE_TTL: "300"
  OPEN_SDBL_POSTGRES_POOL_SIZE: "8"
  OPEN_SDBL_POSTGRES_CONNECT_TIMEOUT_MS: "10000"
  OPEN_SDBL_POSTGRES_POOL_CREATE_TIMEOUT_MS: "10000"
  OPEN_SDBL_POSTGRES_POOL_WAIT_TIMEOUT_MS: "10000"
  OPEN_SDBL_STATEMENT_TIMEOUT_MS: "60000"
  OPEN_SDBL_QUERY_TIMEOUT_MS: "65000"
  OPEN_SDBL_LISTEN: 0.0.0.0:8088
  OPEN_SDBL_LOG: open_sdbl_trino=info
---
apiVersion: apps/v1
kind: Deployment
metadata:
  name: open-sdbl-trino
spec:
  replicas: 2
  selector:
    matchLabels: {app: open-sdbl-trino}
  template:
    metadata:
      labels: {app: open-sdbl-trino}
    spec:
      containers:
        - name: service
          image: registry.example/open-sdbl-trino:0.1.0
          envFrom:
            - configMapRef: {name: open-sdbl-trino}
          env:
            - name: OPEN_SDBL_POSTGRES_USERNAME
              valueFrom: {secretKeyRef: {name: open-sdbl-postgres, key: username}}
            - name: OPEN_SDBL_POSTGRES_PASSWORD
              valueFrom: {secretKeyRef: {name: open-sdbl-postgres, key: password}}
            - name: OPEN_SDBL_REFRESH_TOKEN
              valueFrom: {secretKeyRef: {name: open-sdbl-postgres, key: refresh-token}}
          ports:
            - {name: http, containerPort: 8088}
          livenessProbe:
            httpGet: {path: /health, port: http}
          readinessProbe:
            httpGet: {path: /ready, port: http}
          resources:
            requests: {cpu: 250m, memory: 256Mi}
            limits: {cpu: "2", memory: 2Gi}
---
apiVersion: v1
kind: Service
metadata:
  name: open-sdbl-trino
spec:
  selector: {app: open-sdbl-trino}
  ports:
    - {name: http, port: 8088, targetPort: http}
```

`OPEN_SDBL_POSTGRES_TLS_MODE` accepts `disable`, `prefer`, or `require` and uses
the container's native trust store. Use `require` outside a trusted in-cluster
network.

Optional safeguards are `OPEN_SDBL_MAXIMUM_RESULT_ROWS` (disabled by default,
and never applied silently), `OPEN_SDBL_CONFIG_DECODE_BATCH_SIZE` (default
256), and `OPEN_SDBL_REFRESH_TOKEN`. A configured refresh token enables
`POST /v1/metadata/refresh` with `Authorization: Bearer <token>`; without it,
the endpoint returns 404. `OPEN_SDBL_DATABASE_URL` can replace the separate
PostgreSQL settings, but separate Secret fields are preferable in Kubernetes.

## Smoke test

```bash
curl --fail http://open-sdbl-trino:8088/ready
trino --execute 'SHOW SCHEMAS FROM onec'
trino --execute 'SHOW TABLES FROM onec."Справочник"'
trino --execute 'SELECT "Код", "Наименование" FROM onec."Справочник"."Контрагенты" LIMIT 10'
```

Enable `OPEN_SDBL_LOG=open_sdbl_trino=debug` temporarily and verify that the
last query log contains the physical projection and parameterized PostgreSQL
`LIMIT`. Database URLs and passwords are never logged.
