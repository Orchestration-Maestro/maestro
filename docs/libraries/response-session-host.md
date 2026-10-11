## Approved response-session host libraries

Outside browser targets, `maestro-models` uses `uuid =1.27.0` with defaults off
and no features, together with existing `getrandom =0.4.3`, for fallible v4 request
IDs. Unix adds `rustix =1.1.5`, defaults off, `std` and `system`, for kernel release;
Windows adds `windows-version =0.100.0` with no features for major.minor.build
release spelling. Its only added dependency is `windows-link =0.100.0` without
features. These selected library licenses are MIT OR Apache-2.0; no library type
enters the provider interface.
