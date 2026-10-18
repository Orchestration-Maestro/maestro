# Tests worth keeping

Name the break the test catches. Exercise real behaviour through the public interface, with an independent
expected result: for example, a cart with known prices has a known total, and checkout returns a retrievable order.
Do not recompute expected output using the implementation's algorithm.

Use arrange, act, assert: set up only what matters, perform the operation and observe its contract.
Prefer state and returned results over interaction counts. Interaction assertions belong only where the
interaction itself is the caller's promised behaviour. Avoid checking a database behind an interface merely
because it is convenient; retrieve the result through the interface that promises it.

Make a mental mutation check: what plausible broken implementation would fail this assertion? If none would,
the assertion does not protect the behaviour. Do not add a test for a line that already has the same observation.
Keep tests DAMP (descriptive and meaningful) rather than hiding their intent behind excessive shared setup.

Prefer the real implementation, then a working fake, then a stub response, then a mock interaction expectation.
Control external effects, not internal collaborators. Keep cases deterministic and independent, and use
synchronization rather than timing guesses when ordering is the contract.
