# Turbo 1.0.0 verification

Local Apple Silicon build: 28 plugin/editor regression tests, 3 resize tests,
and pluginval 1.0.4 at strictness 5, seed 42 passed. The native dark editor was
visually checked at 90% and 100% zoom, including the literal fullwidth credit
and Instagram address. Physical DAW gestures and Windows appearance remain
manual acceptance checks. No REAPER was used.

Custom.6 versus Turbo: 108 stereo cases across 44.1/48/96 kHz, blocks
64/257/512/1024, and 9 parameter profiles; 28,311,552 float32 samples compared
bit for bit, with zero mismatches. Auto-gain and response DSP source is identical.
This is evidence for the tested matrix, not a proof for every possible input.

Independent FFmpeg EBU R128 checks: 46 cases, maximum loudness error from one
second after a parameter change onward 0.464 LU (0.5 LU tolerance). Gain freezes
when disabled, and state/silence cases pass. Response-shaper checks pass,
including 11,286,000 exactly equal samples with neutral compressor ratios and
27 checks for the sole Output Gain stage.

Text contrast ratios: main text 14.03, muted title text 8.39, creator credit
10.25, slider text on fill 6.80; control borders on the main background 3.84.

The GitHub release archives contain native CI regression-test and pluginval
logs for their platform. CI runs pluginval with --skip-gui-tests because hosted runners have no usable
OpenGL context. Full GUI construction is validated on the local Mac.
A universal Mac binary is validated on Apple Silicon;
its Intel slice is built and architecture checked. Windows x64 is validated on
a native Windows runner. See release-specific evidence rather than assuming a
cross-compiled binary was exercised on Windows.

The inherited mono processing issue remains; use stereo instances. Mac builds
are ad-hoc signed and are not notarized. No DAW mouse acceptance is claimed.
