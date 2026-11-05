# Test TLS identity

`gateway-cert.pem` and `gateway-key.pem` are a self-signed P-256
certificate for `localhost` / `127.0.0.1`, generated once with OpenSSL for
the gateway's TLS tests. They protect nothing and must never be used
outside tests.
