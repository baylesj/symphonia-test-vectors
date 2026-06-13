use std::env;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use serde::{Deserialize, Serialize};
use md5::{Md5, Digest};

use symphonia::core::audio::sample::Sample;
use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::errors::Error;
use symphonia::core::formats::FormatOptions;
use symphonia::core::formats::probe::Hint;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::formats::TrackType;

#[derive(Serialize, Deserialize)]
struct Expected {
    duration_frames: u64,
    sample_rate: u32,
    channels: u32,
    symphonia_pcm_md5: String,
    ffmpeg_pcm_md5: String,
}

#[derive(Serialize, Deserialize)]
struct Sidecar {
    file: String,
    expected: Expected,
}

fn get_metadata(file_path: &str) -> Result<(u64, u32, u32), Box<dyn std::error::Error>> {
    let output = Command::new("ffprobe")
        .args(&["-v", "error", "-show_entries", "stream=channels,sample_rate,duration", "-of", "json", file_path])
        .output()?;

    if !output.status.success() {
        return Err(format!("ffprobe failed: {}", String::from_utf8_lossy(&output.stderr)).into());
    }

    let stdout = String::from_utf8(output.stdout)?;
    let parsed: serde_json::Value = serde_json::from_str(&stdout)?;
    
    let stream = &parsed["streams"][0];
    
    let sample_rate_str = stream["sample_rate"].as_str().ok_or("Missing or invalid sample_rate")?;
    let sample_rate: u32 = sample_rate_str.parse()?;
    
    let channels = stream["channels"].as_u64().ok_or("Missing or invalid channels")? as u32;
    
    let duration_str = stream["duration"].as_str().ok_or("Missing or invalid duration")?;
    let duration_sec: f64 = duration_str.parse()?;
    
    let duration_frames = (duration_sec * sample_rate as f64).round() as u64;

    Ok((duration_frames, sample_rate, channels))
}

fn get_ffmpeg_pcm_md5(file_path: &str) -> Result<String, Box<dyn std::error::Error>> {
    let output = Command::new("ffmpeg")
        .args(&["-v", "error", "-i", file_path, "-f", "s16le", "-acodec", "pcm_s16le", "-"])
        .stdout(Stdio::piped())
        .output()?;

    if !output.status.success() {
        return Err(format!("ffmpeg failed: {}", String::from_utf8_lossy(&output.stderr)).into());
    }

    let mut hasher = Md5::new();
    hasher.update(&output.stdout);
    let result = hasher.finalize();

    Ok(hex::encode(result))
}

fn get_symphonia_pcm_md5(file_path: &str) -> Result<String, Box<dyn std::error::Error>> {
    let file = Box::new(File::open(Path::new(file_path))?);
    let mss = MediaSourceStream::new(file, Default::default());
    let hint = Hint::new();
    let fmt_opts: FormatOptions = Default::default();
    let meta_opts: MetadataOptions = Default::default();
    let dec_opts: AudioDecoderOptions = Default::default();

    let mut format =
        symphonia::default::get_probe().probe(&hint, mss, fmt_opts, meta_opts)?;

    let track = format.default_track(TrackType::Audio).ok_or("No audio track found")?;
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(track.codec_params.as_ref().unwrap().audio().unwrap(), &dec_opts)?;

    let track_id = track.id;
    let mut hasher = Md5::new();
    let mut samples: Vec<i16> = Default::default();

    loop {
        let packet = match format.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => break, // EOF
            Err(Error::ResetRequired) => continue,
            Err(Error::IoError(err)) if err.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(_) => break, // Any other error ends the loop
        };

        if packet.track_id != track_id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(audio_buf) => {
                samples.resize(audio_buf.samples_interleaved(), i16::MID);
                audio_buf.copy_to_slice_interleaved(&mut samples);
                
                // Convert i16 samples to bytes (little endian to match s16le from ffmpeg)
                for sample in &samples {
                    hasher.update(&sample.to_le_bytes());
                }
            }
            Err(Error::DecodeError(_)) => (),
            Err(_) => break,
        }
    }

    let result = hasher.finalize();
    Ok(hex::encode(result))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <file>", args[0]);
        std::process::exit(1);
    }

    let file_path = &args[1];
    if !Path::new(file_path).is_file() {
        eprintln!("File not found: {}", file_path);
        std::process::exit(1);
    }

    println!("Processing {}...", file_path);

    let (duration_frames, sample_rate, channels) = get_metadata(file_path)?;
    let ffmpeg_pcm_md5 = get_ffmpeg_pcm_md5(file_path)?;
    let symphonia_pcm_md5 = get_symphonia_pcm_md5(file_path)?;

    let expected = Expected {
        duration_frames,
        sample_rate,
        channels,
        symphonia_pcm_md5,
        ffmpeg_pcm_md5,
    };

    let file_name = Path::new(file_path).file_name().unwrap().to_str().unwrap().to_string();
    let sidecar = Sidecar {
        file: file_name,
        expected,
    };

    let sidecar_path = format!("{}.json", file_path);
    let json_string = serde_json::to_string_pretty(&sidecar)?;

    let mut file = File::create(&sidecar_path)?;
    file.write_all(json_string.as_bytes())?;
    file.write_all(b"\n")?;

    println!("Sidecar generated: {}", sidecar_path);

    Ok(())
}
