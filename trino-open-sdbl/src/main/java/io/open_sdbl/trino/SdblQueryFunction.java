package io.open_sdbl.trino;

import io.airlift.slice.Slice;
import io.trino.spi.TrinoException;
import io.trino.spi.connector.ConnectorAccessControl;
import io.trino.spi.connector.ConnectorSession;
import io.trino.spi.connector.ConnectorTransactionHandle;
import io.trino.spi.function.table.AbstractConnectorTableFunction;
import io.trino.spi.function.table.Argument;
import io.trino.spi.function.table.Descriptor;
import io.trino.spi.function.table.ScalarArgument;
import io.trino.spi.function.table.ScalarArgumentSpecification;
import io.trino.spi.function.table.TableFunctionAnalysis;
import io.trino.spi.type.TypeManager;

import java.util.List;
import java.util.Map;
import java.util.Optional;

import static io.trino.spi.StandardErrorCode.INVALID_FUNCTION_ARGUMENT;
import static io.trino.spi.function.table.ReturnTypeSpecification.GenericTable.GENERIC_TABLE;
import static io.trino.spi.type.VarcharType.VARCHAR;

/**
 * Polymorphic table function that exposes the result of a read-only SDBL query.
 *
 * <p>Analysis asks the Rust service to compile and prepare the query without reading rows. The
 * returned descriptor becomes the table function's dynamic Trino row type.
 */
final class SdblQueryFunction
        extends AbstractConnectorTableFunction
{
    private final OpenSdblClient client;
    private final TypeManager types;

    SdblQueryFunction(OpenSdblClient client, TypeManager types)
    {
        super(
                "system",
                "sdbl",
                List.of(ScalarArgumentSpecification.builder().name("QUERY").type(VARCHAR).build()),
                GENERIC_TABLE);
        this.client = client;
        this.types = types;
    }

    @Override
    public TableFunctionAnalysis analyze(
            ConnectorSession session,
            ConnectorTransactionHandle transaction,
            Map<String, Argument> arguments,
            ConnectorAccessControl accessControl)
    {
        ScalarArgument argument = (ScalarArgument) arguments.get("QUERY");
        String query = ((Slice) argument.getValue()).toStringUtf8();
        if (query.isBlank()) {
            throw new TrinoException(INVALID_FUNCTION_ARGUMENT, "SDBL query must not be empty");
        }
        Model.SdblPrepareResponse prepared = client.prepareSdbl(query);
        List<Descriptor.Field> fields = prepared.columns().stream()
                .map(column -> new Descriptor.Field(column.name(), Optional.of(Types.resolve(types, column.type()))))
                .toList();
        return TableFunctionAnalysis.builder()
                .returnedType(new Descriptor(fields))
                .handle(new Model.SdblFunctionHandle(query, prepared.columns()))
                .build();
    }
}
