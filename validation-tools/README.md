These offline C++ VST3 hosts were used on macOS, without REAPER. They require
Steinberg's VST3 SDK headers (the JUCE source tree also includes them).

    clang++ -std=c++17 -O2 -I/path/to/VST3_SDK turbo_features.cpp -o turbo_features
    ./turbo_features '/path/to/Spectral Compressor Turbo.vst3/Contents/MacOS/Spectral Compressor Turbo' reports

The feature host applies real VST3 audio automation, compares Delta samples,
checks gain traces with monitoring on/off, and writes synthetic drum-loop audio
and gain traces. Input references are latency aligned. `vst3_parity.cpp` compares
matched parameter profiles between two plugin binaries, with optional one-bit
negative control. `auto_gain_host.cpp` renders parameter steps and checks gain
freeze/state/manual takeover. Source provenance and test limits are in VALIDATION.md.
