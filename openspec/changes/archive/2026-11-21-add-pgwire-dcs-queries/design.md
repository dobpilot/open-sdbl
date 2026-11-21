## XML parsing

Use `xmlparser`, a dependency-free, iterative XML tokenizer, to check
size-sensitive structure before passing client XML to the existing
`roxmltree` parser. Limit nesting, attributes per element, namespace
declarations and adjacent text segments to bound recursive parsing and
the cost of building the tree. Check the estimated initial tree allocation
against the gateway budget. The tree parser validates names and
namespaces and decodes XML character data. Both dependencies are
confined to the pgwire crate.
