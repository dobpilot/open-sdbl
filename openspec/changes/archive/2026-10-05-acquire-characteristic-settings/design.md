# Design

Use the existing serialized parser and runtime metadata projection for measured class 34 resources. Native 8.3.27 XML and resource fixtures from open-bsl are the evidence; catalog layouts are not reused by assumption. Only complete recognized class and attribute collections are published. Unknown types or malformed ownership produce None rather than partial success. References retain the existing object_reference_type projection. Named type expansion uses the existing shared resolver once per acquisition.

Version 0.9.7 applies consistently to workspace packages. Work is isolated on a dedicated branch.
