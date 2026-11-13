# Third-Party Notices

Pubseed's own source code is licensed under the MIT License. See `LICENSE`.

This file covers code and libraries in Pubseed that come from other projects. Their original licences still apply to them.

## Pubsplash

Parts of `src/acc.rs` and `src/speech.rs` are adapted from Pubsplash, an accessible AudioPub streaming client: the way list boxes and other controls are made to read properly with screen readers, and the UI Automation provider Pubseed uses to speak through the running screen reader.

Upstream repository:
https://github.com/cha0t1cnu3tral/pubsplash

Pubsplash is licensed as follows:

```
MIT License

Copyright (c) 2026 Brandon Cross

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## wxWidgets

Pubseed's interface is built on wxWidgets, which is compiled into the program through wxDragon. wxWidgets is distributed under the wxWindows Library Licence, an LGPL variant with an exception that allows programs built with it to be distributed under their own terms.

Project:
https://www.wxwidgets.org

## Symphonia

Audio is decoded by Symphonia, used through rodio. Symphonia is licensed under the Mozilla Public License 2.0, and Pubseed uses it unmodified. Its source is available at:
https://github.com/pdeljanov/Symphonia

## Rust crates

Pubseed depends on a number of other Rust crates, listed in `Cargo.toml` and `Cargo.lock`. Each is used under its own licence, almost all of them MIT, Apache-2.0, or a choice between the two. Where a crate offers more than one licence, Pubseed uses it under MIT or Apache-2.0.
