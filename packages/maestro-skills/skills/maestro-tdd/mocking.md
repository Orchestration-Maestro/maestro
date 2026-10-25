# Mock only at system boundaries

Use mocks for external services, time, randomness and sometimes filesystem or database effects.
Prefer a real test database or filesystem where practical. Do not mock your own classes and internal collaborators
when their real behaviour can be reached through the public seam.

Pass a boundary dependency into the module rather than constructing it inside the operation.
Prefer operation-specific interfaces over one generic fetch function whose mock must redispatch on strings.
Each controlled response should identify the actual operation and return its real contract shape.

Keep the surrounding behaviour real. A fake payment response is useful; mocking the entire checkout skips the
validation and error policy the test needs to protect. Assert the returned order and state, not the incidental
number of internal method calls. See [tests](tests.md) for the behavioural test discipline.
