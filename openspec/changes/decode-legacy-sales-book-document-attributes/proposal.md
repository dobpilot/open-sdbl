# Decode measured legacy sales book document attributes

## Why
Original BUH compilation reaches the sales book document object module and cannot bind its Boolean manual-correction attribute because the SDK does not project its legacy document profile. All seven descriptors exist but their declaration and owner membership are unknown.

## What Changes
Recognize the retained class 38/51 document owner with identity 1/7 and extend the existing strict staged own-attribute decoder with its measured 4/5 wrappers and 1/7 identities. Preserve the existing class 40/53 profile. No new public API, runtime object behavior or numbering inference.

## Impact
Only selection and local validation of legacy document own attributes, native fixtures and regression tests. Versions remain 0.9.20 pending release authorization.
