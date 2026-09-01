#!/usr/bin/env bash
set -euo pipefail

repository_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
plugin_project="$repository_root/trino-open-sdbl"
plugin_directory="$plugin_project/target/plugin"
plugin_jar="$plugin_directory/trino-open-sdbl-0.1.0-SNAPSHOT.jar"
runtime=${CONTAINER_RUNTIME:-}

if [[ -z "$runtime" ]]; then
    if command -v podman >/dev/null 2>&1; then
        runtime=podman
    elif command -v docker >/dev/null 2>&1; then
        runtime=docker
    else
        echo "Neither podman nor docker is installed" >&2
        exit 1
    fi
fi

if ! command -v "$runtime" >/dev/null 2>&1; then
    echo "Container runtime '$runtime' was not found" >&2
    exit 1
fi

container_name=${TRINO_CONTAINER_NAME:-open-sdbl-trino-476}
trino_image=${TRINO_IMAGE:-trinodb/trino:476}
bind_address=${TRINO_BIND_ADDRESS:-127.0.0.1}
trino_port=${TRINO_PORT:-8080}
catalog_name=${TRINO_CATALOG_NAME:-onec}
open_sdbl_host_port=${OPEN_SDBL_HOST_PORT:-8088}
request_timeout_ms=${OPEN_SDBL_REQUEST_TIMEOUT_MS:-65000}
startup_timeout=${TRINO_STARTUP_TIMEOUT:-90}

network_arguments=()
host_arguments=()
build_user_arguments=(--user "$(id -u):$(id -g)")
if [[ $(basename -- "$runtime") == podman ]]; then
    build_user_arguments=(--userns=keep-id --user "$(id -u):$(id -g)")
    open_sdbl_uri=${OPEN_SDBL_URI:-http://127.0.0.1:$open_sdbl_host_port}
    if [[ -z ${OPEN_SDBL_URI+x} ]]; then
        network_arguments=(--network "pasta:-T,$open_sdbl_host_port")
    fi
else
    open_sdbl_uri=${OPEN_SDBL_URI:-http://host.docker.internal:$open_sdbl_host_port}
    host_arguments=(--add-host host.docker.internal:host-gateway)
fi

if [[ ! "$catalog_name" =~ ^[A-Za-z0-9_-]+$ ]]; then
    echo "TRINO_CATALOG_NAME may contain only letters, digits, '_' and '-'" >&2
    exit 1
fi
for numeric_setting in "$trino_port" "$open_sdbl_host_port" "$request_timeout_ms" "$startup_timeout"; do
    if [[ ! "$numeric_setting" =~ ^[0-9]+$ ]]; then
        echo "Ports, timeouts and limits must be non-negative integers" >&2
        exit 1
    fi
done

needs_plugin_build=false
if [[ ! -f "$plugin_jar" ]]; then
    needs_plugin_build=true
elif [[ -n "$(find "$plugin_project/pom.xml" "$plugin_project/src" -type f -newer "$plugin_jar" -print -quit)" ]]; then
    needs_plugin_build=true
fi

if [[ "$needs_plugin_build" == true ]]; then
    echo "Building the Trino 476 plugin..."
    if command -v mvn >/dev/null 2>&1; then
        (cd -- "$plugin_project" && mvn -q test package)
    else
        "$runtime" run --rm \
            "${build_user_arguments[@]}" \
            --env HOME=/tmp \
            --volume "$plugin_project:/source:Z" \
            --workdir /source \
            maven:3.9.9-eclipse-temurin-24 \
            mvn -q -Dmaven.repo.local=/tmp/m2 test package
    fi
fi

if [[ ! -f "$plugin_jar" ]]; then
    echo "Plugin artifact was not created at $plugin_jar" >&2
    exit 1
fi

runtime_directory="$repository_root/target/trino-run"
catalog_file="$runtime_directory/$catalog_name.properties"
runtime_plugin_directory="$runtime_directory/plugin"
mkdir -p -- "$runtime_directory"
cat >"$catalog_file" <<EOF
connector.name=open_sdbl
open-sdbl.uri=$open_sdbl_uri
open-sdbl.request-timeout-ms=$request_timeout_ms
EOF

if "$runtime" container inspect "$container_name" >/dev/null 2>&1; then
    echo "Replacing existing container $container_name..."
    "$runtime" rm --force "$container_name" >/dev/null
fi

# Never mount Maven's mutable output directly into a running JVM. Replacing a
# JAR in place can leave Trino's URLClassLoader with a mixed old/new archive.
rm -rf -- "$runtime_plugin_directory"
mkdir -p -- "$runtime_plugin_directory"
cp -a -- "$plugin_directory/." "$runtime_plugin_directory/"

echo "Starting $trino_image as $container_name..."
"$runtime" run --detach \
    --name "$container_name" \
    "${network_arguments[@]}" \
    "${host_arguments[@]}" \
    --publish "$bind_address:$trino_port:8080" \
    --volume "$runtime_plugin_directory:/usr/lib/trino/plugin/open-sdbl:ro,z" \
    --volume "$catalog_file:/etc/trino/catalog/$catalog_name.properties:ro,z" \
    "$trino_image" >/dev/null

deadline=$((SECONDS + startup_timeout))
until "$runtime" exec "$container_name" trino --output-format NULL --execute 'SELECT 1' >/dev/null 2>&1; do
    if [[ $("$runtime" inspect --format '{{.State.Running}}' "$container_name" 2>/dev/null) != true ]]; then
        echo "Trino container stopped during startup" >&2
        "$runtime" logs --tail 80 "$container_name" >&2 || true
        exit 1
    fi
    if ((SECONDS >= deadline)); then
        echo "Trino did not become ready in ${startup_timeout}s" >&2
        "$runtime" logs --tail 80 "$container_name" >&2 || true
        exit 1
    fi
    sleep 2
done

echo "Trino 476 is ready: http://$bind_address:$trino_port"
echo "Catalog '$catalog_name' uses $open_sdbl_uri"
echo "CLI: $runtime exec -it $container_name trino --catalog $catalog_name"
echo "Stop: $runtime rm --force $container_name"
