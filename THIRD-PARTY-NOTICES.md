# Third-party notices

## Package pattern matcher

The file pattern matcher in `crates/maestro-pattern/src/minimatch/`, re-exported
by packages, derives from minimatch 10.2.5 by Isaac Z. Schlueter
and contributors. This source is licensed under the
[Blue Oak Model License 1.0.0](https://blueoakcouncil.org/license/1.0.0).

Brace expansion uses brace-expansion 0.1.1 under MIT OR Apache-2.0.

## Package ignore rules

The ignore-rule matcher in `crates/maestro-pattern/src/ignore.rs`, shared by
package and skill discovery, derives from ignore 7.0.5.

```text
Copyright (c) 2013 Kael Zhang <i@kael.me>, contributors
http://kael.me/

Permission is hereby granted, free of charge, to any person obtaining
a copy of this software and associated documentation files (the
"Software"), to deal in the Software without restriction, including
without limitation the rights to use, copy, modify, merge, publish,
distribute, sublicense, and/or sell copies of the Software, and to
permit persons to whom the Software is furnished to do so, subject to
the following conditions:

The above copyright notice and this permission notice shall be
included in all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE
LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION
WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
```

## Package filesystem expansion

The synchronous filesystem expansion in
`crates/maestro-packages/src/package_manager/glob/` derives from glob 13.0.6 and
path-scurry 2.0.2 by Isaac Z. Schlueter and contributors, licensed under the
[Blue Oak Model License 1.0.0](https://blueoakcouncil.org/license/1.0.0).
The compiled filesystem-pattern policy uses the matcher attribution above.

These are owned implementations; the original packages are not runtime
dependencies.
