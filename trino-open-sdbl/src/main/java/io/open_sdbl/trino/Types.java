package io.open_sdbl.trino;

import io.trino.spi.type.Type;
import io.trino.spi.type.TypeManager;
import io.trino.spi.type.TypeSignature;
import io.trino.spi.type.TypeSignatureParameter;

import java.util.ArrayList;
import java.util.List;

import static io.trino.spi.type.TypeSignatureParameter.numericParameter;

/** Resolves the bounded set of Trino type signatures emitted by the Rust service. */
final class Types
{
    private Types() {}

    static Type resolve(TypeManager manager, String signature)
    {
        int opening = signature.indexOf('(');
        if (opening < 0) {
            return manager.getType(new TypeSignature(signature));
        }
        String base = signature.substring(0, opening);
        String arguments = signature.substring(opening + 1, signature.length() - 1);
        List<TypeSignatureParameter> parameters = new ArrayList<>();
        for (String argument : arguments.split(",")) {
            parameters.add(numericParameter(Long.parseLong(argument.trim())));
        }
        return manager.getType(new TypeSignature(base, parameters));
    }
}
