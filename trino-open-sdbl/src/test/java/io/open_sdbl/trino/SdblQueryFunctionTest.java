package io.open_sdbl.trino;

import org.junit.jupiter.api.Test;

import java.net.URI;
import java.time.Duration;

import static io.trino.spi.function.table.ReturnTypeSpecification.GenericTable;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class SdblQueryFunctionTest
{
    @Test
    void declaresThePolymorphicSystemSdblContract()
    {
        OpenSdblClient client = new OpenSdblClient(URI.create("http://127.0.0.1:1"), Duration.ofSeconds(1));
        SdblQueryFunction function = new SdblQueryFunction(client, null);

        assertEquals("system", function.getSchema());
        assertEquals("sdbl", function.getName());
        assertEquals(1, function.getArguments().size());
        assertEquals("QUERY", function.getArguments().getFirst().getName());
        assertTrue(function.getReturnTypeSpecification() instanceof GenericTable);
    }
}
