# Spectral Compressor Turbo

An FFT compressor by **Robbert van der Helm & modified by ferropop**, with a dark interface
and interactive processing response shaper, among many other QOL improvements.
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

MAC USERS : you may have to run the following command from Terminal:
sudo xattr -cr /Library/Audio/Plug-Ins/VST3/Spectral\ Compressor\ Turbo.vst3 

## Controls

- Every slider uses relative dragging. Clicking alone does not change its value.
  Shift-drag makes fine adjustments; Alt/Option-click opens numeric entry.
- Command-click on macOS / Ctrl-click on Windows resets a parameter to its default.
- Auto Gain Compensation sets the actual Output Gain. Smart averaging is on by
  default: K-weighted 3-second input/output energy averages, gentle steady
  tracking, and a brief fast relearn after processing edits. It reduces recurring
  kick/tail gain pumping. Settings can restore the original 400 ms momentary
  behavior. Turning Auto Gain off leaves the current Output Gain in place.
- DELTA auditions latency-aligned dry minus processed mix, before the sole Output
  Gain stage, with a 5 ms switching fade. Auto Gain continues measuring the normal
  processed signal so the difference signal cannot influence gain calibration.
- Settings contains a palette manager. CGA is the default, using the supplied
  black, white, #8BFCFB cyan, and #DC40F0 magenta swatches. Turbo Dark, Amber,
  Ocean, and Paper are included. Edit the four hex colors, apply, name/save, or
  delete a custom preset. Custom presets persist in the user's app settings;
  the active palette also travels with the plugin's saved state.
- Click-drag in the graph creates a bell under your pointer. Drag nodes to shape
  processing, Alt-click deletes a bell, and wheel changes the selected node's Q.
  The permanent HPF/LPF handles define the processing range.
- The response curve weights per-bin compression/expansion strength. It applies
  no audio EQ. Gain reduction/expansion meters and threshold traces remain visible.
- Use the corner handle or − / percentage / + controls for proportional 50–200% zoom.

The dark theme uses dim charcoal surfaces and softer white text, with separate
palette-specific response, downward threshold, and upward threshold colors.
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
checks cover processing, automation, and saved state. CI skips GUI tests because
hosted runners lack a usable OpenGL context; full editor construction is checked
on the local Mac.
platform runner tests do not establish physical DAW mouse interaction.
The original inherited mono processing issue remains: use stereo instances.
This visual/branding release retains the Custom edition's audio processing.
No REAPER is used for automated validation.

GPL-3.0-or-later. See [THIRD-PARTY.md](THIRD-PARTY.md) and
[original Spectral Compressor](https://github.com/robbert-vdh/nih-plug/tree/master/plugins/spectral_compressor).
