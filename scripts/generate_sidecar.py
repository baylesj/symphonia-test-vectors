#!/usr/bin/env python3
import argparse
import subprocess
import json
import os
import hashlib

def get_metadata(file_path):
    cmd = [
        "ffprobe", "-v", "error", "-show_entries",
        "stream=channels,sample_rate,duration",
        "-of", "json", file_path
    ]
    result = subprocess.run(cmd, capture_output=True, text=True, check=True)
    data = json.loads(result.stdout)
    stream = data['streams'][0]
    
    sample_rate = int(stream['sample_rate'])
    channels = int(stream['channels'])
    duration_sec = float(stream['duration'])
    duration_frames = int(round(duration_sec * sample_rate))
    
    return duration_frames, sample_rate, channels

def get_pcm_md5(file_path):
    cmd = [
        "ffmpeg", "-v", "error", "-i", file_path,
        "-f", "s16le", "-acodec", "pcm_s16le", "-"
    ]
    process = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    stdout, stderr = process.communicate()
    
    if process.returncode != 0:
        raise RuntimeError(f"ffmpeg failed: {stderr.decode('utf-8')}")
        
    return hashlib.md5(stdout).hexdigest()

def main():
    parser = argparse.ArgumentParser(description="Generate expected result JSON sidecar for test vectors")
    parser.add_argument("file", help="Path to the media file")
    args = parser.parse_args()

    file_path = args.file
    if not os.path.isfile(file_path):
        print(f"File not found: {file_path}")
        return

    print(f"Processing {file_path}...")
    
    try:
        duration_frames, sample_rate, channels = get_metadata(file_path)
        pcm_md5 = get_pcm_md5(file_path)
    except Exception as e:
        print(f"Error processing file: {e}")
        return

    expected = {
        "duration_frames": duration_frames,
        "sample_rate": sample_rate,
        "channels": channels,
        "pcm_md5": pcm_md5
    }

    file_name = os.path.basename(file_path)
    sidecar_data = {
        "file": file_name,
        "expected": expected
    }

    sidecar_path = os.path.splitext(file_path)[0] + ".json"
    with open(sidecar_path, "w") as f:
        json.dump(sidecar_data, f, indent=2)
        f.write("\n")

    print(f"Sidecar generated: {sidecar_path}")

if __name__ == "__main__":
    main()
