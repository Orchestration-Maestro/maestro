# Extension Examples

Example extensions for Maestro.

## Usage

Build the author component as a WebAssembly component:

```sh
mise exec -- just extension-author-component
```

An extension crate is a `cdylib` whose root names its factory. The
[extension guide](../../docs/extensions/guest.md) describes the interface it
registers.

## Examples

| Extension             | Description                                                                   |
| --------------------- | ----------------------------------------------------------------------------- |
| `author_component.rs` | Registers tools, commands and event handlers through the extension interface. |

The remaining extensions of the reference set arrive one ticket at a time; the
tracking issue is #2074.
