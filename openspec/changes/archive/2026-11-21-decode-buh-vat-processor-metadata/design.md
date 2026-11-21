# Design

Keep generic object_header and header acceptance unchanged. Recognize legacy identity 1/7 only in the measured class 16/12 data processor path. Validate reference 0/0/GUID, nonempty name, synonym tuple, empty comment and zero trailing flags. Reuse collection/declaration parsing and descriptor uniqueness checks. Publish field declarations only after complete collection validation. Section membership and each section field collection remain independently complete or unknown. Preserve prepared 0.9.17 changes; pending SQL corpus expectations remain untouched.
