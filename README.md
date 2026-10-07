# Spectral Compressor Turbo

An FFT compressor by **Robbert van der Helm & ferropop**, with a dark interface
and interactive processing response shaper. GUI credit: **by ＦＥＲＲＯ** ·
[www.instagram.com/ferropop](https://www.instagram.com/ferropop).

[Download Mac and Windows VST3/CLAP builds](https://github.com/ferropop/spectral-compressor-turbo/releases)

## Install

Download and extract the archive for your platform; copy the whole `.vst3`
folder to your VST3 folder, then rescan in your DAW.

- Mac (Apple Silicon and Intel): `~/Library/Audio/Plug-Ins/VST3/`
- Windows x64: `C:\Program Files\Common Files\VST3\`
- CLAP: `~/Library/Audio/Plug-Ins/CLAP/` on Mac, or `C:\Program Files\Common Files\CLAP\` on Windows.

Mac bundles are ad-hoc signed, without Apple notarization. The standalone app
opens an editor preview using the dummy audio backend; the plugin is the DAW effect.
Turbo has a separate plugin ID and coexists with the original and Custom editions.
Existing sessions do not automatically replace older plugin instances.

## Controls

- Every slider uses relative dragging. Clicking alone does not change its value.
  Shift-drag makes fine adjustments; Alt/Option-click opens numeric entry.
- Ctrl-click resets a parameter to its default.
- Auto Gain Compensation measures K-weighted momentary input/output loudness and
  sets the actual Output Gain. It tracks parameter changes with smooth matching;
  turning it off leaves the current gain in place.
- Click-drag in the graph creates a bell under your pointer. Drag nodes to shape
  processing, Alt-click deletes a bell, and wheel changes the selected node's Q.
  The permanent HPF/LPF handles define the processing range.
- The response curve weights per-bin compression/expansion strength. It applies
  no audio EQ. Gain reduction/expansion meters and threshold traces remain visible.
- Use the corner handle or − / percentage / + controls for proportional 50–200% zoom.

The dark theme uses dim charcoal surfaces and softer white text, with separate
gold response, blue downward threshold, and green upward threshold colors.
This follows the emphasis on contrast and restrained background colors in
[Apple's Dark Mode guidance](https://developer.apple.com/design/human-interface-guidelines/dark-mode).

## Build and edit

Install Rust 1.96.1 and the platform's native C/C++ build tools. From this folder:

```sh
cargo xtask bundle spectral_compressor --release --locked
```

For a universal Mac build, install `aarch64-apple-darwin` and
`x86_64-apple-darwin` Rust targets, run
`cargo xtask bundle-universal spectral_compressor --release --locked`, then
`python3 scripts/sign_macos.py`. On Windows, use an MSVC developer environment.
Builds appear in `target/bundled/`. See `.github/workflows/turbo-build.yml` for
reproducible platform builds and native pluginval validation.

Edit `plugins/spectral_compressor/src/editor/theme.css` for colors and
`src/editor.rs` for layout. Full source, locked dependencies, framework patches,
and font/license assets are included. Keep work in WORKING; LATEST and VERSIONS
are completed delivery packages in the local Codex project.

## Verification and known limits

Release archives include build/test evidence under Validation. Native pluginval
checks cover processing, automation, saved state, and editor construction;
platform runner tests do not establish physical DAW mouse interaction.
The original inherited mono processing issue remains: use stereo instances.
This visual/branding release retains the Custom edition's audio processing.
No REAPER is used for automated validation.

GPL-3.0-or-later. See [THIRD-PARTY.md](THIRD-PARTY.md) and
[original Spectral Compressor](https://github.com/robbert-vdh/nih-plug/tree/master/plugins/spectral_compressor).
