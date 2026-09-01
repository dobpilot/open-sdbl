package io.open_sdbl.trino;

import io.trino.spi.connector.Connector;
import io.trino.spi.connector.ConnectorContext;
import io.trino.spi.connector.ConnectorFactory;

import java.net.URI;
import java.time.Duration;
import java.util.Map;

/** Creates {@code open_sdbl} catalog instances from Trino catalog properties. */
public final class OpenSdblConnectorFactory
        implements ConnectorFactory
{
    @Override
    public String getName()
    {
        return "open_sdbl";
    }

    @Override
    public Connector create(String catalogName, Map<String, String> config, ConnectorContext context)
    {
        String uri = config.get("open-sdbl.uri");
        if (uri == null || uri.isBlank()) {
            throw new IllegalArgumentException("open-sdbl.uri is required");
        }
        Duration timeout = Duration.ofMillis(Long.parseLong(
                config.getOrDefault("open-sdbl.request-timeout-ms", "65000")));
        OpenSdblClient client = new OpenSdblClient(URI.create(uri), timeout);
        return new OpenSdblConnector(client, context.getTypeManager());
    }
}
