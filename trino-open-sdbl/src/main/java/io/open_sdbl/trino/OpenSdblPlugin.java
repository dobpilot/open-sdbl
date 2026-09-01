package io.open_sdbl.trino;

import io.trino.spi.Plugin;
import io.trino.spi.connector.ConnectorFactory;

import java.util.List;

/** Trino plugin entry point for the {@code open_sdbl} connector. */
public final class OpenSdblPlugin
        implements Plugin
{
    @Override
    public Iterable<ConnectorFactory> getConnectorFactories()
    {
        return List.of(new OpenSdblConnectorFactory());
    }
}
