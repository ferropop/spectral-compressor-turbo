# Turbo 1.1.1 verification

Reset uses Command-click on macOS and Ctrl-click on Windows, consistently across
numeric sliders, parameter toggles, and response-graph handles. Regression checks
use the native platform modifier; the opposite modifier remains an ordinary
click. Toggle checks start from the opposite of each default, including defaults
that are enabled. No processing code changed in this patch.

Both platform builds run the plugin/editor tests and framework resize checks.
The exact delivered universal Mac VST3 is checked locally with full pluginval
and sample-exact audio comparison against 1.1.0. Detailed results are included
in the delivery's Validation folder. Hosted CI uses headless pluginval; native
modifier-click/DAW shortcut routing and Windows physical GUI acceptance are not
claimed. No REAPER is used. Mac bundles are ad-hoc signed, without notarization.

## Previous 1.1.0 feature verification

35 plugin/editor tests passed, including production relative sliders, Ctrl-click
reset, edge-node release/regrab at multiple scales, palette save/recall/delete,
contrast checks, and the real Settings-button event route. Three framework resize
checks remain part of the platform CI job.

The native Mac standalone preview was checked for CGA default, live Turbo Dark
selection, header controls, and readable palette-manager layout. The VST3 passed
full local pluginval 1.0.4 at strictness 5, seed 42. No REAPER was used.

With Auto Gain and Delta off, 1.1.0 matched 1.0.0 bit for bit across 108 cases
and 28,311,552 float32 samples. This covers the tested stereo matrix, not all
possible input/parameter combinations.

Delta checks: 108 configurations covering 44.1/48/96 kHz, blocks 64/257/1024,
dry/mixed/wet settings, neutral/active processing and two Output Gain settings.
Maximum error against latency-aligned dry minus pre-gain processing was
2.012e-7. Fully dry Delta is exact silence. Auto Gain traces with Delta on/off
were bit-identical across 1495 blocks.

Four sample-exact synthetic drum-loop cases: settled gain movement was
0.000-0.126 dB with Smart, versus 3.776-6.705 dB with momentary matching.
Independent FFmpeg EBU R128 measurements of the last six seconds matched the
input's average loudness within 0.1 LU. This is controlled-loop evidence;
listening acceptance on the user's own loops remains valuable.

The previous 46 parameter-step/automatic-gain render cases also passed independent
FFmpeg momentary checks (maximum post-one-second error 0.467 LU in the initial
1.1 build). Gain disable/freeze, manual takeover, saved state, and silence checks
passed. The longer Smart average needs three seconds of post-edit history for its
final average calibration. Fast relearn preserves responsive editing; steady
tracking has a 0.1 dB deadband and a 0.5 dB/s rate limit.

Native Mac/Windows CI uses headless pluginval because hosted runners lack usable
OpenGL contexts. Full Mac editor construction is additionally checked locally.
Windows physical GUI and Intel-Mac runtime acceptance are not claimed.
Use stereo instances: the inherited mono processing issue remains. Mac bundles
are ad-hoc signed, without Apple notarization.
