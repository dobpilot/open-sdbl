package io.open_sdbl.trino;

import io.trino.spi.connector.Connector;
import io.trino.spi.connector.ConnectorMetadata;
import io.trino.spi.connector.ConnectorRecordSetProvider;
import io.trino.spi.connector.ConnectorSession;
import io.trino.spi.connector.ConnectorSplitManager;
import io.trino.spi.connector.ConnectorSplitSource;
import io.trino.spi.connector.ConnectorTableHandle;
import io.trino.spi.connector.ConnectorTransactionHandle;
import io.trino.spi.connector.Constraint;
import io.trino.spi.connector.DynamicFilter;
import io.trino.spi.connector.FixedSplitSource;
import io.trino.spi.function.table.ConnectorTableFunction;
import io.trino.spi.transaction.IsolationLevel;
import io.trino.spi.type.TypeManager;

import java.util.Set;

/** Read-only Trino connector backed by the open-sdbl Rust service. */
final class OpenSdblConnector
        implements Connector
{
    private final OpenSdblMetadata metadata;
    private final OpenSdblRecordSetProvider records;
    private final Set<ConnectorTableFunction> tableFunctions;

    OpenSdblConnector(OpenSdblClient client, TypeManager types)
    {
        this.metadata = new OpenSdblMetadata(client, types);
        this.records = new OpenSdblRecordSetProvider(client, types);
        this.tableFunctions = Set.of(new SdblQueryFunction(client, types));
    }

    @Override
    public ConnectorTransactionHandle beginTransaction(IsolationLevel level, boolean readOnly, boolean autoCommit)
    {
        return Model.Transaction.INSTANCE;
    }

    @Override
    public ConnectorMetadata getMetadata(ConnectorSession session, ConnectorTransactionHandle transaction)
    {
        return metadata;
    }

    @Override
    public ConnectorSplitManager getSplitManager()
    {
        return new ConnectorSplitManager()
        {
            @Override
            public ConnectorSplitSource getSplits(
                    ConnectorTransactionHandle transaction,
                    ConnectorSession session,
                    ConnectorTableHandle table,
                    DynamicFilter dynamicFilter,
                    Constraint constraint)
            {
                // The MVP intentionally exposes one streaming split. The handle leaves room for
                // metadata-aware range partitioning without pretending that scans are parallel.
                return new FixedSplitSource(Model.Split.INSTANCE);
            }
        };
    }

    @Override
    public ConnectorRecordSetProvider getRecordSetProvider()
    {
        return records;
    }

    @Override
    public Set<ConnectorTableFunction> getTableFunctions()
    {
        return tableFunctions;
    }
}
