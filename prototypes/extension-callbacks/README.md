# Extension callback prototype

A throwaway proof that an authored extension can register callbacks, receive typed
events and finish a pending continuation through one native-async component
contract, with the same author function running through the generated component
adapter and a controlled native adapter.

Run it from the repository root:

```sh
just extension-callbacks-prototype
```

- `wit/` is the canonical contract: records and host-owned resources (`types`),
  registrations and actions (`host`), invocations (`guest`).
- `sdk/` is the author seam: generated records re-exported under domain names,
  callback and context facades, the component adapter and the controlled adapter.
- `extension-example/` is the author function, with a test that runs it through the
  controlled adapter. `extension-example-component/` builds it as a component.
- `probe/` is a test-only host that instantiates the component and runs the same
  scenario, inspects the component's ABI, and keeps the failing exported-resource
  registration (`counterexample/`) as a compile failure.
- `shared/scenario.rs` holds the transcript both adapters must produce.
