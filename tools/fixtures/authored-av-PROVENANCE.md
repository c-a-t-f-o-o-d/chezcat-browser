# Original local authored A/V regression pages

authored-av.html and authored-av-next.html are original project HTML/CSS/
JavaScript regression pages. They use one parser-authored VIDEO, ordinary trusted
App input and the existing source, seek and native-control paths. No site code,
downloaded media, embedded browser, runtime encoder or external decoder is used.

The pages reuse the existing scene-inter.ivf and scene-inter-av.mkv assets and
their original metadata/display references. Their mathematical scene and PCM
signal, development-only encoding tools, independent decoder reference and
display tolerance are documented in [scene-inter-PROVENANCE.md](scene-inter-PROVENANCE.md).
The fixture manifest authored-av.json records the exact reused bytes and page
hashes; it contains no external capture or operational findings.

The routine journey keeps Matroska paused. It checks real selected PCM sample
offsets, requested decoded frames, frozen output, timeline-end selection without
a completed-drain claim, same-element IVF replacement/restoration and navigation
teardown. Native IVF Play/Pause retains its separate video-only regression.
This fixture does not establish advancing PCM playback, hardware/device output,
physical audibility, network Matroska or service compatibility.
