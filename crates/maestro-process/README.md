# maestro-process

Native captured child-process lifecycles: spawn, raw pipe collection, exit and
EOF observation, termination and reaping. A wait takes `WaitOptions`: whether stderr is kept and an
optional post-exit grace after which the bytes read so far are returned, and
`Termination::TerminateThenKill` kills a child still running after its delay.

See the [process contract](../../docs/specs/maestro-port.md#crates-and-delivery-order).
