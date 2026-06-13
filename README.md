# Symphonia Test Vectors

This repository contains a curated corpus of media files used for regression testing the [Symphonia](https://github.com/pdeljanov/Symphonia) multimedia decoding library.

## Structure and Expected Results

To allow automated integration tests to verify decoding correctness without relying on an external binary like `ffmpeg`, each media file may optionally be accompanied by a `.json` sidecar file of the same name. 

These JSON files contain the "source of truth" for the test vector, including:
- Metadata parameters (duration, sample rate, channels)
- A checksum of the raw decoded PCM audio (e.g., MD5) to verify bit-exact decoding.

## Files

### `mp3/`
- `mp3/vbr-with-crc.mp3`: A 3-second VBR MP3 file with CRC protection enabled, used to test edge cases in Xing/Info tag heuristics ([Issue #516](https://github.com/pdeljanov/Symphonia/issues/516)).
- `mp3/vbr-with-crc.json`: Expected metadata and PCM MD5 hash for the file above.
