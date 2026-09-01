package io.open_sdbl.trino;

import com.fasterxml.jackson.core.type.TypeReference;
import com.fasterxml.jackson.databind.ObjectMapper;
import io.trino.spi.TrinoException;

import java.io.IOException;
import java.io.InputStream;
import java.net.URI;
import java.net.URLEncoder;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.util.List;
import java.util.Locale;

import static io.trino.spi.StandardErrorCode.GENERIC_INTERNAL_ERROR;

/**
 * Synchronous HTTP client for the Rust open-sdbl service.
 *
 * <p>Trino invokes connector metadata and record-cursor APIs synchronously. Streaming methods
 * therefore return the response body directly and transfer ownership of that stream to the
 * caller.
 */
final class OpenSdblClient
{
    private final URI baseUri;
    private final Duration timeout;
    private final HttpClient http = HttpClient.newBuilder()
            .connectTimeout(Duration.ofSeconds(10))
            .build();
    private final ObjectMapper json = new ObjectMapper();

    OpenSdblClient(URI baseUri, Duration timeout)
    {
        this.baseUri = baseUri;
        this.timeout = timeout;
    }

    List<String> schemas()
    {
        return get("/v1/metadata/schemas", new TypeReference<>() {});
    }

    List<String> trinoSchemas()
    {
        return schemas().stream()
                .map(name -> name.toLowerCase(Locale.ROOT))
                .toList();
    }

    boolean hasSchema(String schema)
    {
        return resolveSchema(schema) != null;
    }

    List<Model.RemoteTable> tables(String schema)
    {
        String resolvedSchema = schema == null ? null : resolveSchema(schema);
        if (schema != null && resolvedSchema == null) {
            return List.of();
        }
        String query = resolvedSchema == null ? "" : "?schema=" + encode(resolvedSchema);
        return get("/v1/metadata/tables" + query, new TypeReference<>() {});
    }

    Model.RemoteTable table(String schema, String table)
    {
        String resolvedSchema = resolveSchema(schema);
        if (resolvedSchema == null) {
            return null;
        }
        Model.RemoteTable resolvedTable = tables(resolvedSchema).stream()
                .filter(candidate -> candidate.name().equalsIgnoreCase(table))
                .findFirst()
                .orElse(null);
        if (resolvedTable == null) {
            return null;
        }
        String query = "?schema=" + encode(resolvedSchema) + "&table=" + encode(resolvedTable.name());
        try {
            return get("/v1/metadata/table" + query, new TypeReference<>() {});
        }
        catch (TrinoException error) {
            if (error.getMessage().contains("404")) {
                return null;
            }
            throw error;
        }
    }

    InputStream scan(Model.ScanRequest request)
    {
        return stream("/v1/scan", "scan", request);
    }

    Model.SdblPrepareResponse prepareSdbl(String query)
    {
        return post(
                "/v1/sdbl/prepare",
                "SDBL prepare",
                new Model.SdblPrepareRequest(query),
                new TypeReference<>() {});
    }

    InputStream scanSdbl(Model.SdblScanRequest request)
    {
        return stream("/v1/sdbl/scan", "SDBL scan", request);
    }

    ObjectMapper json()
    {
        return json;
    }

    private String resolveSchema(String requested)
    {
        return schemas().stream()
                .filter(schema -> schema.equalsIgnoreCase(requested))
                .findFirst()
                .orElse(null);
    }

    private InputStream stream(String path, String operation, Object request)
    {
        try {
            HttpRequest httpRequest = HttpRequest.newBuilder(baseUri.resolve(path))
                    .timeout(timeout)
                    .header("Content-Type", "application/json")
                    .POST(HttpRequest.BodyPublishers.ofByteArray(json.writeValueAsBytes(request)))
                    .build();
            HttpResponse<InputStream> response = http.send(httpRequest, HttpResponse.BodyHandlers.ofInputStream());
            if (response.statusCode() != 200) {
                String body = new String(response.body().readAllBytes(), StandardCharsets.UTF_8);
                throw failure(operation, response.statusCode(), body);
            }
            return response.body();
        }
        catch (IOException error) {
            throw new TrinoException(GENERIC_INTERNAL_ERROR, "open-sdbl " + operation + " I/O failed", error);
        }
        catch (InterruptedException error) {
            Thread.currentThread().interrupt();
            throw new TrinoException(GENERIC_INTERNAL_ERROR, "open-sdbl " + operation + " interrupted", error);
        }
    }

    private <T> T get(String path, TypeReference<T> type)
    {
        try {
            HttpRequest request = HttpRequest.newBuilder(baseUri.resolve(path))
                    .timeout(timeout)
                    .GET()
                    .build();
            HttpResponse<byte[]> response = http.send(request, HttpResponse.BodyHandlers.ofByteArray());
            if (response.statusCode() != 200) {
                throw failure(path, response.statusCode(), new String(response.body(), StandardCharsets.UTF_8));
            }
            return json.readValue(response.body(), type);
        }
        catch (IOException error) {
            throw new TrinoException(GENERIC_INTERNAL_ERROR, "open-sdbl metadata I/O failed", error);
        }
        catch (InterruptedException error) {
            Thread.currentThread().interrupt();
            throw new TrinoException(GENERIC_INTERNAL_ERROR, "open-sdbl metadata request interrupted", error);
        }
    }

    private <T> T post(String path, String operation, Object body, TypeReference<T> type)
    {
        try {
            HttpRequest request = HttpRequest.newBuilder(baseUri.resolve(path))
                    .timeout(timeout)
                    .header("Content-Type", "application/json")
                    .POST(HttpRequest.BodyPublishers.ofByteArray(json.writeValueAsBytes(body)))
                    .build();
            HttpResponse<byte[]> response = http.send(request, HttpResponse.BodyHandlers.ofByteArray());
            if (response.statusCode() != 200) {
                throw failure(operation, response.statusCode(), new String(response.body(), StandardCharsets.UTF_8));
            }
            return json.readValue(response.body(), type);
        }
        catch (IOException error) {
            throw new TrinoException(GENERIC_INTERNAL_ERROR, "open-sdbl " + operation + " I/O failed", error);
        }
        catch (InterruptedException error) {
            Thread.currentThread().interrupt();
            throw new TrinoException(GENERIC_INTERNAL_ERROR, "open-sdbl " + operation + " interrupted", error);
        }
    }

    private static String encode(String value)
    {
        return URLEncoder.encode(value, StandardCharsets.UTF_8);
    }

    private static TrinoException failure(String operation, int status, String body)
    {
        return new TrinoException(
                GENERIC_INTERNAL_ERROR,
                "open-sdbl " + operation + " failed with HTTP " + status + ": " + body);
    }
}
