# Decode user separation in extended records

## Why

UNF session initialization needs the selected user's acquired data-separation structure. The SDK treats a known zero count as unknown when its record has the measured extended password suffix.

## What changes

Recognize complete extended SHA-256 and SHA-512 record envelopes when acquiring data-separation bindings. Validate the same counters and types as the existing extended envelope decoder. Keep malformed and unmeasured records unknown.
