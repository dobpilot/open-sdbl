# Design

Native evidence is limited to the preserved BUH role payload loaded into an isolated configuration and a separately generated explicitly empty role. Both are valid native roles. The first exports no Rights.xml, while the second exports empty Rights.xml and stores `.0`; their descriptor rights-presence field differs (0/1). Selected users with the role retain role availability and do not gain any of the six tested configuration permissions compared with a baseline user. Two shared client permissions come from the same separate client role.

The database fallback will accept the entire measured Role descriptor profile, including matching own identity, class/header versions, zero rights-presence and remaining flags, a counted synonym, strings, nil owner-reference and complete root. It will construct no objects or templates and no default grants. Unknown or incomplete forms stay unreadable. A declared `.0` takes precedence even when its content is unavailable or malformed.

There is no public API change and no inference that every missing `.0` means empty rights. Original extension adoption is not derived by name; the base configuration contains no same-name role. The exact transferred payload is tested, without extrapolating its shape to borrowed roles or other versions.
