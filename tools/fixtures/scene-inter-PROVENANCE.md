# Original finite interframe fixture provenance

The generator authors the RGB input pixels and stereo PCM16 samples from original
mathematical scene/tone formulas. It uses FFmpeg/libvpx only as development encoding
tools. Independent video references use FFmpeg's native VP8 decoder. No external
codec implementation, executable, downloaded video/audio or operating-system codec
is a production dependency.

The recorded development installation is FFmpeg
`N-124319-gb801f1fe6d-20260503` with libvpx `v1.16.0`. The FFmpeg executable reports
GPL version3 or later; it is not redistributed with these fixtures or the browser.
The JSON metadata records public-safe commands, format/timestamps and payload/
reference hashes. Generation checks run locally and print their results; reports,
temporary probes and logs are not fixture metadata.

The authored video is 64x48, 24 visible VP8 version0 I/P frames at8fps, with
keyframes0/12 and duration3 seconds. The finite Matroska file contains V_VP8 and
A_PCM/INT/LIT tracks: stereo48kHz signed little-endian PCM16, 144,000 frames and
6,000 audio frames per125ms video interval. No WebM, new audio codec, MSE or general
streaming capability is implied by these bytes.

`scene-inter.yuv` is the independent decoded YUV420P reference.
`scene-inter.rgba` retains the default FFmpeg RGBA conversion.
`scene-inter-centered.rgba` is a separate explicit centered-bilinear BT.601 display
reference, with command and profile in `scene-inter-display.json`; it does not
replace the default reference. The defined new display-profile tolerance is mean
RGB error below0.5 and maximum channel error2. Existing fixture tolerances remain
independent. These values define checks, not recorded validation outcomes.

[RFC6386 section5](https://www.rfc-editor.org/rfc/rfc6386.html#section-5) separates
decoded buffers from viewing-format postprocessing; the renderer profile makes
that presentation choice explicit. The generator and metadata do not include
decoder/reference implementation source from the RFC or an external codec.
