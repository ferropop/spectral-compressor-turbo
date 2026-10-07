// Local editor preview for the custom build. The VST3 and CLAP use the same editor.
use nih_plug::wrapper::standalone::nih_export_standalone_with_args;
use spectral_compressor::SpectralCompressor;

fn main() {
    let mut args: Vec<String> = std::env::args().collect();
    // A normal double-click opens the editor without connecting audio devices.
    // Explicit CLI arguments still allow the framework's audio backends.
    if args.len() == 1 {
        args.extend([String::from("--backend"), String::from("dummy")]);
    }
    if !nih_export_standalone_with_args::<SpectralCompressor, _>(args) {
        std::process::exit(1);
    }
}
