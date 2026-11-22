# Version 1 extension object belonging

Measured with 1C 8.3.27.2342 on 2026-11-22 in a disposable file infobase.
The source was the native CFE of the test extension `ProbeRoleExtension`,
built by `ibcmd config import --out`. Its own `DefaultRole` is listed in
`Configuration.Properties.DefaultRoles`. The native version 3 descriptor
of that role has the tail `0,0,00000000-0000-0000-0000-000000000000,0`.

For each controlled CFE, only the header in the resource named by the
role's UUID was replaced. Version 1 uses identity
`{0,0,a845a91d-1f62-428b-bfef-36d05fa6d8f0}`. The role name, body and
remaining resources were kept. The pair GUIDs and tags were copied from
the version 3 header of the test extension's adopted `ProbeRole`.

Each CFE was exported directly with
`ibcmd config export --data=<scratch> --db-path=<disposable-infobase> --file=<cfe> --threads=1 <xml-directory>`.
All exports exited with status 0. The emitted `Roles/DefaultRole.xml`
matched the descriptor's UUID and name:

| Version 1 tail | Serialized input | Native XML result |
| --- | --- | --- |
| `0,0` | [own-zero.serialized](own-zero.serialized) | [DefaultRole.xml](DefaultRole.xml) |
| `0,1,cf4abea3-37b2-11d4-940f-008048da11f9,3` | [own-one.serialized](own-one.serialized) | Same XML |
| `0,2,cf4abea3-37b2-11d4-940f-008048da11f9,3,3245b9fe-57f9-43dc-9c11-030f0617ce16,3` | Two-pair control | Same XML |
| `1,0` | Belonging-code control for the same role | [DefaultRole-adopted.xml](DefaultRole-adopted.xml) |

The three `0` exports were byte-identical to the original version 3
own-role export. Native XML omits `ObjectBelonging` for that role. Changing
the code to `1` made the platform emit `ObjectBelonging=Adopted` for the
same UUID and name. The naturally stored adopted version 1 example
referenced by [name-adoption-095](../name-adoption-095/README.md) has a
one-pair tail and the same explicit Adopted value in
[its XML](../legacy-buh/d44efef3-da72-4d62-a1a2-3fc5a55e23b8.xml).

The serialized inputs and XML are saved as UTF-8 without BOM with LF
line endings.
