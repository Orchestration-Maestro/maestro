# Examples

Extension and SDK examples for Maestro. No example ships yet: each folder holds
an index page that gains a row for an example when it lands.

The default system prompt points the model at this folder. It sits beside the
package manifest, at `<package>/examples`, so the same path works in a source
checkout and in a standalone layout.

## Directories

### [sdk/](sdk/)
Programmatic sessions through the application interface: models, prompts,
tools, extensions and session management.

### [extensions/](extensions/)
Extensions that register tools, commands and event handlers through the
extension interface.

## Documentation

- [SDK examples](sdk/README.md)
- [Extension examples](extensions/README.md)
- [Writing an extension](../docs/extensions/guest.md)
- [Extension loading](../docs/extensions/loading.md)
- [Skills](../docs/resources/skills.md)

Every example file that does not ship yet is listed, with its owner, in
[the examples tracking issue](https://github.com/Orchestration-Maestro/maestro/issues/2074).
